// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Glyph coverage contracts for the vendored face chain.
//!
//! Before the fallback chain existed, every cell the primary JetBrains Mono
//! face could not map — the braille spinner frames, the braille density ramp,
//! `⧉`, `⑂`, and the emoji cells — rasterized as a `.notdef` tofu box. These
//! tests keep that class of defect dead.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};
use termrock::style::{
    BLOCK_RAMP, BRAILLE_RAMP, Glyph, LEFT_BLOCK_RAMP, RolePalette, SHADE_RAMP,
    SPINNER_BRAILLE_FRAMES,
};
use termrock_raster::render_pixmap;

/// Two cells wide so wide (emoji) symbols are stored instead of clipped.
fn cell_buffer(symbol: &str) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
    buffer.set_string(0, 0, symbol, Style::new().fg(Color::Rgb(0, 255, 65)));
    buffer
}

fn ink_pixels(pixmap: &tiny_skia::Pixmap) -> usize {
    pixmap
        .data()
        .chunks_exact(4)
        .filter(|rgba| rgba[0] != 0 || rgba[1] != 0 || rgba[2] != 0)
        .count()
}

/// The tofu law: a character no vendored face maps paints an empty cell, never
/// the `.notdef` box. Renders must be indistinguishable from a blank cell.
#[test]
fn unmapped_character_paints_nothing_instead_of_tofu() {
    // U+0378 is unassigned in every Unicode version the vendored faces predate.
    let unmapped = char::from_u32(0x0378).expect("unassigned codepoint");
    let palette = RolePalette::default();
    let blank = render_pixmap(&cell_buffer(" "), &palette).expect("blank");
    let tofu = render_pixmap(&cell_buffer(&unmapped.to_string()), &palette).expect("unmapped");
    assert_eq!(
        blank.data(),
        tofu.data(),
        "unmapped character rendered ink: .notdef tofu is being rasterized",
    );
}

/// Cells the primary face cannot map must come from a fallback face with real
/// outlines — the canonical braille spinner, the density ramp, and the
/// copy/branch affordances.
#[test]
fn fallback_cells_paint_ink() {
    let palette = RolePalette::default();
    for symbol in ["⠋", "⠹", "⣿", "⧉", "⑂", "⬡", "🚀", "🧪", "✅", "✨"] {
        let pixmap = render_pixmap(&cell_buffer(symbol), &palette)
            .unwrap_or_else(|error| panic!("{symbol}: {error:?}"));
        assert!(
            ink_pixels(&pixmap) > 0,
            "{symbol} resolved through the fallback chain yet painted no ink",
        );
    }
}

/// Every character in the canonical catalog vocabulary must resolve through the
/// vendored face chain, so a newly painted glyph cannot reintroduce tofu (or a
/// silently blank cell) in the PNG output.
#[test]
fn every_catalog_vocabulary_cell_resolves() {
    let mut cells = String::new();
    for glyph in Glyph::ALL {
        cells.push_str(glyph.resolve().text);
    }
    let ramps: [&[char]; 4] = [BLOCK_RAMP, LEFT_BLOCK_RAMP, SHADE_RAMP, BRAILLE_RAMP];
    for ramp in ramps {
        cells.extend(ramp.iter().copied());
    }
    for frame in SPINNER_BRAILLE_FRAMES {
        cells.push_str(frame);
    }
    cells.push_str("╭╮╰╯");
    let unresolved: Vec<String> = cells
        .chars()
        .filter(|character| !character.is_whitespace())
        .filter(|&character| !termrock_raster::is_glyph_mapped(character))
        .map(|character| format!("{character} U+{:04X}", character as u32))
        .collect();
    assert!(
        unresolved.is_empty(),
        "unmapped catalog glyphs: {unresolved:?}"
    );
}
