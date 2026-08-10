// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Empty, loading, error, and banner feedback views.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Role, RolePalette},
    text::{display_cols, take_display_cols},
    widgets::Severity,
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
        let text = if self.frame.is_empty() {
            self.label.to_owned()
        } else {
            format!("{} {}", self.frame, self.label)
        };
        let row = Center::new(area.width, 1)
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        paint_centered_line(area, buffer, row.y, &text, self.system.style(Role::Info));
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

// ErrorView / ErrorState live in `widgets/error_state.rs`.

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
        let (glyph, role) = match self.severity {
            Severity::Info => ("ℹ", Role::Info),
            Severity::Success => ("✓", Role::Success),
            Severity::Warning => ("!", Role::Warning),
            Severity::Error => ("✗", Role::Danger),
        };
        let line = format!("{glyph} {}", self.message);
        let clipped = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
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
) {
    use crate::layout::center_line_x;
    let width = display_cols(text).min(usize::from(area.width));
    let clipped = take_display_cols(text, width);
    let x = center_line_x(area, width as u16);
    buffer.set_stringn(x, y, &clipped, width, style);
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
        assert_eq!(buffer[(0, 0)].symbol(), "✓");
    }
}
