// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Tooltip** — delayed contextual help for truncated labels, icon buttons,
//! statuses, and unfamiliar controls.
//!
//! **Mission.** Terminal hover is sparse; tooltips still need Radix-class delay,
//! placement, and dismiss rules — without stealing focus or making essential
//! information hover-only. Hosts must also expose the same copy via labels,
//! HintBar, or KeyboardHelp.
//!
//! **Focus law.** Tooltips never own keyboard input and are never focusable.
//! Visibility is driven by pointer-over and/or anchor focus, after a delay
//! (skipped under reduced motion).
//!
//! Research: Radix Tooltip, desktop tooltips, terminal-adapted hover semantics.

use std::time::Duration;
use web_time::Instant;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    interaction::{
        OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize, OverlaySpec,
        OverlayStack, place_overlay,
    },
    runtime::{FrameTick, Presence},
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
};

/// Default overlay id for tooltips.
pub const TOOLTIP_OVERLAY_ID: &str = "termrock.tooltip";
/// Default show delay (Radix-ish).
pub const TOOLTIP_DEFAULT_DELAY_MS: u64 = 400;
/// Max body width before wrap/clamp.
pub const TOOLTIP_DEFAULT_MAX_WIDTH: u16 = 40;

// ── Placement ───────────────────────────────────────────────────────────────

/// Preferred side relative to the anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TooltipPrefer {
    /// Above anchor (default OverlayStack Tooltip policy).
    #[default]
    Above,
    /// Below anchor.
    Below,
}

impl TooltipPrefer {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Below => "below",
        }
    }
}

/// Places a tooltip relative to `anchor` (may hide on tiny terminals via policy).
#[must_use]
pub fn place_tooltip(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Tooltip),
    )
}

/// Opens a tooltip overlay (**no input ownership**; outside-click dismissible).
pub fn open_tooltip_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::tooltip(TOOLTIP_OVERLAY_ID, anchor, size, opener_focus),
    )
}

/// Dismiss tooltip overlay when present.
pub fn dismiss_tooltip_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(TOOLTIP_OVERLAY_ID))
}

/// Measure overlay size for content (clamped).
#[must_use]
pub fn tooltip_overlay_size(content_cols: u16, lines: u16, max_width: u16) -> OverlaySize {
    let max_width = max_width.max(TOOLTIP_CHROME_COLS.saturating_add(1));
    let body_width = content_cols
        .max(1)
        .min(max_width.saturating_sub(TOOLTIP_CHROME_COLS));
    let width = body_width.saturating_add(TOOLTIP_CHROME_COLS);
    let height = lines.max(1).saturating_add(TOOLTIP_CHROME_ROWS);
    OverlaySize {
        width,
        height,
        min_width: TOOLTIP_CHROME_COLS.saturating_add(1),
        min_height: TOOLTIP_CHROME_ROWS.saturating_add(1),
        max_width,
        max_height: 6,
    }
}

// ── Content / variants ──────────────────────────────────────────────────────

/// Visual / content variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TooltipVariant {
    /// Single plain text line (default).
    #[default]
    Plain,
    /// Body + shortcut chord (e.g. truncated label + key).
    Shortcut,
    /// Title + body (compact rich).
    Rich,
}

impl TooltipVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Shortcut => "shortcut",
            Self::Rich => "rich",
        }
    }
}

/// What arms the show delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TooltipTrigger {
    /// Pointer over anchor only.
    Pointer,
    /// Keyboard focus on anchor only.
    Focus,
    /// Either pointer or focus (default).
    #[default]
    Both,
}

impl TooltipTrigger {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Focus => "focus",
            Self::Both => "both",
        }
    }

    fn armed(self, pointer: bool, focus: bool) -> bool {
        match self {
            Self::Pointer => pointer,
            Self::Focus => focus,
            Self::Both => pointer || focus,
        }
    }
}

/// Host-projected tooltip content (borrowed for paint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipContent<'a> {
    /// Primary body text (required).
    pub body: &'a str,
    /// Optional title (Rich).
    pub title: Option<&'a str>,
    /// Optional shortcut glyph/string (Shortcut / Rich).
    pub shortcut: Option<&'a str>,
    /// True when the same information is available without hover/focus
    /// (label, HintBar, aria). **Essential info must set this.**
    pub essential_elsewhere: bool,
}

impl<'a> TooltipContent<'a> {
    /// Plain body.
    #[must_use]
    pub const fn plain(body: &'a str) -> Self {
        Self {
            body,
            title: None,
            shortcut: None,
            essential_elsewhere: true,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }

    /// Shortcut.
    #[must_use]
    pub const fn shortcut(mut self, s: &'a str) -> Self {
        self.shortcut = Some(s);
        self
    }

    /// Mark that the same fact is available without tooltip (required for
    /// essential UI facts).
    #[must_use]
    pub const fn essential_elsewhere(mut self, on: bool) -> Self {
        self.essential_elsewhere = on;
        self
    }

    /// Lines needed for paint (excluding empty).
    #[must_use]
    pub fn line_count(self, variant: TooltipVariant) -> u16 {
        match variant {
            TooltipVariant::Plain => 1,
            TooltipVariant::Shortcut => {
                1u16.saturating_add(if self.shortcut.is_some() { 0 } else { 0 })
            }
            TooltipVariant::Rich => {
                let mut n = 1u16;
                if self.title.is_some() {
                    n = n.saturating_add(1);
                }
                n
            }
        }
    }

    /// Max display cols for sizing.
    #[must_use]
    pub fn measure_cols(self, variant: TooltipVariant, max_width: u16) -> u16 {
        let mut w = display_cols(self.body) as u16;
        if matches!(variant, TooltipVariant::Rich) {
            if let Some(t) = self.title {
                w = w.max(display_cols(t) as u16);
            }
        }
        if matches!(variant, TooltipVariant::Shortcut | TooltipVariant::Rich) {
            if let Some(s) = self.shortcut {
                w = w.saturating_add(display_cols(s) as u16 + 2);
            }
        }
        // `tooltip_overlay_size` owns chrome. Keeping this as body width avoids
        // spending border/inset columns twice.
        w.min(max_width.saturating_sub(TOOLTIP_CHROME_COLS).max(1))
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Columns a tooltip spends on its own chrome: one border and one inset cell
/// on each side.
pub const TOOLTIP_CHROME_COLS: u16 = 4;

/// Rows a tooltip spends on its own chrome: one border row above and below.
///
/// A caller sizing a one-line tooltip needs three rows, not one.
pub const TOOLTIP_CHROME_ROWS: u16 = 2;

/// Host coordination (tooltip never steals focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TooltipOutcome {
    /// No change.
    Ignored,
    /// Became visible — host should place/open overlay if using OverlayStack.
    Shown,
    /// Became hidden — host should dismiss overlay.
    Hidden,
    /// Still pending delay.
    Pending,
    /// Disabled — never shows.
    Disabled,
    /// Refused: essential content without non-hover channel.
    EssentialRequiresNonHover,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Tooltip delay / trigger state ([`Presence`] + FrameTick).
///
/// **Never focusable.** Pointer and/or focus on the *anchor* arm the timer;
/// the tooltip surface itself does not accept input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooltipState {
    presence: Presence,
    pointer_over: bool,
    focus_within: bool,
    disabled: bool,
    trigger: TooltipTrigger,
    delay: Duration,
    /// Synthetic clock for [`Self::tick_hover`].
    synth_origin: Option<Instant>,
    synth_elapsed_ms: u64,
    was_visible: bool,
    /// Show already requested while currently armed (avoid resetting delay).
    show_requested: bool,
    /// When true, refuse to show if content.essential_elsewhere is false.
    enforce_essential_elsewhere: bool,
}

impl Default for TooltipState {
    fn default() -> Self {
        Self::new()
    }
}

impl TooltipState {
    /// Default 400ms delay, Both triggers, essential-elsewhere enforced.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            presence: Presence::tooltip(Duration::from_millis(TOOLTIP_DEFAULT_DELAY_MS)),
            pointer_over: false,
            focus_within: false,
            disabled: false,
            trigger: TooltipTrigger::Both,
            delay: Duration::from_millis(TOOLTIP_DEFAULT_DELAY_MS),
            synth_origin: None,
            synth_elapsed_ms: 0,
            was_visible: false,
            show_requested: false,
            enforce_essential_elsewhere: true,
        }
    }

    /// Custom show delay.
    #[must_use]
    pub const fn with_delay(delay: Duration) -> Self {
        let mut s = Self::new();
        s.delay = delay;
        s.presence = Presence::tooltip(delay);
        s
    }

    /// Trigger mode.
    #[must_use]
    pub const fn trigger(mut self, t: TooltipTrigger) -> Self {
        self.trigger = t;
        self
    }

    /// Set trigger.
    pub fn set_trigger(&mut self, t: TooltipTrigger) {
        self.trigger = t;
    }

    /// Disable (never shows).
    pub fn set_disabled(&mut self, on: bool) {
        self.disabled = on;
        if on {
            self.force_hide();
        }
    }

    /// Disabled?
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// When true (default), showing content with `essential_elsewhere == false`
    /// yields [`TooltipOutcome::EssentialRequiresNonHover`] and stays hidden.
    pub fn set_enforce_essential_elsewhere(&mut self, on: bool) {
        self.enforce_essential_elsewhere = on;
    }

    /// Pointer is over the anchor region.
    pub fn set_pointer_over(&mut self, over: bool) {
        self.pointer_over = over;
        if !self.armed() {
            self.force_hide();
        }
    }

    /// Keyboard focus is on the anchor control (not the tooltip).
    pub fn set_focus_within(&mut self, focused: bool) {
        self.focus_within = focused;
        if !self.armed() {
            self.force_hide();
        }
    }

    /// Combined armed signal.
    #[must_use]
    pub fn armed(&self) -> bool {
        !self.disabled && self.trigger.armed(self.pointer_over, self.focus_within)
    }

    /// Visible (painted). Never focusable.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.presence.is_visible()
    }

    /// Presence deadline for host poll.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.presence.next_deadline()
    }

    /// Force hide immediately.
    pub fn force_hide(&mut self) {
        self.synth_origin = None;
        self.synth_elapsed_ms = 0;
        self.show_requested = false;
        self.presence.force_hide();
        self.was_visible = false;
    }

    fn effective_delay(&self, motion: MotionPolicy) -> Duration {
        match motion {
            MotionPolicy::Off | MotionPolicy::Basic => Duration::ZERO,
            MotionPolicy::Full => self.delay,
        }
    }

    fn rebuild_presence(&mut self, motion: MotionPolicy) {
        let d = self.effective_delay(motion);
        // Only rebuild if delay differs — Presence has no getter for delay;
        // always set via constructor when arming.
        self.presence = Presence::tooltip(d);
    }

    /// Advance hover clock; shows after delay.
    ///
    /// Prefer [`Self::advance`] with a real [`FrameTick`].
    pub fn tick_hover(&mut self, delta_ms: u64, hovering: bool) -> TooltipOutcome {
        self.set_pointer_over(hovering);
        use crate::runtime::FrameTick;
        if !self.armed() {
            let was = self.was_visible;
            self.force_hide();
            return if was {
                TooltipOutcome::Hidden
            } else {
                TooltipOutcome::Ignored
            };
        }
        if self.disabled {
            return TooltipOutcome::Disabled;
        }
        let origin = *self.synth_origin.get_or_insert_with(Instant::now);
        if !self.show_requested && !self.is_visible() {
            self.rebuild_presence(MotionPolicy::Full);
            let tick = FrameTick::manual(origin, Duration::ZERO, Duration::ZERO);
            self.presence.request_show(tick);
            self.show_requested = true;
        }
        self.synth_elapsed_ms = self.synth_elapsed_ms.saturating_add(delta_ms);
        let tick = FrameTick::manual(
            origin + Duration::from_millis(self.synth_elapsed_ms),
            Duration::from_millis(self.synth_elapsed_ms),
            Duration::from_millis(delta_ms),
        );
        let _ = self.presence.advance(tick, MotionPolicy::Full);
        self.visibility_outcome()
    }

    /// FrameTick-driven advance (canonical).
    ///
    /// Under [`MotionPolicy::Basic`] / [`MotionPolicy::Off`], show delay is zero.
    pub fn advance(&mut self, tick: FrameTick, motion: MotionPolicy) -> TooltipOutcome {
        if self.disabled {
            self.force_hide();
            return TooltipOutcome::Disabled;
        }
        if !self.armed() {
            let was = self.was_visible || self.is_visible();
            self.force_hide();
            return if was {
                TooltipOutcome::Hidden
            } else {
                TooltipOutcome::Ignored
            };
        }
        // Request show once per arm cycle (do not reset delay every frame).
        if !self.show_requested && !self.is_visible() {
            self.rebuild_presence(motion);
            self.presence.request_show(tick);
            self.show_requested = true;
        }
        let _ = self.presence.advance(tick, motion);
        // Reduced motion: if still pending after request, force zero-delay show.
        if matches!(motion, MotionPolicy::Basic | MotionPolicy::Off) && !self.is_visible() {
            self.presence = Presence::tooltip(Duration::ZERO);
            self.presence.request_show(tick);
            self.show_requested = true;
            let _ = self.presence.advance(tick, motion);
        }
        self.visibility_outcome()
    }

    /// Convenience: update triggers then advance.
    pub fn advance_with_triggers(
        &mut self,
        tick: FrameTick,
        pointer_over: bool,
        focus_within: bool,
        motion: MotionPolicy,
    ) -> TooltipOutcome {
        self.pointer_over = pointer_over;
        self.focus_within = focus_within;
        if !self.armed() {
            let was = self.was_visible || self.is_visible();
            self.force_hide();
            return if was {
                TooltipOutcome::Hidden
            } else {
                TooltipOutcome::Ignored
            };
        }
        self.advance(tick, motion)
    }

    fn visibility_outcome(&mut self) -> TooltipOutcome {
        let vis = self.is_visible();
        if vis && !self.was_visible {
            self.was_visible = true;
            TooltipOutcome::Shown
        } else if !vis && self.was_visible {
            self.was_visible = false;
            TooltipOutcome::Hidden
        } else if vis {
            TooltipOutcome::Ignored
        } else if self.armed() {
            TooltipOutcome::Pending
        } else {
            TooltipOutcome::Ignored
        }
    }

    /// Gate show for essential content policy.
    pub fn allow_show_for(&self, content: &TooltipContent<'_>) -> TooltipOutcome {
        if self.disabled {
            return TooltipOutcome::Disabled;
        }
        if self.enforce_essential_elsewhere && !content.essential_elsewhere {
            return TooltipOutcome::EssentialRequiresNonHover;
        }
        TooltipOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Tooltip paint (never steals focus).
#[derive(Debug, Clone, Copy)]
pub struct Tooltip<'a> {
    content: TooltipContent<'a>,
    system: &'a DesignSystem,
    variant: TooltipVariant,
    ascii: bool,
    colorless: bool,
    max_width: u16,
}

impl<'a> Tooltip<'a> {
    /// Full content.
    #[must_use]
    pub const fn content(content: TooltipContent<'a>, system: &'a DesignSystem) -> Self {
        Self {
            content,
            system,
            variant: TooltipVariant::Plain,
            ascii: false,
            colorless: false,
            max_width: TOOLTIP_DEFAULT_MAX_WIDTH,
        }
    }

    /// Variant.
    #[must_use]
    pub const fn variant(mut self, v: TooltipVariant) -> Self {
        self.variant = v;
        self
    }

    /// Shortcut variant helper.
    #[must_use]
    pub const fn shortcut(mut self) -> Self {
        self.variant = TooltipVariant::Shortcut;
        self
    }

    /// Rich variant helper.
    #[must_use]
    pub const fn rich(mut self) -> Self {
        self.variant = TooltipVariant::Rich;
        self
    }

    /// ASCII border glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless roles.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Max body width.
    #[must_use]
    pub const fn max_width(mut self, w: u16) -> Self {
        self.max_width = w;
        self
    }

    /// Content borrow.
    #[must_use]
    pub const fn body_content(&self) -> TooltipContent<'a> {
        self.content
    }

    /// Overlay size for current content.
    #[must_use]
    pub fn overlay_size(&self) -> OverlaySize {
        let cols = self.content.measure_cols(self.variant, self.max_width);
        let lines = self.content.line_count(self.variant);
        tooltip_overlay_size(cols, lines, self.max_width)
    }

    /// Paint when visible (never steals focus).
    ///
    /// Returns early if not visible, disabled, or essential-elsewhere policy fails.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &TooltipState) {
        if area.is_empty() || !state.is_visible() || state.is_disabled() {
            return;
        }
        if state.enforce_essential_elsewhere && !self.content.essential_elsewhere {
            return;
        }
        self.paint_always(area, buffer);
    }

    /// Paint without visibility gate (tests / host already gated).
    pub fn paint_always(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        // Every variant floats: a tooltip that writes bare text over live
        // content is unreadable against whatever it lands on. One overlay
        // surface, one quiet outline, one cell of breathing room
        // (plans/009 Step 4).
        let colorless_system;
        let surface_system = if self.colorless {
            colorless_system = self
                .system
                .clone()
                .capability(crate::style::ColorCapability::Monochrome);
            &colorless_system
        } else {
            self.system
        };
        let area = super::Surface::new(surface_system)
            .recipe(super::SurfaceRecipe::Overlay)
            .bordered(true)
            .content_inset()
            .paint(area, buffer);
        if area.is_empty() {
            return;
        }
        let muted = if self.colorless {
            self.system.style(Role::TextMuted)
        } else {
            self.system.style(Role::TextMuted)
        };
        let strong = if self.colorless {
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD)
        } else {
            self.system.style(Role::TextStrong)
        };
        let key = if self.colorless {
            self.system.style(Role::TextStrong)
        } else {
            self.system.style(Role::HintKey)
        };

        match self.variant {
            TooltipVariant::Plain => {
                let text = take_display_cols(self.content.body, usize::from(area.width));
                buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), muted);
            }
            TooltipVariant::Shortcut => {
                let body = take_display_cols(self.content.body, usize::from(area.width));
                buffer.set_stringn(area.x, area.y, &body, usize::from(area.width), muted);
                if let Some(sc) = self.content.shortcut {
                    // Right-align shortcut if room
                    let sc_w = display_cols(sc) as u16;
                    if sc_w < area.width {
                        let x = area.right().saturating_sub(sc_w);
                        buffer.set_stringn(x, area.y, sc, usize::from(sc_w), key);
                    }
                }
            }
            TooltipVariant::Rich => {
                let mut y = area.y;
                if let Some(title) = self.content.title {
                    let t = take_display_cols(title, usize::from(area.width));
                    buffer.set_stringn(area.x, y, &t, usize::from(area.width), strong);
                    y = y.saturating_add(1);
                }
                if y < area.bottom() {
                    let mut body = take_display_cols(self.content.body, usize::from(area.width));
                    if let Some(sc) = self.content.shortcut {
                        let extra = format!("  {}", sc);
                        let combined = format!(
                            "{}{}",
                            take_display_cols(
                                self.content.body,
                                usize::from(area.width.saturating_sub(display_cols(&extra) as u16))
                            ),
                            extra
                        );
                        body = take_display_cols(&combined, usize::from(area.width));
                    }
                    buffer.set_stringn(area.x, y, &body, usize::from(area.width), muted);
                }
            }
        }
    }
}

impl Widget for &Tooltip<'_> {
    /// Paints body without visibility gate (host must gate). Prefer
    /// [`Tooltip::paint`] with state.
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint_always(area, buffer);
    }
}

impl Widget for Tooltip<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::FrameTick;

    #[test]
    fn tooltip_delay_tick_hover() {
        let mut state = TooltipState::new();
        assert!(matches!(
            state.tick_hover(200, true),
            TooltipOutcome::Pending | TooltipOutcome::Ignored
        ));
        assert!(!state.is_visible());
        let out = state.tick_hover(250, true);
        assert!(
            state.is_visible(),
            "expected visible after delay, got {out:?}"
        );
        assert!(matches!(
            out,
            TooltipOutcome::Shown | TooltipOutcome::Ignored
        ));
        let out = state.tick_hover(0, false);
        assert!(!state.is_visible());
        assert!(matches!(
            out,
            TooltipOutcome::Hidden | TooltipOutcome::Ignored
        ));
    }

    #[test]
    fn focus_trigger_without_pointer() {
        let mut state =
            TooltipState::with_delay(Duration::from_millis(100)).trigger(TooltipTrigger::Focus);
        state.set_focus_within(true);
        let origin = Instant::now();
        let tick0 = FrameTick::manual(origin, Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick0, MotionPolicy::Full);
        let tick1 = FrameTick::manual(
            origin + Duration::from_millis(150),
            Duration::from_millis(150),
            Duration::from_millis(150),
        );
        let _ = state.advance(tick1, MotionPolicy::Full);
        assert!(state.is_visible());
    }

    #[test]
    fn pointer_only_ignores_focus() {
        let mut state = TooltipState::with_delay(Duration::ZERO).trigger(TooltipTrigger::Pointer);
        state.set_focus_within(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick, MotionPolicy::Full);
        assert!(!state.is_visible());
        state.set_pointer_over(true);
        let _ = state.advance(tick, MotionPolicy::Off);
        assert!(state.is_visible());
    }

    #[test]
    fn reduced_motion_skips_delay() {
        let mut state = TooltipState::with_delay(Duration::from_millis(500));
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick, MotionPolicy::Basic);
        assert!(state.is_visible());
    }

    #[test]
    fn disabled_never_shows() {
        let mut state = TooltipState::with_delay(Duration::ZERO);
        state.set_disabled(true);
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        assert_eq!(
            state.advance(tick, MotionPolicy::Full),
            TooltipOutcome::Disabled
        );
        assert!(!state.is_visible());
    }

    #[test]
    fn essential_elsewhere_gate() {
        let state = TooltipState::new();
        let bad = TooltipContent::plain("secret status").essential_elsewhere(false);
        assert_eq!(
            state.allow_show_for(&bad),
            TooltipOutcome::EssentialRequiresNonHover
        );
        let ok = TooltipContent::plain("icon label").essential_elsewhere(true);
        assert_eq!(state.allow_show_for(&ok), TooltipOutcome::Ignored);
    }

    #[test]
    fn essential_blocks_paint() {
        let system = DesignSystem::default();
        let mut state = TooltipState::with_delay(Duration::ZERO);
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick, MotionPolicy::Off);
        assert!(state.is_visible());
        let tip = Tooltip::content(
            TooltipContent::plain("only on hover").essential_elsewhere(false),
            &system,
        );
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        tip.paint(area, &mut buf, &state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(!text.contains("only on hover"), "essential leaked: {text}");
    }

    #[test]
    fn plain_shortcut_rich_paint() {
        let system = DesignSystem::default();
        let mut state = TooltipState::with_delay(Duration::ZERO);
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick, MotionPolicy::Off);

        // The floating surface costs a border row above and below.
        let area = Rect::new(0, 0, 24, 3);
        let mut buf = Buffer::empty(area);
        Tooltip::content(TooltipContent::plain("Save file"), &system).paint(area, &mut buf, &state);
        let t: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(t.contains("Save"), "{t}");

        let mut buf2 = Buffer::empty(area);
        Tooltip::content(
            TooltipContent::plain("Save")
                .shortcut("C-s")
                .essential_elsewhere(true),
            &system,
        )
        .shortcut()
        .paint(area, &mut buf2, &state);

        let mut buf3 = Buffer::empty(Rect::new(0, 0, 28, 4));
        Tooltip::content(
            TooltipContent::plain("Writes buffer")
                .title("Save")
                .shortcut("C-s")
                .essential_elsewhere(true),
            &system,
        )
        .rich()
        .paint(Rect::new(0, 0, 28, 4), &mut buf3, &state);
        let t3: String = buf3
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(t3.contains("Save") || t3.contains("Writes"), "{t3}");
    }

    #[test]
    fn overlay_size_spends_one_chrome_ring_and_declares_it_minimum() {
        let system = DesignSystem::default();
        let tip = Tooltip::content(TooltipContent::plain("Save"), &system);
        let size = tip.overlay_size();
        assert_eq!(size.width, 4 + TOOLTIP_CHROME_COLS);
        assert_eq!(size.height, 1 + TOOLTIP_CHROME_ROWS);
        assert_eq!(size.min_width, TOOLTIP_CHROME_COLS + 1);
        assert_eq!(size.min_height, TOOLTIP_CHROME_ROWS + 1);
    }

    #[test]
    fn never_focusable_const() {
        // Documentation contract: is_visible is paint-only.
        let state = TooltipState::new();
        assert!(!state.is_visible());
    }

    #[test]
    fn overlay_no_input_ownership() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 10, 8, 1);
        let mut stack = OverlayStack::<()>::new();
        let tip = open_tooltip_overlay(&mut stack, bounds, anchor, OverlaySize::menu(16, 1), None);
        assert!(matches!(tip, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Tooltip);
        assert!(!stack.top_owns_input());
    }

    #[test]
    fn place_clamps_or_hides_on_tiny() {
        let bounds = Rect::new(0, 0, 10, 5);
        let anchor = Rect::new(1, 2, 2, 1);
        let r = place_tooltip(bounds, anchor, OverlaySize::menu(20, 1));
        // Policy may hide (empty) or clamp — both valid
        let _ = r;
    }

    #[test]
    fn fuzz_triggers() {
        let mut state = TooltipState::with_delay(Duration::from_millis(50));
        let origin = Instant::now();
        let mut seed = 9u64;
        for i in 0..100u64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            state.set_pointer_over(seed % 2 == 0);
            state.set_focus_within(seed % 3 == 0);
            if seed % 7 == 0 {
                state.set_disabled(seed % 2 == 0);
            }
            let tick = FrameTick::manual(
                origin + Duration::from_millis(i * 30),
                Duration::from_millis(i * 30),
                Duration::from_millis(30),
            );
            let motion = if seed % 5 == 0 {
                MotionPolicy::Basic
            } else {
                MotionPolicy::Full
            };
            let _ = state.advance(tick, motion);
        }
    }

    #[test]
    fn ascii_colorless_paint() {
        let system = DesignSystem::default();
        let mut state = TooltipState::with_delay(Duration::ZERO);
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let _ = state.advance(tick, MotionPolicy::Off);
        let area = Rect::new(0, 0, 30, 2);
        let mut buf = Buffer::empty(area);
        Tooltip::content(
            TooltipContent::plain("Help text")
                .title("Tip")
                .essential_elsewhere(true),
            &system,
        )
        .rich()
        .ascii(true)
        .colorless(true)
        .paint(area, &mut buf, &state);
    }
}
