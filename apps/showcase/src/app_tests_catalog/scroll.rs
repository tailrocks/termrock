//! Lane `scroll` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:scroll` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_scroll.rs`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect, Size};
use ratatui::widgets::Padding;
use ratatui::widgets::Widget as _;
use termrock::core::edit::clipboard;
use termrock::core::event::{Input, Key, Mouse, MouseKind, Outcome};
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::core::hit::{Axis, Bar};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::{Actions as _, KeyBinding, NavKeys};
use termrock::core::pointer::MouseTarget as _;
use termrock::core::pointer::PointerEvent;
use termrock::core::scroll::{
    FollowCursor, HScroll, ScrollPane, ScrollState, Scrollable, ScrollbarVisibility, TrackClick,
};
use termrock::terminal::{
    ForwardPolicy, Modes, Paste, RowWrap, SCROLLBACK_ROWS, TermCell, TermEvent, TermView,
    TerminalState, TerminalView, TerminalViewport, encode_key, is_spoofing_format,
};
use termrock::theme::Tone;
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::VisualState;
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::ui::reveal::{self, Reveal, RevealLine, RevealMode, RevealState};
use termrock::ui::scroll_view::{Axes, ScrollCmd, ScrollFrame, ScrollView, ViewLayout};
use termrock::widgets::field_common::PointerSelect;
use termrock::widgets::panel::{
    CARD_PAD, CardHeading, Panel, PanelCmd, PanelFocus, PanelKind, ScrollPanel, TITLE_MIN,
};
use termrock::widgets::scrollbar;
use termrock::widgets::viewport::{
    self, CellPos, Line, Mark, Span, TextViewport, ViewportAction, ViewportCmd, WorkCounters,
};
use termrock::widgets::viewport_ops::ViewportOps;

use super::{ScenarioCase, paint_hits, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::ScrollStateGeometry, run: scroll_state_geometry },
    ScenarioCase { id: ScenarioId::ScrollPaneAxes, run: scroll_pane_axes },
    ScenarioCase { id: ScenarioId::ScrollbarRendering, run: scrollbar_rendering },
    ScenarioCase { id: ScenarioId::ScrollViewHorizontal, run: scroll_view_horizontal },
    ScenarioCase { id: ScenarioId::ScrollViewLayout, run: scroll_view_layout },
    ScenarioCase { id: ScenarioId::ScrollViewFollow, run: scroll_view_follow },
    ScenarioCase { id: ScenarioId::ScrollViewCommands, run: scroll_view_commands },
    ScenarioCase { id: ScenarioId::RevealInline, run: reveal_inline },
    ScenarioCase { id: ScenarioId::RevealTooltip, run: reveal_tooltip },
    ScenarioCase { id: ScenarioId::RevealHelpers, run: reveal_helpers },
    ScenarioCase { id: ScenarioId::PanelChrome, run: panel_chrome },
    ScenarioCase { id: ScenarioId::ScrollPanelInput, run: scroll_panel_input },
    ScenarioCase { id: ScenarioId::TextViewportContent, run: text_viewport_content },
    ScenarioCase { id: ScenarioId::TextViewportInput, run: text_viewport_input },
    ScenarioCase { id: ScenarioId::TextViewportLayout, run: text_viewport_layout },
    ScenarioCase { id: ScenarioId::TerminalGridState, run: terminal_grid_state },
    ScenarioCase { id: ScenarioId::TerminalGridView, run: terminal_grid_view },
    ScenarioCase { id: ScenarioId::TerminalViewportPointer, run: terminal_viewport_pointer },
    ScenarioCase { id: ScenarioId::TerminalKeyEncoding, run: terminal_key_encoding },
];

fn scroll_state_geometry() {
    let mut s = ScrollState::new(100).with_context(2);
    assert_eq!(s.max_offset(), 100, "no viewport yet: everything is hidden");
    assert!(!s.overflows(), "a zero viewport never overflows");
    s.set_viewport(10);
    assert_eq!(s.max_offset(), 90);
    assert!(s.overflows() && !s.at_end());
    assert_eq!(s.visible_range(), 0..10);

    assert!(s.scroll_by(5));
    assert_eq!(s.offset, 5);
    assert!(s.scroll_by(-100), "overshoot clamps to the start");
    assert_eq!(s.offset, 0);
    assert!(!s.scroll_by(-1), "already at the start");
    assert!(s.scroll_to(200), "overshoot clamps to the end");
    assert!(s.at_end() && s.offset == 90);
    assert!(s.page_up() && s.offset == 80);
    assert!(s.page_down() && s.offset == 90);
    assert!(s.jump_start() && s.offset == 0);
    assert!(s.jump_end() && s.offset == 90);
    s.set_content(50);
    assert_eq!(s.offset, 40, "shrinking the content clamps the offset");
    s.offset = 99;
    s.clamp();
    assert_eq!(s.offset, 40);
    assert_eq!(s.visible_range(), 40..50);
    s.set_content(100);

    // Keeping a row visible: the context rows are kept around it unless the
    // pointer moved it, which wants the exact row.
    s.jump_start();
    assert!(s.ensure_visible(9));
    assert_eq!(s.offset, 2, "two context rows follow the row 9");
    s.pointer_move();
    assert!(s.ensure_visible(20));
    assert_eq!(s.offset, 11, "a pointer-chosen row needs no context");
    assert!(s.reveal_span(0, 3));
    assert_eq!(s.offset, 0);
    assert!(s.reveal_span(50, 20));
    assert_eq!(
        s.offset, 50,
        "a span taller than the viewport shows its start"
    );

    // Following the cursor: `OnMove` re-reveals only when the cursor or the
    // viewport changed, `Always` re-reveals every call.
    let mut on_move = ScrollState::new(100).with_follow_cursor(FollowCursor::OnMove);
    on_move.set_viewport(10);
    assert_eq!(on_move.follow_cursor, FollowCursor::OnMove);
    assert!(on_move.follow(95));
    assert_eq!(on_move.offset, 87);
    on_move.scroll_to(0);
    assert!(!on_move.follow(95), "the cursor did not move");
    assert_eq!(on_move.offset, 0);
    let mut always = ScrollState::new(100).with_follow_cursor(FollowCursor::Always);
    always.set_viewport(10);
    assert!(always.follow(95));
    always.scroll_to(0);
    assert!(always.follow(95), "Always reveals again");
    assert_eq!(always.offset, 87);

    // Thumb geometry and the track-to-offset maps.
    let mut t = ScrollState::new(100);
    t.set_viewport(10);
    assert_eq!(t.thumb(10), (0, 1));
    t.scroll_to(90);
    assert_eq!(t.thumb(10), (9, 1));
    t.scroll_to(45);
    assert_eq!(t.thumb(10), (5, 1));
    assert_eq!(t.offset_for_thumb_start(5, 10), 50);
    assert_eq!(t.offset_for_track_pos(5, 10), 50);
    assert_eq!(
        ScrollState::new(5).thumb(10),
        (0, 10),
        "no overflow, full thumb"
    );

    // Pressing and dragging the track.
    t.jump_start();
    assert!(!t.press_track(0, 10), "a press on the thumb only grabs it");
    assert!(t.press_track(5, 10), "a press off the thumb jumps there");
    assert_eq!(t.offset, 50);
    assert!(t.drag_track(9, 10));
    assert_eq!(t.offset, 90);
    t.release_track();

    // Page clicks: a click before the thumb pages up, and a held page click
    // does not turn into a drag. `Jump` moves the thumb to the pointer.
    assert!(t.press_track_with(2, 10, TrackClick::Page));
    assert_eq!(t.offset, 80);
    assert!(!t.drag_track(0, 10), "a paging press is not a drag");
    t.release_track();
    assert!(t.press_track_with(0, 10, TrackClick::Jump));
    assert_eq!(t.offset, 0);
    assert_eq!(TrackClick::default(), TrackClick::Page);

    // Scrollbar policy.
    let (mut fits, mut over) = (ScrollState::new(5), ScrollState::new(50));
    fits.set_viewport(10);
    over.set_viewport(10);
    assert!(!ScrollbarVisibility::Auto.shows(&fits) && ScrollbarVisibility::Auto.shows(&over));
    assert!(ScrollbarVisibility::Always.shows(&fits));
    assert!(!ScrollbarVisibility::Always.shows(&ScrollState::new(5)));
    assert!(!ScrollbarVisibility::Never.shows(&over));
    assert_eq!(ScrollbarVisibility::default(), ScrollbarVisibility::Auto);
}

fn scroll_pane_axes() {
    let owner = WidgetId::of("scenario.scrollpane");
    assert_eq!(HScroll::default().step(0).step, 1, "a zero step is raised");
    let off = HScroll::off();
    assert!(!off.enabled && off.bar == ScrollbarVisibility::Never);

    let mut pane = ScrollPane::new(100)
        .horizontal(
            HScroll::default()
                .step(3)
                .bar(ScrollbarVisibility::Always)
                .track_click(TrackClick::Jump),
        )
        .track_click(TrackClick::Jump)
        .columns(false);
    pane.set_viewport(11);
    pane.h.set_content(60);
    pane.h.set_viewport(19);
    assert!(pane.h_cfg.shows_bar(&pane.h));
    assert!(
        !off.shows_bar(&pane.h),
        "a disabled strip never shows a bar"
    );
    assert_eq!(pane.h_step(), 3);
    assert!(pane.h_scrolls());

    // Wheel: horizontal moves one step, vertical moves the given rows up to a page.
    assert_eq!(pane.wheel(Axis::Horizontal, 5).outcome(), Outcome::Changed);
    assert_eq!(pane.h.offset, 3);
    assert_eq!(pane.wheel(Axis::Vertical, 4).outcome(), Outcome::Changed);
    assert_eq!(pane.v.offset, 4);
    assert_eq!(pane.wheel(Axis::Vertical, 1000).outcome(), Outcome::Changed);
    assert_eq!(pane.v.offset, 15, "one wheel notch is at most a page");
    assert!(pane.scroll_by(-100) && pane.offset == 0);
    assert!(pane.scroll_to(10) && pane.v.offset == 10);
    assert!(pane.ensure_visible(50));
    assert!(pane.follow(95));
    pane.set_content(120);
    assert_eq!(pane.v.content_len, 120);
    let mut fixed = ScrollPane::new(10).horizontal(HScroll::off());
    assert_eq!(
        fixed.wheel(Axis::Horizontal, 1).outcome(),
        Outcome::Ignored,
        "a disabled strip ignores the wheel"
    );
    assert_eq!(
        fixed.wheel(Axis::Vertical, 1).outcome(),
        Outcome::Ignored,
        "a pane that fits ignores the wheel"
    );

    // The app policy and the per-pane override.
    let mut app = ScrollPane::new(10);
    app.note_policy(TrackClick::Jump, HScroll::default().step(9));
    assert_eq!(app.app_hscroll().step, 9);
    assert_eq!(app.v_click(), TrackClick::Jump);
    app.begin(TrackClick::Jump);
    assert_eq!(
        app.app_hscroll().step,
        9,
        "begin keeps the noted strip policy"
    );
    app.track_click = Some(TrackClick::Page);
    assert_eq!(app.v_click(), TrackClick::Page, "the pane override wins");

    // Column mode: one step is one column, and a column can be revealed.
    let mut cols = ScrollPane::new(0)
        .horizontal(HScroll::default())
        .columns(true);
    assert!(cols.columns && cols.h_step() == 1);
    assert_eq!(cols.layout_columns(vec![10, 10, 10, 10], 1, 25), 2);
    assert!(cols.reveal_column(3));
    assert_eq!(cols.h.offset, 2);
    assert_eq!(cols.layout_columns(vec![10, 10, 10, 10], 1, 25), 2);
    assert_eq!(cols.h.offset, 2, "the revealed column stays placed");

    // Tracks: draw the bars, then press, drag and release them.
    pane.begin(TrackClick::Page);
    let painted = paint_hits(Rect::new(0, 0, 20, 12), |_, buf, ctx| {
        pane.draw_v(Rect::new(19, 0, 1, 11), buf, ctx, owner, false);
        pane.draw_h(Rect::new(0, 11, 19, 1), buf, ctx, owner, false);
    });
    assert_eq!(pane.v_track(), Rect::new(19, 0, 1, 11));
    assert_eq!(pane.h_track(), Rect::new(0, 11, 19, 1));
    assert_eq!(pane.axis_of(scrollbar::id_for(owner)), Some(Axis::Vertical));
    assert_eq!(
        pane.axis_of(scrollbar::h_id_for(owner)),
        Some(Axis::Horizontal)
    );
    assert_eq!(pane.axis_of(WidgetId::of("scenario.elsewhere")), None);
    let bar = pane
        .bar_of(scrollbar::id_for(owner))
        .expect("the vertical bar is known");
    assert_eq!(bar.track, Rect::new(19, 0, 1, 11));
    assert!(pane.bar_of(WidgetId::of("scenario.elsewhere")).is_none());
    assert!(pane.held().is_none());

    pane.scroll_to(0);
    let before = pane.v.offset;
    assert_eq!(pane.press(Position::new(19, 9)).outcome(), Outcome::Changed);
    assert_eq!(pane.held(), Some(Axis::Vertical));
    assert!(
        pane.v.offset > before,
        "a jump click moves toward the pointer"
    );
    assert_eq!(pane.drag(Position::new(19, 2)).outcome(), Outcome::Changed);
    pane.release();
    assert!(pane.held().is_none());
    assert_eq!(
        pane.press(Position::new(3, 3)).outcome(),
        Outcome::Ignored,
        "a press off both tracks is ignored"
    );

    pane.h.jump_start();
    assert_eq!(
        pane.press_on(Axis::Horizontal, Position::new(12, 11))
            .outcome(),
        Outcome::Changed
    );
    assert_eq!(pane.held(), Some(Axis::Horizontal));
    assert_eq!(
        pane.drag_on(Axis::Horizontal, Position::new(18, 11))
            .outcome(),
        Outcome::Changed
    );
    pane.release();
    assert_eq!(
        pane.bar(&bar, Position::new(19, 1), true).outcome(),
        Outcome::Changed
    );
    assert_eq!(
        pane.bar(&bar, Position::new(19, 4), false).outcome(),
        Outcome::Changed
    );
    pane.release();

    pane.clear_track(Axis::Vertical);
    assert_eq!(pane.v_track(), Rect::ZERO);
    assert_eq!(
        pane.press_on(Axis::Vertical, Position::new(19, 4))
            .outcome(),
        Outcome::Ignored
    );
    pane.set_track(owner, Axis::Vertical, Rect::new(19, 0, 1, 11));
    assert_eq!(pane.v_track(), Rect::new(19, 0, 1, 11));
    assert!(pane.axis(Axis::Horizontal).scroll_to(5));
    assert_eq!(pane.h.offset, 5, "axis hands out the state of one axis");
    assert!(pane.axis(Axis::Vertical).scroll_to(0));
    drop(painted);
}

/// A `Scrollable` host is routed through the trait's provided methods.
struct Host {
    pane: ScrollPane,
    after: Vec<(Axis, bool)>,
}

impl Scrollable for Host {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.pane
    }

    fn after_scroll(&mut self, axis: Axis, moved: bool) -> bool {
        self.after.push((axis, moved));
        false
    }
}

fn scrollbar_rendering() {
    let owner = WidgetId::of("scenario.bars");
    let mut state = ScrollState::new(100);
    state.set_viewport(10);

    // Vertical bar: thumb at the top, track below, and a registered bar region.
    let painted = paint_hits(Rect::new(0, 0, 6, 12), |_, buf, ctx| {
        scrollbar::render_vertical(Rect::new(5, 0, 1, 10), buf, ctx, owner, &state, false);
    });
    assert_eq!(painted.buf[(5, 0)].symbol(), scrollbar::THUMB);
    assert_eq!(painted.buf[(5, 9)].symbol(), scrollbar::TRACK);
    assert!(scrollbar::is_bar_of(scrollbar::id_for(owner), owner));
    assert!(scrollbar::is_bar_of(scrollbar::h_id_for(owner), owner));
    assert!(!scrollbar::is_bar_of(WidgetId::of("scenario.other"), owner));

    // Pointer events on the bar drive the state through `scrollbar::pointer`.
    let bar_id = scrollbar::id_for(owner);
    let event = |phase: Phase, at: Position| {
        PointerEvent::new(&painted.hits, phase, bar_id, at, 1, KeyModifiers::NONE)
    };
    let out = scrollbar::pointer(
        &event(Phase::Press, Position::new(5, 6)),
        owner,
        &mut state,
        TrackClick::Jump,
    );
    assert_eq!(out, Some(Outcome::Changed));
    assert!(state.offset > 0);
    let out = scrollbar::pointer(
        &event(Phase::Drag, Position::new(5, 9)),
        owner,
        &mut state,
        TrackClick::Jump,
    );
    assert_eq!(out, Some(Outcome::Changed));
    assert_eq!(state.offset, 90, "dragging to the bottom reaches the end");
    let out = scrollbar::pointer(
        &event(Phase::Release, Position::new(5, 9)),
        owner,
        &mut state,
        TrackClick::Jump,
    );
    assert_eq!(out, Some(Outcome::Consumed));
    assert_eq!(
        scrollbar::pointer(
            &event(Phase::Press, Position::new(5, 6)),
            WidgetId::of("scenario.other"),
            &mut state,
            TrackClick::Jump,
        ),
        None,
        "another container's pointer event is not ours"
    );

    // The track helpers work on a bare track rectangle.
    let track = Rect::new(5, 0, 1, 10);
    state.jump_start();
    assert!(scrollbar::press(track, Position::new(5, 9), &mut state));
    assert!(state.offset > 0, "the default click pages down");
    state.release_track();
    state.jump_start();
    assert!(scrollbar::press_with(
        track,
        Position::new(5, 9),
        &mut state,
        TrackClick::Jump
    ));
    assert_eq!(state.offset, 90);
    assert!(scrollbar::drag(track, Position::new(5, 0), &mut state));
    assert_eq!(state.offset, 0);
    state.release_track();

    let h_track = Rect::new(0, 11, 5, 1);
    let mut h = ScrollState::new(50);
    h.set_viewport(5);
    assert!(scrollbar::press_h(
        h_track,
        Position::new(4, 11),
        &mut h,
        TrackClick::Jump
    ));
    assert_eq!(h.offset, 45);
    assert!(scrollbar::drag_h(h_track, Position::new(0, 11), &mut h));
    assert_eq!(h.offset, 0);
    h.release_track();

    // `apply` and `apply_with` pick the right axis from the bar.
    let v_bar = Bar {
        id: bar_id,
        owner,
        axis: Axis::Vertical,
        track,
    };
    let h_bar = Bar {
        id: scrollbar::h_id_for(owner),
        owner,
        axis: Axis::Horizontal,
        track: h_track,
    };
    state.jump_start();
    assert!(scrollbar::apply(
        &v_bar,
        Position::new(5, 9),
        true,
        &mut state
    ));
    state.release_track();
    state.jump_start();
    assert!(scrollbar::apply_with(
        &v_bar,
        Position::new(5, 9),
        true,
        &mut state,
        TrackClick::Jump
    ));
    assert!(scrollbar::apply(
        &v_bar,
        Position::new(5, 0),
        false,
        &mut state
    ));
    state.release_track();
    h.jump_start();
    assert!(scrollbar::apply_with(
        &h_bar,
        Position::new(4, 11),
        true,
        &mut h,
        TrackClick::Jump
    ));
    assert!(scrollbar::apply(
        &h_bar,
        Position::new(0, 11),
        false,
        &mut h
    ));
    h.release_track();

    // Horizontal bar: drawn when it overflows, or when asked to always show.
    let fits = ScrollState::new(3);
    let painted = paint_hits(Rect::new(0, 0, 10, 3), |_, buf, ctx| {
        scrollbar::render_horizontal(Rect::new(0, 0, 10, 1), buf, ctx, owner, &h, false);
        scrollbar::render_horizontal(Rect::new(0, 1, 10, 1), buf, ctx, owner, &fits, false);
        scrollbar::render_horizontal_with(
            Rect::new(0, 2, 10, 1),
            buf,
            ctx,
            owner,
            &ScrollState::new(3),
            false,
            true,
        );
    });
    assert_eq!(painted.buf[(0, 0)].symbol(), scrollbar::H_THUMB);
    assert_eq!(painted.buf[(9, 0)].symbol(), scrollbar::H_TRACK);
    assert_eq!(
        painted.buf[(0, 1)].symbol(),
        " ",
        "nothing to scroll, no bar"
    );
    assert_eq!(
        painted.buf[(0, 2)].symbol(),
        scrollbar::H_THUMB,
        "an always-on bar with nothing to scroll is one full thumb"
    );

    // Divider: a plain rule when nothing overflows, the bar when it does.
    let painted = paint_hits(Rect::new(0, 0, 4, 6), |_, buf, ctx| {
        scrollbar::render_divider(Rect::new(0, 0, 1, 5), buf, ctx, owner, &fits, false);
        scrollbar::render_divider(Rect::new(2, 0, 1, 5), buf, ctx, owner, &state, false);
    });
    assert_ne!(painted.buf[(0, 2)].symbol(), scrollbar::THUMB);
    assert_ne!(
        painted.buf[(0, 2)].symbol(),
        " ",
        "the divider rule is drawn"
    );
    assert!(
        (0..5).any(|y| painted.buf[(2, y)].symbol() == scrollbar::THUMB),
        "an overflowing divider is a scrollbar"
    );
    let mut pane = ScrollPane::from(state);
    let painted = paint_hits(Rect::new(0, 0, 4, 6), |_, buf, ctx| {
        pane.draw_divider(Rect::new(1, 0, 1, 5), buf, ctx, owner, false);
    });
    assert!(
        (0..5).any(|y| painted.buf[(1, y)].symbol() == scrollbar::THUMB),
        "draw_divider paints the pane's own bar"
    );
    assert_eq!(pane.v_track(), Rect::new(1, 0, 1, 5));

    // Fade: content loses a column to the bar, and the kept rows stay untouched.
    let mut faded = ScrollPane::new(50);
    faded.set_viewport(8);
    faded.scroll_to(10);
    let area = Rect::new(0, 0, 10, 8);
    let painted = paint_hits(Rect::new(0, 0, 10, 8), |_, buf, ctx| {
        for y in 0..8 {
            buf.set_string(0, y, "text text", ratatui::style::Style::default());
        }
        let content = faded.draw_with_fade(area, buf, ctx, owner, false, &[]);
        assert_eq!(content, Rect::new(0, 0, 9, 8), "the bar takes one column");
        let free = scrollbar::render_with_fade(area, buf, ctx, owner, &faded.v, false);
        assert_eq!(free, content);
        let kept = scrollbar::render_with_fade_except(area, buf, ctx, owner, &faded.v, false, &[0]);
        assert_eq!(kept, content);
    });
    assert!(text_of(&painted.buf).contains("text text"));
    let mut roomy = None;
    paint_hits(area, |area, buf, ctx| {
        roomy = Some(scrollbar::render_with_fade(
            area,
            buf,
            ctx,
            owner,
            &ScrollState::new(3),
            false,
        ));
    });
    assert_eq!(roomy, Some(area), "a pane that fits keeps its whole area");

    // Layout helpers.
    let bars = scrollbar::Bars::split(Rect::new(0, 0, 10, 6), true, true);
    assert_eq!(bars.content, Rect::new(0, 0, 9, 5));
    assert_eq!(bars.vertical, Some(Rect::new(9, 0, 1, 5)));
    assert_eq!(bars.horizontal, Some(Rect::new(0, 5, 9, 1)));
    let none = scrollbar::Bars::split(Rect::new(0, 0, 10, 6), false, false);
    assert!(none.vertical.is_none() && none.horizontal.is_none());
    assert_eq!(scrollbar::GAP, 1);
    assert_eq!(scrollbar::reserve(scrollbar::GAP), 2);
    let split = scrollbar::split(Rect::new(0, 0, 10, 4), true, scrollbar::GAP);
    assert_eq!(split.body, Rect::new(0, 0, 9, 4));
    assert_eq!(split.content, Rect::new(0, 0, 8, 4));
    assert_eq!(split.track, Some(Rect::new(9, 0, 1, 4)));
    let flat: scrollbar::VSplit = scrollbar::split(Rect::new(0, 0, 10, 4), false, 1);
    assert_eq!(flat.content, Rect::new(0, 0, 10, 4));
    assert!(flat.track.is_none());

    // The position label reads the visible rows.
    let mut label = ScrollState::new(100);
    label.set_viewport(10);
    label.scroll_to(40);
    assert_eq!(scrollbar::position_label(&label), "41–50 of 100");
    assert_eq!(scrollbar::position_label(&ScrollState::new(3)), "");

    // A Scrollable host is driven through the trait: wheel, bar press, drag, input.
    let mut host = Host {
        pane: ScrollPane::new(100),
        after: Vec::new(),
    };
    host.pane.set_viewport(10);
    host.pane.track_click = Some(TrackClick::Jump);
    let painted = paint_hits(Rect::new(0, 0, 6, 12), |_, buf, ctx| {
        host.pane
            .draw_v(Rect::new(5, 0, 1, 10), buf, ctx, owner, false);
    });
    drop(painted);
    assert_eq!(
        Scrollable::wheel(&mut host, Axis::Vertical, 3).outcome(),
        Outcome::Changed
    );
    assert_eq!(host.pane.v.offset, 3);
    assert_eq!(
        host.bar_press(Position::new(5, 9)).outcome(),
        Outcome::Changed
    );
    assert_eq!(
        host.bar_drag(Position::new(5, 0)).outcome(),
        Outcome::Changed
    );
    host.pane.release();
    let bar = host.pane.bar_of(bar_id).expect("the bar is known");
    assert_eq!(
        host.bar_input(&bar, Position::new(5, 9), true).outcome(),
        Outcome::Changed
    );
    assert!(
        host.after.iter().all(|(axis, _)| *axis == Axis::Vertical) && host.after.len() == 4,
        "every route settles through after_scroll: {:?}",
        host.after
    );
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn alt(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::ALT,
    }
}

/// One character per column, so a scrolled frame says which column is first.
fn ruler(inner: Rect, buf: &mut Buffer, _: &mut RenderCtx) {
    for y in inner.top()..inner.bottom() {
        for x in inner.left()..inner.right() {
            let digit = char::from_digit(u32::from(x) % 36, 36).expect("a base-36 digit");
            buf[(x, y)].set_char(digit);
        }
    }
}

fn first_row(buf: &Buffer, width: u16) -> String {
    (0..width)
        .map(|x| buf[(x, 0)].symbol().to_owned())
        .collect()
}

/// The horizontal-only view: Home resets the horizontal offset while the
/// content is scrolled right, and every pointer route reaches the strip.
fn scroll_view_horizontal() {
    let id = WidgetId::of("scenario.hview");
    let screen = Rect::new(0, 0, 20, 6);
    let content = Size::new(60, 4);
    let mut view = ScrollView::new(id)
        .axes(Axes::HORIZONTAL)
        .bars(ScrollbarVisibility::Auto)
        .padding(Padding::ZERO)
        .scrollbar_gap(1)
        .follow_focus(true)
        .context(1)
        .keys(ScrollCmd::default_keymap())
        .track_click(TrackClick::Jump);
    let layout: ViewLayout = view.layout(screen, content);
    assert!(layout.overflows());
    assert_eq!(layout.viewport, Rect::new(0, 0, 20, 5));
    assert!(layout.v_track.is_none(), "horizontal only: no vertical bar");
    assert_eq!(layout.h_track, Some(Rect::new(0, 5, 20, 1)));
    assert!(layout.corner.is_none());
    let draw = |view: &mut ScrollView| {
        paint_hits(screen, |area, buf, ctx| {
            view.render(area, content, buf, ctx, ruler)
        })
    };
    let painted = draw(&mut view);
    assert_eq!(view.last_layout(), layout);
    assert_eq!(first_row(&painted.buf, 20), "0123456789abcdefghij");
    assert_eq!(painted.buf[(0, 5)].symbol(), scrollbar::H_THUMB);
    assert_eq!(view.offset(), (0, 0));

    // Vertical commands do nothing here: the content fits vertically.
    assert_eq!(view.on_key(&key(KeyCode::End)), None);
    assert_eq!(view.on_key(&key(KeyCode::PageDown)), None);
    assert_eq!(
        view.on_key(&key(KeyCode::Char('x'))),
        None,
        "not a scroll key"
    );
    assert_eq!(
        view.on_key(&alt(KeyCode::PageUp)),
        Some(Outcome::Consumed),
        "already at the left edge"
    );

    // Page right, then Home resets the horizontal offset to zero.
    assert_eq!(view.on_key(&alt(KeyCode::PageDown)), Some(Outcome::Changed));
    assert_eq!(view.offset(), (20, 0));
    let painted = draw(&mut view);
    assert_eq!(first_row(&painted.buf, 4), "klmn", "column 20 is now first");
    assert_eq!(view.on_key(&key(KeyCode::Home)), Some(Outcome::Changed));
    assert_eq!(view.offset(), (0, 0), "Home resets the horizontal offset");
    let painted = draw(&mut view);
    assert_eq!(first_row(&painted.buf, 4), "0123");
    assert_eq!(view.on_key(&key(KeyCode::Home)), Some(Outcome::Consumed));
    view.on_key(&alt(KeyCode::PageDown));
    assert_eq!(view.on_key(&alt(KeyCode::PageUp)), Some(Outcome::Changed));
    assert_eq!(view.offset(), (0, 0));

    // The wheel moves one strip step, the bar jumps to the pointer.
    let wheel = PointerEvent::wheel_on(
        &painted.hits,
        id,
        Axis::Horizontal,
        1,
        Position::new(5, 2),
        KeyModifiers::NONE,
    );
    let out = view.pointer(&wheel).expect("the view owns the wheel");
    assert_eq!(out.outcome(), Outcome::Changed);
    assert_eq!(view.offset().0, 4);
    let bar = |phase: Phase, x: u16| {
        PointerEvent::new(
            &painted.hits,
            phase,
            scrollbar::h_id_for(id),
            Position::new(x, 5),
            1,
            KeyModifiers::NONE,
        )
    };
    let out = view.pointer(&bar(Phase::Press, 19)).expect("a bar press");
    assert_eq!(out.outcome(), Outcome::Changed);
    assert_eq!(view.offset().0, 40, "the track click jumps to the end");
    let out = view.pointer(&bar(Phase::Release, 19)).expect("a release");
    assert_eq!(out.outcome(), Outcome::Consumed);
    let elsewhere = PointerEvent::wheel_on(
        &painted.hits,
        WidgetId::of("scenario.elsewhere"),
        Axis::Horizontal,
        1,
        Position::new(5, 2),
        KeyModifiers::NONE,
    );
    assert!(view.pointer(&elsewhere).is_none(), "not this view's event");

    // Programmatic scrolling and reset.
    assert!(view.scroll_to(15, 99));
    assert_eq!(view.offset(), (15, 0), "the vertical axis cannot move");
    assert!(!view.scroll_to(15, 0));
    view.reset();
    assert_eq!(view.offset(), (0, 0));

    // The page-click policy: `Page` moves a page, not to the pointer.
    let mut paged = ScrollView::new(id)
        .axes(Axes::HORIZONTAL)
        .track_click(TrackClick::Page);
    let painted = paint_hits(screen, |area, buf, ctx| {
        paged.render(area, content, buf, ctx, ruler)
    });
    let press = PointerEvent::new(
        &painted.hits,
        Phase::Press,
        scrollbar::h_id_for(id),
        Position::new(19, 5),
        1,
        KeyModifiers::NONE,
    );
    let out = paged.pointer(&press).expect("a bar press");
    assert_eq!(out.outcome(), Outcome::Changed);
    assert_eq!(paged.offset().0, 20, "a page click moves one page");
}

/// Layouts of both axes, bar policies, padding, the gap and the legacy
/// begin/end frame.
fn scroll_view_layout() {
    let id = WidgetId::of("scenario.view.layout");
    let area = Rect::new(0, 0, 20, 8);
    let content = Size::new(40, 12);
    assert_eq!(Axes::default(), Axes::BOTH);
    assert_eq!((Axes::BOTH.vertical, Axes::BOTH.horizontal), (true, true));
    assert_eq!(
        (Axes::VERTICAL.vertical, Axes::VERTICAL.horizontal),
        (true, false)
    );
    assert_eq!(
        (Axes::HORIZONTAL.vertical, Axes::HORIZONTAL.horizontal),
        (false, true)
    );

    let mut view = ScrollView::new(id);
    let l: ViewLayout = view.layout(area, content);
    assert_eq!(l.viewport, Rect::new(0, 0, 18, 7));
    assert_eq!(l.content, Rect::new(0, 0, 40, 12));
    assert_eq!(l.v_track, Some(Rect::new(19, 0, 1, 7)));
    assert_eq!(l.h_track, Some(Rect::new(0, 7, 19, 1)));
    assert_eq!(l.corner, Some(Rect::new(19, 7, 1, 1)));
    assert!(l.overflows());

    let painted = paint_hits(area, |area, buf, ctx| {
        buf.set_string(19, 7, "X", ratatui::style::Style::default());
        let mut frame: ScrollFrame = view.begin(area, content, buf, ctx);
        assert_eq!(frame.layout(), l);
        let inner = frame.area();
        assert_eq!(inner, Rect::new(0, 0, 40, 12));
        ruler(inner, frame.buffer(buf), ctx);
        view.end(frame, buf, ctx);
    });
    assert_eq!(view.last_layout(), l);
    assert_eq!(first_row(&painted.buf, 6), "012345");
    assert_eq!(painted.buf[(19, 0)].symbol(), scrollbar::THUMB);
    assert_eq!(painted.buf[(0, 7)].symbol(), scrollbar::H_THUMB);
    assert_eq!(painted.buf[(19, 7)].symbol(), " ", "the corner is blanked");
    assert!(
        view.last_work().allocated > 0,
        "an overflowing frame uses scratch"
    );
    assert!(view.last_work().copied > 0);

    // Only one axis, no bars, always-on bars.
    let v = ScrollView::new(id)
        .axes(Axes::VERTICAL)
        .layout(area, content);
    assert!(v.v_track.is_some() && v.h_track.is_none());
    let never = ScrollView::new(id)
        .bars(ScrollbarVisibility::Never)
        .layout(area, content);
    assert!(never.v_track.is_none() && never.h_track.is_none());
    let always = ScrollView::new(id)
        .bars(ScrollbarVisibility::Always)
        .layout(area, Size::new(5, 2));
    assert!(always.v_track.is_some() && always.h_track.is_some());
    let fits = ScrollView::new(id).layout(area, Size::new(5, 2));
    assert!(!fits.overflows() && fits.v_track.is_none());

    // The gap between content and bar, and the padding around the content.
    let roomy = ScrollView::new(id).axes(Axes::VERTICAL);
    let tight = ScrollView::new(id).axes(Axes::VERTICAL).scrollbar_gap(0);
    assert_eq!(roomy.layout(area, content).viewport.width, 18);
    assert_eq!(tight.layout(area, content).viewport.width, 19);
    let padded = ScrollView::new(id)
        .padding(Padding::new(1, 2, 1, 2))
        .layout(area, Size::new(10, 3));
    assert_eq!(padded.inner, Rect::new(1, 1, 17, 5));
    assert!(!padded.overflows());

    // `fit` asks the measure again at the width left by the bar.
    let measure = |room: Size| Size::new(room.width.min(30), 12);
    let fitted = ScrollView::new(id)
        .axes(Axes::VERTICAL)
        .fit(Size::new(20, 6), measure);
    assert_eq!(
        fitted,
        Size::new(18, 12),
        "the bar costs a gap and a column"
    );
    let none = ScrollView::new(id)
        .axes(Axes::VERTICAL)
        .bars(ScrollbarVisibility::Never)
        .fit(Size::new(20, 6), measure);
    assert_eq!(none, Size::new(20, 12));

    // The non-overflowing begin path paints straight into the buffer.
    let mut small = ScrollView::new(id);
    let painted = paint_hits(area, |area, buf, ctx| {
        small.render(area, Size::new(5, 2), buf, ctx, ruler)
    });
    assert_eq!(first_row(&painted.buf, 5), "01234");
    assert_eq!(small.last_work().allocated, 0);
}

/// Following the focus target: `follow_focus` and `context` decide the offset.
fn scroll_view_follow() {
    let id = WidgetId::of("scenario.view.follow");
    let screen = Rect::new(0, 0, 20, 8);
    let content = Size::new(20, 100);
    let target = Some(Rect::new(0, 20, 1, 1));
    let offset_with = |view: &mut ScrollView| {
        paint_hits(screen, |area, buf, ctx| {
            view.render_window(screen, area, content, target, buf, ctx, |_, _, _| {});
        });
        view.offset().1
    };
    let mut default = ScrollView::new(id).axes(Axes::VERTICAL);
    assert_eq!(
        offset_with(&mut default),
        14,
        "one context row below the target"
    );
    let mut tight = ScrollView::new(id).axes(Axes::VERTICAL).context(0);
    assert_eq!(offset_with(&mut tight), 13);
    let mut wide = ScrollView::new(id).axes(Axes::VERTICAL).context(2);
    assert_eq!(offset_with(&mut wide), 15);
    let mut fixed = ScrollView::new(id).axes(Axes::VERTICAL).follow_focus(false);
    assert_eq!(
        offset_with(&mut fixed),
        0,
        "follow_focus(false) leaves it alone"
    );
    wide.reset();
    assert_eq!(wide.offset(), (0, 0));
}

const FOX: &str = "the quick brown fox jumps over the lazy dog and keeps running far away";

/// One frame with a chosen pointer state; overlays are flushed like a shell does.
fn paint_with(
    area: Rect,
    interaction: Interaction,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    draw(area, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    buf
}

fn reveal_inline() {
    let cfg = Reveal::OFF
        .hover(true)
        .keyboard(Some(KeyCode::Right))
        .mode(RevealMode::InlineWrap)
        .max_rows(3)
        .scroll(true)
        .wheel_lines(2)
        .delay_ms(0)
        .wheel_passes_at_ends(false);
    assert!(cfg.hover && cfg.scroll && !cfg.wheel_passes_at_ends);
    assert_eq!(cfg.keyboard, Some(KeyCode::Right));
    assert_eq!(cfg.mode, RevealMode::InlineWrap);
    assert_eq!((cfg.max_rows, cfg.wheel_lines, cfg.delay_ms), (3, 2, 0));
    assert!(!Reveal::OFF.hover && Reveal::OFF.keyboard.is_none());
    assert_eq!(RevealMode::default(), RevealMode::Tooltip);
    assert_eq!(Reveal::default().key_label(), Some("→"));
    for (code, label) in [
        (KeyCode::Left, "←"),
        (KeyCode::Up, "↑"),
        (KeyCode::Down, "↓"),
        (KeyCode::Enter, "Enter"),
        (KeyCode::Tab, "Tab"),
        (KeyCode::Char(' '), "Space"),
        (KeyCode::Char('?'), "?"),
        (KeyCode::F(1), "key"),
    ] {
        assert_eq!(Reveal::OFF.keyboard(Some(code)).key_label(), Some(label));
    }
    assert_eq!(Reveal::OFF.key_label(), None, "no key, no label");

    // A frame of the inline reveal for a cut row under the cursor.
    let id = WidgetId::of("scenario.reveal.inline");
    let frame = |state: &mut RevealState| {
        let mut lines: Option<Vec<RevealLine>> = None;
        paint_with(
            Rect::new(0, 0, 30, 12),
            Interaction::default(),
            |_, _, ctx| {
                state.begin(ctx);
                lines = state.inline(ctx, id, 10, FOX, true, 10);
            },
        );
        lines
    };
    for passes in [true, false] {
        let mut state = RevealState::new(Some(cfg.wheel_passes_at_ends(passes)));
        assert_eq!(state.effective().wheel_passes_at_ends, passes);
        assert!(!state.available() && state.hint().is_none() && !state.scrolls());
        assert_eq!(state.on_wheel(1), None, "nothing to scroll yet");
        assert!(frame(&mut state).is_none(), "closed: the row stays cut");
        assert!(state.available(), "the row is cut and under the cursor");
        assert_eq!(state.hint().map(|h| h.0), Some("→"));

        assert_eq!(state.on_key(&key(KeyCode::Right)), Some(Outcome::Changed));
        assert!(state.open);
        assert_eq!(state.hint().map(|h| h.0), Some("←"), "open: the key closes");
        let lines = frame(&mut state).expect("open: the wrapped lines");
        assert_eq!(lines.len(), 3, "max_rows caps the window");
        assert!(
            !lines[0].more && lines[2].more,
            "more rows below: {lines:?}"
        );
        assert!(state.scrolls());

        assert_eq!(
            state.on_key(&key(KeyCode::PageDown)),
            Some(Outcome::Changed)
        );
        let lines = frame(&mut state).expect("still open");
        assert!(lines[0].more, "rows hidden above after paging: {lines:?}");
        assert_eq!(state.on_wheel(100), Some(Outcome::Changed));
        let lines = frame(&mut state).expect("still open");
        assert!(lines[0].more && !lines[2].more, "at the end: {lines:?}");
        let at_end = state.on_wheel(1);
        assert_eq!(
            at_end,
            if passes {
                None
            } else {
                Some(Outcome::Consumed)
            },
            "the wheel at the end passes on only when asked"
        );
        assert_eq!(state.on_key(&key(KeyCode::PageUp)), Some(Outcome::Changed));
        assert_eq!(
            state.on_key(&Key {
                code: KeyCode::Right,
                mods: KeyModifiers::CONTROL
            }),
            None,
            "a chord is not the reveal key"
        );
        assert_eq!(state.on_key(&key(KeyCode::Left)), Some(Outcome::Changed));
        assert!(!state.open);
        assert_eq!(state.on_key(&key(KeyCode::PageDown)), None);
        state.on_key(&key(KeyCode::Right));
        state.close();
        assert!(!state.open);
    }

    // scroll(false): the open reveal is a fixed window.
    let mut fixed = RevealState::new(Some(cfg.scroll(false)));
    frame(&mut fixed);
    fixed.on_key(&key(KeyCode::Right));
    let lines = frame(&mut fixed).expect("open");
    assert_eq!(lines.len(), 3);
    assert!(!fixed.scrolls(), "scroll(false) never scrolls");
    assert_eq!(fixed.on_wheel(1), None);
    assert_eq!(fixed.on_key(&key(KeyCode::PageDown)), None);

    // No keyboard key: the state never opens, and `begin` closes it.
    let mut hover_only = RevealState::new(Some(Reveal::OFF.hover(true)));
    assert_eq!(hover_only.on_key(&key(KeyCode::Right)), None);
    hover_only.open = true;
    frame(&mut hover_only);
    assert!(!hover_only.open);
    assert!(!hover_only.available());

    // With no config the state follows the app policy of the context.
    let mut inherited = RevealState::new(None);
    frame(&mut inherited);
    assert_eq!(inherited.effective(), Reveal::default());
    assert!(inherited.config.is_none());
}

fn reveal_tooltip() {
    let id = WidgetId::of("scenario.reveal.tip");
    let area = Rect::new(2, 1, 8, 1);
    let screen = Rect::new(0, 0, 40, 8);
    let mut state = RevealState::new(Some(Reveal::default()));
    let mut hint = None;
    let mut peeked = None;
    let mut cut = false;
    let buf = paint_with(screen, Interaction::default(), |_, buf, ctx| {
        state.begin(ctx);
        cut = state.cell(ctx, id, area, FOX, true);
        buf.set_string(
            2,
            1,
            reveal::cut(area, FOX),
            ratatui::style::Style::default(),
        );
        hint = ctx.reveal_hint;
        peeked = ctx.peek.clone();
    });
    assert!(cut, "the text is wider than its cell");
    assert_eq!(
        hint.map(|h| h.0),
        Some("→"),
        "the cut row advertises its key"
    );
    assert_eq!(
        peeked.as_deref(),
        Some(FOX),
        "the status line can peek at it"
    );
    assert!(!text_of(&buf).contains("lazy dog"), "closed: no tooltip");

    assert_eq!(state.on_key(&key(KeyCode::Right)), Some(Outcome::Changed));
    let buf = paint_with(screen, Interaction::default(), |_, buf, ctx| {
        state.begin(ctx);
        state.cell(ctx, id, area, FOX, true);
        buf.set_string(
            2,
            1,
            reveal::cut(area, FOX),
            ratatui::style::Style::default(),
        );
    });
    assert!(
        text_of(&buf).contains("lazy dog and keeps running far away"),
        "open: the full text is shown in a tooltip:\n{}",
        text_of(&buf)
    );

    // A cell that fits has nothing to reveal.
    let mut fits = RevealState::new(Some(Reveal::default()));
    paint_with(screen, Interaction::default(), |_, _, ctx| {
        fits.begin(ctx);
        assert!(!fits.cell(ctx, id, Rect::new(2, 1, 20, 1), "short", true));
    });
    assert!(!fits.available());
}

fn reveal_helpers() {
    let area = Rect::new(2, 1, 8, 1);
    let screen = Rect::new(0, 0, 40, 8);
    let id = reveal::cut_id(area);
    assert_ne!(id, reveal::cut_id(Rect::new(3, 1, 8, 1)), "one id per cell");
    let hovered = Interaction {
        hover: Some(id),
        ..Interaction::default()
    };
    let draw_cut = |interaction: Interaction, cfg: Reveal| {
        paint_with(screen, interaction, |_, buf, ctx| {
            ctx.reveal = cfg;
            let shown = reveal::cut(area, FOX);
            assert_eq!(shown, "the qui…");
            buf.set_string(2, 1, shown, ratatui::style::Style::default());
        })
    };

    // Hover shows the full text, off does not, and a pointer that has not
    // rested long enough asks for a later frame.
    let tip = draw_cut(hovered, Reveal::default());
    assert!(text_of(&tip).contains("lazy dog and keeps running far away"));
    let off = draw_cut(hovered, Reveal::OFF);
    assert!(!text_of(&off).contains("lazy dog"));
    let early = Interaction {
        hover_ms: Some(100),
        ..hovered
    };
    let waiting = draw_cut(early, Reveal::default());
    assert!(!text_of(&waiting).contains("lazy dog"));
    let (_, wait_ms) = reveal::hover_wait().expect("a frame is waiting on the delay");
    assert_eq!(wait_ms, 200, "300 ms delay, 100 ms rested");
    let rested = Interaction {
        hover_ms: Some(300),
        ..hovered
    };
    assert!(text_of(&draw_cut(rested, Reveal::default())).contains("lazy dog"));
    assert!(
        text_of(&draw_cut(hovered, Reveal::OFF.hover(true))).contains("lazy dog"),
        "hover(true) turns the pointer tooltip on"
    );
    assert!(
        !text_of(&draw_cut(hovered, Reveal::default().hover(false))).contains("lazy dog"),
        "hover(false) turns it off"
    );
    assert!(
        text_of(&draw_cut(early, Reveal::default().delay_ms(0))).contains("lazy dog"),
        "a zero delay needs no rest"
    );
    assert!(
        !text_of(&draw_cut(early, Reveal::default().delay_ms(900))).contains("lazy dog"),
        "a long delay waits longer"
    );
    paint_with(screen, early, |_, _, ctx| {
        assert!(reveal::hover_ready(ctx, 50));
        assert!(!reveal::hover_ready(ctx, 400));
    });
    paint_with(screen, Interaction::default(), |_, _, ctx| {
        assert!(reveal::hover_ready(ctx, 400), "no rest time means ready");
    });

    // `truncated` with an explicit config, and `queue_cut` by hand.
    let widget = WidgetId::of("scenario.reveal.widget");
    let on_widget = Interaction {
        hover: Some(widget),
        ..Interaction::default()
    };
    let buf = paint_with(screen, on_widget, |_, _, ctx| {
        let cfg = Some(Reveal::default().delay_ms(0));
        assert!(!reveal::truncated(ctx, cfg, widget, area, "short"));
        assert!(reveal::truncated(ctx, cfg, widget, area, FOX));
    });
    assert!(text_of(&buf).contains("lazy dog"));
    let buf = paint_with(screen, hovered, |_, _, _| {
        reveal::queue_cut(area, "short");
        reveal::queue_cut(area, FOX);
    });
    assert!(
        text_of(&buf).contains("lazy dog"),
        "a queued cut is revealed"
    );

    // peek, fit and clamp feed the status line.
    paint_with(screen, Interaction::default(), |_, _, ctx| {
        reveal::peek(ctx, "one\ntwo\tthree");
        assert_eq!(ctx.peek.as_deref(), Some("one two three"));
        reveal::peek(ctx, "ignored");
        assert_eq!(ctx.peek.as_deref(), Some("one two three"), "the first wins");
    });
    paint_with(screen, Interaction::default(), |_, _, ctx| {
        assert_eq!(reveal::fit(ctx, area, FOX, false), "the qui…");
        assert!(ctx.peek.is_none(), "unfocused rows do not peek");
        assert_eq!(reveal::fit(ctx, area, "tiny", true), "tiny");
        assert!(ctx.peek.is_none(), "a row that fits has nothing to peek");
        assert_eq!(reveal::fit(ctx, area, FOX, true), "the qui…");
        assert_eq!(ctx.peek.as_deref(), Some(FOX));
    });
    paint_with(screen, Interaction::default(), |_, _, ctx| {
        let two = reveal::clamp(ctx, Rect::new(0, 0, 10, 2), FOX, true);
        assert_eq!(two.len(), 2);
        assert!(
            two[1].ends_with('…'),
            "the last row is cut with an ellipsis"
        );
        assert_eq!(ctx.peek.as_deref(), Some(FOX));
        let all = reveal::clamp(ctx, Rect::new(0, 0, 40, 4), "short text", false);
        assert_eq!(all, ["short text"]);
        assert!(reveal::clamp(ctx, Rect::new(0, 0, 0, 2), FOX, false).is_empty());
    });

    // The hint a cut row advertises is inserted after the first hint, once.
    let mut hints = vec![("Enter", "Open"), ("Esc", "Close")];
    let reveal_hint = Some(("→", "Show all"));
    reveal::advertise(&mut hints, reveal_hint, |h| h.0, |k, l| (k, l));
    assert_eq!(
        hints,
        [("Enter", "Open"), ("→", "Show all"), ("Esc", "Close")]
    );
    reveal::advertise(&mut hints, reveal_hint, |h| h.0, |k, l| (k, l));
    assert_eq!(hints.len(), 3, "never twice");
    reveal::advertise(&mut hints, None, |h| h.0, |k, l| (k, l));
    assert_eq!(hints.len(), 3, "nothing to advertise");

    // The wheel scrolls a pointer tooltip that is taller than its room.
    let tall = Reveal::default().max_rows(3).wheel_lines(2);
    let long = [FOX, FOX, FOX].join(" ");
    let buf = paint_with(screen, hovered, |_, buf, ctx| {
        ctx.reveal = tall;
        buf.set_string(
            2,
            1,
            reveal::cut(area, &long),
            ratatui::style::Style::default(),
        );
    });
    let rows = super::rows(&buf);
    let y = rows
        .iter()
        .position(|r| r.contains("the quick"))
        .unwrap_or_else(|| panic!("the tooltip:\n{}", text_of(&buf)));
    let x = rows[y].find("the quick").expect("its text") as u16;
    let wheel = |kind: MouseKind, at: Position| {
        reveal::on_pointer_wheel(&Input::Mouse(Mouse::new(kind, at)))
    };
    let inside = Position::new(x, y as u16);
    assert_eq!(wheel(MouseKind::WheelDown, inside), Some(Outcome::Changed));
    assert_eq!(
        wheel(MouseKind::WheelDown, Position::new(39, 7)),
        None,
        "outside the tip"
    );
    assert_eq!(wheel(MouseKind::Move, inside), None, "not a wheel");
    assert_eq!(reveal::on_pointer_wheel(&Input::Tick), None);
    let mut moved = 1;
    while wheel(MouseKind::WheelDown, inside) == Some(Outcome::Changed) {
        moved += 1;
        assert!(moved < 20, "the wheel reaches the end");
    }
    // The wheel step is the configured number of lines: fewer lines per
    // notch need more notches to reach the end.
    let notches = |lines: u16| {
        let cfg = Reveal::default().max_rows(3).wheel_lines(lines);
        let buf = paint_with(screen, hovered, |_, buf, ctx| {
            ctx.reveal = cfg;
            buf.set_string(
                2,
                1,
                reveal::cut(area, &long),
                ratatui::style::Style::default(),
            );
        });
        let at = super::rows(&buf)
            .iter()
            .position(|r| r.contains("the quick"))
            .expect("the tooltip") as u16;
        while wheel(MouseKind::WheelUp, Position::new(x, at)) == Some(Outcome::Changed) {}
        let mut count = 0;
        while wheel(MouseKind::WheelDown, Position::new(x, at)) == Some(Outcome::Changed) {
            count += 1;
            assert!(count < 20, "the wheel reaches the end");
        }
        count
    };
    assert!(notches(1) > notches(3), "wheel_lines sets the step");
    assert_eq!(
        moved,
        notches(2),
        "the tooltip above used two lines a notch"
    );
    assert_eq!(
        wheel(MouseKind::WheelDown, inside),
        None,
        "at the end it passes through"
    );
    assert_eq!(
        reveal::on_pointer_wheel(&Input::Mouse(
            Mouse::new(MouseKind::WheelUp, inside).with_mods(KeyModifiers::SHIFT)
        )),
        None,
        "a chord is not a scroll"
    );
}

/// Panel chrome: both kinds, the heading styles, focus marks, meta and sizing.
fn panel_chrome() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    assert_eq!(CARD_PAD, 2);
    assert_eq!(TITLE_MIN, 12);
    assert_eq!(
        CardHeading::ALL.map(CardHeading::label),
        ["plain", "rule", "band", "emphasis"]
    );
    let area = Rect::new(0, 0, 40, 8);
    let render = |panel: &Panel, area: Rect| {
        let mut buf = Buffer::empty(area);
        let inner = panel.render(area, &mut buf, &theme);
        (buf, inner)
    };
    let row0 = |buf: &Buffer| first_row(buf, buf.area.width);

    let card = Panel::card(Some("Details"));
    assert_eq!(card.kind, PanelKind::Card);
    assert_eq!((card.title_min, card.pad_x), (TITLE_MIN, CARD_PAD));
    assert_eq!(card.bg(&theme), theme.surface);
    let (buf, inner) = render(&card, area);
    assert_eq!(inner, Rect::new(2, 2, 36, 5), "title row, then the content");
    assert!(row0(&buf).contains("Details"));
    assert_eq!(render(&Panel::card(None), area).1, Rect::new(2, 1, 36, 6));
    assert_eq!(
        render(&Panel::card(Some("Details")).pad_x(1), area).1,
        Rect::new(1, 2, 38, 5),
        "pad_x moves the content edge"
    );

    let framed = Panel::framed(Some("Explorer"));
    assert_eq!(framed.kind, PanelKind::Framed);
    assert_eq!(framed.bg(&theme), theme.canvas);
    let (fbuf, finner) = render(&framed, area);
    assert_eq!(finner, Rect::new(3, 1, 35, 6));
    assert_eq!(fbuf[(0, 0)].symbol(), theme.glyphs.border.top_left);
    assert!(row0(&fbuf).contains(" Explorer "));
    let mut custom = Panel::card(Some("Details"));
    custom.bg_override = Some(ratatui::style::Color::Rgb(1, 2, 3));
    assert_eq!(custom.bg(&theme), ratatui::style::Color::Rgb(1, 2, 3));

    // Heading styles: a rule, a band, bold, or plain. A frame has no heading.
    let heading =
        |style: CardHeading| render(&Panel::card(Some("Details")).heading_style(style), area).0;
    let rule = heading(CardHeading::Rule);
    assert_eq!(rule[(2, 1)].symbol(), theme.glyphs.h_divider);
    let plain = heading(CardHeading::Plain);
    assert_ne!(plain[(2, 1)].symbol(), theme.glyphs.h_divider);
    let band = heading(CardHeading::Band);
    assert_ne!(
        band[(0, 0)].bg,
        band[(0, 4)].bg,
        "the band is lighter than the body"
    );
    assert_eq!(plain[(0, 0)].bg, plain[(0, 4)].bg);
    let emphasis = heading(CardHeading::Emphasis);
    assert!(
        emphasis[(2, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert!(
        !plain[(2, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_eq!(
        render(
            &Panel::framed(Some("Explorer")).heading_style(CardHeading::Band),
            area
        )
        .0,
        fbuf,
        "a frame ignores the heading style"
    );

    // Focus: the title is brighter, and the bar mark adds the gutter.
    let state = VisualState {
        focused: true,
        ..VisualState::default()
    };
    let (marked, _) = render(
        &Panel::card(Some("Details"))
            .focused(true)
            .focus_mark(PanelFocus::Bar),
        area,
    );
    assert_eq!(marked[(1, 0)].symbol(), theme.gutter_symbol(state));
    let (titled, _) = render(
        &Panel::card(Some("Details"))
            .focused(true)
            .focus_mark(PanelFocus::Title),
        area,
    );
    assert_ne!(titled[(1, 0)].symbol(), theme.gutter_symbol(state));
    assert_ne!(
        titled[(2, 0)].fg,
        buf[(2, 0)].fg,
        "a focused title is brighter"
    );
    let (focused_frame, _) = render(&Panel::framed(Some("Explorer")).focused(true), area);
    assert_ne!(
        focused_frame[(0, 1)].fg,
        fbuf[(0, 1)].fg,
        "a focused frame is lit"
    );

    // Meta: full, short, and the room the title keeps.
    let narrow = Rect::new(0, 0, 20, 6);
    let meta = Panel::card(Some("Details"))
        .meta("3 of 12 · filtered")
        .meta_short("3/12");
    assert!(row0(&render(&meta, area).0).contains("3 of 12 · filtered"));
    let short = row0(&render(&meta, narrow).0);
    assert!(
        short.contains("3/12") && !short.contains("filtered"),
        "{short}"
    );
    let long_title = || Panel::card(Some("Details of the thing")).meta("abcdefgh");
    assert!(!row0(&render(&long_title(), narrow).0).contains("abcdefgh"));
    assert!(row0(&render(&long_title().title_min(4), narrow).0).contains("abcdefgh"));
    let (mut late, inner) = render(&card, area);
    card.draw_meta(area, &mut late, &theme, "late meta");
    assert!(row0(&late).contains("late meta"), "{}", row0(&late));
    card.mark_clipped(inner, &mut late, &theme);
    assert_eq!(
        late[(inner.right() - 1, inner.bottom() - 1)].symbol(),
        theme.glyphs.ellipsis
    );

    // Sizing: outer adds the chrome, inner takes it away.
    let content = Size::new(10, 3);
    assert_eq!(Panel::card(Some("Hi")).outer(content), Size::new(14, 6));
    assert_eq!(Panel::card(None).outer(content), Size::new(14, 5));
    assert_eq!(Panel::card(Some("Hi")).inner(Size::new(14, 6)), content);
    assert_eq!(Panel::framed(Some("Hi")).outer(content), Size::new(15, 5));
    assert_eq!(Panel::framed(Some("Hi")).inner(Size::new(15, 5)), content);

    // try_render reports a panel with no room for content as None.
    let mut small = Buffer::empty(Rect::new(0, 0, 4, 2));
    assert_eq!(
        card.try_render(Rect::new(0, 0, 4, 2), &mut small, &theme),
        None
    );
    let mut ok = Buffer::empty(area);
    assert_eq!(
        card.try_render(area, &mut ok, &theme),
        Some(Rect::new(2, 2, 36, 5))
    );
}

fn plain_line(theme: &Theme, _: &str) -> ratatui::style::Style {
    theme.secondary()
}

/// Everything a reader does to a log panel: keys, tail following, wrapping,
/// the horizontal strip, selection with the pointer, copy, and the bars.
fn scroll_panel_input() {
    let id = WidgetId::of("scenario.panel.input");
    let area = Rect::new(0, 0, 30, 8);
    let lines = |n: usize| {
        (0..n)
            .map(|i| format!("line {i:02} 0123456789"))
            .collect::<Vec<_>>()
    };
    let keymap = || {
        vec![
            (KeyBinding::char('g'), PanelCmd::Follow),
            (KeyBinding::char('y'), PanelCmd::Copy),
            (KeyBinding::char('a'), PanelCmd::SelectAll),
        ]
    };
    let draw = |panel: &mut ScrollPanel| {
        paint_hits(area, |area, buf, ctx| {
            panel.render(area, buf, ctx, ctx.theme.canvas, plain_line)
        })
    };

    let mut panel = ScrollPanel::new(id, lines(30))
        .nav(NavKeys::DESKTOP.vim(true))
        .keymap(keymap())
        .context_menu(true)
        .pointer_select(PointerSelect::DEFAULT)
        .horizontal(HScroll::default().step(2))
        .track_click(TrackClick::Jump);
    assert!(panel.nav.vim && panel.edit_menu.enabled);
    assert_eq!(panel.pointer_select, PointerSelect::DEFAULT);
    assert_eq!(panel.horizontal.map(|h| h.step), Some(2));
    assert_eq!(panel.scroll.track_click, Some(TrackClick::Jump));
    draw(&mut panel);
    assert_eq!(panel.scroll.viewport_len, 8);
    panel.measure(Rect::new(0, 0, 30, 4));
    assert_eq!(panel.scroll.viewport_len, 4, "measure sizes the viewport");
    panel.measure(area);

    // Keys: arrows, vim, pages and the ends.
    assert_eq!(panel.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(panel.scroll.v.offset, 1);
    assert_eq!(panel.on_key(&key(KeyCode::Up)), Outcome::Changed);
    assert_eq!(panel.on_key(&key(KeyCode::Char('j'))), Outcome::Changed);
    assert_eq!(panel.scroll.v.offset, 1, "vim j moves down");
    assert_eq!(panel.on_key(&key(KeyCode::Char('k'))), Outcome::Changed);
    assert_eq!(panel.on_key(&key(KeyCode::PageDown)), Outcome::Changed);
    assert_eq!(panel.scroll.v.offset, 8);
    assert_eq!(panel.on_key(&key(KeyCode::End)), Outcome::Changed);
    assert_eq!(panel.scroll.v.offset, 22);
    assert_eq!(panel.on_key(&key(KeyCode::Home)), Outcome::Changed);
    assert_eq!(panel.on_key(&key(KeyCode::Home)), Outcome::Consumed);
    assert_eq!(panel.on_key(&key(KeyCode::Char('x'))), Outcome::Ignored);
    assert_eq!(
        panel.on_key(&key(KeyCode::Right)),
        Outcome::Ignored,
        "nothing to scroll sideways"
    );
    let off = ScrollPanel::new(id, lines(30)).nav(NavKeys::DESKTOP.page(false).home_end(false));
    let mut off = off;
    draw(&mut off);
    assert_eq!(off.on_key(&key(KeyCode::PageDown)), Outcome::Ignored);
    assert_eq!(off.on_key(&key(KeyCode::End)), Outcome::Ignored);

    // Tail following: the bound key toggles it, scrolling away drops it.
    let mut tail = ScrollPanel::new(id, lines(30)).tail(true).keymap(keymap());
    assert!(tail.tail && tail.follow);
    draw(&mut tail);
    assert_eq!(tail.scroll.v.offset, 22, "the tail follows new lines");
    assert_eq!(
        tail.on_key(&key(KeyCode::Char('f'))),
        Outcome::Ignored,
        "f is unbound"
    );
    assert_eq!(tail.on_key(&key(KeyCode::Up)), Outcome::Changed);
    assert!(!tail.follow);
    assert_eq!(tail.on_key(&key(KeyCode::Char('g'))), Outcome::Changed);
    assert!(
        tail.follow && tail.scroll.v.offset == 22,
        "following jumps to the end"
    );
    assert_eq!(tail.on_key(&key(KeyCode::Char('g'))), Outcome::Changed);
    assert!(!tail.follow);
    tail.push("line 30 added".to_owned());
    draw(&mut tail);
    assert_eq!(
        tail.scroll.v.offset, 22,
        "not following: new lines wait below"
    );

    // Selection through the keymap: select all, copy, escape.
    assert_eq!(tail.selection(), None);
    assert_eq!(
        tail.on_key(&key(KeyCode::Char('y'))),
        Outcome::Ignored,
        "nothing selected"
    );
    assert_eq!(tail.on_key(&key(KeyCode::Char('a'))), Outcome::Changed);
    assert_eq!(tail.select_all(), Outcome::Consumed, "already everything");
    draw(&mut tail);
    assert!(tail.selection().is_some());
    let all = tail.lines().join("\n");
    assert_eq!(tail.selected_text().as_deref(), Some(all.as_str()));
    assert_eq!(tail.on_key(&key(KeyCode::Char('y'))), Outcome::Consumed);
    assert_eq!(clipboard::text().as_deref(), Some(all.as_str()));
    assert_eq!(tail.on_key(&key(KeyCode::Esc)), Outcome::Changed);
    assert_eq!(tail.selected_text(), None);
    assert_eq!(tail.clear_selection(), Outcome::Ignored);
    let mut empty = ScrollPanel::new(id, Vec::new());
    assert_eq!(
        empty.select_all(),
        Outcome::Consumed,
        "no lines, nothing to select"
    );

    // Wrapping: long lines become several rows and the strip is off.
    let mut wrapped = ScrollPanel::new(id, vec!["x".repeat(50)]).wrap(true);
    assert!(wrapped.wrap);
    paint_hits(Rect::new(0, 0, 20, 4), |area, buf, ctx| {
        wrapped.render(area, buf, ctx, ctx.theme.canvas, plain_line)
    });
    assert_eq!(
        wrapped.scroll.v.content_len, 3,
        "50 cells wrap into 3 rows of 18"
    );
    assert_eq!(wrapped.on_key(&key(KeyCode::Right)), Outcome::Ignored);

    // Horizontal strip: a wide line scrolls by the configured step.
    let ruler_line: String = (0..80u32)
        .map(|i| char::from_digit(i % 36, 36).expect("a base-36 digit"))
        .collect();
    let mut wide =
        ScrollPanel::new(id, vec![ruler_line.clone()]).horizontal(HScroll::default().step(2));
    let painted = draw(&mut wide);
    assert_eq!(first_row(&painted.buf, 4), "0123");
    assert_eq!(
        painted.buf[(0, 7)].symbol(),
        scrollbar::H_THUMB,
        "the strip is drawn"
    );
    assert_eq!(wide.on_key(&key(KeyCode::Right)), Outcome::Changed);
    assert_eq!(wide.scroll.h.offset, 2);
    let painted = draw(&mut wide);
    assert_eq!(
        first_row(&painted.buf, 4),
        "…345",
        "the cut left edge is marked"
    );
    assert_eq!(wide.on_key(&key(KeyCode::Left)), Outcome::Changed);
    assert_eq!(wide.scroll.h.offset, 0);

    // Pointer selection: drag, word, line, and the policy that switches it off.
    let painted = draw(&mut panel);
    let event = |hits: &termrock::core::hit::HitRegistry, phase: Phase, at: Position, count: u8| {
        PointerEvent::new(hits, phase, id, at, count, KeyModifiers::NONE)
    };
    let _ = panel.pointer(&event(&painted.hits, Phase::Press, Position::new(0, 0), 1));
    let _ = panel.pointer(&event(&painted.hits, Phase::Drag, Position::new(6, 0), 1));
    assert_eq!(panel.selected_text().as_deref(), Some("line 0"));
    panel.clear_selection();
    let _ = panel.pointer(&event(&painted.hits, Phase::Press, Position::new(2, 1), 2));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("line"),
        "a double press selects a word"
    );
    panel.clear_selection();
    let _ = panel.pointer(&event(&painted.hits, Phase::Press, Position::new(2, 1), 3));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("line 01 0123456789"),
        "a triple press selects the line"
    );
    let mut fixed = ScrollPanel::new(id, lines(30)).pointer_select(PointerSelect::OFF);
    let painted = draw(&mut fixed);
    let _ = fixed.pointer(&event(&painted.hits, Phase::Press, Position::new(0, 0), 1));
    let _ = fixed.pointer(&event(&painted.hits, Phase::Drag, Position::new(6, 0), 1));
    assert_eq!(
        fixed.selected_text(),
        None,
        "OFF: the pointer selects nothing"
    );

    // The context menu opens on a secondary press only when enabled.
    let painted = draw(&mut panel);
    let _ = panel.pointer(&event(
        &painted.hits,
        Phase::Secondary,
        Position::new(3, 2),
        1,
    ));
    assert!(panel.edit_menu.is_open());
    let mut quiet = ScrollPanel::new(id, lines(30)).context_menu(false);
    let painted = draw(&mut quiet);
    let _ = quiet.pointer(&event(
        &painted.hits,
        Phase::Secondary,
        Position::new(3, 2),
        1,
    ));
    assert!(!quiet.edit_menu.is_open());

    // The bars: wheel, press, drag and the generic bar route.
    let mut bars = ScrollPanel::new(id, lines(30)).track_click(TrackClick::Jump);
    draw(&mut bars);
    assert_eq!(
        Scrollable::wheel(&mut bars, Axis::Vertical, 3).outcome(),
        Outcome::Changed
    );
    assert_eq!(bars.scroll.v.offset, 3);
    assert_eq!(
        bars.bar_press(Position::new(29, 7)).outcome(),
        Outcome::Changed
    );
    assert_eq!(
        bars.bar_drag(Position::new(29, 0)).outcome(),
        Outcome::Changed
    );
    bars.scroll.release();
    let bar = bars
        .scroll
        .bar_of(scrollbar::id_for(id))
        .expect("the panel's bar is known");
    assert_eq!(
        bars.bar_input(&bar, Position::new(29, 7), true).outcome(),
        Outcome::Changed
    );
    assert_eq!(
        Scrollable::wheel(&mut bars, Axis::Horizontal, 1).outcome(),
        Outcome::Ignored,
        "nothing to scroll sideways"
    );
}

fn numbered(count: usize) -> Vec<Line> {
    (0..count)
        .map(|n| vec![Span::plain(format!("row {n:02} the quick brown fox"))])
        .collect()
}

/// Spans: styles, painting and wrapping, then the line store of a viewport.
fn text_viewport_content() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let styled = Span::plain("x").bold().italic().underline().reversed();
    assert!(styled.bold && styled.italic && styled.underline && styled.reversed);
    assert_eq!(Span::muted("m").tone, Tone::Muted);
    assert_eq!(Span::new("n", Tone::Normal), Span::plain("n"));
    assert!(!Span::plain("p").bold);

    let line: Line = vec![Span::plain("ab").bold(), Span::muted("cd")];
    assert_eq!(viewport::line_text(&line), "abcd");
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    let width = viewport::render_spans(&mut buf, 0, 0, 10, &theme, &line, theme.canvas);
    assert_eq!(width, 4);
    assert_eq!(first_row(&buf, 5), "abcd ");
    assert!(
        buf[(0, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert!(
        !buf[(2, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_ne!(buf[(0, 0)].fg, buf[(2, 0)].fg, "muted text is dimmer");
    let mut cut = Buffer::empty(Rect::new(0, 0, 10, 1));
    let width = viewport::render_spans(&mut cut, 0, 0, 3, &theme, &line, theme.canvas);
    assert_eq!(width, 3, "painting stops at the room given");
    assert_eq!(first_row(&cut, 4), "ab… ", "and marks the cut");
    let wrapped = viewport::wrap_spans(&[Span::plain("hello "), Span::plain("world").bold()], 5);
    assert_eq!(wrapped.len(), 2);
    assert!(viewport::line_text(&wrapped[0]).starts_with("hello"));
    assert_eq!(viewport::line_text(&wrapped[1]), "world");
    assert!(wrapped[1][0].bold, "a wrapped piece keeps its style");

    // The line store.
    let id = WidgetId::of("scenario.tv.content");
    let mut v = TextViewport::new(id);
    assert!(v.is_empty());
    assert_eq!(v.len(), 0);
    assert!(v.last().is_none() && v.line(0).is_none() && v.line_text(0).is_none());
    let first = v.revision();
    v.push(vec![Span::plain("alpha")]);
    assert!(v.revision() > first, "a change bumps the revision");
    v.extend(vec![
        vec![Span::muted("beta")],
        vec![Span::plain("gam"), Span::plain("ma")],
    ]);
    v.extend(Vec::new());
    assert_eq!(v.len(), 3);
    assert_eq!(v.line_text(2), Some("gamma"));
    assert_eq!(v.lines().count(), 3);
    assert_eq!(v.last(), v.line(2));
    v.replace_last(vec![Span::plain("gamma!")]);
    assert_eq!(v.line_text(2), Some("gamma!"));
    let revision = v.revision();
    v.replace_last(vec![Span::plain("gamma!")]);
    assert_eq!(
        v.revision(),
        revision,
        "an identical replacement changes nothing"
    );
    assert_eq!(
        v.pop().map(|l| viewport::line_text(&l)),
        Some("gamma!".to_owned())
    );
    assert_eq!(v.len(), 2);
    v.set_lines(numbered(4));
    assert_eq!(v.line_text(3), Some("row 03 the quick brown fox"));
    v.clear();
    assert!(v.is_empty() && v.pop().is_none());
    v.replace_last(vec![Span::plain("only")]);
    assert_eq!(v.len(), 1, "replacing the last of nothing appends");

    // Retention drops the oldest lines.
    let mut capped = TextViewport::new(id).max_lines(3);
    assert_eq!(capped.retention(), Some(3));
    capped.extend(numbered(5));
    assert_eq!(capped.len(), 3);
    assert_eq!(capped.line_text(0), Some("row 02 the quick brown fox"));
    capped.set_max_lines(Some(2));
    assert_eq!(capped.len(), 2);
    capped.set_max_lines(None);
    assert_eq!(capped.retention(), None);
    let from = TextViewport::with_lines(id, numbered(6));
    assert_eq!(from.len(), 6);
    assert_eq!(TextViewport::new(id).retention(), None);

    // Marks and reveal: a search hit is scrolled into view, even sideways.
    let mut v = TextViewport::with_lines(id, numbered(100)).horizontal(HScroll::default());
    v.follow = false;
    v.set_area(Rect::new(0, 0, 20, 5));
    assert!(v.marks().is_empty());
    v.set_marks(vec![Mark {
        line: 40,
        range: 4..9,
        current: true,
    }]);
    assert_eq!(v.marks().len(), 1);
    v.reveal_line(40);
    assert!(
        v.scroll.v.visible_range().contains(&40),
        "{:?}",
        v.scroll.v.visible_range()
    );
    assert!(!v.follow, "revealing a line leaves the tail");
    assert!(v.h_enabled());
    assert!(
        v.reveal_cols(30..35),
        "a column off to the right is revealed"
    );
    assert!(v.scroll.h.offset > 0);
    v.scroll.h.offset = 0;
    assert!(
        v.scroll.h.visible_range().contains(&15),
        "the start is in view"
    );
    assert!(
        v.reveal_cols(15..25),
        "a range whose end is off to the right is revealed"
    );
    assert!(
        v.scroll.h.visible_range().contains(&24),
        "the last column of the range is in view: {:?}",
        v.scroll.h.visible_range()
    );
    v.scroll.h.offset = 0;
    v.reveal_mark(60, 4..9);
    assert!(v.scroll.v.visible_range().contains(&60));
    v.clear_marks();
    assert!(v.marks().is_empty());
    assert!(
        !TextViewport::new(id).reveal_cols(0..5),
        "no strip, nothing to reveal"
    );

    // The work done is counted.
    let mut counted = TextViewport::with_lines(id, numbered(10));
    assert_eq!(counted.work_counters(), WorkCounters::default());
    counted.set_area(Rect::new(0, 0, 40, 5));
    let work = counted.work_counters();
    assert_eq!(work.segmented_lines, 10, "each line is segmented once");
    counted.set_area(Rect::new(0, 0, 40, 5));
    assert_eq!(
        counted.work_counters().segmented_lines,
        10,
        "a prepared view does no work"
    );
}

/// Reading, selecting and copying in a viewport, by key and by pointer.
fn text_viewport_input() {
    let id = WidgetId::of("scenario.tv.input");
    let area = Rect::new(0, 0, 40, 8);
    let mut v = TextViewport::with_lines(id, numbered(60)).nav(NavKeys::DESKTOP.vim(true));
    v.set_area(area);
    assert!(v.follow && v.is_at_tail());
    assert_eq!(
        v.scrollback_depth(),
        0,
        "on the live tail nothing is behind"
    );
    let painted = paint_hits(area, |area, buf, ctx| {
        v.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(first_row(&painted.buf, 26), "row 52 the quick brown fox");

    // Keys scroll, and say when following changed.
    let (out, action) = v.on_key(&key(KeyCode::Up));
    assert_eq!(out, Outcome::Changed);
    assert_eq!(action, Some(ViewportAction::FollowChanged(false)));
    assert!(v.scrollback_depth() > 0);
    assert_eq!(
        v.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, Some(ViewportAction::FollowChanged(true)))
    );
    let (out, action) = v.on_key(&key(KeyCode::Char('f')));
    assert_eq!(
        (out, action),
        (Outcome::Changed, Some(ViewportAction::FollowChanged(false)))
    );
    assert_eq!(
        v.on_key(&key(KeyCode::Char('f'))),
        (Outcome::Changed, Some(ViewportAction::FollowChanged(true)))
    );
    assert_eq!(
        v.on_key(&key(KeyCode::Char('k'))).0,
        Outcome::Changed,
        "vim k moves up"
    );
    assert_eq!(v.on_key(&key(KeyCode::PageUp)).0, Outcome::Changed);
    assert_eq!(v.on_key(&key(KeyCode::Home)).0, Outcome::Changed);
    assert_eq!(v.scroll.v.offset, 0);
    assert_eq!(v.on_key(&key(KeyCode::Home)), (Outcome::Consumed, None));
    assert_eq!(v.on_key(&key(KeyCode::PageDown)).0, Outcome::Changed);
    assert_eq!(
        v.on_key(&key(KeyCode::End)),
        (Outcome::Changed, Some(ViewportAction::FollowChanged(true))),
        "End returns to the live tail"
    );
    assert_eq!(v.on_key(&key(KeyCode::Char('x'))), (Outcome::Ignored, None));
    assert_eq!(
        v.on_key(&key(KeyCode::Esc)),
        (Outcome::Ignored, None),
        "no selection to clear"
    );
    v.set_follow(false);
    assert!(!v.follow);
    v.set_follow(true);
    assert!(v.follow && v.scroll.at_end());

    // Pointer selection: drag, word, double click, select all, clear.
    let (from, to) = (Position::new(4, 1), Position::new(13, 1));
    assert_eq!(v.pos_at(from), Some(CellPos { line: 53, col: 4 }));
    assert_eq!(v.screen_pos(CellPos { line: 53, col: 4 }), Some(from));
    assert_eq!(
        v.screen_pos(CellPos { line: 2, col: 0 }),
        None,
        "scrolled out of view"
    );
    assert!(!v.has_anchor() && !v.has_selection());
    v.on_click(from);
    assert!(v.has_anchor(), "a press anchors the selection");
    v.on_drag(to);
    assert!(v.has_selection());
    assert_eq!(v.selected_text().as_deref(), Some("53 the qu"));
    let (a, b) = v.selection().expect("a selection");
    assert_eq!((a.line, a.col, b.line), (53, 4, 53));
    assert_eq!(v.context_point(), to, "the menu opens at the drag end");
    assert_eq!(v.clear_selection(), Outcome::Changed);
    assert_eq!(v.selected_text(), None);
    assert_eq!(v.select_word_at(Position::new(9, 2)), Outcome::Changed);
    assert_eq!(v.selected_text().as_deref(), Some("the"));
    v.clear_selection();
    v.on_press_with(Position::new(9, 2), 2, KeyModifiers::NONE);
    assert_eq!(
        v.selected_text().as_deref(),
        Some("the"),
        "a double press selects a word"
    );
    v.on_press_with(Position::new(9, 2), 3, KeyModifiers::NONE);
    assert_eq!(
        v.selected_text().as_deref(),
        Some("row 54 the quick brown fox")
    );
    v.clear_selection();
    v.on_click_with(Position::new(9, 2), 2);
    v.on_click_with(Position::new(9, 2), 3);
    assert_eq!(v.select_all(), Outcome::Changed);
    assert!(v.selected_text().expect("all").starts_with("row 00"));
    assert_eq!(v.select_all(), Outcome::Consumed, "already everything");
    v.clear_selection();

    // The keymap: select all, copy and the shifted arrows say what they did.
    let keymap = vec![
        (KeyBinding::char('g'), ViewportCmd::ToggleFollow),
        (KeyBinding::char('a'), ViewportCmd::SelectAll),
        (KeyBinding::char('c'), ViewportCmd::Copy),
    ];
    let mut mapped = TextViewport::with_lines(id, numbered(60)).keymap(keymap);
    mapped.set_area(area);
    assert_eq!(
        mapped.on_key(&key(KeyCode::Char('f'))),
        (Outcome::Ignored, None),
        "f is unbound"
    );
    assert_eq!(
        mapped.on_key(&key(KeyCode::Char('g'))),
        (Outcome::Changed, Some(ViewportAction::FollowChanged(false)))
    );
    assert_eq!(
        mapped.on_key(&key(KeyCode::Char('c'))),
        (Outcome::Consumed, None),
        "nothing to copy"
    );
    assert_eq!(
        mapped.on_key(&key(KeyCode::Char('a'))),
        (Outcome::Changed, Some(ViewportAction::SelectionChanged))
    );
    let (out, action) = mapped.on_key(&key(KeyCode::Char('c')));
    assert_eq!(out, Outcome::Changed);
    let Some(ViewportAction::Copy(text)) = action else {
        panic!("copy reports the text: {action:?}");
    };
    assert!(text.starts_with("row 00") && text.ends_with("row 59 the quick brown fox"));
    assert_eq!(
        mapped.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(ViewportAction::SelectionChanged))
    );
    mapped.on_click(Position::new(4, 1));
    let shifted = Key {
        code: KeyCode::Right,
        mods: KeyModifiers::SHIFT,
    };
    assert_eq!(
        mapped.on_key(&shifted).1,
        Some(ViewportAction::SelectionChanged)
    );

    // A policy that switches pointer selection off.
    let mut still = TextViewport::with_lines(id, numbered(60)).pointer_select(PointerSelect::OFF);
    still.set_area(area);
    still.on_click(from);
    still.on_drag(to);
    assert!(!still.has_selection());
}

/// Layout options: horizontal strip, wrapping, the bar gap, track clicks, and
/// the shared-reference draw through `ViewportOps`.
fn text_viewport_layout() {
    let id = WidgetId::of("scenario.tv.layout");
    let area = Rect::new(0, 0, 40, 8);
    let long = "x".repeat(120);
    let wide_lines = || {
        (0..20)
            .map(|_| vec![Span::plain(&long)])
            .collect::<Vec<_>>()
    };

    // The horizontal strip scrolls by its step; off, it does nothing.
    let mut wide =
        TextViewport::with_lines(id, wide_lines()).horizontal(HScroll::default().step(5));
    wide.set_area(area);
    assert!(wide.h_enabled());
    assert_eq!(wide.hbar().height, 1, "a bottom strip for the wide text");
    assert_eq!(wide.on_key(&key(KeyCode::Right)), (Outcome::Changed, None));
    assert_eq!(wide.scroll.h.offset, 5);
    assert_eq!(wide.on_key(&key(KeyCode::Left)), (Outcome::Changed, None));
    assert_eq!(wide.scroll.h.offset, 0);
    let mut fixed = TextViewport::with_lines(id, wide_lines()).horizontal(HScroll::off());
    fixed.set_area(area);
    assert!(!fixed.h_enabled());
    assert_eq!(fixed.on_key(&key(KeyCode::Right)), (Outcome::Ignored, None));
    assert_eq!(fixed.hbar().height, 0);

    // Wrapping turns the strip into more rows, and the gap narrows the text.
    let exact = || {
        (0..20)
            .map(|_| vec![Span::plain("y".repeat(39))])
            .collect::<Vec<_>>()
    };
    let mut roomy = TextViewport::with_lines(id, exact()).wrap(true);
    roomy.set_area(Rect::new(0, 0, 40, 5));
    let mut tight = TextViewport::with_lines(id, exact())
        .wrap(true)
        .scrollbar_gap(0);
    tight.set_area(Rect::new(0, 0, 40, 5));
    assert!(roomy.wrap && !roomy.h_enabled() || roomy.scroll.h.content_len == 0);
    assert_eq!(
        roomy.scroll.v.content_len, 40,
        "39 cells need two rows at width 38"
    );
    assert_eq!(
        tight.scroll.v.content_len, 20,
        "a zero gap keeps one row each"
    );

    // A track click: `Page` pages, `Jump` goes to the pointer.
    for (click, expect_page) in [(TrackClick::Page, true), (TrackClick::Jump, false)] {
        let mut v = TextViewport::with_lines(id, numbered(60)).track_click(click);
        assert_eq!(v.scroll.track_click, Some(click));
        v.set_area(area);
        v.set_follow(false);
        v.scroll.jump_start();
        let painted = paint_hits(area, |area, buf, ctx| {
            v.render(area, buf, ctx, ctx.theme.canvas)
        });
        let press = PointerEvent::new(
            &painted.hits,
            Phase::Press,
            scrollbar::id_for(id),
            Position::new(39, 7),
            1,
            KeyModifiers::NONE,
        );
        let out = v.pointer(&press);
        assert_eq!(out.outcome(), Outcome::Changed);
        if expect_page {
            assert_eq!(v.scroll.v.offset, 8, "one page down");
        } else {
            assert!(v.scroll.v.offset > 8, "the thumb jumped to the pointer");
        }
    }

    // Through the ViewportOps seam: a draw of an unprepared view clones once.
    let mut v = TextViewport::with_lines(id, numbered(60));
    v.set_area(area);
    let ops: &mut dyn ViewportOps = &mut v;
    assert_eq!(ops.id(), id);
    assert!(ops.keyboard_copy() && !ops.draws_track());
    assert_eq!(ops.vbar().width, 1);
    ops.set_follow(false);
    ops.jump_start();
    ops.page_down();
    assert_eq!(ops.scroll().offset, 8);
    assert_eq!(ops.wheel(Axis::Vertical, 2), Outcome::Changed);
    assert_eq!(ops.scroll().offset, 10);
    assert_eq!(
        ops.on_click(Position::new(0, 0)),
        Outcome::Consumed,
        "a press only anchors"
    );
    assert_eq!(ops.on_drag(Position::new(5, 0)), Outcome::Changed);
    assert_eq!(
        ops.selected_text().as_deref(),
        Some("row 1"),
        "five cells from the left edge"
    );
    ops.clear_selection();
    ops.on_press_with(Position::new(0, 0), 1, KeyModifiers::NONE);
    ops.on_click_with(Position::new(0, 0), 1);
    ops.select_word_at(Position::new(2, 0));
    assert_eq!(ops.selected_text().as_deref(), Some("row"));
    assert!(ops.has_selection());
    assert_eq!(ops.context_point().y, 0);
    assert_eq!(ops.caret(), None);
    assert!(ops.caret_visible());
    assert_eq!(
        ops.screen_pos(CellPos { line: 10, col: 1 }),
        Some(Position::new(1, 0))
    );
    let bar = Bar {
        id: scrollbar::id_for(id),
        owner: id,
        axis: Axis::Vertical,
        track: ops.vbar(),
    };
    assert_eq!(
        ops.bar_input(&bar, Position::new(39, 0), false),
        Outcome::Changed,
        "a drag with no grab jumps to the pointer"
    );
    assert_eq!(ops.hbar().height, 0, "no strip for short lines");
    let before = v.work_counters().draw_fallbacks;
    v.push(vec![Span::plain("fresh")]);
    paint_hits(area, |area, buf, ctx| {
        ViewportOps::draw(&v, area, buf, ctx, ctx.theme.canvas, false);
    });
    assert_eq!(
        v.work_counters().draw_fallbacks,
        before + 1,
        "unprepared: one clone"
    );
    v.set_area(area);
    paint_hits(area, |area, buf, ctx| {
        ViewportOps::draw(&v, area, buf, ctx, ctx.theme.canvas, false);
    });
    assert_eq!(
        v.work_counters().draw_fallbacks,
        before + 1,
        "prepared: no clone"
    );
    assert!(v.work_counters().draw_cloned_lines > 0);
    assert!(v.work_counters().draw_cloned_bytes > 0);
}

fn grid_text(view: &TermView<'_>, row: u16) -> String {
    (0..view.cols())
        .map(|col| {
            view.cell(row, col)
                .map_or_else(String::new, |cell| match cell.text() {
                    "" => " ".to_owned(),
                    text => text.to_owned(),
                })
        })
        .collect()
}

/// The terminal grid: feeding bytes, the cursor, modes, history and resizing.
fn terminal_grid_state() {
    let mut state = TerminalState::new(4, 10);
    assert_eq!(state.size(), (4, 10));
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(state.modes(), Modes::default());
    let events = state.feed(b"\x1b]0;shell\x07hello\r\nworld\x1b[31m!\x1b[0m");
    assert!(
        events.contains(&TermEvent::Title("shell".to_owned())),
        "{events:?}"
    );
    {
        let view: TermView<'_> = state.view(0);
        assert_eq!((view.rows(), view.cols()), (4, 10));
        assert_eq!(grid_text(&view, 0).trim_end(), "hello");
        assert_eq!(grid_text(&view, 1).trim_end(), "world!");
        let cell: TermCell<'_> = view.cell(0, 0).expect("a cell");
        assert_eq!(cell.text(), "h");
        assert!(!cell.is_wide_continuation());
        assert!(view.cell(9, 0).is_none(), "no such row");
        assert_eq!(view.row_wrap(0), Some(RowWrap::Hard));
        assert_eq!(view.row_wrap(9), None);
        assert_eq!(view.cursor(), (1, 6));
    }
    assert_eq!(
        state.cursor(Rect::new(2, 3, 10, 4)),
        Some(Position::new(8, 4))
    );
    assert_eq!(
        state.cursor(Rect::new(0, 0, 3, 3)),
        None,
        "outside the area"
    );

    // Wide text and soft wraps.
    let mut wide = TerminalState::new(3, 5);
    wide.feed("界abcdefgh".as_bytes());
    let view = wide.view(0);
    assert_eq!(
        view.cell(0, 0).map(|c| c.text().to_owned()).as_deref(),
        Some("界")
    );
    assert!(
        view.cell(0, 1)
            .expect("the second half")
            .is_wide_continuation()
    );
    assert_eq!(
        view.row_wrap(0),
        Some(RowWrap::Hard),
        "row 0 starts the line"
    );
    assert_eq!(view.row_wrap(1), Some(RowWrap::Soft), "row 1 continues it");

    // Modes: the program asks for application cursor keys, bracketed paste,
    // the alternate screen, and hides the cursor.
    let mut modes = TerminalState::new(3, 10);
    modes.feed(b"\x1b[?1h\x1b[?2004h");
    assert!(modes.modes().app_cursor && modes.modes().bracketed_paste);
    assert!(!modes.modes().alt_screen && !modes.modes().hide_cursor);
    modes.feed(b"\x1b[?1049h\x1b[?25l");
    assert!(modes.modes().alt_screen && modes.modes().hide_cursor);
    assert_eq!(
        modes.cursor(Rect::new(0, 0, 10, 3)),
        None,
        "a hidden cursor has no cell"
    );

    // History: lines that scrolled off, the reading offset, and the cap.
    let mut history = TerminalState::new(3, 10);
    let lines: String = (0..10).map(|n| format!("line {n}\r\n")).collect();
    history.feed(lines.as_bytes());
    assert!(
        history.scrollback_len() >= 8,
        "{}",
        history.scrollback_len()
    );
    assert_eq!(history.scrollback(), 0);
    let live = grid_text(&history.view(0), 0);
    history.set_scrollback(2);
    assert_eq!(history.scrollback(), 2);
    assert_ne!(grid_text(&history.view(history.scrollback()), 0), live);
    history.set_scrollback(0);
    let mut capped = TerminalState::new(2, 10);
    let flood: String = (0..(SCROLLBACK_ROWS + 100))
        .map(|n| format!("{n}\r\n"))
        .collect();
    capped.feed(flood.as_bytes());
    assert_eq!(
        capped.scrollback_len(),
        SCROLLBACK_ROWS,
        "the history is capped"
    );

    // Resize and reset keep the size, drop the content.
    state.resize(5, 20);
    assert_eq!(state.size(), (5, 20));
    state.reset();
    assert_eq!(state.size(), (5, 20));
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(grid_text(&state.view(0), 0).trim_end(), "");

    // Bytes that spoof the text are kept out of every cell.
    assert!(
        is_spoofing_format('\u{202e}'),
        "a right-to-left override spoofs"
    );
    assert!(!is_spoofing_format('a'));
    let mut spoof = TerminalState::new(2, 10);
    spoof.feed("a\u{202e}b".as_bytes());
    assert_eq!(grid_text(&spoof.view(0), 0).trim_end(), "ab");
}

/// The grid widget: colour, no colour, and the focused cursor.
fn terminal_grid_view() {
    let area = Rect::new(0, 0, 8, 3);
    let mut state = TerminalState::new(3, 8);
    state.feed(b"\x1b[31mred\x1b[0m ok");
    let draw = |view: TerminalView<'_>| {
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        buf
    };
    let coloured = draw(TerminalView::new(&state));
    assert_eq!(first_row(&coloured, 8).trim_end(), "red ok");
    assert_eq!(coloured[(0, 0)].fg, ratatui::style::Color::Indexed(1));
    let plain = draw(TerminalView::new(&state).color(false));
    assert_eq!(first_row(&plain, 8).trim_end(), "red ok");
    assert_eq!(
        plain[(0, 0)].fg,
        ratatui::style::Color::Reset,
        "no colour: the host's own"
    );
    let focused = draw(TerminalView::new(&state).focused(true));
    assert!(
        focused[(6, 0)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    );
    assert!(
        !coloured[(6, 0)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    );
}

/// The pointer and the grid size of a terminal viewport.
fn terminal_viewport_pointer() {
    let id = WidgetId::of("scenario.terminal.pointer");
    let area = Rect::new(0, 0, 21, 5);
    assert_eq!(
        TerminalViewport::grid_size(area),
        (5, 20),
        "the track takes a column"
    );
    assert_eq!(TerminalViewport::grid_size(Rect::new(0, 0, 1, 0)), (1, 1));
    let mut viewport =
        TerminalViewport::new(id, TerminalState::new(5, 20)).pointer_select(PointerSelect::DEFAULT);
    viewport.feed(b"hello world\r\nsecond line");
    ViewportOps::set_area(&mut viewport, area);
    assert_eq!(viewport.state().size(), (5, 20));
    viewport.resize(6, 30);
    assert_eq!(
        viewport.state().size(),
        (6, 30),
        "the grid follows the area"
    );
    viewport.resize(5, 20);
    ViewportOps::set_area(&mut viewport, area);

    let ops: &mut dyn ViewportOps = &mut viewport;
    ops.on_click(Position::new(0, 0));
    ops.on_drag(Position::new(5, 0));
    assert_eq!(ops.selected_text().as_deref(), Some("hello"));
    ops.clear_selection();
    TerminalViewport::on_press_with(&mut viewport, Position::new(1, 0), 2, KeyModifiers::NONE);
    assert_eq!(
        ViewportOps::selected_text(&viewport).as_deref(),
        Some("hello"),
        "a double press selects a word"
    );
    TerminalViewport::on_press_with(&mut viewport, Position::new(1, 0), 3, KeyModifiers::NONE);
    assert_eq!(
        ViewportOps::selected_text(&viewport).as_deref(),
        Some("hello world"),
        "a triple press selects the line"
    );
    ViewportOps::clear_selection(&mut viewport);
    TerminalViewport::on_click_with(&mut viewport, Position::new(1, 0), 2);
    assert!(
        ViewportOps::has_selection(&viewport),
        "a double click selects too"
    );
    viewport.resize(6, 30);
    assert!(
        !ViewportOps::has_selection(&viewport),
        "a resize drops the selection"
    );

    let mut fixed =
        TerminalViewport::new(id, TerminalState::new(5, 20)).pointer_select(PointerSelect::OFF);
    fixed.feed(b"hello world");
    ViewportOps::set_area(&mut fixed, area);
    ViewportOps::on_click(&mut fixed, Position::new(0, 0));
    ViewportOps::on_drag(&mut fixed, Position::new(5, 0));
    assert!(
        !ViewportOps::has_selection(&fixed),
        "OFF: the pointer selects nothing"
    );
}

/// What a program receives for a key, and what the host never forwards.
fn terminal_key_encoding() {
    let none = Modes::default();
    let app = Modes {
        app_cursor: true,
        ..Modes::default()
    };
    let chord = |code: KeyCode, mods: KeyModifiers| Key { code, mods };
    let plain = |code: KeyCode| key(code);
    assert_eq!(
        encode_key(&plain(KeyCode::Char('a')), &none),
        Some(b"a".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::Enter), &none),
        Some(b"\r".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::Backspace), &none),
        Some(vec![0x7f])
    );
    assert_eq!(
        encode_key(&plain(KeyCode::BackTab), &none),
        Some(b"\x1b[Z".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::Up), &none),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::Up), &app),
        Some(b"\x1bOA".to_vec()),
        "application cursor keys use SS3"
    );
    assert_eq!(
        encode_key(&chord(KeyCode::Up, KeyModifiers::CONTROL), &none),
        Some(b"\x1b[1;5A".to_vec()),
        "Ctrl is the xterm modifier 5"
    );
    assert_eq!(
        encode_key(&chord(KeyCode::Right, KeyModifiers::SHIFT), &none),
        Some(b"\x1b[1;2C".to_vec())
    );
    assert_eq!(
        encode_key(&chord(KeyCode::Char('c'), KeyModifiers::CONTROL), &none),
        Some(vec![3])
    );
    assert_eq!(
        encode_key(&chord(KeyCode::Char('x'), KeyModifiers::ALT), &none),
        Some(vec![0x1b, b'x']),
        "Alt alone is an ESC prefix"
    );
    assert_eq!(
        encode_key(&plain(KeyCode::F(1)), &none),
        Some(b"\x1bOP".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::F(5)), &none),
        Some(b"\x1b[15~".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::PageDown), &none),
        Some(b"\x1b[6~".to_vec())
    );
    assert_eq!(
        encode_key(&plain(KeyCode::F(13)), &none),
        None,
        "no such key"
    );

    // The forwarding policy keeps Ctrl+D and Ctrl+Q away from the program.
    let policy = ForwardPolicy::default();
    assert_eq!(policy.never, ForwardPolicy::NEVER.to_vec());
    assert_eq!(policy.paste_max_bytes, ForwardPolicy::PASTE_MAX_BYTES);
    let eof = encode_key(&chord(KeyCode::Char('d'), KeyModifiers::CONTROL), &none)
        .expect("Ctrl+D encodes");
    assert_eq!(eof, vec![0x04]);
    assert_eq!(policy.key(eof), None, "end-of-file is never forwarded");
    assert_eq!(policy.key(b"a".to_vec()), Some(b"a".to_vec()));
    let quit = encode_key(&chord(KeyCode::Char('q'), KeyModifiers::CONTROL), &none)
        .expect("Ctrl+Q encodes");
    assert_eq!(quit, vec![0x11]);
    assert_eq!(policy.key(quit), None, "Ctrl+Q is never forwarded either");
    // The default policy refuses a paste past 64 KiB and takes one at the limit.
    let limit = 64 * 1024;
    assert_eq!(
        policy.paste(&"a".repeat(limit), false),
        Paste::Send(vec![b'a'; limit])
    );
    assert_eq!(
        policy.paste(&"a".repeat(limit + 1), false),
        Paste::TooLarge { bytes: limit + 1 }
    );
    let small = ForwardPolicy::new(8);
    assert_eq!(small.paste_max_bytes, 8);
    assert_eq!(
        small.paste("123456789", false),
        Paste::TooLarge { bytes: 9 }
    );
    assert_eq!(
        ForwardPolicy::new(ForwardPolicy::PASTE_MAX_BYTES).paste("ok", false),
        Paste::Send(b"ok".to_vec())
    );
}

/// Every scroll command, rebound to plain letters so each one is driven by
/// its own key, and what each does to the two axes.
fn scroll_view_commands() {
    let id = WidgetId::of("scenario.view.commands");
    let area = Rect::new(0, 0, 20, 8);
    let content = Size::new(40, 12);
    let keys = vec![
        (KeyBinding::char('u'), ScrollCmd::PageUp),
        (KeyBinding::char('d'), ScrollCmd::PageDown),
        (KeyBinding::char('l'), ScrollCmd::PageLeft),
        (KeyBinding::char('r'), ScrollCmd::PageRight),
        (KeyBinding::char('t'), ScrollCmd::Top),
        (KeyBinding::char('b'), ScrollCmd::Bottom),
    ];
    let mut view = ScrollView::new(id).keys(keys);
    paint_hits(area, |area, buf, ctx| {
        view.render(area, content, buf, ctx, ruler)
    });
    let press = |view: &mut ScrollView, c: char| view.on_key(&key(KeyCode::Char(c)));
    assert_eq!(press(&mut view, 'd'), Some(Outcome::Changed), "PageDown");
    assert_eq!(view.offset(), (0, 5), "a page is clamped at the end");
    assert_eq!(press(&mut view, 'd'), Some(Outcome::Consumed));
    assert_eq!(press(&mut view, 'u'), Some(Outcome::Changed), "PageUp");
    assert_eq!(view.offset(), (0, 0));
    assert_eq!(press(&mut view, 'r'), Some(Outcome::Changed), "PageRight");
    assert_eq!(view.offset(), (18, 0));
    assert_eq!(press(&mut view, 'r'), Some(Outcome::Changed));
    assert_eq!(press(&mut view, 'r'), Some(Outcome::Consumed));
    assert_eq!(view.offset(), (22, 0));
    assert_eq!(press(&mut view, 'b'), Some(Outcome::Changed), "Bottom");
    assert_eq!(view.offset(), (22, 5), "Bottom keeps the horizontal offset");
    assert_eq!(press(&mut view, 'l'), Some(Outcome::Changed), "PageLeft");
    assert_eq!(view.offset(), (4, 5));
    assert_eq!(press(&mut view, 't'), Some(Outcome::Changed), "Top");
    assert_eq!(view.offset(), (0, 0), "Top resets both axes");
    assert_eq!(press(&mut view, 't'), Some(Outcome::Consumed));
    assert_eq!(press(&mut view, 'x'), None, "not bound");
    assert_eq!(
        view.on_key(&key(KeyCode::Home)),
        None,
        "the default keys were replaced"
    );
    let labels = [
        ScrollCmd::PageUp,
        ScrollCmd::PageDown,
        ScrollCmd::PageLeft,
        ScrollCmd::PageRight,
        ScrollCmd::Top,
        ScrollCmd::Bottom,
    ]
    .map(|cmd| cmd.label());
    assert_eq!(labels[4], "Scroll to the top");
    assert!(
        labels
            .iter()
            .enumerate()
            .all(|(i, a)| labels[i + 1..].iter().all(|b| a != b)),
        "every command has its own label: {labels:?}"
    );
}

#[test]
fn catalog_scroll_cases_execute() {
    let mut failed = Vec::new();
    for case in CASES {
        if std::panic::catch_unwind(case.run).is_err() {
            failed.push(format!("{:?}", case.id));
        }
    }
    assert!(failed.is_empty(), "failed scenarios: {failed:?}");
}
