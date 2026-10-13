// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Badge — compact status / category indicator with semantic discipline.
//!
//! **Variants:** neutral · info · success · warning · destructive · outline · count.
//! **Actionable** only when [`Badge::interactive`]: selected / focused / disabled
//! then apply; non-interactive badges ignore activation (display-only chrome).
//!
//! Dense-view default: **no background fill** — brackets + role fg + optional
//! [`crate::style::Glyph`] so meaning survives no-color. Optional soft fill for
//! rare emphasis surfaces.
//!
//! References: shadcn Badge, issue labels, btop indicators, agent task status.

#![allow(unused_variables, unused_mut)] // unit-test fixtures
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    default_button_intent,
};
use crate::style::{DesignSystem, Glyph, Role};
use crate::text::{display_cols, take_display_cols};

/// Semantic badge variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BadgeVariant {
    /// Category / meta (muted).
    #[default]
    Neutral,
    /// Informational.
    Info,
    /// Success / healthy.
    Success,
    /// Warning / caution.
    Warning,
    /// Destructive / error / danger.
    Destructive,
    /// Outline chrome (border role, no fill).
    Outline,
    /// Compact numeric count (large values clamp to `99+`).
    Count,
}

impl BadgeVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Destructive => "destructive",
            Self::Outline => "outline",
            Self::Count => "count",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Neutral | Self::Count => Role::TextMuted,
            Self::Info => Role::Info,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Destructive => Role::Danger,
            Self::Outline => Role::Border,
        }
    }

    /// Optional non-color status glyph (never sole meaning — label always present).
    #[must_use]
    pub const fn status_glyph(self) -> Option<Glyph> {
        match self {
            Self::Success => Some(Glyph::Success),
            Self::Warning => Some(Glyph::Warning),
            Self::Destructive => Some(Glyph::Error),
            Self::Info => Some(Glyph::Info),
            _ => None,
        }
    }
}

/// Background fill policy (default soft surface chip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BadgeFill {
    /// Transparent fg-only badge.
    None,
    /// Soft surface under the badge.
    #[default]
    Soft,
}

impl BadgeFill {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Soft => "soft",
        }
    }
}

/// Count formatting for [`BadgeVariant::Count`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BadgeCount {
    /// Raw count.
    pub value: u64,
    /// Cap before `99+` style (default 99).
    pub max_display: u64,
}

impl BadgeCount {
    /// Count with default 99+ clamp.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self {
            value,
            max_display: 99,
        }
    }

    /// Custom clamp threshold.
    #[must_use]
    pub const fn max_display(mut self, max: u64) -> Self {
        self.max_display = if max == 0 { 1 } else { max };
        self
    }

    /// Formatted digits (`"0"` … `"99+"`).
    #[must_use]
    pub fn format(self) -> String {
        if self.value > self.max_display {
            format!("{}+", self.max_display)
        } else {
            self.value.to_string()
        }
    }
}

/// Painted geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BadgeParts {
    /// Outer hit / paint region.
    pub root: Rect,
    /// Content cells actually used (may be narrower than root).
    pub content: Rect,
}

/// Typed outcomes when the badge is interactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BadgeOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Activated (Enter / click).
    Activated,
    /// Selection toggled (host applies).
    Toggled {
        /// New selected flag when host should apply.
        selected: bool,
    },
}

/// Interaction state for actionable badges only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BadgeState {
    /// Keyboard focus (interactive only).
    pub focused: bool,
    /// Selected (interactive only).
    pub selected: bool,
    /// Last painted region.
    pub region: Option<Rect>,
}

impl BadgeState {
    /// Empty state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            selected: false,
            region: None,
        }
    }

    /// Focus flag.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Selection flag.
    pub const fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }
}

/// Compact status / category badge.
#[derive(Debug, Clone, Copy)]
pub struct Badge<'a> {
    label: &'a str,
    system: &'a DesignSystem,
    variant: BadgeVariant,
    count: Option<BadgeCount>,
    fill: BadgeFill,
    interactive: bool,
    disabled: bool,
    /// Prefer glyph prefix for status variants (no-color cue).
    show_glyph: bool,
    /// Max content columns before truncation (0 = use area width).
    max_cols: u16,
}

impl<'a> Badge<'a> {
    /// Neutral/info badge with label (default variant Info for continuity).
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            system,
            variant: BadgeVariant::Info,
            count: None,
            fill: BadgeFill::Soft,
            interactive: false,
            disabled: false,
            show_glyph: true,
            max_cols: 0,
        }
    }

    /// Count badge (`42`, `99+`).
    #[must_use]
    pub const fn count(value: u64, system: &'a DesignSystem) -> Self {
        Self {
            label: "",
            system,
            variant: BadgeVariant::Count,
            count: Some(BadgeCount::new(value)),
            fill: BadgeFill::Soft,
            interactive: false,
            disabled: false,
            show_glyph: false,
            max_cols: 0,
        }
    }

    /// Variant.
    #[must_use]
    pub const fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Neutral category.
    #[must_use]
    pub const fn neutral(mut self) -> Self {
        self.variant = BadgeVariant::Neutral;
        self
    }

    /// Success.
    #[must_use]
    pub const fn success(mut self) -> Self {
        self.variant = BadgeVariant::Success;
        self
    }

    /// Warning.
    #[must_use]
    pub const fn warning(mut self) -> Self {
        self.variant = BadgeVariant::Warning;
        self
    }

    /// Destructive.
    #[must_use]
    pub const fn destructive(mut self) -> Self {
        self.variant = BadgeVariant::Destructive;
        self
    }

    /// Outline.
    #[must_use]
    pub const fn outline(mut self) -> Self {
        self.variant = BadgeVariant::Outline;
        self
    }

    /// Info (default for `new`).
    #[must_use]
    pub const fn info(mut self) -> Self {
        self.variant = BadgeVariant::Info;
        self
    }

    /// Optional count overlay (renders `label count` or count alone).
    #[must_use]
    pub const fn with_count(mut self, value: u64) -> Self {
        self.count = Some(BadgeCount::new(value));
        self
    }

    /// Count formatting.
    #[must_use]
    pub const fn count_spec(mut self, count: BadgeCount) -> Self {
        self.count = Some(count);
        self
    }

    /// Soft fill under badge (opt-in; default none for dense views).
    #[must_use]
    pub const fn fill(mut self, fill: BadgeFill) -> Self {
        self.fill = fill;
        self
    }

    /// Soft fill convenience.
    #[must_use]
    pub const fn soft(mut self) -> Self {
        self.fill = BadgeFill::Soft;
        self
    }

    /// Actionable badge (focus / select / activate apply).
    #[must_use]
    pub const fn interactive(mut self, on: bool) -> Self {
        self.interactive = on;
        self
    }

    /// Disabled (interactive only meaningful).
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    /// Toggle status glyph prefix.
    #[must_use]
    pub const fn show_glyph(mut self, on: bool) -> Self {
        self.show_glyph = on;
        self
    }

    /// Cap content width (truncation).
    #[must_use]
    pub const fn max_cols(mut self, cols: u16) -> Self {
        self.max_cols = cols;
        self
    }

    /// Whether interactive.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        self.interactive
    }

    /// Variant.
    #[must_use]
    pub const fn variant_of(&self) -> BadgeVariant {
        self.variant
    }

    /// Visible body text (without brackets).
    #[must_use]
    pub fn body_text(&self) -> String {
        match self.variant {
            BadgeVariant::Count => self
                .count
                .map(BadgeCount::format)
                .unwrap_or_else(|| "0".into()),
            _ => {
                let mut s = self.label.trim().to_string();
                if let Some(c) = self.count {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(&c.format());
                }
                s
            }
        }
    }

    /// Full painted string including brackets / glyph (for measure + plain).
    #[must_use]
    pub fn decorated(&self, state: Option<&BadgeState>) -> String {
        let mut inner = String::new();
        if self.show_glyph
            && let Some(g) = self.variant.status_glyph()
        {
            let cell = self.system.glyphs.resolve(g).text;
            inner.push_str(cell);
            inner.push(' ');
        }
        inner.push_str(&self.body_text());
        if self.disabled {
            let mark = self.system.glyphs.disabled_mark();
            inner.push(' ');
            inner.push_str(mark);
        }
        // Interactive selected cue (non-color): trailing *
        if self.interactive
            && state.is_some_and(|s| s.selected)
            && !matches!(self.variant, BadgeVariant::Count)
        {
            inner.push('*');
        }
        if matches!(self.variant, BadgeVariant::Outline) {
            return if self.system.glyphs.is_ascii() {
                format!("[{inner}]")
            } else {
                format!("⟨{inner}⟩")
            };
        }
        if matches!(self.fill, BadgeFill::Soft) && !self.system.glyphs.is_ascii() {
            return format!(" {inner} ");
        }
        // Transparent/ASCII badges retain explicit structural delimiters.
        match self.variant {
            BadgeVariant::Count => format!("({inner})"),
            BadgeVariant::Outline => format!("⟨{inner}⟩"),
            _ => format!("[{inner}]"),
        }
    }

    /// Display columns needed (untruncated).
    #[must_use]
    pub fn measure_width(&self, state: Option<&BadgeState>) -> u16 {
        u16::try_from(display_cols(&self.decorated(state)))
            .unwrap_or(1)
            .max(1)
    }

    /// Plain text for copy / a11y (includes meaning for status variants).
    #[must_use]
    pub fn plain(&self) -> String {
        let body = self.body_text();
        if let Some(g) = self.variant.status_glyph() {
            format!("{} ({})", body, g.meaning())
        } else {
            body
        }
    }

    fn paint_style(&self, state: Option<&BadgeState>) -> ratatui_core::style::Style {
        let mut style = if self.disabled {
            self.system.style(Role::TextDisabled)
        } else {
            self.system.style(self.variant.role())
        };
        if self.interactive {
            if state.is_some_and(|s| s.focused) {
                style = style
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    .patch(self.system.style(Role::Focus));
            } else if state.is_some_and(|s| s.selected) {
                style = style.add_modifier(Modifier::BOLD);
            }
        }
        if matches!(self.fill, BadgeFill::None) {
            style = ratatui_core::style::Style { bg: None, ..style };
        } else if matches!(self.fill, BadgeFill::Soft) && !self.disabled {
            // Soft: keep role fg; use Surface as quiet underlay only.
            let surface = self.system.style(Role::Surface);
            if let Some(bg) = surface.bg {
                style = style.bg(bg);
            }
        }
        style
    }

    /// Layout content rect inside `area` (left-aligned, natural width).
    #[must_use]
    pub fn layout(&self, area: Rect, state: Option<&BadgeState>) -> BadgeParts {
        if area.is_empty() {
            return BadgeParts {
                root: area,
                content: area,
            };
        }
        let mut text = self.decorated(state);
        let mut budget = usize::from(area.width);
        if self.max_cols > 0 {
            budget = budget.min(usize::from(self.max_cols));
        }
        text = take_display_cols(&text, budget);
        let w = u16::try_from(display_cols(&text))
            .unwrap_or(0)
            .min(area.width);
        BadgeParts {
            root: area,
            content: Rect {
                x: area.x,
                y: area.y,
                width: w,
                height: 1u16.min(area.height),
            },
        }
    }

    /// Paint into `area`. Updates `state.region` when provided.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: Option<&mut BadgeState>,
    ) -> BadgeParts {
        let focused = state.as_ref().is_some_and(|s| s.focused);
        let selected = state.as_ref().is_some_and(|s| s.selected);
        let snap = BadgeState {
            focused,
            selected,
            region: None,
        };
        let parts = self.layout(area, Some(&snap));
        if let Some(s) = state {
            s.region = Some(parts.content);
        }
        if parts.content.is_empty() {
            return parts;
        }
        let mut text = self.decorated(Some(&snap));
        let budget = usize::from(parts.content.width);
        text = take_display_cols(&text, budget);
        let style = self.paint_style(Some(&snap));
        if matches!(self.fill, BadgeFill::Soft) && !parts.content.is_empty() {
            buffer.set_style(parts.content, style);
        }
        buffer.set_stringn(parts.content.x, parts.content.y, &text, budget, style);
        parts
    }

    /// Key path — only when interactive and not disabled.
    pub fn handle_key(&self, state: &mut BadgeState, key: KeyEvent) -> BadgeOutcome {
        if !self.interactive || self.disabled || !state.focused || key.kind != KeyEventKind::Press {
            return BadgeOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            return self.handle_intent(state, intent);
        }
        if matches!(key.code, KeyCode::Char(' ')) {
            return self.activate(state);
        }
        BadgeOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(&self, state: &mut BadgeState, intent: UiIntent) -> BadgeOutcome {
        if !self.interactive || self.disabled || !state.focused {
            return BadgeOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => self.activate(state),
            _ => BadgeOutcome::Ignored,
        }
    }

    /// Key with EventResult.
    pub fn handle_key_result(
        &self,
        state: &mut BadgeState,
        key: KeyEvent,
    ) -> EventResult<BadgeOutcome> {
        match self.handle_key(state, key) {
            BadgeOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Mouse down on painted region.
    pub fn handle_mouse(&self, state: &mut BadgeState, event: MouseEvent) -> BadgeOutcome {
        if !self.interactive
            || self.disabled
            || event.kind != MouseEventKind::Down(MouseButton::Left)
        {
            return BadgeOutcome::Ignored;
        }
        let Some(region) = state.region else {
            return BadgeOutcome::Ignored;
        };
        if !region.contains(event.position) {
            return BadgeOutcome::Ignored;
        }
        state.focused = true;
        self.activate(state)
    }

    fn activate(&self, state: &mut BadgeState) -> BadgeOutcome {
        state.selected = !state.selected;
        BadgeOutcome::Activated
    }

    /// Register semantic node (interactive → control; else content).
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: Option<&BadgeState>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area, state);
        if parts.content.is_empty() {
            return;
        }
        let label = self.body_text();
        let desc = format!(
            "badge {}; {}",
            self.variant.id(),
            if self.interactive {
                "interactive"
            } else {
                "static"
            }
        );
        let node = if self.interactive && !self.disabled {
            SemanticNode::control(id, parts.content)
                .role(SemanticRole::Button)
                .label(label)
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.is_some_and(|s| s.selected),
                    ..Default::default()
                })
        } else {
            SemanticNode::content(id, parts.content)
                .role(SemanticRole::Status)
                .label(label)
                .description(desc)
                .focusable(false)
        };
        let _ = scene.register(node);
    }
}

impl Widget for &Badge<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer, None);
    }
}

impl Widget for Badge<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::style::GlyphSet;

    #[test]
    fn variants_paint_soft_chip() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
        let _ = Badge::new("NEW", &system)
            .info()
            .paint(Rect::new(0, 0, 16, 1), &mut buf, None);
        assert_eq!(buf[(0, 0)].symbol(), " ");
        assert_eq!(buf[(0, 0)].bg, system.style(Role::Surface).bg.unwrap());
    }

    #[test]
    fn badge_default_is_soft_chip() {
        let system = DesignSystem::default();
        assert_eq!(Badge::new("NEW", &system).fill, BadgeFill::Soft);
        assert_eq!(BadgeFill::default(), BadgeFill::Soft);
    }

    #[test]
    fn count_clamps_large() {
        let system = DesignSystem::default();
        let b = Badge::count(150, &system);
        assert_eq!(b.body_text(), "99+");
        let b = Badge::count(12, &system);
        assert_eq!(b.body_text(), "12");
    }

    #[test]
    fn success_includes_glyph_and_meaning_in_plain() {
        let system = DesignSystem::default();
        let b = Badge::new("ok", &system).success();
        let d = b.decorated(None);
        assert!(d.contains('✓') || d.contains('+'));
        assert!(b.plain().contains("success"));
    }

    #[test]
    fn ascii_glyph_fallback() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let b = Badge::new("ok", &system).success();
        let d = b.decorated(None);
        assert!(d.contains('+'));
    }

    #[test]
    fn non_interactive_ignores_keys() {
        let system = DesignSystem::default();
        let b = Badge::new("x", &system);
        let mut state = BadgeState::new();
        state.set_focused(true);
        assert_eq!(
            b.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            BadgeOutcome::Ignored
        );
    }

    #[test]
    fn interactive_activates() {
        let system = DesignSystem::default();
        let b = Badge::new("filter", &system).interactive(true);
        let mut state = BadgeState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = b.paint(Rect::new(0, 0, 20, 1), &mut buf, Some(&mut state));
        assert_eq!(
            b.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            BadgeOutcome::Activated
        );
        assert!(state.selected);
    }

    #[test]
    fn disabled_blocks_activation() {
        let system = DesignSystem::default();
        let b = Badge::new("x", &system).interactive(true).disabled(true);
        let mut state = BadgeState::new();
        state.set_focused(true);
        assert_eq!(
            b.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            BadgeOutcome::Ignored
        );
    }

    #[test]
    fn narrow_truncates() {
        let system = DesignSystem::default();
        let b = Badge::new("verylongbadge", &system).max_cols(6);
        let parts = b.layout(Rect::new(0, 0, 40, 1), None);
        assert!(parts.content.width <= 6);
    }

    #[test]
    fn outline_uses_angle_brackets() {
        let system = DesignSystem::default();
        let d = Badge::new("meta", &system).outline().decorated(None);
        assert!(d.starts_with('⟨') || d.starts_with('['));
    }

    #[test]
    fn default_fill_uses_surface_background() {
        let system = DesignSystem::default();
        let b = Badge::new("x", &system).success();
        let style = b.paint_style(None);
        assert_eq!(style.bg, system.style(Role::Surface).bg);
    }

    #[test]
    fn measure_and_layout_cheap() {
        let system = DesignSystem::default();
        let b = Badge::new("live", &system).success().with_count(3);
        for _ in 0..20_000 {
            let _ = b.measure_width(None);
            let _ = b.layout(Rect::new(0, 0, 20, 1), None);
        }
    }

    #[test]
    fn semantic_static_vs_interactive() {
        use crate::interaction::SemanticScene;
        let system = DesignSystem::default();
        let b = Badge::new("task", &system).warning();
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        b.register_semantic(&mut scene, "b1", Rect::new(0, 0, 20, 1), None);
        assert!(!scene.nodes()[0].focusable);

        let bi = Badge::new("tag", &system).interactive(true);
        let mut state = BadgeState::new();
        bi.register_semantic(&mut scene, "b2", Rect::new(0, 1, 20, 1), Some(&state));
        assert!(scene.nodes().iter().any(|n| n.focusable));
        let _ = state;
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = Badge::new("x", &system).paint(Rect::new(0, 0, 0, 0), &mut buf, None);
        assert!(parts.content.is_empty() || parts.root.is_empty());
    }

    #[test]
    fn variant_ids_stable() {
        assert_eq!(BadgeVariant::Destructive.id(), "destructive");
        assert_eq!(BadgeFill::None.id(), "none");
    }
}
