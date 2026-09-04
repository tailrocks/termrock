// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Section — editorial grouping without a full border box.
//!
//! **Anatomy:** `root` · `header` · `title` · optional `status` · optional
//! `actions` · optional `description` · optional `divider` · `body`.
//!
//! Quiet hierarchy for forms, settings, documentation, inspectors, and
//! dashboards. Focus belongs to the header when collapsible; otherwise to
//! interactive descendants in `body`. Nested sections use [`Section::indent`]
//! / [`Section::depth`].
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{UiIntent, default_button_intent, default_list_intent};
use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::panel::PanelAction;

/// Visual weight of section chrome (not focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SectionVariant {
    /// Muted title, no divider (default editorial grouping).
    #[default]
    Quiet,
    /// Strong title + divider rule under header.
    Emphasized,
    /// Always paint a divider under the header band (even when quiet title).
    Divided,
}

/// Header action (stable id + label). Same shape as panel header actions.
pub type SectionAction<'a> = PanelAction<'a>;

/// Named geometry for one painted section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SectionParts {
    /// Outer allocation.
    pub root: Rect,
    /// Header band (title + status + actions).
    pub header: Option<Rect>,
    /// Title hit region.
    pub title: Option<Rect>,
    /// Description band under header (when expanded).
    pub description: Option<Rect>,
    /// Actions band (right of header).
    pub actions: Option<Rect>,
    /// Divider row under header/description.
    pub divider: Option<Rect>,
    /// Body for host children / nested sections.
    pub body: Rect,
    /// Clip contract (= body).
    pub clip: Rect,
    /// Hit region for collapse / section-level interaction.
    pub hit: Rect,
}

/// Typed outcomes (no side effects).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SectionOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Collapse toggled.
    ToggleCollapsed {
        /// New collapsed flag.
        collapsed: bool,
    },
    /// Header action activated.
    HeaderAction {
        /// Action id.
        id: String,
    },
}

/// Interaction state for collapsible sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionState {
    /// Collapsed body when collapsible.
    pub collapsed: bool,
    /// Header focus (collapsible only).
    pub focused: bool,
    /// Cached parts from last paint.
    pub parts: Option<SectionParts>,
    /// Header action hits (id, rect).
    pub action_hits: Vec<(String, Rect)>,
}

impl SectionState {
    /// Expanded section.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            collapsed: false,
            focused: false,
            parts: None,
            action_hits: Vec::new(),
        }
    }

    /// Whether collapsed.
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Controlled collapse.
    pub const fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Header focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Key path via intents.
    pub fn handle_key(&mut self, key: KeyEvent, collapsible: bool) -> SectionOutcome {
        if !self.focused || !collapsible || !key.is_press() {
            return SectionOutcome::Ignored;
        }
        let Some(intent) = default_button_intent(key).or_else(|| default_list_intent(key)) else {
            return SectionOutcome::Ignored;
        };
        self.handle_intent(intent, collapsible)
    }

    /// Semantic intent path.
    pub fn handle_intent(&mut self, intent: UiIntent, collapsible: bool) -> SectionOutcome {
        if !self.focused || !collapsible {
            return SectionOutcome::Ignored;
        }
        match intent {
            UiIntent::Toggle | UiIntent::Activate => {
                self.collapsed = !self.collapsed;
                SectionOutcome::ToggleCollapsed {
                    collapsed: self.collapsed,
                }
            }
            UiIntent::Expand => {
                self.collapsed = false;
                SectionOutcome::ToggleCollapsed { collapsed: false }
            }
            UiIntent::Collapse => {
                self.collapsed = true;
                SectionOutcome::ToggleCollapsed { collapsed: true }
            }
            _ => SectionOutcome::Ignored,
        }
    }

    /// Mouse: actions first, then header toggle when collapsible.
    pub fn handle_mouse(&mut self, event: MouseEvent, collapsible: bool) -> SectionOutcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return SectionOutcome::Ignored;
        }
        for (id, rect) in &self.action_hits {
            if rect.contains(event.position) {
                return SectionOutcome::HeaderAction { id: id.clone() };
            }
        }
        if collapsible
            && let Some(parts) = self.parts
            && parts.header.is_some_and(|h| h.contains(event.position))
            && parts.actions.is_none_or(|a| !a.contains(event.position))
        {
            self.collapsed = !self.collapsed;
            return SectionOutcome::ToggleCollapsed {
                collapsed: self.collapsed,
            };
        }
        SectionOutcome::Ignored
    }
}

/// Editorial section chrome.
#[derive(Debug, Clone)]
pub struct Section<'a> {
    title: &'a str,
    description: Option<&'a str>,
    status: Option<&'a str>,
    actions: &'a [SectionAction<'a>],
    variant: SectionVariant,
    collapsible: bool,
    /// Nested depth (0 = root). Affects indent + title role.
    depth: u8,
    /// Extra left indent cells (added to depth*2).
    indent: u16,
    show_divider: Option<bool>,
    system: &'a DesignSystem,
}

impl<'a> Section<'a> {
    /// Quiet section with title.
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            status: None,
            actions: &[],
            variant: SectionVariant::Quiet,
            collapsible: false,
            depth: 0,
            indent: 0,
            show_divider: None,
            system,
        }
    }

    /// Description under title (muted; drops on narrow).
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Status badge text (e.g. "3", "beta").
    #[must_use]
    pub const fn status(mut self, status: &'a str) -> Self {
        self.status = Some(status);
        self
    }

    /// Header actions (right band); contracted when width &lt; 28.
    #[must_use]
    pub const fn actions(mut self, actions: &'a [SectionAction<'a>]) -> Self {
        self.actions = actions;
        self
    }

    /// Visual variant.
    #[must_use]
    pub const fn variant(mut self, variant: SectionVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Emphasized recipe.
    #[must_use]
    pub const fn emphasized(mut self) -> Self {
        self.variant = SectionVariant::Emphasized;
        self
    }

    /// Collapsible header (disclosure + focus/activate).
    #[must_use]
    pub const fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Nesting depth (0 = root). Indent = depth×2 + indent cells.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Whether header claims focus.
    #[must_use]
    pub const fn is_focusable(&self) -> bool {
        self.collapsible
    }

    /// Actions visible at outer width.
    #[must_use]
    pub const fn actions_visible(width: u16) -> bool {
        width >= 28
    }

    /// Description survives at outer width.
    #[must_use]
    pub const fn description_visible(width: u16) -> bool {
        width >= 16
    }

    /// Status survives at outer width.
    #[must_use]
    pub const fn status_visible(width: u16) -> bool {
        width >= 20
    }

    fn wants_divider(&self) -> bool {
        if let Some(show) = self.show_divider {
            return show;
        }
        matches!(
            self.variant,
            SectionVariant::Emphasized | SectionVariant::Divided
        )
    }

    fn left_pad(&self) -> u16 {
        self.indent
            .saturating_add(u16::from(self.depth).saturating_mul(2))
    }

    /// Layout named parts without painting.
    #[must_use]
    pub fn layout(&self, area: Rect, state: Option<&SectionState>) -> SectionParts {
        if area.is_empty() {
            return SectionParts {
                root: area,
                body: area,
                clip: area,
                hit: area,
                ..Default::default()
            };
        }
        let pad = self.left_pad();
        let content = shrink_left(area, pad);
        let collapsed = state.is_some_and(|s| s.collapsed && self.collapsible);
        let show_actions = Self::actions_visible(area.width) && !self.actions.is_empty();
        let show_desc = !collapsed
            && self.description.is_some()
            && Self::description_visible(area.width)
            && content.height > 1;
        let show_divider = !collapsed && self.wants_divider() && content.height > 1;

        let mut y = content.y;
        let header_h: u16 = 1.min(content.height);
        let header = if header_h > 0 {
            Some(Rect {
                x: content.x,
                y,
                width: content.width,
                height: header_h,
            })
        } else {
            None
        };
        y = y.saturating_add(header_h);

        let actions = if show_actions {
            if let Some(h) = header {
                let band_w = self
                    .actions
                    .iter()
                    .map(|a| display_cols(a.label) as u16 + 3)
                    .sum::<u16>()
                    .min(h.width / 2)
                    .max(4)
                    .min(h.width);
                Some(Rect {
                    x: h.x.saturating_add(h.width.saturating_sub(band_w)),
                    y: h.y,
                    width: band_w,
                    height: 1,
                })
            } else {
                None
            }
        } else {
            None
        };

        let title = header.map(|h| {
            let tw = if let Some(a) = actions {
                a.x.saturating_sub(h.x).saturating_sub(1)
            } else {
                h.width
            };
            Rect {
                x: h.x,
                y: h.y,
                width: tw,
                height: 1,
            }
        });

        let description = if show_desc && y < content.bottom() {
            let r = Rect {
                x: content.x,
                y,
                width: content.width,
                height: 1,
            };
            y = y.saturating_add(1);
            Some(r)
        } else {
            None
        };

        let divider = if show_divider && y < content.bottom() {
            let r = Rect {
                x: content.x,
                y,
                width: content.width,
                height: 1,
            };
            y = y.saturating_add(1);
            Some(r)
        } else {
            None
        };

        let body = if collapsed {
            Rect {
                x: content.x,
                y,
                width: content.width,
                height: 0,
            }
        } else {
            Rect {
                x: content.x,
                y,
                width: content.width,
                height: content.bottom().saturating_sub(y),
            }
        };

        let hit = if self.collapsible {
            header.unwrap_or(area)
        } else {
            body
        };

        SectionParts {
            root: area,
            header,
            title,
            description,
            actions,
            divider,
            body,
            clip: body,
            hit,
        }
    }

    /// Paint chrome; returns body rect for children.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        mut state: Option<&mut SectionState>,
    ) -> Rect {
        if area.is_empty() {
            return area;
        }
        let collapsed = state
            .as_ref()
            .is_some_and(|s| s.collapsed && self.collapsible);
        let focused = state.as_ref().is_some_and(|s| s.focused);
        let parts = self.layout(area, state.as_ref().map(|s| &**s));

        let title_style = match (focused && self.collapsible, self.variant, self.depth) {
            (true, _, _) => self.system.style(Role::TextStrong),
            (_, SectionVariant::Emphasized, 0) => self
                .system
                .style(Role::TextStrong)
                .add_modifier(ratatui_core::style::Modifier::BOLD),
            (_, _, d) if d > 0 => self.system.style(Role::TextMuted),
            _ => self.system.style(Role::Text),
        };

        if let Some(title_r) = parts.title {
            let mut label = String::new();
            if self.collapsible {
                let g = if collapsed {
                    self.system.glyphs.disclosure_closed()
                } else {
                    self.system.glyphs.disclosure_open()
                };
                label.push_str(g);
                label.push(' ');
            }
            label.push_str(self.title.trim());
            if Self::status_visible(area.width)
                && let Some(st) = self.status
            {
                label.push_str(" [");
                label.push_str(st.trim());
                label.push(']');
            }
            let t = take_display_cols(&label, usize::from(title_r.width));
            buffer.set_stringn(
                title_r.x,
                title_r.y,
                &t,
                usize::from(title_r.width),
                title_style,
            );
        }

        // Header actions
        let mut action_hits = Vec::new();
        if let Some(band) = parts.actions {
            let style = self.system.style(if focused {
                Role::ActionFocused
            } else {
                Role::TextMuted
            });
            let mut x = band.x;
            for action in self.actions {
                let label = format!("[{}]", action.label.trim());
                let w = (display_cols(&label) as u16).min(band.right().saturating_sub(x));
                if w == 0 {
                    break;
                }
                buffer.set_stringn(x, band.y, &label, usize::from(w), style);
                action_hits.push((
                    action.id.to_string(),
                    Rect {
                        x,
                        y: band.y,
                        width: w,
                        height: 1,
                    },
                ));
                x = x.saturating_add(w).saturating_add(1);
                if x >= band.right() {
                    break;
                }
            }
        }

        if let Some(desc_r) = parts.description
            && let Some(desc) = self.description
        {
            let t = take_display_cols(desc, usize::from(desc_r.width));
            buffer.set_stringn(
                desc_r.x,
                desc_r.y,
                &t,
                usize::from(desc_r.width),
                self.system.style(Role::TextMuted),
            );
        }

        if let Some(div) = parts.divider {
            let rule = self.system.glyphs.rule();
            let line: String = std::iter::repeat_n(rule, usize::from(div.width)).collect();
            buffer.set_stringn(
                div.x,
                div.y,
                &line,
                usize::from(div.width),
                self.system.style(Role::Border),
            );
        }

        if let Some(state) = state.as_deref_mut() {
            state.parts = Some(parts);
            state.action_hits = action_hits;
        }
        parts.body
    }

    /// Registers section header into a semantic scene (collapsible = focusable).
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: Option<&SectionState>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        use crate::interaction::{SemanticNode, SemanticRole};
        let parts = self.layout(area, state);
        let header = parts.header.unwrap_or(area);
        let focusable = self.is_focusable();
        let _ = scene.register(
            SemanticNode::control(id, header)
                .role(SemanticRole::Heading)
                .label(self.title)
                .focusable(focusable)
                .state(crate::interaction::SemanticState {
                    expanded: !state.is_some_and(|s| s.collapsed),
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &Section<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer, None);
    }
}

impl Widget for Section<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn shrink_left(area: Rect, left: u16) -> Rect {
    let x = area.x.saturating_add(left);
    let width = area.width.saturating_sub(left);
    if width == 0 {
        Rect {
            x,
            y: area.y,
            width: 0,
            height: area.height,
        }
    } else {
        Rect {
            x,
            y: area.y,
            width,
            height: area.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::DesignSystem;
    use crate::widgets::tests::click;

    #[test]
    fn quiet_layout_has_header_and_body() {
        let system = DesignSystem::default();
        let section = Section::new("General", &system).description("prefs");
        let parts = section.layout(Rect::new(0, 0, 40, 8), None);
        assert!(parts.header.is_some());
        assert!(parts.description.is_some());
        assert!(parts.body.height > 0);
        assert!(parts.divider.is_none());
    }

    #[test]
    fn emphasized_has_divider() {
        let system = DesignSystem::default();
        let section = Section::new("Safety", &system).emphasized();
        let parts = section.layout(Rect::new(0, 0, 40, 8), None);
        assert!(parts.divider.is_some());
    }

    #[test]
    fn collapsible_toggle() {
        let mut state = SectionState::new();
        state.set_focused(true);
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), true);
        assert!(matches!(
            out,
            SectionOutcome::ToggleCollapsed { collapsed: true }
        ));
        assert!(state.is_collapsed());
    }

    #[test]
    fn non_collapsible_ignores_keys() {
        let mut state = SectionState::new();
        state.set_focused(true);
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), false);
        assert_eq!(out, SectionOutcome::Ignored);
    }

    #[test]
    fn actions_contract_narrow() {
        assert!(!Section::actions_visible(20));
        assert!(Section::actions_visible(28));
        assert!(!Section::description_visible(10));
    }

    #[test]
    fn collapsed_zero_body() {
        let system = DesignSystem::default();
        let section = Section::new("Fold", &system).collapsible(true);
        let mut state = SectionState::new();
        state.set_collapsed(true);
        let parts = section.layout(Rect::new(0, 0, 30, 10), Some(&state));
        assert_eq!(parts.body.height, 0);
    }

    #[test]
    fn nested_indent() {
        let system = DesignSystem::default();
        let root = Section::new("Root", &system);
        let nested = Section::new("Child", &system).depth(1);
        let a = root.layout(Rect::new(0, 0, 40, 6), None);
        let b = nested.layout(Rect::new(0, 0, 40, 6), None);
        assert!(b.header.unwrap().x > a.header.unwrap().x);
    }

    #[test]
    fn header_action_mouse() {
        let system = DesignSystem::default();
        let actions = [SectionAction::new("reset", "Reset")];
        let section = Section::new("Network", &system).actions(&actions);
        let mut state = SectionState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 48, 6));
        let _ = section.paint(Rect::new(0, 0, 48, 6), &mut buf, Some(&mut state));
        assert!(!state.action_hits.is_empty());
        let (id, rect) = &state.action_hits[0];
        assert_eq!(id, "reset");
        let out = state.handle_mouse(click(rect.x, rect.y), false);
        assert!(matches!(out, SectionOutcome::HeaderAction { id } if id == "reset"));
    }

    #[test]
    fn paint_divided_body() {
        let system = DesignSystem::default();
        let section = Section::new("Docs", &system)
            .description("help")
            .variant(SectionVariant::Divided);
        let mut buf = Buffer::empty(Rect::new(0, 0, 32, 8));
        let mut state = SectionState::new();
        let body = section.paint(Rect::new(0, 0, 32, 8), &mut buf, Some(&mut state));
        assert!(body.height > 0);
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let section = Section::new("Perf", &system)
            .description("d")
            .status("n")
            .emphasized()
            .depth(1);
        let area = Rect::new(0, 0, 60, 12);
        for _ in 0..20_000 {
            let _ = section.layout(area, None);
        }
    }

    #[test]
    fn semantic_heading_role() {
        use crate::interaction::SemanticScene;
        let system = DesignSystem::default();
        let section = Section::new("Meta", &system).collapsible(true);
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        section.register_semantic(&mut scene, "s", Rect::new(0, 0, 20, 4), None);
        assert_eq!(scene.len(), 1);
        assert!(scene.nodes()[0].focusable);
    }

    #[test]
    fn focusable_only_when_collapsible() {
        let system = DesignSystem::default();
        assert!(!Section::new("x", &system).is_focusable());
        assert!(Section::new("x", &system).collapsible(true).is_focusable());
    }
}
