// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Icon — paint a semantic [`Glyph`] with width alignment and optional text label.
//!
//! **Critical meaning.** Glyph cells alone must not carry exclusive meaning for
//! host decisions. Use [`Icon::label`] (painted) and/or [`Glyph::meaning`]
//! (always available) so Studio, help, and no-color profiles stay legible.
//!
//! Resolves through [`DesignSystem::glyphs`] (`Unicode` / `Ascii` / `Enhanced`).
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::style::{DesignSystem, Glyph, GlyphResolved, Role};
use crate::widgets::text::{Text, TextSpan};

/// Painted icon geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IconParts {
    /// Full allocation used.
    pub root: Rect,
    /// Glyph cells.
    pub glyph: Rect,
    /// Optional label band after the glyph.
    pub label: Rect,
}

/// Semantic icon primitive.
#[derive(Debug, Clone, Copy)]
pub struct Icon<'a> {
    glyph: Glyph,
    system: &'a DesignSystem,
    role: Role,
    /// Visible text label (prevents glyph-only critical meaning when set).
    label: Option<&'a str>,
    /// Minimum glyph column budget (alignment in tables / toolbars).
    min_width: u16,
    /// Gap between glyph and label.
    gap: u16,
}

impl<'a> Icon<'a> {
    /// Icon for a semantic glyph.
    #[must_use]
    pub const fn new(glyph: Glyph, system: &'a DesignSystem) -> Self {
        Self {
            glyph,
            system,
            role: Role::Text,
            label: None,
            min_width: 0,
            gap: 1,
        }
    }

    /// Role for paint.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Visible label after the glyph (recommended for status/action icons).
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Minimum glyph column width (pads on the right).
    #[must_use]
    pub const fn min_width(mut self, cols: u16) -> Self {
        self.min_width = cols;
        self
    }

    /// Resolved cells under the current design-system glyph profile.
    #[must_use]
    pub fn resolved(&self) -> GlyphResolved {
        self.system.glyphs.resolve(self.glyph)
    }

    /// Accessible meaning (always non-empty).
    #[must_use]
    pub const fn meaning(&self) -> &'static str {
        self.glyph.meaning()
    }

    /// Plain text for copy / help: `"glyph meaning"` or `"glyph label"`.
    #[must_use]
    pub fn plain(&self) -> String {
        let r = self.resolved();
        match self.label {
            Some(l) if !l.is_empty() => format!("{} {l}", r.text),
            _ => format!("{} ({})", r.text, r.meaning),
        }
    }

    /// Layout without paint.
    #[must_use]
    pub fn layout(&self, area: Rect) -> IconParts {
        if area.is_empty() {
            return IconParts {
                root: area,
                glyph: area,
                label: Rect {
                    x: area.x,
                    y: area.y,
                    width: 0,
                    height: 0,
                },
            };
        }
        let r = self.resolved();
        let g_w = r.display_width().max(self.min_width).min(area.width).max(1);
        let glyph = Rect {
            x: area.x,
            y: area.y,
            width: g_w,
            height: 1u16.min(area.height),
        };
        let label = if self.label.is_some() {
            let x = glyph.right().saturating_add(self.gap);
            let w = area.right().saturating_sub(x);
            Rect {
                x,
                y: area.y,
                width: w,
                height: if w > 0 { 1u16.min(area.height) } else { 0 },
            }
        } else {
            Rect {
                x: glyph.right(),
                y: area.y,
                width: 0,
                height: 0,
            }
        };
        IconParts {
            root: area,
            glyph,
            label,
        }
    }

    /// Paint glyph (+ optional label).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> IconParts {
        let parts = self.layout(area);
        if parts.glyph.is_empty() {
            return parts;
        }
        let r = self.resolved();
        let cell = r.aligned(parts.glyph.width);
        let style = self.system.style(self.role);
        buffer.set_stringn(
            parts.glyph.x,
            parts.glyph.y,
            &cell,
            usize::from(parts.glyph.width),
            style,
        );
        if let Some(label) = self.label
            && parts.label.width > 0
        {
            let span = TextSpan::new(label).role(self.role);
            let _ = Text::spans([span], self.system)
                .truncate()
                .paint(parts.label, buffer);
        }
        parts
    }
}

impl Widget for &Icon<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Icon<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meaning_always_present() {
        let system = DesignSystem::default();
        let icon = Icon::new(Glyph::Success, &system);
        assert_eq!(icon.meaning(), "success");
        assert!(icon.plain().contains("success"));
    }

    #[test]
    fn label_prevents_glyph_only_plain() {
        let system = DesignSystem::default();
        let icon = Icon::new(Glyph::Error, &system).label("failed");
        assert!(icon.plain().contains("failed"));
    }

    #[test]
    fn paint_glyph_and_label() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let parts = Icon::new(Glyph::Success, &system)
            .role(Role::Success)
            .label("ok")
            .paint(Rect::new(0, 0, 20, 1), &mut buf);
        assert!(parts.glyph.width >= 1);
        assert!(parts.label.width > 0);
        assert_eq!(buf[(parts.label.x, 0)].symbol(), "o");
    }

    #[test]
    fn min_width_aligns() {
        let system = DesignSystem::default();
        let icon = Icon::new(Glyph::Add, &system).min_width(3);
        let parts = icon.layout(Rect::new(0, 0, 10, 1));
        assert_eq!(parts.glyph.width, 3);
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = Icon::new(Glyph::Close, &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(parts.root.is_empty() || parts.glyph.is_empty());
    }
}
