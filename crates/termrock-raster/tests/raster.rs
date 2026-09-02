// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! End-to-end contracts for deterministic buffer rasterization.

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use sha2::{Digest, Sha256};
use termrock::style::RolePalette;
use termrock_lookbook::frame::story_by_id;
use termrock_raster::{PixelDiff, compare_png_pixels, render_pixmap, render_png};

fn one_cell(symbol: &str, modifier: Modifier, fg: Color, bg: Color) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
    buffer.set_string(
        0,
        0,
        symbol,
        Style::new().fg(fg).bg(bg).add_modifier(modifier),
    );
    buffer
}

fn pixel(pixmap: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * pixmap.width() + x) * 4) as usize;
    pixmap.data()[offset..offset + 4]
        .try_into()
        .expect("RGBA pixel")
}

#[test]
fn panel_story_png_has_exact_size_and_junie_accent() {
    let story = story_by_id("panel/focused").expect("panel/focused story");
    let palette = RolePalette::default();
    let backend = TestBackend::new(story.width, story.height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut interactor = story.mount();
    interactor.set_system(termrock_lookbook::design::lookbook_system(palette.clone()));
    terminal
        .draw(|frame| interactor.render(frame, frame.area()))
        .expect("paint story");
    let mut buffer = terminal.backend().buffer().clone();
    buffer[(1, 1)]
        .set_symbol("█")
        .set_fg(Color::Rgb(0x48, 0xe0, 0x54));
    let png = render_png(&buffer, &palette).expect("render PNG");
    let pixmap = tiny_skia::Pixmap::decode_png(&png).expect("decode PNG");
    assert_eq!(pixmap.width(), u32::from(story.width) * 9);
    assert_eq!(pixmap.height(), u32::from(story.height) * 18);
    assert!(
        pixmap
            .data()
            .chunks_exact(4)
            .any(|rgba| rgba == [0x48, 0xe0, 0x54, 255])
    );
}

#[test]
fn wide_grapheme_spans_two_cells_and_shadow_cell_is_ignored() {
    // A wide symbol the vendored face chain actually maps (CJK is deliberately
    // not vendored; see `fonts.rs`).
    let mut a = Buffer::empty(Rect::new(0, 0, 4, 1));
    a.set_string(0, 0, "🚀", Style::new().fg(Color::Green));
    let mut b = a.clone();
    b[(1, 0)].set_symbol("X");
    let palette = RolePalette::default();
    let png_a = render_png(&a, &palette).expect("render A");
    let png_b = render_png(&b, &palette).expect("render B");
    assert_eq!(compare_png_pixels(&png_a, &png_b), Ok(()));
    let pixmap = tiny_skia::Pixmap::decode_png(&png_a).expect("decode A");
    let mut halves_equal = true;
    for y in 0..18 {
        for x in 0..9 {
            halves_equal &= pixel(&pixmap, x, y) == pixel(&pixmap, x + 9, y);
        }
    }
    assert!(
        !halves_equal,
        "wide glyph must be drawn once across its span"
    );
}

#[test]
fn italic_selects_italic_face() {
    let palette = RolePalette::default();
    let plain = render_png(
        &one_cell("a", Modifier::empty(), Color::Green, Color::Black),
        &palette,
    )
    .unwrap();
    let italic = render_png(
        &one_cell("a", Modifier::ITALIC, Color::Green, Color::Black),
        &palette,
    )
    .unwrap();
    assert!(compare_png_pixels(&plain, &italic).is_err());
}

#[test]
fn bold_selects_bold_face() {
    let palette = RolePalette::default();
    let plain = render_png(
        &one_cell("a", Modifier::empty(), Color::Green, Color::Black),
        &palette,
    )
    .unwrap();
    let bold = render_png(
        &one_cell("a", Modifier::BOLD, Color::Green, Color::Black),
        &palette,
    )
    .unwrap();
    assert!(compare_png_pixels(&plain, &bold).is_err());
}

#[test]
fn dim_darkens_exactly_once() {
    let pixmap = render_pixmap(
        &one_cell("█", Modifier::DIM, Color::Rgb(0, 255, 65), Color::Black),
        &RolePalette::default(),
    )
    .unwrap();
    assert!(
        pixmap
            .data()
            .chunks_exact(4)
            .any(|rgba| rgba == [0, 153, 39, 255])
    );
    assert!(
        !pixmap
            .data()
            .chunks_exact(4)
            .any(|rgba| rgba == [0, 255, 65, 255])
    );
}

#[test]
fn underline_paints_web_consistent_rows() {
    let pixmap = render_pixmap(
        &one_cell(
            " ",
            Modifier::UNDERLINED,
            Color::Rgb(0, 255, 65),
            Color::Black,
        ),
        &RolePalette::default(),
    )
    .unwrap();
    for x in 0..9 {
        assert_eq!(pixel(&pixmap, x, 15), [0, 255, 65, 255]);
        assert_eq!(pixel(&pixmap, x, 16), [0, 255, 65, 255]);
        assert_eq!(pixel(&pixmap, x, 14), [0, 0, 0, 255]);
        assert_eq!(pixel(&pixmap, x, 17), [0, 0, 0, 255]);
    }
}

#[test]
fn crossed_out_paints_mid_cell_rows() {
    let pixmap = render_pixmap(
        &one_cell(
            " ",
            Modifier::CROSSED_OUT,
            Color::Rgb(0, 255, 65),
            Color::Black,
        ),
        &RolePalette::default(),
    )
    .unwrap();
    for x in 0..9 {
        assert_eq!(pixel(&pixmap, x, 8), [0, 255, 65, 255]);
        assert_eq!(pixel(&pixmap, x, 9), [0, 255, 65, 255]);
        assert_eq!(pixel(&pixmap, x, 7), [0, 0, 0, 255]);
        assert_eq!(pixel(&pixmap, x, 10), [0, 0, 0, 255]);
    }
}

#[test]
fn reversed_swaps_fg_bg() {
    let pixmap = render_pixmap(
        &one_cell(
            " ",
            Modifier::REVERSED,
            Color::Rgb(0, 255, 65),
            Color::Black,
        ),
        &RolePalette::default(),
    )
    .unwrap();
    assert!(
        pixmap
            .data()
            .chunks_exact(4)
            .all(|rgba| rgba == [0, 255, 65, 255])
    );
}

#[test]
fn double_render_identical() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 7, 1));
    let cases = [
        ("┌", Modifier::empty()),
        ("─", Modifier::BOLD),
        ("a", Modifier::ITALIC),
        ("█", Modifier::DIM),
        (" ", Modifier::UNDERLINED),
        ("x", Modifier::CROSSED_OUT),
        (" ", Modifier::REVERSED),
    ];
    for (x, (symbol, modifier)) in cases.into_iter().enumerate() {
        buffer[(x as u16, 0)].set_symbol(symbol).set_style(
            Style::new()
                .fg(Color::Rgb(0, 255, 65))
                .bg(Color::Black)
                .add_modifier(modifier),
        );
    }
    let palette = RolePalette::default();
    let pixmap_a = render_pixmap(&buffer, &palette).unwrap();
    let pixmap_b = render_pixmap(&buffer, &palette).unwrap();
    let png_a = pixmap_a.encode_png().unwrap();
    let png_b = pixmap_b.encode_png().unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(pixmap_a.data())),
        hex::encode(Sha256::digest(pixmap_b.data()))
    );
    assert_eq!(
        hex::encode(Sha256::digest(&png_a)),
        hex::encode(Sha256::digest(&png_b))
    );
    assert_eq!(compare_png_pixels(&png_a, &png_b), Ok(()));
}

#[test]
fn pixel_compare_names_first_differing_coordinate() {
    let mut a = tiny_skia::Pixmap::new(3, 2).unwrap();
    let mut b = tiny_skia::Pixmap::new(3, 2).unwrap();
    a.data_mut()
        .chunks_exact_mut(4)
        .for_each(|rgba| rgba.copy_from_slice(&[10, 20, 30, 255]));
    b.data_mut()
        .chunks_exact_mut(4)
        .for_each(|rgba| rgba.copy_from_slice(&[10, 20, 30, 255]));
    let offset = ((3 + 2) * 4) as usize;
    b.data_mut()[offset] ^= 1;
    let result = compare_png_pixels(&a.encode_png().unwrap(), &b.encode_png().unwrap());
    assert!(matches!(
        result,
        Err(PixelDiff::FirstDifference { x: 2, y: 1, .. })
    ));
    assert!(result.unwrap_err().to_string().contains("(2, 1)"));
}

#[test]
fn pixel_compare_dimension_mismatch() {
    let a = tiny_skia::Pixmap::new(1, 1).unwrap().encode_png().unwrap();
    let b = tiny_skia::Pixmap::new(2, 1).unwrap().encode_png().unwrap();
    assert_eq!(
        compare_png_pixels(&a, &b),
        Err(PixelDiff::DimensionMismatch {
            a: (1, 1),
            b: (2, 1)
        }),
    );
}
