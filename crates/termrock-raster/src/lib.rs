// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Deterministic ratatui `Buffer` to PNG rendering for TermRock visual baselines.
//!
//! Rendering uses only vendored fonts and pure-Rust raster/compositing inputs.
//! Baseline comparison always decodes PNGs and compares pixels at zero tolerance.

mod color;
mod compare;
mod fonts;
mod render;

pub use compare::{PixelDiff, compare_png_pixels};
pub use render::{RenderError, render_pixmap, render_png};

/// Whether the vendored face chain maps `character` to a real glyph.
///
/// False means the rasterizer paints an empty cell for `character`; true means
/// it paints its own outline. `.notdef` tofu is never rendered either way.
#[must_use]
pub fn is_glyph_mapped(character: char) -> bool {
    fonts::is_mapped(ratatui::style::Modifier::empty(), character)
}

/// Cell width in pixels (matches the frame/preview seam).
pub const CELL_WIDTH_PX: u32 = 9;
/// Cell height in pixels (the Junie capture contract).
pub const CELL_HEIGHT_PX: u32 = 20;
/// Source capture padding on each side.
pub const PADDING_PX: u32 = 12;
/// Font size in pixels (the Junie capture contract).
pub const FONT_SIZE_PX: f32 = 15.0;
/// Baseline offset from cell top for the vendored outline rasterizer.
pub const BASELINE_PX: u32 = 15;
