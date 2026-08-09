// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Optional terminal image surface protocol.
//!
//! TermRock does not embed media decoders and **never** emits graphics protocol
//! bytes from [`Widget::render`]. Callers supply metadata; this widget paints a
//! product-neutral cell fallback. Pair with [`crate::style::CapabilityPreviewHost`]
//! for generation-safe placement planning and session commands.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, PreviewPresentation, Role, RolePalette},
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

impl ImageProtocol {
    /// Maps a host presentation plan to the protocol enum.
    #[must_use]
    pub const fn from_presentation(presentation: PreviewPresentation) -> Self {
        match presentation {
            PreviewPresentation::CellFallback => Self::Placeholder,
            PreviewPresentation::Kitty => Self::Kitty,
            PreviewPresentation::ITerm2 => Self::ITerm2,
            PreviewPresentation::Sixel => Self::Sixel,
        }
    }

    /// Maps to a host presentation plan.
    #[must_use]
    pub const fn presentation(self) -> PreviewPresentation {
        match self {
            Self::Placeholder => PreviewPresentation::CellFallback,
            Self::Kitty => PreviewPresentation::Kitty,
            Self::ITerm2 => PreviewPresentation::ITerm2,
            Self::Sixel => PreviewPresentation::Sixel,
        }
    }
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
    /// Async load pending.
    pub pending: bool,
    /// Content is stale vs current selection generation.
    pub stale: bool,
    /// Generation token from preview host (0 = unset).
    pub generation: u64,
}

impl<'a> ImageMeta<'a> {
    /// Creates basic metadata without lifecycle flags.
    #[must_use]
    pub const fn new(label: &'a str, protocol: ImageProtocol) -> Self {
        Self {
            label,
            pixel_width: None,
            pixel_height: None,
            protocol,
            pending: false,
            stale: false,
            generation: 0,
        }
    }
}

/// Renders a framed image slot. Does not decode bytes or emit protocol.
#[derive(Debug, Clone, Copy)]
pub struct ImageSurface<'a> {
    meta: ImageMeta<'a>,
    system: &'a DesignSystem,
}

impl<'a> ImageSurface<'a> {
    /// Creates an image surface.
    #[must_use]
    pub const fn new(meta: ImageMeta<'a>, system: &'a DesignSystem) -> Self {
        Self { meta, system }
    }

    /// Meta borrow.
    #[must_use]
    pub const fn meta(&self) -> &ImageMeta<'a> {
        &self.meta
    }
}

/// Describes intended protocol emission for a consumer-owned media session.
///
/// Never write this string to the terminal from Widget code — it is a hint only.
#[must_use]
pub fn protocol_emission_hint(protocol: ImageProtocol, resource_id: &str) -> String {
    match protocol {
        ImageProtocol::Placeholder => format!("cell-fallback:{resource_id}"),
        ImageProtocol::Kitty => format!("kitty-place:{resource_id}"),
        ImageProtocol::Sixel => format!("sixel-place:{resource_id}"),
        ImageProtocol::ITerm2 => format!("iterm2-place:{resource_id}"),
    }
}

impl Widget for &ImageSurface<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let border = self.system.style(Role::Border);
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
        let status = if self.meta.stale {
            "stale"
        } else if self.meta.pending {
            "loading"
        } else {
            "ready"
        };
        let line1 = format!("▣ {}", self.meta.label);
        let line2 = format!("{proto} · {dims} · {status}");
        let style = if self.meta.stale || self.meta.pending {
            self.system.style(Role::TextMuted)
        } else {
            self.system.style(Role::Text)
        };
        buffer.set_stringn(
            inner.x,
            inner.y,
            take_display_cols(&line1, usize::from(inner.width)),
            usize::from(inner.width),
            style,
        );
        if inner.height > 1 {
            buffer.set_stringn(
                inner.x,
                inner.y + 1,
                take_display_cols(&line2, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
        }
        if inner.height > 2 && self.meta.generation > 0 {
            let generation_label = format!("gen {}", self.meta.generation);
            buffer.set_stringn(
                inner.x,
                inner.y + 2,
                take_display_cols(&generation_label, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
        }
    }
}

impl Widget for ImageSurface<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicit owned→borrowed Widget delegate"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn paints_label_and_lifecycle_flags() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let mut meta = ImageMeta::new("shot.png", ImageProtocol::Kitty);
        meta.pending = true;
        meta.generation = 3;
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        Widget::render(ImageSurface::new(meta, &system), area, &mut buffer);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("shot") || text.contains("png"), "{text:?}");
        assert!(
            text.contains("loading") || text.contains("kitty"),
            "{text:?}"
        );
        assert!(text.contains('3') || text.contains("gen"), "{text:?}");
    }

    #[test]
    fn protocol_hint_never_empty() {
        assert!(!protocol_emission_hint(ImageProtocol::Kitty, "a").is_empty());
    }
}
