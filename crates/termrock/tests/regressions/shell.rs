use crate::common::*;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use termrock::clock::TestRequest;
use termrock::core::event::{Input, Outcome};
use termrock::core::focus::FocusChange;
use termrock::core::id::WidgetId;
use termrock::core::refusal::{clear_refusal, raise, take_refusal};
use termrock::runtime::Application;
use termrock::shell::{App, Shell, render};
use termrock::theme::{ColorLevel, Theme, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::widgets::button::Button;
use termrock::widgets::too_small::TooSmall;

#[test]
fn the_shell_clears_an_untaken_refusal_at_the_end_of_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        let mut host = Host::new(Act::Raise(reason()));
        host.input(input);
        assert_eq!(take_refusal(), None, "{name}");
    }
}

#[test]
fn the_shell_clears_a_stale_refusal_before_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        raise(&reason());
        let mut host = Host::new(Act::Nothing);
        host.input(input);
        assert_eq!(host.seen_at_entry, Some(None), "{name}");
    }
}

#[test]
fn a_status_posted_by_a_consumed_key_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Secondary)));
}

#[test]
fn a_key_that_posts_nothing_is_not_a_change() {
    let mut host = Host::new(Act::Nothing);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Ignored);
}

type DrawHook = Box<dyn FnMut(&mut Shell<()>, &mut Vec<WidgetId>, usize)>;

struct Rows {
    shell: Shell<()>,
    visible: Vec<WidgetId>,
    disabled: bool,
    layer: bool,
    draws: usize,
    hook: Option<DrawHook>,
    blurs: Vec<FocusChange>,
    terminal: Terminal<TestBackend>,
}

impl Rows {
    fn new(visible: &[WidgetId]) -> Self {
        Self {
            shell: Shell::new(),
            visible: visible.to_vec(),
            disabled: false,
            layer: false,
            draws: 0,
            hook: None,
            blurs: Vec::new(),
            terminal: Terminal::new(TestBackend::new(40, 6)).expect("test terminal"),
        }
    }

    fn frame(&mut self) -> Buffer {
        let mut terminal = std::mem::replace(
            &mut self.terminal,
            Terminal::new(TestBackend::new(1, 1)).expect("scratch terminal"),
        );
        terminal
            .draw(|f| render(&mut *self, f))
            .expect("draw the frame");
        let buf = terminal.backend().buffer().clone();
        self.terminal = terminal;
        buf
    }

    fn cursor(&mut self) -> Position {
        self.terminal
            .backend_mut()
            .get_cursor_position()
            .expect("cursor")
    }

    fn pending(&mut self) -> bool {
        Application::take_render_invalidated(self)
    }

    fn gutter_rows(&mut self, buf: &Buffer) -> Vec<u16> {
        let bar = self.theme().glyphs.focus_bar;
        (0..buf.area.height)
            .filter(|&y| buf[(0, y)].symbol() == bar)
            .collect()
    }
}

impl App for Rows {
    type Status = ();

    fn shell(&self) -> &Shell<()> {
        &self.shell
    }

    fn shell_mut(&mut self) -> &mut Shell<()> {
        &mut self.shell
    }

    fn theme(&self) -> Theme {
        Theme::for_level(ColorLevel::TrueColor)
    }

    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new("rows").min(1, 1)
    }

    fn now_ms(&self) -> u64 {
        0
    }

    fn tick(&self) -> u64 {
        0
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn layer_open(&self) -> bool {
        self.layer
    }

    fn on_input(&mut self, _input: Input) -> Outcome {
        Outcome::Ignored
    }

    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        self.blurs.push(change);
        Outcome::Ignored
    }

    fn draw(&mut self, _area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.draws += 1;
        if let Some(hook) = self.hook.as_mut() {
            hook(&mut self.shell, &mut self.visible, self.draws);
        }
        ctx.cursor = Some(Position::new(self.draws as u16, 0));
        let bg = ctx.theme.canvas;
        for (y, id) in self.visible.iter().enumerate() {
            let area = Rect::new(0, y as u16, 12, 1);
            Button::primary(*id, "Item")
                .disabled(self.disabled)
                .render(area, buf, ctx, bg);
        }
    }
}

const A: WidgetId = WidgetId::of("rows.a");
const B: WidgetId = WidgetId::of("rows.b");
const C: WidgetId = WidgetId::of("rows.c");

#[test]
fn shell_first_frame_paints_assigned_focus() {
    let mut rows = Rows::new(&[A]);

    let buf = rows.frame();

    assert_eq!(rows.shell.focus.current(), Some(A));
    assert_eq!(
        rows.gutter_rows(&buf),
        [0],
        "the first frame shows its focus"
    );
    assert!(!rows.pending(), "a settled frame asks for nothing more");
}

#[test]
fn shell_stationary_pointer_hover_matches_frame() {
    let mut rows = Rows::new(&[A, B, C]);
    rows.frame();
    rows.shell.focus.set(Some(C));
    let hits = rows.shell.hits.clone();
    rows.shell.pointer.on_move(&hits, Position::new(1, 1));
    rows.frame();
    assert_eq!(rows.shell.pointer.hover, Some(B));

    rows.visible = vec![B, A, C];
    rows.shell
        .pointer
        .scrolled(Position::new(1, 1), Outcome::Changed);
    let moved = rows.frame();

    assert_eq!(
        rows.shell.pointer.hover,
        Some(A),
        "the row under the pointer"
    );
    assert_eq!(
        moved,
        rows.frame(),
        "the pixels of the scroll frame already show the new hover"
    );
}

#[test]
fn shell_stable_frame_draws_once() {
    let mut rows = Rows::new(&[A, B]);
    rows.frame();
    rows.pending();
    let settled = rows.draws;

    for n in 1..=3 {
        rows.frame();
        assert_eq!(rows.draws, settled + n, "one draw for a stable frame");
        assert!(!rows.pending());
    }
}

#[test]
fn shell_empty_disabled_and_modal_rings_settle() {
    let mut empty = Rows::new(&[]);
    empty.frame();
    assert_eq!((empty.draws, empty.shell.focus.current()), (1, None));
    assert!(!empty.pending());

    let mut disabled = Rows::new(&[A, B]);
    disabled.disabled = true;
    let buf = disabled.frame();
    assert_eq!(disabled.draws, 1, "nothing focusable to settle");
    assert!(disabled.gutter_rows(&buf).is_empty());
    assert!(!disabled.pending());

    let mut modal = Rows::new(&[A, B]);
    modal.frame();
    modal.layer = true;
    let buf = modal.frame();
    assert!(
        modal.gutter_rows(&buf).is_empty(),
        "the page under a layer shows no focus"
    );
    let draws = modal.draws;
    modal.frame();
    assert_eq!(modal.draws, draws + 1, "the layer frame is stable");
    assert!(!modal.pending());
}

#[test]
fn shell_correction_delivers_blur_once() {
    let mut rows = Rows::new(&[A, B]);
    rows.frame();
    assert!(rows.blurs.is_empty());

    rows.visible = vec![B];
    let buf = rows.frame();

    assert_eq!(rows.shell.focus.current(), Some(B));
    assert_eq!(rows.gutter_rows(&buf), [0], "the fallback paints its focus");
    assert_eq!(
        rows.blurs,
        [FocusChange {
            from: A,
            to: Some(B)
        }],
        "the render delivers the change, once"
    );
    rows.frame();
    assert_eq!(rows.blurs.len(), 1, "later frames deliver nothing");
}

#[test]
fn shell_focus_changed_during_draw_is_corrected() {
    let mut rows = Rows::new(&[A, B]);
    rows.frame();
    let at = rows.draws + 1;
    rows.hook = Some(Box::new(move |shell, _, draw| {
        if draw == at {
            shell.focus.set(Some(B));
        }
    }));

    let buf = rows.frame();

    assert_eq!(
        rows.draws,
        at + 1,
        "a correction pass follows the side effect"
    );
    assert_eq!(rows.gutter_rows(&buf), [1], "the final pixels show B");
    assert!(!rows.pending());
}

#[test]
fn shell_unstable_second_pass_requests_next_turn() {
    let mut rows = Rows::new(&[A]);
    rows.hook = Some(Box::new(|_, visible, draw| {
        *visible = vec![if draw % 2 == 1 { A } else { B }];
    }));

    rows.frame();

    assert_eq!(rows.draws, 2, "exactly two paints, never a third");
    assert!(rows.pending(), "the unfinished settlement asks for a turn");
    assert!(!rows.pending(), "the request is consumed once");
    assert_eq!(
        rows.cursor(),
        Position::new(2, 0),
        "the last pass owns the cursor"
    );
}

#[test]
fn an_error_cleared_by_the_input_is_a_change() {
    let mut host = Host::new(Act::Nothing);
    host.set_status("write failed", Tone::Error);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), None);
}

#[test]
fn an_identical_status_posted_again_is_not_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Consumed);
}

#[test]
fn the_same_text_in_another_tone_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Warning));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Warning)));
}

#[test]
fn the_same_tone_with_other_text_is_a_change() {
    let mut host = Host::new(Act::Status("second", Tone::Secondary));
    host.set_status("first", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("second".to_owned(), Tone::Secondary)));
}

#[test]
fn a_fence_is_acknowledged_once_and_never_reaches_the_app() {
    termrock::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    let out = host.input(Input::Paste(TestRequest::FENCE.to_owned()));

    assert_eq!(out, Outcome::Ignored);
    assert_eq!(host.inputs, 0, "the app never sees a fence");
    assert_eq!(
        termrock::shell::clipboard::take_output(),
        [TestRequest::FENCE_ACK],
        "one acknowledgement per fence"
    );
}

#[test]
fn the_fence_opt_in_follows_its_own_variable() {
    let set = std::env::var_os(TestRequest::FENCE_ENV).is_some_and(|v| v == "1");
    assert_eq!(TestRequest::fence_opted_in(), set);
    assert_ne!(
        TestRequest::FENCE_ENV,
        TestRequest::ENV,
        "the fence has its own variable, so the clock stays unfrozen"
    );
}

#[test]
fn the_fence_alone_is_acknowledged_and_a_clock_step_stays_a_paste() {
    termrock::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(false);
    host.fence_requests(true);

    let out = host.input(Input::Paste(TestRequest::FENCE.to_owned()));
    assert_eq!(out, Outcome::Ignored);
    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(host.inputs, 1, "only the step reaches the app, as text");
    assert!(host.stepped.is_empty());
    assert_eq!(
        termrock::shell::clipboard::take_output(),
        [TestRequest::FENCE_ACK]
    );
}

#[test]
fn a_clock_step_reaches_the_app_as_a_step_and_not_as_a_paste() {
    termrock::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    host.input(Input::Paste(TestRequest::advance(250)));
    host.input(Input::Paste(TestRequest::advance(40)));

    assert_eq!(host.stepped, [250, 40]);
    assert_eq!(host.inputs, 0);
}

#[test]
fn test_requests_are_plain_pastes_unless_the_shell_accepts_them() {
    termrock::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(false);

    host.input(Input::Paste(TestRequest::FENCE.to_owned()));
    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(host.inputs, 2, "both reach the app as text");
    assert!(host.stepped.is_empty());
    assert_eq!(
        termrock::shell::clipboard::take_output(),
        Vec::<String>::new()
    );
}

#[test]
fn a_stop_handle_ends_the_loop_from_another_thread() {
    use termrock::runtime::Application;

    let host = Host::new(Act::Nothing);
    assert!(!host.should_quit());

    let stop = host.stop_handle();
    std::thread::spawn(move || stop.stop())
        .join()
        .expect("the stopping thread");

    assert!(
        host.should_quit(),
        "the loop reads should_quit after a stop"
    );
}

mod keyboard_transition {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Position, Rect};
    use std::time::Duration;
    use termrock::core::event::{Key, Mouse, MouseKind};
    use termrock::core::focus::FocusRing;
    use termrock::core::hit::HitRegistry;
    use termrock::core::id::WidgetId;
    use termrock::core::keymap::KeyBinding;
    use termrock::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
    use termrock::core::response::Response;
    use termrock::shell::{
        App, KeyHost, KeyRouter, PageKey, Shell, ShellAction, ShellKeys, handle,
    };
    use termrock::theme::{ColorLevel, Theme};
    use termrock::ui::ctx::{FocusVisible, RenderCtx, fill};
    use termrock::widgets::button::Button;
    use termrock::widgets::too_small::TooSmall;

    const SIZE: Rect = Rect::new(0, 0, 60, 8);
    const SLOT: Rect = Rect::new(4, 3, 14, 1);
    const OFF_BUTTON: (u16, u16) = (40, 6);
    const TIP_DELAY: u64 = 700;
    const FLASH_SETTLE: u64 = 200;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Cmd {
        Ping,
    }

    fn id() -> WidgetId {
        WidgetId::of("kt.button")
    }

    fn button(visible: FocusVisible, disabled: bool) -> Button {
        Button::secondary(id(), "Run task")
            .tooltip("Start the task in the background")
            .disabled(disabled)
            .focus_visible(visible)
    }

    struct Fixture {
        shell: Shell<()>,
        keys: ShellKeys<Cmd>,
        button: Button,
        page_out: Outcome,
        page_keys: usize,
        pings: usize,
        now: u64,
        draws: usize,
        frame: Buffer,
    }

    impl Fixture {
        fn new(visible: FocusVisible, page_out: Outcome) -> Self {
            Self::with(button(visible, false), page_out)
        }

        fn with(button: Button, page_out: Outcome) -> Self {
            let mut keys = ShellKeys::new();
            keys.keys.push((
                KeyBinding::key(KeyCode::Char('p')),
                ShellAction::App(Cmd::Ping),
            ));
            let mut fixture = Self {
                shell: Shell::new(),
                keys,
                button,
                page_out,
                page_keys: 0,
                pings: 0,
                now: 0,
                draws: 0,
                frame: Buffer::empty(SIZE),
            };
            fixture.paint();
            fixture.paint();
            fixture.draws = 0;
            fixture
        }

        fn paint(&mut self) {
            let theme = Theme::for_level(ColorLevel::TrueColor);
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let interaction = self.shell.interaction(self.now, self.now, 0);
            let mut buf = Buffer::empty(SIZE);
            fill(&mut buf, SIZE, theme.base());
            {
                let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
                self.button.render(SLOT, &mut buf, &mut ctx, theme.canvas);
                ctx.flush_overlays(&mut buf);
            }
            self.shell.hits = hits;
            self.shell.ring = ring;
            self.shell.pointer.refresh_hover(&self.shell.hits);
            self.shell.upkeep(false, PageKey::ROOT, Some(id()));
            self.frame = buf;
            self.draws += 1;
        }

        fn send(&mut self, input: Input) -> Outcome {
            let out = handle(self, input);
            if out == Outcome::Changed {
                self.paint();
            }
            out
        }

        fn key(&mut self, code: KeyCode) -> Outcome {
            self.send(Input::Key(key(code)))
        }

        fn mouse(&mut self, kind: MouseKind, x: u16, y: u16) -> Outcome {
            self.send(Input::Mouse(Mouse::new(kind, Position::new(x, y))))
        }

        fn hover_button(&mut self) {
            self.mouse(MouseKind::Move, SLOT.x + 2, SLOT.y);
        }

        fn click_button_then_leave(&mut self) {
            self.mouse(MouseKind::Move, SLOT.x + 2, SLOT.y);
            self.mouse(MouseKind::Down, SLOT.x + 2, SLOT.y);
            self.mouse(MouseKind::Up, SLOT.x + 2, SLOT.y);
            self.mouse(MouseKind::Move, OFF_BUTTON.0, OFF_BUTTON.1);
            self.wait(FLASH_SETTLE);
        }

        fn wait(&mut self, ms: u64) {
            self.now += ms;
            self.paint();
        }
    }

    fn reference(visible: FocusVisible, focused: bool) -> Buffer {
        let mut b = button(visible, false);
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let (mut buf, _) = draw(focused.then(id), |buf, ctx| {
            fill(buf, SIZE, theme.base());
            b.render(SLOT, buf, ctx, theme.canvas);
        });
        buf.area = SIZE;
        buf
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height).map(|y| row(buf, y)).collect()
    }

    fn styles(buf: &Buffer) -> Vec<(ratatui::style::Color, ratatui::style::Color)> {
        buf.content().iter().map(|c| (c.fg, c.bg)).collect()
    }

    fn same_cells(a: &Buffer, b: &Buffer) -> bool {
        rows(a) == rows(b) && styles(a) == styles(b)
    }

    impl App for Fixture {
        type Status = ();

        fn shell(&self) -> &Shell<()> {
            &self.shell
        }

        fn shell_mut(&mut self) -> &mut Shell<()> {
            &mut self.shell
        }

        fn theme(&self) -> Theme {
            Theme::for_level(ColorLevel::TrueColor)
        }

        fn too_small(&self) -> TooSmall<'_> {
            TooSmall::new("fixture")
        }

        fn now_ms(&self) -> u64 {
            self.now
        }

        fn tick(&self) -> u64 {
            0
        }

        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }

        fn draw(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

        fn on_input(&mut self, input: Input) -> Outcome {
            match input {
                Input::Mouse(m) => PointerRouter::route(self, m),
                other => KeyRouter::input(self, other),
            }
        }
    }

    impl PointerHost for Fixture {
        fn pointer_parts(&mut self) -> PointerParts<'_> {
            self.shell.pointer_parts()
        }

        fn layer_open(&self) -> bool {
            false
        }

        fn now_ms(&self) -> u64 {
            self.now
        }

        fn deliver(&mut self, _ev: &PointerEvent) -> Response<()> {
            Response::default()
        }
    }

    impl KeyHost for Fixture {
        type Cmd = Cmd;

        fn shell_keys(&self) -> &ShellKeys<Cmd> {
            &self.keys
        }

        fn shell_action(&mut self, action: ShellAction<Cmd>, _key: &Key) -> Outcome {
            if action == ShellAction::App(Cmd::Ping) {
                self.pings += 1;
                return Outcome::Consumed;
            }
            Outcome::Ignored
        }

        fn page_key(&mut self, _key: &Key) -> Outcome {
            self.page_keys += 1;
            self.page_out
        }

        fn request_quit(&mut self) -> Outcome {
            Outcome::Ignored
        }
    }

    #[test]
    fn keyboard_transition_repaints_hover_on_unbound_key() {
        let mut host = Fixture::new(FocusVisible::Always, Outcome::Ignored);
        let rest = reference(FocusVisible::Always, true);
        assert!(same_cells(&host.frame, &rest), "fixture starts at rest");

        host.hover_button();
        assert!(
            !same_cells(&host.frame, &rest),
            "hover is painted before the key"
        );
        let drawn = host.draws;

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Changed);
        assert_eq!(host.page_keys, 1, "the page still saw the key");
        assert_eq!(host.draws, drawn + 1, "one repaint for the transition");
        assert!(
            same_cells(&host.frame, &rest),
            "the repaint drops the hover style"
        );

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Ignored);
        assert_eq!(
            host.draws,
            drawn + 1,
            "a repeat of the key does not repaint"
        );
    }

    #[test]
    fn keyboard_transition_repaints_hover_on_a_disabled_control() {
        let mut host = Fixture::with(button(FocusVisible::Always, true), Outcome::Ignored);
        host.hover_button();
        let drawn = host.draws;

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Changed);
        assert_eq!(host.draws, drawn + 1);
        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Ignored);
        assert_eq!(host.draws, drawn + 1);
    }

    #[test]
    fn keyboard_transition_repaints_keyboard_only_focus() {
        let mut host = Fixture::new(FocusVisible::KeyboardOnly, Outcome::Consumed);
        let hidden = reference(FocusVisible::KeyboardOnly, false);
        let shown = reference(FocusVisible::KeyboardOnly, true);
        assert!(!same_cells(&hidden, &shown), "the gutter is the difference");

        host.click_button_then_leave();
        assert!(
            same_cells(&host.frame, &hidden),
            "a clicked button shows no keyboard focus"
        );
        let drawn = host.draws;

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Changed);
        assert_eq!(host.draws, drawn + 1);
        assert!(
            same_cells(&host.frame, &shown),
            "the first key paints the focus gutter"
        );

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Consumed);
        assert_eq!(host.draws, drawn + 1, "the second key does not repaint");
        assert!(same_cells(&host.frame, &shown));
    }

    #[test]
    fn keyboard_transition_clears_visible_tooltip() {
        let mut host = Fixture::new(FocusVisible::Always, Outcome::Ignored);
        let rest = reference(FocusVisible::Always, true);

        host.hover_button();
        host.wait(TIP_DELAY);
        assert!(
            rows(&host.frame)
                .iter()
                .any(|r| r.contains("Start the task")),
            "the tooltip is revealed after the delay"
        );

        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Changed);
        assert!(
            !rows(&host.frame)
                .iter()
                .any(|r| r.contains("Start the task")),
            "the tooltip is gone from the next frame"
        );
        assert!(
            same_cells(&host.frame, &rest),
            "no lingering hover style either"
        );
    }

    #[test]
    fn keyboard_transition_routes_action_once() {
        let mut host = Fixture::new(FocusVisible::Always, Outcome::Ignored);
        host.hover_button();
        let drawn = host.draws;

        assert_eq!(host.key(KeyCode::Char('p')), Outcome::Changed);
        assert_eq!(host.pings, 1, "the bound command ran exactly once");
        assert_eq!(
            host.page_keys, 0,
            "a consumed command never reaches the page"
        );
        assert_eq!(host.draws, drawn + 1);

        assert_eq!(host.key(KeyCode::Char('p')), Outcome::Consumed);
        assert_eq!(host.pings, 2);
        assert_eq!(host.draws, drawn + 1);
    }

    #[test]
    fn keyboard_transition_leaves_a_resting_shell_unrepainted() {
        let mut host = Fixture::new(FocusVisible::KeyboardOnly, Outcome::Ignored);
        assert_eq!(host.key(KeyCode::Char('x')), Outcome::Ignored);
        assert_eq!(host.draws, 0);
    }

    #[test]
    fn keyboard_transition_restores_hover_after_the_pointer_moves() {
        let mut host = Fixture::new(FocusVisible::Always, Outcome::Ignored);
        let rest = reference(FocusVisible::Always, true);
        host.hover_button();
        let hovered = host.frame.clone();
        host.key(KeyCode::Char('x'));
        assert!(same_cells(&host.frame, &rest));

        host.mouse(MouseKind::Move, SLOT.x + 3, SLOT.y);
        assert!(same_cells(&host.frame, &hovered), "a move restores hover");
    }

    #[test]
    fn keyboard_transition_synthetic_replay_preserves_pointer_mode() {
        let mut host = Fixture::new(FocusVisible::KeyboardOnly, Outcome::Consumed);
        host.click_button_then_leave();
        let before = (
            host.shell.pointer.hover,
            host.shell.pointer.hover_suppressed,
            host.shell.pointer.pointer_focus,
        );
        assert!(before.2, "the click left pointer focus on");

        assert_eq!(
            KeyRouter::replay(&mut host, &key(KeyCode::Char('x'))),
            Outcome::Consumed
        );
        assert_eq!(
            KeyRouter::replay(&mut host, &key(KeyCode::Char('p'))),
            Outcome::Consumed
        );

        let after = (
            host.shell.pointer.hover,
            host.shell.pointer.hover_suppressed,
            host.shell.pointer.pointer_focus,
        );
        assert_eq!(after, before, "a synthetic replay is not a physical key");
        assert_eq!(host.pings, 1);
    }
}

mod layer_focus {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use termrock::core::event::{Key, Outcome};
    use termrock::core::focus::{Focus, FocusChange, FocusRing};
    use termrock::core::id::WidgetId;
    use termrock::ui::ctx::RenderCtx;
    use termrock::ui::layer::{
        DismissReason, Dismissed, Layer, LayerCx, LayerOut, LayerSpec, LayerTag, ShellLayers,
    };
    use termrock::ui::modal::ModalStack;
    use termrock::ui::popup::Dismiss;

    const A: WidgetId = WidgetId::of("lf.a");
    const B: WidgetId = WidgetId::of("lf.b");
    const C: WidgetId = WidgetId::of("lf.c");
    const D: WidgetId = WidgetId::of("lf.d");
    const E: WidgetId = WidgetId::of("lf.e");

    struct Probe {
        spec: LayerSpec,
        out: Option<LayerOut<()>>,
    }

    impl Probe {
        fn boxed(spec: LayerSpec) -> Box<dyn Layer<(), ()>> {
            Box::new(Self { spec, out: None })
        }
    }

    impl Layer<(), ()> for Probe {
        fn spec(&self) -> LayerSpec {
            self.spec
        }
        fn render(&mut self, _: Rect, _: &mut Buffer, _: &mut RenderCtx, _: &()) {}
        fn on_key(&mut self, _: &Key, _: &mut LayerCx<'_, ()>) -> Outcome {
            Outcome::Ignored
        }
        fn on_dismiss(&mut self, _: DismissReason) {}
        fn take_result(&mut self) -> Option<LayerOut<()>> {
            self.out.take()
        }
    }

    type Layers = ShellLayers<(), ()>;

    fn modal(initial: WidgetId, restore: bool) -> Box<dyn Layer<(), ()>> {
        Probe::boxed(
            LayerSpec::modal(Dismiss::LIGHT)
                .initial_focus(initial)
                .restore_focus(restore),
        )
    }

    fn focused(id: WidgetId) -> Focus {
        let mut focus = Focus::default();
        focus.set(Some(id));
        focus.take_changes();
        focus
    }

    fn finish(layers: &mut Layers) {
        layers.top_as_mut::<Probe>().expect("probe on top").out = Some(LayerOut::Close(()));
    }

    fn close_by_result(layers: &mut Layers, focus: &mut Focus) {
        finish(layers);
        layers.take(focus).expect("the layer closes");
    }

    fn ring_of(ids: &[WidgetId]) -> FocusRing {
        let mut ring = FocusRing::default();
        for id in ids {
            ring.register(*id);
        }
        ring
    }

    #[test]
    fn layer_no_restore_keeps_action_focus() {
        for (restore, after) in [(false, C), (true, A)] {
            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            assert_eq!(focus.current(), Some(B));

            focus.set(Some(C));
            close_by_result(&mut layers, &mut focus);

            assert_eq!(focus.current(), Some(after), "restore_focus = {restore}");
        }
    }

    #[test]
    fn layer_no_restore_dismiss_and_clear() {
        for (restore, after) in [(false, C), (true, A)] {
            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            focus.set(Some(C));
            let closed = layers.dismiss(DismissReason::Esc, &mut focus);
            assert!(matches!(closed, Dismissed::Closed(..)));
            assert_eq!(focus.current(), Some(after), "dismiss, restore = {restore}");

            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            focus.set(Some(C));
            layers.clear(&mut focus);
            assert!(layers.is_empty());
            assert_eq!(focus.current(), Some(after), "clear, restore = {restore}");
        }
    }

    #[test]
    fn nested_clear_matches_top_down_pops_without_intermediate_changes() {
        let flag_sets = [
            ([true, false, true], A),
            ([false, true, false], B),
            ([false, false, false], E),
            ([true, true, true], A),
            ([true, false, false], A),
            ([false, false, true], C),
        ];
        for (flags, destination) in flag_sets {
            let open_all = |layers: &mut Layers, focus: &mut Focus| {
                for (initial, restore) in [B, C, D].into_iter().zip(flags) {
                    layers.open(modal(initial, restore), LayerTag::new("t"), (), focus);
                }
                focus.set(Some(E));
                focus.take_changes();
            };

            let mut popped = Layers::new();
            let mut popped_focus = focused(A);
            open_all(&mut popped, &mut popped_focus);
            while popped.pop(&mut popped_focus).is_some() {}

            let mut layers = Layers::new();
            let mut focus = focused(A);
            open_all(&mut layers, &mut focus);
            layers.clear(&mut focus);

            assert_eq!(focus.current(), popped_focus.current(), "flags {flags:?}");
            assert_eq!(focus.current(), Some(destination), "flags {flags:?}");
            let changes = focus.take_changes();
            let expected: Vec<FocusChange> = if focus.current() == Some(E) {
                Vec::new()
            } else {
                vec![FocusChange {
                    from: E,
                    to: focus.current(),
                }]
            };
            assert_eq!(changes, expected, "flags {flags:?}");
        }
    }

    #[test]
    fn tooltip_without_initial_focus_preserves_owner() {
        let mut layers = Layers::new();
        let mut focus = focused(A);

        layers.open(
            Probe::boxed(LayerSpec::tooltip()),
            LayerTag::new("tip"),
            (),
            &mut focus,
        );
        assert_eq!(focus.current(), Some(A), "opening the tooltip keeps focus");
        assert!(focus.take_changes().is_empty(), "no transient blur on open");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A));
        assert!(focus.take_changes().is_empty(), "no blur on close");
    }

    #[test]
    fn tooltip_close_keeps_a_focus_the_owner_moved() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(
            Probe::boxed(LayerSpec::tooltip()),
            LayerTag::new("tip"),
            (),
            &mut focus,
        );
        focus.set(Some(C));
        let closed = layers.dismiss(DismissReason::Esc, &mut focus);
        assert!(matches!(closed, Dismissed::Closed(..)));
        assert_eq!(focus.current(), Some(C));
    }

    #[test]
    fn modal_without_initial_focus_uses_ring_fallback() {
        let mut layers = Layers::new();
        let mut focus = focused(A);

        layers.open(
            Probe::boxed(LayerSpec::modal(Dismiss::LIGHT)),
            LayerTag::new("m"),
            (),
            &mut focus,
        );
        assert_eq!(focus.current(), None, "an inert layer clears the old focus");

        let ring = ring_of(&[B, C]);
        focus.ensure_valid(&ring);
        assert_eq!(focus.current(), Some(B), "the ring picks its first control");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A), "the modal default restores");
    }

    #[test]
    fn retain_invalidates_removed_layer_return_targets() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(modal(B, true), LayerTag::new("low"), (), &mut focus);
        layers.open(modal(C, true), LayerTag::new("top"), (), &mut focus);
        assert_eq!(focus.current(), Some(C));

        layers.retain(|e| e.tag.kind != "low");
        assert_eq!(layers.len(), 1);
        assert_eq!(focus.current(), Some(C), "retain leaves focus alone");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(
            focus.current(),
            None,
            "the top layer's saved target (inside the removed layer) is not revived"
        );
        let ring = ring_of(&[A]);
        focus.ensure_valid(&ring);
        assert_eq!(
            focus.current(),
            Some(A),
            "shared fallback picks a valid control"
        );
    }

    #[test]
    fn retain_keeps_return_targets_when_nothing_below_was_removed() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(modal(B, true), LayerTag::new("low"), (), &mut focus);
        layers.open(modal(C, true), LayerTag::new("top"), (), &mut focus);

        layers.retain(|e| e.tag.kind != "top");
        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A));
    }

    #[test]
    fn legacy_modal_push_preserves_default_restore() {
        let mut stack: ModalStack<u8> = ModalStack::new();
        let mut focus = focused(A);

        stack.push(1, &mut focus, Some(B));
        assert_eq!(focus.current(), Some(B));
        stack.push(2, &mut focus, None);
        assert_eq!(focus.current(), None);

        assert_eq!(stack.pop(&mut focus), Some(2));
        assert_eq!(focus.current(), Some(B));
        focus.set(Some(C));
        stack.clear(&mut focus);
        assert_eq!(focus.current(), Some(A));
    }

    #[test]
    fn push_with_focus_policy_separates_open_from_close() {
        use termrock::ui::modal::FocusOnOpen;

        let mut stack: ModalStack<u8> = ModalStack::new();
        let mut focus = focused(A);

        stack.push_with_focus_policy(1, &mut focus, FocusOnOpen::Preserve, true);
        assert_eq!(focus.current(), Some(A), "Preserve leaves focus alone");
        focus.set(Some(B));
        stack.pop(&mut focus);
        assert_eq!(
            focus.current(),
            Some(A),
            "restore = true returns to the opener"
        );

        stack.push_with_focus_policy(2, &mut focus, FocusOnOpen::Set(Some(C)), false);
        assert_eq!(focus.current(), Some(C));
        focus.set(Some(D));
        stack.pop(&mut focus);
        assert_eq!(
            focus.current(),
            Some(D),
            "restore = false keeps the moved focus"
        );
    }
}
