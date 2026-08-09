// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Empty, loading, error, and banner feedback views.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Role, RolePalette},
    text::{display_cols, take_display_cols},
    widgets::Severity,
};

/// Centered empty-state message with optional non-color glyph.
#[derive(Debug, Clone, Copy)]
pub struct EmptyState<'a> {
    title: &'a str,
    detail: Option<&'a str>,
    glyph: &'a str,
    system: &'a DesignSystem,
}

impl<'a> EmptyState<'a> {
    /// Creates an empty state with the default hollow-circle glyph.
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            detail: None,
            glyph: "○",
            system,
        }
    }

    /// Sets secondary detail text.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Overrides the non-color status glyph.
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.glyph = glyph;
        self
    }
}

impl Widget for &EmptyState<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let title = format!("{} {}", self.glyph, self.title);
        let mut y = area.y + area.height.saturating_sub(2) / 2;
        paint_centered_line(area, buffer, y, &title, self.system.style(Role::TextMuted));
        if let Some(detail) = self.detail {
            y = y.saturating_add(1);
            if y < area.bottom() {
                paint_centered_line(
                    area,
                    buffer,
                    y,
                    detail,
                    self.system.style(Role::TextDisabled),
                );
            }
        }
    }
}

impl Widget for EmptyState<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

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
        let text = if self.frame.is_empty() {
            self.label.to_owned()
        } else {
            format!("{} {}", self.frame, self.label)
        };
        let y = area.y + area.height.saturating_sub(1) / 2;
        paint_centered_line(area, buffer, y, &text, self.system.style(Role::Info));
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

/// Error or failure surface with non-color marker.
#[derive(Debug, Clone, Copy)]
pub struct ErrorView<'a> {
    title: &'a str,
    detail: Option<&'a str>,
    system: &'a DesignSystem,
}

impl<'a> ErrorView<'a> {
    /// Creates an error view with the danger cross marker.
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            detail: None,
            system,
        }
    }

    /// Sets secondary detail text.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }
}

impl Widget for &ErrorView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let title = format!("✗ {}", self.title);
        let mut y = area.y + area.height.saturating_sub(2) / 2;
        paint_centered_line(area, buffer, y, &title, self.system.style(Role::Danger));
        if let Some(detail) = self.detail {
            y = y.saturating_add(1);
            if y < area.bottom() {
                paint_centered_line(area, buffer, y, detail, self.system.style(Role::TextMuted));
            }
        }
    }
}

impl Widget for ErrorView<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

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

/// Skeleton placeholder lines for loading lists.
#[derive(Debug, Clone, Copy)]
pub struct Skeleton<'a> {
    rows: u16,
    system: &'a DesignSystem,
}

impl<'a> Skeleton<'a> {
    /// Creates a skeleton with the requested row count.
    #[must_use]
    pub const fn new(rows: u16, system: &'a DesignSystem) -> Self {
        Self { rows, system }
    }
}

impl Widget for &Skeleton<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let rows = self.rows.min(area.height);
        let bar_width = area.width.saturating_mul(3) / 4;
        for row in 0..rows {
            let y = area.y.saturating_add(row);
            let indent = if row % 2 == 0 { 0 } else { 2u16 };
            let width = bar_width
                .saturating_sub(indent)
                .min(area.width.saturating_sub(indent));
            if width == 0 {
                continue;
            }
            let x = area.x.saturating_add(indent);
            let fill = "░".repeat(usize::from(width));
            buffer.set_stringn(
                x,
                y,
                &fill,
                usize::from(width),
                self.system.style(Role::TextDisabled),
            );
        }
    }
}

impl Widget for Skeleton<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn paint_centered_line(
    area: Rect,
    buffer: &mut Buffer,
    y: u16,
    text: &str,
    style: ratatui_core::style::Style,
) {
    let width = display_cols(text).min(usize::from(area.width));
    let clipped = take_display_cols(text, width);
    let x = area
        .x
        .saturating_add(area.width.saturating_sub(width as u16) / 2);
    buffer.set_stringn(x, y, &clipped, width, style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;

    #[test]
    fn empty_state_paints_glyph_and_title() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 5));
        EmptyState::new("No results", &system)
            .detail("Try another query")
            .render(Rect::new(0, 0, 40, 5), &mut buffer);
        let mut painted = String::new();
        for y in 0..5 {
            for x in 0..40 {
                painted.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(painted.contains('○'), "{painted:?}");
        assert!(painted.contains("No results"), "{painted:?}");
    }

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
