// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use ratatui::{buffer::Buffer, style::Modifier};
use swash::{
    scale::{Render, ScaleContext, Source},
    zeno::Format,
};
use termrock::style::{Role, RolePalette};
use unicode_width::UnicodeWidthStr;

use crate::{BASELINE_PX, CELL_HEIGHT_PX, CELL_WIDTH_PX, FONT_SIZE_PX, color::color_to_rgb, fonts};

/// Errors from rendering or encoding.
#[derive(Debug)]
pub enum RenderError {
    /// Buffer or resulting pixel dimensions are empty or invalid.
    EmptyBuffer,
    /// PNG encoding failed.
    Encode(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBuffer => f.write_str("cannot rasterize an empty buffer"),
            Self::Encode(message) => write!(f, "cannot encode PNG: {message}"),
        }
    }
}

impl std::error::Error for RenderError {}

fn fill_rect(
    pixmap: &mut tiny_skia::Pixmap,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    rgb: [u8; 3],
) {
    for y in top..top + height {
        for x in left..left + width {
            let offset = ((y * pixmap.width() + x) * 4) as usize;
            pixmap.data_mut()[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

fn composite_glyph(
    pixmap: &mut tiny_skia::Pixmap,
    image: &swash::scale::image::Image,
    pen_x: i32,
    pen_y: i32,
    clip: (i32, i32, i32, i32),
    fg: [u8; 3],
) {
    let left = pen_x + image.placement.left;
    let top = pen_y - image.placement.top;
    for image_y in 0..image.placement.height as i32 {
        let y = top + image_y;
        if y < clip.1 || y >= clip.3 {
            continue;
        }
        for image_x in 0..image.placement.width as i32 {
            let x = left + image_x;
            if x < clip.0 || x >= clip.2 {
                continue;
            }
            let alpha =
                image.data[(image_y as u32 * image.placement.width + image_x as u32) as usize];
            if alpha == 0 {
                continue;
            }
            let offset = ((y as u32 * pixmap.width() + x as u32) * 4) as usize;
            let pixel = &mut pixmap.data_mut()[offset..offset + 4];
            for channel in 0..3 {
                pixel[channel] = ((u16::from(fg[channel]) * u16::from(alpha)
                    + u16::from(pixel[channel]) * u16::from(255 - alpha))
                    / 255) as u8;
            }
            pixel[3] = 255;
        }
    }
}

/// Rasterize a ratatui buffer using vendored JetBrains Mono faces.
pub fn render_pixmap(
    buffer: &Buffer,
    palette: &RolePalette,
) -> Result<tiny_skia::Pixmap, RenderError> {
    let area = buffer.area;
    let width = u32::from(area.width)
        .checked_mul(CELL_WIDTH_PX)
        .ok_or(RenderError::EmptyBuffer)?;
    let height = u32::from(area.height)
        .checked_mul(CELL_HEIGHT_PX)
        .ok_or(RenderError::EmptyBuffer)?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(RenderError::EmptyBuffer)?;
    let canvas = palette
        .style(Role::Canvas)
        .bg
        .map_or([0, 0, 0], |color| color_to_rgb(color, false));
    fill_rect(&mut pixmap, 0, 0, width, height, canvas);
    let mut scale_context = ScaleContext::new();

    for row in 0..area.height {
        let mut column = 0;
        while column < area.width {
            let cell = &buffer[(area.x + column, area.y + row)];
            let mut fg = color_to_rgb(cell.fg, true);
            let mut bg = color_to_rgb(cell.bg, false);
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.modifier.contains(Modifier::DIM) {
                fg = fg.map(|channel| (u16::from(channel) * 6 / 10) as u8);
            }
            let symbol = cell.symbol();
            let span = UnicodeWidthStr::width(symbol)
                .clamp(1, 2)
                .min(usize::from(area.width - column)) as u16;
            let left = u32::from(column) * CELL_WIDTH_PX;
            let top = u32::from(row) * CELL_HEIGHT_PX;
            let span_width = u32::from(span) * CELL_WIDTH_PX;
            fill_rect(&mut pixmap, left, top, span_width, CELL_HEIGHT_PX, bg);

            if !symbol.is_empty() && !symbol.chars().all(char::is_whitespace) {
                let font = fonts::face(cell.modifier);
                let mut scaler = scale_context
                    .builder(font)
                    .size(FONT_SIZE_PX)
                    .hint(true)
                    .build();
                for character in symbol.chars() {
                    let glyph = font.charmap().map(character);
                    if let Some(image) = Render::new(&[Source::Outline])
                        .format(Format::Alpha)
                        .render(&mut scaler, glyph)
                    {
                        composite_glyph(
                            &mut pixmap,
                            &image,
                            left as i32,
                            (top + BASELINE_PX) as i32,
                            (
                                left as i32,
                                top as i32,
                                (left + span_width) as i32,
                                (top + CELL_HEIGHT_PX) as i32,
                            ),
                            fg,
                        );
                    }
                }
            }
            if cell.modifier.contains(Modifier::UNDERLINED) {
                fill_rect(&mut pixmap, left, top + 15, span_width, 2, fg);
            }
            if cell.modifier.contains(Modifier::CROSSED_OUT) {
                fill_rect(&mut pixmap, left, top + 8, span_width, 2, fg);
            }
            column += span;
        }
    }
    Ok(pixmap)
}

/// Rasterize and encode a ratatui buffer as PNG.
pub fn render_png(buffer: &Buffer, palette: &RolePalette) -> Result<Vec<u8>, RenderError> {
    render_pixmap(buffer, palette)?
        .encode_png()
        .map_err(|error| RenderError::Encode(error.to_string()))
}
