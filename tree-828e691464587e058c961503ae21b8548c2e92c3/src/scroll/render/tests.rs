use ratatui_core::{
    backend::TestBackend,
    layout::Rect,
    style::{Color, Style},
    terminal::Terminal,
    text::{Line, Span},
};

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
        render_scrollbar(
            &mut buffer,
            area,
            ScrollbarSpec::new(
                scroll::ScrollAxis::Vertical,
                ScrollbarGeometry::new(20, 5, offset),
            ),
            &Theme::default(),
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
    render_scrollbar(
        &mut buffer,
        area,
        ScrollbarSpec::new(
            scroll::ScrollAxis::Vertical,
            ScrollbarGeometry::new(10, 5, 0),
        )
        .style(ScrollbarStyle::Block),
        &Theme::default(),
    );
    assert!((0..5).any(|y| buffer[(0, y)].symbol() == "█"));
}

#[test]
fn scrollbar_uses_semantic_theme_roles() {
    let theme = Theme::default()
        .with_role(Role::ScrollTrack, Style::new().fg(Color::Red))
        .with_role(Role::ScrollThumb, Style::new().fg(Color::Blue));
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 5));
    let area = buffer.area;
    render_scrollbar(
        &mut buffer,
        area,
        ScrollbarSpec::new(
            scroll::ScrollAxis::Vertical,
            ScrollbarGeometry::new(10, 5, 0),
        ),
        &theme,
    );
    assert_eq!(buffer[(0, 0)].fg, Color::Blue);
    assert_eq!(buffer[(0, 4)].fg, Color::Red);
}

#[test]
fn fixed_prefix_scroll_preserves_prefix_and_unicode_cells() {
    let backend = TestBackend::new(8, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_line_with_fixed_prefix_scroll(
                frame,
                Rect::new(0, 0, 8, 1),
                0,
                Line::from(vec![
                    Span::styled("P:", Style::new().fg(Color::Green)),
                    Span::raw("東京-tail"),
                ]),
                2,
                2,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].symbol(), "P");
    assert_eq!(buffer[(1, 0)].symbol(), ":");
    assert_eq!(buffer[(2, 0)].symbol(), "京");
}

#[test]
fn delta_helpers_clamp_or_preserve_overshoot_as_named() {
    let mut clamped = 4;
    apply_scroll_delta(&mut clamped, 10, 5, 12);
    assert_eq!(clamped, 7);
    let mut free = 4;
    apply_scroll_delta_unclamped(&mut free, 10);
    assert_eq!(free, 14);
    assert_eq!(clamp_scroll_offset(12, 5, &mut free), 7);
}
