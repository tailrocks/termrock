mod common;

use common::*;
use std::time::Duration;

use junie_tui::clock::TestRequest;
use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::refusal::{clear_refusal, raise, take_refusal};
use junie_tui::runtime::Application;
use junie_tui::shell::{App, Shell, render};
use junie_tui::theme::{ColorLevel, Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::too_small::TooSmall;
use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

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
