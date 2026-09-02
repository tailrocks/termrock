// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Empty, loading, error, and banner feedback views.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Glyph, Role, RolePalette},
    text::{display_cols, take_display_cols},
    widgets::{SemanticStatus, Severity},
};

// EmptyState lives in `widgets/empty_state.rs`.

/// Loading placeholder with optional spinner frame and label.
#[derive(Debug, Clone, Copy)]
pub struct LoadingView<'a> {
    label: &'a str,
    frame: &'a str,
    system: &'a DesignSystem,
}

impl<'a> LoadingView<'a> {
    /// Creates a loading view with a braille spinner frame.
    #[must_use]
    pub const fn new(label: &'a str, frame: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            frame,
            system,
        }
    }
}

impl Widget for &LoadingView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        use crate::layout::{Center, CenterAxis};
        let rail = self.system.glyphs.resolve(Glyph::RailHeavy).text;
        let frame = if self.system.motion.animate_spinners() && !self.frame.is_empty() {
            self.frame
        } else {
            SemanticStatus::Running.glyph()
        };
        let text = format!("{rail} {frame} {}", self.label);
        let row = Center::new(area.width, 1)
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        let x = paint_centered_line(area, buffer, row.y, &text, self.system.style(Role::Text));
        let status_style = self.system.style(Role::TextMuted);
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            Rect::new(x, row.y, area.right().saturating_sub(x), 1),
            0,
            rail,
            status_style,
        );
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            Rect::new(x, row.y, area.right().saturating_sub(x), 1),
            2,
            frame,
            status_style,
        );
    }
}

impl Widget for LoadingView<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ErrorState lives in `widgets/error_state.rs`.

/// Single-line status banner (success/warning/error/info).
#[derive(Debug, Clone, Copy)]
pub struct Banner<'a> {
    message: &'a str,
    severity: Severity,
    system: &'a DesignSystem,
}

impl<'a> Banner<'a> {
    /// Creates a banner for the given severity.
    #[must_use]
    pub const fn new(message: &'a str, severity: Severity, system: &'a DesignSystem) -> Self {
        Self {
            message,
            severity,
            system,
        }
    }
}

impl Widget for &Banner<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let rail = self.system.glyphs.resolve(Glyph::RailHeavy).text;
        let (glyph, role) = match self.severity {
            Severity::Info => ("•", Role::TextSecondary),
            Severity::Success => ("✓", Role::TextStrong),
            Severity::Warning => ("!", Role::Warning),
            Severity::Error => ("×", Role::Danger),
        };
        let line = format!("{rail} {glyph} {}", self.message);
        let clipped = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            self.system.style(Role::Text),
        );
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            area,
            0,
            rail,
            self.system.style(role),
        );
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            area,
            2,
            glyph,
            self.system.style(role),
        );
    }
}

impl Widget for Banner<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// Skeleton lives in `widgets/skeleton.rs`.

fn paint_centered_line(
    area: Rect,
    buffer: &mut Buffer,
    y: u16,
    text: &str,
    style: ratatui_core::style::Style,
) -> u16 {
    use crate::layout::center_line_x;
    let width = display_cols(text).min(usize::from(area.width));
    let clipped = take_display_cols(text, width);
    let x = center_line_x(area, width as u16);
    buffer.set_stringn(x, y, &clipped, width, style);
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;

    #[test]
    fn banner_uses_non_color_success_glyph() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));
        Banner::new("Saved", Severity::Success, &system)
            .render(Rect::new(0, 0, 20, 1), &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "┃");
        assert_eq!(buffer[(2, 0)].symbol(), "✓");
    }
}
