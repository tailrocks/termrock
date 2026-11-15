use super::common::*;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::help::HelpOverlay;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

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
    let short = Rect::new(0, 0, 80, 10);
    let narrow = Rect::new(0, 0, 10, 10);
    let mut help = overlay();

    let mut buf = sentinel_buffer(short);
    paint(&mut help, short, &mut buf);
    assert!(
        help.scroll.v.max_offset() > 0,
        "content must overflow at {short:?} for the scroll check to mean anything"
    );
    for _ in 0..3 {
        help.on_key(&key(KeyCode::Down));
    }
    assert!(help.scroll.v.offset > 0, "Down did not scroll");

    let mut buf = sentinel_buffer(narrow);
    paint(&mut help, narrow, &mut buf);

    let mut buf = sentinel_buffer(short);
    let hits = paint(&mut help, short, &mut buf);
    assert!(
        help.scroll.v.offset <= help.scroll.v.max_offset(),
        "scroll offset {} outside content after restore",
        help.scroll.v.offset
    );
    assert!(!help.done.area.is_empty(), "Done not restored after resize");

    help.on_key(&key(KeyCode::Esc));
    assert!(help.closed, "Esc did not close the restored overlay");

    help.closed = false;
    let mut buf = sentinel_buffer(short);
    let hits_again = paint(&mut help, short, &mut buf);
    assert_eq!(
        hits.regions().len(),
        hits_again.regions().len(),
        "repaint changed the registered regions"
    );
    let done = help.done.area;
    let clicked = hits_again
        .resolve(Position::new(done.x + done.width / 2, done.y))
        .expect("Done is registered at its painted area");
    help.on_click(clicked.owner);
    assert!(
        help.closed,
        "clicking the registered Done region did not close the restored overlay"
    );
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

fn rendered_text(buf: &Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn help_zero_action_budget_keeps_binding_order() {
    let canvas = Rect::new(0, 0, 60, 24);
    let mut found_zero_budget = false;
    for w in 0..=60u16 {
        let mut help = HelpOverlay::new(
            ID,
            "Scope",
            vec![(
                "Keys",
                vec![("x1", "◆one"), ("x2", "◆two"), ("x3", "◆three")],
            )],
        );
        let mut buf = sentinel_buffer(canvas);
        paint(&mut help, Rect::new(0, 0, w, 24), &mut buf);
        let text = rendered_text(&buf);

        // No action glyph painted means the action budget was zero. Widths
        // too small for any body are skipped: nothing is shown to order.
        let shown: Vec<&str> = ["x1", "x2", "x3"]
            .into_iter()
            .filter(|k| text.contains(k))
            .collect();
        if text.contains('◆') || shown.is_empty() {
            continue;
        }
        assert_eq!(
            shown.len(),
            3,
            "a zero action budget dropped a binding at width {w}\n{text}"
        );
        found_zero_budget = true;
        let positions: Vec<usize> = shown
            .iter()
            .map(|k| {
                assert_eq!(
                    text.matches(k).count(),
                    1,
                    "binding {k} not shown exactly once at width {w}\n{text}"
                );
                text.find(k).unwrap()
            })
            .collect();
        assert!(
            positions.windows(2).all(|p| p[0] < p[1]),
            "bindings out of order at width {w}: {positions:?}\n{text}"
        );
    }
    assert!(
        found_zero_budget,
        "no width in 0..=60 produced a zero action budget, so the case never ran"
    );
}

#[test]
fn help_wide_narrow_wide_renders_identically() {
    let wide = Rect::new(0, 0, 100, 30);
    let mut help = overlay();

    let mut first = sentinel_buffer(wide);
    paint(&mut help, wide, &mut first);
    let first_frame = help.area;
    let first_done = help.done.area;

    let mut narrow = sentinel_buffer(wide);
    paint(&mut help, Rect::new(5, 3, 9, 4), &mut narrow);
    let mut empty = sentinel_buffer(wide);
    paint(&mut help, Rect::new(5, 3, 0, 0), &mut empty);

    let mut again = sentinel_buffer(wide);
    paint(&mut help, wide, &mut again);

    for y in wide.y..wide.bottom() {
        for x in wide.x..wide.right() {
            assert_eq!(
                again[(x, y)],
                first[(x, y)],
                "cell ({x},{y}) differs after narrow and empty renders"
            );
        }
    }
    assert_eq!(
        help.area, first_frame,
        "frame changed on the wide re-render"
    );
    assert_eq!(
        help.done.area, first_done,
        "Done changed on the wide re-render"
    );
}
