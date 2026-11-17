//! Lane `layout` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:layout` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_layout.rs`.

use super::*;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::list::{KeyStep, Moved};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::response::Response;
use junie_tui::core::scroll::TrackClick;
use junie_tui::ui::columns::{
    Col, ColumnSizing, DEFAULT_SEAM_W, HeaderAction, HeaderLabels, MIN_USER_WIDTH, header_menu,
    render_seam, seam_part, solve,
};
use junie_tui::ui::content::ContentState;
use junie_tui::ui::layout::{
    Maximized, Overflow, SplitCmd, SplitKeys, Stack, StackChild, StackLayout, StackScroll,
};
use junie_tui::ui::list_core::ListCore;
use junie_tui::ui::pane_tree::{
    Direction, Frames, PaneCmd, PaneId, PaneNode, PaneRefusal, PaneResult, PaneTree, Seam, nearest,
};
use junie_tui::widgets::action_row::ActionRow;
use junie_tui::widgets::breadcrumb::Breadcrumb;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::chips::{ChipCmd, overflow_run};
use junie_tui::widgets::detail_card::{DetailCard, DetailLayout};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::empty::{self, EmptyAction, EmptyKind};
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::notice::{Notice, NoticeKind};
use junie_tui::widgets::page_header::{Heading, PageHeader};
use junie_tui::widgets::rail::{RailAction, SidebarRail};
use junie_tui::widgets::splitter::SeamDoubleClick;
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::table::SortDir;
use junie_tui::widgets::too_small::TooSmall;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::LayoutSplitGeometry, run: layout_split_geometry },
    ScenarioCase { id: ScenarioId::LayoutSplitKeys, run: layout_split_keys },
    ScenarioCase { id: ScenarioId::LayoutStackFlow, run: layout_stack_flow },
    ScenarioCase { id: ScenarioId::LayoutStackScroll, run: layout_stack_scroll },
    ScenarioCase { id: ScenarioId::LayoutSplitterSeam, run: layout_splitter_seam },
    ScenarioCase { id: ScenarioId::LayoutPaneNodes, run: layout_pane_nodes },
    ScenarioCase { id: ScenarioId::LayoutPaneTreeFocus, run: layout_pane_tree_focus },
    ScenarioCase { id: ScenarioId::LayoutPaneTreeSeams, run: layout_pane_tree_seams },
    ScenarioCase { id: ScenarioId::LayoutColumnSolve, run: layout_column_solve },
    ScenarioCase { id: ScenarioId::LayoutColumnSizing, run: layout_column_sizing },
    ScenarioCase { id: ScenarioId::LayoutColumnHeaderMenu, run: layout_column_header_menu },
    ScenarioCase { id: ScenarioId::LayoutChipBarActions, run: layout_chip_bar_actions },
    ScenarioCase { id: ScenarioId::LayoutChipBarOverflow, run: layout_chip_bar_overflow },
    ScenarioCase { id: ScenarioId::LayoutToolbarActions, run: layout_toolbar_actions },
    ScenarioCase { id: ScenarioId::LayoutToolbarOverflow, run: layout_toolbar_overflow },
    ScenarioCase { id: ScenarioId::LayoutStepRail, run: layout_step_rail },
    ScenarioCase { id: ScenarioId::LayoutStepRailScroll, run: layout_step_rail_scroll },
    ScenarioCase { id: ScenarioId::LayoutListCore, run: layout_list_core },
    ScenarioCase { id: ScenarioId::LayoutNoticeKinds, run: layout_notice_kinds },
    ScenarioCase { id: ScenarioId::LayoutNoticeWrapping, run: layout_notice_wrapping },
    ScenarioCase { id: ScenarioId::LayoutEmptyState, run: layout_empty_state },
    ScenarioCase { id: ScenarioId::LayoutPageHeader, run: layout_page_header },
    ScenarioCase { id: ScenarioId::LayoutDetailCard, run: layout_detail_card },
    ScenarioCase { id: ScenarioId::LayoutSidebarRail, run: layout_sidebar_rail },
    ScenarioCase { id: ScenarioId::LayoutTooSmall, run: layout_too_small },
    ScenarioCase { id: ScenarioId::LayoutBreadcrumb, run: layout_breadcrumb },
];

/// A key with the given modifiers.
fn chord(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

/// One painted frame under a chosen pointer state.
fn paint_with(
    area: Rect,
    interaction: Interaction,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Painted {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Painted { buf, hits }
}

fn layout_split_geometry() {
    let area = Rect::new(2, 1, 41, 10);
    let mut split = Split::new(50, 4, 4);
    assert_eq!(
        (split.percent, split.min_first, split.min_second),
        (50, 4, 4)
    );
    assert_eq!(split.maximized, Maximized::None);

    // A side-by-side split spends one gap column between the halves.
    let (left, right) = split.layout(SplitDir::Horizontal, area, 1);
    assert_eq!(left, Rect::new(2, 1, 20, 10));
    assert_eq!(right, Rect::new(23, 1, 20, 10));
    assert_eq!(split.horizontal(area, 1), (left, right));
    assert_eq!(
        split.handle(SplitDir::Horizontal, area, 1),
        Rect::new(22, 1, 1, 10),
        "the handle is the gap column between the halves"
    );
    assert_eq!(
        split.handle(SplitDir::Horizontal, area, 0),
        Rect::ZERO,
        "no gap leaves nothing to grab"
    );

    // A stacked split spends one gap row.
    let (top, bottom) = split.layout(SplitDir::Vertical, area, 1);
    assert_eq!(top, Rect::new(2, 1, 41, 4));
    assert_eq!(bottom, Rect::new(2, 6, 41, 5));
    assert_eq!(split.vertical(area, 1), (top, bottom));
    assert_eq!(
        split.handle(SplitDir::Vertical, area, 1),
        Rect::new(2, 5, 41, 1)
    );

    // Dragging moves the seam to the pointer and reports a real change once.
    assert!(split.drag_to(SplitDir::Horizontal, area, 1, Position::new(12, 3)));
    assert_eq!(split.layout(SplitDir::Horizontal, area, 1).0.width, 10);
    assert!(
        !split.drag_to(SplitDir::Horizontal, area, 1, Position::new(12, 3)),
        "the same position changes nothing"
    );
    assert!(split.drag_to(SplitDir::Horizontal, area, 1, Position::new(0, 3)));
    assert_eq!(
        split.layout(SplitDir::Horizontal, area, 1).0.width,
        4,
        "the first half stops at its minimum"
    );
    assert!(split.drag_to(SplitDir::Horizontal, area, 1, Position::new(200, 3)));
    assert_eq!(
        split.layout(SplitDir::Horizontal, area, 1).1.width,
        4,
        "the second half stops at its minimum"
    );
    let mut cramped = Split::new(50, 30, 30);
    assert!(
        !cramped.drag_to(SplitDir::Horizontal, area, 1, Position::new(12, 3)),
        "an area below the two minima cannot be split"
    );

    // Nudging is a drag by whole cells from the current seam.
    let mut split = Split::new(50, 4, 4);
    split.nudge(SplitDir::Horizontal, area, 1, 3);
    assert_eq!(split.layout(SplitDir::Horizontal, area, 1).0.width, 23);
    split.nudge(SplitDir::Horizontal, area, 1, -5);
    assert_eq!(split.layout(SplitDir::Horizontal, area, 1).0.width, 18);
    split.nudge(SplitDir::Vertical, area, 1, 1);
    assert_eq!(split.layout(SplitDir::Vertical, area, 1).0.height, 5);

    // Growing works on the percentage and stays inside 5 to 95.
    let mut grown = Split::new(50, 1, 1);
    grown.grow(10);
    assert_eq!(grown.percent, 60);
    grown.grow(500);
    assert_eq!(grown.percent, 95);
    grown.grow(-500);
    assert_eq!(grown.percent, 5);

    // Maximizing hands one side the whole area and toggles back.
    let mut split = Split::new(50, 4, 4);
    split.toggle_max(Maximized::First);
    assert_eq!(split.maximized, Maximized::First);
    assert_eq!(
        split.layout(SplitDir::Horizontal, area, 1),
        (area, Rect::ZERO)
    );
    assert_eq!(split.handle(SplitDir::Horizontal, area, 1), Rect::ZERO);
    assert_eq!(split.extent_for(10, 30, 1), 10, "a maximized first side");
    split.toggle_max(Maximized::Second);
    assert_eq!(split.maximized, Maximized::Second);
    assert_eq!(
        split.layout(SplitDir::Vertical, area, 1),
        (Rect::ZERO, area)
    );
    assert_eq!(split.extent_for(10, 30, 1), 30, "a maximized second side");
    split.toggle_max(Maximized::Second);
    assert_eq!(split.maximized, Maximized::None);

    // Reset drops the drag, the maximize state and the percentage change.
    split.grow(20);
    split.drag_to(SplitDir::Horizontal, area, 1, Position::new(30, 3));
    split.toggle_max(Maximized::First);
    split.reset();
    assert_eq!(split.percent, split.default_percent);
    assert_eq!(split.maximized, Maximized::None);
    assert_eq!(split.layout(SplitDir::Horizontal, area, 1).0.width, 20);

    // The natural extent starts from the default ratio; the live extent
    // follows the dragged one.
    let mut split = Split::new(50, 4, 4);
    assert_eq!(split.extent_for(10, 10, 1), 21);
    assert_eq!(split.natural_extent(10, 10, 1), 21);
    let wide = Rect::new(0, 0, 101, 4);
    split.drag_to(SplitDir::Horizontal, wide, 1, Position::new(25, 0));
    assert_eq!(
        split.extent_for(10, 10, 1),
        41,
        "a quarter split needs more"
    );
    assert_eq!(
        split.natural_extent(10, 10, 1),
        21,
        "the natural extent ignores the drag"
    );

    // Both directions render the same two panes into a Buffer.
    for (dir, expect) in [
        (SplitDir::Horizontal, "left|right"),
        (SplitDir::Vertical, "top|bottom"),
    ] {
        let split = Split::new(50, 1, 1);
        let buf = paint(Rect::new(0, 0, 20, 7), |area, buf, _| {
            let (a, b) = split.layout(dir, area, 1);
            let (first, second) = expect.split_once('|').expect("two labels");
            buf.set_string(a.x, a.y, first, ratatui::style::Style::new());
            buf.set_string(b.x, b.y, second, ratatui::style::Style::new());
        });
        let text = text_of(&buf);
        assert!(text.contains(expect.split('|').next().unwrap()), "{text}");
        assert!(text.contains(expect.split('|').nth(1).unwrap()), "{text}");
    }
}

fn layout_split_keys() {
    let area = Rect::new(0, 0, 41, 10);
    let alt = KeyModifiers::ALT | KeyModifiers::SHIFT;
    let left = chord(KeyCode::Left, alt);
    let right = chord(KeyCode::Right, alt);
    let up = chord(KeyCode::Up, alt);
    let down = chord(KeyCode::Down, alt);

    let keys = SplitKeys::default();
    assert_eq!(keys.step, 2);
    assert_eq!(keys.delta(&left, SplitDir::Horizontal), Some(-2));
    assert_eq!(keys.delta(&right, SplitDir::Horizontal), Some(2));
    assert_eq!(keys.delta(&up, SplitDir::Vertical), Some(-2));
    assert_eq!(keys.delta(&down, SplitDir::Vertical), Some(2));
    assert_eq!(
        keys.delta(&up, SplitDir::Horizontal),
        None,
        "a vertical command does not move a side-by-side seam"
    );
    assert_eq!(keys.delta(&key(KeyCode::Left), SplitDir::Horizontal), None);
    for (cmd, label) in [
        (SplitCmd::Left, "Move seam left"),
        (SplitCmd::Right, "Move seam right"),
        (SplitCmd::Up, "Move seam up"),
        (SplitCmd::Down, "Move seam down"),
    ] {
        assert_eq!(cmd.label(), label);
    }
    assert_eq!(
        junie_tui::core::keymap::lookup(&keys.keymap, &up),
        Some(SplitCmd::Up)
    );
    assert_eq!(
        junie_tui::core::keymap::lookup(&keys.keymap, &down),
        Some(SplitCmd::Down)
    );

    // apply moves the seam and says whether the layout changed.
    let mut split = Split::new(50, 4, 4);
    assert_eq!(
        keys.apply(&left, &mut split, SplitDir::Horizontal, area, 1),
        Some(Outcome::Changed)
    );
    assert_eq!(split.layout(SplitDir::Horizontal, area, 1).0.width, 18);
    assert_eq!(
        keys.apply(&down, &mut split, SplitDir::Vertical, area, 1),
        Some(Outcome::Changed)
    );
    assert_eq!(
        keys.apply(
            &key(KeyCode::Left),
            &mut split,
            SplitDir::Horizontal,
            area,
            1
        ),
        None,
        "an unbound key is not a seam command"
    );
    let mut pinned = Split::new(50, 20, 20);
    assert_eq!(
        keys.apply(&left, &mut pinned, SplitDir::Horizontal, area, 1),
        Some(Outcome::Consumed),
        "a seam at its minimum eats the key and stays"
    );
    let mut maximized = Split::new(50, 4, 4);
    maximized.toggle_max(Maximized::First);
    assert_eq!(
        keys.apply(&right, &mut maximized, SplitDir::Horizontal, area, 1),
        Some(Outcome::Consumed),
        "a maximized split has no seam to move"
    );

    // The step is a builder; a bigger step moves farther.
    let wide = SplitKeys::default().step(5);
    assert_eq!(wide.step, 5);
    assert_eq!(wide.delta(&right, SplitDir::Horizontal), Some(5));

    // The keymap is a builder; a remap replaces the defaults entirely.
    let remap: Vec<(KeyBinding, SplitCmd)> = vec![
        (KeyBinding::char('h'), SplitCmd::Left),
        (KeyBinding::char('l'), SplitCmd::Right),
    ];
    let vim = SplitKeys::default().keymap(remap.clone());
    assert_eq!(vim.keymap, remap);
    assert_eq!(
        vim.delta(&key(KeyCode::Char('h')), SplitDir::Horizontal),
        Some(-2)
    );
    assert_eq!(vim.delta(&left, SplitDir::Horizontal), None);

    // The hint names the pair of keys and the action, per direction.
    let h = keys.hint(SplitDir::Horizontal).expect("a horizontal hint");
    assert_eq!(h.action, "Resize");
    assert!(h.key.contains('←') && h.key.contains('→'), "{}", h.key);
    let v = keys.hint(SplitDir::Vertical).expect("a vertical hint");
    assert!(v.key.contains('↑') && v.key.contains('↓'), "{}", v.key);
    let single = SplitKeys::default().keymap(vec![(KeyBinding::char('h'), SplitCmd::Left)]);
    assert!(
        single.hint(SplitDir::Horizontal).is_some(),
        "one bound key still earns a hint"
    );
    assert!(
        single.hint(SplitDir::Vertical).is_none(),
        "no bound key, no hint"
    );
}

fn stack_children() -> [StackChild; 3] {
    [
        StackChild::fixed(3),
        StackChild::flex(2, 6),
        StackChild::fixed(4),
    ]
}

fn layout_stack_flow() {
    let children = stack_children();
    assert_eq!((children[0].height, children[0].min), (3, 3));
    assert_eq!((children[1].height, children[1].min), (6, 2));
    let area = Rect::new(0, 0, 30, 10);

    // No gap: the flexible child gives up rows and everything still fits.
    let flat = Stack::default().gap(0);
    assert_eq!(flat.measure(&children), 13);
    let mut state = StackScroll::default();
    let laid: StackLayout = flat.layout(area, &children, &mut state, None);
    assert_eq!(laid.slot(0), Some(Rect::new(0, 0, 30, 3)));
    assert_eq!(laid.slot(1), Some(Rect::new(0, 3, 30, 3)));
    assert_eq!(laid.slot(2), Some(Rect::new(0, 6, 30, 4)));
    assert_eq!(laid.track, None, "nothing overflows, so no scrollbar");
    assert_eq!((laid.hidden_above, laid.hidden_below), (0, 0));
    assert_eq!(laid.slot(9), None);

    // A gap pushes the stack past the area; the default scrolls.
    let gapped = Stack::default().gap(1);
    assert_eq!(gapped.measure(&children), 15);
    let mut state = StackScroll::default();
    let laid = gapped.layout(area, &children, &mut state, None);
    assert_eq!(laid.track, Some(Rect::new(29, 0, 1, 10)));
    assert_eq!(laid.slot(0), Some(Rect::new(0, 0, 28, 3)));
    assert_eq!(laid.slot(1), Some(Rect::new(0, 4, 28, 6)));
    assert_eq!(laid.slot(2), None);
    assert_eq!(laid.hidden_below, 1);

    // The scrollbar gap is the spacing between content and bar.
    let mut state = StackScroll::default();
    let wide_gap = gapped.scrollbar_gap(3);
    let laid = wide_gap.layout(area, &children, &mut state, None);
    assert_eq!(laid.slot(0).map(|r| r.width), Some(26));

    assert_eq!(
        Stack::default().overflow,
        Overflow::Scroll,
        "scrolling is the default"
    );
    assert_eq!(gapped.overflow(Overflow::Scroll).overflow, Overflow::Scroll);

    // Clip drops the scrollbar and shows what fits.
    let mut state = StackScroll::default();
    let clip = gapped.overflow(Overflow::Clip);
    assert_eq!(clip.overflow, Overflow::Clip);
    let laid = clip.layout(area, &children, &mut state, None);
    assert_eq!(laid.track, None);
    assert_eq!(laid.slot(0), Some(Rect::new(0, 0, 30, 3)));
    assert_eq!(laid.slot(1), Some(Rect::new(0, 4, 30, 2)));
    assert_eq!(laid.slot(2), None);
    assert_eq!(laid.hidden_below, 1);

    // A larger minimum for flexible children stops them shrinking so far.
    let mut state = StackScroll::default();
    let roomy = Stack::default().gap(0).min_child_rows(5);
    assert_eq!(roomy.min_child_rows, 5);
    let laid = roomy.layout(area, &children, &mut state, None);
    assert!(laid.track.is_some(), "the minimum forces a scroll");

    // The track-click policy travels with the stack into its scroll state: a
    // click at the foot of the track pages by a screen or jumps to the click.
    let many: Vec<StackChild> = (0..12).map(|_| StackChild::fixed(1)).collect();
    let short = Rect::new(0, 0, 30, 4);
    let click_foot = |stack: Stack| {
        let mut state = StackScroll::default();
        stack.layout(short, &many, &mut state, None);
        assert!(
            state.on_scrollbar(Position::new(29, 3)),
            "the click scrolls"
        );
        state.scroll.offset
    };
    let jump = Stack::default().track_click(TrackClick::Jump);
    assert_eq!(jump.track_click, TrackClick::Jump);
    assert_eq!(Stack::default().track_click, TrackClick::Page);
    let (paged, jumped) = (click_foot(Stack::default()), click_foot(jump));
    assert!(
        paged > 0 && jumped > paged,
        "jump goes to the click, paging only a screen: {jumped} vs {paged}"
    );

    // Focus keeps the focused child visible by scrolling to it.
    let mut state = StackScroll::default();
    let laid = gapped.layout(area, &children, &mut state, Some(2));
    assert_eq!(laid.slot(0), None, "the first child scrolled away");
    assert_eq!(laid.slot(2).map(|r| r.height), Some(4));
    assert_eq!(laid.hidden_above, 2);

    // Chrome paints the scrollbar only when there is a track.
    let id = WidgetId::of("scenario.layout.stack");
    let mut state = StackScroll::default();
    let laid = gapped.layout(area, &children, &mut state, None);
    let buf = paint(area, |area, buf, ctx| {
        Stack::chrome(&laid, area, buf, ctx, id, &state)
    });
    assert_eq!(buf[(29, 0)].symbol(), ctx_glyph().scroll_thumb);
    let flat_laid = flat.layout(area, &children, &mut StackScroll::default(), None);
    let buf = paint(area, |area, buf, ctx| {
        Stack::chrome(&flat_laid, area, buf, ctx, id, &StackScroll::default())
    });
    assert_eq!(buf[(29, 0)].symbol(), " ", "no track, no chrome");
}

fn ctx_glyph() -> junie_tui::theme::Glyphs {
    Theme::for_level(ColorLevel::TrueColor).glyphs
}

fn layout_stack_scroll() {
    let id = WidgetId::of("scenario.layout.stackscroll");
    let children = stack_children();
    let area = Rect::new(0, 0, 30, 10);
    let stack = Stack::default().gap(1);
    let mut state = StackScroll::default();
    stack.layout(area, &children, &mut state, None);

    assert!(state.on_wheel(5), "the wheel scrolls one child per notch");
    assert_eq!(state.scroll.offset, 1);
    let laid = stack.layout(area, &children, &mut state, None);
    assert_eq!(laid.slot(0), None);
    assert_eq!(laid.slot(1).map(|r| r.y), Some(0));
    assert_eq!((laid.hidden_above, laid.hidden_below), (1, 1));
    assert!(state.on_wheel(-1));
    assert!(!state.on_wheel(-1), "the top stops the wheel");
    assert_eq!(state.scroll.offset, 0);

    // The scrollbar track pages the stack; the thumb drags it.
    assert!(
        state.on_scrollbar(Position::new(29, 9)),
        "a click below the thumb pages down"
    );
    let paged = state.scroll.offset;
    assert!(paged > 0, "the track click scrolled down");
    assert!(
        !state.on_scrollbar_drag(Position::new(29, 0)),
        "a page click is not the start of a drag"
    );
    state.scroll.release_track();
    let (start, _) = state.scroll.thumb(10);
    assert!(
        !state.on_scrollbar(Position::new(29, start as u16)),
        "pressing the thumb only grabs it"
    );
    assert!(
        state.on_scrollbar_drag(Position::new(29, 0)),
        "drag the thumb to the top"
    );
    assert!(state.scroll.offset < paged);
    state.on_wheel(1);
    state.reset();
    assert_eq!(state.scroll.offset, 0, "reset returns to the top");

    // Without a laid-out track the scrollbar does nothing.
    let mut fresh = StackScroll::default();
    assert!(!fresh.on_scrollbar(Position::new(29, 9)));
    assert!(!fresh.on_scrollbar_drag(Position::new(29, 5)));

    // Pointer routing through real hit regions.
    let mut state = StackScroll::default();
    let laid = stack.layout(area, &children, &mut state, None);
    let frame = paint_hits(area, |area, buf, ctx| {
        Stack::chrome(&laid, area, buf, ctx, id, &state)
    });
    let bar = junie_tui::widgets::scrollbar::id_for(id);
    let wheel = |axis, delta| {
        PointerEvent::wheel_on(
            &frame.hits,
            id,
            axis,
            delta,
            Position::new(5, 5),
            KeyModifiers::NONE,
        )
    };
    assert_eq!(
        state.pointer(id, &wheel(Axis::Vertical, 1)),
        Some(Outcome::Changed)
    );
    assert_eq!(state.scroll.offset, 1);
    assert_eq!(
        state.pointer(id, &wheel(Axis::Horizontal, 1)),
        Some(Outcome::Ignored)
    );
    assert_eq!(
        state.pointer(
            WidgetId::of("scenario.layout.other"),
            &wheel(Axis::Vertical, 1)
        ),
        None,
        "an event for another widget is not ours"
    );
    let on_bar = |phase, y| {
        PointerEvent::new(
            &frame.hits,
            phase,
            bar,
            Position::new(29, y),
            1,
            KeyModifiers::NONE,
        )
    };
    assert_eq!(
        state.pointer(id, &on_bar(Phase::Press, 0)),
        Some(Outcome::Changed),
        "a press on the track above the thumb pages up"
    );
    assert_eq!(state.scroll.offset, 0);
    assert_eq!(
        state.pointer(id, &on_bar(Phase::Drag, 9)),
        Some(Outcome::Consumed),
        "a page click is not the start of a drag"
    );
    assert_eq!(
        state.pointer(id, &on_bar(Phase::Release, 9)),
        Some(Outcome::Consumed)
    );
    assert_eq!(
        state.pointer(id, &on_bar(Phase::Move, 9)),
        Some(Outcome::Ignored)
    );

    // A stack that fits has nothing to wheel.
    let flat = Stack::default().gap(0);
    let mut state = StackScroll::default();
    let laid = flat.layout(area, &children, &mut state, None);
    let frame = paint_hits(area, |area, buf, ctx| {
        Stack::chrome(&laid, area, buf, ctx, id, &state)
    });
    let ev = PointerEvent::wheel_on(
        &frame.hits,
        id,
        Axis::Vertical,
        1,
        Position::new(5, 5),
        KeyModifiers::NONE,
    );
    assert_eq!(
        state.pointer(id, &ev),
        Some(Outcome::Ignored),
        "a stack that fits ignores the wheel"
    );
}

fn layout_splitter_seam() {
    let id = WidgetId::of("scenario.layout.seam");
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let strip = Rect::new(5, 0, 1, 4);
    let canvas = theme.canvas;

    // Drawing rules: thin at rest, heavy on hover, heavy and accent while held.
    let drawn = |dir: SplitDir, strip: Rect, interaction: Interaction, held: bool| {
        paint_with(Rect::new(0, 0, 12, 6), interaction, |_, buf, ctx| {
            let mut seam = Splitter::new(id, dir);
            if held {
                seam.render_held(strip, buf, ctx, canvas, true);
            } else {
                seam.render(strip, buf, ctx, canvas);
            }
        })
    };
    let rest = drawn(SplitDir::Horizontal, strip, Interaction::default(), false);
    let hover = drawn(
        SplitDir::Horizontal,
        strip,
        Interaction {
            hover: Some(id),
            ..Default::default()
        },
        false,
    );
    let pressed = drawn(
        SplitDir::Horizontal,
        strip,
        Interaction {
            hover: Some(id),
            pressed: Some(id),
            ..Default::default()
        },
        false,
    );
    let held = drawn(SplitDir::Horizontal, strip, Interaction::default(), true);
    let seam_cell = |p: &Painted| p.buf[(5, 1)].clone();
    assert_eq!(seam_cell(&rest).symbol(), theme.glyphs.divider);
    assert_eq!(seam_cell(&rest).fg, theme.ladders.seam.rest);
    assert_eq!(seam_cell(&hover).symbol(), theme.glyphs.scroll_thumb);
    assert_eq!(seam_cell(&hover).fg, theme.ladders.seam.hover);
    assert_eq!(seam_cell(&pressed).symbol(), theme.glyphs.scroll_thumb);
    assert_eq!(seam_cell(&pressed).fg, theme.ladders.seam.pressed);
    assert_eq!(
        seam_cell(&held).fg,
        theme.ladders.seam.pressed,
        "held reads as pressed"
    );
    assert_ne!(
        seam_cell(&hover).fg,
        seam_cell(&rest).fg,
        "rest and hover differ"
    );
    for y in 0..4 {
        assert_eq!(
            hover.buf[(4, y)].symbol(),
            " ",
            "hover restyles only the seam cells"
        );
        assert_eq!(hover.buf[(6, y)].symbol(), " ");
    }
    // A horizontal rule for a stacked split.
    let rule = Rect::new(0, 2, 8, 1);
    let thin = drawn(SplitDir::Vertical, rule, Interaction::default(), false);
    assert_eq!(thin.buf[(3, 2)].symbol(), theme.glyphs.h_divider);
    let heavy = drawn(SplitDir::Vertical, rule, Interaction::default(), true);
    assert_eq!(heavy.buf[(3, 2)].symbol(), theme.glyphs.scroll_h_thumb);

    // Hit area: the seam grows by the pad on each side and is clipped.
    let mut seam = Splitter::new(id, SplitDir::Horizontal).hit_pad(2);
    assert_eq!(seam.hit_pad, 2);
    assert_eq!(
        seam.hit_area(Rect::new(0, 0, 12, 6)),
        Rect::ZERO,
        "nothing drawn yet"
    );
    let frame = paint_with(
        Rect::new(0, 0, 12, 6),
        Interaction::default(),
        |_, buf, ctx| seam.render(strip, buf, ctx, canvas),
    );
    assert_eq!(seam.area, strip);
    assert_eq!(seam.hit_area(Rect::new(0, 0, 12, 6)), Rect::new(3, 0, 5, 4));
    assert_eq!(seam.hit_area(Rect::new(0, 0, 6, 6)), Rect::new(3, 0, 3, 4));
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.owner == id && r.area.contains(Position::new(3, 1))),
        "a press two cells left of the seam still grabs it"
    );
    let mut flat = Splitter::new(id, SplitDir::Vertical).hit_pad(1);
    paint_with(
        Rect::new(0, 0, 12, 6),
        Interaction::default(),
        |_, buf, ctx| flat.render(rule, buf, ctx, canvas),
    );
    assert_eq!(flat.hit_area(Rect::new(0, 0, 12, 6)), Rect::new(0, 1, 8, 3));

    // Gestures: press fixes the grab offset, drag moves the seam, click resets.
    let container = Rect::new(0, 0, 41, 5);
    let mut split = Split::new(50, 4, 4);
    let mut seam = Splitter::new(id, SplitDir::Horizontal).hit_pad(1);
    seam.on_press(&split, container, 1, Position::new(19, 2));
    assert_eq!(
        seam.on_drag(&mut split, container, 1, Position::new(29, 2)),
        Outcome::Changed
    );
    assert_eq!(
        split.layout(SplitDir::Horizontal, container, 1).0.width,
        30,
        "the grab offset keeps the seam under the pointer"
    );
    assert_eq!(
        seam.on_drag(&mut split, container, 1, Position::new(29, 2)),
        Outcome::Consumed,
        "no movement, nothing changed"
    );
    let mut cold = Splitter::new(id, SplitDir::Horizontal);
    let mut split_b = Split::new(50, 4, 4);
    assert_eq!(
        cold.on_drag(&mut split_b, container, 1, Position::new(10, 2)),
        Outcome::Changed,
        "a drag without a press grabs where it starts"
    );

    assert_eq!(seam.on_click(&mut split, 1), Outcome::Consumed);
    assert_eq!(seam.on_click(&mut split, 2), Outcome::Changed, "reset");
    assert_eq!(split.percent, split.default_percent);
    assert_eq!(
        seam.on_click(&mut split, 2),
        Outcome::Consumed,
        "already at the default"
    );
    assert_eq!(seam.double_click, SeamDoubleClick::Reset);
    let mut ignoring =
        Splitter::new(id, SplitDir::Horizontal).double_click(SeamDoubleClick::Ignore);
    assert_eq!(ignoring.double_click, SeamDoubleClick::Ignore);
    let mut moved = Split::new(50, 4, 4);
    moved.drag_to(SplitDir::Horizontal, container, 1, Position::new(10, 2));
    let before = moved;
    assert_eq!(ignoring.on_click(&mut moved, 2), Outcome::Consumed);
    assert_eq!(
        moved, before,
        "an ignored double click leaves the split alone"
    );
}

fn layout_pane_nodes() {
    use ratatui::layout::Size;
    // Build `1 | (3 over 2)` by splitting leaves.
    let mut node = PaneNode::Leaf(1);
    assert!(node.split_leaf(1, 2, SplitDir::Horizontal, false));
    assert!(
        node.split_leaf(2, 3, SplitDir::Vertical, true),
        "new pane first"
    );
    assert!(
        !node.split_leaf(42, 9, SplitDir::Horizontal, false),
        "no such pane"
    );
    assert_eq!(node.leaves(), vec![1, 3, 2]);
    assert!(matches!(
        node,
        PaneNode::Split {
            dir: SplitDir::Horizontal,
            ..
        }
    ));

    let mut path = Vec::new();
    assert!(node.path_to(3, &mut path));
    assert_eq!(path, vec![1, 0]);
    assert!(!node.path_to(42, &mut Vec::new()));
    assert!(matches!(
        node.node_at_mut(&[]),
        Some(PaneNode::Split { .. })
    ));
    assert!(matches!(
        node.node_at_mut(&[1]),
        Some(PaneNode::Split {
            dir: SplitDir::Vertical,
            ..
        })
    ));
    assert!(matches!(node.node_at_mut(&[1, 0]), Some(PaneNode::Leaf(3))));
    assert!(
        node.node_at_mut(&[0, 0]).is_none(),
        "a leaf has no children"
    );

    // Layout lists every pane rectangle and every seam with its tree path.
    let (mut panes, mut seams, mut at) = (Vec::new(), Vec::new(), Vec::new());
    let area = Rect::new(0, 0, 41, 13);
    node.layout(area, 1, &mut panes, &mut seams, &mut at);
    assert_eq!(
        panes,
        vec![
            (1, Rect::new(0, 0, 20, 13)),
            (3, Rect::new(21, 0, 20, 6)),
            (2, Rect::new(21, 7, 20, 6)),
        ]
    );
    let seam_paths: Vec<(Vec<u8>, SplitDir, Rect)> = seams
        .iter()
        .map(|s: &Seam| (s.path.clone(), s.dir, s.handle))
        .collect();
    assert_eq!(
        seam_paths,
        vec![
            (vec![], SplitDir::Horizontal, Rect::new(20, 0, 1, 13)),
            (vec![1], SplitDir::Vertical, Rect::new(21, 6, 20, 1)),
        ]
    );
    assert_eq!(seams[1].container, Rect::new(21, 0, 20, 13));

    // The natural size grows with the panes' wishes and the seams between them.
    let natural = node.natural_size(1, &|id: PaneId| Size::new(10 + id as u16, 4));
    assert_eq!(natural, Size::new(26, 9));

    assert!(node.remove_leaf(3), "the sibling takes the split's place");
    assert_eq!(node.leaves(), vec![1, 2]);
    assert!(!node.remove_leaf(42));
    let mut lone = PaneNode::Leaf(7);
    assert!(!lone.remove_leaf(7), "the last pane cannot be removed");
}

fn tree_three() -> PaneTree {
    let mut tree = PaneTree::new(WidgetId::of("scenario.layout.panetree"), 1);
    assert!(tree.split(1, 2, SplitDir::Horizontal, false));
    assert!(tree.split(2, 3, SplitDir::Vertical, true));
    tree
}

fn pane_names(id: PaneId) -> String {
    match id {
        1 => "shell".to_owned(),
        2 => "logs".to_owned(),
        _ => "build".to_owned(),
    }
}

fn layout_pane_tree_focus() {
    let area = Rect::new(0, 0, 41, 13);
    let mut tree = tree_three();
    assert_eq!(tree.leaves(), vec![1, 3, 2]);
    assert_eq!(tree.focused, 3, "a new pane takes focus");
    assert!(!tree.split(42, 4, SplitDir::Horizontal, false));
    assert_eq!(tree.zoomed(), None);
    assert!(tree.framed(), "a split tree draws frames by default");
    tree.layout(area);
    assert_eq!(
        tree.panes(),
        &[
            (1, Rect::new(0, 0, 20, 13)),
            (3, Rect::new(21, 0, 20, 6)),
            (2, Rect::new(21, 7, 20, 6)),
        ]
    );
    assert_eq!(tree.seams().len(), 2);
    assert_ne!(tree.seam_id(0), tree.seam_id(1));

    // Spatial focus moves toward the nearest pane in that direction.
    let seen: PaneResult = tree.focus_toward(Direction::Left);
    assert_eq!(seen, Ok(Outcome::Changed));
    assert_eq!(tree.focused, 1);
    assert_eq!(tree.focus_toward(Direction::Right), Ok(Outcome::Changed));
    assert_eq!(tree.focused, 3);
    assert_eq!(
        tree.focus_toward(Direction::Up),
        Err(PaneRefusal::NoNeighbour)
    );
    assert_eq!(tree.focus_toward(Direction::Down), Ok(Outcome::Changed));
    assert_eq!(tree.focused, 2);
    let rects = tree.panes().to_vec();
    assert_eq!(nearest(&rects, 2, Direction::Up), Some(3));
    assert_eq!(nearest(&rects, 2, Direction::Left), Some(1));
    assert_eq!(nearest(&rects, 1, Direction::Right), Some(3));
    assert_eq!(nearest(&rects, 3, Direction::Down), Some(2));
    assert_eq!(nearest(&rects, 1, Direction::Left), None);
    assert_eq!(nearest(&rects, 99, Direction::Left), None);

    // Tab order cycles through the leaves.
    assert_eq!(tree.focus_next(false), Ok(Outcome::Changed));
    assert_eq!(tree.focused, 1);
    assert_eq!(tree.focus_next(true), Ok(Outcome::Changed));
    assert_eq!(tree.focused, 2);
    assert_eq!(
        PaneTree::new(WidgetId::of("scenario.layout.solo"), 1).focus_next(false),
        Err(PaneRefusal::NoNeighbour)
    );

    // Keyboard commands through the default keymap.
    let k = |code, mods| chord(code, mods);
    assert_eq!(
        tree.on_key(&k(KeyCode::F(6), KeyModifiers::NONE)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(tree.focused, 1);
    assert_eq!(
        tree.on_key(&k(KeyCode::F(6), KeyModifiers::SHIFT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(tree.focused, 2);
    assert_eq!(
        tree.on_key(&k(KeyCode::Up, KeyModifiers::ALT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(tree.focused, 3);
    assert_eq!(
        tree.on_key(&k(KeyCode::Down, KeyModifiers::ALT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(
        tree.on_key(&k(KeyCode::Left, KeyModifiers::ALT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(tree.focused, 1);
    assert_eq!(
        tree.on_key(&k(KeyCode::Right, KeyModifiers::ALT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(
        tree.on_key(&key(KeyCode::Char('q'))),
        None,
        "not a pane key"
    );
    for (cmd, label) in [
        (PaneCmd::Next, "Next pane"),
        (PaneCmd::Prev, "Previous pane"),
        (PaneCmd::FocusLeft, "Pane to the left"),
        (PaneCmd::FocusRight, "Pane to the right"),
        (PaneCmd::FocusUp, "Pane above"),
        (PaneCmd::FocusDown, "Pane below"),
        (PaneCmd::Zoom, "Zoom pane"),
    ] {
        assert_eq!(cmd.label(), label);
    }

    // Seam keys resize the pane's seam by the key step.
    tree.focused = 1;
    let grow = chord(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    assert_eq!(tree.on_key(&grow), Some(Ok(Outcome::Changed)));
    tree.layout(area);
    assert_eq!(tree.panes()[0].1.width, 22);
    assert_eq!(tree.resize(Direction::Left, 5), Ok(Outcome::Changed));
    tree.layout(area);
    assert_eq!(tree.panes()[0].1.width, 17);
    assert_eq!(
        tree.resize(Direction::Up, 1),
        Err(PaneRefusal::NothingToResize),
        "pane 1 has no horizontal seam above it"
    );
    assert_eq!(tree.resize(Direction::Left, 100), Ok(Outcome::Changed));
    tree.layout(area);
    assert_eq!(
        tree.panes()[0].1.width,
        12,
        "stops at the minimum pane width"
    );
    assert_eq!(tree.resize(Direction::Left, 1), Err(PaneRefusal::AtMinimum));

    // Zoom shows one pane and refuses navigation and resizing.
    tree.focused = 3;
    assert_eq!(tree.toggle_zoom(), Ok(Outcome::Changed));
    assert_eq!(tree.zoomed(), Some(3));
    tree.layout(area);
    assert_eq!(tree.panes(), &[(3, area)]);
    assert!(tree.seams().is_empty());
    assert_eq!(
        tree.focus_toward(Direction::Left),
        Err(PaneRefusal::ZoomedFocus)
    );
    assert_eq!(tree.focus_next(false), Err(PaneRefusal::ZoomedFocus));
    assert_eq!(
        tree.resize(Direction::Left, 1),
        Err(PaneRefusal::ZoomedResize)
    );
    assert_eq!(tree.toggle_zoom(), Ok(Outcome::Changed));
    assert_eq!(tree.zoomed(), None);
    assert_eq!(
        tree.on_key(&k(KeyCode::Char('z'), KeyModifiers::ALT)),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(tree.zoomed(), Some(3));
    tree.toggle_zoom().expect("unzoom");
    assert_eq!(
        PaneTree::new(WidgetId::of("scenario.layout.solo"), 1).toggle_zoom(),
        Err(PaneRefusal::OnePane)
    );
    for (refusal, message) in [
        (PaneRefusal::ZoomedFocus, "Unzoom to move focus"),
        (PaneRefusal::ZoomedResize, "Unzoom to resize"),
        (PaneRefusal::NoNeighbour, "No pane that way"),
        (
            PaneRefusal::NothingToResize,
            "Nothing to resize in that direction",
        ),
        (
            PaneRefusal::AtMinimum,
            "Cannot resize: pane at minimum size",
        ),
        (PaneRefusal::OnePane, "Nothing to zoom: one pane"),
    ] {
        assert_eq!(refusal.message(), message);
    }

    // Closing removes a pane and keeps a valid focus.
    assert_eq!(tree.close(42), Err(PaneRefusal::NoNeighbour));
    tree.focused = 3;
    assert_eq!(tree.close(3), Ok(()));
    assert_eq!(tree.leaves(), vec![1, 2]);
    assert_eq!(tree.focused, 1, "focus falls back to the first pane");
    assert_eq!(tree.close(1), Ok(()));
    assert_eq!(tree.close(2), Err(PaneRefusal::OnePane));
    assert_eq!(tree.leaves(), vec![2]);

    // Frame policy.
    let mut solo = PaneTree::new(WidgetId::of("scenario.layout.solo"), 1);
    assert!(!solo.framed());
    solo = solo.frames(Frames::Always);
    assert!(solo.framed());
    assert!(!tree_three().frames(Frames::Never).framed());
    assert!(tree_three().frames(Frames::WhenSplit).framed());
}

fn layout_pane_tree_seams() {
    let area = Rect::new(0, 0, 41, 13);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let id = WidgetId::of("scenario.layout.panetree");

    // Builders: minimum size, hit pad, key maps.
    let tree = tree_three()
        .min_size(4, 3)
        .hit_pad(0)
        .keys(vec![(KeyBinding::char('n'), PaneCmd::Next)])
        .split_keys(SplitKeys::default().step(3));
    assert_eq!((tree.min_size, tree.hit_pad), ((4, 3), 0));
    assert_eq!(tree.split_keys.step, 3);
    let mut keyed = tree.clone();
    assert_eq!(
        keyed.on_key(&key(KeyCode::Char('n'))),
        Some(Ok(Outcome::Changed))
    );
    assert_eq!(
        keyed.on_key(&key(KeyCode::F(6))),
        None,
        "the remap replaced F6"
    );
    let mut stepped = tree_three().split_keys(SplitKeys::default().step(3));
    stepped.focused = 1;
    stepped.layout(area);
    let grow = chord(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    assert_eq!(stepped.on_key(&grow), Some(Ok(Outcome::Changed)));
    stepped.layout(area);
    assert_eq!(
        stepped.panes()[0].1.width,
        23,
        "the seam key moves by the configured step"
    );

    // Natural size counts the frame cells on every pane.
    let wish = |_: PaneId| ratatui::layout::Size::new(10, 4);
    assert_eq!(tree.natural_size(&wish), ratatui::layout::Size::new(25, 13));
    let mut zoomed = tree.clone();
    zoomed.toggle_zoom().expect("zoom");
    assert_eq!(
        zoomed.natural_size(&wish),
        ratatui::layout::Size::new(12, 6)
    );
    assert_eq!(
        tree.clone().frames(Frames::Never).natural_size(&wish),
        ratatui::layout::Size::new(21, 9)
    );

    // Render: frames carry titles and the seams are drawn thin at rest.
    let mut tree = tree;
    let mut inner = Vec::new();
    let frame = paint_hits(area, |area, buf, ctx| {
        inner = tree.render(area, buf, ctx, theme.canvas, &pane_names);
    });
    let text = text_of(&frame.buf);
    for title in ["shell", "logs", "build"] {
        assert!(text.contains(title), "{title}:\n{text}");
    }
    assert_eq!(
        inner[0],
        (1, Rect::new(1, 1, 18, 11)),
        "content sits inside the frame"
    );
    assert_eq!(frame.buf[(20, 3)].symbol(), theme.glyphs.divider);
    assert_eq!(frame.buf[(20, 3)].fg, theme.ladders.seam.rest);

    // The hit pad widens the seam's grab area on each side.
    let grab_width = |frame: &Painted, tree: &PaneTree| {
        let seam = tree.seam_id(0);
        frame
            .hits
            .regions()
            .iter()
            .filter(|r| r.id == seam)
            .map(|r| r.area.width)
            .max()
    };
    let mut padded = tree_three();
    let padded_frame = paint_hits(area, |area, buf, ctx| {
        padded.render(area, buf, ctx, theme.canvas, &pane_names);
    });
    assert_eq!(
        grab_width(&frame, &tree),
        Some(1),
        "no pad, a one cell seam"
    );
    assert_eq!(
        grab_width(&padded_frame, &padded),
        Some(3),
        "the default pad adds a cell on each side"
    );
    let mut bare = tree.clone().frames(Frames::Never);
    let mut raw = Vec::new();
    let unframed = paint_hits(area, |area, buf, ctx| {
        raw = bare.render(area, buf, ctx, theme.canvas, &pane_names);
    });
    assert_eq!(raw[0], (1, Rect::new(0, 0, 20, 13)), "no frame, no inset");
    assert!(!text_of(&unframed.buf).contains("shell"));

    // Hover heavies the seam; a press holds it in the accent.
    let render_at = |tree: &mut PaneTree, interaction: Interaction| {
        paint_with(area, interaction, |area, buf, ctx| {
            tree.render(area, buf, ctx, ctx.theme.canvas, &pane_names);
        })
    };
    let seam0 = tree.seam_id(0);
    let hovered = render_at(
        &mut tree,
        Interaction {
            hover: Some(seam0),
            ..Default::default()
        },
    );
    assert_eq!(hovered.buf[(20, 3)].symbol(), theme.glyphs.scroll_thumb);
    assert_eq!(hovered.buf[(20, 3)].fg, theme.ladders.seam.hover);
    assert_eq!(
        hovered.buf[(40, 6)].symbol(),
        theme.glyphs.h_divider,
        "the other seam stays thin"
    );

    let at = |phase, x, y| {
        PointerEvent::new(
            &frame.hits,
            phase,
            seam0,
            Position::new(x, y),
            1,
            KeyModifiers::NONE,
        )
    };
    assert_eq!(
        tree.on_pointer(&at(Phase::Press, 20, 5)),
        Some(Outcome::Consumed)
    );
    let held = render_at(&mut tree, Interaction::default());
    assert_eq!(held.buf[(20, 3)].symbol(), theme.glyphs.scroll_thumb);
    assert_eq!(
        held.buf[(20, 3)].fg,
        theme.ladders.seam.pressed,
        "a held seam is accent"
    );
    assert_eq!(held.buf[(40, 6)].fg, theme.ladders.seam.rest);
    assert_eq!(
        tree.on_pointer(&at(Phase::Drag, 25, 5)),
        Some(Outcome::Changed)
    );
    tree.layout(area);
    assert_eq!(tree.panes()[0].1.width, 25);
    assert_eq!(
        tree.on_pointer(&at(Phase::Drag, 25, 5)),
        Some(Outcome::Consumed),
        "no movement"
    );
    assert_eq!(
        tree.on_pointer(&at(Phase::DragEnd, 25, 5)),
        Some(Outcome::Changed)
    );
    assert_eq!(
        tree.on_pointer(&at(Phase::Release, 25, 5)),
        Some(Outcome::Consumed)
    );
    let after = render_at(&mut tree, Interaction::default());
    assert_eq!(
        after.buf[(25, 3)].fg,
        theme.ladders.seam.rest,
        "released seams rest"
    );

    // A drag cannot squeeze a pane below the minimum size.
    let mut strict = tree_three().min_size(12, 3);
    let frame = paint_hits(area, |area, buf, ctx| {
        strict.render(area, buf, ctx, ctx.theme.canvas, &pane_names);
    });
    let seam = strict.seam_id(0);
    let at = |phase, x| {
        PointerEvent::new(
            &frame.hits,
            phase,
            seam,
            Position::new(x, 5),
            1,
            KeyModifiers::NONE,
        )
    };
    assert_eq!(
        strict.on_pointer(&at(Phase::Press, 20)),
        Some(Outcome::Consumed)
    );
    assert_eq!(
        strict.on_pointer(&at(Phase::Drag, 39)),
        Some(Outcome::Changed)
    );
    strict.layout(area);
    assert_eq!(
        strict.panes()[1].1.width,
        12,
        "the right column keeps its minimum"
    );
    strict.on_pointer(&at(Phase::Release, 39));

    // Double click resets the seam to its default.
    let dbl = PointerEvent::new(
        &frame.hits,
        Phase::Click,
        seam,
        Position::new(30, 5),
        2,
        KeyModifiers::NONE,
    );
    assert_eq!(strict.on_pointer(&dbl), Some(Outcome::Changed));
    strict.layout(area);
    assert_eq!(strict.panes()[0].1.width, 20);

    // Clicking a pane focuses it.
    let pane = PointerEvent::new(
        &frame.hits,
        Phase::Click,
        id.child(1),
        Position::new(3, 3),
        1,
        KeyModifiers::NONE,
    );
    strict.focused = 3;
    assert_eq!(strict.on_pointer(&pane), Some(Outcome::Changed));
    assert_eq!(strict.focused, 1);
    let stray = PointerEvent::new(
        &frame.hits,
        Phase::Click,
        WidgetId::of("scenario.layout.stray"),
        Position::new(3, 3),
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(strict.on_pointer(&stray), None);

    // Splitting mid-drag drops the grab: the old seam id goes dead.
    let mut live = tree_three();
    let frame = paint_hits(area, |area, buf, ctx| {
        live.render(area, buf, ctx, ctx.theme.canvas, &pane_names);
    });
    let seam = live.seam_id(0);
    let at = |phase, x| {
        PointerEvent::new(
            &frame.hits,
            phase,
            seam,
            Position::new(x, 5),
            1,
            KeyModifiers::NONE,
        )
    };
    live.on_pointer(&at(Phase::Press, 20));
    assert!(live.split(1, 4, SplitDir::Vertical, false));
    assert_eq!(
        live.on_pointer(&at(Phase::Drag, 30)),
        Some(Outcome::Consumed),
        "the stale gesture moves nothing"
    );

    // Hints name pane keys while unzoomed and the restore key while zoomed.
    let hint_text = |tree: &PaneTree| -> Vec<String> {
        tree.hints().iter().map(|h| h.action.to_string()).collect()
    };
    let solo = PaneTree::new(WidgetId::of("scenario.layout.solo"), 1);
    assert_eq!(hint_text(&solo), ["Next pane", "Resize"]);
    assert_eq!(hint_text(&tree_three()), ["Next pane", "Resize", "Zoom"]);
    let mut z = tree_three();
    z.toggle_zoom().expect("zoom");
    assert_eq!(hint_text(&z), ["Restore"]);
}

fn layout_column_solve() {
    // Constructors: content grows to its measure, fixed pins, builders refine.
    let content = Col::content(4, 10);
    assert_eq!(
        (
            content.min,
            content.ideal,
            content.max,
            content.flex,
            content.drop
        ),
        (4, 10, 10, 0, None)
    );
    assert_eq!(Col::content(6, 3).ideal, 6, "never below the minimum");
    let fixed = Col::fixed(6);
    assert_eq!((fixed.min, fixed.ideal, fixed.max), (6, 6, 6));
    let flexy = Col::content(4, 10).flex(1, 30);
    assert_eq!((flexy.flex, flexy.max), (1, 30));
    assert_eq!(
        Col::content(4, 10).flex(1, 5).max,
        10,
        "max never below ideal"
    );
    let capped = Col::fixed(5).max(9);
    assert_eq!(capped.max, 9);
    assert_eq!(Col::fixed(5).max(2).max, 5);
    let droppable = Col::content(3, 8).droppable(1);
    assert_eq!(droppable.drop, Some(1));

    // Plenty of room: every column reaches its ideal, flex takes the rest.
    let cols = [fixed, flexy, droppable];
    assert_eq!(solve(&cols, 40, 1), vec![6, 24, 8]);
    assert_eq!(
        solve(&cols, 40, 1).iter().sum::<u16>() + 2,
        40,
        "widths and gaps fill the width"
    );
    // Tight: the droppable column disappears (width 0) before others shrink.
    assert_eq!(solve(&cols, 14, 1), vec![6, 7, 0]);
    // Plain columns without flex stop at their max.
    assert_eq!(solve(&[capped], 20, 0), vec![9]);
    // Without droppable columns, the last column gives way first.
    assert_eq!(solve(&[fixed, Col::fixed(4)], 5, 1), vec![4, 0]);
    // The lowest drop priority leaves first; ties drop the rightmost.
    let ranked = [
        Col::fixed(5).droppable(2),
        Col::fixed(5).droppable(1),
        Col::fixed(5).droppable(1),
    ];
    assert_eq!(solve(&ranked, 11, 1), vec![5, 5, 0]);
    assert_eq!(solve(&ranked, 5, 1), vec![5, 0, 0]);
    assert_eq!(solve(&[], 10, 1), Vec::<u16>::new());
}

fn layout_column_sizing() {
    assert_eq!(MIN_USER_WIDTH, 3);
    assert_eq!(DEFAULT_SEAM_W, 2);
    let mut sizing = ColumnSizing::default();
    assert!(sizing.resizable);
    assert_eq!(
        (sizing.seam_w, sizing.min_width),
        (DEFAULT_SEAM_W, MIN_USER_WIDTH)
    );
    assert!(sizing.is_default());
    assert_eq!(sizing.user(1), None);
    assert_eq!(
        sizing.width(1, 9),
        9,
        "the measured width stands without an override"
    );

    sizing.set(1, 1);
    assert_eq!(
        sizing.user(1),
        Some(3),
        "a user width stays above the minimum"
    );
    assert_eq!(sizing.width(1, 9), 3);
    assert!(!sizing.is_default());
    sizing.set(3, 12);
    assert_eq!(sizing.user(3), Some(12));
    assert_eq!(sizing.user(2), None);
    let base = [Col::fixed(6), Col::fixed(6), Col::fixed(6), Col::fixed(6)];
    let applied = sizing.apply(&base);
    assert_eq!(
        applied.iter().map(|c| c.ideal).collect::<Vec<_>>(),
        vec![6, 3, 6, 12]
    );
    sizing.clear(1);
    assert_eq!(sizing.user(1), None);
    sizing.clear(99);
    assert!(!sizing.is_default(), "column 3 is still overridden");

    // A drag measures from where the seam was pressed.
    assert_eq!(sizing.dragging(), None);
    assert_eq!(sizing.drag_to(5), None, "no press, no drag");
    sizing.press(0, 6, 20);
    assert_eq!(sizing.dragging(), Some(0));
    assert_eq!(sizing.drag_to(24), Some((0, 10)));
    assert_eq!(sizing.user(0), Some(10));
    assert_eq!(sizing.drag_to(24), None, "same width, no change");
    assert_eq!(sizing.drag_to(0), Some((0, 3)), "never below the minimum");
    sizing.release();
    assert_eq!(sizing.dragging(), None);
    sizing.press(2, 6, 10);
    sizing.reset();
    assert_eq!(sizing.dragging(), None, "reset drops a held seam");
    assert!(sizing.is_default());

    let mut wide = ColumnSizing::default();
    wide.min_width = 8;
    wide.seam_w = 4;
    wide.resizable = false;
    wide.set(0, 2);
    assert_eq!(wide.user(0), Some(8), "the minimum is configurable");
    assert!(!wide.resizable);

    // The seam's hit rectangle straddles the column edge and is clipped.
    let limit = Rect::new(0, 0, 40, 5);
    assert_eq!(
        ColumnSizing::default().seam_rect(10, 2, 1, limit),
        Rect::new(9, 2, 2, 1)
    );
    assert_eq!(
        ColumnSizing::default().seam_rect(39, 2, 1, limit),
        Rect::new(38, 2, 2, 1)
    );
    assert_eq!(
        ColumnSizing::default().seam_rect(40, 2, 1, limit),
        Rect::new(39, 2, 1, 1),
        "clipped to the limit"
    );
    assert_eq!(wide.seam_rect(10, 2, 2, limit), Rect::new(9, 2, 4, 1));

    // Drawing: nothing at rest, the heavy glyph on hover, accent while held.
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let owner = WidgetId::of("scenario.layout.cols");
    let part = seam_part(2);
    assert_eq!(owner.part(part), owner.part(seam_part(2)));
    assert_ne!(owner.part(part), owner.part(seam_part(3)));
    let hit = Rect::new(9, 0, 2, 1);
    let draw = |interaction: Interaction, held: bool, hit: Rect| {
        paint_with(Rect::new(0, 0, 20, 2), interaction, |_, buf, ctx| {
            render_seam(buf, ctx, (owner, 2), hit, 10, theme.canvas, held)
        })
    };
    let rest = draw(Interaction::default(), false, hit);
    assert_eq!(
        rest.buf[(10, 0)].symbol(),
        " ",
        "a resting seam draws nothing"
    );
    assert!(
        rest.hits
            .regions()
            .iter()
            .any(|r| r.owner == owner && r.part == part && r.area == hit),
        "the seam still registers its hit region"
    );
    let hover = draw(
        Interaction {
            hover: Some(owner.part(part)),
            ..Default::default()
        },
        false,
        hit,
    );
    assert_eq!(hover.buf[(10, 0)].symbol(), theme.glyphs.scroll_thumb);
    assert_eq!(hover.buf[(10, 0)].fg, theme.ladders.seam.hover);
    let held = draw(Interaction::default(), true, hit);
    assert_eq!(held.buf[(10, 0)].fg, theme.ladders.seam.pressed);
    let none = draw(Interaction::default(), true, Rect::ZERO);
    assert!(
        none.hits.regions().is_empty(),
        "an empty seam registers nothing"
    );
    assert_eq!(none.buf[(10, 0)].symbol(), " ");
}

fn layout_column_header_menu() {
    let labels = HeaderLabels::default();
    assert_eq!(labels, HeaderLabels::DEFAULT);
    assert_eq!(labels.sort_asc, "Sort Ascending");
    let actions = |menu: &[(HeaderAction, junie_tui::widgets::menu::MenuItem)]| {
        menu.iter().map(|(a, _)| *a).collect::<Vec<_>>()
    };

    let all = header_menu(&labels, true, None, true, true);
    assert_eq!(
        actions(&all),
        vec![
            HeaderAction::SortAsc,
            HeaderAction::SortDesc,
            HeaderAction::ClearSort,
            HeaderAction::Autofit,
            HeaderAction::ResetWidths,
        ]
    );
    let item = |menu: &[(HeaderAction, junie_tui::widgets::menu::MenuItem)], a: HeaderAction| {
        menu.iter()
            .find(|(x, _)| *x == a)
            .expect("action")
            .1
            .clone()
    };
    assert!(!item(&all, HeaderAction::SortAsc).item.disabled);
    let unsorted = item(&all, HeaderAction::ClearSort);
    assert!(unsorted.item.disabled, "nothing to clear");
    assert_eq!(unsorted.item.reason.as_deref(), Some("Not sorted"));
    assert!(unsorted.separator_after, "resize items follow a rule");
    assert!(!item(&all, HeaderAction::ResetWidths).item.disabled);

    let asc = header_menu(&labels, true, Some(SortDir::Asc), true, false);
    assert_eq!(
        item(&asc, HeaderAction::SortAsc).item.reason.as_deref(),
        Some("Already sorted ascending")
    );
    assert!(!item(&asc, HeaderAction::SortDesc).item.disabled);
    assert!(!item(&asc, HeaderAction::ClearSort).item.disabled);
    assert_eq!(
        item(&asc, HeaderAction::ResetWidths).item.reason.as_deref(),
        Some("Columns have their default widths")
    );
    let desc = header_menu(&labels, true, Some(SortDir::Desc), true, true);
    assert_eq!(
        item(&desc, HeaderAction::SortDesc).item.reason.as_deref(),
        Some("Already sorted descending")
    );

    let fixed = header_menu(&labels, false, None, false, false);
    assert_eq!(
        actions(&fixed),
        vec![
            HeaderAction::SortAsc,
            HeaderAction::SortDesc,
            HeaderAction::ClearSort
        ],
        "a column that cannot resize offers no resize items"
    );
    for a in actions(&fixed) {
        assert_eq!(
            item(&fixed, a).item.reason.as_deref(),
            Some("This column does not sort")
        );
    }
    assert!(!item(&fixed, HeaderAction::ClearSort).separator_after);

    // Labels are overridable per app.
    let custom = HeaderLabels {
        sort_asc: "A to Z",
        autofit: "Fit",
        ..HeaderLabels::DEFAULT
    };
    let menu = header_menu(&custom, true, None, true, true);
    assert_eq!(item(&menu, HeaderAction::SortAsc).item.label, "A to Z");
    assert_eq!(item(&menu, HeaderAction::Autofit).item.label, "Fit");
    assert_eq!(
        item(&menu, HeaderAction::SortDesc).item.label,
        "Sort Descending"
    );
}

fn chip_set() -> Vec<Chip> {
    let mut locked = Chip::new("locked");
    locked.removable = false;
    vec![Chip::new("alpha"), Chip::new("beta"), locked]
}

fn chip_frame(bar: &mut ChipBar, width: u16) -> Painted {
    paint_hits(Rect::new(0, 0, width, 6), |area, buf, ctx| {
        bar.render(Rect::new(0, 0, area.width, 1), buf, ctx, ctx.theme.canvas)
    })
}

fn layout_chip_bar_actions() {
    let id = WidgetId::of("scenario.layout.chipbar");
    let fresh = Chip::new("alpha");
    assert!(fresh.enabled && fresh.removable && !fresh.error);
    let mut bar = ChipBar::new(id);
    bar.set_chips(chip_set());
    bar.set_lead(Some("match all".to_owned()));

    // Ids are stable parts of the bar's own id and all differ.
    let ids = [
        bar.chip_id(0),
        bar.chip_id(1),
        bar.close_id(0),
        bar.add_id(),
        bar.lead_id(),
        bar.more_id(),
        bar.menu_id(),
    ];
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }

    // Before a render nothing the bar owns can act.
    assert_eq!(bar.on_click(bar.chip_id(0)), (Outcome::Consumed, None));
    let frame = chip_frame(&mut bar, 80);
    let text = text_of(&frame.buf);
    for shown in ["match all", "alpha", "beta", "locked", "+ Add filter"] {
        assert!(text.contains(shown), "{shown}:\n{text}");
    }
    let owned = |target| {
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == target && !r.area.is_empty())
    };
    assert!(
        owned(bar.chip_id(0))
            && owned(bar.close_id(0))
            && owned(bar.add_id())
            && owned(bar.lead_id())
    );
    assert!(!owned(bar.close_id(2)), "a locked chip has no close button");
    assert!(!owned(bar.more_id()), "nothing overflows at this width");

    // Clicks resolve to typed actions.
    assert_eq!(
        bar.on_click(bar.chip_id(1)),
        (Outcome::Changed, Some(ChipAction::Activate(1)))
    );
    assert_eq!(bar.cursor, 1);
    assert_eq!(
        bar.on_click(bar.close_id(0)),
        (Outcome::Changed, Some(ChipAction::Remove(0)))
    );
    assert_eq!(
        bar.on_click(bar.add_id()),
        (Outcome::Changed, Some(ChipAction::Add))
    );
    assert_eq!(bar.cursor, 3, "the add button is the last stop");
    assert_eq!(
        bar.on_click(bar.lead_id()),
        (Outcome::Changed, Some(ChipAction::Lead))
    );
    assert_eq!(
        bar.on_click(WidgetId::of("scenario.layout.nothing")),
        (Outcome::Ignored, None)
    );

    // Keys: navigation moves the cursor, commands emit actions.
    bar.cursor = 0;
    assert_eq!(bar.on_key(&key(KeyCode::Right)), (Outcome::Changed, None));
    assert_eq!(bar.cursor, 1);
    assert_eq!(bar.on_key(&key(KeyCode::Left)), (Outcome::Changed, None));
    assert_eq!(
        bar.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(ChipAction::Activate(0)))
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Changed, Some(ChipAction::Toggle(0)))
    );
    for removal in [KeyCode::Delete, KeyCode::Backspace, KeyCode::Char('x')] {
        assert_eq!(
            bar.on_key(&key(removal)),
            (Outcome::Changed, Some(ChipAction::Remove(0))),
            "{removal:?} removes the chip under the cursor"
        );
    }
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('+'))),
        (Outcome::Changed, Some(ChipAction::Add))
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('X'))),
        (Outcome::Changed, Some(ChipAction::ClearAll))
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('m'))),
        (Outcome::Changed, Some(ChipAction::Lead))
    );
    bar.cursor = 2;
    assert_eq!(
        bar.on_key(&key(KeyCode::Delete)),
        (Outcome::Ignored, None),
        "a locked chip cannot be removed"
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('2'))),
        (Outcome::Ignored, None),
        "digits toggle only when asked to"
    );
    let mut bare = ChipBar::new(WidgetId::of("scenario.layout.bare"));
    assert_eq!(bare.on_key(&key(KeyCode::Enter)).1, Some(ChipAction::Add));
    bare.set_add_label(None);
    assert_eq!(bare.on_key(&key(KeyCode::Enter)), (Outcome::Ignored, None));
    assert_eq!(
        bare.on_key(&key(KeyCode::Char('m'))),
        (Outcome::Ignored, None),
        "no lead, no lead command"
    );
    for (cmd, label) in [
        (ChipCmd::Remove, "Remove"),
        (ChipCmd::Add, "Add"),
        (ChipCmd::ClearAll, "Clear all"),
        (ChipCmd::ShowHidden, "Show hidden"),
        (ChipCmd::Lead, "Switch the match mode"),
    ] {
        assert_eq!(cmd.label(), label);
    }

    // Digits toggle the numbered chip and move the cursor to it.
    let mut digits = ChipBar::new(WidgetId::of("scenario.layout.digits")).digit_toggles(true);
    digits.set_chips(chip_set());
    assert!(digits.digit_toggles);
    assert_eq!(
        digits.on_key(&key(KeyCode::Char('2'))),
        (Outcome::Changed, Some(ChipAction::Toggle(1)))
    );
    assert_eq!(digits.cursor, 1);
    assert_eq!(
        digits.on_key(&key(KeyCode::Char('9'))),
        (Outcome::Ignored, None)
    );

    // A key map replaces the defaults; a nav profile adds vim keys.
    let mut remapped = ChipBar::new(WidgetId::of("scenario.layout.remap"))
        .keymap(vec![(KeyBinding::char('d'), ChipCmd::Remove)])
        .nav(NavKeys::VIM);
    remapped.set_chips(chip_set());
    chip_frame(&mut remapped, 80);
    assert_eq!(remapped.keymap.len(), 1);
    assert_eq!(
        remapped.on_key(&key(KeyCode::Char('d'))),
        (Outcome::Changed, Some(ChipAction::Remove(0)))
    );
    assert_eq!(
        remapped.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(
        remapped.on_key(&key(KeyCode::Char('l'))),
        (Outcome::Changed, None)
    );
    assert_eq!(remapped.cursor, 1, "vim l moves right");
    assert_eq!(bar.nav, NavKeys::DESKTOP);
    assert_eq!(remapped.nav, NavKeys::VIM);

    // Context items follow the chip: toggle wording, locked removal, bulk clear.
    let items = bar.context_items(0);
    assert_eq!(
        items.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
        vec![
            ChipAction::Toggle(0),
            ChipAction::Remove(0),
            ChipAction::ClearAll
        ]
    );
    assert_eq!(items[0].1.item.label, "Turn off");
    assert!(!items[1].1.item.disabled);
    let locked = bar.context_items(2);
    assert_eq!(
        locked[1].1.item.reason.as_deref(),
        Some("This filter stays")
    );
    assert!(locked[1].1.item.disabled);
    bar.update_chip(0, |chip| chip.enabled = false);
    assert_eq!(bar.context_items(0)[0].1.item.label, "Turn on");
    assert!(bar.context_items(99).is_empty());

    // Shift+F10 opens the chip's menu; choosing an item emits its action.
    chip_frame(&mut bar, 80);
    bar.cursor = 1;
    let menu_key = chord(KeyCode::F(10), KeyModifiers::SHIFT);
    assert_eq!(bar.on_key(&menu_key), (Outcome::Changed, None));
    assert!(bar.menu_open());
    let open = chip_frame(&mut bar, 80);
    assert!(
        text_of(&open.buf).contains("Turn off"),
        "{}",
        text_of(&open.buf)
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(ChipAction::Toggle(1)))
    );
    assert!(!bar.menu_open());
    bar.on_key(&menu_key);
    assert!(bar.menu_open());
    bar.on_key(&key(KeyCode::Esc));
    assert!(!bar.menu_open(), "Esc closes the menu without an action");
}

fn many_chips() -> Vec<Chip> {
    ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"]
        .iter()
        .map(|l| Chip::new(l))
        .collect()
}

fn layout_chip_bar_overflow() {
    // The run helper: how many chips fit, and the `+N` marker for the rest.
    assert_eq!(overflow_run(&[5, 5, 5], 100, 1), (3, None));
    assert_eq!(
        overflow_run(&[10, 10, 10, 10], 25, 1),
        (2, Some("+2".to_owned()))
    );
    assert_eq!(overflow_run(&[], 10, 1), (0, None));
    assert_eq!(
        overflow_run(&[30, 30], 3, 1),
        (0, Some("+2".to_owned())),
        "only the marker fits"
    );
    assert_eq!(
        overflow_run(&[30, 30], 1, 1),
        (0, None),
        "not even the marker fits"
    );

    let mut bar = ChipBar::new(WidgetId::of("scenario.layout.overflow"));
    bar.set_chips(many_chips());
    bar.set_add_label(None);
    let frame = chip_frame(&mut bar, 36);
    let text = text_of(&frame.buf);
    assert!(text.contains("alpha") && text.contains("beta"), "{text}");
    assert!(!text.contains("zeta"), "the last chip is hidden:\n{text}");
    assert!(text.contains("+3") || text.contains("+4"), "{text}");
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == bar.more_id() && !r.area.is_empty()),
        "the overflow marker is a button"
    );

    // Down opens the hidden-chip menu; choosing a row activates that chip.
    assert!(!bar.menu_open());
    assert_eq!(bar.on_key(&key(KeyCode::Down)), (Outcome::Changed, None));
    assert!(bar.menu_open());
    let open = chip_frame(&mut bar, 36);
    assert!(
        text_of(&open.buf).contains("zeta"),
        "{}",
        text_of(&open.buf)
    );
    let (_, action) = bar.on_key(&key(KeyCode::Enter));
    let Some(ChipAction::Activate(hidden)) = action else {
        panic!("a hidden chip row activates its chip: {action:?}");
    };
    assert!(hidden >= 2, "the first hidden chip");
    assert!(!bar.menu_open());
    assert_eq!(bar.cursor, hidden);

    // Clicking the marker toggles the same menu.
    chip_frame(&mut bar, 36);
    assert_eq!(bar.on_click(bar.more_id()), (Outcome::Changed, None));
    assert!(bar.menu_open());
    chip_frame(&mut bar, 36);
    assert_eq!(bar.on_click(bar.more_id()), (Outcome::Changed, None));
    assert!(!bar.menu_open());

    // Without the overflow menu the marker is plain text and Down opens nothing.
    let mut plain = ChipBar::new(WidgetId::of("scenario.layout.plainmark")).overflow_menu(false);
    plain.set_chips(many_chips());
    plain.set_add_label(None);
    assert!(!plain.overflow_menu);
    let frame = chip_frame(&mut plain, 36);
    assert!(text_of(&frame.buf).contains('+'));
    assert!(
        !frame.hits.regions().iter().any(|r| r.id == plain.more_id()),
        "no button, no region"
    );
    plain.on_key(&key(KeyCode::Down));
    assert!(!plain.menu_open());

    // Without the marker nothing announces the hidden chips.
    let mut silent = ChipBar::new(WidgetId::of("scenario.layout.silent")).overflow_marker(false);
    silent.set_chips(many_chips());
    silent.set_add_label(None);
    assert!(!silent.overflow_marker);
    let text = text_of(&chip_frame(&mut silent, 36).buf);
    assert!(!text.contains("+3") && !text.contains("+4"), "{text}");
    silent.on_key(&key(KeyCode::Down));
    assert!(!silent.menu_open());

    // The close button and the overflow button are replaceable.
    let width_of = |bar: &mut ChipBar| {
        let frame = chip_frame(bar, 80);
        frame
            .hits
            .regions()
            .iter()
            .find(|r| r.id == bar.close_id(0))
            .map(|r| r.area.width)
            .expect("a close region")
    };
    let mut default_close = ChipBar::new(WidgetId::of("scenario.layout.close.a"));
    default_close.set_chips(many_chips());
    let mut roomy = ChipBar::new(WidgetId::of("scenario.layout.close.b")).close_target_w(5);
    roomy.set_chips(many_chips());
    assert!(width_of(&mut roomy) > width_of(&mut default_close));
    let mut custom = ChipBar::new(WidgetId::of("scenario.layout.close.c"))
        .close_button(IconButton::new(WidgetId::of("scenario.layout.x"), "x").pad(2));
    custom.set_chips(many_chips());
    assert_eq!(
        width_of(&mut custom),
        5,
        "one glyph plus two pads each side"
    );
    let hovered_more = |bar: &mut ChipBar| {
        let hover = Interaction {
            hover: Some(bar.more_id()),
            ..Default::default()
        };
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(Rect::new(0, 0, 36, 1));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, hover, &mut hits, &mut ring);
        bar.render(Rect::new(0, 0, 36, 1), &mut buf, &mut ctx, theme.canvas);
        let frame = Painted { buf, hits };
        let at = (0..36)
            .find(|&x| frame.buf[(x, 0)].symbol() == "+")
            .unwrap_or_else(|| panic!("the marker:\n{}", text_of(&frame.buf)));
        frame.buf[(at, 0)].style()
    };
    let mut stock = ChipBar::new(WidgetId::of("scenario.layout.more.a"));
    stock.set_chips(many_chips());
    stock.set_add_label(None);
    let mut styled = ChipBar::new(WidgetId::of("scenario.layout.more.b"))
        .more_button(IconButton::new(WidgetId::of("scenario.layout.more"), "+").danger_hover(true));
    styled.set_chips(many_chips());
    styled.set_add_label(None);
    assert_ne!(
        hovered_more(&mut stock),
        hovered_more(&mut styled),
        "the replacement button decides how the marker reacts to hover"
    );
}

fn tool_items() -> Vec<ToolItem> {
    vec![
        ToolItem::new("Run").shortcut("^R").tip("Run the query"),
        ToolItem::new("Stop").reason("No query is running"),
        ToolItem::new("Wrap").toggle(false).separator(),
        ToolItem::new("Save").disabled(true),
        ToolItem::new("Open"),
    ]
}

fn toolbar_frame(bar: &mut Toolbar, width: u16) -> Painted {
    paint_hits(Rect::new(0, 0, width, 6), |area, buf, ctx| {
        bar.render(Rect::new(0, 0, area.width, 1), buf, ctx, ctx.theme.canvas)
    })
}

fn layout_toolbar_actions() {
    let id = WidgetId::of("scenario.layout.toolbar");
    let theme = Theme::for_level(ColorLevel::TrueColor);

    // Item builders set the data the toolbar acts on.
    let items = tool_items();
    assert_eq!(items[0].shortcut.as_deref(), Some("^R"));
    assert_eq!(items[0].tip.as_deref(), Some("Run the query"));
    assert!(items[1].disabled, "a reason disables the item");
    assert_eq!(items[1].reason.as_deref(), Some("No query is running"));
    assert_eq!(items[2].on, Some(false));
    assert!(items[2].separator_after);
    assert!(items[3].disabled && items[3].reason.is_none());
    assert!(!items[4].disabled && items[4].on.is_none());

    let mut bar = Toolbar::new(id, items);
    assert_eq!(bar.cursor, 0, "the cursor starts on the first enabled item");
    assert!(bar.skip_disabled && !bar.wrap && bar.overflow);
    assert_eq!(bar.gap, 1);
    assert_eq!(
        Toolbar::new(
            id,
            vec![ToolItem::new("a").disabled(true), ToolItem::new("b")]
        )
        .cursor,
        1
    );

    // Natural width: the full run fits exactly, one cell less overflows.
    let natural = bar.width();
    let frame = toolbar_frame(&mut bar, natural);
    assert!(
        bar.hidden().is_empty(),
        "the natural width shows everything"
    );
    let text = text_of(&frame.buf);
    for shown in ["Run", "^R", "Stop", "Wrap", "Save", "Open"] {
        assert!(text.contains(shown), "{shown}:\n{text}");
    }
    assert!(text.contains("○"), "an off toggle shows its mark:\n{text}");
    assert!(
        text.contains(theme.glyphs.divider),
        "a separator follows Wrap:\n{text}"
    );
    assert_eq!(bar.areas.len(), 5);
    assert!(bar.areas.iter().all(|r| !r.is_empty()));
    assert!(!bar.on_more());
    toolbar_frame(&mut bar, natural - 1);
    assert!(!bar.hidden().is_empty(), "one cell less hides an item");
    toolbar_frame(&mut bar, natural);

    // Ids: every item has its own, and none is the More button.
    let ids: Vec<_> = (0..5).map(|i| bar.item_id(i)).collect();
    for (i, a) in ids.iter().enumerate() {
        assert_ne!(*a, bar.more_id());
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_ne!(bar.menu_id(), bar.more_id());

    // Clicks.
    assert_eq!(
        bar.on_click(bar.item_id(0)),
        (Outcome::Changed, Some(ToolbarAction::Activated(0)))
    );
    assert_eq!(
        bar.on_click(bar.item_id(1)),
        (Outcome::Consumed, None),
        "a disabled item eats the click"
    );
    assert_eq!(
        bar.on_click(bar.item_id(2)),
        (Outcome::Changed, Some(ToolbarAction::Toggled(2, true)))
    );
    assert_eq!(bar.items()[2].on, Some(true), "the toggle flipped");
    assert_eq!(
        bar.on_click(WidgetId::of("scenario.layout.elsewhere")),
        (Outcome::Ignored, None)
    );
    toolbar_frame(&mut bar, natural);
    assert_eq!(
        bar.on_click(bar.item_id(2)),
        (Outcome::Changed, Some(ToolbarAction::Toggled(2, false)))
    );

    // Keys: navigation skips disabled items, Enter and Space activate.
    toolbar_frame(&mut bar, natural);
    bar.cursor = 0;
    assert_eq!(bar.on_key(&key(KeyCode::Right)).0, Outcome::Changed);
    assert_eq!(bar.cursor, 2, "Stop is disabled and skipped");
    assert_eq!(
        bar.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(ToolbarAction::Toggled(2, true)))
    );
    toolbar_frame(&mut bar, natural);
    bar.cursor = 0;
    assert_eq!(
        bar.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Changed, Some(ToolbarAction::Activated(0)))
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('q'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(
        Toolbar::new(id, Vec::new()).on_key(&key(KeyCode::Enter)),
        (Outcome::Ignored, None)
    );

    // The walk is configurable: stop on disabled items, wrap, vim keys.
    let mut stops = Toolbar::new(id, tool_items()).skip_disabled(false);
    assert!(!stops.skip_disabled);
    toolbar_frame(&mut stops, natural);
    stops.on_key(&key(KeyCode::Right));
    assert_eq!(stops.cursor, 1, "a disabled item is a stop");
    assert_eq!(
        stops.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, None)
    );
    let mut edge = Toolbar::new(id, tool_items());
    toolbar_frame(&mut edge, natural);
    edge.on_key(&key(KeyCode::Left));
    assert_eq!(edge.cursor, 0, "the first item stops the walk");
    let mut wrapped = Toolbar::new(id, tool_items()).wrap(true);
    assert!(wrapped.wrap);
    toolbar_frame(&mut wrapped, natural);
    wrapped.on_key(&key(KeyCode::Left));
    assert_eq!(wrapped.cursor, 4, "the walk wraps to the last item");
    let mut vim = Toolbar::new(id, tool_items()).nav(NavKeys::VIM);
    assert_eq!(vim.nav, NavKeys::VIM);
    toolbar_frame(&mut vim, natural);
    vim.on_key(&key(KeyCode::Char('l')));
    assert_eq!(vim.cursor, 2, "vim l walks right");

    // The gap is the spacing between items.
    let mut tight = Toolbar::new(id, tool_items()).gap(0);
    let mut airy = Toolbar::new(id, tool_items()).gap(3);
    assert_eq!(tight.gap, 0);
    toolbar_frame(&mut tight, 120);
    toolbar_frame(&mut airy, 120);
    assert_eq!(tight.areas[1].x - tight.areas[0].right(), 0);
    assert_eq!(airy.areas[1].x - airy.areas[0].right(), 3);
    assert!(airy.width() > tight.width());

    // A tip shows on hover.
    let hover = Interaction {
        hover: Some(bar.item_id(0)),
        ..Default::default()
    };
    let tipped = paint_with(Rect::new(0, 0, 60, 6), hover, |area, buf, ctx| {
        bar.render(Rect::new(0, 0, area.width, 1), buf, ctx, ctx.theme.canvas)
    });
    assert!(
        text_of(&tipped.buf).contains("Run the query"),
        "{}",
        text_of(&tipped.buf)
    );
}

fn layout_toolbar_overflow() {
    let id = WidgetId::of("scenario.layout.toolbar.more");
    let mut bar = Toolbar::new(id, tool_items());
    let frame = toolbar_frame(&mut bar, 30);
    assert!(
        !bar.hidden().is_empty(),
        "five items do not fit in 30 cells"
    );
    let hidden = bar.hidden().to_vec();
    assert!(hidden.contains(&4), "the last item is hidden: {hidden:?}");
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == bar.more_id() && !r.area.is_empty()),
        "the More button is a hit region"
    );
    assert!(!text_of(&frame.buf).contains("Open"));

    // The cursor walks onto the More stop and Enter opens the hidden items.
    for _ in 0..6 {
        bar.on_key(&key(KeyCode::Right));
        if bar.on_more() {
            break;
        }
    }
    assert!(bar.on_more(), "the walk reaches the More stop");
    assert_eq!(bar.on_key(&key(KeyCode::Enter)), (Outcome::Changed, None));
    assert!(bar.menu_open());
    let open = toolbar_frame(&mut bar, 30);
    assert!(
        text_of(&open.buf).contains("Open"),
        "{}",
        text_of(&open.buf)
    );
    let (_, action) = bar.on_key(&key(KeyCode::Enter));
    assert!(
        matches!(
            action,
            Some(ToolbarAction::Activated(_) | ToolbarAction::Toggled(..))
        ),
        "the first enabled hidden row activates: {action:?}"
    );
    assert!(!bar.menu_open());
    assert!(bar.on_more(), "the cursor rests on the More button");

    // Clicking the More button toggles the same menu.
    toolbar_frame(&mut bar, 30);
    assert_eq!(bar.on_click(bar.more_id()), (Outcome::Changed, None));
    assert!(bar.menu_open());
    toolbar_frame(&mut bar, 30);
    assert_eq!(bar.on_click(bar.more_id()), (Outcome::Changed, None));
    assert!(!bar.menu_open());
    bar.on_key(&key(KeyCode::Enter));
    toolbar_frame(&mut bar, 30);
    bar.on_key(&key(KeyCode::Esc));
    assert!(!bar.menu_open(), "Esc closes the menu");

    // A hidden toggle shows its mark in the menu.
    let mut toggles = Toolbar::new(
        id,
        vec![
            ToolItem::new("Aaaaaaaa"),
            ToolItem::new("Bbbbbbbb"),
            ToolItem::new("Cccc").toggle(true),
        ],
    );
    toolbar_frame(&mut toggles, 22);
    assert!(toggles.hidden().contains(&2), "{:?}", toggles.hidden());
    toggles.on_click(toggles.more_id());
    let menu = toolbar_frame(&mut toggles, 22);
    assert!(
        text_of(&menu.buf).contains("● Cccc"),
        "{}",
        text_of(&menu.buf)
    );

    // A hidden disabled item says why while the menu cursor rests on it.
    let mut why = Toolbar::new(
        id,
        vec![
            ToolItem::new("Aaaaaaaa"),
            ToolItem::new("Bbbbbbbb"),
            ToolItem::new("Cccc").reason("Offline"),
        ],
    );
    toolbar_frame(&mut why, 22);
    assert!(why.hidden().contains(&2), "{:?}", why.hidden());
    why.on_click(why.more_id());
    toolbar_frame(&mut why, 22);
    why.on_key(&key(KeyCode::Down));
    let menu = toolbar_frame(&mut why, 22);
    assert!(
        text_of(&menu.buf).contains("Offline"),
        "{}",
        text_of(&menu.buf)
    );

    // The More glyph and button are replaceable.
    let mut glyph = Toolbar::new(id, tool_items()).more_glyph("»");
    let text = text_of(&toolbar_frame(&mut glyph, 30).buf);
    assert!(text.contains('»'), "{text}");
    let mut button = Toolbar::new(id, tool_items())
        .more_button(IconButton::new(WidgetId::of("scenario.layout.more"), "~").pad(2));
    let text = text_of(&toolbar_frame(&mut button, 30).buf);
    assert!(text.contains('~'), "{text}");

    // Overflow off: nothing is hidden behind a button, the bar just clips.
    let mut clipped = Toolbar::new(id, tool_items()).overflow(false);
    assert!(!clipped.overflow);
    let frame = toolbar_frame(&mut clipped, 30);
    assert!(clipped.hidden().is_empty());
    assert!(
        !frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == clipped.more_id()),
        "no More button without overflow"
    );
    assert!(!text_of(&frame.buf).contains("Open"));
}

fn pipeline() -> Vec<Step> {
    ["Checkout", "Build", "Test", "Deploy"]
        .iter()
        .map(|l| Step::new(l))
        .collect()
}

fn step_frame(rail: &mut StepRail, area: Rect, tick: u64) -> Painted {
    let interaction = Interaction {
        tick,
        ..Default::default()
    };
    paint_with(area, interaction, |area, buf, ctx| {
        rail.render(area, buf, ctx, ctx.theme.canvas)
    })
}

fn layout_step_rail() {
    let id = WidgetId::of("scenario.layout.steps");
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let step = Step::new("Checkout");
    assert_eq!(
        (step.state, step.meta.as_deref()),
        (StepState::Queued, None)
    );

    // State words and terminal-ness.
    for (state, label, terminal) in [
        (StepState::Queued, "queued", false),
        (StepState::Running, "running", false),
        (StepState::Done, "done", true),
        (StepState::Skipped, "skipped", true),
        (StepState::Failed, "failed", true),
        (StepState::Blocked, "blocked", false),
    ] {
        assert_eq!(state.label(), label);
        assert_eq!(state.terminal(), terminal, "{label}");
    }

    let mut rail = StepRail::new(id, pipeline());
    assert!(!rail.selectable && rail.numbered);
    assert_eq!(rail.frontier(), Some(0));
    assert_eq!(rail.counts(), (0, 0, 0));
    assert_eq!(rail.failed(), None);

    // Progress moves the frontier and the counters.
    rail.set_state(0, StepState::Done);
    rail.set_state(1, StepState::Running);
    rail.set_meta(1, Some("2.1s".to_owned()));
    rail.set_state(2, StepState::Skipped);
    rail.set_state(99, StepState::Failed);
    rail.set_meta(99, None);
    assert_eq!(rail.frontier(), Some(1));
    assert_eq!(rail.counts(), (1, 1, 0));
    assert_eq!(rail.failed(), None);
    let area = Rect::new(0, 0, 40, 4);
    let frame = step_frame(&mut rail, area, 0);
    let rows = rows(&frame.buf);
    assert!(
        rows[0].contains("01") && rows[0].contains("Checkout"),
        "{rows:?}"
    );
    assert!(
        rows[0].contains(theme.glyphs.done),
        "a done step is ticked: {rows:?}"
    );
    assert!(
        rows[1].contains("Build") && rows[1].contains("2.1s"),
        "{rows:?}"
    );
    assert!(
        rows[2].contains("skipped"),
        "a skipped step says so: {rows:?}"
    );
    assert!(
        rows[3].contains("queued"),
        "a queued step says so: {rows:?}"
    );
    let spin_a = frame.buf[(1, 1)].symbol().to_owned();
    let spin_b = step_frame(&mut rail, area, 3).buf[(1, 1)]
        .symbol()
        .to_owned();
    assert_ne!(spin_a, spin_b, "a running step spins with the tick");
    rail.set_state(3, StepState::Failed);
    rail.set_state(2, StepState::Blocked);
    assert_eq!(rail.failed(), Some(3));
    assert_eq!(rail.counts(), (1, 0, 1));
    let rows = rows_of(&step_frame(&mut rail, area, 0).buf);
    assert!(rows[2].contains("blocked"), "{rows:?}");
    assert!(rows[3].contains('!'), "a failed step is marked: {rows:?}");
    rail.numbered = false;
    let plain = rows_of(&step_frame(&mut rail, area, 0).buf);
    assert!(!plain[0].contains("01"), "numbers are optional: {plain:?}");

    // A static rail ignores input and registers no rows.
    assert_eq!(rail.on_key(&key(KeyCode::Down)), Outcome::Ignored);
    assert_eq!(rail.on_click(1), Outcome::Consumed);
    assert_eq!(rail.cursor, 0);
    let frame = step_frame(&mut rail, area, 0);
    assert!(!frame.hits.regions().iter().any(|r| r.id == rail.row_id(1)));

    // A selectable rail walks and clicks.
    let mut pick = StepRail::new(id, pipeline()).selectable(true);
    assert!(pick.selectable);
    let frame = step_frame(&mut pick, area, 0);
    for i in 0..4 {
        assert!(
            frame.hits.regions().iter().any(|r| r.id == pick.row_id(i)),
            "row {i} is a hit region"
        );
    }
    assert_ne!(pick.row_id(0), pick.row_id(1));
    assert_eq!(pick.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(pick.cursor, 1);
    assert_eq!(pick.on_key(&key(KeyCode::Char('x'))), Outcome::Ignored);
    assert_eq!(
        pick.on_key(&chord(KeyCode::Down, KeyModifiers::CONTROL)),
        Outcome::Ignored,
        "a chord is not navigation"
    );
    assert_eq!(pick.on_click(3), Outcome::Changed);
    assert_eq!(pick.cursor, 3);
    assert_eq!(pick.on_click(9), Outcome::Consumed, "no such row");
    let click = frame.event(Phase::Click, id, Part::ROW, Position::new(2, 2));
    assert_eq!(pick.pointer(&click), Response::<()>::changed());
    assert_eq!(pick.cursor, 2);
    let mut empty = StepRail::new(id, Vec::new()).selectable(true);
    assert_eq!(empty.on_key(&key(KeyCode::Down)), Outcome::Ignored);

    // The walk keys are configurable.
    let mut vim = StepRail::new(id, pipeline())
        .selectable(true)
        .nav(NavKeys::VIM);
    assert_eq!(vim.on_key(&key(KeyCode::Char('j'))), Outcome::Changed);
    assert_eq!(vim.cursor, 1);
    let mut desk = StepRail::new(id, pipeline()).selectable(true);
    assert_eq!(desk.on_key(&key(KeyCode::Char('j'))), Outcome::Ignored);

    // A rail that is loading or failed shows that instead of rows.
    let mut loading = StepRail::new(id, pipeline()).content(ContentState::loading("Preparing"));
    let text = text_of(&step_frame(&mut loading, area, 0).buf);
    assert!(
        text.contains("Preparing") && !text.contains("Checkout"),
        "{text}"
    );
    let mut broken = StepRail::new(id, pipeline())
        .content(ContentState::error(EmptyState::new("Pipeline unavailable")));
    let text = text_of(&step_frame(&mut broken, area, 0).buf);
    assert!(text.contains("Pipeline unavailable"), "{text}");
}

fn rows_of(buf: &Buffer) -> Vec<String> {
    rows(buf)
}

fn layout_step_rail_scroll() {
    let id = WidgetId::of("scenario.layout.steps.scroll");
    let long = || -> Vec<Step> { (1..=12).map(|n| Step::new(&format!("Step {n}"))).collect() };
    let area = Rect::new(0, 0, 30, 4);
    let bar = junie_tui::widgets::scrollbar::id_for(id);
    let press_bottom = |rail: &mut StepRail| {
        let frame = step_frame(rail, area, 0);
        let ev = PointerEvent::new(
            &frame.hits,
            Phase::Press,
            bar,
            Position::new(29, 3),
            1,
            KeyModifiers::NONE,
        );
        let _ = rail.pointer(&ev);
        rail.scroll.offset
    };
    let mut paged = StepRail::new(id, long());
    let mut jumped = StepRail::new(id, long()).track_click(TrackClick::Jump);
    assert_eq!(jumped.core.scroll.track_click, Some(TrackClick::Jump));
    let (page, jump) = (press_bottom(&mut paged), press_bottom(&mut jumped));
    assert!(page > 0, "a track click scrolls");
    assert!(
        jump > page,
        "jump goes to the click, paging only a screen: {jump} vs {page}"
    );
}

struct Labels(&'static [&'static str]);

impl junie_tui::core::list::RowSource for Labels {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn label(&self, i: usize) -> &str {
        self.0[i]
    }
}

fn layout_list_core() {
    let rows = Labels(&["alpha", "beta", "gamma"]);
    let mut core = ListCore::new(3);
    assert_eq!(core.cursor, 0);
    assert!(!core.wrap && core.skip_disabled && core.skip_when_empty);
    assert_eq!(core.content, ContentState::Ready);

    // The cursor model mirrors the walk settings.
    let model = core.cursor_model();
    assert_eq!(
        (model.index, model.wrap, model.skip_disabled),
        (0, false, true)
    );
    core.cursor = 2;
    core.wrap = true;
    core.skip_disabled = false;
    let model = core.cursor_model();
    assert_eq!(
        (model.index, model.wrap, model.skip_disabled),
        (2, true, false)
    );
    core.cursor = 0;
    core.wrap = false;

    // Keys turn into a step and the move it caused.
    core.type_ahead = false;
    let (step, moved) = core
        .key_step(&key(KeyCode::Down), &rows)
        .expect("a nav key");
    assert_eq!(step, KeyStep::Nav(junie_tui::core::keymap::Nav::Down));
    assert_eq!(moved, Moved::To(1));
    assert!(
        core.key_step(&key(KeyCode::F(5)), &rows).is_none(),
        "not a list key"
    );
    core.type_ahead = true;
    core.type_ahead_precedence = Some(junie_tui::core::list::TypeAheadPrecedence::TypeAhead);
    let (step, moved) = core
        .key_step(&key(KeyCode::Char('g')), &rows)
        .expect("a typed key");
    assert_eq!(step, KeyStep::Type('g'));
    assert_eq!(moved.index(), Some(2), "type-ahead jumps to the match");

    // Rows exist only when content is ready and non-empty.
    assert!(core.has_rows(3));
    assert!(!core.has_rows(0));
    core.content = ContentState::loading("Loading");
    assert!(!core.has_rows(3), "loading hides the rows");
    core.content = ContentState::Ready;

    // An empty list is skipped by Tab unless focused or told otherwise.
    assert!(core.is_focus_stop(3, false));
    assert!(!core.is_focus_stop(0, false), "an empty list is skipped");
    assert!(core.is_focus_stop(0, true), "a focused list stays a stop");
    core.skip_when_empty = false;
    assert!(core.is_focus_stop(0, false));
    core.skip_when_empty = true;

    // Content states draw in place of rows and say they did.
    let area = Rect::new(0, 0, 30, 5);
    let draw = |core: &ListCore, len: usize, default: Option<&str>| {
        let mut drew = false;
        let frame = paint(area, |area, buf, ctx| {
            drew = core.draw_content(len, area, buf, ctx, ctx.theme.canvas, default);
        });
        (drew, text_of(&frame))
    };
    let (drew, _) = draw(&core, 3, None);
    assert!(!drew, "ready rows are the caller's to draw");
    let (drew, text) = draw(&core, 0, Some("No rows"));
    assert!(drew && text.contains("No rows"), "{text}");
    let (drew, text) = draw(&core, 0, None);
    assert!(drew && text.trim().is_empty(), "nothing to say: {text}");
    core.empty = Some(EmptyState::new("Nothing here"));
    let (drew, text) = draw(&core, 0, Some("No rows"));
    assert!(
        drew && text.contains("Nothing here") && !text.contains("No rows"),
        "{text}"
    );
    core.content = ContentState::loading("Fetching rows");
    let (drew, text) = draw(&core, 3, None);
    assert!(drew && text.contains("Fetching rows"), "{text}");
    core.content = ContentState::error(EmptyState::new("Cannot load"));
    let (drew, text) = draw(&core, 3, None);
    assert!(drew && text.contains("Cannot load"), "{text}");
}

fn layout_notice_kinds() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    assert_eq!(NoticeKind::default(), NoticeKind::Error);
    for (kind, tone) in [
        (NoticeKind::Error, Tone::Error),
        (NoticeKind::Warning, Tone::Warning),
        (NoticeKind::Info, Tone::Normal),
        (NoticeKind::Success, Tone::Normal),
    ] {
        assert_eq!(kind.message_tone(), tone);
    }

    let base = Notice::new(NoticeKind::Info, "Saved");
    assert_eq!((base.message_lines, base.max_lines), (2, 2));
    assert_eq!(base.detail_tone, Tone::Secondary);
    assert!(base.mark_cut && base.detail.is_none() && base.hint.is_none());
    let retargeted = base.clone().kind(NoticeKind::Warning);
    assert_eq!(retargeted.kind, NoticeKind::Warning);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    retargeted.render_theme(buf.area, &mut buf, &theme, theme.canvas);
    assert_eq!(
        buf[(0, 0)].symbol(),
        theme.glyphs.warning,
        "the kind decides the mark"
    );

    // Each kind has its own mark and message colour.
    let area = Rect::new(0, 0, 30, 3);
    for (notice, kind, mark) in [
        (Notice::error("Disk full"), NoticeKind::Error, "!"),
        (
            Notice::warning("Disk full"),
            NoticeKind::Warning,
            theme.glyphs.warning,
        ),
        (
            Notice::info("Disk full"),
            NoticeKind::Info,
            theme.glyphs.info,
        ),
        (
            Notice::success("Disk full"),
            NoticeKind::Success,
            theme.glyphs.done,
        ),
    ] {
        assert_eq!(notice.kind, kind);
        let mut buf = Buffer::empty(area);
        let used = notice.render_theme(area, &mut buf, &theme, theme.canvas);
        assert_eq!(used, 1);
        assert_eq!(buf[(0, 0)].symbol(), mark, "{kind:?} mark");
        assert_eq!(buf[(2, 0)].symbol(), "D");
        assert_eq!(
            buf[(2, 0)].fg,
            theme.tone(kind.message_tone()),
            "{kind:?} ink"
        );
    }
    let marks: Vec<String> = [
        Notice::error("x"),
        Notice::warning("x"),
        Notice::info("x"),
        Notice::success("x"),
    ]
    .iter()
    .map(|n| {
        paint(area, |a, buf, ctx| {
            n.render(a, buf, ctx, ctx.theme.canvas);
        })[(0, 0)]
            .symbol()
            .to_owned()
    })
    .collect();
    let distinct: std::collections::BTreeSet<_> = marks.iter().collect();
    assert_eq!(distinct.len(), 4, "four kinds, four marks: {marks:?}");

    // A detail line and a hint add rows, each in its own tone.
    let full = Notice::error("Upload failed")
        .detail("The server refused the file")
        .hint("Press r to retry");
    assert_eq!(full.height(30), 3);
    let mut buf = Buffer::empty(area);
    assert_eq!(full.render_theme(area, &mut buf, &theme, theme.canvas), 3);
    assert_eq!(buf[(2, 1)].fg, theme.tone(Tone::Secondary));
    assert_eq!(buf[(2, 2)].fg, theme.muted().fg.expect("a muted ink"));
    let faint = full.clone().detail_tone(Tone::Faint);
    assert_eq!(faint.detail_tone, Tone::Faint);
    let mut buf = Buffer::empty(area);
    faint.render_theme(area, &mut buf, &theme, theme.canvas);
    assert_eq!(buf[(2, 1)].fg, theme.tone(Tone::Faint));

    // Too narrow for text: only the mark is drawn.
    let mut buf = Buffer::empty(Rect::new(0, 0, 2, 3));
    assert_eq!(
        full.render_theme(Rect::new(0, 0, 2, 3), &mut buf, &theme, theme.canvas),
        1
    );
    assert_eq!(
        full.render_theme(
            Rect::ZERO,
            &mut Buffer::empty(Rect::ZERO),
            &theme,
            theme.canvas
        ),
        0
    );
}

fn layout_notice_wrapping() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let long = "The disk is full and the write was rejected by the host";
    let narrow = Rect::new(0, 0, 22, 8);
    let wide = Notice::error(long);
    assert_eq!(wide.height(22), 2, "two message lines by default");
    assert_eq!(wide.clone().message_lines(1).height(22), 1);
    assert_eq!(wide.clone().message_lines(3).height(22), 3);
    assert_eq!(wide.height(80), 1, "it fits one line when wide");

    // Detail lines are capped by max_lines.
    let detailed = Notice::error("Failed").detail(long);
    assert_eq!(detailed.height(22), 3);
    assert_eq!(detailed.clone().max_lines(1).height(22), 2);
    assert_eq!(detailed.clone().max_lines(3).height(22), 4);

    // A cut line ends in an ellipsis unless asked not to.
    let render = |n: &Notice| {
        let mut buf = Buffer::empty(narrow);
        let used = n.render_theme(narrow, &mut buf, &theme, theme.canvas);
        (used, rows(&buf))
    };
    let (used, cut) = render(&wide.clone().message_lines(1));
    assert_eq!(used, 1);
    assert!(cut[0].trim_end().ends_with('…'), "{cut:?}");
    let (_, plain) = render(&wide.clone().message_lines(1).mark_cut(false));
    assert!(!plain[0].contains('…'), "{plain:?}");
    assert!(!wide.clone().message_lines(1).mark_cut(false).mark_cut);
    let (used, two) = render(&wide);
    assert_eq!(used, 2);
    assert!(
        two[0].contains("The disk") && !two[1].trim().is_empty(),
        "{two:?}"
    );
    assert_eq!(used, wide.height(22));

    // The frame renders through the context too, at any area.
    let frame = paint(narrow, |a, buf, ctx| {
        assert_eq!(wide.render(a, buf, ctx, ctx.theme.canvas), 2);
    });
    assert!(text_of(&frame).contains("The disk"));
}

fn layout_empty_state() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, 40, 10);
    assert_eq!(EmptyKind::default(), EmptyKind::Empty);

    // The plain state centers a muted title and a faint hint.
    let blank = EmptyState::new("No results").hint("Try a broader query");
    assert_eq!(blank.kind, EmptyKind::Empty);
    assert_eq!(blank.hint.as_deref(), Some("Try a broader query"));
    let mut buf = Buffer::empty(area);
    empty::render(area, &mut buf, &theme, &blank, theme.canvas);
    let lines = rows(&buf);
    let title_row = lines
        .iter()
        .position(|l| l.contains("No results"))
        .expect("title");
    let hint_row = lines
        .iter()
        .position(|l| l.contains("Try a broader"))
        .expect("hint");
    assert_eq!(
        hint_row,
        title_row + 2,
        "a blank row separates hint from title"
    );
    let at = lines[title_row].find("No results").expect("column");
    assert_eq!(at, (40 - "No results".len()) / 2, "centered");
    assert_eq!(
        buf[(at as u16, title_row as u16)].fg,
        theme.muted().fg.expect("muted")
    );
    assert_eq!(
        buf[(12, hint_row as u16)].fg,
        theme.faint().fg.expect("faint")
    );

    // The error state leads with a bold mark in the error colour.
    let broken = EmptyState::error("Cannot connect");
    assert_eq!(broken.kind, EmptyKind::Error);
    let mut buf = Buffer::empty(area);
    empty::render(area, &mut buf, &theme, &broken, theme.canvas);
    let lines = rows(&buf);
    let y = lines
        .iter()
        .position(|l| l.contains("! Cannot connect"))
        .expect("error title");
    let x = lines[y].find('!').expect("mark") as u16;
    assert_eq!(
        buf[(x, y as u16)].fg,
        theme.error_fg().fg.expect("error ink")
    );

    // Height counts the title, the hint with its gap and the action row.
    assert_eq!(empty::height(&EmptyState::new("a"), 40), 1);
    assert_eq!(empty::height(&blank, 40), 3);
    let retry = WidgetId::of("scenario.layout.empty.retry");
    let dismiss = WidgetId::of("scenario.layout.empty.dismiss");
    let primary = EmptyAction::primary(retry, "Retry");
    let secondary = EmptyAction::secondary(dismiss, "Dismiss");
    assert!(primary.primary && !secondary.primary);
    assert_eq!((primary.id, primary.label.as_str()), (retry, "Retry"));
    let acting = EmptyState::error("Cannot connect")
        .hint("Check the host")
        .actions(vec![primary.clone(), secondary.clone()]);
    assert_eq!(acting.actions.len(), 2);
    assert_eq!(empty::height(&acting, 40), 5);

    // With actions the context renderer adds buttons and hit regions.
    let frame = paint_hits(area, |a, buf, ctx| {
        empty::render_ctx(a, buf, ctx, &acting, ctx.theme.canvas)
    });
    let text = text_of(&frame.buf);
    assert!(text.contains("Retry") && text.contains("Dismiss"), "{text}");
    assert!(frame.hits.regions().iter().any(|r| r.id == retry));
    assert!(frame.hits.regions().iter().any(|r| r.id == dismiss));
    let short = Rect::new(0, 0, 40, 3);
    let frame = paint_hits(short, |a, buf, ctx| {
        empty::render_ctx(a, buf, ctx, &acting, ctx.theme.canvas)
    });
    assert!(
        !text_of(&frame.buf).contains("Retry"),
        "no room for actions"
    );
    let plain = paint_hits(area, |a, buf, ctx| {
        empty::render_ctx(a, buf, ctx, &blank, ctx.theme.canvas)
    });
    assert!(plain.hits.regions().is_empty());
    assert!(text_of(&plain.buf).contains("No results"));

    // Clicks and keys resolve to the action under them.
    assert_eq!(acting.on_click(retry), Some(&primary));
    assert_eq!(acting.on_click(dismiss), Some(&secondary));
    assert_eq!(
        acting.on_click(WidgetId::of("scenario.layout.empty.other")),
        None
    );
    let (outcome, action) = acting.on_key(retry, &key(KeyCode::Enter));
    assert_ne!(outcome, Outcome::Ignored);
    assert_eq!(action, Some(&primary));
    let (_, action) = acting.on_key(dismiss, &key(KeyCode::Char(' ')));
    assert_eq!(action, Some(&secondary));
    assert_eq!(
        acting.on_key(
            WidgetId::of("scenario.layout.empty.other"),
            &key(KeyCode::Enter)
        ),
        (Outcome::Ignored, None)
    );
    assert_eq!(acting.on_key(retry, &key(KeyCode::Char('q'))).1, None);
}

fn layout_page_header() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, 50, 6);
    let header = PageHeader::new("Meters");
    assert_eq!((header.inset, header.right_gap), (1, 4));
    assert_eq!(header.height(), 1);

    let full = PageHeader::new("Meters")
        .status("live", Tone::Success)
        .right("v1.2")
        .subtitle("CPU and memory", Tone::Muted);
    assert_eq!(full.height(), 2);
    assert_eq!(
        full.body(area),
        Rect::new(0, 3, 50, 3),
        "a blank row follows the header"
    );
    assert_eq!(header.body(area), Rect::new(0, 2, 50, 4));
    assert_eq!(
        full.body(Rect::new(2, 1, 20, 2)),
        Rect::new(2, 3, 20, 0),
        "never past the area"
    );

    let mut buf = Buffer::empty(area);
    assert_eq!(full.render(area, &mut buf, &theme), 2);
    let lines = rows(&buf);
    assert_eq!(
        lines[0].find("Meters"),
        Some(1),
        "the title sits at the inset"
    );
    assert!(lines[0].contains("Meterslive"), "{lines:?}");
    assert_eq!(
        lines[0].find("v1.2"),
        Some(45),
        "right text keeps the inset: {lines:?}"
    );
    assert_eq!(lines[1].find("CPU and memory"), Some(1));
    assert_eq!(buf[(1, 0)].fg, theme.title().fg.expect("title ink"));
    assert_eq!(buf[(7, 0)].fg, theme.tone(Tone::Success), "status tone");
    assert_eq!(buf[(1, 1)].fg, theme.tone(Tone::Muted), "subtitle tone");
    let mut buf = Buffer::empty(area);
    assert_eq!(header.render(area, &mut buf, &theme), 1);
    assert_eq!(
        PageHeader::new("x").render(Rect::ZERO, &mut Buffer::empty(Rect::ZERO), &theme),
        0
    );
    assert_eq!(
        full.render(Rect::new(0, 0, 50, 1), &mut Buffer::empty(area), &theme),
        1,
        "one row has no room for the subtitle"
    );

    // Inset moves the title; the right gap decides when the right text drops.
    let flush = PageHeader::new("Meters").inset(0);
    let mut buf = Buffer::empty(area);
    flush.render(area, &mut buf, &theme);
    assert_eq!(rows(&buf)[0].find("Meters"), Some(0));
    let tight = Rect::new(0, 0, 14, 1);
    let crowded = PageHeader::new("Meters").right("v1.2");
    let mut buf = Buffer::empty(tight);
    crowded.render(tight, &mut buf, &theme);
    assert!(!rows(&buf)[0].contains("v1.2"), "{:?}", rows(&buf));
    let mut buf = Buffer::empty(tight);
    crowded.clone().right_gap(0).render(tight, &mut buf, &theme);
    assert!(rows(&buf)[0].contains("v1.2"), "{:?}", rows(&buf));

    // Headings: text, optional right-aligned meta, a tone.
    let heading = Heading::new("Recent");
    assert_eq!((heading.tone, heading.meta.as_deref()), (Tone::Muted, None));
    let tagged = heading.clone().meta("3 items").tone(Tone::Secondary);
    assert_eq!(tagged.meta.as_deref(), Some("3 items"));
    assert_eq!(tagged.tone, Tone::Secondary);
    let strip = Rect::new(0, 0, 30, 1);
    let mut buf = Buffer::empty(strip);
    tagged.render(strip, &mut buf, &theme, theme.canvas);
    let line = rows(&buf).remove(0);
    assert!(
        line.starts_with("Recent") && line.ends_with("3 items"),
        "{line}"
    );
    assert_eq!(buf[(0, 0)].fg, theme.tone_on(Tone::Secondary, theme.canvas));
    assert_eq!(buf[(29, 0)].fg, theme.tone_on(Tone::Faint, theme.canvas));
    let mut buf = Buffer::empty(strip);
    heading.render(strip, &mut buf, &theme, theme.canvas);
    assert_eq!(buf[(0, 0)].fg, theme.tone_on(Tone::Muted, theme.canvas));
    let squeezed = Rect::new(0, 0, 10, 1);
    let mut buf = Buffer::empty(squeezed);
    tagged.render(squeezed, &mut buf, &theme, theme.canvas);
    assert!(
        !rows(&buf)[0].contains("items"),
        "meta yields to the text: {:?}",
        rows(&buf)
    );
}

fn layout_detail_card() {
    let area = Rect::new(0, 0, 44, 14);
    let props = || {
        vec![
            Prop::new("Host", "db.example.com"),
            Prop::new("User", "admin"),
        ]
    };
    let connect = WidgetId::of("scenario.layout.card.connect");
    let edit = WidgetId::of("scenario.layout.card.edit");
    let draw = |card: &DetailCard, interaction: Interaction| {
        let mut connect_btn = Button::primary(connect, "Connect");
        let mut edit_btn = Button::secondary(edit, "Edit");
        let mut layout: Option<DetailLayout> = None;
        let frame = paint_with(area, interaction, |a, buf, ctx| {
            layout = Some(card.render(
                a,
                buf,
                ctx,
                ctx.theme.canvas,
                &mut [&mut connect_btn, &mut edit_btn],
            ));
        });
        (frame, layout.expect("rendered"))
    };

    let card = DetailCard::new("Production")
        .meta("postgres")
        .props(props())
        .actions(ActionRow::new());
    assert_eq!(card.reserve, 3);
    assert!(!card.fit && !card.focused && card.max_width.is_none());
    let (frame, layout) = draw(&card, Interaction::default());
    let text = text_of(&frame.buf);
    for shown in [
        "Production",
        "postgres",
        "Host",
        "db.example.com",
        "User",
        "Connect",
        "Edit",
    ] {
        assert!(text.contains(shown), "{shown}:\n{text}");
    }
    assert_eq!(layout.card, area, "the card fills its area by default");
    assert_eq!(layout.props_h, 2);
    assert_eq!(layout.actions.len(), 2);
    assert!(layout.actions.iter().all(|r| r.y == layout.actions[0].y));
    assert!(layout.slot.height > 0 && layout.slot.bottom() <= layout.actions[0].y);
    assert_ne!(layout.bg, ratatui::style::Color::Reset);
    assert!(frame.hits.regions().iter().any(|r| r.id == connect));

    // A size cap pins the card to the corner of its area.
    let capped = card.clone().max_size(Some(30), Some(8));
    assert_eq!((capped.max_width, capped.max_height), (Some(30), Some(8)));
    let (_, layout) = draw(&capped, Interaction::default());
    assert_eq!(layout.card, Rect::new(0, 0, 30, 8));
    let (_, layout) = draw(
        &card.clone().max_size(None, Some(9)),
        Interaction::default(),
    );
    assert_eq!(layout.card, Rect::new(0, 0, 44, 9));

    // Fit shrinks the card to its props plus the reserved action rows.
    let fitted = card.clone().fit(true);
    assert!(fitted.fit);
    let (_, small) = draw(&fitted, Interaction::default());
    assert!(small.card.height < area.height, "{small:?}");
    let (_, roomy) = draw(&fitted.clone().reserve(9), Interaction::default());
    assert_eq!(
        roomy.card.height - small.card.height,
        6,
        "reserve is whole rows"
    );
    assert_eq!(fitted.clone().reserve(9).reserve, 9);

    // Focus is a visible change of the frame.
    let (plain, _) = draw(&card, Interaction::default());
    let (focused, _) = draw(&card.clone().focused(true), Interaction::default());
    assert!(card.clone().focused(true).focused);
    assert_ne!(plain.buf, focused.buf, "a focused card draws differently");

    // The empty form shows only the empty state.
    let hollow = DetailCard::empty(EmptyState::new("Pick a connection"));
    assert!(hollow.empty.is_some());
    let (frame, layout) = draw(&hollow, Interaction::default());
    assert!(text_of(&frame.buf).contains("Pick a connection"));
    assert!(layout.actions.is_empty() && layout.card.is_empty());
}

fn layout_sidebar_rail() {
    let id = WidgetId::of("scenario.layout.rail");
    assert_eq!(SidebarRail::WIDTH, 2);
    let rail = SidebarRail::new(id, "Files");
    assert!(rail.collapsed_rail);
    assert_eq!(rail.width(), SidebarRail::WIDTH);
    let open = rail.clone().collapsed_rail(false);
    assert!(!open.collapsed_rail);
    assert_eq!(open.width(), 0, "an expanded sidebar leaves no rail");
    assert_ne!(rail.collapse_id(), id);

    // The collapsed rail is a one-column strip with the label running down.
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, 2, 10);
    let frame = paint_hits(area, |a, buf, ctx| {
        rail.render_rail(a, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(frame.buf[(0, 0)].symbol(), theme.glyphs.overflow_right);
    let column: String = (2..7)
        .map(|y| frame.buf[(0, y)].symbol().to_owned())
        .collect();
    assert_eq!(column, "Files");
    assert_eq!(frame.buf[(0, 8)].symbol(), " ");
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == id && r.area == Rect::new(0, 0, 1, 10)),
        "the strip is clickable"
    );
    let wide = SidebarRail::new(id, "日本");
    let frame = paint(area, |a, buf, ctx| {
        wide.render_rail(a, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(
        frame[(0, 2)].symbol(),
        "…",
        "a wide cluster becomes a placeholder"
    );
    let silent = paint_hits(area, |a, buf, ctx| {
        open.render_rail(a, buf, ctx, ctx.theme.canvas)
    });
    assert!(silent.hits.regions().is_empty() && text_of(&silent.buf).trim().is_empty());
    let short = paint(Rect::new(0, 0, 2, 4), |a, buf, ctx| {
        rail.render_rail(a, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(
        short[(0, 3)].symbol(),
        "i",
        "the label is clipped to the strip"
    );
    assert_eq!(short[(0, 4 - 1)].symbol(), "i");

    // Clicks: the strip expands, the collapse control collapses.
    assert_eq!(rail.on_click(id), Some(RailAction::Expand));
    assert_eq!(
        rail.on_click(rail.collapse_id()),
        Some(RailAction::Collapse)
    );
    assert_eq!(
        rail.on_click(WidgetId::of("scenario.layout.rail.other")),
        None
    );
    assert_eq!(
        open.on_click(id),
        None,
        "an open sidebar's body is not the strip"
    );
    assert_eq!(
        open.on_click(open.collapse_id()),
        Some(RailAction::Collapse)
    );

    // The collapse control sits in the panel's top-right corner when it fits.
    let panel = Rect::new(0, 0, 30, 6);
    let frame = paint_hits(panel, |a, buf, ctx| {
        open.render_collapse(a, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(frame.buf[(26, 0)].symbol(), theme.glyphs.overflow_left);
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == open.collapse_id() && r.area == Rect::new(25, 0, 3, 1))
    );
    let narrow = paint_hits(Rect::new(0, 0, 11, 6), |a, buf, ctx| {
        open.render_collapse(a, buf, ctx, ctx.theme.canvas)
    });
    assert!(narrow.hits.regions().is_empty(), "no room, no control");
    let flat = paint_hits(Rect::new(0, 0, 30, 1), |a, buf, ctx| {
        open.render_collapse(a, buf, ctx, ctx.theme.canvas)
    });
    assert!(flat.hits.regions().is_empty());
}

fn layout_too_small() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    assert_eq!((TooSmall::MIN_WIDTH, TooSmall::MIN_HEIGHT), (72, 20));
    let guard = TooSmall::new("Showcase");
    assert!(guard.applies(Rect::new(0, 0, 71, 20)));
    assert!(guard.applies(Rect::new(0, 0, 72, 19)));
    assert!(!guard.applies(Rect::new(0, 0, 72, 20)));
    assert!(!guard.applies(Rect::new(0, 0, 200, 60)));
    let lenient = TooSmall::new("Showcase").min(40, 10);
    assert!(!lenient.applies(Rect::new(0, 0, 41, 11)));
    assert!(lenient.applies(Rect::new(0, 0, 39, 11)));

    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    guard.render(area, &mut buf, &theme);
    let text = text_of(&buf);
    for line in [
        "Showcase",
        "Terminal too small",
        "Need 72×20, have 40×10",
        "q Quit",
    ] {
        assert!(text.contains(line), "{line}:\n{text}");
    }
    let lines = rows(&buf);
    let brand = lines
        .iter()
        .position(|l| l.contains("Showcase"))
        .expect("brand");
    let hint = lines
        .iter()
        .position(|l| l.contains("q Quit"))
        .expect("hint");
    assert_eq!(hint, brand + 4, "the quit hint sits apart from the message");
    assert_eq!(
        lines[brand].find("Showcase"),
        Some((40 - 8) / 2),
        "centered"
    );
    assert_eq!(buf[(16, brand as u16)].fg, theme.title().fg.expect("title"));

    // The wording of the hint and the brand colour belong to the app.
    let custom = TooSmall::new("Showcase")
        .quit_hint("Esc leaves")
        .brand_style(ratatui::style::Style::new().fg(ratatui::style::Color::Magenta))
        .min(60, 8);
    let mut buf = Buffer::empty(area);
    custom.render(area, &mut buf, &theme);
    let text = text_of(&buf);
    assert!(
        text.contains("Esc leaves") && !text.contains("q Quit"),
        "{text}"
    );
    assert!(text.contains("Need 60×8, have 40×10"), "{text}");
    let lines = rows(&buf);
    let y = lines
        .iter()
        .position(|l| l.contains("Showcase"))
        .expect("brand") as u16;
    assert_eq!(buf[(16, y)].fg, ratatui::style::Color::Magenta);

    // Inside a larger buffer it paints only its own area.
    let inner = Rect::new(4, 2, 30, 6);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
    guard.render(inner, &mut buf, &theme);
    let lines = rows(&buf);
    assert!(
        lines[0].trim().is_empty() && lines[9].trim().is_empty(),
        "{lines:?}"
    );
    assert!(lines[2..8].iter().any(|l| l.contains("Terminal too small")));
    assert!(lines.iter().all(|l| l[..4].trim().is_empty()), "{lines:?}");
    // A tiny area truncates instead of overflowing.
    let tiny = Rect::new(0, 0, 8, 3);
    let mut buf = Buffer::empty(tiny);
    guard.render(tiny, &mut buf, &theme);
    assert!(text_of(&buf).contains("Showcase"));
}

fn layout_breadcrumb() {
    let id = WidgetId::of("scenario.layout.crumbs");
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let path = || -> Vec<String> {
        ["Home", "Projects", "terminal", "src"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    };
    let mut crumbs = Breadcrumb::new(id, path());
    assert!(crumbs.clickable);
    let sep = format!(" {} ", theme.glyphs.overflow_right);
    let wide = Rect::new(0, 0, 60, 1);
    let draw = |crumbs: &mut Breadcrumb, area: Rect| {
        paint_hits(area, |a, buf, ctx| {
            crumbs.render(a, buf, ctx, ctx.theme.canvas)
        })
    };
    let frame = draw(&mut crumbs, wide);
    let line = rows(&frame.buf).remove(0);
    assert_eq!(
        line.trim_end(),
        format!("Home{sep}Projects{sep}terminal{sep}src")
    );
    assert!(crumbs.areas.iter().all(|r| !r.is_empty()));
    assert_ne!(crumbs.segment_id(0), crumbs.segment_id(1));
    for i in 0..3 {
        assert!(
            frame
                .hits
                .regions()
                .iter()
                .any(|r| r.id == crumbs.segment_id(i) && r.area == crumbs.areas[i]),
            "segment {i} is a link"
        );
    }
    assert!(
        !frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == crumbs.segment_id(3)),
        "the current page is not a link"
    );

    // Clicking a parent segment says which level to go to.
    assert_eq!(crumbs.on_click(crumbs.segment_id(1)), Some(1));
    assert_eq!(crumbs.on_click(crumbs.segment_id(0)), Some(0));
    assert_eq!(
        crumbs.on_click(crumbs.segment_id(3)),
        None,
        "the last is current"
    );
    assert_eq!(
        crumbs.on_click(WidgetId::of("scenario.layout.crumbs.other")),
        None
    );
    assert_eq!(
        Breadcrumb::new(id, Vec::new()).on_click(crumbs.segment_id(0)),
        None
    );

    // Not clickable: plain text and no regions.
    let mut flat = Breadcrumb::new(id, path()).clickable(false);
    assert!(!flat.clickable);
    let frame = draw(&mut flat, wide);
    assert!(frame.hits.regions().is_empty());
    assert_eq!(flat.on_click(flat.segment_id(1)), None);
    assert!(text_of(&frame.buf).contains("Projects"));

    // Narrow: earlier segments collapse into an ellipsis, the last stays.
    let mut narrow = Breadcrumb::new(id, path());
    let frame = draw(&mut narrow, Rect::new(0, 0, 18, 1));
    let line = rows(&frame.buf).remove(0);
    assert!(line.starts_with(theme.glyphs.ellipsis), "{line:?}");
    assert!(line.trim_end().ends_with("src"), "{line:?}");
    assert!(!line.contains("Home"), "{line:?}");
    assert!(
        narrow.areas[0].is_empty(),
        "a collapsed segment has no area"
    );
    let frame = draw(&mut narrow, Rect::ZERO);
    assert!(frame.hits.regions().is_empty());
}
