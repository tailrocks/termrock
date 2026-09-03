// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Collapsible — accessible disclosure for optional detail.
//!
//! **Anatomy:** `root` · `trigger` · optional `content` (body).
//!
//! Semantic expand/collapse via [`UiIntent`]. Focus stays on the trigger;
//! content descendants receive focus only while open (host wires focus graph).
//!
//! **Controlled vs uncontrolled.** Pass [`Collapsible::open`] each frame for
//! controlled open state; omit it and [`CollapsibleState`] owns open/closed.
//!
//! Collapsible paints only through `Collapsible::paint(area, buffer, state)`;
//! a stateless render would rebuild `CollapsibleState` per frame and repaint
//! the disclosure closed, losing animation and focus geometry.
//!
//! **Keep-mounted policy.** When closed, layout always collapses body height to
//! zero. [`CollapsedContentPolicy::KeepMounted`] is a host signal: keep domain
//! child *state* alive while not painting; [`Unmount`] allows dropping children.
//!
//! Glyphs use [`GlyphSet`] disclosure markers (ASCII fallbacks).
//!
//! References: Radix Collapsible, tree disclosures, agent tool-detail expansion.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{UiIntent, default_tree_intent};
use crate::style::{DesignSystem, Role};
use crate::text::take_display_cols;

/// Visual density of the trigger row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CollapsibleVariant {
    /// Compact inline trigger (default).
    #[default]
    Inline,
    /// Stronger section-style trigger (text-strong, optional bottom rule).
    Section,
}

impl CollapsibleVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Section => "section",
        }
    }
}

/// Host policy for child widgets while closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CollapsedContentPolicy {
    /// Host may drop / skip painting children (default).
    #[default]
    Unmount,
    /// Host should keep child domain state alive; body geometry is still zero
    /// while closed (paint is skipped).
    KeepMounted,
}

impl CollapsedContentPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unmount => "unmount",
            Self::KeepMounted => "keep-mounted",
        }
    }

    /// Whether host should retain child state when closed.
    #[must_use]
    pub const fn keep_state(self) -> bool {
        matches!(self, Self::KeepMounted)
    }
}

/// Named geometry for one disclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CollapsibleParts {
    /// Outer allocation.
    pub root: Rect,
    /// Trigger / header hit target.
    pub trigger: Rect,
    /// Content body (zero height when closed).
    pub content: Rect,
    /// Clip contract (= content).
    pub clip: Rect,
    /// Whether content is open this frame.
    pub open: bool,
    /// Host child-state policy for this frame (geometry still collapses when closed).
    pub content_policy: CollapsedContentPolicy,
}

/// Typed outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CollapsibleOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Disclosure opened.
    Opened,
    /// Disclosure closed.
    Closed,
}

impl CollapsibleOutcome {
    /// Whether open state changed.
    #[must_use]
    pub const fn changed(self) -> bool {
        matches!(self, Self::Opened | Self::Closed)
    }

    /// Resulting open flag if changed.
    #[must_use]
    pub const fn open(self) -> Option<bool> {
        match self {
            Self::Opened => Some(true),
            Self::Closed => Some(false),
            Self::Ignored => None,
        }
    }
}

/// Interaction + uncontrolled open state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollapsibleState {
    /// Uncontrolled open flag (ignored when paint is controlled).
    pub open: bool,
    /// Trigger owns keyboard focus.
    pub focused: bool,
    /// Cached parts from last paint.
    pub parts: Option<CollapsibleParts>,
}

impl CollapsibleState {
    /// Closed, unfocused.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open: false,
            focused: false,
            parts: None,
        }
    }

    /// Start open.
    #[must_use]
    pub const fn initially_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Whether open (uncontrolled store).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Controlled-style setter for uncontrolled store.
    pub const fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    /// Trigger focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    fn apply_open(&mut self, open: bool) -> CollapsibleOutcome {
        if self.open == open {
            return CollapsibleOutcome::Ignored;
        }
        self.open = open;
        if open {
            CollapsibleOutcome::Opened
        } else {
            CollapsibleOutcome::Closed
        }
    }

    /// Key path via intents (Activate / Toggle / Expand / Collapse).
    ///
    /// Uses the tree intent map so Right/`l` expand and Left/`h` collapse.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        disabled: bool,
        controlled_open: Option<bool>,
    ) -> CollapsibleOutcome {
        if disabled || !self.focused || !key.is_press() {
            return CollapsibleOutcome::Ignored;
        }
        let Some(intent) = default_tree_intent(key) else {
            return CollapsibleOutcome::Ignored;
        };
        self.handle_intent(intent, disabled, controlled_open)
    }

    /// Semantic intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        disabled: bool,
        controlled_open: Option<bool>,
    ) -> CollapsibleOutcome {
        if disabled || !self.focused {
            return CollapsibleOutcome::Ignored;
        }
        // Controlled: emit desired open without mutating store (host applies).
        let current = controlled_open.unwrap_or(self.open);
        match intent {
            UiIntent::Activate | UiIntent::Toggle => {
                let next = !current;
                if controlled_open.is_some() {
                    return if next {
                        CollapsibleOutcome::Opened
                    } else {
                        CollapsibleOutcome::Closed
                    };
                }
                self.apply_open(next)
            }
            UiIntent::Expand => {
                if current {
                    return CollapsibleOutcome::Ignored;
                }
                if controlled_open.is_some() {
                    return CollapsibleOutcome::Opened;
                }
                self.apply_open(true)
            }
            UiIntent::Collapse => {
                if !current {
                    return CollapsibleOutcome::Ignored;
                }
                if controlled_open.is_some() {
                    return CollapsibleOutcome::Closed;
                }
                self.apply_open(false)
            }
            _ => CollapsibleOutcome::Ignored,
        }
    }

    /// Click trigger focuses and toggles (same controlled rules).
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        disabled: bool,
        controlled_open: Option<bool>,
    ) -> CollapsibleOutcome {
        if disabled || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return CollapsibleOutcome::Ignored;
        }
        let Some(parts) = self.parts else {
            return CollapsibleOutcome::Ignored;
        };
        if !parts.trigger.contains(event.position) {
            return CollapsibleOutcome::Ignored;
        }
        self.focused = true;
        let current = controlled_open.unwrap_or(self.open);
        let next = !current;
        if controlled_open.is_some() {
            return if next {
                CollapsibleOutcome::Opened
            } else {
                CollapsibleOutcome::Closed
            };
        }
        self.apply_open(next)
    }
}

/// Disclosure primitive (trigger + optional content band).
#[derive(Debug, Clone)]
pub struct Collapsible<'a> {
    system: &'a DesignSystem,
    trigger: &'a str,
    /// Controlled open override; `None` → state owns open.
    open: Option<bool>,
    variant: CollapsibleVariant,
    disabled: bool,
    content_policy: CollapsedContentPolicy,
    /// Nesting depth (indent = depth×2).
    depth: u8,
    /// Extra indent cells.
    indent: u16,
    /// When open, preferred body height if host does not grow area (0 = rest of area).
    preferred_content_height: u16,
}

impl<'a> Collapsible<'a> {
    /// Inline disclosure with trigger label.
    #[must_use]
    pub const fn new(trigger: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            trigger,
            open: None,
            variant: CollapsibleVariant::Inline,
            disabled: false,
            content_policy: CollapsedContentPolicy::Unmount,
            depth: 0,
            indent: 0,
            preferred_content_height: 0,
        }
    }

    /// Controlled open state for this frame.
    #[must_use]
    pub const fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Variant recipe.
    #[must_use]
    pub const fn variant(mut self, variant: CollapsibleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Section-style trigger.
    #[must_use]
    pub const fn section(mut self) -> Self {
        self.variant = CollapsibleVariant::Section;
        self
    }

    /// Inline compact trigger.
    #[must_use]
    pub const fn inline(mut self) -> Self {
        self.variant = CollapsibleVariant::Inline;
        self
    }

    /// Disabled (no toggle).
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Child state policy while closed.
    #[must_use]
    pub const fn content_policy(mut self, policy: CollapsedContentPolicy) -> Self {
        self.content_policy = policy;
        self
    }

    /// Keep child state while closed.
    #[must_use]
    pub const fn keep_mounted(mut self) -> Self {
        self.content_policy = CollapsedContentPolicy::KeepMounted;
        self
    }

    /// Nesting depth.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Extra left indent.
    #[must_use]
    pub const fn indent(mut self, indent: u16) -> Self {
        self.indent = indent;
        self
    }

    /// Preferred content height when open (0 = fill remaining).
    #[must_use]
    pub const fn preferred_content_height(mut self, rows: u16) -> Self {
        self.preferred_content_height = rows;
        self
    }

    /// Whether this instance is disabled.
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Content policy.
    #[must_use]
    pub const fn policy(&self) -> CollapsedContentPolicy {
        self.content_policy
    }

    /// Resolved open flag for this frame.
    #[must_use]
    pub fn resolved_open(&self, state: &CollapsibleState) -> bool {
        self.open.unwrap_or(state.open)
    }
    fn left_pad(&self) -> u16 {
        self.indent
            .saturating_add(u16::from(self.depth).saturating_mul(2))
    }

    /// Layout without painting.
    #[must_use]
    pub fn layout(&self, area: Rect, state: &CollapsibleState) -> CollapsibleParts {
        if area.is_empty() {
            return CollapsibleParts {
                root: area,
                trigger: area,
                content: area,
                clip: area,
                open: false,
                content_policy: self.content_policy,
            };
        }
        let pad = self.left_pad();
        let x = area.x.saturating_add(pad);
        let width = area.width.saturating_sub(pad);
        let open = self.resolved_open(state);
        let trigger_h = 1u16.min(area.height);
        let trigger = Rect {
            x,
            y: area.y,
            width,
            height: trigger_h,
        };
        let body_y = area.y.saturating_add(trigger_h);
        let content = if !open || body_y >= area.bottom() {
            Rect {
                x,
                y: body_y,
                width,
                height: 0,
            }
        } else {
            let rest = area.bottom().saturating_sub(body_y);
            let h = if self.preferred_content_height > 0 {
                self.preferred_content_height.min(rest)
            } else {
                rest
            };
            Rect {
                x,
                y: body_y,
                width,
                height: h,
            }
        };
        CollapsibleParts {
            root: area,
            trigger,
            content,
            clip: content,
            open,
            content_policy: self.content_policy,
        }
    }

    /// Paint trigger; returns content rect for host children.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut CollapsibleState) -> Rect {
        let parts = self.layout(area, state);
        state.parts = Some(parts);
        if area.is_empty() || parts.trigger.is_empty() {
            return parts.content;
        }

        let open = parts.open;
        let glyph = if open {
            self.system.glyphs.disclosure_open()
        } else {
            self.system.glyphs.disclosure_closed()
        };
        // GlyphSet swaps Unicode ▾/▸ → ASCII v/> so open state survives no-color + ascii.
        let mut label = String::new();
        label.push_str(glyph);
        label.push(' ');
        label.push_str(self.trigger.trim());
        if self.disabled {
            // Non-color disabled cue (color role may collapse under NO_COLOR).
            label.push_str(" ·");
        }

        // A Section trigger is already TextStrong, so "focused" repainted the
        // same style and the focus ring vanished — including for the three
        // Accordion recipes that inherit it. Focus adds weight and an accent
        // gutter; the label tone stays its own (plans/021 Step 4).
        let focused = state.focused && !self.disabled;
        let style = if self.disabled {
            self.system.style(Role::TextDisabled)
        } else {
            let base = match self.variant {
                CollapsibleVariant::Section => self.system.style(Role::TextStrong),
                CollapsibleVariant::Inline => self.system.style(Role::Text),
            };
            if focused {
                base.add_modifier(ratatui_core::style::Modifier::BOLD)
            } else {
                base
            }
        };

        if focused && parts.trigger.width > 0 {
            let gutter = self.system.glyphs.selection_gutter();
            buffer.set_stringn(
                parts.trigger.x,
                parts.trigger.y,
                gutter,
                1,
                self.system.style(Role::Accent),
            );
        }
        let t = take_display_cols(&label, usize::from(parts.trigger.width));
        buffer.set_stringn(
            parts.trigger.x,
            parts.trigger.y,
            &t,
            usize::from(parts.trigger.width),
            style,
        );

        // Section: fill remaining trigger cells with a quiet rule when open (no extra row).
        if matches!(self.variant, CollapsibleVariant::Section) && open && parts.trigger.width > 0 {
            let used = crate::text::display_cols(&t) as u16;
            if used < parts.trigger.width {
                let rule = self.system.glyphs.rule();
                let fill_x = parts.trigger.x.saturating_add(used);
                let fill_w = parts.trigger.width.saturating_sub(used);
                let rule_fill = rule.repeat(usize::from(fill_w));
                let pad = take_display_cols(&rule_fill, usize::from(fill_w));
                buffer.set_stringn(
                    fill_x,
                    parts.trigger.y,
                    pad.as_ref(),
                    usize::from(fill_w),
                    self.system.style(Role::Border),
                );
            }
        }

        parts.content
    }

    /// Register trigger as focusable control (content children registered by host).
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &CollapsibleState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        use crate::interaction::{SemanticNode, SemanticRole, SemanticState};
        let parts = self.layout(area, state);
        let _ = scene.register(
            SemanticNode::control(id, parts.trigger)
                .role(SemanticRole::Button)
                .label(self.trigger)
                .focusable(!self.disabled)
                .state(SemanticState {
                    expanded: parts.open,
                    ..Default::default()
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::widgets::tests::click;

    #[test]
    fn uncontrolled_toggle_via_enter() {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Opened);
        assert!(state.is_open());
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Closed);
    }

    #[test]
    fn expand_collapse_intents() {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        assert_eq!(
            state.handle_intent(UiIntent::Expand, false, None),
            CollapsibleOutcome::Opened
        );
        assert_eq!(
            state.handle_intent(UiIntent::Expand, false, None),
            CollapsibleOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(UiIntent::Collapse, false, None),
            CollapsibleOutcome::Closed
        );
    }

    #[test]
    fn expand_collapse_keys() {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            false,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Opened);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            false,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Closed);
    }

    #[test]
    fn controlled_does_not_mutate_store() {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        state.set_open(false);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            Some(false),
        );
        assert_eq!(out, CollapsibleOutcome::Opened);
        assert!(!state.is_open()); // host must apply
    }

    #[test]
    fn disabled_ignores_keys_and_mouse() {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            true,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Ignored);
        let system = DesignSystem::default();
        let c = Collapsible::new("Details", &system).disabled(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
        let body = c.paint(Rect::new(0, 0, 20, 4), &mut buf, &mut state);
        assert_eq!(body.height, 0);
        let out = state.handle_mouse(click(0, 0), true, None);
        assert_eq!(out, CollapsibleOutcome::Ignored);
    }

    #[test]
    fn closed_body_zero_open_fills_rest() {
        let system = DesignSystem::default();
        let c = Collapsible::new("More", &system);
        let mut state = CollapsibleState::new();
        let parts = c.layout(Rect::new(0, 0, 30, 6), &state);
        assert_eq!(parts.content.height, 0);
        state.set_open(true);
        let parts = c.layout(Rect::new(0, 0, 30, 6), &state);
        assert_eq!(parts.content.height, 5);
        assert_eq!(parts.trigger.height, 1);
    }

    #[test]
    fn preferred_content_height() {
        let system = DesignSystem::default();
        let c = Collapsible::new("X", &system)
            .open(true)
            .preferred_content_height(2);
        let state = CollapsibleState::new().initially_open(true);
        let parts = c.layout(Rect::new(0, 0, 20, 10), &state);
        assert_eq!(parts.content.height, 2);
    }

    #[test]
    fn nested_indent() {
        let system = DesignSystem::default();
        let outer = Collapsible::new("Outer", &system);
        let inner = Collapsible::new("Inner", &system).depth(1);
        let state = CollapsibleState::new();
        let a = outer.layout(Rect::new(0, 0, 40, 4), &state);
        let b = inner.layout(Rect::new(0, 0, 40, 4), &state);
        assert!(b.trigger.x > a.trigger.x);
    }

    #[test]
    fn mouse_toggle_on_trigger() {
        let system = DesignSystem::default();
        let c = Collapsible::new("Tool", &system);
        let mut state = CollapsibleState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 5));
        let _ = c.paint(Rect::new(0, 0, 24, 5), &mut buf, &mut state);
        let out = state.handle_mouse(click(1, 0), false, None);
        assert_eq!(out, CollapsibleOutcome::Opened);
    }

    #[test]
    fn keep_mounted_policy_flag() {
        let system = DesignSystem::default();
        let c = Collapsible::new("X", &system).keep_mounted();
        assert!(c.policy().keep_state());
        assert_eq!(c.policy().id(), "keep-mounted");
        // geometry still collapses when closed; policy surfaces on parts
        let state = CollapsibleState::new();
        let parts = c.layout(Rect::new(0, 0, 20, 5), &state);
        assert_eq!(parts.content.height, 0);
        assert!(parts.content_policy.keep_state());
    }

    #[test]
    fn unfocused_ignores_keys() {
        let mut state = CollapsibleState::new();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            None,
        );
        assert_eq!(out, CollapsibleOutcome::Ignored);
    }

    #[test]
    fn semantic_registers_expanded() {
        use crate::interaction::SemanticScene;
        let system = DesignSystem::default();
        let c = Collapsible::new("Details", &system);
        let state = CollapsibleState::new().initially_open(true);
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        c.register_semantic(&mut scene, "c", Rect::new(0, 0, 20, 4), &state);
        assert_eq!(scene.len(), 1);
        assert!(scene.nodes()[0].focusable);
        assert!(scene.nodes()[0].state.expanded);
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let c = Collapsible::new("Perf", &system).section().depth(1);
        let state = CollapsibleState::new().initially_open(true);
        let area = Rect::new(0, 0, 40, 12);
        for _ in 0..50_000 {
            let _ = c.layout(area, &state);
        }
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let c = Collapsible::new("X", &system);
        let mut state = CollapsibleState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let body = c.paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(body.width, 0);
    }

    #[test]
    fn variant_ids_stable() {
        assert_eq!(CollapsibleVariant::Section.id(), "section");
    }
}
