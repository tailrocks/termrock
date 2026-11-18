use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect, Size};

use crate::scroll_view::{Axes, ScrollView};
use termrock_controls::scrollbar::{h_id_for, id_for};
use termrock_core::event::{Mouse, MouseKind, Outcome};
use termrock_core::focus::{Focus, FocusRing};
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_core::intent::Phase;
use termrock_core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
use termrock_core::response::Response;
use termrock_core::scroll::TrackClick;
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, Theme};

const INNER: WidgetId = WidgetId::of("nested_scrollbar.inner");
const OUTER: WidgetId = WidgetId::of("nested_scrollbar.outer");
const MID: WidgetId = WidgetId::of("nested_scrollbar.mid");

struct Level {
    view: ScrollView,
    area: Rect,
    content: Size,
}

struct Rig {
    levels: Vec<Level>,
    inner: ScrollView,
    inner_area: Rect,
    inner_content: Size,
    screen: Rect,
    hits: HitRegistry,
    router: PointerRouter,
    focus: Focus,
    ring: FocusRing,
    now: u64,
    seen: Vec<PointerEvent>,
}

impl PointerHost for Rig {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        PointerParts {
            router: &mut self.router,
            hits: &self.hits,
            focus: &mut self.focus,
            ring: &self.ring,
        }
    }

    fn layer_open(&self) -> bool {
        false
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        if ev.owner() == INNER {
            self.seen.push(*ev);
        }
        for level in &mut self.levels {
            if let Some(r) = level.view.pointer(ev) {
                return r;
            }
        }
        self.inner
            .pointer(ev)
            .unwrap_or_else(|| Outcome::Ignored.into())
    }
}

fn paint(
    levels: &mut [Level],
    inner: &mut ScrollView,
    at: (Rect, Size),
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    match levels.split_first_mut() {
        Some((level, rest)) => {
            let (area, content) = (level.area, level.content);
            level.view.render(area, content, buf, ctx, |_, buf, ctx| {
                paint(rest, inner, at, buf, ctx);
            });
        }
        None => inner.render(at.0, at.1, buf, ctx, |_, _, _| {}),
    }
}

impl Rig {
    fn new(
        levels: Vec<Level>,
        inner: ScrollView,
        inner_area: Rect,
        inner_content: Size,
        screen: Rect,
    ) -> Self {
        let mut rig = Self {
            levels,
            inner,
            inner_area,
            inner_content,
            screen,
            hits: HitRegistry::default(),
            router: PointerRouter::default(),
            focus: Focus::default(),
            ring: FocusRing::default(),
            now: 0,
            seen: Vec::new(),
        };
        rig.render();
        rig
    }

    fn render(&mut self) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(self.screen);
        self.hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut self.hits, &mut ring);
        paint(
            &mut self.levels,
            &mut self.inner,
            (self.inner_area, self.inner_content),
            &mut buf,
            &mut ctx,
        );
    }

    fn scroll_level(&mut self, level: usize, x: usize, y: usize) {
        assert!(
            self.levels[level].view.scroll_to(x, y),
            "level {level} moved"
        );
        self.render();
    }

    fn shift(&self) -> (u16, u16) {
        self.levels.iter().fold((0, 0), |(x, y), l| {
            let (ox, oy) = l.view.offset();
            (x + ox as u16, y + oy as u16)
        })
    }

    fn screen_of(&self, owner: (u16, u16)) -> Position {
        let (dx, dy) = self.shift();
        Position::new(owner.0 - dx, owner.1 - dy)
    }

    fn mouse(&mut self, kind: MouseKind, at: Position) -> Outcome {
        let out = PointerRouter::route(self, Mouse::new(kind, at));
        self.render();
        out
    }

    fn at_owner(&mut self, kind: MouseKind, owner: (u16, u16)) -> Outcome {
        let at = self.screen_of(owner);
        self.mouse(kind, at)
    }

    fn play(&mut self, script: &[(MouseKind, (u16, u16))]) {
        for &(kind, owner) in script {
            self.at_owner(kind, owner);
        }
    }

    fn bar_event(&self, phase: Phase) -> &PointerEvent {
        self.seen
            .iter()
            .find(|e| e.is(phase))
            .unwrap_or_else(|| panic!("no {phase:?} event reached the inner scrollbar"))
    }
}

fn level(id: WidgetId, axes: Axes, area: Rect, content: Size) -> Level {
    Level {
        view: ScrollView::new(id).axes(axes),
        area,
        content,
    }
}

fn single(click: TrackClick) -> Rig {
    let outer = level(
        OUTER,
        Axes::VERTICAL,
        Rect::new(0, 0, 30, 10),
        Size::new(28, 40),
    );
    let inner = ScrollView::new(INNER)
        .axes(Axes::VERTICAL)
        .track_click(click);
    Rig::new(
        vec![outer],
        inner,
        Rect::new(2, 20, 20, 8),
        Size::new(18, 30),
        Rect::new(0, 0, 30, 10),
    )
}

fn reference(click: TrackClick) -> Rig {
    let inner = ScrollView::new(INNER)
        .axes(Axes::VERTICAL)
        .track_click(click);
    Rig::new(
        Vec::new(),
        inner,
        Rect::new(2, 20, 20, 8),
        Size::new(18, 30),
        Rect::new(0, 0, 30, 40),
    )
}

fn assert_matches_reference(nested: &Rig, reference: &Rig) {
    assert_eq!(
        nested.seen.len(),
        reference.seen.len(),
        "the same pointer events reach the scrollbar owner"
    );
    for (n, r) in nested.seen.iter().zip(&reference.seen) {
        assert_eq!(n.phase(), r.phase());
        assert_eq!(n.pos(), r.pos(), "{:?} position", n.phase());
        assert_eq!(
            n.bar.map(|b| b.track),
            r.bar.map(|b| b.track),
            "{:?} track",
            n.phase()
        );
    }
    assert_eq!(
        nested.inner.offset(),
        reference.inner.offset(),
        "the inner scroll state ends where the unscrolled reference ends"
    );
    assert_ne!(
        reference.inner.offset(),
        (0, 0),
        "the gesture moved the view"
    );
}

#[test]
fn nested_scrollbar_vertical_matches_unscrolled_reference() {
    for click in [TrackClick::Jump, TrackClick::Page] {
        let mut nested = single(click);
        nested.scroll_level(0, 0, 15);
        let script = [
            (MouseKind::Down, (21, 20 + 2)),
            (MouseKind::Drag, (21, 20 + 4)),
            (MouseKind::Up, (21, 20 + 4)),
        ];
        let mut flat = reference(click);
        flat.play(&script);
        nested.play(&script);

        let press = nested.bar_event(Phase::Press);
        assert_eq!(press.pos(), Position::new(21, 22), "{click:?} owner pos");
        assert_eq!(
            press.bar.map(|b| b.track),
            Some(Rect::new(21, 20, 1, 8)),
            "{click:?} owner-space full track"
        );
        assert_matches_reference(&nested, &flat);
    }
}

#[test]
fn nested_scrollbar_thumb_grab_keeps_its_cell() {
    let mut nested = single(TrackClick::Page);
    nested.scroll_level(0, 0, 15);
    let mut flat = reference(TrackClick::Page);
    let script = [
        (MouseKind::Down, (21, 20)),
        (MouseKind::Drag, (21, 23)),
        (MouseKind::Up, (21, 23)),
    ];
    flat.play(&script);
    nested.play(&script);
    assert_matches_reference(&nested, &flat);
}

#[test]
fn nested_scrollbar_horizontal_and_double_nesting() {
    let build = |nested: bool| {
        let levels = if nested {
            vec![
                level(
                    OUTER,
                    Axes::BOTH,
                    Rect::new(3, 2, 30, 10),
                    Size::new(80, 60),
                ),
                level(MID, Axes::BOTH, Rect::new(6, 8, 24, 10), Size::new(60, 40)),
            ]
        } else {
            Vec::new()
        };
        let screen = if nested {
            Rect::new(0, 0, 40, 14)
        } else {
            Rect::new(0, 0, 70, 70)
        };
        let inner = ScrollView::new(INNER).axes(Axes::BOTH);
        Rig::new(
            levels,
            inner,
            Rect::new(10, 24, 20, 6),
            Size::new(50, 30),
            screen,
        )
    };
    let mut flat = build(false);
    let mut nested = build(true);
    nested.scroll_level(0, 2, 6);
    nested.scroll_level(1, 4, 15);
    assert_eq!(nested.shift(), (6, 21), "two levels accumulate");

    let h = [
        (MouseKind::Down, (14, 29)),
        (MouseKind::Drag, (20, 29)),
        (MouseKind::Up, (20, 29)),
    ];
    flat.play(&h);
    nested.play(&h);
    let press = nested.bar_event(Phase::Press);
    assert_eq!(press.pos(), Position::new(14, 29));
    assert_eq!(
        press.bar.map(|b| (b.id, b.track)),
        Some((h_id_for(INNER), Rect::new(10, 29, 19, 1)))
    );
    assert_matches_reference(&nested, &flat);

    let before = (nested.seen.len(), flat.seen.len());
    let v = [
        (MouseKind::Down, (29, 24)),
        (MouseKind::Drag, (29, 27)),
        (MouseKind::Up, (29, 27)),
    ];
    flat.play(&v);
    nested.play(&v);
    assert!(nested.seen.len() > before.0 && flat.seen.len() > before.1);
    let press = nested
        .seen
        .iter()
        .rev()
        .find(|e| e.is(Phase::Press))
        .expect("vertical press");
    assert_eq!(press.bar.map(|b| b.id), Some(id_for(INNER)));
    assert_eq!(press.pos(), Position::new(29, 24));
    assert_matches_reference(&nested, &flat);
}

#[test]
fn nested_scrollbar_clipping_keeps_full_track_extent() {
    for click in [TrackClick::Jump, TrackClick::Page] {
        let build = |nested: bool| {
            let levels = if nested {
                vec![level(
                    OUTER,
                    Axes::VERTICAL,
                    Rect::new(0, 0, 30, 5),
                    Size::new(28, 40),
                )]
            } else {
                Vec::new()
            };
            let screen = Rect::new(0, 0, 30, if nested { 5 } else { 40 });
            let inner = ScrollView::new(INNER)
                .axes(Axes::VERTICAL)
                .track_click(click);
            Rig::new(
                levels,
                inner,
                Rect::new(2, 20, 20, 8),
                Size::new(18, 30),
                screen,
            )
        };
        let mut flat = build(false);
        let mut nested = build(true);
        nested.scroll_level(0, 0, 22);
        let visible = nested.hits.bar(id_for(INNER)).expect("a visible fragment");
        assert_eq!(
            visible.track.height, 5,
            "the registered screen bar is the clipped fragment"
        );

        let script = [
            (MouseKind::Down, (21, 26)),
            (MouseKind::Up, (21, 26)),
            (MouseKind::Down, (21, 24)),
            (MouseKind::Up, (21, 24)),
        ];
        flat.play(&script);
        nested.play(&script);
        let press = nested.bar_event(Phase::Press);
        assert_eq!(
            press.bar.map(|b| b.track),
            Some(Rect::new(21, 20, 1, 8)),
            "{click:?} the event carries the whole logical track"
        );
        assert_matches_reference(&nested, &flat);
    }
}

fn thumb_pressed_then_clipped_away() -> Rig {
    let mut nested = single(TrackClick::Page);
    nested.scroll_level(0, 0, 15);
    let thumb = nested.screen_of((21, 20));
    nested.mouse(MouseKind::Down, thumb);
    nested.levels[0].view.scroll_to(0, 28);
    nested.render();
    assert!(
        nested.hits.bar(id_for(INNER)).is_none(),
        "the scrollbar left the visible window"
    );
    nested
}

#[test]
fn nested_scrollbar_capture_survives_missing_region() {
    let mut nested = thumb_pressed_then_clipped_away();
    let mut flat = reference(TrackClick::Page);
    flat.play(&[(MouseKind::Down, (21, 20))]);

    flat.play(&[(MouseKind::Drag, (21, 23))]);
    nested.mouse(MouseKind::Drag, Position::new(21, 8));
    let drag = nested.bar_event(Phase::Drag);
    assert_eq!(drag.pos(), Position::new(21, 23));
    assert_eq!(drag.bar.map(|b| b.track), Some(Rect::new(21, 20, 1, 8)));
    assert_eq!(
        nested.inner.offset(),
        flat.inner.offset(),
        "the drag moves the inner view with the grab taken at press"
    );

    nested.mouse(MouseKind::Up, Position::new(21, 9));
    let end = nested.bar_event(Phase::DragEnd);
    assert_eq!(end.pos(), Position::new(21, 24));
    assert_eq!(end.bar.map(|b| b.track), Some(Rect::new(21, 20, 1, 8)));
    let release = nested.bar_event(Phase::Release);
    assert_eq!(release.pos(), Position::new(21, 24));
    assert_eq!(release.bar.map(|b| b.track), Some(Rect::new(21, 20, 1, 8)));
    assert!(nested.router.grabbed().is_none(), "the capture is cleared");
    assert!(nested.router.pressed.is_none());
}

#[test]
fn nested_scrollbar_release_without_drag_keeps_capture() {
    let mut nested = thumb_pressed_then_clipped_away();
    nested.mouse(MouseKind::Up, Position::new(21, 9));
    assert!(
        nested.seen.iter().all(|e| !e.is(Phase::DragEnd)),
        "no drag happened"
    );
    let release = nested.bar_event(Phase::Release);
    assert_eq!(release.pos(), Position::new(21, 24));
    assert_eq!(release.bar.map(|b| b.track), Some(Rect::new(21, 20, 1, 8)));
    assert!(nested.router.grabbed().is_none());
}

#[test]
fn nested_scrollbar_track_repeat_uses_screen_hit() {
    let mut nested = single(TrackClick::Page);
    nested.scroll_level(0, 0, 15);
    let mut flat = reference(TrackClick::Page);

    flat.at_owner(MouseKind::Down, (21, 24));
    nested.at_owner(MouseKind::Down, (21, 24));
    assert_eq!(nested.inner.offset(), flat.inner.offset());
    let first = nested.inner.offset();
    assert_ne!(first, (0, 0), "the press paged");
    assert!(
        nested.router.track_repeating(),
        "pointer rests on the track"
    );

    for rig in [&mut flat, &mut nested] {
        rig.now = 0;
        PointerRouter::tick(rig);
        rig.now = 300;
        PointerRouter::tick(rig);
        rig.render();
    }
    assert_eq!(
        nested.inner.offset(),
        flat.inner.offset(),
        "the held track press repeats exactly like the reference"
    );
    assert_ne!(nested.inner.offset(), first, "the repeat paged again");

    nested.mouse(MouseKind::Drag, Position::new(21, 22));
    assert!(
        !nested.router.track_repeating(),
        "owner y22 lies in the logical track, but the screen pointer left the visible one"
    );
}
