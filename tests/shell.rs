mod common;

use common::*;
use junie_tui::clock::TestRequest;
use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::refusal::{clear_refusal, raise, take_refusal};
use junie_tui::theme::Tone;
use ratatui::crossterm::event::KeyCode;

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
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    let out = host.input(Input::Paste(TestRequest::FENCE.to_owned()));

    assert_eq!(out, Outcome::Ignored);
    assert_eq!(host.inputs, 0, "the app never sees a fence");
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        [TestRequest::FENCE_ACK],
        "one acknowledgement per fence"
    );
}

#[test]
fn a_clock_step_reaches_the_app_as_a_step_and_not_as_a_paste() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    host.input(Input::Paste(TestRequest::advance(250)));
    host.input(Input::Paste(TestRequest::advance(40)));

    assert_eq!(host.stepped, [250, 40]);
    assert_eq!(host.inputs, 0);
}

#[test]
fn test_requests_are_plain_pastes_unless_the_shell_accepts_them() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(false);

    host.input(Input::Paste(TestRequest::FENCE.to_owned()));
    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(host.inputs, 2, "both reach the app as text");
    assert!(host.stepped.is_empty());
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        Vec::<String>::new()
    );
}

#[test]
fn a_stop_handle_ends_the_loop_from_another_thread() {
    use junie_tui::runtime::Application;

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
    use junie_tui::core::event::{Key, Mouse, MouseKind};
    use junie_tui::core::focus::FocusRing;
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::core::id::WidgetId;
    use junie_tui::core::keymap::KeyBinding;
    use junie_tui::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
    use junie_tui::core::response::Response;
    use junie_tui::shell::{
        App, KeyHost, KeyRouter, PageKey, Shell, ShellAction, ShellKeys, handle,
    };
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::{FocusVisible, RenderCtx, fill};
    use junie_tui::widgets::button::Button;
    use junie_tui::widgets::too_small::TooSmall;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Position, Rect};
    use std::time::Duration;

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
