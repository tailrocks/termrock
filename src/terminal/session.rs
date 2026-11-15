//! A terminal session over a transport: the lifecycle a host needs to show a
//! program that runs on the other side of a link.
//!
//! [`TerminalSessionController`] owns one boxed [`SessionLink`], the screen
//! ([`TerminalViewport`]), the attach status and lease, the replay window that
//! follows an attach, size forwarding, input, paste, focus reports and the
//! bounded queue of side effects the host reads back. The library knows no
//! host language: a failed attach carries the host's own fault type `F`, and
//! the words, the title and notice sanitising and the clipboard permission
//! come from a mandatory [`SessionHostPolicy`].
//!
//! The controller never does real I/O. A host provides the transport through
//! [`SessionSource`] and drives the controller from its own clock: `pump` for
//! the events that arrived, `next_replay_deadline` for when the replay window
//! next needs a pump.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::Arc;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use super::{ForwardPolicy, Paste, TermEvent, TerminalState, TerminalViewport, encode_key};
use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::widgets::viewport_ops::ViewportOps;

const FOCUS_IN: &[u8] = b"\x1b[I";
const FOCUS_OUT: &[u8] = b"\x1b[O";

/// The most side effects a controller keeps for the host. When the queue is
/// full the oldest effect goes first.
pub const PENDING_EFFECTS: usize = 16;

/// What the host's fault type must be: a value the controller stores in the
/// status and in an effect, compares and hands to the host unchanged.
pub trait SessionFault: Clone + Debug + PartialEq + Eq + Send + Sync + 'static {}

impl<T: Clone + Debug + PartialEq + Eq + Send + Sync + 'static> SessionFault for T {}

/// A grid size in rows and columns: what the program is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSize {
    pub rows: u16,
    pub cols: u16,
}

/// Whether this client may type into the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLease {
    Owner,
    ReadOnly,
}

/// What a link did with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSent {
    Queued,
    /// The queue is full; the command was not taken.
    Full,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// Bytes the operator typed or pasted.
    Input(Vec<u8>),
    /// Bytes the terminal answers to a query of the program.
    Reply(Vec<u8>),
    /// Repaint the screen: the link resizes by one row and back.
    Redraw,
    Detach,
}

/// Why a link ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEnd<F> {
    /// The session was detached, by the operator or by the other side.
    Detached,
    /// The stream closed without a word.
    StreamClosed,
    /// The other side refused the attach.
    Refused(F),
    Failed(F),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent<F> {
    /// The attach completed: the replay of the screen follows.
    Attached,
    Lease(SessionLease),
    Output(Vec<u8>),
    /// A line the other side wants the operator to read.
    Notice(String),
    /// The program ended with this exit code.
    Exited(Option<i32>),
    Ended(SessionEnd<F>),
}

/// One attach to a program. A link is owned by one controller; dropping it
/// closes the attach.
pub trait SessionLink<F>: Send + Debug {
    fn send(&self, command: SessionCommand) -> SessionSent;

    /// The grid the program should have now. The latest size wins: a burst of
    /// sizes sends the last one.
    fn set_size(&self, size: SessionSize);

    /// The events that arrived since the last call, in order. `now_ms` is the
    /// host's clock, so a scripted link plays on the same time as the host.
    ///
    /// The bytes of the returned [`SessionEvent::Output`] events together stay
    /// within `output_budget`. A chunk that does not fit is split into ordered
    /// pieces and its rest comes with a later call; a budget of 0 returns no
    /// output byte, and no event behind a withheld output overtakes it.
    ///
    /// The budget bounds output bytes and nothing else. The number of control
    /// events and the time `drain` takes are not bounded, and the controller
    /// processes every returned event in order.
    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<SessionEvent<F>>;
}

/// Opens links lazily, at the first size a session can use. It never blocks:
/// a failure to start is an `Err` with the fault the host shows. The source
/// captures anything the host asked for at construction, such as what to run.
pub trait SessionSource<F>: Send + Sync + Debug {
    fn open(&self, size: SessionSize) -> Result<Box<dyn SessionLink<F>>, F>;

    /// Starts the program again after it exited and opens a link to the new
    /// run. Never blocks, like `open`.
    fn restart(&self, size: SessionSize) -> Result<Box<dyn SessionLink<F>>, F>;

    /// The fault of a stream that closed before the attach completed. The
    /// library has no fault text of its own to invent.
    fn closed_before_attach(&self) -> F;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDetach {
    Operator,
    TransportLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus<F> {
    /// No link yet: the session waits for room, or for a retry.
    Pending,
    /// A link exists and the replay has not finished.
    Attaching,
    Attached(SessionLease),
    Detached(SessionDetach),
    Exited(Option<i32>),
    /// The attach failed.
    Failed(F),
}

/// How long the replay window of an attach stays open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayTiming {
    /// The window closes this long after the attach at the latest.
    pub replay_cap_ms: i64,
    /// The window cannot close before this long after the attach.
    pub nudge_ms: i64,
    /// After the nudge it closes once the output has been quiet this long.
    pub quiet_ms: i64,
}

impl Default for ReplayTiming {
    fn default() -> Self {
        Self {
            replay_cap_ms: 800,
            nudge_ms: 600,
            quiet_ms: 150,
        }
    }
}

/// The numbers a controller works with. The values are used as supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfig {
    /// The smallest usable grid. An area below it waits for room.
    pub minimum: SessionSize,
    /// The rows of history a fresh screen keeps.
    pub scrollback: usize,
    /// The most output bytes one `pump` asks the link for.
    pub feed_budget_bytes: usize,
    /// The most bytes of text one paste may hold.
    pub paste_max_bytes: usize,
    /// A refused clipboard write is reported at most this often.
    pub refusal_status_every_ms: u32,
    pub timing: ReplayTiming,
}

impl SessionConfig {
    pub const MINIMUM: SessionSize = SessionSize { rows: 4, cols: 20 };

    pub fn new(scrollback: usize) -> Self {
        Self {
            minimum: Self::MINIMUM,
            scrollback,
            feed_budget_bytes: 64 * 1024,
            paste_max_bytes: ForwardPolicy::PASTE_MAX_BYTES,
            refusal_status_every_ms: 10_000,
            timing: ReplayTiming::default(),
        }
    }
}

/// What the controller knows when a program asks for a clipboard write.
#[derive(Debug, Clone, Copy)]
pub struct SessionClipboardContext {
    pub owner: bool,
    pub focused: bool,
    pub last_gesture_ms: Option<i64>,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionClipboardRefusal {
    Write,
    Read,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionClipboardDecision {
    /// Copy this text.
    Copy(String),
    /// Drop the request and tell the operator.
    Refuse(SessionClipboardRefusal),
    /// Drop the request without a word.
    Drop,
}

/// What a host decides for a session, synchronously at the event: the text
/// that reaches the operator and the clipboard permission. There is no
/// permissive default, so a host states its policy.
pub trait SessionHostPolicy: Send + Sync + Debug {
    /// The title a program set, sanitised for display. Empty clears it.
    fn title(&self, raw: &str) -> String;

    /// A line the other side sent, sanitised. An empty line is discarded.
    fn notice(&self, raw: &str) -> String;

    /// Decides one OSC 52 write, `<selection>;<data>`.
    fn clipboard(
        &self,
        context: SessionClipboardContext,
        payload: &str,
    ) -> SessionClipboardDecision;
}

/// What a session asks of its host, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEffect<F> {
    Detached(SessionDetach),
    Failed(F),
    Notice(String),
    ClipboardRefused(SessionClipboardRefusal),
    Copy(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKey {
    /// The session swallowed the key: nothing reached the program.
    Consumed,
    /// The key went to the program.
    Forwarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPaste {
    Sent,
    NeedsConfirm {
        lines: usize,
    },
    TooLarge,
    /// The link's queue is full: nothing was sent.
    Full,
    /// The session has no program to take it.
    Dropped,
}

/// The window after an attach in which the replay of the screen arrives. The
/// session answers no query until it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Replay {
    Idle,
    Open { since: i64, quiet_from: i64 },
    Closed,
}

impl Replay {
    fn is_open(self) -> bool {
        matches!(self, Replay::Open { .. })
    }

    fn open(&mut self, now: i64) {
        *self = Replay::Open {
            since: now,
            quiet_from: now,
        };
    }

    fn output(&mut self, now: i64) {
        if let Replay::Open { quiet_from, .. } = self {
            *quiet_from = now;
        }
    }

    fn deadline(self, timing: &ReplayTiming) -> Option<i64> {
        let Replay::Open { since, quiet_from } = self else {
            return None;
        };
        let capped = since.saturating_add(timing.replay_cap_ms);
        let quiet = since
            .saturating_add(timing.nudge_ms)
            .max(quiet_from.saturating_add(timing.quiet_ms));
        Some(capped.min(quiet))
    }

    /// Closes the window when the time has come; returns whether it did.
    fn tick(&mut self, now: i64, timing: &ReplayTiming) -> bool {
        let closes = self.deadline(timing).is_some_and(|at| now >= at);
        if closes {
            *self = Replay::Closed;
        }
        closes
    }
}

/// How a session asks its source for a link: the same program again, or a new
/// run of one that exited.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SessionAttempt {
    #[default]
    Again,
    Restart,
}

/// The reusable lifecycle of a terminal session over a transport.
#[derive(Debug)]
pub struct TerminalSessionController<F> {
    source: Arc<dyn SessionSource<F>>,
    policy: Arc<dyn SessionHostPolicy>,
    config: SessionConfig,
    forward: ForwardPolicy,
    link: Option<Box<dyn SessionLink<F>>>,
    status: SessionStatus<F>,
    lease: Option<SessionLease>,
    view: TerminalViewport,
    area: Rect,
    wanted: Option<SessionSize>,
    forwarded: Option<SessionSize>,
    too_small: bool,
    title: Option<String>,
    focused: bool,
    attention: bool,
    last_gesture_ms: Option<i64>,
    last_refusal_ms: Option<i64>,
    replay: Replay,
    attempt: SessionAttempt,
    attached_once: bool,
    has_output: bool,
    dropped_replies: usize,
    said: VecDeque<SessionEffect<F>>,
}

impl<F: SessionFault> TerminalSessionController<F> {
    pub fn new(
        id: WidgetId,
        source: Arc<dyn SessionSource<F>>,
        config: SessionConfig,
        policy: Arc<dyn SessionHostPolicy>,
    ) -> Self {
        let state = TerminalState::with_scrollback(24, 80, config.scrollback);
        Self {
            source,
            policy,
            forward: ForwardPolicy::new(config.paste_max_bytes),
            config,
            link: None,
            status: SessionStatus::Pending,
            lease: None,
            view: TerminalViewport::new(id, state),
            area: Rect::ZERO,
            wanted: None,
            forwarded: None,
            too_small: false,
            title: None,
            focused: false,
            attention: false,
            last_gesture_ms: None,
            last_refusal_ms: None,
            replay: Replay::Idle,
            attempt: SessionAttempt::Again,
            attached_once: false,
            has_output: false,
            dropped_replies: 0,
            said: VecDeque::new(),
        }
    }

    pub fn status(&self) -> &SessionStatus<F> {
        &self.status
    }

    /// The screen of the program: the viewport that owns its grid.
    pub fn viewport(&self) -> &TerminalViewport {
        &self.view
    }

    /// The screen for scrolling and selection. A grid size set through this
    /// handle is undone by the next `sync_size`, which keeps the grid at the
    /// size forwarded to the program.
    pub fn viewport_mut(&mut self) -> &mut TerminalViewport {
        &mut self.view
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Whether a bell or notification came while the session was not focused.
    pub fn attention(&self) -> bool {
        self.attention
    }

    /// Whether the program has drawn anything on the current screen.
    pub fn has_output(&self) -> bool {
        self.has_output
    }

    /// Replies a full queue made the controller drop.
    pub fn dropped_replies(&self) -> usize {
        self.dropped_replies
    }

    /// Whether a link is there to take what is typed.
    pub fn live(&self) -> bool {
        self.link.is_some()
            && matches!(
                self.status,
                SessionStatus::Attaching | SessionStatus::Attached(_)
            )
    }

    /// Whether the last area was below the configured minimum.
    pub fn too_small(&self) -> bool {
        self.too_small
    }

    /// Whether the session is shown the program's first screen again: from
    /// the attach until the replay window closes.
    fn replaying(&self) -> bool {
        self.replay.is_open() || self.status == SessionStatus::Attaching
    }

    fn owns(&self) -> bool {
        self.lease == Some(SessionLease::Owner)
    }

    fn say(&mut self, effect: SessionEffect<F>) {
        if self.said.len() == PENDING_EFFECTS {
            self.said.pop_front();
        }
        self.said.push_back(effect);
    }

    fn send(&self, command: SessionCommand) -> SessionSent {
        match &self.link {
            Some(link) => link.send(command),
            None => SessionSent::Closed,
        }
    }

    fn send_input(&mut self, bytes: Vec<u8>, now_ms: i64) -> SessionSent {
        if !self.live() {
            return SessionSent::Closed;
        }
        let sent = self.send(SessionCommand::Input(bytes));
        if sent == SessionSent::Queued {
            self.last_gesture_ms = Some(now_ms);
        }
        sent
    }

    fn fresh_grid(&mut self, size: SessionSize) {
        let state = TerminalState::with_scrollback(size.rows, size.cols, self.config.scrollback);
        let mut view = TerminalViewport::new(self.view.id, state);
        view.set_area(self.area);
        self.view = view;
        self.title = None;
        self.has_output = false;
    }

    fn open_link(&mut self, attempt: SessionAttempt) {
        let Some(size) = self.wanted else { return };
        self.link = None;
        self.fresh_grid(size);
        self.attached_once = false;
        self.lease = None;
        self.replay = Replay::Idle;
        self.last_gesture_ms = None;
        let opened = match attempt {
            SessionAttempt::Again => self.source.open(size),
            SessionAttempt::Restart => self.source.restart(size),
        };
        match opened {
            Ok(link) => {
                self.link = Some(link);
                self.forwarded = Some(size);
                self.status = SessionStatus::Attaching;
            }
            Err(fault) => {
                self.forwarded = None;
                self.status = SessionStatus::Failed(fault.clone());
                self.say(SessionEffect::Failed(fault));
            }
        }
    }

    /// Builds a new link and a new screen at the size the session has now. A
    /// session below the floor waits for room and opens at the first size
    /// that fits.
    fn relink(&mut self, attempt: SessionAttempt) {
        if self.too_small {
            self.link = None;
            self.status = SessionStatus::Pending;
            self.attempt = attempt;
        } else {
            self.open_link(attempt);
        }
    }

    /// Takes the events that arrived and applies them in order. Returns
    /// whether anything changed. With no link it does nothing.
    pub fn pump(&mut self, now_ms: i64) -> bool {
        let Some(link) = &self.link else {
            return false;
        };
        let events = link.drain(now_ms, self.config.feed_budget_bytes);
        let mut changed = !events.is_empty();
        for event in events {
            self.handle(event, now_ms);
        }
        if self.link.is_some() && self.replay.tick(now_ms, &self.config.timing) {
            if let Some(lease) = self.lease {
                self.status = SessionStatus::Attached(lease);
            }
            changed = true;
        }
        changed
    }

    /// When the open replay window closes if nothing more arrives, so a host
    /// that is otherwise idle knows when to pump next. `None` outside a
    /// replay window: a live session never asks to be pumped forever.
    pub fn next_replay_deadline(&self) -> Option<i64> {
        self.replay.deadline(&self.config.timing)
    }

    fn handle(&mut self, event: SessionEvent<F>, now_ms: i64) {
        match event {
            SessionEvent::Attached => {
                self.attached_once = true;
                self.lease = None;
                self.replay.open(now_ms);
            }
            SessionEvent::Lease(lease) => {
                if self.lease != Some(lease) {
                    self.last_gesture_ms = None;
                }
                self.lease = Some(lease);
                let settled =
                    self.replay == Replay::Closed && self.status == SessionStatus::Attaching;
                if settled || matches!(self.status, SessionStatus::Attached(_)) {
                    self.status = SessionStatus::Attached(lease);
                }
            }
            SessionEvent::Output(bytes) => {
                self.has_output = true;
                self.replay.output(now_ms);
                let events = self.view.feed(&bytes);
                for event in events {
                    self.term_event(event, now_ms);
                }
            }
            SessionEvent::Notice(text) => {
                let clean = self.policy.notice(&text);
                if !clean.is_empty() {
                    self.say(SessionEffect::Notice(clean));
                }
            }
            SessionEvent::Exited(code) => {
                self.link = None;
                self.replay = Replay::Closed;
                self.status = SessionStatus::Exited(code);
            }
            SessionEvent::Ended(end) => self.ended(end),
        }
    }

    fn ended(&mut self, end: SessionEnd<F>) {
        self.link = None;
        self.replay = Replay::Closed;
        self.status = match end {
            SessionEnd::Detached => SessionStatus::Detached(SessionDetach::Operator),
            SessionEnd::StreamClosed | SessionEnd::Failed(_) if self.attached_once => {
                SessionStatus::Detached(SessionDetach::TransportLost)
            }
            SessionEnd::StreamClosed => SessionStatus::Failed(self.source.closed_before_attach()),
            SessionEnd::Failed(fault) | SessionEnd::Refused(fault) => SessionStatus::Failed(fault),
        };
        match &self.status {
            SessionStatus::Detached(cause) => {
                let cause = *cause;
                self.say(SessionEffect::Detached(cause));
            }
            SessionStatus::Failed(fault) => {
                let fault = fault.clone();
                self.say(SessionEffect::Failed(fault));
            }
            _ => {}
        }
    }

    fn term_event(&mut self, event: TermEvent, now_ms: i64) {
        match event {
            TermEvent::Reply(bytes) => {
                if !self.attached_once || self.replay.is_open() || !self.owns() {
                    return;
                }
                if self.send(SessionCommand::Reply(bytes)) == SessionSent::Full {
                    self.dropped_replies += 1;
                }
            }
            TermEvent::Title(raw) => {
                let title = self.policy.title(&raw);
                self.title = (!title.is_empty()).then_some(title);
            }
            TermEvent::ClipboardWrite(payload) => {
                if !self.replaying() {
                    self.clipboard(&payload, now_ms);
                }
            }
            TermEvent::Bell | TermEvent::Notification(_) => {
                if !self.focused && !self.replaying() {
                    self.attention = true;
                }
            }
            TermEvent::Cwd(_) | TermEvent::ScrollbackCleared => {}
        }
    }

    fn clipboard(&mut self, payload: &str, now_ms: i64) {
        let context = SessionClipboardContext {
            owner: self.owns(),
            focused: self.focused,
            last_gesture_ms: self.last_gesture_ms,
            now_ms,
        };
        match self.policy.clipboard(context, payload) {
            SessionClipboardDecision::Copy(text) => self.say(SessionEffect::Copy(text)),
            SessionClipboardDecision::Drop => {}
            SessionClipboardDecision::Refuse(refusal) => {
                let every = i64::from(self.config.refusal_status_every_ms);
                let due = self
                    .last_refusal_ms
                    .is_none_or(|at| now_ms.saturating_sub(at) >= every);
                if due {
                    self.last_refusal_ms = Some(now_ms);
                    self.say(SessionEffect::ClipboardRefused(refusal));
                }
            }
        }
    }

    /// What the session asked of its host since the last call.
    pub fn take_effects(&mut self) -> Vec<SessionEffect<F>> {
        self.said.drain(..).collect()
    }

    /// Enter on a detached or failed session opens it again, on an exited one
    /// starts the program anew. Any other key goes to the program through the
    /// forward policy.
    pub fn key(&mut self, key: &Key, now_ms: i64) -> SessionKey {
        if key.code == KeyCode::Enter && key.mods.difference(KeyModifiers::SHIFT).is_empty() {
            match self.status {
                SessionStatus::Detached(_) | SessionStatus::Failed(_) => {
                    self.relink(SessionAttempt::Again);
                    return SessionKey::Consumed;
                }
                SessionStatus::Exited(_) => {
                    self.relink(SessionAttempt::Restart);
                    return SessionKey::Consumed;
                }
                _ => {}
            }
        }
        if !self.live() {
            return SessionKey::Consumed;
        }
        self.view.clear_selection();
        self.view.set_follow(true);
        let modes = self.view.state().modes();
        let Some(bytes) = encode_key(key, &modes).and_then(|b| self.forward.key(b)) else {
            return SessionKey::Consumed;
        };
        if self.send_input(bytes, now_ms) == SessionSent::Queued {
            SessionKey::Forwarded
        } else {
            SessionKey::Consumed
        }
    }

    /// The operator clicked in the session.
    pub fn gesture(&mut self, now_ms: i64) {
        if self.live() {
            self.last_gesture_ms = Some(now_ms);
        }
    }

    /// A wheel step the session takes for itself on the alternate screen, or
    /// `None` to let the viewport scroll.
    pub fn wheel(&mut self, axis: Axis, delta: i32) -> Option<Outcome> {
        let modes = self.view.state().modes();
        if !modes.alt_screen {
            return None;
        }
        if axis != Axis::Vertical
            || self.view.state().mouse_reporting()
            || delta == 0
            || !self.owns()
        {
            return Some(Outcome::Consumed);
        }
        let code = if delta < 0 {
            KeyCode::Up
        } else {
            KeyCode::Down
        };
        let arrow = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        let bytes = encode_key(&arrow, &modes)?;
        for _ in 0..delta.unsigned_abs() {
            if self.send(SessionCommand::Input(bytes.clone())) != SessionSent::Queued {
                break;
            }
        }
        Some(Outcome::Changed)
    }

    pub fn paste(&mut self, text: &str, now_ms: i64) -> SessionPaste {
        if !self.live() {
            return SessionPaste::Dropped;
        }
        let bracketed = self.view.state().modes().bracketed_paste;
        let decided = self.forward.paste(text, bracketed);
        self.paste_with(decided, now_ms)
    }

    /// A paste the operator confirmed after `NeedsConfirm`.
    pub fn paste_confirmed(&mut self, text: &str, now_ms: i64) -> SessionPaste {
        if !self.live() {
            return SessionPaste::Dropped;
        }
        let bracketed = self.view.state().modes().bracketed_paste;
        let decided = if bracketed {
            self.forward.paste(text, true)
        } else {
            self.forward.confirmed(text)
        };
        self.paste_with(decided, now_ms)
    }

    fn paste_with(&mut self, decided: Paste, now_ms: i64) -> SessionPaste {
        match decided {
            Paste::Send(bytes) if bytes.is_empty() => SessionPaste::Sent,
            Paste::Send(bytes) => match self.send_input(bytes, now_ms) {
                SessionSent::Queued => SessionPaste::Sent,
                SessionSent::Full => SessionPaste::Full,
                SessionSent::Closed => SessionPaste::Dropped,
            },
            Paste::NeedsConfirm { lines } => SessionPaste::NeedsConfirm { lines },
            Paste::TooLarge { .. } => SessionPaste::TooLarge,
        }
    }

    /// Gives the session the area its screen is drawn in. An area below the
    /// configured minimum waits for room; the first size that fits opens the
    /// link, and a later size is forwarded to it.
    pub fn sync_size(&mut self, inner: Rect) {
        self.area = inner;
        self.view.set_area(inner);
        let (rows, cols) = TerminalViewport::grid_size(inner);
        let fits = cols >= self.config.minimum.cols && rows >= self.config.minimum.rows;
        self.too_small = !fits;
        if !fits {
            return;
        }
        let size = SessionSize { rows, cols };
        self.wanted = Some(size);
        match self.status {
            SessionStatus::Pending => {
                let attempt = std::mem::take(&mut self.attempt);
                self.open_link(attempt);
            }
            SessionStatus::Attaching | SessionStatus::Attached(_) => {
                self.view.resize(rows, cols);
                if self.forwarded != Some(size) {
                    if let Some(link) = &self.link {
                        link.set_size(size);
                    }
                    self.forwarded = Some(size);
                }
            }
            _ => {}
        }
    }

    /// Focus reports go to an owner that asked for them, whether or not the
    /// replay window is open.
    pub fn focus_changed(&mut self, focused: bool) {
        if focused == self.focused {
            return;
        }
        self.focused = focused;
        if focused {
            self.attention = false;
        }
        if self.live() && self.owns() && self.view.state().focus_events() {
            let report = if focused { FOCUS_IN } else { FOCUS_OUT };
            self.send(SessionCommand::Input(report.to_vec()));
        }
    }

    pub fn redraw(&mut self) {
        if self.live() {
            self.send(SessionCommand::Redraw);
        }
    }

    /// Drops the history; no byte goes to the program.
    pub fn clear(&mut self) {
        self.view.clear_scrollback();
    }

    pub fn close_needs_confirm(&self) -> bool {
        matches!(self.status, SessionStatus::Attached(_))
    }

    /// Replaces the numbers the controller works with. The paste limit takes
    /// effect at once; the minimum applies at the next `sync_size`, the
    /// scrollback at the next fresh screen.
    pub fn set_config(&mut self, config: SessionConfig) {
        self.forward = ForwardPolicy::new(config.paste_max_bytes);
        self.config = config;
    }

    pub fn set_host_policy(&mut self, policy: Arc<dyn SessionHostPolicy>) {
        self.policy = policy;
    }
}
