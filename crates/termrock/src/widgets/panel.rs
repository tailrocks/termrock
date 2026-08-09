// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Token-driven panel chrome with priority-aware title slots.
//!
//! Border *weight* never encodes focus — only semantic theme roles do.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Span, widgets::Widget};
use ratatui_widgets::block::Block;

use crate::style::{
        DesignSystem,
        PanelChrome,
        PanelRecipe,
    };
use crate::text::display_cols;

// PanelChrome lives in `style` (sole chrome enum). Re-exported from widgets::mod.

/// Priority-ordered title/footer slots for one-line panel chrome.
///
/// Narrow drop order: footer → trailing → subtitle → leading → title (last).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelSlots<'a> {
    /// Primary title (survives longest under contraction).
    pub title: Option<&'a str>,
    /// Secondary title text after the primary.
    pub subtitle: Option<&'a str>,
    /// Leading status glyph/text before the title.
    pub leading: Option<&'a str>,
    /// Trailing badge/action label on the title line.
    pub trailing: Option<&'a str>,
    /// Footer hint/status on the bottom border.
    pub footer: Option<&'a str>,
}

impl<'a> PanelSlots<'a> {
    /// Resolves which slots survive at the available title width.
    #[must_use]
    pub fn for_width(self, width: u16) -> Self {
        let mut slots = self;
        // Drop order: footer → trailing → subtitle → leading → title.
        if width < 24 {
            slots.footer = None;
        }
        if width < 20 {
            slots.trailing = None;
        }
        if width < 14 {
            slots.subtitle = None;
        }
        if width < 10 {
            slots.leading = None;
        }
        slots
    }

    /// Formats the top title span content (without outer spaces).
    #[must_use]
    pub fn title_text(self) -> Option<String> {
        if self.title.is_none()
            && self.leading.is_none()
            && self.subtitle.is_none()
            && self.trailing.is_none()
        {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(leading) = self.leading {
            parts.push(leading.trim().to_string());
        }
        if let Some(title) = self.title {
            parts.push(title.trim().to_string());
        }
        if let Some(subtitle) = self.subtitle {
            parts.push(format!("· {}", subtitle.trim()));
        }
        if let Some(trailing) = self.trailing {
            parts.push(format!("[{}]", trailing.trim()));
        }
        let text = parts.join(" ");
        if text.is_empty() { None } else { Some(text) }
    }
}

#[derive(Debug, Clone)]
/// A bordered container painted through [`DesignSystem`] recipes.
pub struct Panel<'a> {
    slots: PanelSlots<'a>,
    emphasis: PanelChrome,
    style: Option<Style>,
    tokens: &'a DesignSystem,
}

impl<'a> Panel<'a> {
    /// Creates an untitled panel from design tokens (canonical constructor).
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            slots: PanelSlots {
                title: None,
                subtitle: None,
                leading: None,
                trailing: None,
                footer: None,
            },
            emphasis: PanelChrome::Normal,
            style: None,
            tokens,
        }
    }

    /// Alias for [`Self::new`].
    #[must_use]
    pub const fn from_tokens(tokens: &'a DesignSystem) -> Self {
        Self::new(tokens)
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.slots.title = Some(title);
        self
    }

    #[must_use]
    /// Sets the optional subtitle (drops before title under narrow pressure).
    pub const fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.slots.subtitle = Some(subtitle);
        self
    }

    #[must_use]
    /// Sets leading status chrome on the title line.
    pub const fn leading(mut self, leading: &'a str) -> Self {
        self.slots.leading = Some(leading);
        self
    }

    #[must_use]
    /// Sets trailing badge/action text on the title line.
    pub const fn trailing(mut self, trailing: &'a str) -> Self {
        self.slots.trailing = Some(trailing);
        self
    }

    #[must_use]
    /// Sets footer hint on the bottom border (drops first under narrow pressure).
    pub const fn footer(mut self, footer: &'a str) -> Self {
        self.slots.footer = Some(footer);
        self
    }

    #[must_use]
    /// Replaces all panel slots at once.
    pub const fn slots(mut self, slots: PanelSlots<'a>) -> Self {
        self.slots = slots;
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis.
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Canonical chrome setter (alias of [`Self::emphasis`]).
    #[must_use]
    pub const fn chrome(mut self, chrome: PanelChrome) -> Self {
        self.emphasis = chrome;
        self
    }

    #[must_use]
    /// Overrides the recipe border style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Resolves the panel recipe for current emphasis.
    #[must_use]
    pub fn recipe(&self) -> PanelRecipe {
        self.tokens.panel_recipe(self.emphasis)
    }

    /// Palette borrow from the design system.
    #[must_use]
    pub const fn palette(&self) -> &crate::style::RolePalette {
        self.tokens.palette()
    }

    /// Slot projection after contraction for a given outer width.
    #[must_use]
    pub fn slots_for_width(&self, width: u16) -> PanelSlots<'a> {
        // Border corners consume 2 cells; title padding uses ~2 more.
        self.slots.for_width(width.saturating_sub(4))
    }

    #[must_use]
    /// Builds the surrounding block from the recipe (single-line border only).
    pub fn block(&self) -> Block<'a> {
        // Unknown width at block-build time: keep all slots; render path may
        // re-resolve. Consumers painting with known width should use
        // [`Self::block_for_width`].
        self.block_for_width(u16::MAX)
    }

    /// Builds chrome contracted to the available outer width.
    #[must_use]
    pub fn block_for_width(&self, width: u16) -> Block<'a> {
        let recipe = self.recipe();
        let border = self.style.unwrap_or(recipe.border);
        let mut block = Block::bordered().border_style(border);
        let slots = self.slots_for_width(width);
        if let Some(title) = slots.title_text() {
            // Clamp title display width roughly for very narrow frames.
            let budget = width.saturating_sub(4).max(1);
            let clipped = if display_cols(&title) > usize::from(budget) {
                // Keep start of primary title text.
                title
                    .chars()
                    .take(usize::from(budget.saturating_sub(1)))
                    .collect::<String>()
                    + "…"
            } else {
                title
            };
            block = block.title(Span::styled(format!(" {clipped} "), recipe.title));
        }
        if let Some(footer) = slots.footer {
            block = block.title_bottom(Span::styled(format!(" {} ", footer.trim()), recipe.title));
        }
        block
    }

    #[must_use]
    /// Returns the content rectangle inside panel chrome.
    pub fn inner(&self, area: Rect) -> Rect {
        self.block_for_width(area.width).inner(area)
    }
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.block_for_width(area.width).render(area, buffer);
    }
}

impl Widget for Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_recipe_focus_uses_border_focused_not_weight() {
        let tokens = DesignSystem::default();
        let normal = tokens.panel_recipe(PanelChrome::Normal);
        let focused = tokens.panel_recipe(PanelChrome::Focused);
        assert_ne!(normal.border, focused.border);
        let panel = Panel::new(&tokens)
            .emphasis(PanelChrome::Focused)
            .title("T");
        assert_eq!(panel.recipe().border, focused.border);
    }

    #[test]
    fn panel_slots_drop_trailing_before_title() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("Main")
            .subtitle("sub")
            .leading("*")
            .trailing("act")
            .footer("hint");
        let wide = panel.slots_for_width(80);
        assert!(wide.footer.is_some());
        assert!(wide.trailing.is_some());
        let mid = panel.slots_for_width(18);
        assert!(mid.trailing.is_none());
        assert_eq!(mid.title, Some("Main"));
        let tiny = panel.slots_for_width(8);
        assert!(tiny.leading.is_none());
        assert_eq!(tiny.title, Some("Main"));
    }
}
