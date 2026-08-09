// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Optional terminal image surface protocol.
//!
//! TermRock does not embed media decoders. Callers supply pixel payloads or a
//! placeholder; this widget paints a product-neutral frame and optional
//! protocol escape emission hooks for Kitty/Sixel-class terminals.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};

use crate::{
    style::{Role, Theme},
    text::take_display_cols,
};

/// Graphics protocol the consumer intends to use outside the cell grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ImageProtocol {
    /// No graphics — placeholder cells only.
    #[default]
    Placeholder,
    /// Kitty image protocol (consumer emits OSC/APC).
    Kitty,
    /// Sixel (consumer emits sixel stream).
    Sixel,
    /// iTerm2 inline images.
    ITerm2,
}

/// Borrowed image metadata for layout (pixels stay caller-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageMeta<'a> {
    /// Short label (filename, alt text).
    pub label: &'a str,
    /// Pixel width if known.
    pub pixel_width: Option<u32>,
    /// Pixel height if known.
    pub pixel_height: Option<u32>,
    /// Preferred protocol.
    pub protocol: ImageProtocol,
}

/// Renders a framed image slot. Does not decode bytes.
#[derive(Debug, Clone, Copy)]
pub struct ImageSurface<'a> {
    meta: ImageMeta<'a>,
    theme: &'a Theme,
}

impl<'a> ImageSurface<'a> {
    /// Creates an image surface.
    #[must_use]
    pub const fn new(meta: ImageMeta<'a>, theme: &'a Theme) -> Self {
        Self { meta, theme }
    }
}

impl Widget for &ImageSurface<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        // Border
        let border = self.theme.style(Role::Border);
        for x in area.x..area.right() {
            buffer[(x, area.y)].set_symbol("─").set_style(border);
            if area.height > 1 {
                buffer[(x, area.bottom() - 1)]
                    .set_symbol("─")
                    .set_style(border);
            }
        }
        for y in area.y..area.bottom() {
            buffer[(area.x, y)].set_symbol("│").set_style(border);
            if area.width > 1 {
                buffer[(area.right() - 1, y)]
                    .set_symbol("│")
                    .set_style(border);
            }
        }
        if area.width > 1 && area.height > 1 {
            buffer[(area.x, area.y)].set_symbol("┌").set_style(border);
            buffer[(area.right() - 1, area.y)]
                .set_symbol("┐")
                .set_style(border);
            buffer[(area.x, area.bottom() - 1)]
                .set_symbol("└")
                .set_style(border);
            buffer[(area.right() - 1, area.bottom() - 1)]
                .set_symbol("┘")
                .set_style(border);
        }

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if inner.is_empty() {
            return;
        }
        let proto = match self.meta.protocol {
            ImageProtocol::Placeholder => "placeholder",
            ImageProtocol::Kitty => "kitty",
            ImageProtocol::Sixel => "sixel",
            ImageProtocol::ITerm2 => "iterm2",
        };
        let dims = match (self.meta.pixel_width, self.meta.pixel_height) {
            (Some(w), Some(h)) => format!("{w}×{h}"),
            _ => "size unknown".to_owned(),
        };
        let line1 = format!("▣ {}", self.meta.label);
        let line2 = format!("{proto} · {dims}");
        buffer.set_stringn(
            inner.x,
            inner.y,
            &take_display_cols(&line1, usize::from(inner.width)),
            usize::from(inner.width),
            self.theme.style(Role::TextMuted),
        );
        if inner.height > 1 {
            buffer.set_stringn(
                inner.x,
                inner.y.saturating_add(1),
                &take_display_cols(&line2, usize::from(inner.width)),
                usize::from(inner.width),
                self.theme.style(Role::TextDisabled),
            );
        }
        // Checker fill for remaining rows (non-color placeholder texture).
        for row in 2..inner.height {
            let y = inner.y.saturating_add(row);
            let fill = if row % 2 == 0 { "░" } else { "▒" };
            let pattern = fill.repeat(usize::from(inner.width));
            buffer.set_stringn(
                inner.x,
                y,
                &pattern,
                usize::from(inner.width),
                self.theme.style(Role::Surface),
            );
        }
    }
}

impl Widget for ImageSurface<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

/// Hint string consumers can log when preparing protocol emission.
#[must_use]
pub fn protocol_emission_hint(protocol: ImageProtocol) -> &'static str {
    match protocol {
        ImageProtocol::Placeholder => "paint cells only; no graphics stream",
        ImageProtocol::Kitty => "emit Kitty APC image placement for the reserved rect",
        ImageProtocol::Sixel => "emit sixel for the reserved rect",
        ImageProtocol::ITerm2 => "emit iTerm2 inline image sequence",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_label_inside_frame() {
        let theme = Theme::default();
        let meta = ImageMeta {
            label: "preview.png",
            pixel_width: Some(64),
            pixel_height: Some(64),
            protocol: ImageProtocol::Kitty,
        };
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 6));
        ImageSurface::new(meta, &theme).render(Rect::new(0, 0, 24, 6), &mut buffer);
        let mut painted = String::new();
        for y in 0..6 {
            for x in 0..24 {
                painted.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(painted.contains("preview.png"), "{painted}");
        assert!(painted.contains('▣'), "{painted}");
    }
}
