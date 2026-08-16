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

/// Cell width in pixels (matches the frame/preview seam).
pub const CELL_WIDTH_PX: u32 = 9;
/// Cell height in pixels (matches the frame/preview seam).
pub const CELL_HEIGHT_PX: u32 = 18;
/// Font size in pixels: max(11, floor(18 * 0.78)) = 14.
pub const FONT_SIZE_PX: f32 = 14.0;
/// Baseline offset from cell top in pixels: floor(18 * 0.78) = 14.
pub const BASELINE_PX: u32 = 14;
