//! The scripted host behind the Terminal pane page's session controller demo.
//!
//! The library owns the lifecycle (`TerminalSessionController`). This module
//! is only fixture data and wiring: a scripted transport that plays timed
//! events on a virtual clock and honours the drain budget, a deny-by-default
//! host policy, and the small presentation the page draws around the screen.
//! No transport I/O happens here.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Widget;
use termrock::core::event::Key;
use termrock::core::hit::Axis;
use termrock::core::id::WidgetId;
use termrock::terminal::{
    SessionClipboardContext, SessionClipboardDecision, SessionClipboardRefusal, SessionCommand,
    SessionConfig, SessionDetach, SessionEffect, SessionEnd, SessionEvent, SessionHostPolicy,
    SessionKey, SessionLease, SessionLink, SessionPaste, SessionSent, SessionSize, SessionSource,
    SessionStatus, TerminalSessionController, TerminalView,
};
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};

use super::terminal_pane::{caret, key_name};

/// The virtual clock moves in steps of this many milliseconds, and the
/// controller is pumped at each one.
pub const STEP_MS: i64 = 100;

const ADVANCE_MS: i64 = 200;
const PLAY_MS: i64 = 1_000;
const KEPT_EFFECTS: usize = 3;
const KEPT_COMMANDS: usize = 3;
const LINK_CLOSED_MS: i64 = 1_500;
const SEVERED_END_MS: i64 = 2_500;

const BANNER: &[u8] = b"\x1b]0;demo session\x07\x1b[2J\x1b[H\x1b[1;32mdemo\x1b[0m$ ls\r\n\x1b[34mdocs\x1b[0m  notes.txt\r\n\x1b[1;32mdemo\x1b[0m$ ";
const RESTARTED: &[u8] = b"\x1b[2J\x1b[H\x1b[2mrestarted\x1b[0m\r\n\x1b[1;32mdemo\x1b[0m$ ";
const QUERY: &[u8] = b"\x1b[3;5H\x1b[6n";
const BELL: &[u8] = b"\x07";
const COPY_ATTEMPT: &[u8] = b"\x1b]52;c;SGVsbG8=\x07";

const SINGLE_LINE: &str = "echo hello";
const THREE_LINES: &str = "one\ntwo\nthree";

/// A failed attach in the host's own words, as the library never names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFault(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Owner,
    Viewer,
    Detaches,
    Exits,
    Fails,
    Refuses,
    Severs,
}

impl Script {
    pub fn from_option(value: &str) -> Self {
        match value {
            "viewer" => Self::Viewer,
            "detaches" => Self::Detaches,
            "exits" => Self::Exits,
            "fails" => Self::Fails,
            "refused" => Self::Refuses,
            "severed" => Self::Severs,
            _ => Self::Owner,
        }
    }
}

pub type CommandLog = Arc<Mutex<Vec<String>>>;

#[derive(Debug, Clone)]
struct Step {
    after_ms: i64,
    event: SessionEvent<DemoFault>,
    sent: usize,
}

fn at(after_ms: i64, event: SessionEvent<DemoFault>) -> Step {
    Step {
        after_ms,
        event,
        sent: 0,
    }
}

fn output(after_ms: i64, bytes: &[u8]) -> Step {
    at(after_ms, SessionEvent::Output(bytes.to_vec()))
}

fn attach(lease: SessionLease, screen: &[u8]) -> Vec<Step> {
    vec![
        at(0, SessionEvent::Attached),
        at(0, SessionEvent::Lease(lease)),
        output(0, screen),
    ]
}

fn plan(script: Script, again: bool) -> Vec<Step> {
    let mut steps = attach(SessionLease::Owner, BANNER);
    match script {
        Script::Owner => steps.extend([
            output(1_200, QUERY),
            output(1_500, BELL),
            output(2_000, COPY_ATTEMPT),
        ]),
        Script::Viewer => {
            steps = vec![
                at(0, SessionEvent::Attached),
                at(0, SessionEvent::Lease(SessionLease::ReadOnly)),
                at(
                    0,
                    SessionEvent::Notice("another client controls this session".into()),
                ),
                output(0, BANNER),
                output(1_200, QUERY),
                output(2_000, COPY_ATTEMPT),
                at(2_500, SessionEvent::Lease(SessionLease::Owner)),
                at(2_500, SessionEvent::Notice("control handed over".into())),
            ];
        }
        Script::Detaches if !again => {
            steps.push(at(1_500, SessionEvent::Ended(SessionEnd::Detached)));
        }
        Script::Exits if !again => steps.push(at(1_500, SessionEvent::Exited(Some(0)))),
        Script::Refuses if !again => {
            steps = vec![at(
                0,
                SessionEvent::Ended(SessionEnd::Refused(DemoFault(
                    "the other side refused the attach",
                ))),
            )];
        }
        Script::Severs if !again => steps.push(at(
            SEVERED_END_MS,
            SessionEvent::Ended(SessionEnd::Failed(DemoFault("the connection dropped"))),
        )),
        Script::Detaches | Script::Exits | Script::Fails | Script::Refuses | Script::Severs => {}
    }
    steps
}

/// When the link of the severed script stops taking commands: before it
/// reports the end, as a dying connection does.
fn closes_after_ms(script: Script, again: bool) -> Option<i64> {
    (script == Script::Severs && !again).then_some(LINK_CLOSED_MS)
}

/// Plays timed events on the host's clock. The first drain starts the clock.
/// It honours the drain contract: a chunk larger than the budget goes out in
/// ordered pieces, a budget of 0 releases no output byte, and nothing
/// overtakes an output that is not yet fully out.
#[derive(Debug)]
struct ScriptedLink {
    plan: Mutex<Plan>,
    commands: CommandLog,
    closes_after_ms: Option<i64>,
    closed: AtomicBool,
}

#[derive(Debug)]
struct Plan {
    started: Option<i64>,
    steps: VecDeque<Step>,
}

impl ScriptedLink {
    fn new(steps: Vec<Step>, commands: &CommandLog) -> Self {
        Self {
            plan: Mutex::new(Plan {
                started: None,
                steps: steps.into(),
            }),
            commands: Arc::clone(commands),
            closes_after_ms: None,
            closed: AtomicBool::new(false),
        }
    }

    fn closing_after(mut self, closes_after_ms: Option<i64>) -> Self {
        self.closes_after_ms = closes_after_ms;
        self
    }

    fn log(&self, line: String) {
        self.commands.lock().expect("commands").push(line);
    }
}

impl SessionLink<DemoFault> for ScriptedLink {
    fn send(&self, command: SessionCommand) -> SessionSent {
        if self.closed.load(Ordering::Relaxed) {
            return SessionSent::Closed;
        }
        self.log(match command {
            SessionCommand::Input(bytes) => format!("input {}", caret(&bytes)),
            SessionCommand::Reply(bytes) => format!("reply {}", caret(&bytes)),
            SessionCommand::Redraw => "redraw".to_owned(),
            SessionCommand::Detach => "detach".to_owned(),
        });
        SessionSent::Queued
    }

    fn set_size(&self, size: SessionSize) {
        self.log(format!("size {}x{}", size.cols, size.rows));
    }

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<SessionEvent<DemoFault>> {
        let mut plan = self.plan.lock().expect("plan");
        let started = *plan.started.get_or_insert(now_ms);
        if self
            .closes_after_ms
            .is_some_and(|after| started.saturating_add(after) <= now_ms)
        {
            self.closed.store(true, Ordering::Relaxed);
        }
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(step) = plan.steps.front_mut() {
            if started.saturating_add(step.after_ms) > now_ms {
                break;
            }
            let SessionEvent::Output(chunk) = &step.event else {
                if let Some(step) = plan.steps.pop_front() {
                    out.push(step.event);
                }
                continue;
            };
            let remaining = chunk.len() - step.sent;
            let take = remaining.min(output_budget.saturating_sub(bytes));
            if remaining > 0 && take == 0 {
                break;
            }
            if take > 0 {
                out.push(SessionEvent::Output(
                    chunk[step.sent..step.sent + take].to_vec(),
                ));
                step.sent += take;
                bytes += take;
            }
            if step.sent == chunk.len() {
                plan.steps.pop_front();
            } else {
                break;
            }
        }
        out
    }
}

#[derive(Debug)]
struct ScriptedSource {
    script: Script,
    opens: AtomicUsize,
    commands: CommandLog,
}

impl ScriptedSource {
    fn link(
        &self,
        steps: Vec<Step>,
        closes_after_ms: Option<i64>,
    ) -> Result<Box<dyn SessionLink<DemoFault>>, DemoFault> {
        Ok(Box::new(
            ScriptedLink::new(steps, &self.commands).closing_after(closes_after_ms),
        ))
    }
}

impl SessionSource<DemoFault> for ScriptedSource {
    fn open(&self, _size: SessionSize) -> Result<Box<dyn SessionLink<DemoFault>>, DemoFault> {
        let again = self.opens.fetch_add(1, Ordering::Relaxed) > 0;
        if self.script == Script::Fails && !again {
            return Err(DemoFault("no capacity, the host is busy"));
        }
        self.link(
            plan(self.script, again),
            closes_after_ms(self.script, again),
        )
    }

    fn restart(&self, size: SessionSize) -> Result<Box<dyn SessionLink<DemoFault>>, DemoFault> {
        if self.script == Script::Exits {
            return self.link(attach(SessionLease::Owner, RESTARTED), None);
        }
        self.open(size)
    }

    fn closed_before_attach(&self) -> DemoFault {
        DemoFault("the stream closed before the attach")
    }
}

/// Deny-by-default: no program writes or reads this host's clipboard, unless
/// the page's "Clipboard policy" option lets a write through. Titles and
/// notices lose their control characters.
#[derive(Debug)]
struct DemoPolicy {
    copies: bool,
}

/// The host policy a session starts with and the page's option swaps.
pub fn host_policy(copies: bool) -> Arc<dyn SessionHostPolicy> {
    Arc::new(DemoPolicy { copies })
}

fn plain(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_owned()
}

impl SessionHostPolicy for DemoPolicy {
    fn title(&self, raw: &str) -> String {
        plain(raw)
    }

    fn notice(&self, raw: &str) -> String {
        plain(raw)
    }

    fn clipboard(
        &self,
        _context: SessionClipboardContext,
        payload: &str,
    ) -> SessionClipboardDecision {
        let data = payload.split_once(';').map_or(payload, |(_, data)| data);
        if data == "?" {
            SessionClipboardDecision::Refuse(SessionClipboardRefusal::Read)
        } else if self.copies {
            SessionClipboardDecision::Copy(data.to_owned())
        } else {
            SessionClipboardDecision::Refuse(SessionClipboardRefusal::Write)
        }
    }
}

fn describe(effect: &SessionEffect<DemoFault>) -> String {
    match effect {
        SessionEffect::Detached(SessionDetach::Operator) => "detached by the operator".to_owned(),
        SessionEffect::Detached(SessionDetach::TransportLost) => "transport lost".to_owned(),
        SessionEffect::Failed(fault) => format!("failed: {}", fault.0),
        SessionEffect::Notice(text) => format!("notice: {text}"),
        SessionEffect::ClipboardRefused(SessionClipboardRefusal::Write) => {
            "clipboard write refused".to_owned()
        }
        SessionEffect::ClipboardRefused(SessionClipboardRefusal::Read) => {
            "clipboard read refused".to_owned()
        }
        SessionEffect::ClipboardRefused(SessionClipboardRefusal::TooLarge) => {
            "clipboard write too large".to_owned()
        }
        SessionEffect::Copy(text) => format!("copy {} characters", text.chars().count()),
    }
}

fn status_line(status: &SessionStatus<DemoFault>) -> (String, Tone) {
    match status {
        SessionStatus::Pending => ("pending".to_owned(), Tone::Warning),
        SessionStatus::Attaching => ("attaching".to_owned(), Tone::Secondary),
        SessionStatus::Attached(SessionLease::Owner) => {
            ("attached · owner".to_owned(), Tone::Success)
        }
        SessionStatus::Attached(SessionLease::ReadOnly) => {
            ("attached · viewer".to_owned(), Tone::Muted)
        }
        SessionStatus::Detached(SessionDetach::Operator) => {
            ("detached · operator".to_owned(), Tone::Warning)
        }
        SessionStatus::Detached(SessionDetach::TransportLost) => {
            ("detached · transport lost".to_owned(), Tone::Warning)
        }
        SessionStatus::Exited(Some(code)) => (format!("exited · code {code}"), Tone::Muted),
        SessionStatus::Exited(None) => ("exited".to_owned(), Tone::Muted),
        SessionStatus::Failed(_) => ("failed".to_owned(), Tone::Error),
    }
}

/// What Enter does in a status: the key that retries, restarts or nothing.
fn enter_does(status: &SessionStatus<DemoFault>) -> &'static str {
    match status {
        SessionStatus::Detached(_) => "reattaches",
        SessionStatus::Failed(_) => "retries the attach",
        SessionStatus::Exited(_) => "restarts the program",
        _ => "goes to the program",
    }
}

/// What the grid shows in place of the program while there is no screen.
fn placeholder(session: &TerminalSessionController<DemoFault>) -> Option<[String; 2]> {
    let min = SessionConfig::MINIMUM;
    match session.status() {
        SessionStatus::Pending if session.too_small() => Some([
            "Waiting for room".to_owned(),
            format!("needs {}x{}", min.cols, min.rows),
        ]),
        SessionStatus::Pending => Some(["Pending".to_owned(), "no link yet".to_owned()]),
        SessionStatus::Attaching => Some([
            "Attaching…".to_owned(),
            "the screen is replaying".to_owned(),
        ]),
        SessionStatus::Failed(fault) if !session.has_output() => {
            ["Attach failed".to_owned(), fault.0.to_owned()].into()
        }
        _ => None,
    }
}

/// The page's scripted session without its presentation: a controller over
/// the script's transport with the deny-by-default policy, and the log of
/// what the controller sent to the link. It has no area yet.
pub fn controller(
    script: Script,
    feed_budget_bytes: usize,
) -> (TerminalSessionController<DemoFault>, CommandLog) {
    let commands = CommandLog::default();
    let source = Arc::new(ScriptedSource {
        script,
        opens: AtomicUsize::new(0),
        commands: Arc::clone(&commands),
    });
    let mut session = TerminalSessionController::new(
        WidgetId::of("terminalpane.session"),
        source,
        SessionConfig::new(200),
        host_policy(false),
    );
    session.set_config(SessionConfig {
        feed_budget_bytes,
        ..SessionConfig::new(200)
    });
    (session, commands)
}

pub struct SessionDemo {
    session: TerminalSessionController<DemoFault>,
    commands: CommandLog,
    rows: u16,
    cols: u16,
    budget: usize,
    now_ms: i64,
    held_paste: Option<String>,
    copies: bool,
    effects: Vec<String>,
    result: Option<String>,
}

impl SessionDemo {
    /// A demo whose program area is `cols` x `rows`, or one below the
    /// session's minimum when `fits` is false.
    pub fn new(script: Script, rows: u16, cols: u16, fits: bool, budget: usize) -> Self {
        let (rows, cols) = if fits {
            (rows, cols)
        } else {
            (
                SessionConfig::MINIMUM.rows - 1,
                SessionConfig::MINIMUM.cols - 4,
            )
        };
        let (mut session, commands) = controller(script, budget);
        session.sync_size(Rect::new(0, 0, cols + 1, rows));
        if session.live() {
            session.pump(0);
        }
        let mut demo = Self {
            session,
            commands,
            rows,
            cols,
            budget,
            now_ms: 0,
            held_paste: None,
            copies: false,
            effects: Vec::new(),
            result: None,
        };
        demo.collect();
        demo
    }

    /// Lets a program's clipboard write through as a `Copy` effect, or
    /// refuses it again, by swapping the session's host policy.
    pub fn clipboard_copies(mut self, copies: bool) -> Self {
        self.copies = copies;
        self.session.set_host_policy(host_policy(copies));
        self
    }

    pub fn held_paste(&self) -> bool {
        self.held_paste.is_some()
    }

    /// The operator clicked in the pane.
    pub fn gesture(&mut self) {
        self.session.gesture(self.now_ms);
    }

    pub fn focus(&mut self, focused: bool) {
        self.session.focus_changed(focused);
        self.collect();
    }

    fn collect(&mut self) {
        let fresh = self.session.take_effects();
        self.effects.extend(fresh.iter().map(describe));
        let excess = self.effects.len().saturating_sub(KEPT_EFFECTS);
        self.effects.drain(..excess);
    }

    fn run(&mut self, ms: i64) {
        for _ in 0..(ms / STEP_MS).max(1) {
            self.now_ms += STEP_MS;
            self.session.pump(self.now_ms);
        }
        self.collect();
    }

    pub fn advance(&mut self) {
        self.run(ADVANCE_MS);
        self.result = Some(format!("clock at {} ms", self.now_ms));
    }

    pub fn play(&mut self) {
        self.run(PLAY_MS);
        self.result = Some(format!("clock at {} ms", self.now_ms));
    }

    pub fn press(&mut self, key: &Key) {
        let before = self.session.status().clone();
        let sent = self.session.key(key, self.now_ms);
        self.collect();
        self.result = Some(format!(
            "{} → {}",
            key_name(key),
            match (sent, &before, self.session.status()) {
                (SessionKey::Forwarded, ..) => "sent to the program".to_owned(),
                (SessionKey::Consumed, before, after) if before != after => {
                    "opened a new link".to_owned()
                }
                (SessionKey::Consumed, ..) => "not sent".to_owned(),
            }
        ));
    }

    pub fn type_key(&mut self) {
        self.press(&Key {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::NONE,
        });
    }

    pub fn can_retry(&self) -> bool {
        matches!(
            self.session.status(),
            SessionStatus::Detached(_) | SessionStatus::Exited(_) | SessionStatus::Failed(_)
        )
    }

    pub fn retry(&mut self) {
        self.press(&Key {
            code: KeyCode::Enter,
            mods: KeyModifiers::NONE,
        });
    }

    fn sent_count(&self) -> usize {
        self.commands.lock().expect("commands").len()
    }

    /// One wheel step up. On the alternate screen it becomes an arrow key
    /// for the program; elsewhere the viewport would scroll.
    pub fn wheel_up(&mut self) {
        let before = self.sent_count();
        let outcome = self.session.wheel(Axis::Vertical, -1);
        let sent = self.sent_count() > before;
        self.collect();
        self.result = Some(match outcome {
            None => "wheel → not on the alternate screen, the viewport would scroll".to_owned(),
            Some(_) if sent => "wheel → up arrow sent to the program".to_owned(),
            Some(_) => "wheel → consumed, nothing sent".to_owned(),
        });
    }

    pub fn redraw(&mut self) {
        let before = self.sent_count();
        self.session.redraw();
        self.collect();
        self.result = Some(if self.sent_count() > before {
            "redraw → sent to the link".to_owned()
        } else {
            "redraw → no link to take it".to_owned()
        });
    }

    pub fn clear_history(&mut self) {
        self.session.clear();
        self.collect();
        self.result = Some("history cleared · no byte sent to the program".to_owned());
    }

    /// Blanks the host's own screen through the viewport handle. The program
    /// keeps running and gets no bytes.
    pub fn hard_reset(&mut self) {
        self.session.viewport_mut().reset();
        self.collect();
        self.result = Some("screen reset here · no byte sent to the program".to_owned());
    }

    fn paste_result(&mut self, text: &str, sent: SessionPaste) {
        self.collect();
        self.result = Some(match sent {
            SessionPaste::Sent => "paste → sent to the program".to_owned(),
            SessionPaste::NeedsConfirm { lines } => {
                self.held_paste = Some(text.to_owned());
                format!("paste of {lines} lines is held until you confirm it")
            }
            SessionPaste::TooLarge => "paste → too large, nothing sent".to_owned(),
            SessionPaste::Full => "paste → the link's queue is full".to_owned(),
            SessionPaste::Dropped => "paste → no program to take it".to_owned(),
        });
    }

    pub fn paste_one_line(&mut self) {
        let sent = self.session.paste(SINGLE_LINE, self.now_ms);
        self.paste_result(SINGLE_LINE, sent);
    }

    pub fn paste_three_lines(&mut self) {
        let sent = self.session.paste(THREE_LINES, self.now_ms);
        self.paste_result(THREE_LINES, sent);
    }

    pub fn confirm_paste(&mut self) {
        if let Some(text) = self.held_paste.take() {
            let sent = self.session.paste_confirmed(&text, self.now_ms);
            self.paste_result(&text, sent);
        }
    }

    fn props(&self) -> Vec<Prop> {
        let (status, tone) = status_line(self.session.status());
        let commands = self.commands.lock().expect("commands");
        let flag = |on: bool| if on { "on" } else { "off" };
        let mut props = vec![
            Prop::new("status", status).tone(tone),
            Prop::new("Enter", enter_does(self.session.status())),
            Prop::new("clock", format!("{} ms", self.now_ms)),
            Prop::new(
                "replay",
                self.session
                    .next_replay_deadline()
                    .map_or_else(|| "not open".to_owned(), |at| format!("closes by {at} ms")),
            ),
            Prop::new("title", self.session.title().unwrap_or("none")),
            Prop::new("attention", flag(self.session.attention())),
            Prop::new(
                "dropped replies",
                self.session.dropped_replies().to_string(),
            ),
            Prop::new(
                "area",
                format!(
                    "{}x{} · minimum {}x{}",
                    self.cols,
                    self.rows,
                    SessionConfig::MINIMUM.cols,
                    SessionConfig::MINIMUM.rows
                ),
            ),
            Prop::new("drain budget", format!("{} bytes", self.budget)),
            Prop::new(
                "clipboard",
                if self.copies {
                    "copies a write"
                } else {
                    "refuses a write"
                },
            ),
            Prop::new("link commands", commands.len().to_string()),
            Prop::new(
                "closing",
                if self.session.close_needs_confirm() {
                    "asks first"
                } else {
                    "no question"
                },
            ),
        ];
        if let SessionStatus::Failed(fault) = self.session.status() {
            props.push(Prop::new("fault", fault.0).wrap().tone(Tone::Error));
        }
        props
    }

    /// Draws the frame, the grid or its placeholder and the status sheet.
    /// Returns the frame, so the page can lay its controls below.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        reserved_below: u16,
        focused: bool,
        colour: bool,
    ) -> Rect {
        let t = ctx.theme;
        let clock = format!("{} ms", self.now_ms);
        let panel = Panel::framed(Some("session"))
            .meta(&clock)
            .focused(ctx.interaction.focused(super::terminal_pane::PANE));
        let outer = panel.outer(Size::new(self.cols + 1, self.rows));
        let frame = Rect::new(
            area.x,
            area.y,
            outer.width.min(area.width),
            outer.height.min(area.height.saturating_sub(reserved_below)),
        );
        let inner = panel.render(frame, buf, t);
        ctx.control(super::terminal_pane::PANE, inner, false);
        ctx.clickable(super::terminal_pane::PANE, frame);
        match placeholder(&self.session) {
            Some(lines) => {
                for (row, line) in lines.iter().enumerate() {
                    let y = inner.y + 1 + row as u16;
                    if y < inner.bottom() {
                        buf.set_string(
                            inner.x + 1,
                            y,
                            termrock::ui::text::truncate(
                                line,
                                inner.width.saturating_sub(1) as usize,
                            ),
                            t.faint().bg(panel.bg(t)),
                        );
                    }
                }
            }
            None => TerminalView::new(self.session.viewport().state())
                .focused(focused)
                .color(colour)
                .render(
                    Rect::new(
                        inner.x,
                        inner.y,
                        inner.width.saturating_sub(1),
                        inner.height,
                    ),
                    buf,
                ),
        }

        let side_x = frame.right() + 2;
        if area.right() > side_x + 24 {
            let side = Rect::new(side_x, area.y, area.right() - side_x, frame.height);
            props::render(side, buf, t, &self.props(), t.canvas);
        }
        frame
    }

    /// The lines under the controls: the last action, the effects the
    /// session handed to the host and the commands it sent to the link.
    pub fn log_lines(&self) -> [(String, Tone); 3] {
        let commands = self.commands.lock().expect("commands");
        let sent: Vec<&str> = commands
            .iter()
            .rev()
            .take(KEPT_COMMANDS)
            .rev()
            .map(String::as_str)
            .collect();
        let result = self
            .result
            .clone()
            .unwrap_or_else(|| "the scripted clock stands at 0 ms".to_owned());
        let effects = if self.effects.is_empty() {
            "effects  none yet".to_owned()
        } else {
            format!("effects  {}", self.effects.join(" · "))
        };
        let sent = if sent.is_empty() {
            "sent  nothing yet".to_owned()
        } else {
            format!("sent  {}", sent.join(" · "))
        };
        [
            (format!("last  {result}"), Tone::Normal),
            (effects, Tone::Muted),
            (sent, Tone::Muted),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(chunks: &[&[u8]]) -> Vec<Step> {
        chunks.iter().map(|c| output(0, c)).collect()
    }

    fn link(steps: Vec<Step>) -> ScriptedLink {
        ScriptedLink::new(steps, &CommandLog::default())
    }

    fn lengths(events: &[SessionEvent<DemoFault>]) -> Vec<usize> {
        events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::Output(bytes) => Some(bytes.len()),
                _ => None,
            })
            .collect()
    }

    fn joined(events: &[SessionEvent<DemoFault>]) -> Vec<u8> {
        events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::Output(bytes) => Some(bytes.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    #[test]
    fn scripted_link_splits_the_first_chunk_to_the_budget() {
        let link = link(steps(&[b"0123456789"]));
        let mut all = Vec::new();
        let mut sizes = Vec::new();
        for _ in 0..4 {
            let events = link.drain(0, 3);
            sizes.extend(lengths(&events));
            all.extend(joined(&events));
        }
        assert_eq!(sizes, [3, 3, 3, 1]);
        assert_eq!(all, b"0123456789", "the pieces concatenate to the chunk");
    }

    #[test]
    fn scripted_link_zero_budget_holds_output_and_keeps_fifo() {
        let link = link(vec![
            at(0, SessionEvent::Attached),
            output(0, b"abc"),
            at(0, SessionEvent::Notice("after".into())),
        ]);
        assert_eq!(link.drain(0, 0), [SessionEvent::Attached]);
        assert_eq!(link.drain(1, 0), [], "nothing overtakes a held output");
        let events = link.drain(2, 2);
        assert_eq!(joined(&events), b"ab");
        assert_eq!(events.len(), 1, "the notice waits behind the remainder");
        let events = link.drain(3, 8);
        assert_eq!(joined(&events), b"c");
        assert_eq!(events.last(), Some(&SessionEvent::Notice("after".into())));
    }

    #[test]
    fn scripted_link_budget_spans_chunks_and_empty_chunks_progress() {
        let link = link(steps(&[b"ab", b"", b"cde", b"f"]));
        let events = link.drain(0, 4);
        assert_eq!(
            lengths(&events),
            [2, 2],
            "the third chunk is cut at the budget"
        );
        let events = link.drain(1, 4);
        assert_eq!(lengths(&events), [1, 1]);
        assert_eq!(joined(&events), b"ef");
    }

    #[test]
    fn scripted_link_waits_for_its_clock() {
        let link = link(vec![at(0, SessionEvent::Attached), output(500, b"later")]);
        assert_eq!(
            link.drain(100, 64).len(),
            1,
            "the clock starts at the first drain"
        );
        assert_eq!(link.drain(599, 64), []);
        assert_eq!(joined(&link.drain(600, 64)), b"later");
        let late = ScriptedLink::new(vec![output(i64::MAX, b"never")], &CommandLog::default());
        assert_eq!(late.drain(i64::MAX - 1, 64), [], "a huge delay saturates");
    }

    fn row(demo: &SessionDemo, row: u16) -> String {
        let view = demo.session.viewport().state().view(0);
        (0..view.cols())
            .filter_map(|col| view.cell(row, col))
            .filter(|cell| !cell.is_wide_continuation())
            .map(|cell| match cell.text() {
                "" => " ".to_owned(),
                text => text.to_owned(),
            })
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    fn sent(demo: &SessionDemo) -> Vec<String> {
        demo.commands.lock().expect("commands").clone()
    }

    fn status(demo: &SessionDemo) -> SessionStatus<DemoFault> {
        demo.session.status().clone()
    }

    fn demo(script: Script) -> SessionDemo {
        SessionDemo::new(script, 12, 40, true, 64 * 1024)
    }

    fn session_ids() -> Vec<&'static str> {
        termrock::catalog::CAPABILITIES
            .iter()
            .map(|cap| cap.id)
            .filter(|id| {
                let name = id.trim_start_matches("terminal::");
                id.starts_with("terminal::")
                    && [
                        "Session",
                        "TerminalSessionController",
                        "ReplayTiming",
                        "PENDING_EFFECTS",
                    ]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
            })
            .collect()
    }

    #[test]
    fn showcase_terminal_session_capabilities_are_registered() {
        use crate::coverage::TERMINAL_SESSION_COVERAGE;
        use termrock::catalog::ALLOWLIST;

        let ids = session_ids();
        assert!(
            ids.len() > 90,
            "the session API is in the catalog: {}",
            ids.len()
        );
        for id in &ids {
            let shown = TERMINAL_SESSION_COVERAGE.contains(id);
            let allowed = ALLOWLIST.iter().find(|a| a.id == *id);
            assert!(
                shown != allowed.is_some(),
                "{id} is either shown by the Terminal pane page or allowlisted with a reason, not both and not neither"
            );
            if let Some(allowed) = allowed {
                assert!(!allowed.reason.trim().is_empty(), "{id} has no reason");
            }
        }
        for id in TERMINAL_SESSION_COVERAGE {
            assert!(
                ids.contains(id),
                "declared id is not a catalog capability: {id}"
            );
        }
    }

    #[test]
    fn owner_script_attaches_answers_and_denies_the_clipboard() {
        let mut demo = demo(Script::Owner);
        assert_eq!(status(&demo), SessionStatus::Attaching);
        assert_eq!(demo.session.next_replay_deadline(), Some(600));
        assert!(
            placeholder(&demo.session).is_some(),
            "the replay shows a placeholder"
        );
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
        assert_eq!(row(&demo, 0), "demo$ ls");
        assert_eq!(demo.session.title(), Some("demo session"));
        assert!(demo.session.next_replay_deadline().is_none());

        demo.play();
        assert!(
            sent(&demo).contains(&"reply ESC [ 3 ; 5 R".to_owned()),
            "the owner answers the query: {:?}",
            sent(&demo)
        );
        assert!(demo.session.attention(), "the bell of an unfocused session");
        assert_eq!(demo.effects, ["clipboard write refused"], "deny by default");
        demo.focus(true);
        assert!(!demo.session.attention(), "focus clears the attention");
    }

    #[test]
    fn a_small_drain_budget_stretches_the_replay_to_its_cap() {
        let mut generous = demo(Script::Owner);
        generous.run(600);
        assert_eq!(
            status(&generous),
            SessionStatus::Attached(SessionLease::Owner)
        );

        let mut small = SessionDemo::new(Script::Owner, 12, 40, true, 8);
        small.run(700);
        assert_eq!(
            status(&small),
            SessionStatus::Attaching,
            "output keeps arriving, so the quiet timer keeps restarting"
        );
        assert_eq!(small.session.next_replay_deadline(), Some(800));
        small.run(100);
        assert_eq!(status(&small), SessionStatus::Attached(SessionLease::Owner));
        small.run(300);
        assert_eq!(row(&small, 0), "demo$ ls", "no byte was lost to the budget");
    }

    #[test]
    fn viewer_script_reads_then_receives_the_lease() {
        let mut demo = demo(Script::Viewer);
        demo.play();
        assert_eq!(
            status(&demo),
            SessionStatus::Attached(SessionLease::ReadOnly)
        );
        assert_eq!(
            demo.effects,
            ["notice: another client controls this session"]
        );
        demo.play();
        assert!(
            !sent(&demo).iter().any(|c| c.starts_with("reply")),
            "a viewer answers no query: {:?}",
            sent(&demo)
        );
        demo.type_key();
        assert_eq!(sent(&demo).last().map(String::as_str), Some("input a"));
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
        assert!(
            demo.effects
                .contains(&"notice: control handed over".to_owned())
        );
    }

    #[test]
    fn detached_script_reattaches_with_enter() {
        let mut demo = demo(Script::Detaches);
        demo.play();
        demo.play();
        assert_eq!(
            status(&demo),
            SessionStatus::Detached(SessionDetach::Operator)
        );
        assert_eq!(row(&demo, 0), "demo$ ls", "the last screen stays");
        demo.retry();
        assert_eq!(status(&demo), SessionStatus::Attaching);
        assert_eq!(row(&demo, 0), "", "a fresh grid");
        demo.play();
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
    }

    #[test]
    fn exited_script_restarts_with_enter() {
        let mut demo = demo(Script::Exits);
        demo.play();
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Exited(Some(0)));
        demo.retry();
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
        assert_eq!(
            row(&demo, 0),
            "restarted",
            "the restart plays its own screen"
        );
    }

    #[test]
    fn failed_script_retries_with_enter() {
        let mut demo = demo(Script::Fails);
        assert_eq!(
            status(&demo),
            SessionStatus::Failed(DemoFault("no capacity, the host is busy"))
        );
        assert_eq!(demo.effects, ["failed: no capacity, the host is busy"]);
        assert!(placeholder(&demo.session).is_some());
        demo.retry();
        demo.play();
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
    }

    #[test]
    fn an_area_below_the_minimum_waits_for_room() {
        let mut demo = SessionDemo::new(Script::Owner, 12, 40, false, 64 * 1024);
        assert_eq!(status(&demo), SessionStatus::Pending);
        assert!(demo.session.too_small());
        demo.play();
        assert_eq!(
            status(&demo),
            SessionStatus::Pending,
            "time does not give it room"
        );
        assert!(sent(&demo).is_empty(), "no link was opened");
        demo.paste_one_line();
        assert_eq!(
            demo.result.as_deref(),
            Some("paste → no program to take it")
        );
    }

    #[test]
    fn a_three_line_paste_waits_for_its_confirmation() {
        let mut demo = demo(Script::Owner);
        demo.play();
        demo.paste_one_line();
        assert_eq!(
            sent(&demo).last().map(String::as_str),
            Some("input e c h o   h e l l o")
        );
        assert!(!demo.held_paste());
        let before = sent(&demo).len();
        demo.paste_three_lines();
        assert!(demo.held_paste(), "the program asked for no brackets");
        assert_eq!(
            sent(&demo).len(),
            before,
            "nothing is sent before the confirm"
        );
        demo.confirm_paste();
        assert!(!demo.held_paste());
        assert_eq!(demo.result.as_deref(), Some("paste → sent to the program"));
        assert_eq!(sent(&demo).len(), before + 1);
    }

    #[test]
    fn wheel_up_is_an_arrow_key_only_on_the_alternate_screen() {
        let mut demo = demo(Script::Owner);
        demo.play();
        let before = sent(&demo);
        demo.wheel_up();
        assert_eq!(
            demo.result.as_deref(),
            Some("wheel → not on the alternate screen, the viewport would scroll")
        );
        assert_eq!(sent(&demo), before, "nothing is sent to the program");

        demo.session.viewport_mut().feed(b"\x1b[?1049h");
        demo.wheel_up();
        assert_eq!(
            sent(&demo).last().map(String::as_str),
            Some("input ESC [ A")
        );
        assert_eq!(
            demo.result.as_deref(),
            Some("wheel → up arrow sent to the program")
        );
    }

    #[test]
    fn redraw_is_sent_to_a_live_link_and_not_without_one() {
        let mut failed = demo(Script::Fails);
        failed.redraw();
        assert_eq!(
            failed.result.as_deref(),
            Some("redraw → no link to take it")
        );
        assert!(sent(&failed).is_empty(), "a failed session has no link");

        let mut owner = demo(Script::Owner);
        owner.redraw();
        assert_eq!(sent(&owner).last().map(String::as_str), Some("redraw"));
        assert_eq!(owner.result.as_deref(), Some("redraw → sent to the link"));
    }

    #[test]
    fn clear_history_drops_the_scrollback_and_sends_nothing() {
        let mut demo = demo(Script::Owner);
        demo.play();
        let lines: Vec<u8> = (0..40)
            .flat_map(|i| format!("line {i}\r\n").into_bytes())
            .collect();
        demo.session.viewport_mut().feed(&lines);
        assert!(demo.session.viewport().state().scrollback_len() > 0);
        let before = sent(&demo);
        demo.clear_history();
        assert_eq!(demo.session.viewport().state().scrollback_len(), 0);
        assert_eq!(sent(&demo), before, "clear sends nothing to the program");
        assert_eq!(
            demo.result.as_deref(),
            Some("history cleared · no byte sent to the program")
        );
    }

    #[test]
    fn hard_reset_blanks_the_screen_and_keeps_the_session() {
        let mut demo = demo(Script::Owner);
        demo.play();
        assert_eq!(row(&demo, 0), "demo$ ls");
        let before = sent(&demo);
        demo.hard_reset();
        assert_eq!(row(&demo, 0), "", "the host's screen is blank");
        assert_eq!(status(&demo), SessionStatus::Attached(SessionLease::Owner));
        assert_eq!(sent(&demo), before, "no byte goes to the program");
    }
}
