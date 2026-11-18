mod common;

use common::*;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::hit::{Axis, Bar};
use junie_tui::core::id::WidgetId;
use junie_tui::core::scroll::{HScroll, Scrollable, TrackClick};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::edit_menu::edit_menu_key;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::viewport::{CellPos, Line, Mark, Span, TextViewport, WorkCounters};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.viewport");
const AREA: Rect = Rect::new(0, 0, 40, 8);

fn lines() -> Vec<Line> {
    (0..60)
        .map(|n| vec![Span::plain(format!("row {n:02} the quick brown fox"))])
        .collect()
}

fn viewport() -> TextViewport {
    let mut v = TextViewport::with_lines(ID, lines());
    v.follow = true;
    v.set_area(AREA);
    v
}

fn paint(v: &dyn ViewportOps, caret_allowed: bool) -> (Buffer, Option<Position>) {
    let mut cursor = None;
    let (buf, _) = draw(None, |buf, ctx| {
        cursor = v.draw(AREA, buf, ctx, ctx.theme.canvas, caret_allowed);
    });
    (buf, cursor)
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..AREA.height).map(|y| row(buf, y)).collect()
}

#[test]
fn the_ops_scroll_exactly_like_the_text_viewport() {
    let mut direct = viewport();
    let mut via = viewport();
    let ops: &mut dyn ViewportOps = &mut via;

    assert!(ops.follow());
    assert_eq!(ops.id(), ID);
    for _ in 0..3 {
        let a = Scrollable::wheel(&mut direct, Axis::Vertical, -1).outcome();
        assert_eq!(ops.wheel(Axis::Vertical, -1), a);
    }
    assert!(!ops.follow() && !direct.follow);
    assert_eq!(ops.scroll().offset, direct.scroll.offset);
    assert_eq!(ops.scroll().visible_range(), direct.scroll.visible_range());
    assert_eq!(ops.scroll().content_len, 60);

    ops.page_up();
    direct.scroll.page_up();
    assert_eq!(ops.scroll().offset, direct.scroll.offset);
    assert!(!ops.follow(), "page up never returns to the live tail");

    ops.jump_start();
    direct.scroll.jump_start();
    assert_eq!(ops.scroll().offset, 0);
    assert_eq!(direct.scroll.offset, 0);

    for _ in 0..30 {
        ops.page_down();
        direct.scroll.page_down();
        direct.follow = direct.is_at_tail();
        assert_eq!(ops.scroll().offset, direct.scroll.offset);
        assert_eq!(ops.follow(), direct.follow);
    }
    assert!(
        ops.follow(),
        "paging down to the end follows the tail again"
    );

    ops.wheel(Axis::Vertical, -1);
    assert!(!ops.follow());
    ops.set_follow(true);
    assert!(ops.follow());
    assert_eq!(ops.scroll().offset, ops.scroll().max_offset());
}

#[test]
fn the_ops_select_and_copy_exactly_like_the_text_viewport() {
    let mut direct = viewport();
    let mut via = viewport();
    let ops: &mut dyn ViewportOps = &mut via;

    assert!(!ops.has_selection());
    assert_eq!(ops.selected_text(), None);
    let from = Position::new(4, 1);
    let to = Position::new(13, 1);
    direct.on_click(from);
    ops.on_click(from);
    assert_eq!(ops.on_drag(to), direct.on_drag(to));
    assert!(ops.has_selection() && direct.has_selection());
    assert_eq!(ops.selected_text(), direct.selected_text());
    assert_eq!(ops.selected_text().as_deref(), Some("53 the qu"));
    assert_eq!(ops.context_point(), direct.context_point());

    ops.clear_selection();
    assert!(!ops.has_selection());
    assert_eq!(ops.selected_text(), None);

    let at = Position::new(9, 2);
    assert_eq!(ops.select_word_at(at), direct.select_word_at(at));
    assert_eq!(ops.selected_text(), direct.selected_text());
    assert_eq!(ops.selected_text().as_deref(), Some("the"));
}

#[test]
fn the_ops_report_the_caret_and_the_bars_like_the_text_viewport() {
    let mut direct = viewport();
    direct.caret = Some(CellPos { line: 59, col: 4 });
    direct.caret_visible = true;
    let ops: &dyn ViewportOps = &direct;

    assert_eq!(ops.caret(), Some(CellPos { line: 59, col: 4 }));
    assert!(ops.caret_visible());
    assert_eq!(
        ops.screen_pos(CellPos { line: 59, col: 4 }),
        direct.screen_pos(CellPos { line: 59, col: 4 })
    );
    assert_eq!(
        ops.screen_pos(CellPos { line: 59, col: 4 }),
        Some(Position::new(4, 7))
    );
    assert_eq!(ops.hbar(), direct.hbar());
    assert!(
        ops.keyboard_copy(),
        "the text viewport copies with the y key"
    );
    assert!(
        !ops.draws_track(),
        "the host draws the scroll bar of a text viewport"
    );

    let mut resized = viewport();
    let ops: &mut dyn ViewportOps = &mut resized;
    let narrow = Rect::new(2, 1, 20, 4);
    ops.set_area(narrow);
    let mut expect = viewport();
    expect.set_area(narrow);
    assert_eq!(ops.hbar(), expect.hbar());
    assert_eq!(ops.scroll().visible_range(), expect.scroll.visible_range());
    assert_eq!(
        ops.hbar().height,
        1,
        "text wider than the area gets a bottom bar"
    );
    assert_eq!(
        ops.scroll().visible_range().len(),
        3,
        "the bar takes the fourth row"
    );
}

#[test]
fn draw_paints_the_viewport_and_leaves_it_untouched() {
    let mut direct = viewport();
    let (want, _) = draw(None, |buf, ctx| {
        let mut copy = direct.clone();
        copy.render(AREA, buf, ctx, ctx.theme.canvas);
    });
    let (got, cursor) = paint(&direct, true);
    assert_eq!(rows(&got), rows(&want));
    assert!(
        row(&got, 0).starts_with("row 52 the quick brown fox"),
        "{}",
        row(&got, 0)
    );
    assert!(
        row(&got, 7).starts_with("row 59 the quick brown fox"),
        "{}",
        row(&got, 7)
    );
    assert_eq!(cursor, None, "no caret, no hardware cursor");

    let before = (direct.scroll.offset, direct.follow, direct.revision());
    let again = paint(&direct, true);
    assert_eq!(rows(&again.0), rows(&got));
    assert_eq!(
        (direct.scroll.offset, direct.follow, direct.revision()),
        before
    );

    direct.set_follow(false);
    direct.scroll.jump_start();
    let (top, _) = paint(&direct, true);
    assert!(
        row(&top, 0).starts_with("row 00 the quick brown fox"),
        "{}",
        row(&top, 0)
    );
}

#[test]
fn draw_shows_the_hardware_cursor_only_when_the_caret_is_allowed_and_live() {
    let mut v = viewport();
    v.caret = Some(CellPos { line: 59, col: 4 });
    v.caret_visible = true;
    let (_, shown) = paint(&v, true);
    assert_eq!(shown, Some(Position::new(4, 7)));
    let (_, withheld) = paint(&v, false);
    assert_eq!(
        withheld, None,
        "a dialog, a drag or another focus withholds it"
    );

    v.caret_visible = false;
    assert_eq!(paint(&v, true).1, None, "the program hid the caret");

    v.caret_visible = true;
    v.set_follow(false);
    v.scroll.jump_start();
    assert_eq!(
        paint(&v, true).1,
        None,
        "a scrolled-back view has no live caret"
    );

    let mut short = TextViewport::with_lines(ID, lines().into_iter().take(5).collect());
    short.set_area(AREA);
    short.caret = Some(CellPos { line: 4, col: 2 });
    short.caret_visible = true;
    short.follow = true;
    assert_eq!(paint(&short, true).1, Some(Position::new(2, 4)));
    short.follow = false;
    assert_eq!(
        paint(&short, true).1,
        None,
        "off the live tail the caret is not where the program is typing"
    );
}

#[test]
fn a_text_viewport_drag_after_the_area_collapsed_is_ignored() {
    let mut v = viewport();
    v.on_press_with(
        Position::new(1, 0),
        1,
        ratatui::crossterm::event::KeyModifiers::NONE,
    );
    v.set_area(Rect::new(3, 2, 0, 0));
    assert_eq!(
        v.on_drag(Position::new(2, 2)),
        junie_tui::core::event::Outcome::Ignored
    );
    v.set_area(AREA);
    assert_eq!(
        v.on_drag(Position::new(4, 2)),
        junie_tui::core::event::Outcome::Ignored,
        "the press ended with the area: a later drag selects nothing"
    );
    assert!(!v.has_selection());
}

fn wide() -> TextViewport {
    let long = "x".repeat(120);
    let mut v = TextViewport::with_lines(ID, (0..20).map(|_| vec![Span::plain(&long)]).collect());
    v.set_area(AREA);
    v
}

#[test]
fn the_app_scroll_policy_of_a_drawn_frame_reaches_the_input_of_a_view_drawn_from_a_shared_reference()
 {
    let mut v = wide();
    paint(&v, false);
    assert_eq!(
        ViewportOps::wheel(&mut v, Axis::Horizontal, 3),
        junie_tui::core::event::Outcome::Changed,
        "the default policy scrolls sideways"
    );
    ViewportOps::wheel(&mut v, Axis::Horizontal, -9);

    draw(None, |buf, ctx| {
        ctx.hscroll = junie_tui::core::scroll::HScroll::off();
        v.draw(AREA, buf, ctx, ctx.theme.canvas, false);
    });

    assert_eq!(
        ViewportOps::wheel(&mut v, Axis::Horizontal, 3),
        junie_tui::core::event::Outcome::Ignored,
        "the policy of the frame turned sideways scrolling off for the input too"
    );
}

#[test]
fn paging_up_or_jumping_to_the_start_from_the_live_tail_stops_following() {
    for leave in [
        |ops: &mut dyn ViewportOps| ops.page_up(),
        |ops: &mut dyn ViewportOps| ops.jump_start(),
    ] {
        let mut v = viewport();
        let ops: &mut dyn ViewportOps = &mut v;
        assert!(ops.follow());
        assert!(ops.scroll().at_end(), "the view starts on the live tail");

        leave(ops);

        assert!(!ops.follow(), "a view that left the tail stops following");
        for _ in 0..8 {
            ops.page_down();
        }
        assert!(ops.follow(), "paging back to the tail follows it again");
    }
}

fn vertical_bar(v: &dyn ViewportOps) -> Bar {
    Bar {
        id: scrollbar::id_for(ID),
        owner: ID,
        axis: Axis::Vertical,
        track: v.vbar(),
    }
}

fn drawn_with(v: &TextViewport, click: TrackClick) {
    draw(None, |buf, ctx| {
        ctx.track_click = click;
        v.draw(AREA, buf, ctx, ctx.theme.canvas, false);
    });
}

#[test]
fn a_text_view_drawn_from_a_shared_reference_reports_the_track_along_its_area() {
    let mut v = viewport();
    paint(&v, false);
    let track = Rect::new(AREA.right() - 1, AREA.y, 1, AREA.height);
    assert_eq!(ViewportOps::vbar(&v), track);

    let wide = wide();
    paint(&wide, false);
    assert_eq!(
        ViewportOps::vbar(&wide),
        Rect::new(AREA.right() - 1, AREA.y, 1, AREA.height - 1),
        "the bottom row belongs to the horizontal bar"
    );

    let mut short = TextViewport::with_lines(ID, lines().into_iter().take(3).collect());
    short.set_area(AREA);
    paint(&short, false);
    assert_eq!(ViewportOps::vbar(&short), Rect::ZERO, "nothing to scroll");

    let bar = vertical_bar(&v);
    let before = v.scroll().offset;
    ViewportOps::bar_input(&mut v, &bar, Position::new(track.x, track.y), true);
    assert!(
        v.scroll().offset < before,
        "a press above the thumb of the host-drawn track pages up"
    );
    assert!(!v.follow());
}

#[test]
fn the_track_click_each_text_view_was_drawn_with_decides_its_own_press() {
    let mut first = viewport();
    let mut second = viewport();
    drawn_with(&first, TrackClick::Jump);
    drawn_with(&second, TrackClick::Page);

    let top = Position::new(AREA.right() - 1, AREA.y);
    let end = first.scroll().offset;
    let bar = vertical_bar(&first);
    ViewportOps::bar_input(&mut first, &bar, top, true);
    let bar = vertical_bar(&second);
    ViewportOps::bar_input(&mut second, &bar, top, true);

    assert_eq!(
        first.scroll().offset,
        0,
        "the first view was drawn with jump: a press at the top of the track lands at the start"
    );
    assert_eq!(
        second.scroll().offset,
        end - AREA.height as usize,
        "the second view was drawn with page: the same press moves one page"
    );
}

#[test]
fn the_horizontal_policy_each_text_view_was_drawn_with_decides_its_own_wheel() {
    let mut off = wide();
    let mut on = wide();
    draw(None, |buf, ctx| {
        ctx.hscroll = HScroll::off();
        off.draw(AREA, buf, ctx, ctx.theme.canvas, false);
    });
    paint(&on, false);

    assert_eq!(
        ViewportOps::wheel(&mut off, Axis::Horizontal, 3),
        junie_tui::core::event::Outcome::Ignored,
        "the first view was drawn with sideways scrolling off"
    );
    assert_eq!(
        ViewportOps::wheel(&mut on, Axis::Horizontal, 3),
        junie_tui::core::event::Outcome::Changed,
        "the view drawn after it kept the default policy"
    );
}

struct Frame {
    buf: Buffer,
    ctx_cursor: Option<Position>,
    hits: String,
    returned: Option<Position>,
}

fn frame(
    focus: Option<WidgetId>,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> Option<Position>,
) -> Frame {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    let returned = paint(&mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    let ctx_cursor = ctx.cursor;
    drop(ctx);
    Frame {
        buf,
        ctx_cursor,
        hits: format!("{:?} {:?}", hits.regions(), hits.bars()),
        returned,
    }
}

fn shared_frame(v: &TextViewport, area: Rect, focus: Option<WidgetId>, allowed: bool) -> Frame {
    frame(focus, |buf, ctx| {
        v.draw(area, buf, ctx, ctx.theme.canvas, allowed)
    })
}

fn mutable_frame(v: &TextViewport, focus: Option<WidgetId>, allowed: bool) -> Frame {
    let mut copy = v.clone();
    copy.caret_visible = v.caret_visible && allowed;
    let mut frame = frame(focus, |buf, ctx| {
        copy.render(AREA, buf, ctx, ctx.theme.canvas);
        None
    });
    frame.returned = (copy.follow && copy.caret_visible)
        .then(|| copy.caret.and_then(|at| copy.screen_pos(at)))
        .flatten();
    frame
}

fn assert_same(got: &Frame, want: &Frame, what: &str) {
    assert_eq!(got.buf, want.buf, "{what}: cells");
    assert_eq!(got.ctx_cursor, want.ctx_cursor, "{what}: ctx cursor");
    assert_eq!(got.hits, want.hits, "{what}: hit regions");
    assert_eq!(got.returned, want.returned, "{what}: hardware cursor");
}

fn work(v: &TextViewport) -> WorkCounters {
    v.work_counters()
}

fn draw_clones(v: &TextViewport) -> (usize, usize, usize) {
    let w = work(v);
    (w.draw_fallbacks, w.draw_cloned_lines, w.draw_cloned_bytes)
}

fn wide_lines(n: usize) -> Vec<Line> {
    (0..n)
        .map(|i| {
            vec![Span::plain(format!(
                "row {i:03} {}",
                "wide text ".repeat(9)
            ))]
        })
        .collect()
}

fn short_lines(n: usize) -> Vec<Line> {
    (0..n)
        .map(|i| vec![Span::plain(format!("row {i} of the history"))])
        .collect()
}

fn configured(
    wrap: bool,
    hoff: isize,
    selected: bool,
    marked: bool,
    menu: bool,
    follow: bool,
) -> TextViewport {
    let mut v = TextViewport::with_lines(ID, wide_lines(60)).wrap(wrap);
    v.caret = Some(CellPos { line: 59, col: 4 });
    v.caret_visible = true;
    v.follow = true;
    v.set_area(AREA);
    if !follow {
        v.set_follow(false);
        v.scroll.scroll_by(-20);
    }
    if hoff > 0 {
        v.scroll.h.scroll_by(hoff);
    }
    if selected {
        v.on_click(Position::new(4, 1));
        v.on_drag(Position::new(13, 3));
        assert!(v.has_selection(), "the matrix really selects");
    }
    if marked {
        let top = v.scroll.offset;
        v.set_marks(vec![
            Mark {
                line: top + 1,
                range: 4..7,
                current: true,
            },
            Mark {
                line: top + 2,
                range: 0..3,
                current: false,
            },
        ]);
    }
    if menu {
        let f10 = chord(
            ratatui::crossterm::event::KeyCode::F(10),
            ratatui::crossterm::event::KeyModifiers::SHIFT,
        );
        edit_menu_key(&mut v, &f10);
        assert!(v.edit_menu.is_open(), "the matrix really opens the menu");
    }
    if hoff > 0 && !wrap {
        assert!(v.scroll.h.offset > 0, "the matrix really scrolls sideways");
    }
    v
}

#[test]
fn prepared_draw_matches_mutable_render() {
    let mut checked = 0;
    for bits in 0..256u32 {
        let on = |n: u32| bits & (1 << n) != 0;
        let (wrap, h, selected, marked, menu, follow, allowed, focused) =
            (on(0), on(1), on(2), on(3), on(4), on(5), on(6), on(7));
        let v = configured(wrap, if h { 5 } else { 0 }, selected, marked, menu, follow);
        let focus = focused.then_some(ID);
        let before = draw_clones(&v);
        let got = shared_frame(&v, AREA, focus, allowed);
        assert_eq!(
            draw_clones(&v),
            before,
            "wrap={wrap} h={h} sel={selected} marks={marked} menu={menu} follow={follow}: a prepared view is drawn without a clone"
        );
        let want = mutable_frame(&v, focus, allowed);
        assert_same(
            &got,
            &want,
            &format!(
                "wrap={wrap} h={h} sel={selected} marks={marked} menu={menu} follow={follow} allowed={allowed} focused={focused}"
            ),
        );
        checked += 1;
    }
    assert_eq!(checked, 256);
}

#[test]
fn prepared_draw_preserves_model_but_notes_frame_policy() {
    let mut v = wide();
    v.caret = Some(CellPos { line: 19, col: 2 });
    v.caret_visible = true;
    let snapshot = |v: &TextViewport| {
        (
            v.revision(),
            v.follow,
            v.scroll.offset,
            v.scroll.h.offset,
            v.caret_visible,
            v.caret,
        )
    };
    let before = snapshot(&v);

    draw(None, |buf, ctx| {
        ctx.track_click = TrackClick::Jump;
        v.draw(AREA, buf, ctx, ctx.theme.canvas, false);
    });
    assert_eq!(snapshot(&v), before, "the draw edits no model state");
    assert_eq!(
        draw_clones(&v),
        (0, 0, 0),
        "a track-click policy is not a layout input"
    );

    let top = Position::new(AREA.right() - 1, AREA.y);
    let bar = vertical_bar(&v);
    ViewportOps::bar_input(&mut v, &bar, top, true);
    assert_eq!(
        v.scroll().offset,
        0,
        "the policy noted by the prepared draw reaches the next bar press"
    );

    draw(None, |buf, ctx| {
        ctx.hscroll = HScroll::off();
        v.draw(AREA, buf, ctx, ctx.theme.canvas, false);
    });
    assert_eq!(
        ViewportOps::wheel(&mut v, Axis::Horizontal, 3),
        junie_tui::core::event::Outcome::Ignored,
        "the horizontal policy noted by a draw reaches the next wheel"
    );
}

#[test]
fn prepared_draw_mismatch_uses_documented_fallback() {
    type Change = (&'static str, fn(&mut TextViewport), Rect);
    let moved = Rect::new(1, 0, 38, 7);
    let cases: [Change; 4] = [
        ("area", |_| {}, moved),
        ("wrap", |v| v.wrap = true, AREA),
        ("scrollbar gap", |v| v.scrollbar_gap = 4, AREA),
        (
            "content",
            |v| v.push(vec![Span::plain("an appended line")]),
            AREA,
        ),
    ];
    for (what, change, area) in cases {
        let mut v = wide();
        change(&mut v);
        let got = shared_frame(&v, area, None, true);
        let w = work(&v);
        assert_eq!(w.draw_fallbacks, 1, "{what}: one documented fallback");
        assert_eq!(w.draw_cloned_lines, v.len(), "{what}: every line copied");
        assert!(w.draw_cloned_bytes > 0, "{what}: source bytes are counted");

        let mut fresh = v.clone();
        let want = frame(None, |buf, ctx| {
            fresh.render(area, buf, ctx, ctx.theme.canvas);
            None
        });
        assert_eq!(got.buf, want.buf, "{what}: the fallback is not stale");
    }

    let v = wide();
    let got = frame(None, |buf, ctx| {
        ctx.hscroll = HScroll::off();
        v.draw(AREA, buf, ctx, ctx.theme.canvas, true)
    });
    assert_eq!(work(&v).draw_fallbacks, 1, "horizontal policy: fallback");
    let mut fresh = v.clone();
    let want = frame(None, |buf, ctx| {
        ctx.hscroll = HScroll::off();
        fresh.render(AREA, buf, ctx, ctx.theme.canvas);
        None
    });
    assert_eq!(got.buf, want.buf, "horizontal policy: not stale");
}

#[test]
fn prepared_draw_after_set_area_returns_to_fast_path() {
    let mut v = wide();
    v.wrap = true;
    v.scrollbar_gap = 3;
    shared_frame(&v, AREA, None, true);
    assert_eq!(draw_clones(&v).0, 1, "unprepared: one fallback");

    let moved = Rect::new(2, 1, 30, 6);
    v.set_area(moved);
    let after = draw_clones(&v);
    for _ in 0..4 {
        let got = shared_frame(&v, moved, None, true);
        let mut expect = v.clone();
        let want = frame(None, |buf, ctx| {
            expect.render(moved, buf, ctx, ctx.theme.canvas);
            None
        });
        assert_eq!(got.buf, want.buf);
    }
    assert_eq!(draw_clones(&v), after, "prepared frames copy nothing");

    v.push(vec![Span::plain("one more line")]);
    shared_frame(&v, moved, None, true);
    assert_eq!(draw_clones(&v).0, after.0 + 1, "an edit needs set_area");
    v.set_area(moved);
    let edited = draw_clones(&v);
    shared_frame(&v, moved, None, true);
    assert_eq!(draw_clones(&v), edited, "set_area prepares the new content");
}

#[test]
fn prepared_draw_context_policy_then_set_area_fast_path() {
    let mut v = wide();
    let off = |v: &TextViewport| {
        frame(None, |buf, ctx| {
            ctx.hscroll = HScroll::off();
            v.draw(AREA, buf, ctx, ctx.theme.canvas, true)
        })
    };
    off(&v);
    assert_eq!(draw_clones(&v).0, 1, "the new policy is a layout input");

    v.set_area(AREA);
    let prepared = draw_clones(&v);
    off(&v);
    off(&v);
    assert_eq!(
        draw_clones(&v),
        prepared,
        "set_area adopted the noted policy: the next draws take the fast path"
    );
}

#[test]
fn prepared_draw_does_not_copy_history() {
    for n in [1_000usize, 100_000] {
        let mut v = TextViewport::with_lines(ID, short_lines(n));
        v.follow = true;
        let area = Rect::new(0, 0, 80, 24);
        v.set_area(area);
        let setup = work(&v);
        let revision = v.revision();

        for _ in 0..5 {
            frame(None, |buf, ctx| {
                v.draw(area, buf, ctx, ctx.theme.canvas, true)
            });
        }

        let after = work(&v);
        assert_eq!(
            (
                after.draw_fallbacks,
                after.draw_cloned_lines,
                after.draw_cloned_bytes
            ),
            (0, 0, 0),
            "{n} lines: five unchanged frames copy no history"
        );
        assert_eq!(
            (
                after.segmented_lines,
                after.reflowed_lines,
                after.index_rebuilds
            ),
            (
                setup.segmented_lines,
                setup.reflowed_lines,
                setup.index_rebuilds
            ),
            "{n} lines: unchanged frames do no layout work"
        );
        assert_eq!(v.revision(), revision, "{n} lines: a frame is not an edit");

        v.wrap = true;
        frame(None, |buf, ctx| {
            v.draw(area, buf, ctx, ctx.theme.canvas, true)
        });
        assert_eq!(
            draw_clones(&v).1,
            n,
            "{n} lines: the unprepared fallback copies every line and says so"
        );
    }
}
