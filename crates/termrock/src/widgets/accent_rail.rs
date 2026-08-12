// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Semantic one-column accent rail for composed blocks.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::style::{DesignSystem, Glyph, Role, blend_toward, effective_alpha, wave_brightness};

/// Left-edge semantic chrome with deterministic optional motion.
#[derive(Debug, Clone, Copy)]
pub struct AccentRail<'a> {
    system: &'a DesignSystem,
    role: Role,
    glyph: Glyph,
    active: bool,
    tick: u64,
    speed: f32,
}

impl<'a> AccentRail<'a> {
    /// Creates a static heavy rail using a semantic color role.
    #[must_use]
    pub const fn new(system: &'a DesignSystem, role: Role) -> Self {
        Self {
            system,
            role,
            glyph: Glyph::RailHeavy,
            active: false,
            tick: 0,
            speed: 1.0,
        }
    }

    /// Selects a catalog glyph for the rail.
    #[must_use]
    pub const fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = glyph;
        self
    }

    /// Selects the compact collapsed-block rail glyph.
    #[must_use]
    pub const fn collapsed(mut self, collapsed: bool) -> Self {
        if collapsed {
            self.glyph = Glyph::RailCollapsed;
        }
        self
    }

    /// Enables the deterministic brightness wave.
    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Supplies the host-owned frame tick.
    #[must_use]
    pub const fn tick(mut self, tick: u64) -> Self {
        self.tick = tick;
        self
    }

    /// Sets wave speed in rows per tick.
    #[must_use]
    pub const fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
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
        let resolved = self.glyph.resolve(self.system.glyphs);
        let base = self.system.style(self.role).fg.unwrap_or(Color::Reset);
        let canvas = self.system.style(Role::Canvas).bg.unwrap_or(Color::Reset);
        for row in 0..rail.height {
            let brightness = if self.active {
                effective_alpha(
                    self.system.motion,
                    wave_brightness(self.tick, row, 32, self.speed),
                )
            } else {
                1.0
            };
            let color = blend_toward(base, canvas, 1.0 - brightness);
            buffer.set_stringn(
                rail.x,
                rail.y.saturating_add(row),
                resolved.text,
                1,
                Style::new().fg(color),
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
    use crate::style::Motion;

    #[test]
    fn layout_is_safe_at_tiny_widths() {
        let system = DesignSystem::phosphor();
        for width in 1..=3 {
            let area = Rect::new(2, 3, width, 4);
            let (rail, content) = AccentRail::new(&system, Role::Accent).layout(area);
            assert_eq!(rail.width, 1);
            assert!(content.right() <= area.right());
        }
    }

    #[test]
    fn reduced_motion_frames_are_identical() {
        let mut system = DesignSystem::phosphor();
        system.motion = Motion::Reduced;
        let area = Rect::new(0, 0, 4, 12);
        let render = |tick| {
            let mut buffer = Buffer::empty(area);
            let _ = AccentRail::new(&system, Role::Accent)
                .active(true)
                .tick(tick)
                .paint(area, &mut buffer);
            buffer
        };
        assert_eq!(render(0), render(19));
    }

    #[test]
    fn collapsed_variant_uses_catalog_fallback() {
        let mut system = DesignSystem::phosphor();
        system.glyphs = crate::style::GlyphSet::Ascii;
        let area = Rect::new(0, 0, 2, 2);
        let mut buffer = Buffer::empty(area);
        let _ = AccentRail::new(&system, Role::Accent)
            .collapsed(true)
            .paint(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "|");
    }
}
