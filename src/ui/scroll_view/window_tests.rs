use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color, Style};
use ratatui::widgets::Padding;

use super::{Axes, ScrollView, VisibleWindow};
use crate::core::event::{Key, Outcome};
use crate::core::focus::FocusRing;
use crate::core::hit::{HitRegion, HitRegistry};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::widgets::scrollbar::id_for;
use crate::widgets::select::Select;

const VIEW: WidgetId = WidgetId::of("scroll_window.view");
const ROWS: WidgetId = WidgetId::of("scroll_window.rows");
const INNER: WidgetId = WidgetId::of("scroll_window.inner");
const ROW_WIDTH: u16 = 20;

struct Drawn {
    buf: Buffer,
    hits: HitRegistry,
    cursor: Option<Position>,
}

fn draw(screen: Rect, paint: impl FnOnce(&mut Buffer, &mut RenderCtx)) -> Drawn {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    let backdrop = Style::new().bg(Color::Rgb(9, 9, 9));
    buf.set_style(screen, backdrop);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    paint(&mut buf, &mut ctx);
    let cursor = ctx.cursor;
    Drawn { buf, hits, cursor }
}

fn row_rect(rect: Rect, index: usize) -> (PartRef, Rect) {
    (PartRef::item(Part::ROW, ItemKey::index(index)), rect)
}

fn paint_row(buf: &mut Buffer, ctx: &mut RenderCtx, rect: Rect, index: usize) {
    if rect.is_empty() {
        return;
    }
    let text = format!("row {index:05} text");
    buf.set_string(rect.x, rect.y, text, Style::new().fg(Color::White));
    let (part, rect) = row_rect(rect, index);
    ctx.hits.register_part(ROWS, part, rect);
}

fn legacy_rows(
    view: &mut ScrollView,
    area: Rect,
    rows: u16,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    let content = Size::new(ROW_WIDTH, rows);
    view.render(area, content, buf, ctx, |inner, buf, ctx| {
        for index in 0..usize::from(rows) {
            let rect = Rect::new(inner.x, inner.y + index as u16, ROW_WIDTH, 1);
            paint_row(buf, ctx, rect, index);
        }
    });
}

#[derive(Default)]
struct Visits {
    callbacks: usize,
    rows: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
fn window_rows(
    view: &mut ScrollView,
    screen: Rect,
    area: Rect,
    rows: u16,
    target: Option<Rect>,
    visits: &mut Visits,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) -> VisibleWindow {
    let content = Size::new(ROW_WIDTH, rows);
    view.render_window(
        screen,
        area,
        content,
        target,
        buf,
        ctx,
        |window, buf, ctx| {
            visits.callbacks += 1;
            for index in window.content_rows() {
                visits.rows.push(index);
                let rect = window.project_content_rect(Rect::new(0, index as u16, ROW_WIDTH, 1));
                paint_row(buf, ctx, rect, index);
            }
        },
    )
}

fn prime(view: &mut ScrollView, screen: Rect, area: Rect, rows: u16) {
    let mut visits = Visits::default();
    draw(screen, |buf, ctx| {
        window_rows(view, screen, area, rows, None, &mut visits, buf, ctx);
    });
}

fn regions(hits: &HitRegistry) -> Vec<HitRegion> {
    hits.regions().to_vec()
}

#[test]
fn scroll_window_scratch_cells_ignore_content_height() {
    let screen = Rect::new(0, 0, 120, 40);
    let mut legacy_cells = Vec::new();
    for rows in [100u16, 1000, 10000] {
        let mut view = ScrollView::new(VIEW).axes(Axes::VERTICAL);
        let mut visits = Visits::default();
        let mut window = None;
        let first = draw(screen, |buf, ctx| {
            window = Some(window_rows(
                &mut view,
                screen,
                screen,
                rows,
                None,
                &mut visits,
                buf,
                ctx,
            ));
        });
        let window = window.expect("rendered");
        let viewport_cells =
            usize::from(window.viewport.width) * usize::from(window.viewport.height);
        let work = view.last_work();
        assert_eq!(visits.callbacks, 1, "rows={rows}");
        assert!(visits.rows.len() <= usize::from(window.viewport.height));
        assert_eq!(work.allocated, viewport_cells, "rows={rows}");
        assert!(work.filled <= viewport_cells && work.copied <= viewport_cells);
        assert_eq!(work.copied, viewport_cells);
        drop(first);

        let mut again = Visits::default();
        draw(screen, |buf, ctx| {
            window_rows(&mut view, screen, screen, rows, None, &mut again, buf, ctx);
        });
        let work = view.last_work();
        assert_eq!(again.callbacks, 1);
        assert_eq!(
            again.rows, visits.rows,
            "unchanged redraw visits the same rows"
        );
        assert!(work.allocated <= viewport_cells && work.copied <= viewport_cells);

        let mut legacy = ScrollView::new(VIEW).axes(Axes::VERTICAL);
        draw(screen, |buf, ctx| {
            legacy_rows(&mut legacy, screen, rows, buf, ctx)
        });
        legacy_cells.push(legacy.last_work());
    }
    for (work, rows) in legacy_cells.iter().zip([100usize, 1000, 10000]) {
        assert!(
            work.filled >= rows * 118,
            "legacy fills the whole canvas: {work:?}"
        );
        assert!(work.allocated >= work.filled);
    }
    assert!(legacy_cells[2].allocated > 90 * legacy_cells[0].allocated / 10);
}

#[test]
fn scroll_window_matches_legacy_simple_rows() {
    let screen = Rect::new(0, 0, 70, 24);
    let cases = [
        (Rect::new(0, 0, 40, 10), Padding::ZERO, 0usize),
        (Rect::new(3, 2, 40, 10), Padding::ZERO, 7),
        (Rect::new(3, 2, 40, 10), Padding::ZERO, 90),
        (Rect::new(5, 4, 40, 10), Padding::new(3, 1, 2, 2), 0),
        (Rect::new(5, 4, 40, 10), Padding::new(3, 1, 2, 2), 1),
        (Rect::new(5, 4, 40, 10), Padding::new(3, 1, 2, 2), 33),
    ];
    for (area, padding, offset) in cases {
        let rows = 100;
        let mut old = ScrollView::new(VIEW)
            .axes(Axes::VERTICAL)
            .padding(padding)
            .follow_focus(false);
        draw(screen, |buf, ctx| {
            legacy_rows(&mut old, area, rows, buf, ctx)
        });
        old.scroll_to(0, offset);
        let legacy = draw(screen, |buf, ctx| {
            legacy_rows(&mut old, area, rows, buf, ctx)
        });

        let mut new = ScrollView::new(VIEW)
            .axes(Axes::VERTICAL)
            .padding(padding)
            .follow_focus(false);
        prime(&mut new, screen, area, rows);
        new.scroll_to(0, offset);
        let mut visits = Visits::default();
        let window = draw(screen, |buf, ctx| {
            window_rows(&mut new, screen, area, rows, None, &mut visits, buf, ctx);
        });
        let label = format!("area={area:?} padding={padding:?} offset={offset}");

        assert_eq!(legacy.buf, window.buf, "cells: {label}");
        assert_eq!(
            regions(&legacy.hits),
            regions(&window.hits),
            "hits: {label}"
        );
        assert_eq!(
            legacy.hits.bars(),
            window.hits.bars(),
            "visible bars: {label}"
        );
        assert_eq!(old.offset(), new.offset(), "{label}");
        assert_eq!(old.last_layout(), new.last_layout(), "{label}");
        assert_eq!(old.pane.v.content_len, new.pane.v.content_len);
        assert_eq!(old.pane.v.viewport_len, new.pane.v.viewport_len);
        assert_eq!(
            old.on_key(&Key {
                code: KeyCode::PageDown,
                mods: KeyModifiers::NONE
            }),
            new.on_key(&Key {
                code: KeyCode::PageDown,
                mods: KeyModifiers::NONE
            }),
            "{label}"
        );
        assert_eq!(old.offset(), new.offset(), "after PageDown: {label}");
        assert!(visits.rows.len() <= 10, "{label}");
        assert!(new.offset().1 > 0 || offset == 0, "{label}");
        let first = visits.rows.first().copied().unwrap_or(0);
        assert_eq!(
            window
                .hits
                .resolve(Position::new(area.x + padding.left, area.y + padding.top)),
            legacy
                .hits
                .resolve(Position::new(area.x + padding.left, area.y + padding.top)),
            "row key at the first visible content cell: {label} first={first}"
        );
    }
}

#[test]
fn scroll_window_padding_maps_source_row_zero() {
    let screen = Rect::new(0, 0, 40, 20);
    let area = Rect::new(4, 3, 30, 6);
    let padding = Padding::new(3, 1, 2, 2);
    let rows = 5u16;
    let mut view = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .padding(padding)
        .follow_focus(false);

    prime(&mut view, screen, area, rows);
    let mut seen = Vec::new();
    let mut zero = None;
    let mut window = None;
    for offset in [0usize, 1, 2, 3] {
        view.scroll_to(0, offset);
        let mut visits = Visits::default();
        draw(screen, |buf, ctx| {
            let w = window_rows(&mut view, screen, area, rows, None, &mut visits, buf, ctx);
            zero = Some(w.project_content_rect(Rect::new(0, 0, ROW_WIDTH, 1)));
            window = Some(w);
        });
        seen.push(visits.rows);
    }
    assert_eq!(
        seen[0],
        vec![0, 1, 2, 3],
        "top padding rows are not model rows"
    );
    assert_eq!(seen[1], vec![0, 1, 2, 3, 4]);
    assert_eq!(seen[2], vec![0, 1, 2, 3, 4]);
    assert_eq!(
        seen[3],
        vec![1, 2, 3, 4],
        "no fabricated rows past the model"
    );
    let window = window.expect("rendered");
    assert_eq!(window.content_origin, Position::new(3, 2));
    assert_eq!(
        zero,
        Some(Rect::new(0, 0, 0, 0)),
        "row zero scrolled away at offset 3"
    );

    view.scroll_to(0, 0);
    let mut visits = Visits::default();
    draw(screen, |buf, ctx| {
        let w = window_rows(&mut view, screen, area, rows, None, &mut visits, buf, ctx);
        assert_eq!(
            w.project_content_rect(Rect::new(0, 0, ROW_WIDTH, 1)),
            Rect::new(area.x + 3, area.y + 2, ROW_WIDTH, 1)
        );
    });

    let empty = ScrollView::new(VIEW).axes(Axes::VERTICAL).padding(padding);
    let mut empty = empty;
    let mut visits = Visits::default();
    draw(screen, |buf, ctx| {
        let w = window_rows(&mut empty, screen, area, 0, None, &mut visits, buf, ctx);
        assert_eq!(w.content_rows(), 0..0);
        assert_eq!(w.rows(), 0..6);
    });
    assert!(visits.rows.is_empty());
}

#[test]
fn scroll_window_focus_target_reveals_before_paint() {
    let screen = Rect::new(0, 0, 120, 40);
    let rows = 10_000u16;
    let mut view = ScrollView::new(VIEW).axes(Axes::VERTICAL);
    let target = Rect::new(0, 9000, ROW_WIDTH, 1);
    let mut visits = Visits::default();
    let mut window = None;
    let first = draw(screen, |buf, ctx| {
        let content = Size::new(ROW_WIDTH, rows);
        window = Some(view.render_window(
            screen,
            screen,
            content,
            Some(target),
            buf,
            ctx,
            |window, buf, ctx| {
                visits.callbacks += 1;
                for index in window.content_rows() {
                    visits.rows.push(index);
                    let rect =
                        window.project_content_rect(Rect::new(0, index as u16, ROW_WIDTH, 1));
                    paint_row(buf, ctx, rect, index);
                    if index == 9000 {
                        ctx.set_cursor(Position::new(rect.x, rect.y));
                    }
                }
            },
        ));
    });
    let window = window.expect("rendered");
    assert_eq!(
        window.logical.y, 8962,
        "target revealed with one row of context"
    );
    assert_eq!(visits.callbacks, 1);
    assert_eq!(visits.rows.first(), Some(&8962));
    assert_eq!(visits.rows.len(), 40, "only the visible rows were produced");
    assert_eq!(first.cursor, Some(Position::new(0, 38)));
    assert!(
        first
            .hits
            .resolve(Position::new(1, 38))
            .is_some_and(|hit| hit.part == PartRef::item(Part::ROW, ItemKey::index(9000)))
    );
    assert!(
        visits.rows.len() <= 40,
        "no hidden full draw to find the target"
    );

    view.scroll_to(0, 100);
    let mut again = Visits::default();
    draw(screen, |buf, ctx| {
        window_rows(
            &mut view,
            screen,
            screen,
            rows,
            Some(target),
            &mut again,
            buf,
            ctx,
        );
    });
    assert_eq!(
        view.offset(),
        (0, 100),
        "an unchanged target does not undo a user scroll"
    );

    let next = Rect::new(0, 9500, ROW_WIDTH, 1);
    draw(screen, |buf, ctx| {
        window_rows(
            &mut view,
            screen,
            screen,
            rows,
            Some(next),
            &mut again,
            buf,
            ctx,
        );
    });
    assert_eq!(view.offset(), (0, 9462), "a new target is followed");
}

fn open_select() -> Select {
    let mut select = Select::new(
        WidgetId::of("scroll_window.select"),
        "Region",
        &["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"],
        0,
    );
    let key = Key {
        code: KeyCode::Enter,
        mods: KeyModifiers::NONE,
    };
    select.on_key(&key);
    select
}

#[test]
fn scroll_window_popup_uses_full_screen_after_body_clip() {
    let screen = Rect::new(0, 0, 80, 24);
    let area = Rect::new(10, 5, 40, 8);
    let mut view = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    prime(&mut view, screen, area, 100);
    view.scroll_to(0, 4);
    let mut select = open_select();
    let bottom = area.bottom();
    let drawn = draw(screen, |buf, ctx| {
        let mut visits = Visits::default();
        let window = window_rows(&mut view, screen, area, 100, None, &mut visits, buf, ctx);
        assert_eq!(window.screen, screen);
        let anchor = window.project_rect(Rect::new(2, 8, 24, 2));
        assert_eq!(anchor.y, window.viewport.y + 4);
        select.render(anchor, buf, ctx, Color::Reset);
        ctx.flush_overlays(buf);
    });
    let below = Position::new(area.x + 7, bottom + 1);
    assert!(
        drawn.buf[below].symbol().trim() != "",
        "the popup list is painted below the clipped body"
    );
    let option = drawn.hits.hit(below);
    assert!(
        (0..6).any(|i| option == Some(select.option_id(i))),
        "the live controller owns the geometry outside the viewport: {option:?}"
    );
}

#[test]
fn scroll_window_nested_popup_preserves_root_screen() {
    let screen = Rect::new(0, 0, 80, 24);
    let outer_area = Rect::new(2, 2, 20, 8);
    let mut outer = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let mut inner = ScrollView::new(INNER)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let mut select = open_select();
    let mut roots = Vec::new();
    let drawn = draw(screen, |buf, ctx| {
        let mut anchor = Rect::ZERO;
        let outer_window = outer.render_window(
            screen,
            outer_area,
            Size::new(18, 40),
            None,
            buf,
            ctx,
            |window, buf, ctx| {
                roots.push((window.screen, buf.area));
                let area = window.project_content_rect(Rect::new(0, 1, 16, 4));
                let inner_window = inner.render_window(
                    window.screen,
                    area,
                    Size::new(16, 30),
                    None,
                    buf,
                    ctx,
                    |inner, buf, _| roots.push((inner.screen, buf.area)),
                );
                anchor = inner_window.project_rect(Rect::new(0, 0, 14, 2));
            },
        );
        assert_eq!(outer_window.screen, screen);
        select.render(anchor, buf, ctx, Color::Reset);
        ctx.flush_overlays(buf);
    });
    assert_eq!(roots.len(), 2);
    for (root, scratch) in roots {
        assert_eq!(
            root, screen,
            "every nested phase carries the real root screen"
        );
        assert!(
            scratch.width <= outer_area.width && scratch.height <= outer_area.height,
            "scratch is the viewport, not the root: {scratch:?}"
        );
    }
    let low = Position::new(outer_area.x + 3, outer_area.bottom() + 1);
    assert!(
        drawn.hits.hit(low).is_some(),
        "the popup extends below the 20x8 body"
    );
}

#[test]
fn scroll_window_nested_projection_is_applied_once() {
    let screen = Rect::new(0, 0, 80, 30);
    let mut outer = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let mut inner = ScrollView::new(INNER)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let render = |outer: &mut ScrollView, inner: &mut ScrollView| {
        draw(screen, |buf, ctx| {
            outer.render_window(
                screen,
                Rect::new(0, 0, 60, 20),
                Size::new(58, 100),
                None,
                buf,
                ctx,
                |window, buf, ctx| {
                    let area = window.project_content_rect(Rect::new(5, 40, 30, 10));
                    inner.render_window(
                        window.screen,
                        area,
                        Size::new(28, 50),
                        None,
                        buf,
                        ctx,
                        |rows, buf, ctx| {
                            for index in rows.content_rows() {
                                let rect =
                                    rows.project_content_rect(Rect::new(0, index as u16, 10, 1));
                                paint_row(buf, ctx, rect, index);
                                if index == 25 {
                                    ctx.set_cursor(Position::new(rect.x + 2, rect.y));
                                }
                            }
                        },
                    );
                },
            );
        })
    };
    render(&mut outer, &mut inner);
    outer.scroll_to(0, 35);
    render(&mut outer, &mut inner);
    inner.scroll_to(0, 20);
    let drawn = render(&mut outer, &mut inner);
    let hit = drawn.hits.resolve(Position::new(6, 5));
    assert_eq!(
        hit.map(|h| h.part),
        Some(PartRef::item(Part::ROW, ItemKey::index(20))),
        "inner row 20 sits at the inner viewport's first row, projected once"
    );
    assert_eq!(drawn.cursor, Some(Position::new(7, 10)));
    let bar = drawn.hits.bar(id_for(INNER)).expect("inner track");
    let full = inner.last_layout().v_track.expect("track");
    assert_eq!(bar.track, full);
    assert_eq!(drawn.hits.logical_bar(id_for(INNER)), Some(bar));
    assert_eq!(drawn.hits.shift_of(id_for(INNER)), (0, 0));
    assert_eq!(inner.pane.v.offset, 20);
    assert_eq!(outer.pane.v.offset, 35);
    let (start, len) = inner.pane.v.thumb(usize::from(full.height));
    assert!(start > 0 && start + len <= usize::from(full.height));
}

#[test]
fn scroll_window_clips_body_overpaint_hits_and_cursor() {
    let screen = Rect::new(0, 0, 30, 12);
    let area = Rect::new(4, 2, 12, 5);
    let mut view = ScrollView::new(VIEW).axes(Axes::VERTICAL);
    let drawn = draw(screen, |buf, ctx| {
        view.render_window(
            screen,
            area,
            Size::new(10, 40),
            None,
            buf,
            ctx,
            |window, scratch, ctx| {
                assert_eq!(scratch.area, window.viewport);
                ctx.hits.register(ROWS, Rect::new(0, 0, 30, 12));
                ctx.set_cursor(Position::new(0, 0));
                scratch.set_string(
                    window.viewport.x,
                    window.viewport.y,
                    "x".repeat(60),
                    Style::new(),
                );
            },
        );
    });
    let viewport = view.last_layout().viewport;
    for region in drawn.hits.regions() {
        assert!(
            viewport.contains(region.area.as_position())
                || region.area.is_empty()
                || region.id == id_for(VIEW),
            "{region:?} escaped the viewport"
        );
    }
    assert_eq!(
        drawn.cursor, None,
        "a cursor outside the viewport is dropped"
    );
    assert_eq!(drawn.buf[(0, 0)].symbol(), " ");
    assert_eq!(drawn.buf[(area.right() + 3, area.y)].symbol(), " ");
}

#[test]
fn scroll_window_empty_and_resize_are_bounded() {
    let screen = Rect::new(0, 0, 60, 20);
    let mut view = ScrollView::new(VIEW)
        .axes(Axes::BOTH)
        .padding(Padding::new(1, 1, 1, 1));

    for (area, content) in [
        (Rect::new(0, 0, 0, 0), Size::new(10, 10)),
        (Rect::new(0, 0, 1, 1), Size::new(10, 10)),
        (Rect::new(2, 2, 12, 5), Size::new(0, 0)),
        (Rect::new(2, 2, 12, 5), Size::new(100, 100)),
        (Rect::new(40, 10, 40, 40), Size::new(100, 100)),
    ] {
        let mut visits = Visits::default();
        let mut shown = None;
        draw(screen, |buf, ctx| {
            shown = Some(view.render_window(
                screen,
                area,
                content,
                Some(Rect::new(500, 500, 3, 3)),
                buf,
                ctx,
                |window, _, _| {
                    visits.callbacks += 1;
                    assert!(window.viewport.width as usize * window.viewport.height as usize > 0);
                },
            ));
        });
        let shown = shown.expect("rendered");
        let cells = usize::from(shown.viewport.width) * usize::from(shown.viewport.height);
        let work = view.last_work();
        assert!(work.allocated <= cells && work.filled <= cells && work.copied <= cells);
        assert!(shown.viewport.intersection(screen) == shown.viewport);
        assert_eq!(visits.callbacks, usize::from(cells > 0), "{area:?}");
    }

    let mut wide = ScrollView::new(VIEW).axes(Axes::BOTH);
    let first = draw(screen, |buf, ctx| {
        let mut visits = Visits::default();
        window_rows(
            &mut wide,
            screen,
            Rect::new(0, 0, 40, 12),
            100,
            None,
            &mut visits,
            buf,
            ctx,
        );
    });
    let tall = wide.last_layout();
    let second = draw(screen, |buf, ctx| {
        let mut visits = Visits::default();
        window_rows(
            &mut wide,
            screen,
            Rect::new(0, 0, 20, 6),
            100,
            None,
            &mut visits,
            buf,
            ctx,
        );
    });
    let small = wide.last_layout();
    assert!(small.viewport.height < tall.viewport.height);
    let area = Rect::new(0, 0, 20, 6);
    for region in second.hits.regions() {
        assert!(
            area.contains(region.area.as_position()),
            "stale geometry {region:?}"
        );
    }
    assert!(
        first
            .hits
            .regions()
            .iter()
            .any(|r| r.area.bottom() > area.bottom()),
        "the larger frame really had geometry outside the smaller one"
    );
    assert_eq!(
        wide.pane.v.viewport_len,
        usize::from(small.viewport.height),
        "pane tracks the resized viewport"
    );
}

#[test]
fn scroll_window_projection_helpers_map_at_nonzero_origins() {
    let window = VisibleWindow {
        logical: Rect::new(5, 30, 20, 10),
        viewport: Rect::new(7, 4, 20, 10),
        screen: Rect::new(0, 0, 80, 24),
        content_origin: Position::new(2, 3),
        content_rows: (3, 100),
    };
    assert_eq!(
        window.project_rect(Rect::new(0, 28, 30, 5)),
        Rect::new(7, 4, 20, 3),
        "clipped to the window then moved once"
    );
    assert_eq!(window.project_rect(Rect::new(0, 0, 4, 4)), Rect::ZERO);
    assert_eq!(
        window.project_point(Position::new(5, 30)),
        Some(Position::new(7, 4))
    );
    assert_eq!(window.project_point(Position::new(25, 30)), None);
    assert_eq!(
        window.project_content_rect(Rect::new(3, 28, 4, 3)),
        Rect::new(7, 5, 4, 3),
        "model rect (3, 28) is padded-canvas (5, 31)"
    );
    assert_eq!(window.rows(), 30..40);
    assert_eq!(window.content_rows(), 27..37);
}

#[test]
fn scroll_window_matches_the_legacy_hit_of_a_bar_press() {
    let screen = Rect::new(0, 0, 60, 20);
    let area = Rect::new(2, 2, 30, 8);
    let mut old = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let mut new = ScrollView::new(VIEW)
        .axes(Axes::VERTICAL)
        .follow_focus(false);
    let legacy = draw(screen, |buf, ctx| legacy_rows(&mut old, area, 60, buf, ctx));
    let mut visits = Visits::default();
    let window = draw(screen, |buf, ctx| {
        window_rows(&mut new, screen, area, 60, None, &mut visits, buf, ctx);
    });
    let track = old.last_layout().v_track.expect("track");
    let at = Position::new(track.x, track.y + 3);
    assert_eq!(legacy.hits.hit(at), window.hits.hit(at));
    assert_eq!(legacy.hits.hit(at), Some(id_for(VIEW)));
    assert_eq!(
        old.on_key(&Key {
            code: KeyCode::End,
            mods: KeyModifiers::NONE
        }),
        Some(Outcome::Changed)
    );
}
