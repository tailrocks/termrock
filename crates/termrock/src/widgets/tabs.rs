// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composable tab strip for views, documents, and question groups.
//!
//! **Mission.** Multi-panel surfaces need a stable tab list with roving focus,
//! badges/status, close hooks, overflow, and narrow contraction — while each
//! **panel's internal state stays host-owned** (keyed by tab id).
//!
//! **vs [`SegmentedControl`](super::SegmentedControl).** SegmentedControl switches
//! mode on a **single** surface. Tabs imply **separate content panels** (lazy or
//! preserved) and richer chrome (close, status, overflow menu).
//!
//! **vs navigation lists / Sidebar.** Sidebar is primary app routing with
//! hierarchy; Tabs are local view switches within a workspace region.
//!
//! **Activation.** [`TabsActivation::Automatic`] selects on focus move (browser-
//! like). [`TabsActivation::Manual`] moves focus with arrows and activates with
//! Enter (Radix manual).
//!
//! **Narrow.** Scrolling window → overflow `…` menu → Select-like trigger.
//!
//! Research: Radix Tabs, terminal editors, Zellij, Posting, browser tab overflow.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Span,
    widgets::StatefulWidget,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, HitRegion, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Glyph, Role},
    text::{display_cols, take_display_cols},
};

/// Single space between adjacent horizontal tab cells.
pub const TAB_GAP: u16 = 1;
/// Width under which strip prefers Select-like collapse.
pub const TABS_SELECT_MAX_WIDTH: u16 = 18;
/// Width under which overflow `…` is preferred over full expand.
pub const TABS_OVERFLOW_MAX_WIDTH: u16 = 42;

// ── Geometry helpers (preserved) ────────────────────────────────────────────

/// Per-tab descriptor shared by terminal tab renderers.
#[derive(Debug, Clone)]
pub struct TabCell<'a> {
    /// Caller-visible label.
    pub label: &'a str,
    /// Whether this painted tab is active.
    pub active: bool,
    /// First painted display column relative to the tab strip.
    pub start_col: u16,
    /// Number of painted display columns in the tab hit region.
    pub cell_cols: u16,
}

/// Build tab-cell geometry from `(label, active)` pairs.
#[must_use]
pub fn lay_out_tabs<'a>(labels: &[(&'a str, bool)], start_col: u16) -> Vec<TabCell<'a>> {
    let mut col = start_col;
    let mut out = Vec::with_capacity(labels.len());
    for &(label, active) in labels {
        let label_cols = u16::try_from(UnicodeWidthStr::width(label)).unwrap_or(u16::MAX);
        let cell_cols = label_cols.saturating_add(2);
        out.push(TabCell {
            label,
            active,
            start_col: col,
            cell_cols,
        });
        col = col.saturating_add(cell_cols).saturating_add(TAB_GAP);
    }
    out
}

/// Index of the tab cell whose column range contains `col`.
#[must_use]
pub fn tab_at_column(cells: &[TabCell<'_>], col: u16) -> Option<usize> {
    cells.iter().position(|cell| {
        col >= cell.start_col && col < cell.start_col.saturating_add(cell.cell_cols)
    })
}

// ── Model ───────────────────────────────────────────────────────────────────

/// Strip orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TabsOrientation {
    /// Left-to-right strip (default).
    #[default]
    Horizontal,
    /// Top-to-bottom stack.
    Vertical,
}

impl TabsOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// When focus becomes the selected panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TabsActivation {
    /// Arrow/focus movement also selects (automatic).
    #[default]
    Automatic,
    /// Arrows move focus only; Enter / click activates.
    Manual,
}

impl TabsActivation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

/// Layout presentation under width pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TabsPresentation {
    /// All enabled tabs visible (or scroll window covers them).
    #[default]
    Expanded,
    /// Horizontal scroll window over tabs.
    Scrolling,
    /// Some tabs behind overflow `…`.
    Overflow,
    /// Collapsed Select-like trigger (host paints menu from overflow ids).
    Select,
}

impl TabsPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Scrolling => "scrolling",
            Self::Overflow => "overflow",
            Self::Select => "select",
        }
    }
}

/// Non-color status for a tab (glyph when width allows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TabStatus {
    /// No status mark.
    #[default]
    None,
    /// Busy / running.
    Running,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// Unsaved / dirty.
    Dirty,
}

impl TabStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Running => "running",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Dirty => "dirty",
        }
    }

    /// ASCII / unicode mark.
    #[must_use]
    pub const fn mark(self, ascii: bool) -> Option<&'static str> {
        match (self, ascii) {
            (Self::None, _) => None,
            (Self::Running, true) => Some("*"),
            (Self::Running, false) => Some("●"),
            (Self::Success, true) => Some("+"),
            (Self::Success, false) => Some("✓"),
            (Self::Warning, true) => Some("!"),
            (Self::Warning, false) => Some("⚠"),
            (Self::Error, true) => Some("x"),
            (Self::Error, false) => Some("✗"),
            (Self::Dirty, true) => Some("."),
            (Self::Dirty, false) => Some("•"),
        }
    }
}

/// A stable tab label and presentation flags (borrowed frame projection).
///
/// Panel content is **not** stored here — host keeps per-id panel state.
#[derive(Debug, Clone)]
pub struct Tab<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: &'a str,
    /// Optional styled glyph displayed before the label (status stories).
    pub glyph: Option<Span<'a>>,
    /// Optional badge text after label (`3`, `ERR`).
    pub badge: Option<&'a str>,
    /// Semantic status (non-color mark when glyphs shown).
    pub status: TabStatus,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Show close affordance and emit [`TabsOutcome::CloseRequested`].
    pub closable: bool,
}

impl<'a, Id> Tab<'a, Id> {
    /// Enabled tab with label.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            glyph: None,
            badge: None,
            status: TabStatus::None,
            enabled: true,
            closable: false,
        }
    }

    /// Glyph span.
    #[must_use]
    pub fn glyph(mut self, glyph: Span<'a>) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Badge.
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: TabStatus) -> Self {
        self.status = status;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Closable.
    #[must_use]
    pub const fn closable(mut self, on: bool) -> Self {
        self.closable = on;
        self
    }
}

/// Tabs interaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TabsOutcome<Id> {
    /// No effect.
    Ignored,
    /// Chrome / hover / scroll.
    Changed,
    /// Roving focus moved (may equal selection in automatic mode).
    FocusChanged {
        /// Focused tab.
        id: Option<Id>,
    },
    /// Selected (activated) panel changed — host shows that panel.
    SelectionChanged {
        /// Selected tab.
        id: Id,
    },
    /// Close requested for tab (host removes panel state).
    CloseRequested {
        /// Tab to close.
        id: Id,
    },
    /// Overflow menu should open (host paints menu of overflow ids).
    OverflowOpened {
        /// Ids not shown inline.
        overflow_ids: Vec<Id>,
    },
    /// Overflow closed.
    OverflowClosed,
    /// Host should reorder tabs (list order is host-owned).
    ReorderRequested {
        /// From index in current projection.
        from: usize,
        /// To index.
        to: usize,
    },
    /// Presentation class changed.
    PresentationChanged {
        /// Presentation.
        presentation: TabsPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`Tabs`].
///
/// **Panel state is not stored** — host maps `selected` id → panel widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsState<Id> {
    /// Selected (active panel) id.
    pub selected: Option<Id>,
    /// Hovered id (pointer).
    pub hovered: Option<Id>,
    /// Whether the strip owns keyboard focus.
    pub focused: bool,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
    /// Close glyph hits.
    pub close_regions: Vec<(Id, Rect)>,
    /// Overflow trigger rect.
    pub overflow_trigger: Option<Rect>,
    /// Ids currently in overflow (host menu).
    pub overflow_ids: Vec<Id>,
    /// Roving focus among tabs.
    collection: CollectionState<Id>,
    activation: TabsActivation,
    orientation: TabsOrientation,
    presentation: TabsPresentation,
    /// First visible index when scrolling.
    scroll_offset: usize,
    overflow_open: bool,
    enabled: bool,
    root: Rect,
    /// The tab the strip is moving away from, and when the move started.
    ///
    /// An active fill that snaps reads as a jump between two unrelated
    /// strips; carrying the previous tab lets the fill blend (plans/014).
    previous: Option<Id>,
    changed_at_ms: u64,
}

impl<Id> Default for TabsState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> TabsState<Id> {
    /// Empty tabs state (horizontal roving by default).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            hovered: None,
            focused: false,
            regions: Vec::new(),
            close_regions: Vec::new(),
            overflow_trigger: None,
            overflow_ids: Vec::new(),
            collection: CollectionState::new()
                .wrap(true)
                .orientation(crate::interaction::RovingOrientation::Horizontal),
            activation: TabsActivation::Automatic,
            orientation: TabsOrientation::Horizontal,
            presentation: TabsPresentation::Expanded,
            scroll_offset: 0,
            overflow_open: false,
            enabled: true,
            root: Rect::default(),
            previous: None,
            changed_at_ms: 0,
        }
    }

    /// Activation mode.
    #[must_use]
    pub const fn with_activation(mut self, activation: TabsActivation) -> Self {
        self.activation = activation;
        self
    }

    /// Orientation.
    #[must_use]
    pub fn with_orientation(mut self, orientation: TabsOrientation) -> Self
    where
        Id: Clone,
    {
        self.orientation = orientation;
        self.collection = match orientation {
            TabsOrientation::Horizontal => self
                .collection
                .orientation(crate::interaction::RovingOrientation::Horizontal),
            TabsOrientation::Vertical => self
                .collection
                .orientation(crate::interaction::RovingOrientation::Vertical),
        };
        self
    }

    /// Initial selection.
    #[must_use]
    pub fn with_selected(mut self, id: Id) -> Self
    where
        Id: Clone + PartialEq,
    {
        self.selected = Some(id.clone());
        self.collection.set_active(Some(id));
        self
    }

    /// Selected id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Focused tab id (roving).
    #[must_use]
    pub fn focused_tab(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Activation.
    #[must_use]
    pub const fn activation(&self) -> TabsActivation {
        self.activation
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(&self) -> TabsOrientation {
        self.orientation
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> TabsPresentation {
        self.presentation
    }

    /// Overflow open.
    #[must_use]
    pub const fn is_overflow_open(&self) -> bool {
        self.overflow_open
    }

    /// Focus strip.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Set selected panel id (host or internal).
    pub fn set_selected(&mut self, id: Option<Id>)
    where
        Id: Clone + PartialEq,
    {
        if self.selected != id {
            self.previous = self.selected.clone();
        }
        self.selected = id.clone();
        if let Some(id) = id {
            self.collection.set_active(Some(id));
        }
    }

    /// Records when the active tab changed, in runner milliseconds.
    ///
    /// Hosts that animate call this from their tick; hosts that do not leave
    /// it alone and the fill snaps, which is the settled frame.
    pub const fn mark_changed_at(&mut self, elapsed_ms: u64) {
        self.changed_at_ms = elapsed_ms;
    }

    /// How far the active-fill blend has run at `elapsed_ms` (`1.0` settled).
    #[must_use]
    pub fn blend_fraction(&self, elapsed_ms: u64, duration_ms: u64) -> f32 {
        if self.previous.is_none() || duration_ms == 0 {
            return 1.0;
        }
        let since = elapsed_ms.saturating_sub(self.changed_at_ms);
        if since >= duration_ms {
            return 1.0;
        }
        since as f32 / duration_ms as f32
    }

    /// Presentation for bounds.
    #[must_use]
    pub fn presentation_for_bounds(bounds: Rect, orientation: TabsOrientation) -> TabsPresentation {
        match orientation {
            TabsOrientation::Vertical => {
                if bounds.height < 4 {
                    TabsPresentation::Select
                } else {
                    TabsPresentation::Expanded
                }
            }
            TabsOrientation::Horizontal => {
                if bounds.width < TABS_SELECT_MAX_WIDTH {
                    TabsPresentation::Select
                } else if bounds.width < TABS_OVERFLOW_MAX_WIDTH {
                    TabsPresentation::Overflow
                } else {
                    TabsPresentation::Expanded
                }
            }
        }
    }

    fn items_from_tabs(tabs: &[Tab<'_, Id>]) -> Vec<CollectionItem<Id>>
    where
        Id: Clone,
    {
        tabs.iter()
            .map(|t| CollectionItem::new(t.id.clone(), t.label.to_owned()).enabled(t.enabled))
            .collect()
    }

    fn activate(&mut self, id: Id, tabs: &[Tab<'_, Id>]) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !tabs.iter().any(|tab| tab.id == id && tab.enabled) {
            return TabsOutcome::Ignored;
        }
        if self.selected.as_ref() == Some(&id) {
            return TabsOutcome::Changed;
        }
        self.previous = self.selected.clone();
        self.selected = Some(id.clone());
        self.collection.set_active(Some(id.clone()));
        TabsOutcome::SelectionChanged { id }
    }

    /// Key adapter — pass current tab projection.
    pub fn handle_key(&mut self, key: KeyEvent, tabs: &[Tab<'_, Id>]) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return TabsOutcome::Ignored;
        }
        if !self.focused {
            return TabsOutcome::Ignored;
        }

        let items = Self::items_from_tabs(tabs);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Close: Ctrl+W / Delete on focused closable
        if (ctrl && matches!(key.code, KeyCode::Char('w') | KeyCode::Char('W')))
            || matches!(key.code, KeyCode::Delete)
        {
            if let Some(id) = self.collection.active().cloned() {
                if tabs.iter().any(|t| t.id == id && t.closable && t.enabled) {
                    return TabsOutcome::CloseRequested { id };
                }
            }
        }

        // Overflow toggle
        if matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O'))
            && matches!(
                self.presentation,
                TabsPresentation::Overflow | TabsPresentation::Select
            )
        {
            if self.overflow_open {
                self.overflow_open = false;
                return TabsOutcome::OverflowClosed;
            }
            self.overflow_open = true;
            return TabsOutcome::OverflowOpened {
                overflow_ids: self.overflow_ids.clone(),
            };
        }

        // Esc closes overflow
        if key.code == KeyCode::Esc && self.overflow_open {
            self.overflow_open = false;
            return TabsOutcome::OverflowClosed;
        }

        // Reorder hooks: Ctrl+Left/Right or Alt+arrows
        if (ctrl || alt)
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            )
        {
            if let Some(from) = self.collection.active_index(&items) {
                let to = match key.code {
                    KeyCode::Left | KeyCode::Up => from.saturating_sub(1),
                    KeyCode::Right | KeyCode::Down => (from + 1).min(items.len().saturating_sub(1)),
                    _ => from,
                };
                if to != from {
                    return TabsOutcome::ReorderRequested { from, to };
                }
            }
            return TabsOutcome::Ignored;
        }

        // Enter activates in manual mode (and always if focus differs)
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            if let Some(id) = self.collection.active().cloned() {
                return self.activate(id, tabs);
            }
            return TabsOutcome::Ignored;
        }

        // Home/End
        if key.code == KeyCode::Home {
            let out = self.collection.move_first(&items);
            return self.after_focus_move(out, tabs);
        }
        if key.code == KeyCode::End {
            let out = self.collection.move_last(&items);
            return self.after_focus_move(out, tabs);
        }

        match self.collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                self.ensure_focus_visible(tabs);
                match self.activation {
                    TabsActivation::Automatic => {
                        if let Some(id) = to.clone() {
                            let _ = self.activate(id, tabs);
                        }
                        TabsOutcome::FocusChanged { id: to }
                    }
                    TabsActivation::Manual => TabsOutcome::FocusChanged { id: to },
                }
            }
            CollectionOutcome::Scrolled => {
                self.ensure_focus_visible(tabs);
                TabsOutcome::Changed
            }
            CollectionOutcome::Ignored => TabsOutcome::Ignored,
        }
    }

    fn after_focus_move(
        &mut self,
        out: CollectionOutcome<Id>,
        tabs: &[Tab<'_, Id>],
    ) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        match out {
            CollectionOutcome::ActiveChanged { to, .. } => match self.activation {
                TabsActivation::Automatic => {
                    if let Some(id) = to.clone() {
                        let _ = self.activate(id, tabs);
                    }
                    TabsOutcome::FocusChanged { id: to }
                }
                TabsActivation::Manual => TabsOutcome::FocusChanged { id: to },
            },
            CollectionOutcome::Scrolled => TabsOutcome::Changed,
            CollectionOutcome::Ignored => TabsOutcome::Ignored,
        }
    }

    fn ensure_focus_visible(&mut self, tabs: &[Tab<'_, Id>])
    where
        Id: PartialEq,
    {
        let Some(active) = self.collection.active() else {
            return;
        };
        let Some(idx) = tabs.iter().position(|t| &t.id == active) else {
            return;
        };
        // keep scroll window covering focus (simple: offset = idx if ahead)
        if idx < self.scroll_offset {
            self.scroll_offset = idx;
        }
    }

    /// Intent path.
    pub fn handle_intent(&mut self, intent: UiIntent, tabs: &[Tab<'_, Id>]) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled || !self.focused {
            return TabsOutcome::Ignored;
        }
        let items = Self::items_from_tabs(tabs);
        match intent {
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(id) = self.collection.active().cloned() {
                    return self.activate(id, tabs);
                }
                TabsOutcome::Ignored
            }
            UiIntent::Close => {
                if self.overflow_open {
                    self.overflow_open = false;
                    return TabsOutcome::OverflowClosed;
                }
                if let Some(id) = self.collection.active().cloned() {
                    if tabs.iter().any(|t| t.id == id && t.closable) {
                        return TabsOutcome::CloseRequested { id };
                    }
                }
                TabsOutcome::Ignored
            }
            other => {
                let out = self.collection.handle_intent(other, &items);
                self.after_focus_move(out, tabs)
            }
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, tabs: &[Tab<'_, Id>]) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled {
            return TabsOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left) => {
                for r in &self.regions {
                    if r.area.contains(event.position) {
                        self.hovered = Some(r.id.clone());
                        return TabsOutcome::Changed;
                    }
                }
                if self.hovered.take().is_some() {
                    return TabsOutcome::Changed;
                }
                TabsOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.focused = true;
                // close glyphs first
                for (id, rect) in &self.close_regions {
                    if rect.contains(event.position) {
                        return TabsOutcome::CloseRequested { id: id.clone() };
                    }
                }
                if let Some(tr) = self.overflow_trigger {
                    if tr.contains(event.position) {
                        self.overflow_open = !self.overflow_open;
                        return if self.overflow_open {
                            TabsOutcome::OverflowOpened {
                                overflow_ids: self.overflow_ids.clone(),
                            }
                        } else {
                            TabsOutcome::OverflowClosed
                        };
                    }
                }
                for r in &self.regions {
                    if r.area.contains(event.position) {
                        let id = r.id.clone();
                        self.collection.set_active(Some(id.clone()));
                        return self.activate(id, tabs);
                    }
                }
                TabsOutcome::Ignored
            }
            _ => TabsOutcome::Ignored,
        }
    }

    /// Select from overflow menu (host).
    pub fn select_overflow_id(&mut self, id: Id) -> TabsOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.overflow_ids.contains(&id) {
            return TabsOutcome::Ignored;
        }
        self.overflow_open = false;
        self.previous = self.selected.clone();
        self.selected = Some(id.clone());
        self.collection.set_active(Some(id.clone()));
        TabsOutcome::SelectionChanged { id }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// How a tab strip marks the active tab.
///
/// The rule row is the canonical default; alternate cues remain available for
/// shells whose content geometry calls for another treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TabsActiveCue {
    /// A bold label with no rule of its own, for hosts that draw the rule.
    AccentPill,
    /// The active tab sits on the body's own ground, joined to the pane below.
    Connected,
    /// A leading marker plus weight — the colourless-safe fallback.
    Marker,
    /// A semantic rule under the active tab (default).
    ///
    /// The rule uses the accent role while the selected tab owns focus and
    /// the ordinary border role while the strip is unfocused.
    #[default]
    Rule,
}

impl TabsActiveCue {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AccentPill => "accent-pill",
            Self::Connected => "connected",
            Self::Marker => "marker",
            Self::Rule => "rule",
        }
    }
}

/// Keyboard- and pointer-navigable tab strip.
#[derive(Debug, Clone, Copy)]
pub struct Tabs<'a, Id> {
    tabs: &'a [Tab<'a, Id>],
    gap: u16,
    system: &'a DesignSystem,
    show_close: bool,
    active_cue: TabsActiveCue,
}

impl<'a, Id> Tabs<'a, Id> {
    /// Chooses how the active tab is marked (default [`TabsActiveCue::Rule`]).
    #[must_use]
    pub const fn active_cue(mut self, cue: TabsActiveCue) -> Self {
        self.active_cue = cue;
        self
    }

    /// Creates a tab strip over borrowed tabs.
    #[must_use]
    pub const fn new(tabs: &'a [Tab<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            tabs,
            active_cue: TabsActiveCue::Rule,
            gap: TAB_GAP,
            system,
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            show_close: true,
        }
    }

    /// Spacing between adjacent horizontal items.
    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// ASCII status / close marks.
    #[must_use]
    /// Paint close affordances for closable tabs.
    pub const fn show_close(mut self, on: bool) -> Self {
        self.show_close = on;
        self
    }

    /// Preferred paint entry.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TabsState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.regions.clear();
        state.close_regions.clear();
        state.overflow_trigger = None;
        state.overflow_ids.clear();
        state.root = area;
        if area.is_empty() || self.tabs.is_empty() {
            return;
        }

        let items = TabsState::<Id>::items_from_tabs(self.tabs);
        let _ = state.collection.reconcile(&items);
        if state.selected.is_none() {
            if let Some(t) = self.tabs.iter().find(|t| t.enabled) {
                state.selected = Some(t.id.clone());
            }
        }
        if state.collection.active().is_none() {
            if let Some(id) = state.selected.clone() {
                state.collection.set_active(Some(id));
            }
        }

        // Measure total width for expand vs scroll
        let total_w = self.measure_total_width(state);
        let presentation = match state.orientation {
            TabsOrientation::Vertical => {
                if self.tabs.len() <= 1 || area.height as usize >= self.tabs.len() {
                    TabsPresentation::Expanded
                } else if area.height < 4 {
                    TabsPresentation::Select
                } else {
                    TabsPresentation::Overflow
                }
            }
            TabsOrientation::Horizontal => {
                // Single tab always expanded (theme/hit tests use narrow width).
                if self.tabs.len() <= 1 || total_w <= area.width {
                    TabsPresentation::Expanded
                } else if area.width < TABS_SELECT_MAX_WIDTH {
                    TabsPresentation::Select
                } else if area.width < TABS_OVERFLOW_MAX_WIDTH {
                    TabsPresentation::Overflow
                } else {
                    TabsPresentation::Scrolling
                }
            }
        };
        if presentation != state.presentation {
            state.presentation = presentation;
        }

        match state.orientation {
            TabsOrientation::Vertical => self.paint_vertical(area, buffer, state),
            TabsOrientation::Horizontal => match state.presentation {
                TabsPresentation::Select => self.paint_select(area, buffer, state),
                TabsPresentation::Overflow => self.paint_overflow(area, buffer, state),
                TabsPresentation::Scrolling => self.paint_scrolling(area, buffer, state),
                TabsPresentation::Expanded => {
                    self.paint_expanded(area, buffer, state, 0, self.tabs.len())
                }
            },
        }
    }

    fn measure_total_width(&self, state: &TabsState<Id>) -> u16 {
        let show_status = crate::layout::tabs_show_status_glyphs(state.root.width.max(80));
        let mut w = 0u16;
        for (i, tab) in self.tabs.iter().enumerate() {
            w = w.saturating_add(self.tab_width(tab, show_status));
            if i + 1 < self.tabs.len() {
                w = w.saturating_add(self.gap);
            }
        }
        w
    }

    fn tab_width(&self, tab: &Tab<'a, Id>, show_status: bool) -> u16 {
        let mut cols = UnicodeWidthStr::width(tab.label).saturating_add(2); // padding
        if show_status {
            if tab.glyph.is_some() {
                cols = cols.saturating_add(2);
            } else if tab.status.mark(false).is_some() {
                cols = cols.saturating_add(2);
            }
        }
        if let Some(b) = tab.badge {
            cols = cols
                .saturating_add(UnicodeWidthStr::width(b))
                .saturating_add(1);
        }
        if self.show_close && tab.closable {
            cols = cols.saturating_add(2);
        }
        u16::try_from(cols).unwrap_or(u16::MAX)
    }

    fn paint_select(&self, area: Rect, buffer: &mut Buffer, state: &mut TabsState<Id>)
    where
        Id: Clone + PartialEq,
    {
        let label = state
            .selected
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.label)
            .unwrap_or("Tabs");
        let mark = { "▾" };
        let text = format!(" {label} {mark} ");
        // The collapsed trigger is a control, not a selection slab: focus
        // reads through weight and the active tone, never a reversal.
        let style = self
            .system
            .style(if state.focused {
                Role::TabActive
            } else {
                Role::TabInactive
            })
            .add_modifier(if state.focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&text, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        state.overflow_trigger = Some(area);
        state.overflow_ids = self.tabs.iter().map(|t| t.id.clone()).collect();
        if let Some(id) = state.selected.clone() {
            state.regions.push(HitRegion {
                id,
                area: Rect::new(area.x, area.y, area.width, area.height.min(1)),
            });
        }
    }

    fn paint_overflow(&self, area: Rect, buffer: &mut Buffer, state: &mut TabsState<Id>)
    where
        Id: Clone + PartialEq,
    {
        let show_status = crate::layout::tabs_show_status_glyphs(area.width);
        let ellipsis_w = 3u16;
        let budget = area.width.saturating_sub(ellipsis_w).saturating_sub(1);
        let mut x = area.x;
        let mut used = 0u16;
        let mut last_fit = 0usize;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width(tab, show_status);
            let need = if i == 0 {
                w
            } else {
                w.saturating_add(self.gap)
            };
            if used.saturating_add(need) > budget && i > 0 {
                break;
            }
            if i > 0 {
                x = x.saturating_add(self.gap);
                used = used.saturating_add(self.gap);
            }
            let rect = self.paint_one_tab(tab, x, area, buffer, state, show_status);
            x = x.saturating_add(rect.width);
            used = used.saturating_add(rect.width);
            last_fit = i + 1;
        }
        for tab in self.tabs.iter().skip(last_fit) {
            state.overflow_ids.push(tab.id.clone());
        }
        if last_fit < self.tabs.len() {
            let tr = Rect::new(
                area.right().saturating_sub(ellipsis_w),
                area.y,
                ellipsis_w.min(area.width),
                1,
            );
            buffer.set_stringn(
                tr.x,
                tr.y,
                // Catalog-resolved and padded to the same budget in both
                // profiles, so the overflow trigger does not shift by a cell
                // between Unicode and ASCII.
                &Glyph::Ellipsis.resolve().aligned(ellipsis_w),
                usize::from(tr.width),
                self.system.style(Role::TabInactive),
            );
            state.overflow_trigger = Some(tr);
        }
    }

    fn paint_scrolling(&self, area: Rect, buffer: &mut Buffer, state: &mut TabsState<Id>)
    where
        Id: Clone + PartialEq,
    {
        let show_status = crate::layout::tabs_show_status_glyphs(area.width);
        // ensure focus in window
        if let Some(active) = state.collection.active() {
            if let Some(idx) = self.tabs.iter().position(|t| &t.id == active) {
                if idx < state.scroll_offset {
                    state.scroll_offset = idx;
                }
            }
        }
        let mut x = area.x;
        let mut i = state.scroll_offset;
        while i < self.tabs.len() {
            let tab = &self.tabs[i];
            let w = self.tab_width(tab, show_status);
            if x.saturating_add(w) > area.right() {
                // remaining to overflow for discoverability
                for t in self.tabs.iter().skip(i) {
                    state.overflow_ids.push(t.id.clone());
                }
                break;
            }
            let rect = self.paint_one_tab(tab, x, area, buffer, state, show_status);
            x = x.saturating_add(rect.width).saturating_add(self.gap);
            i += 1;
        }
        if state.scroll_offset > 0 || i < self.tabs.len() {
            // scroll cues as overflow trigger for remaining
            if i < self.tabs.len() {
                let tr = Rect::new(area.right().saturating_sub(2), area.y, 2.min(area.width), 1);
                buffer.set_stringn(tr.x, tr.y, "›", 1, self.system.style(Role::TextMuted));
                state.overflow_trigger = Some(tr);
            }
        }
    }

    fn paint_expanded(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TabsState<Id>,
        start: usize,
        end: usize,
    ) where
        Id: Clone + PartialEq,
    {
        let show_status = crate::layout::tabs_show_status_glyphs(area.width);
        let mut x = area.x;
        for (i, tab) in self
            .tabs
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            if x >= area.right() {
                for t in self.tabs.iter().skip(i) {
                    state.overflow_ids.push(t.id.clone());
                }
                break;
            }
            let rect = self.paint_one_tab(tab, x, area, buffer, state, show_status);
            x = x.saturating_add(rect.width).saturating_add(self.gap);
        }
    }

    fn paint_vertical(&self, area: Rect, buffer: &mut Buffer, state: &mut TabsState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if matches!(state.presentation, TabsPresentation::Select) {
            self.paint_select(area, buffer, state);
            return;
        }
        let show_status = true;
        let mut y = area.y;
        for tab in self.tabs {
            if y >= area.bottom() {
                state.overflow_ids.push(tab.id.clone());
                continue;
            }
            let row = Rect::new(area.x, y, area.width, 1);
            let _ = self.paint_one_tab_in_rect(tab, row, buffer, state, show_status);
            y = y.saturating_add(1);
        }
    }

    fn paint_one_tab(
        &self,
        tab: &Tab<'a, Id>,
        x: u16,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TabsState<Id>,
        show_status: bool,
    ) -> Rect
    where
        Id: Clone + PartialEq,
    {
        let w = self
            .tab_width(tab, show_status)
            .min(area.right().saturating_sub(x));
        let rect = Rect::new(x, area.y, w, area.height.min(2).max(1));
        self.paint_one_tab_in_rect(tab, rect, buffer, state, show_status)
    }

    fn paint_one_tab_in_rect(
        &self,
        tab: &Tab<'a, Id>,
        rect: Rect,
        buffer: &mut Buffer,
        state: &mut TabsState<Id>,
        show_status: bool,
    ) -> Rect
    where
        Id: Clone + PartialEq,
    {
        if rect.is_empty() {
            return rect;
        }
        let selected = state.selected.as_ref() == Some(&tab.id);
        let focused_tab = state.collection.active() == Some(&tab.id) && state.focused;
        let hovered = state.hovered.as_ref() == Some(&tab.id);
        let role = match (selected, hovered) {
            (true, true) => Role::TabActive,
            (true, false) => Role::TabActive,
            (false, true) => Role::TabInactive,
            (false, false) => Role::TabInactive,
        };
        let mut style = self.system.style(role);
        let mut marker = "";
        if selected {
            // The active cue is named, not improvised (plans/015 Step 3), and
            // it is the accent rule — never a wash: the selection tint is a
            // selection plane, and the active tab spends its green on the
            // `━` rule alone (D2 TabActive, D9 green budget).
            style = style.add_modifier(Modifier::BOLD);
            match self.active_cue {
                TabsActiveCue::AccentPill | TabsActiveCue::Rule => {}
                TabsActiveCue::Connected => {
                    if style.bg.is_none()
                        && let Some(bg) = self.system.style(Role::Surface).bg
                    {
                        style = style.bg(bg);
                    }
                }
                TabsActiveCue::Marker => {
                    marker = {
                        self.system
                            .glyphs
                            .resolve(crate::style::Glyph::ChevronRight)
                            .text
                    };
                }
            }
        }
        if focused_tab && !selected {
            // Roving focus that has not committed yet: brighten the tab's
            // leading edge rather than reversing the whole label, so the cue
            // does not depend on the strip having a second row (plans/021
            // Step 4, design law §5.2).
            style = style
                .patch(self.system.style(Role::BorderFocused))
                .add_modifier(Modifier::BOLD);
        } else if focused_tab && selected {
            style = style.add_modifier(Modifier::BOLD);
        } else if hovered {
            // junie: hover is one surface plane up, never a colour.
            if let Some(bg) = self.system.style(Role::Surface).bg {
                style = style.bg(self.system.lift(bg));
            }
        }
        if !tab.enabled {
            // Disabled is a different fact from "inactive": muting both meant a
            // tab you cannot click looked exactly like one you can.
            style = self.system.style(Role::TextDisabled);
        }

        let mut parts = String::from(" ");
        if !marker.is_empty() {
            parts.push_str(marker);
            parts.push(' ');
        }
        if show_status {
            if let Some(g) = &tab.glyph {
                parts.push_str(g.content.as_ref());
                parts.push(' ');
            } else if let Some(m) = tab.status.mark(false) {
                parts.push_str(m);
                parts.push(' ');
            }
        }
        parts.push_str(tab.label);
        if let Some(b) = tab.badge {
            parts.push(' ');
            parts.push_str(b);
        }
        if self.show_close && tab.closable {
            parts.push_str(" ×");
        }
        parts.push(' ');

        let label_h = 1u16;
        let label_rect = Rect::new(rect.x, rect.y, rect.width, label_h);
        buffer.set_stringn(
            label_rect.x,
            label_rect.y,
            take_display_cols(&parts, usize::from(label_rect.width)),
            usize::from(label_rect.width),
            style,
        );

        if selected && matches!(self.active_cue, TabsActiveCue::Rule) && rect.height > 1 {
            // N3: an active tab states itself with the strong rule in the
            // accent, whenever it is active; an inactive tab draws no line at
            // all — a faint rule under every tab is noise, not a cue.
            let rule = self.system.glyphs.rule_strong();
            let line: String = std::iter::repeat_n(rule, usize::from(rect.width)).collect();
            buffer.set_stringn(
                rect.x,
                rect.y.saturating_add(1),
                &line,
                usize::from(rect.width),
                self.system.style(Role::Accent),
            );
        }

        // Restyle glyph span color when present
        if show_status
            && label_rect.width > 1
            && let Some(glyph) = &tab.glyph
        {
            buffer.set_span(
                label_rect.x.saturating_add(1),
                label_rect.y,
                glyph,
                label_rect.width.saturating_sub(1),
            );
        }

        if tab.enabled {
            state.regions.push(HitRegion {
                id: tab.id.clone(),
                area: Rect::new(rect.x, rect.y, rect.width, rect.height.min(2)),
            });
        }
        if self.show_close && tab.closable && tab.enabled {
            let close_x = rect.right().saturating_sub(2);
            state.close_regions.push((
                tab.id.clone(),
                Rect::new(close_x, rect.y, 2.min(rect.width), 1),
            ));
        }
        rect
    }

    /// Semantic registration for the strip.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &TabsState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: std::fmt::Debug,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "tabs {} {} n={}",
            state.orientation.id(),
            state.presentation.id(),
            self.tabs.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Tab)
                .label("tabs")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: false,
                    invalid: false,
                    expanded: state.overflow_open,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tabs<'_, Id> {
    type State = TabsState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Tabs<'_, Id> {
    type State = TabsState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;
    use ratatui_core::style::{Color, Style};

    fn sample_tabs() -> [Tab<'static, &'static str>; 4] {
        [
            Tab::new("overview", "Overview").status(TabStatus::Success),
            Tab::new("details", "Details"),
            Tab::new("logs", "Logs")
                .closable(true)
                .status(TabStatus::Running),
            Tab::new("disabled", "Disabled").enabled(false),
        ]
    }

    #[test]
    fn default_rule_and_hit_regions_share_two_row_geometry() {
        let tabs = [
            Tab {
                id: "overview",
                label: "Overview",
                glyph: None,
                badge: None,
                status: TabStatus::None,
                enabled: true,
                closable: false,
            },
            Tab {
                id: "disabled",
                label: "Disabled",
                glyph: None,
                badge: None,
                status: TabStatus::None,
                enabled: false,
                closable: false,
            },
        ];
        let area = Rect::new(3, 4, 30, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState {
            selected: Some("overview"),
            hovered: None,
            focused: true,
            ..TabsState::default()
        };
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());
        (&Tabs::new(&tabs, &system).gap(1)).render(area, &mut buffer, &mut state);

        assert!(buffer[(3, 4)].modifier.contains(Modifier::BOLD));
        // No wash behind the active tab: the cue is the accent rule (D2/D9).
        assert_eq!(buffer[(3, 4)].bg, Color::Reset);
        assert_eq!(buffer[(3, 5)].symbol(), system.glyphs.rule_strong());
        assert_eq!(buffer[(3, 5)].fg, theme.style(Role::Accent).fg.unwrap());
        assert_eq!(state.regions.len(), 1);
        assert!(state.regions[0].area.contains(Position::new(3, 5)));
    }

    #[test]
    fn active_rule_is_always_the_accent() {
        // N3: an active tab states itself with the strong rule in the accent
        // whenever it is active — focus never gates the cue, and an inactive
        // tab draws no rule at all.
        let tabs = [Tab::new("overview", "Overview")];
        let area = Rect::new(0, 0, 16, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState::new().with_selected("overview");
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());

        Tabs::new(&tabs, &system).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(0, 1)].symbol(), system.glyphs.rule_strong());
        assert_eq!(buffer[(0, 1)].fg, theme.style(Role::Accent).fg.unwrap());
        // The active label sits on the strip's own ground, not on the tint.
        assert_eq!(buffer[(0, 0)].bg, Color::Reset);
    }

    #[test]
    fn glyph_span_style_overrides_the_tab_foreground_without_a_wash() {
        let tabs = [Tab {
            id: "running",
            label: "Build",
            glyph: Some(Span::styled("●", Style::new().fg(Color::Yellow))),
            badge: None,
            status: TabStatus::None,
            enabled: true,
            closable: false,
        }];
        let area = Rect::new(0, 0, 20, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState::default();
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());

        (&Tabs::new(&tabs, &system)
            .active_cue(TabsActiveCue::AccentPill)
            .gap(1))
            .render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(1, 0)].symbol(), "●");
        assert_eq!(buffer[(1, 0)].fg, Color::Yellow);
        // The glyph rides the label ground; the active tab carries no wash.
        assert_eq!(buffer[(1, 0)].bg, Color::Reset);
    }

    #[test]
    fn tab_geometry_uses_display_columns_and_excludes_gaps() {
        let cells = lay_out_tabs(&[("界", true), ("b", false)], 5);
        assert_eq!(cells[0].cell_cols, 4);
        assert_eq!(cells[1].start_col, 10);
        assert_eq!(tab_at_column(&cells, 5), Some(0));
        assert_eq!(tab_at_column(&cells, 8), Some(0));
        assert_eq!(tab_at_column(&cells, 9), None);
        assert_eq!(tab_at_column(&cells, 10), Some(1));
        assert_eq!(tab_at_column(&cells, 13), None);
    }

    #[test]
    fn automatic_activation_on_arrow() {
        let tabs = sample_tabs();
        let mut state = TabsState::new()
            .with_selected("overview")
            .with_activation(TabsActivation::Automatic);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &tabs),
            TabsOutcome::FocusChanged { .. } | TabsOutcome::SelectionChanged { .. }
        ));
        assert_eq!(state.selected(), Some(&"details"));
    }

    #[test]
    fn manual_activation_requires_enter() {
        let tabs = sample_tabs();
        let mut state = TabsState::new()
            .with_selected("overview")
            .with_activation(TabsActivation::Manual);
        state.set_focused(true);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &tabs);
        assert_eq!(state.selected(), Some(&"overview"));
        assert_eq!(state.focused_tab(), Some(&"details"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &tabs),
            TabsOutcome::SelectionChanged { id: "details" }
        ));
    }

    #[test]
    fn close_requested() {
        let tabs = sample_tabs();
        let mut state = TabsState::new().with_selected("logs");
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
                &tabs
            ),
            TabsOutcome::CloseRequested { id: "logs" }
        ));
    }

    #[test]
    fn reorder_hook() {
        let tabs = sample_tabs();
        let mut state = TabsState::new().with_selected("overview");
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL), &tabs),
            TabsOutcome::ReorderRequested { from: 0, to: 1 }
        ));
    }

    #[test]
    fn narrow_select_presentation() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let mut state = TabsState::new().with_selected("overview");
        state.set_focused(true);
        let area = Rect::new(0, 0, 14, 2);
        let mut buf = Buffer::empty(area);
        Tabs::new(&tabs, &system).paint(area, &mut buf, &mut state);
        assert_eq!(state.presentation(), TabsPresentation::Select);
        assert!(state.overflow_trigger.is_some());
    }

    #[test]
    fn overflow_presentation() {
        let many: Vec<Tab<'_, &str>> = (0..8)
            .map(|i| {
                // leak labels for 'static-ish test — use static array instead
                Tab::new(
                    ["a", "b", "c", "d", "e", "f", "g", "h"][i],
                    [
                        "AlphaTab",
                        "BetaTabX",
                        "GammaTabs",
                        "DeltaTab",
                        "EpsilTab",
                        "ZetaTabs",
                        "EtaTabsX",
                        "ThetaTab",
                    ][i],
                )
            })
            .collect();
        let system = DesignSystem::default();
        let mut state = TabsState::new().with_selected("a");
        let area = Rect::new(0, 0, 30, 2);
        let mut buf = Buffer::empty(area);
        Tabs::new(&many, &system).paint(area, &mut buf, &mut state);
        assert!(matches!(
            state.presentation(),
            TabsPresentation::Overflow | TabsPresentation::Scrolling | TabsPresentation::Select
        ));
    }

    #[test]
    fn escape_closes_only_the_overflow_layer() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let mut state = TabsState::new().with_selected("overview");
        state.set_focused(true);
        let area = Rect::new(0, 0, 14, 2);
        let mut buffer = Buffer::empty(area);
        Tabs::new(&tabs, &system).paint(area, &mut buffer, &mut state);

        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), &tabs,),
            TabsOutcome::OverflowOpened { .. }
        ));
        assert!(state.is_overflow_open());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &tabs),
            TabsOutcome::OverflowClosed
        );
        assert!(!state.is_overflow_open());
        assert_eq!(state.selected(), Some(&"overview"));
    }

    #[test]
    fn vertical_orientation() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let mut state = TabsState::new()
            .with_selected("overview")
            .with_orientation(TabsOrientation::Vertical);
        state.set_focused(true);
        let area = Rect::new(0, 0, 16, 6);
        let mut buf = Buffer::empty(area);
        Tabs::new(&tabs, &system).paint(area, &mut buf, &mut state);
        assert!(state.regions.len() >= 2);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &tabs),
            TabsOutcome::FocusChanged { .. } | TabsOutcome::SelectionChanged { .. }
        ));
    }

    #[test]
    fn mouse_select() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let mut state = TabsState::new().with_selected("overview");
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        Tabs::new(&tabs, &system).paint(area, &mut buf, &mut state);
        let details = state
            .regions
            .iter()
            .find(|r| r.id == "details")
            .expect("details hit");
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(details.area.x, details.area.y),
                    modifiers: KeyModifiers::NONE,
                },
                &tabs
            ),
            TabsOutcome::SelectionChanged { id: "details" }
        ));
    }

    #[test]
    fn disabled_tabs_are_not_focusable_or_activatable() {
        let tabs = sample_tabs();
        let mut state = TabsState::new().with_selected("disabled");
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &tabs),
            TabsOutcome::Ignored
        );

        state.set_enabled(false);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &tabs),
            TabsOutcome::Ignored
        );
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(0, 0),
                    modifiers: KeyModifiers::NONE,
                },
                &tabs,
            ),
            TabsOutcome::Ignored
        );
    }

    #[test]
    fn fuzz_keys() {
        let tabs = sample_tabs();
        let mut state = TabsState::new().with_selected("overview");
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &tabs);
        }
    }

    #[test]
    fn paint_hot_path() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let mut state = TabsState::new().with_selected("overview");
        let area = Rect::new(0, 0, 48, 2);
        let mut buf = Buffer::empty(area);
        let w = Tabs::new(&tabs, &system);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let tabs = sample_tabs();
        let system = DesignSystem::default();
        let state = TabsState::new().with_selected("overview");
        let mut scene = SemanticScene::<&str, ()>::default();
        Tabs::new(&tabs, &system).register_semantic(
            &mut scene,
            "tabs",
            Rect::new(0, 0, 40, 2),
            &state,
        );
        assert!(scene.get(&"tabs").is_some());
    }

    #[test]
    fn every_active_cue_marks_the_active_tab_differently() {
        let system = DesignSystem::default();
        let tabs = [Tab::new("a", "Files"), Tab::new("b", "Search")];
        let area = Rect::new(0, 0, 30, 2);
        let mut frames = Vec::new();
        for cue in [
            TabsActiveCue::AccentPill,
            TabsActiveCue::Connected,
            TabsActiveCue::Marker,
            TabsActiveCue::Rule,
        ] {
            let mut state = TabsState::new().with_selected("a");
            let mut buffer = Buffer::empty(area);
            Tabs::new(&tabs, &system)
                .active_cue(cue)
                .render(area, &mut buffer, &mut state);
            frames.push((cue, buffer));
        }
        for (i, (cue, frame)) in frames.iter().enumerate() {
            for (other_cue, other) in frames.iter().skip(i + 1) {
                assert_ne!(
                    frame.content(),
                    other.content(),
                    "{cue:?} and {other_cue:?} paint the same frame"
                );
            }
        }
        // The default is the focus-aware rule, not an implicit fill.
        let mut state = TabsState::new().with_selected("a");
        let mut buffer = Buffer::empty(area);
        Tabs::new(&tabs, &system).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 1)].symbol(), system.glyphs.rule_strong());
        assert_eq!(buffer[(0, 1)].fg, system.style(Role::Accent).fg.unwrap());
    }

    #[test]
    fn panel_state_not_in_tabs_state() {
        // compile-time doc: TabsState has no panel content fields
        let s = TabsState::<&str>::new();
        assert!(s.selected().is_none());
    }
}
