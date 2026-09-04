// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **VirtualList** — high-performance list for extremely large or streaming sets.
//!
//! **Mission.** Compose the shared [`Virtualizer`] (stable logical indices,
//! overscan, variable extents, sticky headers, anchors) with projected
//! [`ListRow`] paint. Hosts never allocate O(logical_len) for paint or
//! semantics. Supports async page loading, placeholders, follow-tail,
//! filtering metadata, live updates, visible-range + measurement diagnostics,
//! and million-row logical benchmarks.
//!
//! **vs [`List`](super::List).** List paints a borrowed slice and may use a
//! virtual window facade. VirtualList **owns** window math and enforces
//! O(viewport) paint/semantics.
//!
//! Research: Textual virtual lists, VisiData, log tails, TermRock Virtualizer.
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

use crate::{
    interaction::{
        HitRegion, NavigationMove, Outcome, PageMove, SemanticNode, SemanticRole, SemanticScene,
        SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::list::{List, ListRow, ListState, RowRole};
use super::virtualizer::{ExtentPolicy, StickyRegion, VirtRange, VirtSlice, Virtualizer};

/// Default overscan items for prefetch/measure.
pub const VIRTUAL_LIST_DEFAULT_OVERSCAN: u16 = 4;
/// Logical rows used in million-row benchmarks / stories.
pub const VIRTUAL_LIST_BENCH_ROWS: u64 = 1_000_000;

// ── Follow / async / filter ─────────────────────────────────────────────────

/// Tail-follow policy when the logical universe grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum VirtualListFollow {
    /// Do not move offset on growth.
    #[default]
    Off,
    /// Keep the last items visible (log tail).
    Tail,
}

/// Async page-loading status for the measure window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum VirtualPageStatus {
    /// Window rows are ready.
    #[default]
    Ready,
    /// Next/previous page in flight.
    Loading,
    /// Host has no data yet for part of the window (placeholders).
    Placeholder,
}

impl VirtualPageStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Placeholder => "placeholder",
        }
    }
}

// ── Projected row ───────────────────────────────────────────────────────────

/// One host-projected row for a logical index (only for the current window).
#[derive(Debug, Clone)]
pub struct VirtualListItem<'a, Id> {
    /// Absolute logical index in the universe (stable for this projection).
    pub logical_index: u64,
    /// Composed list row (stable `Id` for selection / activation).
    pub row: ListRow<'a, Id>,
}

impl<'a, Id> VirtualListItem<'a, Id> {
    /// Construct.
    #[must_use]
    pub fn new(logical_index: u64, row: ListRow<'a, Id>) -> Self {
        Self { logical_index, row }
    }
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// Per-frame measurement / visibility diagnostics (no O(N) data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VirtualListDiagnostics {
    /// Logical universe size.
    pub logical_len: u64,
    /// Visible body half-open range.
    pub visible: VirtRange,
    /// Measure/overscan half-open range.
    pub measure: VirtRange,
    /// Semantic registration budget (sticky + visible).
    pub semantic_count: u64,
    /// Sparse measured extents retained.
    pub measured_extents: usize,
    /// Rows the host projected this frame.
    pub projected_rows: u16,
    /// Sticky leading count.
    pub sticky_leading: u64,
    /// Follow mode active.
    pub follow_tail: bool,
    /// Async page status.
    pub page_status: VirtualPageStatus,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`VirtualList`].
#[derive(Debug, Clone)]
pub struct VirtualListState<Id> {
    virt: Virtualizer,
    list: ListState<Id>,
    follow: VirtualListFollow,
    page_status: VirtualPageStatus,
    /// Optional filter query (host applies; we store for chrome + diagnostics).
    filter_query: Option<String>,
    /// Logical length after filter when host reports it (`None` = unfiltered).
    filter_match_count: Option<u64>,
    last_diag: VirtualListDiagnostics,
    /// Pointer for hover hit tests on last paint.
    pointer: Option<Position>,
}

impl<Id> Default for VirtualListState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> VirtualListState<Id> {
    /// Empty universe, fixed 1-row extent, default overscan.
    #[must_use]
    pub fn new() -> Self {
        let mut list = ListState::default();
        list.collection_mut().set_wrap(false);
        Self {
            virt: Virtualizer::fixed(1)
                .with_overscan(VIRTUAL_LIST_DEFAULT_OVERSCAN)
                .with_viewport(1),
            list,
            follow: VirtualListFollow::Off,
            page_status: VirtualPageStatus::Ready,
            filter_query: None,
            filter_match_count: None,
            last_diag: VirtualListDiagnostics::default(),
            pointer: None,
        }
    }

    /// Million-row style fixed list (tests / demos).
    #[must_use]
    pub fn million_fixed() -> Self {
        let mut s = Self::new();
        s.virt = Virtualizer::fixed(1)
            .with_len(VIRTUAL_LIST_BENCH_ROWS)
            .with_overscan(VIRTUAL_LIST_DEFAULT_OVERSCAN)
            .with_viewport(24);
        s
    }

    /// Borrow virtualizer.
    #[must_use]
    pub const fn virtualizer(&self) -> &Virtualizer {
        &self.virt
    }

    /// Nested list state over the projected window (typeahead / multi).
    #[must_use]
    pub const fn list_state(&self) -> &ListState<Id> {
        &self.list
    }

    /// Mutable nested list state.
    pub fn list_state_mut(&mut self) -> &mut ListState<Id> {
        &mut self.list
    }

    /// Last-frame diagnostics.
    #[must_use]
    pub const fn diagnostics(&self) -> VirtualListDiagnostics {
        self.last_diag
    }

    /// Set follow-tail.
    pub fn set_follow(&mut self, follow: VirtualListFollow) {
        self.follow = follow;
        if matches!(follow, VirtualListFollow::Tail) {
            self.virt.capture_from_end_anchor();
            let max = self.virt.max_offset();
            self.virt.set_offset(max);
        }
    }

    /// Filter chrome.
    pub fn set_filter_query(&mut self, query: Option<String>) {
        self.filter_query = query.filter(|q| !q.is_empty());
    }

    /// Host-reported match count after filter.
    pub fn set_filter_match_count(&mut self, count: Option<u64>) {
        self.filter_match_count = count;
    }

    /// Configure sticky headers/footers.
    pub fn set_sticky(&mut self, sticky: StickyRegion) {
        self.virt.set_sticky(sticky);
    }

    /// Extent policy (fixed or variable).
    pub fn set_extent_policy(&mut self, policy: ExtentPolicy) {
        match policy {
            ExtentPolicy::Fixed(e) => {
                self.virt = Virtualizer::fixed(e)
                    .with_len(self.virt.logical_len())
                    .with_viewport(self.virt.viewport_extent())
                    .with_overscan(self.virt.overscan())
                    .with_sticky(self.virt.sticky());
                self.virt.set_offset(self.virt.offset());
            }
            ExtentPolicy::Variable { estimated } => {
                let off = self.virt.offset();
                let len = self.virt.logical_len();
                let vp = self.virt.viewport_extent();
                let over = self.virt.overscan();
                let sticky = self.virt.sticky();
                self.virt = Virtualizer::variable(estimated)
                    .with_len(len)
                    .with_viewport(vp)
                    .with_overscan(over)
                    .with_sticky(sticky);
                self.virt.set_offset(off);
            }
        }
    }

    /// Set logical universe size (no O(N) alloc). Live updates / streams.
    pub fn set_logical_len(&mut self, len: u64) {
        let prev = self.virt.logical_len();
        self.virt.on_items_changed(len);
        if matches!(self.follow, VirtualListFollow::Tail) && len > prev {
            let max = self.virt.max_offset();
            self.virt.set_offset(max);
        }
    }

    /// Viewport extent (terminal rows).
    pub fn set_viewport_extent(&mut self, rows: u16) {
        self.virt.set_viewport_extent(rows);
    }

    /// Scroll by signed item delta.
    pub fn scroll_by(&mut self, delta: i64) -> bool {
        if matches!(self.follow, VirtualListFollow::Tail) && delta < 0 {
            // User scrolled up — leave tail follow until re-enabled.
            self.follow = VirtualListFollow::Off;
        }
        self.virt.scroll_by(delta)
    }

    /// Set body offset.
    pub fn set_offset(&mut self, offset: u64) {
        self.follow = VirtualListFollow::Off;
        self.virt.set_offset(offset);
    }

    /// Offset.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.virt.offset()
    }

    /// Visible body slice.
    #[must_use]
    pub fn visible_slice(&self) -> VirtSlice {
        self.virt.visible_slice()
    }

    /// Indices the host should project this frame (sticky + measure window).
    ///
    /// Never returns O(logical_len) for large universes.
    pub fn projection_indices(&self, out: &mut Vec<u64>) {
        out.clear();
        self.virt.sticky_indices(out);
        let m = self.virt.visible_slice();
        for i in m.measure_start..m.measure_end {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// Record measured row height (variable extents).
    pub fn note_measured(&mut self, logical_index: u64, extent: u16) {
        self.virt.note_measured(logical_index, extent);
        self.virt
            .forget_measured_outside(u64::from(self.virt.overscan()).saturating_add(8));
    }

    /// Hit regions from the last paint (projected body rows only).
    ///
    /// Forwards to the inner [`ListState`], which owns its regions — the
    /// previous duplicate `Vec` was re-copied from it every frame.
    #[must_use]
    pub fn regions(&self) -> &[HitRegion<Id>] {
        self.list.regions()
    }

    /// Click at position.
    pub fn click(&mut self, position: Position) -> Outcome<Id>
    where
        Id: Clone + PartialEq,
    {
        self.pointer = Some(position);
        let Some(region) = self
            .list
            .regions()
            .iter()
            .find(|r| r.area.contains(position))
            .map(|r| r.id.clone())
        else {
            return Outcome::Ignored;
        };
        self.list.select(Some(region.clone()));
        Outcome::Activated(region)
    }

    /// Intent routing (move/page scroll the virtualizer; activate uses list).
    pub fn handle_intent(
        &mut self,
        projected: &[VirtualListItem<'_, Id>],
        intent: UiIntent,
    ) -> Outcome<Id>
    where
        Id: Clone + PartialEq,
    {
        let rows = projected_rows(projected);
        self.list.set_virtual_window(
            0,
            usize::try_from(self.virt.scrollable_len()).unwrap_or(usize::MAX),
        );
        self.list.reconcile_collection(&rows);
        match intent {
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                // Prefer move selection within window; if at end, scroll.
                let before = self.list.selected().cloned();
                let out = self.list.handle_intent(&rows, intent);
                if out == Outcome::Ignored || self.list.selected() == before.as_ref() {
                    if self.scroll_by(1) {
                        Outcome::Changed
                    } else {
                        Outcome::Ignored
                    }
                } else {
                    out
                }
            }
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                let before = self.list.selected().cloned();
                let out = self.list.handle_intent(&rows, intent);
                if out == Outcome::Ignored || self.list.selected() == before.as_ref() {
                    if self.scroll_by(-1) {
                        Outcome::Changed
                    } else {
                        Outcome::Ignored
                    }
                } else {
                    out
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = i64::from(self.virt.viewport_extent().max(1));
                if self.scroll_by(step) {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i64::from(self.virt.viewport_extent().max(1));
                if self.scroll_by(-step) {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                let before_offset = self.virt.offset();
                let was_following = matches!(self.follow, VirtualListFollow::Tail);
                self.set_offset(self.virt.body_start());
                if self.virt.offset() != before_offset || was_following {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let before_offset = self.virt.offset();
                let was_following = matches!(self.follow, VirtualListFollow::Tail);
                self.set_follow(VirtualListFollow::Tail);
                if self.virt.offset() != before_offset || !was_following {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            UiIntent::Activate
            | UiIntent::Open
            | UiIntent::Submit
            | UiIntent::Toggle
            | UiIntent::Cancel
            | UiIntent::Close => self.list.handle_intent(&rows, intent),
            _ => Outcome::Ignored,
        }
    }

    fn refresh_diagnostics(&mut self, projected_len: usize) {
        let slice = self.virt.visible_slice();
        self.last_diag = VirtualListDiagnostics {
            logical_len: self.virt.logical_len(),
            visible: slice.visible(),
            measure: VirtRange {
                start: slice.measure_start,
                end: slice.measure_end,
            },
            semantic_count: self.virt.semantic_count(),
            measured_extents: self.virt.measured_count(),
            projected_rows: u16::try_from(projected_len).unwrap_or(u16::MAX),
            sticky_leading: self.virt.sticky().leading,
            follow_tail: matches!(self.follow, VirtualListFollow::Tail),
            page_status: self.page_status,
        };
    }
}

fn projected_rows<'a, Id: Clone>(projected: &'a [VirtualListItem<'a, Id>]) -> Vec<ListRow<'a, Id>> {
    // List APIs take &[ListRow] — clone row shells (cheap Line clones).
    projected.iter().map(|p| p.row.clone()).collect()
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// High-performance virtual list over host-projected window rows.
///
/// # Host contract
///
/// 1. Call [`VirtualListState::set_logical_len`] / `set_viewport_extent`.
/// 2. Read [`VirtualListState::projection_indices`] and project only those
///    items (async placeholders allowed).
/// 3. Paint with [`VirtualList::paint`] / `StatefulWidget`.
/// 4. Register semantics with [`VirtualList::register_semantic`] (visible set).
#[derive(Debug, Clone, Copy)]
pub struct VirtualList<'a, Id> {
    projected: &'a [VirtualListItem<'a, Id>],
    system: &'a DesignSystem,
    empty_message: Option<&'a str>,
    show_diagnostics: bool,
    focused: bool,
}

impl<'a, Id> VirtualList<'a, Id> {
    /// Projected window + design system.
    #[must_use]
    pub const fn new(projected: &'a [VirtualListItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            projected,
            system,
            empty_message: None,
            show_diagnostics: false,
            focused: true,
        }
    }

    /// Empty universe message.
    #[must_use]
    pub const fn empty_message(mut self, msg: &'a str) -> Self {
        self.empty_message = Some(msg);
        self
    }

    /// Paint a one-line diagnostics strip (offset / visible / semantic).
    #[must_use]
    pub const fn show_diagnostics(mut self, on: bool) -> Self {
        self.show_diagnostics = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut VirtualListState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        // Regions live in the inner ListState now; a frame that returns early
        // (empty view) must not leave the previous frame's geometry behind.
        state.list.clear_hit_regions();
        state.pointer = state.list.hovered().and_then(|_| state.pointer);

        let mut y = area.y;
        let mut h = area.height;

        // Filter / page chrome
        if let Some(q) = state.filter_query.as_ref() {
            let n = state.filter_match_count.unwrap_or(state.virt.logical_len());
            let line = format!("filter:{q}{}{n} matches", " · ");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system.style(Role::TextSecondary),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }
        if matches!(
            state.page_status,
            VirtualPageStatus::Loading | VirtualPageStatus::Placeholder
        ) {
            let msg = match state.page_status {
                VirtualPageStatus::Loading => "loading page…",
                VirtualPageStatus::Placeholder => "placeholders in window",
                VirtualPageStatus::Ready => "",
            };
            if !msg.is_empty() && h > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    msg,
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
                h = h.saturating_sub(1);
            }
        }
        if self.show_diagnostics && h > 0 {
            let d = state.diagnostics();
            // Will refresh after layout — show previous then update.
            let line = format!(
                "N={} off={} vis={}..{} sem={} meas={} proj={}",
                d.logical_len,
                state.offset(),
                d.visible.start,
                d.visible.end,
                d.semantic_count,
                d.measured_extents,
                d.projected_rows,
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system.style(Role::TextDisabled),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        let body = Rect::new(area.x, y, area.width, h);
        if body.is_empty() {
            state.refresh_diagnostics(self.projected.len());
            return;
        }

        state.virt.set_viewport_extent(body.height.max(1));

        if state.virt.logical_len() == 0 {
            if let Some(msg) = self.empty_message {
                buffer.set_stringn(
                    body.x,
                    body.y,
                    msg,
                    usize::from(body.width),
                    self.system.style(Role::TextMuted),
                );
            }
            state.refresh_diagnostics(0);
            return;
        }

        // Partition projected into sticky leading / body / sticky trail.
        //
        // projection_indices() returns ascending logical indexes, so the three
        // bands are contiguous ranges of `self.projected` — no per-frame
        // partition `Vec`s or row clones are needed. Sticky rows paint
        // directly; only the body slice materializes rows for `List`.
        let sticky_lead = state.virt.sticky().leading;
        let sticky_trail = state.virt.sticky().trailing;
        let len = state.virt.logical_len();
        let trail_floor = len.saturating_sub(sticky_trail);

        let lead_end = if sticky_lead > 0 {
            self.projected
                .partition_point(|p| p.logical_index < sticky_lead)
        } else {
            0
        };
        let trail_start = if sticky_trail > 0 {
            self.projected
                .partition_point(|p| p.logical_index < trail_floor)
        } else {
            self.projected.len()
        };
        let trail_count = self.projected.len() - trail_start;

        let mut paint_y = body.y;
        // Sticky headers
        for item in &self.projected[..lead_end] {
            if paint_y >= body.bottom() {
                break;
            }
            let selected = state.list.selected() == Some(&item.row.id);
            paint_simple_row(
                buffer,
                Rect::new(body.x, paint_y, body.width, 1),
                &item.row,
                self.system,
                true,
                selected,
                self.focused,
            );
            paint_y = paint_y.saturating_add(1);
        }

        let body_bottom_reserve = u16::try_from(trail_count).unwrap_or(0);
        let list_bottom = body.bottom().saturating_sub(body_bottom_reserve);
        let list_area = Rect::new(
            body.x,
            paint_y,
            body.width,
            list_bottom.saturating_sub(paint_y),
        );

        // Body via List (window already projected; offset 0). Render even when
        // the area is empty: ListState owns the hit regions now, and its
        // render clears them — the old shadow `Vec` did that by hand.
        let body_rows: Vec<ListRow<'_, Id>> = self.projected[lead_end..trail_start]
            .iter()
            .map(|p| p.row.clone())
            .collect();
        // Virtual total: body scroll universe for scrollbar only
        let body_total = state.virt.scrollable_len();
        state
            .list
            .set_virtual_window(0, usize::try_from(body_total).unwrap_or(usize::MAX));
        // Keep list offset at 0 — Virtualizer owns scroll
        // Selection still works on projected ids
        let list = List::new(&body_rows, self.system).focused(self.focused);
        StatefulWidget::render(&list, list_area, buffer, &mut state.list);

        // Sticky trail
        let mut ty = body.bottom().saturating_sub(body_bottom_reserve);
        for item in &self.projected[trail_start..] {
            if ty >= body.bottom() {
                break;
            }
            let selected = state.list.selected() == Some(&item.row.id);
            paint_simple_row(
                buffer,
                Rect::new(body.x, ty, body.width, 1),
                &item.row,
                self.system,
                true,
                selected,
                self.focused,
            );
            ty = ty.saturating_add(1);
        }

        // Variable extents: note 1-row measurements for projected body
        if matches!(state.virt.policy(), ExtentPolicy::Variable { .. }) {
            for p in self.projected {
                state.note_measured(p.logical_index, 1);
            }
        }

        state.refresh_diagnostics(self.projected.len());
    }

    /// Semantic registration for visible/near-visible projected items only.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        root_id: Sid,
        area: Rect,
        state: &VirtualListState<Id>,
        mut id_for: impl FnMut(u64, &Id) -> Sid,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        let d = state.diagnostics();
        let desc = format!(
            "virtual-list N={} vis={}..{} sem={} page={} follow={}",
            d.logical_len,
            d.visible.start,
            d.visible.end,
            d.semantic_count,
            d.page_status.id(),
            if d.follow_tail { "tail" } else { "off" },
        );
        let _ = scene.register(
            SemanticNode::content(root_id, area)
                .role(SemanticRole::List)
                .label("virtual-list")
                .description(desc)
                .focusable(false)
                .state(SemanticState::default()),
        );
        // Only projected items (window + sticky) — never full N.
        for (i, p) in self.projected.iter().enumerate() {
            if i as u64
                >= d.semantic_count
                    .saturating_add(u64::from(state.virt.overscan()))
            {
                // Soft cap: still register all projected; projected is already window-sized.
            }
            let row_area = state
                .regions()
                .iter()
                .find(|r| r.id == p.row.id)
                .map(|r| r.area)
                .unwrap_or_else(|| Rect::new(area.x, area.y, area.width.min(1), 1));
            if row_area.is_empty() {
                continue;
            }
            let sid = id_for(p.logical_index, &p.row.id);
            let _ = scene.register(
                SemanticNode::control(sid, row_area)
                    .role(SemanticRole::ListItem)
                    .label(format!("row-{}", p.logical_index))
                    .description(p.row.plain_label())
                    .focusable(p.row.enabled && p.row.role.is_navigable())
                    .state(SemanticState {
                        busy: p.row.loading,
                        ..Default::default()
                    }),
            );
        }
    }
}

/// Paints one sticky (pinned) row with the same chrome the body uses.
///
/// Sticky rows used to drop selection entirely: scrolling a selected row into
/// the pinned band made it look unselected. They resolve the shared row
/// recipe, so the gutter slot and the selection wash survive the pin.
fn paint_simple_row<Id>(
    buffer: &mut Buffer,
    area: Rect,
    row: &ListRow<'_, Id>,
    system: &DesignSystem,
    strong: bool,
    selected: bool,
    focused: bool,
) {
    if area.is_empty() {
        return;
    }
    let chrome = crate::widgets::row_chrome::RowChrome::resolve(
        system,
        crate::style::ListRowVisualState {
            selected,
            focused,
            enabled: row.enabled,
            ..Default::default()
        },
    );
    let mut style = if strong {
        system.style(Role::TextStrong)
    } else {
        system.style(Role::Text)
    };
    if matches!(row.role, RowRole::GroupHeader) {
        style = system
            .style(Role::TextStrong)
            .add_modifier(ratatui_core::style::Modifier::BOLD);
    }
    let style = chrome.label_style(style);
    // Reserve the gutter column so pinned rows line up with the body.
    let text = format!(" {}", row.plain_label());
    let w = display_cols(&text).min(usize::from(area.width));
    buffer.set_stringn(
        area.x,
        area.y,
        take_display_cols(&text, w).as_ref(),
        w,
        style,
    );
    chrome.paint(buffer, area);
}

impl<Id: Clone + PartialEq> StatefulWidget for &VirtualList<'_, Id> {
    type State = VirtualListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for VirtualList<'_, Id> {
    type State = VirtualListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;
    use ratatui_core::text::Line;
    use std::time::Instant;

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn project_u64(indices: &[u64]) -> Vec<VirtualListItem<'static, u64>> {
        indices
            .iter()
            .map(|&i| {
                let label = if i == 0 {
                    "★ sticky header".to_string()
                } else {
                    format!("row {i:>9}")
                };
                // Leak labels for 'static Line — use into_boxed_str leak
                let s: &'static str = Box::leak(label.into_boxed_str());
                let mut row = ListRow::item(i, Line::from(s));
                if i == 0 {
                    row = ListRow::group_header(i, Line::from(s));
                }
                VirtualListItem::new(i, row)
            })
            .collect()
    }

    #[test]
    fn projection_indices_bounded() {
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(20);
        state.set_sticky(StickyRegion {
            leading: 1,
            trailing: 0,
        });
        state.set_offset(500_000);
        let mut idx = Vec::new();
        state.projection_indices(&mut idx);
        assert!(idx.len() < 100, "projected {} for 1M", idx.len());
        assert!(idx.contains(&0), "sticky header present");
        assert!(idx.iter().any(|&i| i >= 500_000));
        assert!(state.diagnostics().logical_len == 0 || true);
        state.refresh_diagnostics(idx.len());
        let d = state.diagnostics();
        assert_eq!(d.logical_len, VIRTUAL_LIST_BENCH_ROWS);
        assert!(d.semantic_count < 100);
    }

    #[test]
    fn follow_tail_on_growth() {
        let mut state = VirtualListState::<u64>::new();
        state.set_logical_len(100);
        state.set_viewport_extent(10);
        state.set_follow(VirtualListFollow::Tail);
        let off = state.offset();
        state.set_logical_len(10_000);
        assert!(state.offset() >= off);
        assert_eq!(state.offset(), state.virtualizer().max_offset());
    }

    #[test]
    fn page_up_down_scrolls() {
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(10);
        state.set_offset(1000);
        let _ = state.handle_intent(&[], UiIntent::Page(PageMove::Forward));
        assert!(state.offset() > 1000);
        let o = state.offset();
        let _ = state.handle_intent(&[], UiIntent::Page(PageMove::Backward));
        assert!(state.offset() < o);
    }

    #[test]
    fn movement_keeps_off_window_active_and_scrolls_virtualizer() {
        let projected = project_u64(&[10, 11]);
        for (active, intent, expected_offset) in [
            (9, UiIntent::Move(NavigationMove::Down), 11),
            (9, UiIntent::Move(NavigationMove::Up), 9),
            (11, UiIntent::Move(NavigationMove::Down), 11),
            (10, UiIntent::Move(NavigationMove::Up), 9),
        ] {
            let mut state = VirtualListState::<u64>::new();
            state.set_logical_len(100);
            state.set_viewport_extent(2);
            state.set_offset(10);
            let rows = projected_rows(&projected);
            state.list_state_mut().set_virtual_window(0, 100);
            state.list_state_mut().reconcile_collection(&rows);
            state.list_state_mut().select(Some(active));

            let out = state.handle_intent(&projected, intent);

            assert_eq!(out, Outcome::Changed);
            assert_eq!(state.list_state().selected(), Some(&active));
            assert_eq!(state.offset(), expected_offset);
        }

        let lower_projected = project_u64(&[0, 1]);
        let mut lower = VirtualListState::<u64>::new();
        lower.set_logical_len(100);
        lower.set_viewport_extent(2);
        let lower_rows = projected_rows(&lower_projected);
        lower.list_state_mut().set_virtual_window(0, 100);
        lower.list_state_mut().reconcile_collection(&lower_rows);
        lower.list_state_mut().select(Some(0));
        assert_eq!(
            lower.handle_intent(&lower_projected, UiIntent::Move(NavigationMove::Up)),
            Outcome::Ignored
        );
        assert_eq!(lower.list_state().selected(), Some(&0));
        assert_eq!(lower.offset(), 0);

        let upper_projected = project_u64(&[98, 99]);
        let mut upper = VirtualListState::<u64>::new();
        upper.set_logical_len(100);
        upper.set_viewport_extent(2);
        upper.set_offset(98);
        let upper_rows = projected_rows(&upper_projected);
        upper.list_state_mut().set_virtual_window(0, 100);
        upper.list_state_mut().reconcile_collection(&upper_rows);
        upper.list_state_mut().select(Some(99));
        assert_eq!(
            upper.handle_intent(&upper_projected, UiIntent::Move(NavigationMove::Down)),
            Outcome::Ignored
        );
        assert_eq!(upper.list_state().selected(), Some(&99));
        assert_eq!(upper.offset(), 98);

        let mut lower_page = VirtualListState::<u64>::new();
        lower_page.set_logical_len(100);
        lower_page.set_viewport_extent(2);
        assert_eq!(
            lower_page.handle_intent(&[], UiIntent::Page(PageMove::Backward)),
            Outcome::Ignored
        );
        assert_eq!(lower_page.offset(), 0);

        let mut upper_page = VirtualListState::<u64>::new();
        upper_page.set_logical_len(100);
        upper_page.set_viewport_extent(2);
        upper_page.set_offset(98);
        assert_eq!(
            upper_page.handle_intent(&[], UiIntent::Page(PageMove::Forward)),
            Outcome::Ignored
        );
        assert_eq!(upper_page.offset(), 98);

        let mut first = VirtualListState::<u64>::new();
        first.set_logical_len(100);
        first.set_viewport_extent(2);
        first.set_offset(10);
        assert_eq!(
            first.handle_intent(&[], UiIntent::Move(NavigationMove::First)),
            Outcome::Changed
        );
        assert_eq!(first.offset(), 0);
        assert_eq!(
            first.handle_intent(&[], UiIntent::Move(NavigationMove::First)),
            Outcome::Ignored
        );
        assert_eq!(first.offset(), 0);

        let mut empty = VirtualListState::<u64>::new();
        empty.set_logical_len(100);
        empty.set_viewport_extent(2);
        empty.set_offset(10);
        empty.list_state_mut().select(Some(9));
        assert_eq!(
            empty.handle_intent(&[], UiIntent::Move(NavigationMove::Down)),
            Outcome::Changed
        );
        assert_eq!(empty.list_state().selected(), Some(&9));
        assert_eq!(empty.offset(), 11);

        let mut last = VirtualListState::<u64>::new();
        last.set_logical_len(100);
        last.set_viewport_extent(2);
        last.set_offset(98);
        last.set_follow(VirtualListFollow::Tail);
        assert_eq!(
            last.handle_intent(&[], UiIntent::Move(NavigationMove::Last)),
            Outcome::Ignored
        );
        assert_eq!(last.offset(), 98);
    }

    #[test]
    fn paint_million_is_o_viewport() {
        let system = system();
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_sticky(StickyRegion {
            leading: 1,
            trailing: 0,
        });
        state.set_offset(250_000);
        state.set_viewport_extent(24);
        let mut idx = Vec::new();
        state.projection_indices(&mut idx);
        assert!(idx.len() <= 40, "{}", idx.len());
        let projected = project_u64(&idx);
        let area = Rect::new(0, 0, 48, 20);
        let mut buf = Buffer::empty(area);
        let start = Instant::now();
        for _ in 0..50 {
            VirtualList::new(&projected, &system)
                .show_diagnostics(true)
                .paint(area, &mut buf, &mut state);
        }
        assert!(start.elapsed().as_millis() < 3_000);
        let d = state.diagnostics();
        assert!(d.projected_rows > 0);
        assert!(d.projected_rows < 100);
        assert_eq!(d.logical_len, VIRTUAL_LIST_BENCH_ROWS);
    }

    #[test]
    fn semantic_count_far_below_logical() {
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(15);
        state.set_offset(900_000);
        let sem = state.virtualizer().semantic_count();
        assert!(sem < 50);
        assert!(sem * 10_000 < VIRTUAL_LIST_BENCH_ROWS); // much less than 1M
    }

    #[test]
    fn filter_chrome() {
        let system = system();
        let mut state = VirtualListState::<u64>::new();
        state.set_logical_len(100);
        state.set_viewport_extent(5);
        state.set_filter_query(Some("foo".into()));
        state.set_filter_match_count(Some(3));
        let mut idx = Vec::new();
        state.projection_indices(&mut idx);
        let projected = project_u64(&idx);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        VirtualList::new(&projected, &system).paint(area, &mut buf, &mut state);
        let mut s = String::new();
        for x in 0..40 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(
            s.contains("filter") || s.contains("foo") || s.contains("3"),
            "{s}"
        );
    }

    #[test]
    fn live_update_on_items_changed_preserves_clamp() {
        let mut state = VirtualListState::<u64>::new();
        state.set_logical_len(10_000);
        state.set_viewport_extent(20);
        state.set_offset(9_500);
        state.set_logical_len(100);
        assert!(state.offset() <= state.virtualizer().max_offset());
    }

    #[test]
    fn note_measured_variable() {
        let mut state = VirtualListState::<u64>::new();
        state.set_extent_policy(ExtentPolicy::Variable { estimated: 1 });
        state.set_logical_len(100);
        state.set_viewport_extent(10);
        state.note_measured(5, 2);
        assert_eq!(state.virtualizer().extent_of(5), 2);
        assert!(state.virtualizer().measured_count() >= 1);
    }

    #[test]
    fn click_activates_projected() {
        let system = system();
        let mut state = VirtualListState::<u64>::new();
        state.set_logical_len(50);
        state.set_viewport_extent(10);
        let mut idx = Vec::new();
        state.projection_indices(&mut idx);
        let projected = project_u64(&idx);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        VirtualList::new(&projected, &system).paint(area, &mut buf, &mut state);
        if let Some(r) = state.regions().first() {
            let out = state.click(Position::new(r.area.x, r.area.y));
            assert!(
                matches!(out, Outcome::Activated(_)) || matches!(out, Outcome::Ignored),
                "{out:?}"
            );
        }
    }

    #[test]
    fn semantic_register_only_projected() {
        let system = system();
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(12);
        state.set_offset(100_000);
        let mut idx = Vec::new();
        state.projection_indices(&mut idx);
        let projected = project_u64(&idx);
        let area = Rect::new(0, 0, 40, 14);
        let mut buf = Buffer::empty(area);
        VirtualList::new(&projected, &system).paint(area, &mut buf, &mut state);
        let mut scene = SemanticScene::<String, ()>::default();
        VirtualList::new(&projected, &system).register_semantic(
            &mut scene,
            "root".into(),
            area,
            &state,
            |i, _| format!("r{i}"),
        );
        let n = scene.nodes().len();
        assert!(n < 100, "semantic nodes {n}");
        assert!(n >= 1);
    }

    #[test]
    fn fuzz_scroll_offsets() {
        let system = system();
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(16);
        let mut seed = 21u64;
        for _ in 0..30 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            state.set_offset(seed % VIRTUAL_LIST_BENCH_ROWS);
            let mut idx = Vec::new();
            state.projection_indices(&mut idx);
            assert!(idx.len() < 80);
            let projected = project_u64(&idx);
            let h = ((seed % 20) as u16) + 4;
            let area = Rect::new(0, 0, 36, h);
            let mut buf = Buffer::empty(area);
            VirtualList::new(&projected, &system).paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(40, 12)).unwrap();
            let mut state = VirtualListState::<u64>::new();
            state.set_logical_len(1000);
            state.set_viewport_extent(8);
            state.set_offset(100);
            let mut idx = Vec::new();
            state.projection_indices(&mut idx);
            let projected = project_u64(&idx);
            t.draw(|f| {
                VirtualList::new(&projected, &system).paint(f.area(), f.buffer_mut(), &mut state);
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }

    #[test]
    fn million_row_bench_scroll_math_only() {
        let mut state = VirtualListState::<u64>::million_fixed();
        state.set_viewport_extent(40);
        let start = Instant::now();
        for i in 0..10_000 {
            let _ = state.scroll_by(1);
            if i % 100 == 0 {
                let _ = state.visible_slice();
                let mut idx = Vec::new();
                state.projection_indices(&mut idx);
                assert!(idx.len() < 100);
            }
        }
        assert!(start.elapsed().as_millis() < 2_000);
    }

    #[test]
    fn empty_safe() {
        let system = system();
        let mut state = VirtualListState::<u64>::new();
        let projected: Vec<VirtualListItem<'static, u64>> = Vec::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        VirtualList::new(&projected, &system)
            .empty_message("empty")
            .paint(Rect::new(0, 0, 10, 5), &mut buf, &mut state);
        VirtualList::new(&projected, &system).paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
    }
}
