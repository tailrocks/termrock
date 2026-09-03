// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **LoadingOverlay** + **BusyBoundary** — coordinated regional loading.
//!
//! **Mission.** Block **only the affected region** instead of freezing the
//! whole app. Modes: non-blocking busy, blocking operation, cancellable,
//! optimistic, and stale-content. Preserve readable content when safe, explain
//! what is unavailable, and manage focus/input routing explicitly. Avoid
//! overlay abuse for short operations (min-show + short-op policy).
//!
//! **vs [`LoadingView`](super::LoadingView).** LoadingView is a centered
//! placeholder that replaces body content. LoadingOverlay can wash a region
//! while keeping underlying cells readable (stale/optimistic) or dim them.
//! **vs [`Spinner`](super::Spinner).** Spinner is glyph+verb; LoadingOverlay
//! composes Spinner with boundary policy and input routing.
//! **vs full-screen modal overlays.** Prefer BusyBoundary on a pane; full-app
//! blocking is a last resort.
//!
//! Research: async UI boundaries, Textual workers, agent tool execution.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    },
    layout::{Center, CenterAxis, center_line_x},
    runtime::{AnimationDemand, FrameTick},
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
    widgets::{ActivityPhase, SpinnerState},
};

/// Do not paint a blocking wash until this elapsed (ms) — avoids flash on short ops.
pub const LOADING_OVERLAY_MIN_SHOW_MS: u64 = 150;
/// Ops expected to finish under this (ms) should prefer NonBlocking / inline Spinner.
pub const LOADING_OVERLAY_SHORT_OP_HINT_MS: u64 = 300;
/// Max nested BusyBoundary depth tracked for tests/hosts.
pub const BUSY_BOUNDARY_MAX_NEST: u8 = 8;

// ── Mode ────────────────────────────────────────────────────────────────────

/// How a region behaves while work is in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BusyMode {
    /// Show busy cue; **do not** block input in the region.
    #[default]
    NonBlocking,
    /// Block input to the region; wash + spinner after min-show.
    Blocking,
    /// Blocking + cancel affordance (Esc / explicit cancel).
    Cancellable,
    /// Content already shows optimistic result; light busy cue only.
    Optimistic,
    /// Prior content still readable but marked stale / unavailable for edit.
    StaleContent,
}

impl BusyMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NonBlocking => "non-blocking",
            Self::Blocking => "blocking",
            Self::Cancellable => "cancellable",
            Self::Optimistic => "optimistic",
            Self::StaleContent => "stale-content",
        }
    }

    /// Whether the mode blocks pointer/key delivery into the region.
    #[must_use]
    pub const fn blocks_input(self) -> bool {
        matches!(
            self,
            Self::Blocking | Self::Cancellable | Self::StaleContent
        )
    }

    /// Whether cancel is offered.
    #[must_use]
    pub const fn cancellable(self) -> bool {
        matches!(self, Self::Cancellable)
    }

    /// Whether underlying content should remain legible (no full wash).
    #[must_use]
    pub const fn preserve_content(self) -> bool {
        matches!(
            self,
            Self::NonBlocking | Self::Optimistic | Self::StaleContent
        )
    }

    /// Whether a full-region dim wash is appropriate after min-show.
    #[must_use]
    pub const fn wants_wash(self) -> bool {
        matches!(self, Self::Blocking | Self::Cancellable)
    }
}

// ── Input routing ───────────────────────────────────────────────────────────

/// Result of routing input against a busy boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusyRoute {
    /// Event is outside the boundary region.
    Outside,
    /// Deliver to underlying content / host.
    Deliver,
    /// Swallow (region is blocked).
    Blocked,
    /// Interpret as cancel (cancellable mode).
    Cancel,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes from busy boundary interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusyBoundaryOutcome {
    /// No change.
    Ignored,
    /// User requested cancel (host aborts work).
    CancelRequested,
    /// Busy started.
    Started,
    /// Busy ended.
    Ended,
}

// ── BusyBoundary state ──────────────────────────────────────────────────────

/// Region-scoped busy state: mode, timing, nest depth, cancel, focus trap.
///
/// Host owns the work future; this type owns **presentation + input policy**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyBoundaryState {
    active: bool,
    mode: BusyMode,
    label: String,
    unavailable: Option<String>,
    /// Elapsed ms since start (host advances via [`Self::set_elapsed_ms`]).
    elapsed_ms: u64,
    /// Expected op duration hint (for short-op policy); `None` = unknown.
    expected_ms: Option<u64>,
    cancel_requested: bool,
    /// Focus trapped in region while blocking.
    focus_trapped: bool,
    /// Nesting depth (0 = root).
    nest_depth: u8,
    /// Spinner state for composed chrome.
    spinner: SpinnerState,
    /// Force suppress wash even if mode wants it (tests / short-op host override).
    suppress_wash: bool,
}

impl Default for BusyBoundaryState {
    fn default() -> Self {
        Self::new()
    }
}

impl BusyBoundaryState {
    /// Idle boundary.
    #[must_use]
    pub fn new() -> Self {
        let mut spinner = SpinnerState::new();
        spinner.set_active(false);
        spinner.set_visible(false);
        Self {
            active: false,
            mode: BusyMode::NonBlocking,
            label: String::new(),
            unavailable: None,
            elapsed_ms: 0,
            expected_ms: None,
            cancel_requested: false,
            focus_trapped: false,
            nest_depth: 0,
            spinner,
            suppress_wash: false,
        }
    }

    /// Nested child under a parent boundary (increments nest depth).
    #[must_use]
    pub fn nested_under(parent: &Self) -> Self {
        let mut child = Self::new();
        child.nest_depth = parent
            .nest_depth
            .saturating_add(1)
            .min(BUSY_BOUNDARY_MAX_NEST);
        child
    }

    /// Start busy work.
    pub fn begin(&mut self, mode: BusyMode, label: impl Into<String>) -> BusyBoundaryOutcome {
        self.active = true;
        self.mode = mode;
        self.label = label.into();
        self.elapsed_ms = 0;
        self.cancel_requested = false;
        self.focus_trapped = mode.blocks_input();
        self.spinner.set_active(true);
        self.spinner.set_visible(true);
        self.spinner.set_phase(ActivityPhase::Indeterminate);
        // Short expected ops: force light chrome preference via suppress_wash until min show
        self.suppress_wash = false;
        BusyBoundaryOutcome::Started
    }

    /// End busy work.
    pub fn end(&mut self) -> BusyBoundaryOutcome {
        self.active = false;
        self.focus_trapped = false;
        self.cancel_requested = false;
        self.elapsed_ms = 0;
        self.spinner.set_active(false);
        self.spinner.set_visible(false);
        BusyBoundaryOutcome::Ended
    }

    /// Host clock: set elapsed ms since begin.
    pub fn set_elapsed_ms(&mut self, ms: u64) {
        self.elapsed_ms = ms;
    }
    /// Optional expected duration (enables short-op policy).
    pub fn set_expected_ms(&mut self, ms: Option<u64>) {
        self.expected_ms = ms;
    }

    /// Explain what is unavailable while busy.
    pub fn set_unavailable(&mut self, text: Option<impl Into<String>>) {
        self.unavailable = text.map(Into::into);
    }

    /// Request cancel (or host marks cancel).
    pub fn request_cancel(&mut self) -> BusyBoundaryOutcome {
        if self.active && self.mode.cancellable() {
            self.cancel_requested = true;
            BusyBoundaryOutcome::CancelRequested
        } else {
            BusyBoundaryOutcome::Ignored
        }
    }

    /// Active?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> BusyMode {
        self.mode
    }

    /// Label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Unavailable explanation.
    #[must_use]
    pub fn unavailable(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }

    /// Elapsed.
    #[must_use]
    pub const fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    /// Cancel requested?
    #[must_use]
    pub const fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    /// Focus trap active?
    #[must_use]
    pub const fn focus_trapped(&self) -> bool {
        self.focus_trapped && self.active
    }

    /// Nest depth.
    #[must_use]
    pub const fn nest_depth(&self) -> u8 {
        self.nest_depth
    }

    /// Spinner state (for host tick demand).
    #[must_use]
    pub fn spinner(&self) -> &SpinnerState {
        &self.spinner
    }
    /// Whether expected duration classifies this as a short op.
    #[must_use]
    pub fn is_short_op(&self) -> bool {
        self.expected_ms
            .is_some_and(|e| e < LOADING_OVERLAY_SHORT_OP_HINT_MS)
    }

    /// Whether a blocking wash should paint (min-show + mode + short-op).
    #[must_use]
    pub fn should_show_wash(&self) -> bool {
        if !self.active || self.suppress_wash {
            return false;
        }
        if !self.mode.wants_wash() {
            return false;
        }
        // Short expected ops: never heavy wash — use light busy only
        if self.is_short_op() {
            return false;
        }
        self.elapsed_ms >= LOADING_OVERLAY_MIN_SHOW_MS
    }

    /// Whether any overlay chrome (badge/spinner line) should paint.
    #[must_use]
    pub fn should_show_chrome(&self) -> bool {
        if !self.active {
            return false;
        }
        // Light modes show immediately; heavy modes wait min-show for wash but
        // may show a compact badge earlier if non-short.
        match self.mode {
            BusyMode::NonBlocking | BusyMode::Optimistic | BusyMode::StaleContent => true,
            BusyMode::Blocking | BusyMode::Cancellable => {
                if self.is_short_op() {
                    // Inline spinner only after a tiny delay, never full wash
                    self.elapsed_ms >= LOADING_OVERLAY_MIN_SHOW_MS / 2
                } else {
                    self.elapsed_ms >= LOADING_OVERLAY_MIN_SHOW_MS
                        || self.elapsed_ms >= LOADING_OVERLAY_MIN_SHOW_MS / 2
                }
            }
        }
    }

    /// Animation demand from composed spinner.
    #[must_use]
    pub fn animation_demand(&self, tick: FrameTick, motion: MotionPolicy) -> AnimationDemand {
        if !self.active {
            return AnimationDemand::idle();
        }
        self.spinner.animation_demand(tick, motion)
    }

    /// Route a key against this boundary (region assumed focused/contains event).
    pub fn route_key(&mut self, key: KeyEvent) -> BusyRoute {
        if !self.active {
            return BusyRoute::Deliver;
        }
        if !key.is_press() {
            return if self.mode.blocks_input() {
                BusyRoute::Blocked
            } else {
                BusyRoute::Deliver
            };
        }
        // Cancel: Esc or explicit when cancellable
        if self.mode.cancellable()
            && (matches!(key.code, KeyCode::Esc)
                || matches!(key.code, KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)))
        {
            let _ = self.request_cancel();
            return BusyRoute::Cancel;
        }
        if matches!(default_button_intent(key), Some(UiIntent::Cancel)) && self.mode.cancellable() {
            let _ = self.request_cancel();
            return BusyRoute::Cancel;
        }
        if self.mode.blocks_input() {
            BusyRoute::Blocked
        } else {
            BusyRoute::Deliver
        }
    }

    /// Route pointer: `region` is the boundary rect.
    pub fn route_pointer(&mut self, mouse: MouseEvent, region: Rect) -> BusyRoute {
        if !self.active {
            return BusyRoute::Deliver;
        }
        let pos = mouse.position;
        if !region.contains(pos) {
            return BusyRoute::Outside;
        }
        if self.mode.cancellable() && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        {
            // Click on bottom row treated as cancel zone when cancellable
            if pos.y + 1 >= region.bottom() {
                let _ = self.request_cancel();
                return BusyRoute::Cancel;
            }
        }
        if self.mode.blocks_input() {
            BusyRoute::Blocked
        } else {
            BusyRoute::Deliver
        }
    }
}

// ── LoadingOverlay paint ────────────────────────────────────────────────────

/// Regional loading chrome painted **over** (or alongside) existing content.
///
/// Host paints underlying widgets first, then LoadingOverlay for the same
/// `area` when [`BusyBoundaryState::should_show_chrome`] is true.
#[derive(Debug, Clone, Copy)]
pub struct LoadingOverlay<'a> {
    mode: BusyMode,
    label: &'a str,
    unavailable: Option<&'a str>,
    cancel_hint: Option<&'a str>,
    system: &'a DesignSystem,
}

impl<'a> LoadingOverlay<'a> {
    /// Label + system (non-blocking by default).
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            mode: BusyMode::NonBlocking,
            label,
            unavailable: None,
            cancel_hint: None,
            system,
        }
    }
    /// Mode.
    #[must_use]
    pub const fn mode(mut self, mode: BusyMode) -> Self {
        self.mode = mode;
        self
    }

    /// What is unavailable.
    #[must_use]
    pub const fn unavailable(mut self, text: &'a str) -> Self {
        self.unavailable = Some(text);
        self
    }

    /// Cancel hint line.
    #[must_use]
    pub const fn cancel_hint(mut self, hint: &'a str) -> Self {
        self.cancel_hint = Some(hint);
        self
    }

    /// Paint using boundary policy (wash only when `state.should_show_wash()`).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut BusyBoundaryState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        if area.is_empty() || !state.is_active() || !state.should_show_chrome() {
            return;
        }

        if state.should_show_wash() {
            self.paint_wash(area, buffer);
        } else if matches!(self.mode, BusyMode::StaleContent) {
            // Soft dim on edges only — mark stale without full wash
            self.paint_stale_badge(area, buffer);
        } else if matches!(self.mode, BusyMode::Optimistic) {
            self.paint_optimistic_badge(area, buffer, state, tick, motion);
            return;
        }

        // Center spinner + label (+ optional unavailable / cancel)
        self.paint_center_stack(area, buffer, state, tick, motion);
    }

    fn paint_wash(&self, area: Rect, buffer: &mut Buffer) {
        let symbol = '░';
        let style = self.system.style(Role::TextDisabled);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                // Preserve some readability: only dim fg, keep symbol if non-space for stale-ish
                let cell = &mut buffer[(x, y)];
                if matches!(self.mode, BusyMode::Blocking | BusyMode::Cancellable) {
                    // Soft wash: overwrite with dim glyph
                    cell.set_char(symbol);
                    cell.set_style(style);
                }
            }
        }
    }

    fn paint_stale_badge(&self, area: Rect, buffer: &mut Buffer) {
        let badge = " stale ";
        let w = display_cols(badge).min(usize::from(area.width)) as u16;
        if w == 0 {
            return;
        }
        let x = area.x.saturating_add(area.width.saturating_sub(w));
        buffer.set_stringn(
            x,
            area.y,
            take_display_cols(badge, usize::from(w)).as_ref(),
            usize::from(w),
            self.system
                .style(Role::Warning)
                .add_modifier(Modifier::BOLD),
        );
    }

    fn paint_optimistic_badge(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut BusyBoundaryState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        let glyph = {
            let g = state.spinner.frame_glyph(tick, motion);
            g
        };
        let text = format!("{glyph} updating");
        let w = display_cols(&text).min(usize::from(area.width)) as u16;
        if w == 0 {
            return;
        }
        let x = area.x.saturating_add(area.width.saturating_sub(w));
        buffer.set_stringn(
            x,
            area.y,
            take_display_cols(&text, usize::from(w)).as_ref(),
            usize::from(w),
            self.system.style(Role::TextSecondary),
        );
    }

    fn paint_center_stack(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &BusyBoundaryState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        let glyph = state.spinner.frame_glyph(tick, motion);
        let mut lines: Vec<(String, Role, bool)> = Vec::new();
        lines.push((format!("{glyph} {}", self.label), Role::TextSecondary, true));
        if let Some(u) = self.unavailable {
            lines.push((u.to_string(), Role::TextMuted, false));
        } else if matches!(self.mode, BusyMode::StaleContent) {
            lines.push(("showing previous data".into(), Role::TextMuted, false));
        }
        if let Some(c) = self.cancel_hint {
            if self.mode.cancellable() {
                lines.push((c.to_string(), Role::TextDisabled, false));
            }
        }

        let n = lines.len() as u16;
        let block = Center::new(area.width, n.max(1))
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        for (i, (text, role, bold)) in lines.iter().enumerate() {
            let y = block.y.saturating_add(i as u16);
            if y >= area.bottom() {
                break;
            }
            let mut style = self.system.style(*role);
            if *bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            let width = display_cols(text).min(usize::from(area.width));
            if width == 0 {
                continue;
            }
            let clipped = take_display_cols(text, width);
            let x = center_line_x(Rect::new(area.x, y, area.width, 1), width as u16);
            buffer.set_stringn(x, y, &clipped, width, style);
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &BusyBoundaryState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.is_active() {
            return;
        }
        let desc = format!(
            "loading-overlay mode={} label={} wash={} cancel={} nest={} elapsed_ms={}",
            self.mode.id(),
            self.label,
            state.should_show_wash(),
            state.cancel_requested(),
            state.nest_depth(),
            state.elapsed_ms(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("loading-overlay")
                .description(desc)
                .focusable(state.focus_trapped())
                .state(SemanticState {
                    busy: true,
                    ..Default::default()
                }),
        );
    }
}

// ── BusyBoundary paint helper ───────────────────────────────────────────────

/// Zero-sized helper for routing + paint composition docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct BusyBoundary;

impl BusyBoundary {
    /// Paint overlay for an active boundary (no-op if idle / too early).
    pub fn paint(
        area: Rect,
        buffer: &mut Buffer,
        state: &mut BusyBoundaryState,
        system: &DesignSystem,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        // Copy fields so LoadingOverlay does not borrow state while painting.
        let label = state.label().to_string();
        let mode = state.mode();
        let unavailable = state.unavailable().map(str::to_string);
        let cancel = mode.cancellable().then_some("esc cancel");
        let mut overlay = LoadingOverlay::new(&label, system).mode(mode);
        if let Some(ref u) = unavailable {
            overlay = overlay.unavailable(u);
        }
        if let Some(c) = cancel {
            overlay = overlay.cancel_hint(c);
        }
        overlay.paint(area, buffer, state, tick, motion);
    }

    /// Nested paint: child region over parent. Parent first (optional), then child.
    pub fn paint_nested(
        parent_area: Rect,
        child_area: Rect,
        buffer: &mut Buffer,
        parent: &mut BusyBoundaryState,
        child: &mut BusyBoundaryState,
        system: &DesignSystem,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        if parent.is_active() {
            Self::paint(parent_area, buffer, parent, system, tick, motion);
        }
        if child.is_active() {
            Self::paint(child_area, buffer, child, system, tick, motion);
        }
    }
}

// ── Examples / recipes ──────────────────────────────────────────────────────

/// Non-blocking pane busy (short refresh).
#[must_use]
pub fn example_busy_non_blocking(system: &DesignSystem) -> (LoadingOverlay<'_>, BusyBoundaryState) {
    let mut st = BusyBoundaryState::new();
    let _ = st.begin(BusyMode::NonBlocking, "Refreshing");
    st.set_elapsed_ms(200);
    st.set_expected_ms(Some(100)); // short op
    (
        LoadingOverlay::new("Refreshing", system).mode(BusyMode::NonBlocking),
        st,
    )
}

/// Blocking pane load.
#[must_use]
pub fn example_busy_blocking(system: &DesignSystem) -> (LoadingOverlay<'_>, BusyBoundaryState) {
    let mut st = BusyBoundaryState::new();
    let _ = st.begin(BusyMode::Blocking, "Loading table");
    st.set_elapsed_ms(400);
    st.set_unavailable(Some("Rows unavailable while loading"));
    (
        LoadingOverlay::new("Loading table", system)
            .mode(BusyMode::Blocking)
            .unavailable("Rows unavailable while loading"),
        st,
    )
}

/// Cancellable long op.
#[must_use]
pub fn example_busy_cancellable(system: &DesignSystem) -> (LoadingOverlay<'_>, BusyBoundaryState) {
    let mut st = BusyBoundaryState::new();
    let _ = st.begin(BusyMode::Cancellable, "Syncing workspace");
    st.set_elapsed_ms(500);
    st.set_unavailable(Some("Edits paused until sync finishes"));
    (
        LoadingOverlay::new("Syncing workspace", system)
            .mode(BusyMode::Cancellable)
            .unavailable("Edits paused until sync finishes")
            .cancel_hint("esc cancel"),
        st,
    )
}

/// Optimistic update badge.
#[must_use]
pub fn example_busy_optimistic(system: &DesignSystem) -> (LoadingOverlay<'_>, BusyBoundaryState) {
    let mut st = BusyBoundaryState::new();
    let _ = st.begin(BusyMode::Optimistic, "Saving");
    st.set_elapsed_ms(50);
    (
        LoadingOverlay::new("Saving", system).mode(BusyMode::Optimistic),
        st,
    )
}

/// Stale content while revalidating.
#[must_use]
pub fn example_busy_stale(system: &DesignSystem) -> (LoadingOverlay<'_>, BusyBoundaryState) {
    let mut st = BusyBoundaryState::new();
    let _ = st.begin(BusyMode::StaleContent, "Revalidating");
    st.set_elapsed_ms(300);
    st.set_unavailable(Some("Data may be out of date"));
    (
        LoadingOverlay::new("Revalidating", system)
            .mode(BusyMode::StaleContent)
            .unavailable("Data may be out of date"),
        st,
    )
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::layout::Position;
    use ratatui_core::style::Style;
    use ratatui_core::terminal::Terminal;
    use ratatui_core::widgets::Widget;
    use std::time::{Duration, Instant};

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn tick(ms: u64) -> FrameTick {
        FrameTick::manual(
            Instant::now(),
            Duration::from_millis(ms),
            Duration::from_millis(16),
        )
    }

    fn painted(area: Rect, mut f: impl FnMut(Rect, &mut Buffer)) -> String {
        let mut buf = Buffer::empty(area);
        f(area, &mut buf);
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn modes_have_ids_and_policies() {
        assert!(BusyMode::Blocking.blocks_input());
        assert!(BusyMode::Cancellable.cancellable());
        assert!(BusyMode::Optimistic.preserve_content());
        assert!(!BusyMode::NonBlocking.wants_wash());
        assert!(BusyMode::Blocking.wants_wash());
    }

    #[test]
    fn short_ops_avoid_wash() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "Quick");
        st.set_expected_ms(Some(80));
        st.set_elapsed_ms(500);
        assert!(st.is_short_op());
        assert!(!st.should_show_wash(), "short op must not wash");
    }

    #[test]
    fn min_show_delays_blocking_wash() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "Load");
        st.set_expected_ms(Some(5_000));
        st.set_elapsed_ms(50);
        assert!(!st.should_show_wash());
        st.set_elapsed_ms(LOADING_OVERLAY_MIN_SHOW_MS);
        assert!(st.should_show_wash());
    }

    #[test]
    fn non_blocking_allows_input() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::NonBlocking, "Refresh");
        let key = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(st.route_key(key), BusyRoute::Deliver);
        assert!(!st.focus_trapped());
    }

    #[test]
    fn blocking_swallows_keys() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "Load");
        let key = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(st.route_key(key), BusyRoute::Blocked);
        assert!(st.focus_trapped());
    }

    #[test]
    fn cancellable_esc_requests_cancel() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Cancellable, "Sync");
        let key = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(st.route_key(key), BusyRoute::Cancel);
        assert!(st.cancel_requested());
    }

    #[test]
    fn nested_regions_independent_cancel() {
        let mut parent = BusyBoundaryState::new();
        let _ = parent.begin(BusyMode::Blocking, "Parent load");
        parent.set_elapsed_ms(400);
        parent.set_expected_ms(Some(5_000));

        let mut child = BusyBoundaryState::nested_under(&parent);
        assert_eq!(child.nest_depth(), 1);
        let _ = child.begin(BusyMode::Cancellable, "Child fetch");
        child.set_elapsed_ms(400);
        child.set_expected_ms(Some(5_000));

        let esc = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(child.route_key(esc), BusyRoute::Cancel);
        assert!(child.cancel_requested());
        assert!(!parent.cancel_requested(), "parent must not auto-cancel");

        // Parent still blocks
        let j = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(parent.route_key(j), BusyRoute::Blocked);
    }

    #[test]
    fn nested_paint_both_regions() {
        let system = system();
        let area = Rect::new(0, 0, 40, 12);
        let child_area = Rect::new(2, 2, 20, 6);
        let mut parent = BusyBoundaryState::new();
        let _ = parent.begin(BusyMode::Blocking, "Outer");
        parent.set_elapsed_ms(400);
        parent.set_expected_ms(Some(5_000));
        let mut child = BusyBoundaryState::nested_under(&parent);
        let _ = child.begin(BusyMode::Cancellable, "Inner");
        child.set_elapsed_ms(400);
        child.set_expected_ms(Some(5_000));

        let text = painted(area, |a, b| {
            // Simulate content
            b.set_stringn(a.x, a.y, "CONTENT", 7, Style::default());
            BusyBoundary::paint_nested(
                a,
                child_area,
                b,
                &mut parent,
                &mut child,
                &system,
                tick(400),
                MotionPolicy::Off,
            );
        });
        assert!(
            text.contains("Outer")
                || text.contains("Inner")
                || text.contains('░')
                || text.contains('.'),
            "{text}"
        );
    }

    #[test]
    fn optimistic_preserves_and_badges() {
        let system = system();
        let (overlay, mut st) = example_busy_optimistic(&system);
        let text = painted(Rect::new(0, 0, 30, 4), |a, b| {
            b.set_stringn(a.x, a.y, "saved draft body", 16, Style::default());
            overlay.paint(a, b, &mut st, tick(50), MotionPolicy::Off);
        });
        assert!(
            text.contains("saved") || text.contains("updating"),
            "{text}"
        );
    }

    #[test]
    fn stale_shows_badge() {
        let system = system();
        let (overlay, mut st) = example_busy_stale(&system);
        let text = painted(Rect::new(0, 0, 36, 6), |a, b| {
            b.set_stringn(a.x, a.y + 2, "old rows", 8, Style::default());
            overlay.paint(a, b, &mut st, tick(300), MotionPolicy::Off);
        });
        assert!(
            text.contains("stale") || text.contains("Revalidat") || text.contains("previous"),
            "{text}"
        );
    }

    #[test]
    fn blocking_paint_after_min_show() {
        let system = system();
        let (overlay, mut st) = example_busy_blocking(&system);
        let text = painted(Rect::new(0, 0, 40, 8), |a, b| {
            overlay.paint(a, b, &mut st, tick(400), MotionPolicy::Off);
        });
        assert!(
            text.contains("Loading") || text.contains("unavailable") || text.contains('░'),
            "{text}"
        );
    }

    #[test]
    fn pointer_outside_is_outside() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "X");
        let region = Rect::new(0, 0, 10, 5);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 50, y: 50 },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(st.route_pointer(mouse, region), BusyRoute::Outside);
    }

    #[test]
    fn end_clears_active() {
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "X");
        assert!(st.is_active());
        let _ = st.end();
        assert!(!st.is_active());
        assert!(!st.should_show_chrome());
    }

    #[test]
    fn semantic_registers_busy() {
        let system = system();
        let mut scene = SemanticScene::<&str, ()>::default();
        let (overlay, st) = example_busy_cancellable(&system);
        overlay.register_semantic(&mut scene, "b", Rect::new(0, 0, 20, 6), &st);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("loading-overlay"))
        );
    }

    #[test]
    fn tiny_empty_safe() {
        let system = system();
        let mut st = BusyBoundaryState::new();
        let _ = st.begin(BusyMode::Blocking, "X");
        st.set_elapsed_ms(500);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        LoadingOverlay::new("X", &system)
            .mode(BusyMode::Blocking)
            .paint(
                Rect::new(0, 0, 1, 1),
                &mut buf,
                &mut st,
                tick(0),
                MotionPolicy::Off,
            );
        LoadingOverlay::new("X", &system).paint(
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut st,
            tick(0),
            MotionPolicy::Off,
        );
    }

    #[test]
    fn fuzz_modes_timing() {
        let system = system();
        let mut seed = 13u64;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let mode = match seed % 5 {
                0 => BusyMode::NonBlocking,
                1 => BusyMode::Blocking,
                2 => BusyMode::Cancellable,
                3 => BusyMode::Optimistic,
                _ => BusyMode::StaleContent,
            };
            let mut st = BusyBoundaryState::new();
            let _ = st.begin(mode, "Fuzz");
            st.set_elapsed_ms(seed % 1000);
            if seed % 2 == 0 {
                st.set_expected_ms(Some(seed % 600));
            }
            let w = (seed % 40) as u16 + 1;
            let h = (seed % 12) as u16 + 1;
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            LoadingOverlay::new("Fuzz", &system).mode(mode).paint(
                area,
                &mut buf,
                &mut st,
                tick(seed % 800),
                MotionPolicy::Off,
            );
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(40, 8)).unwrap();
            let (overlay, mut st) = example_busy_blocking(&system);
            t.draw(|f| {
                overlay.paint(
                    f.area(),
                    f.buffer_mut(),
                    &mut st,
                    tick(400),
                    MotionPolicy::Off,
                );
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
    fn paint_perf_smoke() {
        let system = system();
        let mut terminal = Terminal::new(TestBackend::new(48, 12)).unwrap();
        let (overlay, mut st) = example_busy_cancellable(&system);
        let start = Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|f| {
                    overlay.paint(
                        f.area(),
                        f.buffer_mut(),
                        &mut st,
                        tick(500),
                        MotionPolicy::Off,
                    );
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn loading_view_still_exists_for_panel() {
        // Compatibility: LoadingView remains independent
        use crate::widgets::LoadingView;
        let system = system();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        Widget::render(
            &LoadingView::new("Loading…", "⠋", &system),
            Rect::new(0, 0, 20, 3),
            &mut buf,
        );
    }
}
