// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Glyph coverage contracts for the vendored face chain.
//!
//! Before the fallback chain existed, every cell the primary JetBrains Mono
//! face could not map — the braille spinner frames, the braille density ramp,
//! `⧉`, `⑂`, and the emoji cells — rasterized as a `.notdef` tofu box. These
//! tests keep that class of defect dead.

use std::collections::BTreeSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};
use termrock::style::RolePalette;
use termrock_lookbook::{frame::paint_story_buffer, png::subset_stories};
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

/// Every character any registered subset story paints must resolve through the
/// vendored face chain, so a newly painted glyph cannot reintroduce tofu (or a
/// silently blank cell) in the PNG baselines.
#[test]
fn every_painted_baseline_cell_resolves() {
    let system = termrock_lookbook::design::lookbook_system(RolePalette::default());
    let mut unresolved: BTreeSet<char> = BTreeSet::new();
    for story in subset_stories() {
        let buffer = paint_story_buffer(story, &system, None, None);
        for x in 0..buffer.area.width {
            for y in 0..buffer.area.height {
                for character in buffer[(x, y)].symbol().chars() {
                    if character.is_whitespace() {
                        continue;
                    }
                    if !termrock_raster::is_glyph_mapped(character) {
                        unresolved.insert(character);
                    }
                }
            }
        }
    }
    assert!(
        unresolved.is_empty(),
        "cells painted by the PNG baselines that no vendored face maps: {}",
        unresolved
            .iter()
            .map(|c| format!("{c} U+{:04X}", *c as u32))
            .collect::<Vec<_>>()
            .join(" "),
    );
}
