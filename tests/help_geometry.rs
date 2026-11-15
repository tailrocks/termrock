mod common;

use common::*;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::help::HelpOverlay;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

const ID: WidgetId = WidgetId::of("t.help");
const SENTINEL: &str = "#";

fn overlay() -> HelpOverlay {
    HelpOverlay::new(
        ID,
        "Scope",
        vec![
            ("Empty", vec![]),
            (
                "Navigation 導航 with a deliberately long section heading",
                vec![
                    (
                        "↑/↓ 👍",
                        "move through the rows, a long action text 漢字 that wraps",
                    ),
                    ("Ctrl+Shift+Alt+Q", "Quit the application right away"),
                    ("Enter", "Open"),
                ],
            ),
            ("Editing", vec![("e", "edit the selected cell")]),
        ],
    )
}

fn sentinel_buffer(area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            buf[(x, y)].set_symbol(SENTINEL);
        }
    }
    buf
}

/// Renders into `buf` with a fresh hit registry and focus ring, as a normal frame does.
fn paint(help: &mut HelpOverlay, screen: Rect, buf: &mut Buffer) -> HitRegistry {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    help.render(screen, buf, &mut ctx);
    hits
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.is_empty()
        || (inner.x >= outer.x
            && inner.y >= outer.y
            && inner.right() <= outer.right()
            && inner.bottom() <= outer.bottom())
}

#[test]
fn help_narrow_and_empty_surfaces_are_bounded() {
    let canvas = Rect::new(0, 0, 100, 40);
    let widths = [0u16, 1, 4, 9, 10, 15, 16, 20, 80];
    let heights = [0u16, 1, 2, 5, 10, 24];
    for &w in &widths {
        for &h in &heights {
            let screen = Rect::new(5, 3, w, h);
            let mut help = overlay();
            let mut buf = sentinel_buffer(canvas);
            let hits = paint(&mut help, screen, &mut buf);

            for y in canvas.y..canvas.bottom() {
                for x in canvas.x..canvas.right() {
                    if !screen.contains((x, y).into()) {
                        assert_eq!(
                            buf[(x, y)].symbol(),
                            SENTINEL,
                            "cell ({x},{y}) outside {w}x{h} surface was written"
                        );
                    }
                }
            }
            for region in hits.regions() {
                assert!(
                    contains(screen, region.area),
                    "hit {:?} at {:?} escapes {w}x{h} surface",
                    region.id,
                    region.area
                );
            }
            assert!(
                contains(screen, help.area),
                "frame {:?} escapes {w}x{h} surface",
                help.area
            );
            assert!(
                contains(screen, help.done.area),
                "done {:?} escapes {w}x{h} surface",
                help.done.area
            );
        }
    }
}

#[test]
fn help_empty_frame_clears_old_geometry() {
    let canvas = Rect::new(0, 0, 80, 24);
    let mut help = overlay();

    let mut buf = sentinel_buffer(canvas);
    paint(&mut help, canvas, &mut buf);
    assert!(
        !help.done.area.is_empty(),
        "a normal frame places Done so the next check is meaningful"
    );

    let mut buf = sentinel_buffer(canvas);
    let hits = paint(&mut help, Rect::new(2, 2, 0, 0), &mut buf);
    assert!(
        help.area.is_empty(),
        "empty surface kept frame {:?}",
        help.area
    );
    assert!(
        help.done.area.is_empty(),
        "empty surface kept Done at {:?}",
        help.done.area
    );
    assert!(
        hits.regions().iter().all(|r| r.id != help.done.id),
        "empty surface registered Done"
    );
}

#[test]
fn help_resize_restores_scroll_and_close() {
    let canvas = Rect::new(0, 0, 80, 24);
    let mut help = overlay();

    let mut buf = sentinel_buffer(canvas);
    paint(&mut help, canvas, &mut buf);
    for _ in 0..3 {
        help.on_key(&key(KeyCode::Down));
    }

    let mut narrow = sentinel_buffer(Rect::new(0, 0, 10, 10));
    paint(&mut help, Rect::new(0, 0, 10, 10), &mut narrow);

    let mut buf = sentinel_buffer(canvas);
    paint(&mut help, canvas, &mut buf);
    assert!(
        help.scroll.v.offset <= help.scroll.v.max_offset(),
        "scroll offset {} outside content after restore",
        help.scroll.v.offset
    );
    assert!(!help.done.area.is_empty(), "Done not restored after resize");

    let done = help.done.id;
    help.on_click(done);
    assert!(
        help.closed,
        "clicking restored Done did not close the overlay"
    );

    let mut help = overlay();
    let mut buf = sentinel_buffer(canvas);
    paint(&mut help, canvas, &mut buf);
    help.on_key(&key(KeyCode::Esc));
    assert!(help.closed, "Esc did not close after a narrow render");
}

#[test]
fn help_percent_overflow_cannot_expand_surface() {
    let canvas = Rect::new(0, 0, 120, 60);
    for percent in [0u16, 80, 100, u16::MAX] {
        let screen = Rect::new(3, 2, 90, 40);
        let mut help = overlay();
        help.max_percent = percent;
        let mut buf = sentinel_buffer(canvas);
        paint(&mut help, screen, &mut buf);
        assert!(
            contains(screen, help.area),
            "max_percent {percent} placed frame {:?} outside {screen:?}",
            help.area
        );
    }
}
