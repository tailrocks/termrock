//! The terminal session controller against a scripted transport. Every case
//! drives the public controller the way a host does: size, pump, keys,
//! pastes, focus and the effects it takes back.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::terminal::{
    PENDING_EFFECTS, ReplayTiming, SessionClipboardContext, SessionClipboardDecision,
    SessionClipboardRefusal, SessionCommand, SessionConfig, SessionDetach, SessionEffect,
    SessionEnd, SessionEvent, SessionFault, SessionHostPolicy, SessionKey, SessionLease,
    SessionLink, SessionPaste, SessionSent, SessionSize, SessionSource, SessionStatus,
    TerminalSessionController,
};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fault(&'static str);

#[derive(Debug, Default)]
struct Log {
    commands: Vec<SessionCommand>,
    sizes: Vec<SessionSize>,
    inbox: VecDeque<SessionEvent<Fault>>,
    drains: Vec<(i64, usize)>,
    fed: Vec<usize>,
    full: bool,
}

type Shared = Arc<Mutex<Log>>;

#[derive(Debug)]
struct TestLink(Shared);

impl SessionLink<Fault> for TestLink {
    fn send(&self, command: SessionCommand) -> SessionSent {
        let mut log = self.0.lock().expect("log");
        if log.full {
            return SessionSent::Full;
        }
        log.commands.push(command);
        SessionSent::Queued
    }

    fn set_size(&self, size: SessionSize) {
        self.0.lock().expect("log").sizes.push(size);
    }

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<SessionEvent<Fault>> {
        let mut log = self.0.lock().expect("log");
        log.drains.push((now_ms, output_budget));
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(event) = log.inbox.pop_front() {
            let SessionEvent::Output(chunk) = event else {
                out.push(event);
                continue;
            };
            let room = output_budget - bytes;
            if chunk.len() <= room {
                bytes += chunk.len();
                out.push(SessionEvent::Output(chunk));
                continue;
            }
            if room > 0 {
                bytes += room;
                out.push(SessionEvent::Output(chunk[..room].to_vec()));
            }
            log.inbox
                .push_front(SessionEvent::Output(chunk[room..].to_vec()));
            break;
        }
        log.fed.push(bytes);
        out
    }
}

#[derive(Debug, Default)]
struct Bench {
    links: Mutex<Vec<Shared>>,
    refusals: Mutex<VecDeque<Fault>>,
    restarted: Mutex<Vec<usize>>,
}

impl Bench {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn link(&self, size: SessionSize) -> Result<Box<dyn SessionLink<Fault>>, Fault> {
        if let Some(fault) = self.refusals.lock().expect("refusals").pop_front() {
            return Err(fault);
        }
        let log = Shared::default();
        log.lock().expect("log").sizes.push(size);
        self.links.lock().expect("links").push(log.clone());
        Ok(Box::new(TestLink(log)))
    }

    fn opened(&self) -> usize {
        self.links.lock().expect("links").len()
    }

    fn log(&self, n: usize) -> Shared {
        self.links.lock().expect("links")[n].clone()
    }

    fn push(&self, n: usize, event: SessionEvent<Fault>) {
        self.log(n).lock().expect("log").inbox.push_back(event);
    }

    fn output(&self, n: usize, bytes: &[u8]) {
        self.push(n, SessionEvent::Output(bytes.to_vec()));
    }

    fn commands(&self, n: usize) -> Vec<SessionCommand> {
        self.log(n).lock().expect("log").commands.clone()
    }

    fn sizes(&self, n: usize) -> Vec<SessionSize> {
        self.log(n).lock().expect("log").sizes.clone()
    }

    fn fill(&self, n: usize, full: bool) {
        self.log(n).lock().expect("log").full = full;
    }

    fn drains(&self, n: usize) -> Vec<(i64, usize)> {
        self.log(n).lock().expect("log").drains.clone()
    }

    fn fed(&self, n: usize) -> Vec<usize> {
        self.log(n).lock().expect("log").fed.clone()
    }

    fn queued(&self, n: usize) -> usize {
        self.log(n).lock().expect("log").inbox.len()
    }

    fn restarted(&self) -> Vec<usize> {
        self.restarted.lock().expect("restarted").clone()
    }
}

impl SessionSource<Fault> for Bench {
    fn open(&self, size: SessionSize) -> Result<Box<dyn SessionLink<Fault>>, Fault> {
        self.link(size)
    }

    fn restart(&self, size: SessionSize) -> Result<Box<dyn SessionLink<Fault>>, Fault> {
        let link = self.link(size);
        if link.is_ok() {
            self.restarted
                .lock()
                .expect("restarted")
                .push(self.opened() - 1);
        }
        link
    }

    fn closed_before_attach(&self) -> Fault {
        Fault("closed before attach")
    }
}

const GESTURE_WINDOW_MS: i64 = 5_000;

#[derive(Debug)]
struct Policy;

impl SessionHostPolicy for Policy {
    fn title(&self, raw: &str) -> String {
        raw.chars().filter(|c| !c.is_control()).collect()
    }

    fn notice(&self, raw: &str) -> String {
        raw.chars().filter(|c| !c.is_control()).collect()
    }

    fn clipboard(
        &self,
        context: SessionClipboardContext,
        payload: &str,
    ) -> SessionClipboardDecision {
        let data = payload.split_once(';').map_or(payload, |(_, data)| data);
        let in_window = context
            .last_gesture_ms
            .is_some_and(|at| context.now_ms - at <= GESTURE_WINDOW_MS);
        match data {
            "?" => SessionClipboardDecision::Refuse(SessionClipboardRefusal::Read),
            "drop" => SessionClipboardDecision::Drop,
            "big" => SessionClipboardDecision::Refuse(SessionClipboardRefusal::TooLarge),
            _ => match data.strip_prefix("copy:") {
                Some(text) if context.owner && context.focused && in_window => {
                    SessionClipboardDecision::Copy(text.to_owned())
                }
                _ => SessionClipboardDecision::Refuse(SessionClipboardRefusal::Write),
            },
        }
    }
}

fn controller(bench: &Arc<Bench>) -> TerminalSessionController<Fault> {
    TerminalSessionController::new(
        WidgetId::of("test.session"),
        bench.clone(),
        SessionConfig::new(200),
        Arc::new(Policy),
    )
}

/// The area that gives the program `cols` columns and `rows` rows: one more
/// column goes to the scroll track.
fn area(cols: u16, rows: u16) -> Rect {
    Rect::new(0, 0, cols + 1, rows)
}

fn size(rows: u16, cols: u16) -> SessionSize {
    SessionSize { rows, cols }
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn input(bytes: &[u8]) -> SessionCommand {
    SessionCommand::Input(bytes.to_vec())
}

fn attached(bench: &Arc<Bench>, lease: SessionLease) -> TerminalSessionController<Fault> {
    let mut session = controller(bench);
    session.sync_size(area(80, 24));
    bench.push(0, SessionEvent::Attached);
    bench.push(0, SessionEvent::Lease(lease));
    session.pump(0);
    session
}

fn live(bench: &Arc<Bench>, lease: SessionLease) -> TerminalSessionController<Fault> {
    let mut session = attached(bench, lease);
    session.pump(800);
    session
}

fn row(session: &TerminalSessionController<Fault>, row: u16) -> String {
    let view = session.viewport().state().view(0);
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

#[test]
fn session_controller_replay_suppresses_side_effects() {
    let bench = Bench::new();
    let mut session = attached(&bench, SessionLease::Owner);
    assert_eq!(session.status(), &SessionStatus::Attaching);
    assert_eq!(session.next_replay_deadline(), Some(600));

    bench.output(0, b"\x1b[c\x07\x1b]52;c;copy:replayed\x07");
    assert!(session.pump(100), "output is a change");
    assert_eq!(bench.commands(0), [], "the replayed query is not answered");
    assert_eq!(session.take_effects(), [], "no clipboard effect in replay");
    assert!(
        !session.attention(),
        "a replayed bell asks for no attention"
    );

    assert!(!session.pump(599), "nothing arrived, nothing changed");
    assert_eq!(session.status(), &SessionStatus::Attaching);
    assert!(session.pump(600), "the window closes at the nudge");
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );
    assert_eq!(session.next_replay_deadline(), None);

    bench.output(0, b"\x1b[3;5H\x1b[6n\x07\x1b]52;c;copy:late\x07");
    session.pump(700);
    assert_eq!(
        bench.commands(0),
        [SessionCommand::Reply(b"\x1b[3;5R".to_vec())],
        "a query after the window gets one reply"
    );
    assert!(session.attention(), "a bell after the window is attention");
    assert_eq!(
        session.take_effects(),
        [SessionEffect::ClipboardRefused(
            SessionClipboardRefusal::Write
        )],
        "the host policy refuses an unfocused write"
    );
}

#[test]
fn session_controller_replay_deadline_follows_output() {
    let bench = Bench::new();
    let mut session = attached(&bench, SessionLease::Owner);
    assert_eq!(session.next_replay_deadline(), Some(600));

    bench.output(0, b"a");
    session.pump(550);
    assert_eq!(
        session.next_replay_deadline(),
        Some(700),
        "output near the nudge holds the window until the stream is quiet"
    );
    session.pump(699);
    assert_eq!(session.status(), &SessionStatus::Attaching);
    session.pump(700);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );

    let bench = Bench::new();
    let mut session = attached(&bench, SessionLease::Owner);
    for (now, deadline) in [(100, 600), (450, 600), (500, 650), (600, 750), (700, 800)] {
        bench.output(0, b"x");
        session.pump(now);
        assert_eq!(
            session.next_replay_deadline(),
            Some(deadline),
            "output at {now} ms: quiet 150 ms later, never past the 800 ms cap"
        );
    }
    session.pump(799);
    assert_eq!(session.status(), &SessionStatus::Attaching);
    session.pump(800);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );
}

#[test]
fn session_controller_focus_reports_ignore_the_replay_window() {
    let bench = Bench::new();
    let mut session = attached(&bench, SessionLease::Owner);
    bench.output(0, b"\x1b[?1004h");
    session.pump(10);
    assert_eq!(session.status(), &SessionStatus::Attaching);

    session.focus_changed(true);
    assert_eq!(
        bench.commands(0),
        [input(b"\x1b[I")],
        "the report goes out while the replay is open"
    );
    session.focus_changed(true);
    session.focus_changed(false);
    assert_eq!(
        bench.commands(0),
        [input(b"\x1b[I"), input(b"\x1b[O")],
        "a repeated state sends nothing"
    );
}

#[test]
fn session_controller_readonly_input_and_owner_automation() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::ReadOnly);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::ReadOnly)
    );

    assert_eq!(
        session.key(&key(KeyCode::Char('a')), 1_000),
        SessionKey::Forwarded
    );
    assert_eq!(session.paste("hi", 1_001), SessionPaste::Sent);
    assert_eq!(
        bench.commands(0),
        [input(b"a"), input(b"hi")],
        "a viewer's typing is sent, so the other side can give it the lease"
    );

    bench.output(0, b"\x1b[?1004h\x1b[6n\x1b[?1049h");
    session.pump(1_002);
    session.focus_changed(true);
    assert_eq!(
        session.wheel(Axis::Vertical, -1),
        Some(Outcome::Consumed),
        "a viewer's wheel on the alternate screen sends nothing"
    );
    assert_eq!(
        bench.commands(0).len(),
        2,
        "no reply, focus report or arrow"
    );

    bench.push(0, SessionEvent::Lease(SessionLease::Owner));
    session.pump(1_100);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );
    bench.output(0, b"\x1b]52;c;copy:one\x07");
    session.pump(1_200);
    assert_eq!(
        session.take_effects(),
        [SessionEffect::ClipboardRefused(
            SessionClipboardRefusal::Write
        )],
        "the lease flip cleared the gesture, so the write is refused"
    );

    session.gesture(12_000);
    bench.output(0, b"\x1b]52;c;copy:two\x07");
    session.pump(12_100);
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Copy("two".to_owned())],
        "an owner's write after a gesture goes through"
    );

    assert_eq!(session.wheel(Axis::Vertical, 2), Some(Outcome::Changed));
    session.focus_changed(false);
    let commands = bench.commands(0);
    assert_eq!(
        &commands[2..],
        [input(b"\x1b[B"), input(b"\x1b[B"), input(b"\x1b[O"),],
        "an owner's arrows and focus report go out"
    );
}

#[test]
fn session_controller_resize_retry_restart_transcript() {
    let bench = Bench::new();
    let mut session = controller(&bench);
    assert_eq!(session.status(), &SessionStatus::Pending);
    assert_eq!(bench.opened(), 0, "a new controller holds no link");

    session.sync_size(area(19, 24));
    assert!(session.too_small(), "19 columns are below the floor");
    session.sync_size(area(80, 3));
    assert!(session.too_small(), "3 rows are below the floor");
    assert_eq!(
        (bench.opened(), session.status()),
        (0, &SessionStatus::Pending)
    );

    session.sync_size(area(80, 24));
    assert!(!session.too_small());
    assert_eq!(bench.opened(), 1);
    assert_eq!(bench.sizes(0), [size(24, 80)]);
    session.sync_size(area(80, 24));
    assert_eq!(bench.opened(), 1, "the same size opens nothing");

    session.sync_size(area(100, 30));
    session.sync_size(area(90, 28));
    assert_eq!(
        bench.sizes(0),
        [size(24, 80), size(30, 100), size(28, 90)],
        "each new size is forwarded, the latest last"
    );
    assert_eq!(session.viewport().state().size(), (28, 90));

    bench.push(0, SessionEvent::Attached);
    bench.push(0, SessionEvent::Lease(SessionLease::Owner));
    bench.output(0, b"\x1b]0;shell\x07\x1b]0;\x07\x1b]0;ti\x01tle\x07drawn");
    session.pump(0);
    session.pump(800);
    assert_eq!(
        session.title(),
        Some("title"),
        "the policy sanitised the title"
    );
    assert!(session.has_output());
    assert_eq!(row(&session, 0), "drawn");

    bench.push(0, SessionEvent::Ended(SessionEnd::StreamClosed));
    session.pump(900);
    assert_eq!(
        session.status(),
        &SessionStatus::Detached(SessionDetach::TransportLost),
        "a stream lost after the attach is a detach"
    );
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Detached(SessionDetach::TransportLost)]
    );
    assert!(!session.live());
    assert_eq!(
        row(&session, 0),
        "drawn",
        "the old grid stays after a detach"
    );

    assert_eq!(
        session.key(&key(KeyCode::Enter), 1_000),
        SessionKey::Consumed
    );
    assert_eq!(bench.opened(), 2, "Enter opens again");
    assert!(bench.restarted().is_empty());
    assert_eq!(session.status(), &SessionStatus::Attaching);
    assert_eq!(session.title(), None, "the title belongs to the old run");
    assert!(!session.has_output());
    assert_eq!(row(&session, 0), "", "a fresh grid");
    assert_eq!(bench.sizes(1), [size(28, 90)], "at the size it has now");

    bench.push(1, SessionEvent::Ended(SessionEnd::StreamClosed));
    session.pump(1_100);
    assert_eq!(
        session.status(),
        &SessionStatus::Failed(Fault("closed before attach")),
        "the source names the fault of a stream closed before the attach"
    );
    bench.fill(1, false);
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Failed(Fault("closed before attach"))]
    );

    bench
        .refusals
        .lock()
        .expect("refusals")
        .push_back(Fault("no room"));
    session.key(&key(KeyCode::Enter), 1_200);
    assert_eq!(session.status(), &SessionStatus::Failed(Fault("no room")));
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Failed(Fault("no room"))]
    );

    session.key(&key(KeyCode::Enter), 1_300);
    assert_eq!(bench.opened(), 3);
    bench.push(2, SessionEvent::Attached);
    bench.push(2, SessionEvent::Lease(SessionLease::Owner));
    bench.push(2, SessionEvent::Exited(Some(0)));
    session.pump(1_300);
    assert_eq!(session.status(), &SessionStatus::Exited(Some(0)));
    assert!(!session.live());

    session.sync_size(area(10, 24));
    session.key(&key(KeyCode::Enter), 1_400);
    assert_eq!(
        session.status(),
        &SessionStatus::Pending,
        "a restart below the floor waits for room"
    );
    assert_eq!(bench.opened(), 3);
    session.sync_size(area(80, 24));
    assert_eq!(bench.opened(), 4);
    assert_eq!(bench.restarted(), [3], "the first size that fits restarts");
    assert_eq!(session.status(), &SessionStatus::Attaching);
}

#[test]
fn session_controller_exit_is_not_overridden_by_a_stream_close_in_the_same_drain() {
    // A program exits and its stream closes behind it. Whether the two events
    // come in one drain or in two must not change what the operator sees or
    // what Enter does.
    for same_drain in [false, true] {
        let bench = Bench::new();
        let mut session = live(&bench, SessionLease::Owner);
        bench.push(0, SessionEvent::Exited(Some(3)));
        if same_drain {
            bench.push(0, SessionEvent::Ended(SessionEnd::StreamClosed));
        }
        session.pump(900);
        if !same_drain {
            bench.push(0, SessionEvent::Ended(SessionEnd::StreamClosed));
            session.pump(1_000);
        }
        assert_eq!(
            session.status(),
            &SessionStatus::Exited(Some(3)),
            "same_drain={same_drain}"
        );
        assert_eq!(session.take_effects(), [], "same_drain={same_drain}");
        session.key(&key(KeyCode::Enter), 1_100);
        assert_eq!(
            bench.restarted(),
            [1],
            "same_drain={same_drain}: Enter restarts the program"
        );
    }
}

#[test]
fn session_controller_failed_attach_can_be_refused_by_the_other_side() {
    let bench = Bench::new();
    let mut session = controller(&bench);
    session.sync_size(area(80, 24));
    bench.push(0, SessionEvent::Ended(SessionEnd::Refused(Fault("busy"))));
    session.pump(0);
    assert_eq!(session.status(), &SessionStatus::Failed(Fault("busy")));

    session.key(&key(KeyCode::Enter), 10);
    bench.push(1, SessionEvent::Attached);
    bench.push(1, SessionEvent::Ended(SessionEnd::Failed(Fault("reset"))));
    session.pump(10);
    assert_eq!(
        session.status(),
        &SessionStatus::Detached(SessionDetach::TransportLost),
        "a failure after the attach is a lost transport, not a failed attach"
    );
    session.key(&key(KeyCode::Enter), 20);
    bench.push(2, SessionEvent::Ended(SessionEnd::Detached));
    session.pump(20);
    assert_eq!(
        session.status(),
        &SessionStatus::Detached(SessionDetach::Operator)
    );
}

fn ask(
    bench: &Arc<Bench>,
    session: &mut TerminalSessionController<Fault>,
    payload: &str,
    now: i64,
) -> Vec<SessionEffect<Fault>> {
    bench.output(0, format!("\x1b]52;c;{payload}\x07").as_bytes());
    session.pump(now);
    session.take_effects()
}

fn refused(refusal: SessionClipboardRefusal) -> Vec<SessionEffect<Fault>> {
    vec![SessionEffect::ClipboardRefused(refusal)]
}

#[test]
fn session_controller_clipboard_decisions_and_refusal_throttle() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);
    session.focus_changed(true);
    session.key(&key(KeyCode::Char('a')), 1_000);

    assert_eq!(
        ask(&bench, &mut session, "copy:hi", 1_100),
        [SessionEffect::Copy("hi".to_owned())]
    );
    assert_eq!(ask(&bench, &mut session, "drop", 1_100), []);
    assert_eq!(
        ask(&bench, &mut session, "?", 1_100),
        refused(SessionClipboardRefusal::Read)
    );
    assert_eq!(
        ask(&bench, &mut session, "big", 5_000),
        [],
        "a refusal inside the throttle window is not repeated"
    );
    assert_eq!(ask(&bench, &mut session, "plain", 11_099), []);
    assert_eq!(
        ask(&bench, &mut session, "plain", 11_100),
        refused(SessionClipboardRefusal::Write),
        "exactly the configured interval after the last report"
    );
    assert_eq!(ask(&bench, &mut session, "big", 21_099), []);
    assert_eq!(
        ask(&bench, &mut session, "big", 21_100),
        refused(SessionClipboardRefusal::TooLarge)
    );

    session.set_config(SessionConfig {
        refusal_status_every_ms: 0,
        ..SessionConfig::new(200)
    });
    assert_eq!(
        ask(&bench, &mut session, "plain", 21_200),
        refused(SessionClipboardRefusal::Write)
    );
    assert_eq!(
        ask(&bench, &mut session, "plain", 21_200),
        refused(SessionClipboardRefusal::Write),
        "a configured interval of 0 reports every refusal"
    );
}

#[test]
fn session_controller_effect_queue_keeps_the_newest_sixteen() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);
    for i in 0..20 {
        bench.push(0, SessionEvent::Notice(format!("n{i}")));
    }
    for _ in 0..10 {
        bench.push(0, SessionEvent::Notice("\u{1}\u{2}".to_owned()));
    }
    session.pump(900);
    let effects = session.take_effects();
    assert_eq!(effects.len(), PENDING_EFFECTS);
    let expected: Vec<_> = (4..20)
        .map(|i| SessionEffect::Notice(format!("n{i}")))
        .collect();
    assert_eq!(
        effects, expected,
        "the oldest go first, and a notice the policy emptied never counted"
    );
    assert_eq!(session.take_effects(), [], "the queue is drained");
}

#[test]
fn session_controller_reply_and_paste_limits() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);

    bench.fill(0, true);
    bench.output(0, b"\x1b[6n");
    session.pump(900);
    assert_eq!(session.dropped_replies(), 1, "a full queue drops the reply");
    assert_eq!(bench.commands(0), []);
    assert_eq!(session.paste("x", 910), SessionPaste::Full);
    assert_eq!(
        session.key(&key(KeyCode::Char('a')), 920),
        SessionKey::Consumed,
        "a key the link did not take is not forwarded"
    );
    bench.fill(0, false);
    bench.output(0, b"\x1b[6n");
    session.pump(930);
    assert_eq!(session.dropped_replies(), 1);
    assert_eq!(bench.commands(0).len(), 1, "the reply went out");

    session.set_config(SessionConfig {
        paste_max_bytes: 8,
        ..SessionConfig::new(200)
    });
    assert_eq!(session.paste("123456789", 940), SessionPaste::TooLarge);
    assert_eq!(
        session.paste_confirmed("123456789", 940),
        SessionPaste::TooLarge
    );
    assert_eq!(
        session.paste("a\nb", 950),
        SessionPaste::NeedsConfirm { lines: 2 }
    );
    assert_eq!(session.paste_confirmed("a\nb", 960), SessionPaste::Sent);
    assert_eq!(
        bench.commands(0).last(),
        Some(&input(b"a\nb")),
        "a confirmed paste goes as it is"
    );
    let sent = bench.commands(0).len();
    assert_eq!(session.paste("\u{1}", 970), SessionPaste::Sent);
    assert_eq!(bench.commands(0).len(), sent, "nothing is left to send");

    bench.output(0, b"\x1b[?2004h");
    session.pump(980);
    assert_eq!(session.paste("a\nb", 990), SessionPaste::Sent);
    assert_eq!(
        bench.commands(0).last(),
        Some(&input(b"\x1b[200~a\nb\x1b[201~")),
        "a program that asked for brackets gets them and no confirm"
    );

    let idle = controller(&bench);
    let mut idle = idle;
    assert_eq!(idle.paste("x", 0), SessionPaste::Dropped);
    assert_eq!(idle.paste_confirmed("x", 0), SessionPaste::Dropped);
}

#[test]
fn session_controller_legacy_control_flood_preserves_fifo() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);
    let before = bench.drains(0).len();
    for i in 0..4096 {
        bench.push(0, SessionEvent::Notice(format!("n{i}")));
    }
    assert!(session.pump(900));
    assert_eq!(
        bench.drains(0).len(),
        before + 1,
        "one drain returned every control event: the byte budget does not bound them"
    );
    let expected: Vec<_> = (4080..4096)
        .map(|i| SessionEffect::Notice(format!("n{i}")))
        .collect();
    assert_eq!(
        session.take_effects(),
        expected,
        "all 4096 were processed in order and the last 16 are kept"
    );
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );
}

#[test]
fn session_controller_asks_the_link_for_its_configured_drain_budget() {
    let bench = Bench::new();
    let mut session = TerminalSessionController::new(
        WidgetId::of("test.session"),
        bench.clone(),
        SessionConfig {
            feed_budget_bytes: 257,
            ..SessionConfig::new(200)
        },
        Arc::new(Policy),
    );
    session.sync_size(area(80, 24));
    bench.push(0, SessionEvent::Attached);
    bench.push(0, SessionEvent::Lease(SessionLease::Owner));
    bench.output(0, &vec![b'x'; 1_000]);
    let mut now = 0;
    while bench.queued(0) > 0 {
        session.pump(now);
        now += 1;
    }
    assert!(
        bench.drains(0).iter().all(|(_, budget)| *budget == 257),
        "{:?}",
        bench.drains(0)
    );
    assert_eq!(bench.fed(0), [257, 257, 257, 229]);
    assert_eq!(
        bench.fed(0).iter().sum::<usize>(),
        1_000,
        "no byte was lost"
    );
    assert_eq!(
        row(&session, 0).len(),
        80,
        "the whole stream reached the grid"
    );

    session.set_config(SessionConfig {
        feed_budget_bytes: 0,
        ..SessionConfig::new(200)
    });
    bench.output(0, b"held");
    session.pump(now);
    assert_eq!(
        bench.fed(0).last(),
        Some(&0),
        "a budget of 0 is passed as supplied"
    );
    assert_eq!(bench.queued(0), 1, "the output waits in the link");
    session.set_config(SessionConfig::new(200));
    session.pump(now + 1);
    assert_eq!(bench.queued(0), 0);
}

#[test]
fn session_controller_without_a_link_has_nothing_to_pump() {
    let bench = Bench::new();
    let mut session = controller(&bench);
    assert!(!session.pump(0), "no link, no change");
    assert_eq!(session.next_replay_deadline(), None);
    assert!(!session.live());
    assert!(!session.close_needs_confirm());
    session.redraw();
    session.focus_changed(true);
    assert_eq!(
        session.key(&key(KeyCode::Char('a')), 0),
        SessionKey::Consumed
    );

    let mut session = live(&bench, SessionLease::Owner);
    assert!(session.close_needs_confirm());
    bench.push(0, SessionEvent::Exited(None));
    assert!(session.pump(900));
    assert!(!session.close_needs_confirm());
    assert!(
        !session.pump(1_000),
        "an exited session has no link to pump"
    );
    session.redraw();
    assert_eq!(bench.commands(0), [], "a redraw is not sent to a gone link");
}

#[test]
fn session_controller_redraw_and_clear() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);
    session.redraw();
    assert_eq!(bench.commands(0), [SessionCommand::Redraw]);
    let lines: Vec<u8> = (0..40)
        .flat_map(|i| format!("line {i}\r\n").into_bytes())
        .collect();
    bench.output(0, &lines);
    session.pump(900);
    assert!(session.viewport().state().scrollback_len() > 0);
    session.clear();
    assert_eq!(session.viewport().state().scrollback_len(), 0);
    assert_eq!(
        bench.commands(0).len(),
        1,
        "clear sends nothing to the program"
    );
}

/// The fault the controller holds in its status, if the attach failed. The
/// bound is the host's `SessionFault` contract: the controller stores the
/// fault unchanged.
fn failed_fault<F: SessionFault>(session: &TerminalSessionController<F>) -> Option<F> {
    match session.status() {
        SessionStatus::Failed(fault) => Some(fault.clone()),
        _ => None,
    }
}

#[test]
fn session_fault_from_a_refused_open_reaches_status_and_effect() {
    let bench = Bench::new();
    bench
        .refusals
        .lock()
        .expect("refusals")
        .push_back(Fault("no pty"));
    let mut session = controller(&bench);
    session.sync_size(area(80, 24));
    assert_eq!(failed_fault(&session), Some(Fault("no pty")));
    assert_eq!(session.status(), &SessionStatus::Failed(Fault("no pty")));
    assert_eq!(bench.opened(), 0, "a refused open leaves no link");
    assert_eq!(
        session.take_effects(),
        vec![SessionEffect::Failed(Fault("no pty"))],
    );
}

#[test]
fn session_fault_from_a_stream_closed_before_attach_is_the_source_fault() {
    let bench = Bench::new();
    let mut session = controller(&bench);
    session.sync_size(area(80, 24));
    bench.push(0, SessionEvent::Ended(SessionEnd::StreamClosed));
    session.pump(0);
    assert_eq!(failed_fault(&session), Some(Fault("closed before attach")),);
    assert_eq!(
        session.take_effects(),
        vec![SessionEffect::Failed(Fault("closed before attach"))],
    );
}

#[test]
fn session_controller_viewport_handle_cannot_leave_the_grid_off_the_forwarded_size() {
    let bench = Bench::new();
    let mut session = attached(&bench, SessionLease::Owner);
    assert_eq!(session.viewport().state().size(), (24, 80));

    session.viewport_mut().resize(10, 10);
    session.sync_size(area(80, 24));

    assert_eq!(
        session.viewport().state().size(),
        (24, 80),
        "the grid follows the size the program has"
    );
    assert_eq!(bench.sizes(0), [size(24, 80)], "nothing new is forwarded");
}

/// A link whose stream has ended: every command comes back `Closed`.
#[derive(Debug)]
struct GoneLink;

impl SessionLink<Fault> for GoneLink {
    fn send(&self, _command: SessionCommand) -> SessionSent {
        SessionSent::Closed
    }

    fn set_size(&self, _size: SessionSize) {}

    fn drain(&self, _now_ms: i64, _output_budget: usize) -> Vec<SessionEvent<Fault>> {
        Vec::new()
    }
}

/// Upper-cases the title, so a swapped policy is visible in what the
/// controller shows.
#[derive(Debug)]
struct Shout;

impl SessionHostPolicy for Shout {
    fn title(&self, raw: &str) -> String {
        raw.to_uppercase()
    }

    fn notice(&self, raw: &str) -> String {
        raw.to_owned()
    }

    fn clipboard(
        &self,
        _context: SessionClipboardContext,
        _payload: &str,
    ) -> SessionClipboardDecision {
        SessionClipboardDecision::Drop
    }
}

#[test]
fn session_source_opens_a_link_that_reports_what_it_did_with_each_command() {
    fn host_fault<F: SessionFault>(fault: F) -> F {
        fault
    }
    let bench = Bench::new();
    let source: Arc<dyn SessionSource<Fault>> = bench.clone();
    let minimum: SessionSize = SessionConfig::MINIMUM;
    assert_eq!((minimum.rows, minimum.cols), (4, 20));
    let link: Box<dyn SessionLink<Fault>> = source.open(minimum).expect("a source opens a link");
    assert_eq!(
        bench.sizes(0),
        [minimum],
        "the link starts at the size it was opened at"
    );
    link.set_size(size(30, 100));
    assert_eq!(bench.sizes(0), [minimum, size(30, 100)]);

    assert_eq!(
        link.send(SessionCommand::Input(b"ls".to_vec())),
        SessionSent::Queued
    );
    assert_eq!(link.send(SessionCommand::Detach), SessionSent::Queued);
    assert_eq!(bench.commands(0), [input(b"ls"), SessionCommand::Detach]);
    bench.fill(0, true);
    assert_eq!(
        link.send(input(b"more")),
        SessionSent::Full,
        "a full queue refuses the command"
    );
    assert_eq!(
        bench.commands(0).len(),
        2,
        "a refused command is not queued"
    );
    assert_eq!(GoneLink.send(SessionCommand::Redraw), SessionSent::Closed);

    bench.output(0, b"hello");
    assert_eq!(
        link.drain(0, 3),
        [SessionEvent::Output(b"hel".to_vec())],
        "the budget splits a chunk and the rest waits"
    );
    assert_eq!(link.drain(1, 10), [SessionEvent::Output(b"lo".to_vec())]);
    assert_eq!(
        host_fault(Fault("boom")),
        Fault("boom"),
        "any comparable value is a fault"
    );
    assert_eq!(source.closed_before_attach(), Fault("closed before attach"));
}

#[test]
fn session_replay_timing_is_the_default_window_a_config_carries() {
    let timing = ReplayTiming::default();
    assert_eq!(
        (timing.replay_cap_ms, timing.nudge_ms, timing.quiet_ms),
        (800, 600, 150)
    );
    assert_eq!(SessionConfig::new(10).timing, timing);

    let bench = Bench::new();
    let mut session = TerminalSessionController::new(
        WidgetId::of("test.session"),
        bench.clone(),
        SessionConfig {
            timing: ReplayTiming {
                nudge_ms: 100,
                quiet_ms: 10,
                ..ReplayTiming::default()
            },
            ..SessionConfig::new(200)
        },
        Arc::new(Policy),
    );
    session.sync_size(area(80, 24));
    bench.push(0, SessionEvent::Attached);
    session.pump(0);
    assert_eq!(
        session.next_replay_deadline(),
        Some(100),
        "the nudge of the supplied timing sets the first deadline"
    );
}

#[test]
fn session_host_policy_decides_each_clipboard_request_from_its_context() {
    let policy: &dyn SessionHostPolicy = &Policy;
    let context = SessionClipboardContext {
        owner: true,
        focused: true,
        last_gesture_ms: Some(1_000),
        now_ms: 2_000,
    };
    let ask = |payload: &str| policy.clipboard(context, payload);
    assert_eq!(
        ask("c;copy:text"),
        SessionClipboardDecision::Copy("text".to_owned())
    );
    assert_eq!(ask("c;drop"), SessionClipboardDecision::Drop);
    assert_eq!(
        ask("c;?"),
        SessionClipboardDecision::Refuse(SessionClipboardRefusal::Read)
    );
    let stale = SessionClipboardContext {
        now_ms: 1_000 + GESTURE_WINDOW_MS + 1,
        ..context
    };
    assert_eq!(
        policy.clipboard(stale, "c;copy:text"),
        SessionClipboardDecision::Refuse(SessionClipboardRefusal::Write),
        "a gesture older than the window no longer permits a write"
    );
    let unfocused = SessionClipboardContext {
        focused: false,
        ..context
    };
    assert_eq!(
        policy.clipboard(unfocused, "c;copy:text"),
        SessionClipboardDecision::Refuse(SessionClipboardRefusal::Write)
    );
}

#[test]
fn session_controller_set_host_policy_replaces_the_sanitiser_for_later_events() {
    let bench = Bench::new();
    let mut session = live(&bench, SessionLease::Owner);
    bench.output(0, b"\x1b]0;first\x07");
    session.pump(900);
    assert_eq!(session.title(), Some("first"));

    session.set_host_policy(Arc::new(Shout));
    bench.output(0, b"\x1b]0;second\x07");
    session.pump(1_000);
    assert_eq!(
        session.title(),
        Some("SECOND"),
        "the new policy sanitises the next title"
    );
}
