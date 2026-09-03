use ratatui_core::{buffer::Buffer, layout::Rect};

use super::*;

#[test]
fn scrollbar_styles_use_canonical_glyphs() {
    assert_eq!(ScrollbarStyle::Line.vertical_thumb(), "┃");
    assert_eq!(ScrollbarStyle::Block.vertical_thumb(), "█");
    assert_eq!(SCROLLBAR_HORIZONTAL_THUMB, "━");
}

#[test]
fn vertical_thumb_moves_and_keeps_length() {
    let render = |offset| {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 10));
        let area = buffer.area;
        paint_scrollbar(
            &mut buffer,
            area,
            ScrollbarSpec::new(
                scroll::ScrollAxis::Vertical,
                ScrollbarGeometry::new(20, 5, offset),
            ),
            &crate::style::DesignSystem::default(),
        );
        (0..10)
            .filter(|y| buffer[(0, *y)].symbol() == "┃")
            .collect::<Vec<_>>()
    };
    let top = render(0);
    let bottom = render(15);
    assert_eq!(top.len(), bottom.len());
    assert_eq!(top.first(), Some(&0));
    assert_eq!(bottom.last(), Some(&9));
}

#[test]
fn block_style_only_changes_vertical_thumb() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 5));
    let area = buffer.area;
    paint_scrollbar(
        &mut buffer,
        area,
        ScrollbarSpec::new(
            scroll::ScrollAxis::Vertical,
            ScrollbarGeometry::new(10, 5, 0),
        )
        .style(ScrollbarStyle::Block),
        &crate::style::DesignSystem::default(),
    );
    assert!((0..5).any(|y| buffer[(0, y)].symbol() == "█"));
}

#[test]
fn scrollbar_uses_semantic_theme_roles() {
    let system = crate::style::DesignSystem::junie();
    let theme = system.junie_theme();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 5));
    let area = buffer.area;
    paint_scrollbar(
        &mut buffer,
        area,
        ScrollbarSpec::new(
            scroll::ScrollAxis::Vertical,
            ScrollbarGeometry::new(10, 5, 0),
        )
        .focused(true)
        .hovered(true),
        &system,
    );
    // The thumb states the surface's focus and hover through the one
    // scrollbar resolver; the track stays the quiet rail.
    assert_eq!(
        buffer[(0, 0)].fg,
        theme.scrollbar_thumb(true, true).fg.unwrap()
    );
    assert_eq!(buffer[(0, 4)].fg, theme.scrollbar_track().fg.unwrap());
}

#[test]
fn overflow_gutter_uses_junie_thumb_length() {
    let system = crate::style::DesignSystem::default();
    let gutter = Rect::new(0, 0, 1, 15);
    let mut buffer = Buffer::empty(gutter);
    paint_overflow_scrollbar(&mut buffer, gutter, 24, 15, 0, false, &system);
    let thumbs: Vec<u16> = (0..15)
        .filter(|y| buffer[(0, *y)].symbol() == ScrollbarStyle::Line.vertical_thumb())
        .collect();
    assert_eq!(thumbs, (0..9).collect::<Vec<_>>());
    assert_eq!(buffer[(0, 9)].symbol(), SCROLLBAR_TRACK);
}

#[test]
fn overflow_gutter_paints_the_canonical_language_only_when_scrollable() {
    let system = DesignSystem::default();
    let gutter = Rect::new(0, 0, 1, 4);
    let paint = |total: usize, offset: u16| {
        let mut buffer = Buffer::empty(gutter);
        paint_overflow_scrollbar(&mut buffer, gutter, total, 4, offset, false, &system);
        (0..gutter.height)
            .map(|y| buffer[(0, y)].symbol().to_string())
            .collect::<Vec<_>>()
    };
    // Content fits: a reserved gutter stays blank instead of showing a full thumb.
    assert_eq!(paint(4, 0), vec![" ", " ", " ", " "]);
    let scrolled = paint(16, 0);
    assert_eq!(scrolled[0], ScrollbarStyle::Line.vertical_thumb());
    assert_eq!(scrolled[3], SCROLLBAR_TRACK);
    let bottom = paint(16, 12);
    assert_eq!(bottom[0], SCROLLBAR_TRACK);
    assert_eq!(bottom[3], ScrollbarStyle::Line.vertical_thumb());
}
