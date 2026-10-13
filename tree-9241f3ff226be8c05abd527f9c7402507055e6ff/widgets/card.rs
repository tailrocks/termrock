// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Card — raised content container composed from [`Panel`] + [`Surface`].
//!
//! shadcn-style anatomy without nested box soup:
//! `root` · `header` · `title` · `description` · `body` · `footer`.
//! Tool/dashboard cards build on this primitive; domain policy stays outside.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::style::{DesignSystem, PanelChrome, Role};
use crate::text::take_display_cols;
use crate::widgets::panel::{Panel, PanelBody, PanelParts, PanelState, PanelVariant};

/// Named card geometry (one Surface + one header band; no nested cards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardParts {
    /// Outer rect.
    pub root: Rect,
    /// Title row inside chrome.
    pub header: Option<Rect>,
    /// Optional description row under title.
    pub description: Option<Rect>,
    /// Primary body (children / tool output).
    pub body: Rect,
    /// Footer band.
    pub footer: Option<Rect>,
    /// Hit region.
    pub hit: Rect,
    /// Clip = body.
    pub clip: Rect,
}

impl From<PanelParts> for CardParts {
    fn from(p: PanelParts) -> Self {
        Self {
            root: p.root,
            header: p.header,
            description: None,
            body: p.body,
            footer: p.footer,
            hit: p.hit,
            clip: p.clip,
        }
    }
}

/// Raised card container (Panel + Elevated surface recipe).
#[derive(Debug, Clone)]
pub struct Card<'a> {
    system: &'a DesignSystem,
    title: Option<&'a str>,
    subtitle: Option<&'a str>,
    leading: Option<&'a str>,
    badge: Option<&'a str>,
    trailing: Option<&'a str>,
    description: Option<&'a str>,
    footer: Option<&'a str>,
    emphasis: PanelChrome,
    variant: PanelVariant,
    body: PanelBody,
    body_title: Option<&'a str>,
    body_detail: Option<&'a str>,
    collapsible: bool,
    interactive: bool,
    header_actions: &'a [crate::widgets::panel::PanelAction<'a>],
}

impl<'a> Card<'a> {
    /// Empty raised card.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: None,
            subtitle: None,
            leading: None,
            badge: None,
            trailing: None,
            description: None,
            footer: None,
            emphasis: PanelChrome::Normal,
            variant: PanelVariant::Bordered,
            body: PanelBody::Host,
            body_title: None,
            body_detail: None,
            collapsible: false,
            interactive: false,
            header_actions: &[],
        }
    }

    /// Card title on the header chrome.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Secondary title text (contracts under narrow width).
    #[must_use]
    pub const fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    /// Leading status glyph or marker.
    #[must_use]
    pub const fn leading(mut self, leading: &'a str) -> Self {
        self.leading = Some(leading);
        self
    }

    /// Trailing metadata label.
    #[must_use]
    pub const fn trailing(mut self, trailing: &'a str) -> Self {
        self.trailing = Some(trailing);
        self
    }

    /// Status badge.
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Header actions.
    #[must_use]
    pub const fn header_actions(
        mut self,
        actions: &'a [crate::widgets::panel::PanelAction<'a>],
    ) -> Self {
        self.header_actions = actions;
        self
    }

    /// Description line under the title (shadcn CardDescription).
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Footer hint text.
    #[must_use]
    pub const fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Focus / danger emphasis.
    #[must_use]
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Alias for [`Self::emphasis`].
    #[must_use]
    pub const fn chrome(mut self, chrome: PanelChrome) -> Self {
        self.emphasis = chrome;
        self
    }

    /// Panel border / selection variant.
    #[must_use]
    pub const fn variant(mut self, variant: PanelVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Built-in body mode (host / loading / empty / error).
    #[must_use]
    pub const fn body(mut self, body: PanelBody) -> Self {
        self.body = body;
        self
    }

    /// Empty/error body title.
    #[must_use]
    pub const fn body_title(mut self, title: &'a str) -> Self {
        self.body_title = Some(title);
        self
    }

    /// Empty/error/loading body detail.
    #[must_use]
    pub const fn body_detail(mut self, detail: &'a str) -> Self {
        self.body_detail = Some(detail);
        self
    }

    /// Collapsible card header.
    #[must_use]
    pub const fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Whole card is actionable (focus + activate).
    #[must_use]
    pub const fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        if interactive {
            self.variant = PanelVariant::Interactive;
        }
        self
    }

    /// Whether the card claims card-level keyboard focus.
    #[must_use]
    pub const fn is_focusable(&self) -> bool {
        self.interactive || self.collapsible
    }

    fn panel(&self) -> Panel<'a> {
        let mut p = Panel::new(self.system)
            .emphasis(self.emphasis)
            .variant(if self.interactive {
                PanelVariant::Interactive
            } else {
                self.variant
            })
            .body(self.body)
            .collapsible(self.collapsible)
            .raised(true)
            .header_actions(self.header_actions);
        if let Some(t) = self.title {
            p = p.title(t);
        }
        if let Some(s) = self.subtitle {
            p = p.subtitle(s);
        }
        if let Some(l) = self.leading {
            p = p.leading(l);
        }
        if let Some(b) = self.badge {
            p = p.badge(b);
        }
        if let Some(tr) = self.trailing {
            p = p.trailing(tr);
        }
        if let Some(f) = self.footer {
            p = p.footer(f);
        }
        if let Some(bt) = self.body_title {
            p = p.body_title(bt);
        }
        if let Some(bd) = self.body_detail {
            p = p.body_detail(bd);
        }
        p
    }

    /// Layout with optional description row carved from body via [`Stack`].
    #[must_use]
    pub fn layout(&self, area: Rect, state: Option<&PanelState>) -> CardParts {
        use crate::layout::{FlexSize, Stack};

        let panel = self.panel();
        let parts = panel.layout(area, state);
        let mut card = CardParts::from(parts);
        if let Some(desc) = self.description
            && !desc.is_empty()
            && card.body.height > 1
            && !state.is_some_and(|s| s.collapsed)
        {
            let stacked = Stack::new()
                .gap(0)
                .layout(card.body, &[FlexSize::Fixed(1), FlexSize::Weight(1)]);
            card.description = stacked.get(0);
            card.body = stacked.get(1).unwrap_or(card.body);
            card.clip = card.body;
        }
        card
    }

    /// Content body after chrome + description.
    #[must_use]
    pub fn inner(&self, area: Rect) -> Rect {
        self.layout(area, None).body
    }

    /// Paint raised card chrome + description; returns body rect.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        mut state: Option<&mut PanelState>,
    ) -> Rect {
        if area.is_empty() {
            return area;
        }
        let panel = self.panel();
        let body_after_panel = panel.paint(area, buffer, state.as_deref_mut());

        // Description sits at top of panel body when present.
        let card = self.layout(area, state.as_deref());
        if let Some(desc_rect) = card.description
            && let Some(desc) = self.description
        {
            let t = take_display_cols(desc, usize::from(desc_rect.width));
            buffer.set_stringn(
                desc_rect.x,
                desc_rect.y,
                &t,
                usize::from(desc_rect.width),
                self.system.style(Role::TextMuted),
            );
            return card.body;
        }
        body_after_panel
    }
}

impl Widget for &Card<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer, None);
    }
}

impl Widget for Card<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;
    use crate::widgets::panel::PanelAction;

    #[test]
    fn card_description_carves_body() {
        let system = DesignSystem::default();
        let card = Card::new(&system)
            .title("Metric")
            .description("Requests / min");
        let parts = card.layout(Rect::new(0, 0, 32, 10), None);
        assert!(parts.description.is_some());
        assert!(parts.body.height < 8);
    }

    #[test]
    fn card_inner_for_tool_style() {
        let system = DesignSystem::default();
        let card = Card::new(&system)
            .title("shell")
            .leading("◉")
            .emphasis(PanelChrome::Focused);
        let body = card.inner(Rect::new(0, 0, 40, 6));
        assert!(body.width > 0);
        assert!(body.height > 0);
    }

    #[test]
    fn paint_card_no_panic() {
        let system = DesignSystem::default();
        let card = Card::new(&system)
            .title("Card")
            .description("shadcn-like")
            .footer("esc");
        let mut buf = Buffer::empty(Rect::new(0, 0, 28, 8));
        let body = card.paint(Rect::new(0, 0, 28, 8), &mut buf, None);
        assert!(body.width > 0);
    }

    #[test]
    fn card_badge_and_actions_forward() {
        let system = DesignSystem::default();
        let actions = [PanelAction::new("open", "Open")];
        let card = Card::new(&system)
            .title("Metric")
            .badge("live")
            .header_actions(&actions)
            .description("p99");
        let mut state = PanelState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        let body = card.paint(Rect::new(0, 0, 40, 8), &mut buf, Some(&mut state));
        assert!(body.width > 0);
        assert!(!state.action_hits.is_empty());
        assert_eq!(state.action_hits[0].0, "open");
    }

    #[test]
    fn card_loading_body() {
        let system = DesignSystem::default();
        let card = Card::new(&system)
            .title("Jobs")
            .body(PanelBody::Loading)
            .body_detail("Fetching");
        let mut buf = Buffer::empty(Rect::new(0, 0, 28, 6));
        let body = card.paint(Rect::new(0, 0, 28, 6), &mut buf, None);
        assert!(body.height > 0);
    }

    #[test]
    fn card_interactive_is_focusable_panel() {
        let system = DesignSystem::default();
        let card = Card::new(&system).title("Pick").interactive(true);
        assert!(card.is_focusable());
        assert!(!Card::new(&system).title("Static").is_focusable());
    }
}
