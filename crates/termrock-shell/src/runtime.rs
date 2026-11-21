//! The application contract and the run loop that is generic over it.
//!
//! The terminal session, the external channel, the raw-input sources and
//! policies and the panic bridge live in `termrock-session` and are re-exported
//! here at their old paths. `Application` and the loop code live in this crate
//! beside `App`: with both in one crate `impl<A: App> Application for A` is
//! legal, and with `Application` in `termrock-session` it is not (E0210,
//! `docs/decisions.md`).

use std::io::Write;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::style::Colored;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};
use ratatui::style::Color;

use termrock_core::event::{Input, InputConfig, Mouse, MouseKind, Outcome};
use termrock_core::keyboard::{self, KeyboardProtocol};

#[cfg(test)]
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseEventKind};
#[cfg(test)]
use termrock_core::event::{External, Key};
#[cfg(test)]
use termrock_session::runtime::{RawModeControl, discard_pending};
#[cfg(all(test, feature = "work-metrics"))]
use termrock_session::runtime::{discard_ready, metered::MeteredBackend};

#[cfg(unix)]
pub use termrock_session::runtime::job_control;
pub use termrock_session::runtime::{
    Coalesce, ExternalPolicy, ExternalReceiver, ExternalSendError, ExternalSender, ExternalTurn,
    FocusClickGuard, RawInputPolicy, SessionTerminal, SizeWatch, TerminalSession,
    bounded_external_channel, drain_pending_input, drain_pending_input_bounded, external_channel,
    set_focus_click_guard,
};
use termrock_session::runtime::{
    CrosstermSource, RawBudget, RawRead, RawSource, poll_uninterrupted, scan_next,
};

pub trait Application {
    fn handle(&mut self, input: Input) -> Outcome;
    fn render(&mut self, frame: &mut Frame);
    fn should_quit(&self) -> bool;

    fn tick_interval(&self) -> Duration;

    fn coalesce(&self) -> Coalesce {
        Coalesce::default()
    }

    fn shapes(&self) -> termrock_core::shape::Shapes {
        termrock_core::shape::Shapes::default()
    }

    fn input_config(&self) -> InputConfig {
        InputConfig::default()
    }

    fn raw_input_policy(&self) -> RawInputPolicy {
        RawInputPolicy::default()
    }

    fn keyboard_protocol(&self) -> KeyboardProtocol {
        KeyboardProtocol::current()
    }

    fn take_output(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// Consumes the request for another draw left by the last draw. The
    /// event loop calls it after every draw and keeps drawing while it
    /// returns true.
    fn take_render_invalidated(&mut self) -> bool {
        false
    }

    fn external(&self) -> Option<&ExternalReceiver> {
        None
    }
}

/// Hands the application one bounded group of queued external messages: the
/// runtime's per-turn budget, exposed so an example or test can run it
/// without a terminal.
///
/// The turn handles at most `policy.max_messages` messages (a zero count is
/// one) and stops early once `elapsed()` reaches `policy.max_duration`.
/// `elapsed` reports the time since the turn began; the runtime passes a
/// reading of a real clock and a test passes a fake one. The bound is checked
/// between messages, and the first message is always handled when one is
/// queued.
///
/// The receiver is read again before every message through
/// [`Application::external`], so a handler may replace or remove it. The turn
/// ends at once when [`Application::should_quit`] is true.
pub fn pump_external(
    app: &mut impl Application,
    policy: ExternalPolicy,
    elapsed: impl Fn() -> Duration,
) -> ExternalTurn {
    let mut turn = ExternalTurn::default();
    for count in 0..policy.normalized().max_messages {
        if app.should_quit() {
            return turn;
        }
        if count > 0 && elapsed() >= policy.max_duration {
            turn.exhausted = true;
            return turn;
        }
        let Some(message) = app.external().and_then(ExternalReceiver::try_recv) else {
            return turn;
        };
        termrock_metrics::record_work!(InputDispatches, 1usize);
        turn.changed |= app.handle(Input::External(message)) == Outcome::Changed;
        if app.should_quit() {
            return turn;
        }
    }
    turn.exhausted = true;
    turn
}

/// What one loop iteration does after terminal input: a bounded external
/// group, then the resize check, then the tick. The order and the quit checks
/// between them are the fairness contract between those sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ServiceTurn {
    changed: bool,
    quit: bool,
    ticked: bool,
}

fn service_turn(
    app: &mut impl Application,
    external_pending: &mut bool,
    elapsed: impl Fn() -> Duration,
    resize: impl FnOnce() -> Option<Input>,
    tick_due: impl FnOnce() -> bool,
) -> ServiceTurn {
    let mut turn = ServiceTurn::default();
    match app.external().map(ExternalReceiver::policy) {
        Some(policy) => {
            let external = pump_external(app, policy, elapsed);
            turn.changed |= external.changed;
            *external_pending = external.exhausted;
        }
        None => *external_pending = false,
    }
    if app.should_quit() {
        turn.quit = true;
        return turn;
    }
    if let Some(resize) = resize() {
        termrock_metrics::record_work!(InputDispatches, 1usize);
        turn.changed |= app.handle(resize) == Outcome::Changed;
    }
    if tick_due() {
        turn.ticked = true;
        termrock_metrics::record_work!(InputDispatches, 1usize);
        turn.changed |= app.handle(Input::Tick) == Outcome::Changed;
        turn.quit = app.should_quit();
    }
    turn
}

pub fn run(app: &mut impl Application) -> std::io::Result<()> {
    let mut session = TerminalSession::enter_with(app.keyboard_protocol())?;
    let result = event_loop(&mut session, app);
    let left = session.try_leave();
    result.and(left)
}

/// Test-only knob: `TERMROCK_DRAW_LAG_MS` holds every frame back by that many
/// milliseconds after the input that caused it was handled and before any byte
/// of it reaches the terminal. It makes a slow draw deterministic, so a
/// functional case that reads the screen after a fixed quiet wait instead of
/// waiting for the frame fails on every run, not only on a loaded machine.
fn draw_lag() -> Option<Duration> {
    static LAG: std::sync::OnceLock<Option<Duration>> = std::sync::OnceLock::new();
    *LAG.get_or_init(|| parse_draw_lag(std::env::var("TERMROCK_DRAW_LAG_MS").ok().as_deref()))
}

fn parse_draw_lag(value: Option<&str>) -> Option<Duration> {
    let ms: u64 = value?.parse().ok()?;
    (ms > 0).then(|| Duration::from_millis(ms))
}

fn first_poll_delay() -> Option<Duration> {
    std::env::var("TERMROCK_FIRST_POLL_DELAY_MS")
        .ok()?
        .parse()
        .ok()
        .map(Duration::from_millis)
}

/// Tells the application the size the terminal has after a suspend, and the
/// size watch that it was told.
#[cfg(unix)]
fn deliver_resume_resize(app: &mut impl Application, sizes: &mut SizeWatch, (w, h): (u16, u16)) {
    let resize = Input::Resize(w, h);
    sizes.observe(&resize);
    termrock_metrics::record_work!(InputDispatches, 1usize);
    app.handle(resize);
}

/// The terminal side of one loop turn: painting a frame and reading the
/// current size.
trait TurnTerminal {
    fn draw(&mut self, app: &mut impl Application, dirty: &mut bool) -> std::io::Result<()>;

    fn size(&self) -> Option<(u16, u16)>;
}

struct LiveTerminal<'a> {
    terminal: &'a mut SessionTerminal,
    suppress_colors: bool,
}

impl TurnTerminal for LiveTerminal<'_> {
    fn draw(&mut self, app: &mut impl Application, dirty: &mut bool) -> std::io::Result<()> {
        draw_frame(self.terminal, app, self.suppress_colors, dirty)
    }

    fn size(&self) -> Option<(u16, u16)> {
        terminal_size(self.terminal)
    }
}

/// The loop's bookkeeping between turns.
struct LoopState {
    pump: InputPump,
    sizes: SizeWatch,
    dirty: bool,
    last_tick: Instant,
    external_pending: bool,
    raw_pending: bool,
}

/// How a loop turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnEnd {
    Continue,
    Quit,
}

/// One loop iteration after the wait: a budgeted read of raw terminal input
/// with the draws it causes, then the bounded external group, the resize check
/// and the tick. The event loop runs exactly this body, so the fairness between
/// a raw-input flood and the other sources is tested through it.
fn loop_turn(
    app: &mut impl Application,
    terminal: &mut impl TurnTerminal,
    source: &mut impl RawSource,
    state: &mut LoopState,
    read_input: bool,
    hover_redraw: bool,
    interval: Duration,
) -> std::io::Result<TurnEnd> {
    if read_input {
        let changed = input_turn(
            app,
            &mut state.pump,
            source,
            &mut state.sizes,
            &mut state.raw_pending,
        )?;
        if app.should_quit() {
            return Ok(TurnEnd::Quit);
        }
        if changed {
            terminal.draw(app, &mut state.dirty)?;
            if state.pump.settle(app) {
                terminal.draw(app, &mut state.dirty)?;
            }
        }
    }
    if hover_redraw {
        state.dirty = true;
    }

    let started = Instant::now();
    let turn = service_turn(
        app,
        &mut state.external_pending,
        || started.elapsed(),
        || terminal.size().and_then(|size| state.sizes.check(size)),
        || {
            let due = state.last_tick.elapsed() >= interval;
            if due {
                state.last_tick = Instant::now();
            }
            due
        },
    );
    state.dirty |= turn.changed;
    if turn.quit {
        if turn.ticked && state.dirty {
            terminal.draw(app, &mut state.dirty)?;
        }
        return Ok(TurnEnd::Quit);
    }
    Ok(TurnEnd::Continue)
}

fn event_loop(session: &mut TerminalSession, app: &mut impl Application) -> std::io::Result<()> {
    let suppress_colors = Colored::ansi_color_disabled_memoized();
    let mut state = LoopState {
        pump: InputPump::new(app.coalesce()),
        sizes: SizeWatch::new(terminal_size(session.terminal())),
        dirty: true,
        last_tick: Instant::now(),
        external_pending: false,
        raw_pending: false,
    };

    let mut shapes =
        termrock_core::shape::ShapeSync::new(termrock_core::shape::ShapeSupport::current());
    let mut hover_due: Option<(u64, Instant)> = None;
    let mut first_poll_delay = first_poll_delay();
    loop {
        #[cfg(unix)]
        if job_control::take_request() {
            let size = session.suspend()?;

            shapes.forget();

            deliver_resume_resize(app, &mut state.sizes, size);
            state.dirty = true;
        }
        let terminal = session.terminal();
        if state.dirty {
            state.dirty = false;
            draw_frame(terminal, app, suppress_colors, &mut state.dirty)?;
            if let Some(delay) = first_poll_delay.take() {
                std::thread::sleep(delay);
            }
        }

        if shapes.apply(app.shapes(), terminal.backend_mut())? {
            terminal.backend_mut().flush()?;
        }

        let output = app.take_output();
        if !output.is_empty() {
            for seq in output {
                terminal.backend_mut().write_all(seq.as_bytes())?;
            }
            terminal.backend_mut().flush()?;
        }

        if app.should_quit() {
            return Ok(());
        }
        let interval = app.tick_interval();
        let mut wait = interval.saturating_sub(state.last_tick.elapsed());

        let now = Instant::now();
        hover_due = match (termrock_runtime::reveal::hover_wait(), hover_due) {
            (None, _) => None,
            (Some((frame, _)), Some((seen, due))) if frame == seen => Some((seen, due)),
            (Some((frame, ms)), _) => Some((frame, now + Duration::from_millis(ms))),
        };
        if let Some((_, due)) = hover_due {
            wait = wait.min(due.saturating_duration_since(now));
        }
        if let Some(external) = app.external() {
            wait = wait.min(external.wait_latency());
        }
        wait = turn_wait(
            wait,
            state.raw_pending || state.external_pending || state.dirty,
        );

        let read_input = state.raw_pending || state.pump.has_pending() || poll_uninterrupted(wait)?;
        let hover_redraw = hover_due.is_some_and(|(_, due)| due <= now + wait);
        let end = loop_turn(
            app,
            &mut LiveTerminal {
                terminal,
                suppress_colors,
            },
            &mut CrosstermSource,
            &mut state,
            read_input,
            hover_redraw,
            interval,
        )?;
        if end == TurnEnd::Quit {
            return Ok(());
        }
    }
}

fn terminal_size(terminal: &SessionTerminal) -> Option<(u16, u16)> {
    terminal.size().ok().map(|s| (s.width, s.height))
}

fn effective_input_config(app: InputConfig, protocol_active: bool) -> InputConfig {
    InputConfig {
        legacy_ctrl_punctuation: app.legacy_ctrl_punctuation && !protocol_active,
        ..app
    }
}

fn turn_wait(wait: Duration, pending: bool) -> Duration {
    if pending { Duration::ZERO } else { wait }
}

fn input_turn(
    app: &mut impl Application,
    pump: &mut InputPump,
    source: &mut impl RawSource,
    sizes: &mut SizeWatch,
    raw_pending: &mut bool,
) -> std::io::Result<bool> {
    pump.set_policy(app.coalesce());
    let config = effective_input_config(app.input_config(), keyboard::active());
    let mut budget = RawBudget::new(app.raw_input_policy(), source.now());
    pump.drain(app, || match scan_next(source, &mut budget, config)? {
        RawRead::Input(input) => {
            sizes.observe(&input);
            Ok(Some(input))
        }
        RawRead::Empty => {
            *raw_pending = false;
            Ok(None)
        }
        RawRead::Yielded => {
            *raw_pending = true;
            Ok(None)
        }
    })
}

pub const DRAIN_BUDGET: usize = 256;

pub fn drain_ready_inputs(
    app: &mut impl Application,
    next: impl FnMut() -> std::io::Result<Option<Input>>,
) -> std::io::Result<bool> {
    InputPump::new(Coalesce::OFF).drain(app, next)
}

#[derive(Debug, Clone, Default)]
pub struct InputPump {
    policy: Coalesce,

    pending: Option<Input>,

    settle: Option<Mouse>,
}

impl InputPump {
    pub fn new(policy: Coalesce) -> Self {
        Self {
            policy,
            pending: None,
            settle: None,
        }
    }

    pub fn set_policy(&mut self, policy: Coalesce) {
        self.policy = policy;
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn drain(
        &mut self,
        app: &mut impl Application,
        mut next: impl FnMut() -> std::io::Result<Option<Input>>,
    ) -> std::io::Result<bool> {
        let started = Instant::now();
        let mut unchanged = 0;

        let mut burst: Option<(Input, usize)> = None;
        self.settle = None;
        loop {
            if let Some((last, folded)) = &burst
                && (!self.policy.folds(last, last)
                    || *folded >= self.policy.max_batch
                    || started.elapsed() >= self.policy.max_latency)
            {
                break;
            }
            let input = match self.pending.take() {
                Some(input) => input,
                None => match next()? {
                    Some(input) => input,
                    None => break,
                },
            };
            if let Some((last, _)) = &burst
                && !self.policy.folds(last, &input)
            {
                self.pending = Some(input);
                break;
            }

            let changed = termrock_runtime::reveal::on_pointer_wheel(&input)
                .or_else(|| termrock_runtime::tooltip::on_input(&input))
                .unwrap_or_else(|| {
                    termrock_metrics::record_work!(InputDispatches, 1usize);
                    app.handle(input.clone())
                })
                == Outcome::Changed;
            if app.should_quit() {
                return Ok(changed || burst.is_some());
            }
            match &mut burst {
                Some((last, folded)) => {
                    *last = input;
                    *folded += 1;
                }
                None if changed => burst = Some((input, 1)),
                None => {
                    unchanged += 1;
                    if unchanged >= DRAIN_BUDGET {
                        return Ok(false);
                    }
                }
            }
        }
        if let Some((Input::Mouse(m), folded)) = &burst
            && m.kind == MouseKind::Move
            && *folded > 1
        {
            self.settle = Some(*m);
        }
        Ok(burst.is_some())
    }

    pub fn settle(&mut self, app: &mut impl Application) -> bool {
        match self.settle.take() {
            Some(m) => {
                termrock_metrics::record_work!(InputDispatches, 1usize);
                app.handle(Input::Mouse(m)) == Outcome::Changed
            }
            None => false,
        }
    }
}

fn draw_frame(
    terminal: &mut SessionTerminal,
    app: &mut impl Application,
    suppress_colors: bool,
    dirty: &mut bool,
) -> std::io::Result<()> {
    if let Some(lag) = draw_lag() {
        std::thread::sleep(lag);
    }
    queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
    let drawn = paint(terminal, app, suppress_colors, dirty);
    let ended = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
    drawn?;
    ended
}

fn paint<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut impl Application,
    suppress_colors: bool,
    dirty: &mut bool,
) -> Result<(), B::Error> {
    let drawn = terminal
        .draw(|f| render_frame(app, f, suppress_colors))
        .map(|_| ());
    *dirty |= app.take_render_invalidated();
    drawn
}

fn render_frame(app: &mut impl Application, frame: &mut Frame, suppress_colors: bool) {
    app.render(frame);
    if suppress_colors {
        reset_frame_colors(frame.buffer_mut());
    }
}

fn reset_frame_colors(buffer: &mut Buffer) {
    for cell in &mut buffer.content {
        cell.fg = Color::Reset;
        cell.bg = Color::Reset;
        cell.underline_color = Color::Reset;
    }
}

#[cfg(test)]
mod raw_input_tests {
    #[cfg(feature = "work-metrics")]
    mod work;

    use super::*;
    use ratatui::crossterm::event::{
        KeyEvent, KeyEventKind, KeyEventState, MouseButton, MouseEvent,
    };
    use std::collections::VecDeque;

    struct Script {
        queue: VecDeque<Event>,
        reads: usize,
        base: Instant,
        step: Duration,
        guard: FocusClickGuard,
        fail_read_at: Option<usize>,
        endless: Option<Event>,
    }

    impl Script {
        fn new(queue: impl IntoIterator<Item = Event>) -> Self {
            Self {
                queue: queue.into_iter().collect(),
                reads: 0,
                base: Instant::now(),
                step: Duration::ZERO,
                guard: FocusClickGuard::default(),
                fail_read_at: None,
                endless: None,
            }
        }

        fn releases_then(n: usize, tail: impl IntoIterator<Item = Event>) -> Self {
            Self::new((0..n).map(|_| release('r')).chain(tail))
        }

        fn endless(event: Event) -> Self {
            let mut script = Self::new([]);
            script.endless = Some(event);
            script
        }
    }

    impl RawSource for Script {
        fn poll(&mut self) -> std::io::Result<bool> {
            Ok(self.endless.is_some() || !self.queue.is_empty())
        }

        fn read(&mut self) -> std::io::Result<Event> {
            if self.fail_read_at == Some(self.reads) {
                return Err(std::io::Error::other("read failed"));
            }
            self.reads += 1;
            match &self.endless {
                Some(event) => Ok(event.clone()),
                None => Ok(self.queue.pop_front().expect("poll said ready")),
            }
        }

        fn dropped(&mut self, event: &Event) -> bool {
            let now = self.now();
            self.guard.drops(event, now)
        }

        fn now(&mut self) -> Instant {
            self.base + self.step * u32::try_from(self.reads).unwrap()
        }
    }

    fn release(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        })
    }

    fn press(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn key(c: char) -> Input {
        Input::Key(Key {
            code: KeyCode::Char(c),
            mods: KeyModifiers::NONE,
        })
    }

    fn policy(max_events: usize, max_duration: Duration) -> RawInputPolicy {
        RawInputPolicy {
            max_events,
            max_duration,
        }
    }

    fn budget(source: &mut Script, policy: RawInputPolicy) -> RawBudget {
        RawBudget::new(policy, source.now())
    }

    #[derive(Default)]
    struct Recorder {
        policy: Option<RawInputPolicy>,
        seen: Vec<Input>,
    }

    impl Application for Recorder {
        fn handle(&mut self, input: Input) -> Outcome {
            self.seen.push(input);
            Outcome::Ignored
        }
        fn render(&mut self, _frame: &mut Frame) {}
        fn should_quit(&self) -> bool {
            false
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
        fn coalesce(&self) -> Coalesce {
            Coalesce::OFF
        }
        fn raw_input_policy(&self) -> RawInputPolicy {
            self.policy.unwrap_or_default()
        }
    }

    struct Turns {
        app: Recorder,
        pump: InputPump,
        sizes: SizeWatch,
        raw_pending: bool,
    }

    impl Turns {
        fn new(policy: RawInputPolicy) -> Self {
            Self {
                app: Recorder {
                    policy: Some(policy),
                    seen: Vec::new(),
                },
                pump: InputPump::new(Coalesce::OFF),
                sizes: SizeWatch::default(),
                raw_pending: false,
            }
        }

        fn turn(&mut self, source: &mut Script) -> std::io::Result<bool> {
            input_turn(
                &mut self.app,
                &mut self.pump,
                source,
                &mut self.sizes,
                &mut self.raw_pending,
            )
        }
    }

    #[test]
    fn raw_input_filtered_stream_yields_at_budget() {
        let mut source = Script::releases_then(10_000, [press('a')]);
        let mut budget = budget(&mut source, policy(8, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 8);
        assert_eq!((budget.read(), budget.filtered()), (8, 8));
        assert_eq!(source.queue.len(), 10_001 - 8);
    }

    #[test]
    fn raw_input_budget_is_shared_across_accepted_inputs() {
        let queue = (b'a'..=b'h').flat_map(|c| [release('r'), press(char::from(c))]);
        let mut source = Script::new(queue);
        let mut turns = Turns::new(policy(8, Duration::from_secs(60)));
        turns.turn(&mut source).unwrap();
        assert_eq!(source.reads, 8);
        assert_eq!(turns.app.seen, [key('a'), key('b'), key('c'), key('d')]);
        assert!(turns.raw_pending);
    }

    #[test]
    fn raw_input_fifo_survives_yield() {
        let mut source = Script::releases_then(5, [press('A'), press('B')]);
        let mut turns = Turns::new(policy(3, Duration::from_secs(60)));
        turns.turn(&mut source).unwrap();
        assert!(turns.app.seen.is_empty());
        assert!(turns.raw_pending);
        turns.turn(&mut source).unwrap();
        assert_eq!(turns.app.seen, [key('A')]);
        assert!(turns.raw_pending);
        turns.turn(&mut source).unwrap();
        assert_eq!(turns.app.seen, [key('A'), key('B')]);
        assert!(!turns.raw_pending);
        assert_eq!(source.reads, 7);
    }

    /// An application that records which loop hooks reached it, with a raw
    /// input budget of its own and an external queue.
    struct Flooded {
        raw: RawInputPolicy,
        external: ExternalReceiver,
        seen: Vec<&'static str>,
    }

    impl Application for Flooded {
        fn handle(&mut self, input: Input) -> Outcome {
            let (name, outcome) = match input {
                Input::External(_) => ("external", Outcome::Changed),
                Input::Resize(..) => ("resize", Outcome::Ignored),
                Input::Tick => ("tick", Outcome::Ignored),
                Input::Key(_) => ("key", Outcome::Ignored),
                _ => ("other", Outcome::Ignored),
            };
            self.seen.push(name);
            outcome
        }
        fn render(&mut self, _frame: &mut Frame) {}
        fn should_quit(&self) -> bool {
            false
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
        fn coalesce(&self) -> Coalesce {
            Coalesce::OFF
        }
        fn raw_input_policy(&self) -> RawInputPolicy {
            self.raw
        }
        fn external(&self) -> Option<&ExternalReceiver> {
            Some(&self.external)
        }
    }

    /// A terminal that reports a new size on every look, so every loop turn
    /// has a resize to deliver.
    #[derive(Default)]
    struct Growing {
        looks: std::cell::Cell<u16>,
        draws: usize,
    }

    impl TurnTerminal for Growing {
        fn draw(&mut self, _app: &mut impl Application, _dirty: &mut bool) -> std::io::Result<()> {
            self.draws += 1;
            Ok(())
        }
        fn size(&self) -> Option<(u16, u16)> {
            self.looks.set(self.looks.get() + 1);
            Some((80 + self.looks.get(), 24))
        }
    }

    #[test]
    fn raw_input_flood_allows_tick_external_resize() {
        let (tx, external) = bounded_external_channel(
            std::num::NonZeroUsize::new(16).expect("nonzero"),
            ExternalPolicy {
                max_messages: 1,
                max_duration: Duration::from_secs(60),
            },
        );
        for n in 0..5_u32 {
            assert!(tx.send(n));
        }
        let mut app = Flooded {
            raw: policy(16, Duration::from_secs(60)),
            external,
            seen: Vec::new(),
        };
        let mut source = Script::endless(release('r'));
        let mut screen = Growing::default();
        let mut state = LoopState {
            pump: InputPump::new(Coalesce::OFF),
            sizes: SizeWatch::default(),
            dirty: false,
            last_tick: Instant::now(),
            external_pending: false,
            raw_pending: false,
        };

        for turn in 0..5 {
            let before = source.reads;
            let seen_before = app.seen.len();
            let end = loop_turn(
                &mut app,
                &mut screen,
                &mut source,
                &mut state,
                true,
                false,
                Duration::ZERO,
            )
            .unwrap();
            assert_eq!(end, TurnEnd::Continue);
            assert_eq!(
                source.reads - before,
                16,
                "turn {turn} read past its raw budget"
            );
            assert!(state.raw_pending, "the flood stays pending");
            assert_eq!(
                turn_wait(Duration::from_millis(100), state.raw_pending),
                Duration::ZERO
            );
            assert_eq!(
                app.seen[seen_before..],
                ["external", "resize", "tick"],
                "turn {turn}: the flood never starves the other sources, and they run in order"
            );
        }
        assert_eq!(
            app.seen.iter().filter(|s| **s == "key").count(),
            0,
            "the flood is all filtered release events"
        );
        assert_eq!(screen.draws, 0, "filtered input changes nothing to draw");
        assert!(state.dirty, "the external messages marked the frame dirty");

        let mut drained = Script::new([press('a')]);
        loop_turn(
            &mut app,
            &mut screen,
            &mut drained,
            &mut state,
            true,
            false,
            Duration::ZERO,
        )
        .unwrap();
        assert!(!state.raw_pending);
        assert_eq!(
            turn_wait(Duration::from_millis(100), state.raw_pending),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn raw_input_zero_duration_processes_one_then_yields() {
        let mut source = Script::releases_then(100, [press('a')]);
        let mut budget = budget(&mut source, policy(1000, Duration::ZERO));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 1);
    }

    #[test]
    fn raw_input_duration_is_charged_from_turn_start_between_reads() {
        let mut source = Script::releases_then(100, [press('a')]);
        source.step = Duration::from_millis(1);
        let mut budget = budget(&mut source, policy(1000, Duration::from_millis(3)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 3);
    }

    #[test]
    fn raw_input_zero_event_budget_normalizes_to_one() {
        let mut source = Script::releases_then(10, [press('a')]);
        let mut budget = budget(&mut source, policy(0, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Yielded);
        assert_eq!(source.reads, 1);
    }

    #[test]
    fn raw_input_charges_guard_drops_and_unsupported_events() {
        let mut source = Script::new([
            Event::FocusGained,
            mouse(MouseEventKind::Down(MouseButton::Left)),
            mouse(MouseEventKind::Up(MouseButton::Left)),
            mouse(MouseEventKind::Drag(MouseButton::Middle)),
            release('r'),
            Event::FocusLost,
            press('a'),
        ]);
        let mut budget = budget(&mut source, policy(6, Duration::from_secs(60)));
        let first = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(first, RawRead::Yielded);
        assert_eq!((budget.read(), budget.filtered()), (6, 6));
        let mut next = RawBudget::new(policy(6, Duration::from_secs(60)), source.now());
        let second = scan_next(&mut source, &mut next, InputConfig::default()).unwrap();
        assert_eq!(second, RawRead::Input(key('a')));
    }

    #[test]
    fn raw_input_accepted_input_ends_scan_without_lookahead() {
        let mut source = Script::new([press('a'), press('b')]);
        let mut budget = budget(&mut source, policy(8, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Input(key('a')));
        assert_eq!(source.reads, 1);
        assert_eq!(budget.filtered(), 0);
    }

    #[test]
    fn raw_input_empty_at_exact_budget_is_not_a_yield() {
        let mut source = Script::releases_then(4, []);
        let mut budget = budget(&mut source, policy(4, Duration::from_secs(60)));
        let read = scan_next(&mut source, &mut budget, InputConfig::default()).unwrap();
        assert_eq!(read, RawRead::Empty);
        assert_eq!(source.reads, 4);
    }

    #[test]
    fn raw_input_empty_and_read_error_are_distinct() {
        let mut turns = Turns::new(RawInputPolicy::default());
        turns.raw_pending = true;
        turns.turn(&mut Script::new([])).unwrap();
        assert!(!turns.raw_pending);

        let mut failing = Script::releases_then(3, [press('a')]);
        failing.fail_read_at = Some(2);
        turns.raw_pending = true;
        let error = turns.turn(&mut failing).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(turns.raw_pending);
        assert!(turns.app.seen.is_empty());
    }

    #[derive(Default)]
    struct Mode {
        raw: bool,
        fail_disable: bool,
        calls: Vec<&'static str>,
    }

    impl RawModeControl for Mode {
        fn enabled(&mut self) -> std::io::Result<bool> {
            self.calls.push("enabled");
            Ok(self.raw)
        }
        fn enable(&mut self) -> std::io::Result<()> {
            self.calls.push("enable");
            self.raw = true;
            Ok(())
        }
        fn disable(&mut self) -> std::io::Result<()> {
            self.calls.push("disable");
            if self.fail_disable {
                return Err(std::io::Error::other("disable failed"));
            }
            self.raw = false;
            Ok(())
        }
    }

    struct Interrupted {
        polls: usize,
    }

    impl RawSource for Interrupted {
        fn poll(&mut self) -> std::io::Result<bool> {
            self.polls += 1;
            Err(std::io::ErrorKind::Interrupted.into())
        }
        fn read(&mut self) -> std::io::Result<Event> {
            unreachable!("nothing is read after an interrupted poll")
        }
        fn dropped(&mut self, _event: &Event) -> bool {
            false
        }
        fn now(&mut self) -> Instant {
            Instant::now()
        }
    }

    #[test]
    fn startup_discard_treats_an_interrupted_poll_as_nothing_pending() {
        let mut source = Interrupted { polls: 0 };
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut source, &mut mode, 256).unwrap(), 0);
        assert_eq!(source.polls, 1);
        assert!(
            !mode.raw,
            "raw mode is restored after the interrupted drain"
        );
        assert_eq!(mode.calls, ["enabled", "enable", "disable"]);
    }

    #[test]
    fn startup_discard_is_bounded_and_restores_raw_mode() {
        let mut flood = Script::endless(release('r'));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut flood, &mut mode, 256).unwrap(), 256);
        assert_eq!(flood.reads, 256);
        assert!(!mode.raw);
        assert_eq!(mode.calls, ["enabled", "enable", "disable"]);

        let mut queued = Script::new((0..15).map(|_| release('r')).chain([press('a')]));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut queued, &mut mode, 10).unwrap(), 10);
        assert_eq!(queued.queue.len(), 6);

        let mut short = Script::new([Event::FocusGained, press('a')]);
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut short, &mut mode, 10).unwrap(), 2);

        let mut raw = Script::endless(release('r'));
        let mut mode = Mode {
            raw: true,
            ..Mode::default()
        };
        assert_eq!(discard_pending(&mut raw, &mut mode, 5).unwrap(), 5);
        assert!(mode.raw);
        assert_eq!(mode.calls, ["enabled"]);
    }

    #[test]
    fn startup_discard_zero_bound_touches_nothing() {
        let mut flood = Script::endless(release('r'));
        let mut mode = Mode::default();
        assert_eq!(discard_pending(&mut flood, &mut mode, 0).unwrap(), 0);
        assert_eq!(flood.reads, 0);
        assert!(mode.calls.is_empty());
    }

    #[test]
    fn default_policies_carry_the_documented_default_budgets() {
        let external = ExternalPolicy::default();
        assert_eq!(external.max_messages, ExternalPolicy::DEFAULT_MAX_MESSAGES);
        assert_eq!(external.max_duration, ExternalPolicy::DEFAULT_MAX_DURATION);
        assert_eq!(
            (
                ExternalPolicy::DEFAULT_MAX_MESSAGES,
                ExternalPolicy::DEFAULT_MAX_DURATION
            ),
            (64, Duration::from_millis(2))
        );

        let raw = RawInputPolicy::default();
        assert_eq!(raw.max_events, RawInputPolicy::DEFAULT_MAX_EVENTS);
        assert_eq!(raw.max_duration, RawInputPolicy::DEFAULT_MAX_DURATION);
        assert_eq!(
            (
                RawInputPolicy::DEFAULT_MAX_EVENTS,
                RawInputPolicy::DEFAULT_MAX_DURATION
            ),
            (256, Duration::from_millis(2))
        );
    }

    #[test]
    fn the_public_bounded_drain_with_a_zero_bound_reads_nothing() {
        assert_eq!(drain_pending_input_bounded(0).unwrap(), 0);
    }

    #[test]
    fn startup_discard_restores_raw_mode_on_scan_error_and_unwind() {
        let mut failing = Script::releases_then(3, []);
        failing.fail_read_at = Some(1);
        let mut mode = Mode::default();
        let error = discard_pending(&mut failing, &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(!mode.raw);

        struct Panicking;
        impl RawSource for Panicking {
            fn poll(&mut self) -> std::io::Result<bool> {
                panic!("poll panicked")
            }
            fn read(&mut self) -> std::io::Result<Event> {
                unreachable!()
            }
            fn dropped(&mut self, _event: &Event) -> bool {
                false
            }
            fn now(&mut self) -> Instant {
                Instant::now()
            }
        }
        let mut mode = Mode::default();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = discard_pending(&mut Panicking, &mut mode, 10);
        }));
        assert!(unwound.is_err());
        assert!(!mode.raw);
        assert_eq!(mode.calls, ["enabled", "enable", "disable"]);
    }

    #[test]
    fn startup_discard_error_precedence() {
        let scan_fails = || {
            let mut source = Script::releases_then(3, []);
            source.fail_read_at = Some(1);
            source
        };

        let mut mode = Mode::default();
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");

        let mut mode = Mode {
            fail_disable: true,
            ..Mode::default()
        };
        let error = discard_pending(&mut Script::releases_then(3, []), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "disable failed");

        let mut mode = Mode {
            fail_disable: true,
            ..Mode::default()
        };
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "disable failed");

        let mut mode = Mode {
            raw: true,
            fail_disable: true,
            ..Mode::default()
        };
        assert_eq!(
            discard_pending(&mut Script::releases_then(3, []), &mut mode, 10).unwrap(),
            3
        );
        let error = discard_pending(&mut scan_fails(), &mut mode, 10).unwrap_err();
        assert_eq!(error.to_string(), "read failed");
        assert!(!mode.calls.contains(&"disable"));
    }

    #[test]
    fn raw_input_policy_default_and_normalization() {
        let default = RawInputPolicy::default();
        assert_eq!(default.max_events, 256);
        assert_eq!(default.max_duration, Duration::from_millis(2));
        assert_eq!(policy(0, Duration::ZERO).event_limit(), 1);
    }
}

#[cfg(test)]
mod external_tests {
    use super::*;
    use std::num::NonZeroUsize;

    const LONG: Duration = Duration::from_secs(3600);

    fn policy(max_messages: usize) -> ExternalPolicy {
        ExternalPolicy {
            max_messages,
            max_duration: LONG,
        }
    }

    fn zero_clock() -> Duration {
        Duration::ZERO
    }

    #[derive(Default)]
    struct Fixture {
        receiver: Option<ExternalReceiver>,
        seen: Vec<String>,
        quit_on: Option<u32>,
        refill: Option<ExternalSender>,
        remove_on: Option<u32>,
        quit: bool,
    }

    impl Fixture {
        fn over(receiver: ExternalReceiver) -> Self {
            Self {
                receiver: Some(receiver),
                ..Self::default()
            }
        }

        fn numbers(&self) -> Vec<u32> {
            self.seen
                .iter()
                .filter_map(|s| s.strip_prefix('e')?.parse().ok())
                .collect()
        }
    }

    impl Application for Fixture {
        fn handle(&mut self, input: Input) -> Outcome {
            match input {
                Input::External(message) => {
                    let n = *message.downcast_ref::<u32>().expect("u32 payload");
                    self.seen.push(format!("e{n}"));
                    if self.quit_on == Some(n) {
                        self.quit = true;
                    }
                    if self.remove_on == Some(n) {
                        self.receiver = None;
                    }
                    if let Some(tx) = &self.refill {
                        assert!(tx.send(n + 1000));
                    }
                    Outcome::Changed
                }
                Input::Resize(..) => {
                    self.seen.push("r".into());
                    Outcome::Changed
                }
                Input::Tick => {
                    self.seen.push("t".into());
                    Outcome::Ignored
                }
                Input::Key(_) => {
                    self.seen.push("k".into());
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            }
        }
        fn render(&mut self, _frame: &mut Frame) {}
        fn should_quit(&self) -> bool {
            self.quit
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
        fn external(&self) -> Option<&ExternalReceiver> {
            self.receiver.as_ref()
        }
    }

    fn loaded(count: u32, policy: ExternalPolicy) -> (Fixture, ExternalSender) {
        let (tx, rx) = external_channel();
        for n in 0..count {
            assert!(tx.send(n));
        }
        (Fixture::over(rx.with_policy(policy)), tx)
    }

    fn pump(app: &mut Fixture) -> ExternalTurn {
        let policy = app.external().expect("receiver").policy();
        pump_external(app, policy, zero_clock)
    }

    fn rest(app: &Fixture) -> Vec<u32> {
        std::iter::from_fn(|| app.external()?.try_recv())
            .map(|m| *m.downcast_ref::<u32>().expect("u32 payload"))
            .collect()
    }

    #[test]
    fn external_turn_stops_at_count_and_keeps_fifo() {
        let (mut app, _tx) = loaded(10, policy(3));

        let first = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1, 2]);
        assert!(first.changed && first.exhausted);

        let second = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1, 2, 3, 4, 5]);
        assert!(second.exhausted);

        assert_eq!(rest(&app), [6, 7, 8, 9], "no extra dequeue, order intact");
    }

    #[test]
    fn external_turn_normalizes_a_zero_count_to_one_message() {
        let (mut app, _tx) = loaded(4, policy(0));
        assert_eq!(app.external().unwrap().policy().max_messages, 1);
        pump(&mut app);
        assert_eq!(app.numbers(), [0]);
    }

    #[test]
    fn external_turn_charges_handler_time() {
        let (mut app, _tx) = loaded(
            5,
            ExternalPolicy {
                max_messages: 64,
                max_duration: Duration::from_millis(2),
            },
        );
        let policy = app.external().unwrap().policy();
        let spent = pump_external(&mut app, policy, || Duration::from_millis(3));
        assert_eq!(app.numbers(), [0], "one handler consumed the budget");
        assert!(spent.exhausted);
        assert_eq!(rest(&app), [1, 2, 3, 4]);
    }

    #[test]
    fn external_turn_zero_duration_handles_one_then_yields() {
        let (mut app, _tx) = loaded(
            3,
            ExternalPolicy {
                max_messages: 64,
                max_duration: Duration::ZERO,
            },
        );
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0]);
        assert!(turn.exhausted);
    }

    #[test]
    fn external_turn_quit_stops_immediately() {
        let (mut app, _tx) = loaded(6, policy(64));
        app.quit_on = Some(0);
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0]);
        assert!(!turn.exhausted);
        assert_eq!(rest(&app), [1, 2, 3, 4, 5], "later messages stay queued");
    }

    #[test]
    fn external_turn_replenished_queue_yields() {
        let (mut app, tx) = loaded(1, policy(5));
        app.refill = Some(tx);
        let turn = pump(&mut app);
        assert_eq!(
            app.numbers().len(),
            5,
            "a refilling queue cannot hold the turn"
        );
        assert!(turn.exhausted);
    }

    #[test]
    fn external_turn_receiver_removed_by_handler() {
        let (mut app, _tx) = loaded(4, policy(64));
        app.remove_on = Some(1);
        let turn = pump(&mut app);
        assert_eq!(app.numbers(), [0, 1]);
        assert!(!turn.exhausted);
        assert!(app.external().is_none());
        let mut pending = true;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(!pending);
    }

    #[test]
    fn external_turn_empty_queue_is_not_pending() {
        let (mut app, _tx) = loaded(2, policy(8));
        let mut pending = true;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert_eq!(app.numbers(), [0, 1]);
        assert!(!pending, "the queue emptied inside the bound");
    }

    #[test]
    fn external_turn_exact_count_costs_one_empty_turn() {
        let (mut app, _tx) = loaded(3, policy(3));
        let mut pending = false;
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(pending, "conservatively pending at the bound");
        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert!(!pending);
        assert_eq!(app.numbers(), [0, 1, 2]);
    }

    #[test]
    fn external_backlog_allows_input_resize_and_tick() {
        let (mut app, _tx) = loaded(10, policy(3));
        let mut pending = false;

        let mut keys = vec![Input::Key(Key {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::NONE,
        })]
        .into_iter();
        drain_ready_inputs(&mut app, || Ok(keys.next())).expect("input drains");
        let first = service_turn(
            &mut app,
            &mut pending,
            zero_clock,
            || Some(Input::Resize(80, 24)),
            || true,
        );
        assert!(first.ticked && !first.quit);
        assert_eq!(app.seen, ["k", "e0", "e1", "e2", "r", "t"]);
        assert_eq!(
            turn_wait(Duration::from_millis(50), pending),
            Duration::ZERO
        );

        service_turn(&mut app, &mut pending, zero_clock, || None, || false);
        assert_eq!(app.seen[6..], ["e3", "e4", "e5"]);
    }

    #[test]
    fn external_backlog_quit_skips_resize_and_tick() {
        let (mut app, _tx) = loaded(5, policy(64));
        app.quit_on = Some(1);
        let mut pending = false;
        let turn = service_turn(
            &mut app,
            &mut pending,
            zero_clock,
            || Some(Input::Resize(80, 24)),
            || true,
        );
        assert!(turn.quit && !turn.ticked);
        assert_eq!(app.seen, ["e0", "e1"]);
    }

    #[test]
    fn turn_wait_is_immediate_only_while_pending() {
        let wait = Duration::from_millis(40);
        assert_eq!(turn_wait(wait, false), wait);
        assert_eq!(turn_wait(wait, true), Duration::ZERO);
    }

    fn capacity(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("nonzero")
    }

    #[test]
    fn bounded_external_full_and_closed_preserve_payload() {
        let (tx, rx) = bounded_external_channel(capacity(1), ExternalPolicy::default());
        assert!(tx.send(1_u32));
        let second = External::new(2_u32);
        let Err(ExternalSendError::Full(back)) = tx.try_send(second.clone()) else {
            panic!("a full queue answers Full");
        };
        assert_eq!(back, second, "the same payload comes back");
        assert!(!tx.send(3_u32), "send reports rejection as false");

        let first = rx.try_recv().expect("queued first");
        assert_eq!(first.downcast_ref::<u32>(), Some(&1));
        assert_eq!(tx.try_send(back), Ok(()), "the retry is accepted");
        let again = rx.try_recv().expect("queued retry");
        assert_eq!(again, second);

        let kept = External::new(4_u32);
        drop(rx);
        assert_eq!(
            tx.try_send(kept.clone()),
            Err(ExternalSendError::Closed(kept)),
        );
        assert!(!tx.send(5_u32));
    }

    #[test]
    fn bounded_external_unbounded_sender_reports_closed_with_payload() {
        let (tx, rx) = external_channel();
        drop(rx);
        let kept = External::new(7_u32);
        assert_eq!(
            tx.try_send(kept.clone()),
            Err(ExternalSendError::Closed(kept))
        );
    }

    #[test]
    fn bounded_external_empty_and_dropped_sender_read_as_no_message() {
        let (tx, rx) = bounded_external_channel(capacity(2), ExternalPolicy::default());
        assert!(rx.try_recv().is_none());
        drop(tx);
        assert!(rx.try_recv().is_none());
    }

    #[test]
    fn bounded_external_producers_share_one_capacity_and_keep_order() {
        let (tx, rx) = bounded_external_channel(capacity(3), ExternalPolicy::default());
        let other = tx.clone();
        assert!(tx.send(0_u32));
        assert!(other.send(1_u32));
        assert!(tx.send(2_u32));
        assert!(matches!(
            other.try_send(External::new(3_u32)),
            Err(ExternalSendError::Full(_))
        ));
        let order: Vec<u32> = std::iter::from_fn(|| rx.try_recv())
            .map(|m| *m.downcast_ref::<u32>().expect("u32"))
            .collect();
        assert_eq!(order, [0, 1, 2]);
    }

    #[test]
    fn bounded_external_policy_is_receiver_local_and_normalized() {
        let (_tx, rx) = bounded_external_channel(capacity(4), policy(0));
        assert_eq!(rx.policy().max_messages, 1);
        let mut rx = rx.with_policy(policy(9));
        assert_eq!(rx.policy(), policy(9));
        rx.set_policy(ExternalPolicy::default());
        assert_eq!(rx.policy(), ExternalPolicy::default());
        assert_eq!(ExternalPolicy::default().max_messages, 64);
        assert_eq!(
            ExternalPolicy::default().max_duration,
            Duration::from_millis(2)
        );
    }
}

#[cfg(test)]
mod render_pending_tests {
    use std::collections::VecDeque;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    struct Scripted {
        requests: VecDeque<bool>,
        latch: bool,
        draws: usize,
    }

    impl Scripted {
        fn new(requests: &[bool]) -> Self {
            Self {
                requests: requests.iter().copied().collect(),
                latch: false,
                draws: 0,
            }
        }
    }

    impl Application for Scripted {
        fn handle(&mut self, _input: Input) -> Outcome {
            Outcome::Ignored
        }

        fn render(&mut self, _frame: &mut Frame) {
            self.draws += 1;
            self.latch = self.requests.pop_front().unwrap_or(false);
        }

        fn should_quit(&self) -> bool {
            false
        }

        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }

        fn take_render_invalidated(&mut self) -> bool {
            std::mem::take(&mut self.latch)
        }
    }

    fn terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(4, 2)).expect("test terminal")
    }

    #[test]
    fn render_pending_survives_all_draw_sites() {
        let wait = Duration::from_millis(100);
        let mut terminal = terminal();

        let mut app = Scripted::new(&[true]);
        let mut dirty = false;
        paint(&mut terminal, &mut app, false, &mut dirty).expect("initial draw");
        assert!(dirty, "a request after the initial draw schedules a turn");
        assert_eq!(turn_wait(wait, dirty), Duration::ZERO);

        let mut app = Scripted::new(&[true, false]);
        let mut dirty = false;
        paint(&mut terminal, &mut app, false, &mut dirty).expect("input draw");
        paint(&mut terminal, &mut app, false, &mut dirty).expect("settle draw");
        assert!(
            dirty,
            "the settle draw does not erase the input draw's request"
        );
        assert!(
            !app.take_render_invalidated(),
            "each draw's request is consumed"
        );

        let mut app = Scripted::new(&[true, false]);
        let mut dirty = false;
        paint(&mut terminal, &mut app, false, &mut dirty).expect("dirty draw");
        assert!(
            dirty,
            "clearing dirty before the draw keeps the new request"
        );
        dirty = false;
        paint(&mut terminal, &mut app, false, &mut dirty).expect("requested draw");
        assert!(!dirty, "a stable draw ends the requests");
        assert_eq!(turn_wait(wait, dirty), wait, "an idle turn keeps its wait");
        assert_eq!(app.draws, 2);
    }
}

#[cfg(test)]
mod draw_lag_tests {
    use super::*;

    #[test]
    fn a_whole_positive_millisecond_count_is_a_lag() {
        assert_eq!(
            parse_draw_lag(Some("300")),
            Some(Duration::from_millis(300))
        );
    }

    #[test]
    fn unset_zero_and_junk_leave_the_draw_alone() {
        for value in [
            None,
            Some("0"),
            Some(""),
            Some("-5"),
            Some("fast"),
            Some("1.5"),
        ] {
            assert_eq!(parse_draw_lag(value), None, "{value:?}");
        }
    }
}
