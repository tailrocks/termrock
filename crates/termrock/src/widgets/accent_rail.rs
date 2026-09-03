// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Semantic one-column accent rail for composed blocks.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::style::{DesignSystem, Glyph, Role};

/// Left-edge semantic chrome.
#[derive(Debug, Clone, Copy)]
pub struct AccentRail<'a> {
    system: &'a DesignSystem,
    role: Role,
    glyph: Glyph,
}

impl<'a> AccentRail<'a> {
    /// Creates a static heavy rail using a semantic color role.
    #[must_use]
    pub const fn new(system: &'a DesignSystem, role: Role) -> Self {
        Self {
            system,
            role,
            glyph: Glyph::RailHeavy,
        }
    }

    /// Selects the compact collapsed-block rail glyph.
    #[must_use]
    pub const fn collapsed(mut self, collapsed: bool) -> Self {
        if collapsed {
            self.glyph = Glyph::SelectionGutter;
        }
        self
    }

    /// Splits the outer area into rail and legal content regions.
    #[must_use]
    pub fn layout(&self, area: Rect) -> (Rect, Rect) {
        if area.is_empty() {
            return (Rect::new(area.x, area.y, 0, area.height), area);
        }
        let rail = Rect::new(area.x, area.y, 1, area.height);
        let consumed = 1_u16
            .saturating_add(self.system.spacing.gap)
            .min(area.width);
        let content = Rect::new(
            area.x.saturating_add(consumed),
            area.y,
            area.width.saturating_sub(consumed),
            area.height,
        );
        (rail, content)
    }

    /// Paints the rail and returns the legal content region.
    #[must_use]
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> Rect {
        let (rail, content) = self.layout(area);
        let resolved = self.glyph.resolve();
        let base = self.system.style(self.role).fg.unwrap_or(Color::Reset);
        let _canvas = self.system.style(Role::Canvas).bg.unwrap_or(Color::Reset);
        for row in 0..rail.height {
            // The rail speaks in its role's own colour; presence is carried by
            // words and glyphs, never by an ambient wave.
            buffer.set_stringn(
                rail.x,
                rail.y.saturating_add(row),
                resolved.text,
                1,
                Style::new().fg(base),
            );
        }
        content
    }
}

impl Widget for AccentRail<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::{buffer::Buffer, layout::Rect};

    use super::*;

    #[test]
    fn layout_is_safe_at_tiny_widths() {
        let system = DesignSystem::junie();
        for width in 1..=3 {
            let area = Rect::new(2, 3, width, 4);
            let (rail, content) = AccentRail::new(&system, Role::Accent).layout(area);
            assert_eq!(rail.width, 1);
            assert!(content.right() <= area.right());
        }
    }

    #[test]
    fn the_rail_speaks_in_its_role_colour_not_a_quantized_copy() {
        // junie law: tokens are born at the terminal's rung; the rail never
        // re-projects them. The resting frame is the role colour verbatim.
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 1, 3);
        let mut buffer = Buffer::empty(area);
        let _ = AccentRail::new(&system, Role::Accent).paint(area, &mut buffer);
        let accent = system.style(Role::Accent).fg.unwrap();
        for y in area.top()..area.bottom() {
            assert_eq!(buffer[(0, y)].fg, accent);
        }
    }
}
