use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::engine::{PointerConfig, StatusLine, TestRequest};
use tui_test::{
    CheckedMouseAction, KeyAction, LaunchEnvMode, LaunchOptions, MouseOptions, OpenOptions,
    Operation, OperationResult, RunOptions, Session, SnapshotCell, SnapshotScope, TerminalSnapshot,
    WaitQuietOptions,
};

#[allow(unused_imports)]
pub use crate::bins::{HOLLA, SHOWCASE, TABLEPRO};
const TIMEOUT: Duration = Duration::from_secs(10);
// A quitting app flushes state and restores the terminal, so under load it
// needs more than a screen wait.
const EXIT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

/// Which binary identity a launch is allowed to execute.
///
/// `Candidate` is the ordinary nextest path. It refuses a file whose sha256
/// is one of the recorded reference binaries, so a candidate run cannot be
/// swapped onto the frozen reference build. `Reference` is the capture path:
/// the file name must be `showcase`, `tablepro`, `holla`, or `jackin`, and
/// the sha256 must equal one digest recorded for that name. The top-level
/// digest stays the aarch64-apple-darwin file. `platforms` adds the other
/// host files built from the same reference commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMode {
    Candidate,
    Reference,
}

impl SessionMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Reference => "reference",
        }
    }
}

/// Launch description for a functional session.
///
/// `color`: `None` passes no `--color` and sets no `NO_COLOR`. `Some("nocolor")`
/// sets `NO_COLOR=1`. Any other value is passed as `--color`.
/// `history`: `false` sets `HOLLA_NO_HISTORY=1`. `true` omits that variable.
/// `motion`: when set, `--motion` is appended. `clock` enables the test clock.
/// [`Launch::new`] turns it on for TablePro. The app clock then moves only through
/// [`App::advance_clock`], so a status toast lives until the journey calls
/// [`App::expire_status`] and a press flash until [`App::settle_flash`].
/// `mode`: `Candidate` from [`Launch::new`] and [`App::new`]. `Reference` is
/// opt-in for captures against binaries named by a reference manifest. A
/// reference-only manifest override never participates in candidate checks.
#[derive(Clone, Debug)]
pub struct Launch {
    pub name: String,
    pub bin: PathBuf,
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    pub color: Option<String>,
    pub platform: String,
    pub history: bool,
    pub motion: Option<String>,
    pub clock: bool,
    pub env: Vec<(String, String)>,
    pub term: String,
    pub mode: SessionMode,
    pub reference_manifest: Option<PathBuf>,
}

impl Launch {
    pub fn new(name: &str, bin: impl AsRef<str>) -> Self {
        let bin = bin.as_ref();
        assert!(
            !bin.is_empty(),
            "launch `{name}` has an empty app binary path: the caller passed no candidate path. The root journeys take theirs from tests/functional/bins.rs; functional-cross looks the binary up by name."
        );
        Self {
            name: name.to_string(),
            bin: PathBuf::from(bin),
            args: Vec::new(),
            cols: 80,
            rows: 24,
            color: None,
            platform: "macos".to_string(),
            history: false,
            motion: None,
            clock: bin_name(bin) == Some("tablepro"),
            env: Vec::new(),
            term: "xterm-256color".to_string(),
            mode: SessionMode::Candidate,
            reference_manifest: None,
        }
    }
}

#[derive(Debug)]
pub struct LaunchError {
    pub evidence: PathBuf,
    pub message: String,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "spawn failed: {} ({})",
            self.message,
            self.evidence.display()
        )
    }
}

impl std::error::Error for LaunchError {}

/// How many of the latest checkpoints keep their full-cell JSON when a test
/// fails. Every checkpoint keeps its text and its line in the action trace.
const FAILURE_CELL_CHECKPOINTS: usize = 16;

pub struct App {
    session: Session,
    directory: PathBuf,
    actions: RefCell<Vec<String>>,
    checkpoint: Cell<usize>,
    recent: RefCell<VecDeque<(usize, TerminalSnapshot)>>,
    clock: bool,
    fence: bool,
    /// A button was pressed or released since the last [`App::flash_expired`].
    clicked: Cell<bool>,
}

impl App {
    pub fn new(
        name: &str,
        bin: impl AsRef<str>,
        args: &[&str],
        cols: u16,
        rows: u16,
        color: &str,
    ) -> Self {
        let bin = bin.as_ref();
        let mut launch = Launch::new(name, bin);
        launch.args = args.iter().map(|arg| (*arg).to_string()).collect();
        launch.cols = cols;
        launch.rows = rows;
        launch.color = Some(color.to_string());
        // `Launch::new` enables the clock for TablePro, `App::new` also for jackin. Elsewhere
        // the paste falls through to `on_input` (src/shell/app.rs), so the clock stays off.
        launch.clock = launch.clock
            || bin_name(bin).is_some_and(|name| matches!(name, "jackin" | "jackin-preview"));
        try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn evidence_dir(&self) -> &Path {
        &self.directory
    }

    pub fn child_running(&self) -> bool {
        // `session::Session::is_alive` is crate-private. `Operation::State` is the
        // public view the process watcher fills from that same wait.
        match self
            .session
            .execute(Operation::State)
            .expect("process state")
        {
            OperationResult::State(state) => state.exited.is_none() && state.exit_signal.is_none(),
            _ => panic!("missing process state"),
        }
    }

    pub fn screen(&self) -> TerminalSnapshot {
        self.session
            .capture_snapshot(SnapshotScope::ActiveScreen)
            .expect("capture live terminal")
    }

    pub fn wait_screen(
        &self,
        reason: &str,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        self.wait_screen_for(reason, TIMEOUT, predicate)
    }

    pub fn wait_screen_for(
        &self,
        reason: &str,
        timeout: Duration,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        let deadline = Instant::now() + timeout;
        loop {
            self.assert_running(reason);
            let screen = self.screen();
            if predicate(&screen) {
                self.record(reason, &screen);
                return screen;
            }
            assert!(
                Instant::now() < deadline,
                "timed out: {reason}\n{}",
                text(&screen)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn wait_text_after_animation(&self, needle: &str, animation: Duration) -> TerminalSnapshot {
        self.wait_screen_for(
            &format!("visible {needle}"),
            TIMEOUT + animation,
            |screen| text(screen).contains(needle),
        )
    }

    pub fn stable_screen(&self) -> TerminalSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        self.assert_running("stability");
        let mut screen = self.screen();
        let mut stable_since = Instant::now();
        loop {
            if stable_since.elapsed() >= Duration::from_millis(700) {
                return screen;
            }
            if Instant::now() >= deadline {
                let visible = text(&screen);
                let _ = std::fs::write(self.directory.join("last.txt"), &visible);
                self.log(format!("stability deadline {}", self.directory.display()));
                panic!("stability deadline {}", self.directory.display());
            }
            std::thread::sleep(Duration::from_millis(100));
            self.assert_running("stability");
            let candidate = self.screen();
            if candidate != screen {
                screen = candidate;
                stable_since = Instant::now();
            }
        }
    }

    pub fn visual(&self, test_id: &str) {
        let root = baseline_root(test_id);
        let outcome = self.check_visual_with_precondition(&root, test_id, "");
        // The cells are the accepted paint. A hidden cursor parks on another
        // column of an otherwise equal frame.
        if Self::cursor_only(&outcome) {
            self.log(format!("visual {test_id} cursor only"));
            return;
        }
        assert!(
            outcome.is_pass(),
            "visual mismatch for {test_id}: {outcome:?}"
        );
        self.log(format!("visual {test_id} pass"));
    }

    /// Compare one frame that stays ready for 200ms.
    ///
    /// [`Self::visual`] waits until the screen stops changing. A status toast
    /// that expires during that wait is the frame that gets stored. When the
    /// toast clears after a ready frame, the last ready frame is compared.
    pub fn visual_while(&self, test_id: &str, ready: impl Fn(&TerminalSnapshot) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut good: Option<TerminalSnapshot> = None;
        let mut since: Option<Instant> = None;
        loop {
            let screen = self.screen();
            if ready(&screen) {
                let same = good.as_ref().is_some_and(|prev| prev == &screen);
                if same && since.expect("hold start").elapsed() >= Duration::from_millis(200) {
                    self.compare_frame(test_id, &screen);
                    return;
                }
                if !same {
                    good = Some(screen);
                    since = Some(Instant::now());
                }
            } else if let Some(prev) = good.take() {
                self.compare_frame(test_id, &prev);
                return;
            }
            assert!(
                Instant::now() < deadline && self.child_running(),
                "{test_id} frame not seen\n{}",
                text(&self.screen())
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn compare_frame(&self, test_id: &str, screen: &TerminalSnapshot) {
        let root = baseline_root(test_id);
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            screen.clone(),
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build Tuiscotti state sample");
        let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
        let artifacts = self.directory.join("check-artifacts");
        let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
            .expect("open Tuiscotti frozen store");
        let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
        if Self::cursor_only(&outcome) {
            self.log(format!("visual {test_id} cursor only"));
            return;
        }
        assert!(
            outcome.is_pass(),
            "visual mismatch for {test_id}: {outcome:?}"
        );
        self.log(format!("visual {test_id} pass"));
    }

    pub fn check_visual(
        &self,
        baseline_root: &Path,
        test_id: &str,
    ) -> tuiscotti_latest::check::CheckOutcome {
        self.check_visual_with_precondition(baseline_root, test_id, "")
    }

    pub(crate) fn cursor_only(outcome: &tuiscotti_latest::check::CheckOutcome) -> bool {
        let tuiscotti_latest::check::CheckOutcome::Failure(failure) = outcome else {
            return false;
        };
        let fields = failure.changed_fields();
        failure.changed_cell_total() == 0
            && !fields.is_empty()
            && fields
                .iter()
                .all(|field| field == "cursor.col" || field == "cursor.row")
    }

    /// Compare the live screen with a frozen baseline.
    ///
    /// `precondition` is recorded as `precondition <text>` before the
    /// comparison. [`App::visual`] and [`App::check_visual`] pass an empty
    /// text. This reads the baseline root and writes only under the evidence
    /// directory.
    pub fn check_visual_with_precondition(
        &self,
        baseline_root: &Path,
        test_id: &str,
        precondition: &str,
    ) -> tuiscotti_latest::check::CheckOutcome {
        let _ = self.settle();
        let screen = self.stable_screen();
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            screen,
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build Tuiscotti state sample");
        let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
        let artifacts = self.directory.join("check-artifacts");
        let store = tuiscotti_latest::check::FrozenStore::new(baseline_root, &artifacts)
            .expect("open Tuiscotti frozen store");
        self.log(format!("precondition {precondition}"));
        let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
        let sha = match &outcome {
            tuiscotti_latest::check::CheckOutcome::Pass(success) => {
                success.candidate_sha256().to_string()
            }
            tuiscotti_latest::check::CheckOutcome::Failure(failure) => {
                failure.candidate_sha256().to_string()
            }
        };
        self.log(format!("visual {test_id} candidate {sha}"));
        outcome
    }

    pub fn try_sustained(
        &self,
        hold: Duration,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> bool {
        if !self.child_running() || !predicate(&self.screen()) {
            return false;
        }
        let deadline = Instant::now() + hold * 4 + Duration::from_secs(2);
        let mut since: Option<Instant> = None;
        loop {
            let screen = self.screen();
            if self.child_running() && predicate(&screen) {
                let start = *since.get_or_insert_with(Instant::now);
                if start.elapsed() >= hold {
                    return true;
                }
            } else {
                since = None;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn wait_text(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("visible {needle}"), |screen| {
            text(screen).contains(needle)
        })
    }

    /// Wait until `find` locates a point on the screen, then return it.
    ///
    /// Use it in place of a one-shot `screen()` lookup, which fails when the
    /// frame that holds the target is still being painted.
    pub fn wait_point(
        &self,
        reason: &str,
        find: impl Fn(&TerminalSnapshot) -> Option<Point>,
    ) -> Point {
        let found = RefCell::new(None);
        self.wait_screen(reason, |screen| {
            *found.borrow_mut() = find(screen);
            found.borrow().is_some()
        });
        found
            .into_inner()
            .expect("wait_screen returned with a point")
    }

    /// Press `key` until `reached` accepts the screen.
    ///
    /// After each press it waits for `watch(screen)` to change or for
    /// `reached` to hold. The wait ends on that state, not on a short
    /// deadline, so a loaded machine slows the walk and cannot cut it off. A
    /// press that moves nothing within the harness timeout is a dead end and
    /// fails with the screen. `max_presses` bounds a walk that keeps moving
    /// without arriving.
    pub fn press_until<W: PartialEq>(
        &self,
        key: &str,
        goal: &str,
        max_presses: usize,
        watch: impl Fn(&TerminalSnapshot) -> W,
        reached: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        for _ in 0..max_presses {
            let screen = self.screen();
            if reached(&screen) {
                return screen;
            }
            let before = watch(&screen);
            self.key(key);
            self.wait_screen(&format!("{key} moves toward {goal}"), |screen| {
                reached(screen) || watch(screen) != before
            });
        }
        let screen = self.screen();
        assert!(
            reached(&screen),
            "never reached {goal} with {max_presses} x {key}\n{}",
            text(&screen)
        );
        screen
    }

    pub fn wait_absent(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("absent {needle}"), |screen| {
            !text(screen).contains(needle)
        })
    }

    /// Waits for 150 ms without output, after the app has handled and drawn
    /// every input sent so far.
    ///
    /// Quiet alone is not proof the app acted: a loaded app can still be
    /// reading a wheel step or a key when 150 ms pass, and the frame read then
    /// is the one from before it. The fence queues behind every earlier input,
    /// so the quiet that follows is the quiet after the input's frame. An app
    /// that exited, or does not answer the fence (a reference build), gets
    /// the quiet wait alone.
    pub fn settle(&self) -> TerminalSnapshot {
        if self.fence && !legacy_waits() && self.child_running() {
            self.fence_while_running();
        }
        self.session
            .wait_quiet(&WaitQuietOptions::new(TIMEOUT, Duration::from_millis(150)).unwrap())
            .expect("terminal must settle after the semantic assertion");
        let screen = self.screen();
        self.record("settled", &screen);
        screen
    }

    pub fn point(&self, needle: &str) -> Point {
        find(&self.wait_text(needle), needle)
    }

    pub fn key(&self, key: &str) {
        self.log(format!("key {key}"));
        self.session
            .execute(Operation::Key {
                keys: vec![key.to_string()],
                action: KeyAction::Press,
            })
            .unwrap_or_else(|e| panic!("key {key}: {e}"));
    }

    pub fn type_text(&self, value: &str) {
        self.log(format!("type {} UTF-8 bytes", value.len()));
        self.session
            .write_bytes(value.as_bytes())
            .expect("type through terminal input");
    }

    pub fn paste(&self, value: &str) {
        self.log(format!("paste {} UTF-8 bytes", value.len()));
        self.session
            .paste(value)
            .expect("paste using negotiated terminal mode");
    }

    pub fn advance_clock(&self, milliseconds: u64) {
        if !self.clock {
            self.log("clock refused".to_string());
            return;
        }
        self.paste(&TestRequest::advance(milliseconds));
    }

    /// Lets every status toast reach the end of its lifetime on the stepped clock.
    pub fn expire_status(&self) {
        self.advance_clock(StatusLine::<()>::DEFAULT_TTL_MS);
    }

    /// Lets the pointer press flash reach its deadline on the stepped clock.
    pub fn settle_flash(&self) {
        self.advance_clock(PointerConfig::DEFAULT.flash_ms);
    }

    pub fn move_to(&self, point: Point) {
        self.mouse(CheckedMouseAction::Motion {
            x: point.x,
            y: point.y,
        });
    }

    pub fn down(&self, point: Point) {
        self.mouse(CheckedMouseAction::Press {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    pub fn up(&self, point: Point) {
        self.mouse(CheckedMouseAction::Release {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    pub fn drag_to(&self, point: Point) {
        self.mouse(CheckedMouseAction::Drag {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    fn mouse(&self, action: CheckedMouseAction) {
        self.log(format!("mouse {action:?}"));
        if matches!(
            action,
            CheckedMouseAction::Press { .. } | CheckedMouseAction::Release { .. }
        ) {
            self.clicked.set(true);
        }
        self.session
            .mouse_checked(action)
            .expect("application must negotiate SGR mouse input for this action");
    }

    pub fn click(&self, point: Point) {
        self.down(point);
        self.up(point);
    }

    pub fn click_text(&self, needle: &str) {
        self.click(self.point(needle));
    }

    pub fn wheel(&self, point: Point, down: bool, amount: usize) {
        self.move_to(point);

        let bytes = tui_test::input::mouse::scroll(point.x, point.y, !down);
        self.log(format!("wheel {point:?} down={down} amount={amount}"));
        for _ in 0..amount {
            self.session
                .write_bytes(bytes.as_bytes())
                .expect("wheel through application PTY");
        }
    }

    /// Sends a wheel step and waits for the page to show its effect. `settle`
    /// only waits for 150 ms without output, which a loaded app outlasts, so a
    /// loop that reads `settle` after `wheel` can read the frame from before the
    /// step and send another one. Use this where every step is known to move
    /// the page, and `wheel` where the loop is looking for the end of the scroll.
    pub fn wheel_applied(&self, point: Point, down: bool, amount: usize) -> TerminalSnapshot {
        let before = text(&self.screen());
        self.wheel(point, down, amount);
        self.wait_screen("wheel applied", |screen| text(screen) != before)
    }

    /// Wheels in steps of `amount` notches until `reached(screen)` holds, and
    /// returns the settled screen that satisfies it.
    ///
    /// Every step is read only after [`App::settle`], which fences behind every
    /// notch sent so far. A loop that reads a live `screen` between steps can
    /// see the needle while later notches are still queued, and those notches
    /// move the page past the position the loop stopped at. Here the stop
    /// position is a function of `amount` alone: the first multiple of `amount`
    /// notches that satisfies `reached`, whatever the load on the machine.
    pub fn wheel_until(
        &self,
        point: Point,
        down: bool,
        amount: usize,
        max_steps: usize,
        goal: &str,
        reached: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        for _ in 0..max_steps {
            let screen = self.settle();
            if reached(&screen) {
                return screen;
            }
            self.wheel(point, down, amount);
        }
        let screen = self.settle();
        assert!(
            reached(&screen),
            "wheel never reached {goal} after {max_steps} x {amount} notches\n{}",
            text(&screen)
        );
        screen
    }

    /// Resizes the PTY and returns once the app has drawn at the new size.
    ///
    /// The emulator reports the new cols and rows at once but keeps the old
    /// frame until the app handles the resize and redraws, and a full repaint
    /// arrives in several chunks. A read or click taken before the redraw ends
    /// sees the previous layout or a mix of both. The fence closes the gap:
    /// the shell answers it with a BEL queued behind the frame of every
    /// earlier input, so the bell proves the resize was handled and drawn.
    pub fn resize(&self, cols: u16, rows: u16) {
        self.log(format!("resize {cols}x{rows}"));
        self.session
            .execute(Operation::Resize { cols, rows })
            .expect("resize PTY");
        self.fence();
    }

    fn bells(&self) -> u64 {
        match self
            .session
            .execute(Operation::GetBellCount)
            .expect("bell count")
        {
            OperationResult::BellCount(count) => count,
            _ => panic!("missing bell count"),
        }
    }

    /// Waits until the app has handled and drawn everything sent so far. Only
    /// a candidate Showcase, TablePro, Holla or Jackin answers it (see
    /// [`fenced`]). A reference build or another program gets no wait.
    pub fn fence(&self) {
        if self.fence {
            self.fence_until(false);
        }
    }

    /// Returns once the 140 ms press flash of every click sent so far has
    /// expired in the app's own time, so the next draw paints the settled
    /// colour. A same-size resize is such a draw.
    ///
    /// The fence proves the app has handled the click, so its flash deadline
    /// is at most [`PointerConfig::DEFAULT`]`.flash_ms` after now. Waiting that
    /// long from the fence is exact where a fixed sleep from the click is a guess
    /// a loaded app can lose. A session on the stepped test clock never needs
    /// the wait: its flash expires only on [`App::settle_flash`], and the wall
    /// time that passes changes nothing. A session that does not answer the
    /// fence (a reference build) keeps a 200 ms sleep.
    pub fn flash_expired(&self) {
        let clicked = self.clicked.replace(false);
        if !self.fence || legacy_waits() {
            std::thread::sleep(Duration::from_millis(200));
            return;
        }
        self.fence();
        if clicked && !self.clock {
            std::thread::sleep(Duration::from_millis(PointerConfig::DEFAULT.flash_ms + 10));
        }
    }

    /// Waits out the press flash, then widens the terminal by one column and
    /// restores it. A same-size resize does not force a frame, and the frame
    /// after a flash is the one a capture needs. `ready` must hold on the screen
    /// at both sizes. With `left`, it must also hold before the nudge, and the
    /// message says what the screen showed instead.
    pub fn nudge_width(&self, ready: impl Fn(&TerminalSnapshot) -> bool, left: Option<&str>) {
        self.nudge_sized(|screen, _, _| ready(screen), left);
    }

    /// [`App::nudge_width`] for a `ready` that also needs the size the screen is
    /// expected to have, as columns then rows.
    pub fn nudge_sized(
        &self,
        ready: impl Fn(&TerminalSnapshot, u16, u16) -> bool,
        left: Option<&str>,
    ) {
        self.flash_expired();
        let screen = self.screen();
        let cols = u16::try_from(screen.cols()).expect("cols");
        let rows = u16::try_from(screen.rows()).expect("rows");
        if let Some(left) = left {
            assert!(ready(&screen, cols, rows), "{left}\n{}", text(&screen));
        }
        let wider = cols + 1;
        self.resize(wider, rows);
        self.wait_screen("nudged", |screen| {
            screen.cols() == u32::from(wider)
                && screen.rows() == u32::from(rows)
                && ready(screen, wider, rows)
        });
        self.resize(cols, rows);
        self.wait_screen("size restored", |screen| {
            screen.cols() == u32::from(cols)
                && screen.rows() == u32::from(rows)
                && ready(screen, cols, rows)
        });
    }

    /// Waits out the press flash, hops to 121 columns and back to `cols` by
    /// `rows`, and waits for the frame a capture needs. Below 100 columns the
    /// hop label is gone and `title` is on screen. From 100 up the size chip
    /// names the size.
    pub fn clear_flash_hop(&self, cols: u16, rows: u16, title: &str) {
        self.flash_expired();
        self.resize(121, rows);
        let hopped = format!("121×{rows}");
        self.wait_text(&hopped);
        self.resize(cols, rows);
        self.wait_screen(&format!("settled {cols}x{rows}"), |screen| {
            let visible = text(screen);
            if cols >= 100 {
                visible.contains(&format!("{cols}×{rows}"))
            } else {
                !visible.contains(&hopped) && visible.contains(title)
            }
        });
    }

    /// [`App::clear_flash_hop`] for pages whose hop is to 121x40 and shows
    /// `interim` there. The settled frame holds `token`. Below 100 columns
    /// neither `interim` nor `wide` is left. The pointer then parks on the last
    /// row.
    pub fn clear_flash_interim(&self, cols: u16, rows: u16, token: &str, labels: (&str, &str)) {
        let (interim, wide) = labels;
        self.flash_expired();
        self.resize(121, 40);
        self.wait_text(interim);
        self.resize(cols, rows);
        self.wait_screen(&format!("settled {cols}x{rows}"), |screen| {
            let visible = text(screen);
            if screen.cols() != u32::from(cols)
                || screen.rows() != u32::from(rows)
                || !visible.contains(token)
            {
                return false;
            }
            if cols < 100 {
                !visible.contains(interim) && !visible.contains(wide)
            } else {
                visible.contains(&format!("{cols}×{rows}"))
            }
        });
        self.move_to(Point {
            x: 2,
            y: rows.saturating_sub(1),
        });
    }

    /// [`App::fence`] for a read that is also fine with an app that quits on
    /// the input before it: the wait ends when the child is gone.
    fn fence_while_running(&self) {
        self.fence_until(true);
    }

    fn fence_until(&self, exit_ends_wait: bool) {
        let before = self.bells();
        self.log("fence".to_string());
        self.session
            .paste(TestRequest::FENCE)
            .expect("fence through bracketed paste");
        let deadline = Instant::now() + TIMEOUT;
        while self.bells() == before {
            if exit_ends_wait && !self.child_running() {
                return;
            }
            self.assert_running("fence");
            assert!(
                Instant::now() < deadline,
                "timed out: fence\n{}",
                text(&self.screen())
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// [`App::resize`], then the screen at the new size.
    pub fn wait_size(&self, cols: u16, rows: u16) -> TerminalSnapshot {
        self.resize(cols, rows);
        self.wait_screen(&format!("size {cols}x{rows}"), |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        })
    }

    /// Wait for the child to exit. The wait ends on the exit itself, so only
    /// an app that never quits runs out the budget.
    pub fn wait_exit(&self, reason: &str) {
        let deadline = Instant::now() + EXIT_TIMEOUT;
        while self.child_running() {
            assert!(
                Instant::now() < deadline,
                "{reason}\n{}",
                text(&self.screen())
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Moves to the focused Quit button of the open quit dialog, confirms it and waits for
    /// TablePro to exit cleanly.
    pub fn confirm_quit(&self) {
        self.key("right");
        self.key("enter");
        self.wait_exit("tablepro stayed up after confirming quit");
        self.expect_exited(0);
    }

    pub fn finish(&self) {
        self.assert_running("before Ctrl+Q");
        self.key("ctrl+q");
        self.session
            .execute(Operation::WaitExit {
                timeout_ms: Some(10_000),
            })
            .expect("application must exit after Ctrl+Q");
        self.assert_restored_exit();
    }

    /// The child has already exited. The terminal modes match a clean quit.
    pub(crate) fn assert_restored_exit(&self) {
        let OperationResult::State(state) =
            self.session.execute(Operation::State).expect("exit state")
        else {
            panic!("missing exit state");
        };
        assert_eq!(state.exited, Some(0), "normal application exit");
        assert_eq!(state.exit_signal, None, "application must not be killed");
        assert_eq!(state.mouse_mode, "none", "mouse tracking restored on exit");
        assert!(state.cursor.visible, "terminal cursor restored on exit");
        for mode in ["alternate_screen", "bracketed_paste"] {
            assert_eq!(
                state.modes.get(mode),
                Some(&false),
                "terminal mode {mode} restored on exit"
            );
        }
        self.log("clean exit 0".to_string());
    }

    pub fn expect_exited(&self, code: i32) {
        let alive = self.child_running();
        let OperationResult::State(state) =
            self.session.execute(Operation::State).expect("exit state")
        else {
            panic!("missing exit state");
        };
        assert!(!alive, "process is still running");
        assert_eq!(state.exited, Some(code), "process exit code");
        assert_eq!(state.exit_signal, None, "process must not be killed");
        self.log(format!("observed exit {code}"));
    }

    pub fn kill_child(&self) {
        self.log("kill child".to_string());
        // `session::Session::kill` is crate-private. Signal KILL is the public
        // operation that terminates the child through the same PTY kill.
        self.session
            .execute(Operation::Signal {
                name: "KILL".to_string(),
            })
            .expect("kill child");
        self.wait_exit("child survived Session::kill");
    }

    fn assert_running(&self, reason: &str) {
        let alive = self.child_running();
        let OperationResult::State(state) = self
            .session
            .execute(Operation::State)
            .expect("process state")
        else {
            panic!("missing process state while {reason}");
        };
        if alive && state.exited.is_none() && state.exit_signal.is_none() {
            return;
        }
        let visible = self
            .session
            .capture_snapshot(SnapshotScope::ActiveScreen)
            .map(|screen| text(&screen))
            .unwrap_or_default();
        let _ = std::fs::write(self.directory.join("last.txt"), &visible);
        self.persist_actions();
        panic!(
            "application exited while {reason}: code={:?} signal={:?}\n{visible}",
            state.exited, state.exit_signal
        );
    }

    fn log(&self, message: String) {
        self.actions.borrow_mut().push(message);
        self.persist_actions();
    }

    fn persist_actions(&self) {
        std::fs::write(
            self.directory.join("actions.txt"),
            self.actions.borrow().join("\n"),
        )
        .expect("persist functional action trace");
    }

    fn record(&self, label: &str, screen: &TerminalSnapshot) {
        let number = self.checkpoint.get();
        self.checkpoint.set(number + 1);
        let mut hasher = std::hash::DefaultHasher::new();
        screen.hash(&mut hasher);
        self.log(format!(
            "checkpoint {number:03}: {label} {:016x}",
            hasher.finish()
        ));
        let path = self.directory.join(format!("{number:03}"));
        std::fs::write(path.with_extension("txt"), text(screen)).expect("save terminal text");
        let mut recent = self.recent.borrow_mut();
        if recent.len() == FAILURE_CELL_CHECKPOINTS {
            recent.pop_front();
        }
        recent.push_back((number, screen.clone()));
    }

    fn save_failure_cells(&self) {
        for (number, screen) in self.recent.borrow().iter() {
            if let Ok(json) = serde_json::to_vec(screen) {
                let _ = std::fs::write(self.directory.join(format!("{number:03}.json")), json);
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Ok(screen) = self.session.capture_snapshot(SnapshotScope::ActiveScreen) {
            let _ = std::fs::write(self.directory.join("last.txt"), text(&screen));
            if let Ok(json) = serde_json::to_vec(&screen) {
                let _ = std::fs::write(self.directory.join("last.json"), json);
            }
        }
        let _ = std::fs::write(
            self.directory.join("actions.txt"),
            self.actions.borrow().join("\n"),
        );
        let _ = self.session.close();
        let _ = std::fs::remove_dir_all(self.directory.join("home"));
        if std::thread::panicking() {
            self.save_failure_cells();
            eprintln!("functional evidence: {}", self.directory.display());
        }
    }
}

/// Lets the 140ms press flash reach its deadline, then redraws once. A session on the stepped
/// test clock advances it with [`App::settle_flash`]. Any other session waits on the wall clock.
/// See [`redraw_after_flash`].
pub fn flash_settle(app: &App) {
    if app.clock {
        app.settle_flash();
    } else {
        app.flash_expired();
    }
    redraw_after_flash(app);
}

/// Forces one redraw after the press flash deadline has passed. A same-size resize does not
/// force a frame, so the width is nudged by one column and restored. Below about 100 columns
/// the header omits the size chip, so the nudge waits on the terminal size instead. The redraw
/// also leaves the hidden terminal cursor in a fixed place.
fn redraw_after_flash(app: &App) {
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    if cols >= 100 {
        app.wait_text(&format!("{nudged}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow nudge", |screen| {
            screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        app.wait_screen("narrow restored", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        });
    }
}

/// A candidate build of an app whose shell answers the fence.
fn fenced(launch: &Launch) -> bool {
    launch.mode == SessionMode::Candidate
        && bin_name(launch.bin.to_str().unwrap_or_default()).is_some_and(|name| {
            matches!(
                name,
                "showcase" | "tablepro" | "holla" | "jackin" | "jackin-preview"
            )
        })
}

fn bin_name(bin: &str) -> Option<&str> {
    Path::new(bin).file_name().and_then(|file| file.to_str())
}

pub fn try_launch(launch: Launch) -> Result<App, LaunchError> {
    let directory = evidence_directory(&launch.name);
    let home = directory.join("home");
    std::fs::create_dir_all(&home).expect("create isolated application home");
    let resolved = resolve_bin(&launch.bin);
    let manifest = match (launch.mode, launch.reference_manifest.as_deref()) {
        (SessionMode::Candidate, Some(_)) => {
            return refuse_launch(
                directory,
                "candidate mode cannot select a reference manifest".to_string(),
            );
        }
        (SessionMode::Candidate, None) => read_manifest(&manifest_path()),
        (SessionMode::Reference, Some(path)) => read_manifest(path),
        (SessionMode::Reference, None) => read_manifest(&manifest_path()),
    };
    let sha = if resolved.is_file() {
        Some(file_sha256(&resolved))
    } else {
        None
    };
    if let Some(message) = identity_refusal(&launch, &resolved, sha.as_deref(), &manifest) {
        return refuse_launch(directory, message);
    }
    let args = child_args(&launch);
    let session = Session::new(&launch.name);
    let defaults = OpenOptions::default();
    let spawn = session.spawn_with(
        prepared_launch(&launch, &home),
        RunOptions {
            backend: defaults.backend,
            program: String::new(),
            args: Vec::new(),
            profile: defaults.profile,
            cols: launch.cols,
            rows: launch.rows,
            cwd: None,
            env: Vec::new(),
            wait_ready: Some(false),
            restart: false,
            timeouts: defaults.timeouts,
            recording: defaults.recording,
            style: defaults.style,
        },
    );
    match spawn {
        Ok(_opened) => {
            let sha = sha.expect("launched binary has a digest");
            write_binary_record(&directory, &resolved, &sha);
            write_session(&directory, &launch, &resolved, &sha, &manifest);
            let app = App {
                session,
                directory,
                actions: RefCell::new(vec![format!(
                    "spawn {} {args:?} {}x{} color={:?} clock={}",
                    launch.bin.display(),
                    launch.cols,
                    launch.rows,
                    launch.color,
                    launch.clock
                )]),
                checkpoint: Cell::new(0),
                recent: RefCell::new(VecDeque::new()),
                clock: launch.clock,
                fence: fenced(&launch),
                clicked: Cell::new(false),
            };
            app.persist_actions();
            Ok(app)
        }
        Err(error) => {
            let message = format!("{}: {error}", launch.bin.display());
            std::fs::write(directory.join("spawn-error.txt"), &message)
                .expect("write spawn-error.txt");
            Err(LaunchError {
                evidence: directory,
                message,
            })
        }
    }
}

/// The point two columns into the one `label` whose row `row_ok` accepts. It waits for that row
/// to be painted.
pub fn button(app: &App, label: &str, row_ok: impl Fn(&str) -> bool) -> Point {
    app.wait_point(&format!("one button {label:?}"), |screen| {
        let hits: Vec<_> = find_all(screen, label)
            .into_iter()
            .filter(|point| row_ok(&row_text(screen, u32::from(point.y))))
            .collect();
        (hits.len() == 1).then(|| Point {
            x: hits[0].x + 2,
            y: hits[0].y,
        })
    })
}

/// Presses `key` until the focused row, the first one carrying the `▎` marker, contains `marker`.
/// A jackin screen walk of at most 12 presses. See [`App::press_until`].
pub fn step_until(app: &App, key: &str, marker: &str) {
    app.press_until(key, marker, 12, focused_line, |screen| {
        focused_line(screen).contains(marker)
    });
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

pub fn text(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn row_text(screen: &TerminalSnapshot, row: u32) -> String {
    screen
        .row(row)
        .expect("row in terminal")
        .iter()
        .map(SnapshotCell::text)
        .collect()
}

pub fn find_all(screen: &TerminalSnapshot, needle: &str) -> Vec<Point> {
    assert!(!needle.is_empty(), "locator must not be empty");
    let mut matches = Vec::new();
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        for (byte, _) in line.match_indices(needle) {
            let mut offset = 0;
            for (column, cell) in screen.row(row).unwrap().iter().enumerate() {
                let next = offset + cell.text().len();
                if byte >= offset && byte < next {
                    matches.push(Point {
                        x: u16::try_from(column).unwrap(),
                        y: u16::try_from(row).unwrap(),
                    });
                    break;
                }
                offset = next;
            }
        }
    }
    matches
}

pub fn find(screen: &TerminalSnapshot, needle: &str) -> Point {
    let matches = find_all(screen, needle);
    assert_eq!(
        matches.len(),
        1,
        "expected one physical-row match for {needle:?}, got {matches:?}\n{}",
        text(screen)
    );
    matches[0]
}

pub fn find_in(
    screen: &TerminalSnapshot,
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
    needle: &str,
) -> Point {
    let matches: Vec<_> = find_all(screen, needle)
        .into_iter()
        .filter(|point| point.x >= left && point.x <= right && point.y >= top && point.y <= bottom)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one match for {needle:?} inside ({left},{top})-({right},{bottom}), got {matches:?}\n{}",
        text(screen)
    );
    matches[0]
}

/// One painted grapheme cluster: its text, the cells it spans, whether it is
/// bold, and the background of every cell it spans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    pub text: String,
    pub width: u16,
    pub bold: bool,
    pub bg: Vec<tui_test::SnapshotColor>,
}

/// The first `count` clusters on `from`'s row, starting at `from.x`. A wide
/// cluster is one entry that spans its continuation cells.
pub fn clusters(screen: &TerminalSnapshot, from: Point, count: usize) -> Vec<Cluster> {
    let row = screen.row(u32::from(from.y)).expect("row in terminal");
    let mut out = Vec::new();
    let mut column = usize::from(from.x);
    while out.len() < count && column < row.len() {
        let cell = &row[column];
        let span = 1 + row[column + 1..]
            .iter()
            .take_while(|next| next.is_continuation())
            .count();
        out.push(Cluster {
            text: cell.text().to_string(),
            width: u16::try_from(span).expect("cluster width"),
            bold: cell.has(tui_test::terminal::cell::Attrs::BOLD),
            bg: row[column..column + span].iter().map(|c| c.bg()).collect(),
        });
        column += span;
    }
    out
}

pub fn cell_bg(screen: &TerminalSnapshot, point: Point) -> tui_test::SnapshotColor {
    screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell within terminal")
        .bg()
}

pub fn crop(screen: &TerminalSnapshot, point: Point, width: u16, height: u16) -> Vec<SnapshotCell> {
    (point.y..point.y + height)
        .flat_map(|y| {
            (point.x..point.x + width).map(move |x| {
                screen
                    .cell_at(u32::from(x), u32::from(y))
                    .expect("crop within terminal")
                    .clone()
            })
        })
        .collect()
}

pub fn file_sha256(path: &Path) -> String {
    let output = if Path::new("/usr/bin/shasum").is_file() {
        std::process::Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .arg(path)
            .output()
    } else {
        std::process::Command::new("/usr/bin/sha256sum")
            .arg(path)
            .output()
    }
    .expect("run sha256");
    assert!(
        output.status.success(),
        "sha256 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("sha256 output")
        .split_whitespace()
        .next()
        .expect("sha256 digest")
        .to_string()
}

fn evidence_directory(name: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    workspace_root()
        .join("target/functional")
        .join(format!("{name}-{}-{nanos}-{seq}", std::process::id()))
}

/// The functional baseline tree, relative to the workspace root. It holds the
/// `functional/` and `showcase/` captures of the Showcase, TablePro and Holla
/// journeys and the frozen `pre_refactor/` oracle that those journeys, Jackin
/// and the cross-binary cases check against. Every lookup goes through
/// [`oracle_baselines`] and [`baseline_root`], so relocating the tree is a
/// change to this constant and a `git mv`.
const BASELINES: &str = "tests/functional/baselines";

/// The directory that holds the `pre_refactor/` capture directories.
pub fn oracle_baselines() -> PathBuf {
    workspace_root().join(BASELINES)
}

/// The baseline directory for `test_id`: the shared tree for `pre_refactor/`
/// ids, else the `tests/functional/baselines` of the target's own package.
pub fn baseline_root(test_id: &str) -> PathBuf {
    if test_id.starts_with("pre_refactor/") {
        oracle_baselines()
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines")
    }
}

pub fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest.join("Cargo.toml").is_file() && manifest.join("apps/jackin/Cargo.toml").is_file() {
        return manifest;
    }
    let workspace = manifest.join("../..");
    if manifest.join("Cargo.toml").is_file()
        && manifest.join("src").is_dir()
        && workspace.join("Cargo.toml").is_file()
        && workspace.join("apps/jackin/Cargo.toml").is_file()
    {
        return workspace.canonicalize().expect("app workspace root");
    }
    panic!(
        "CARGO_MANIFEST_DIR is neither the workspace root nor apps/jackin: {}",
        manifest.display()
    );
}

fn child_args(launch: &Launch) -> Vec<String> {
    let mut args = launch.args.clone();
    if let Some(color) = launch.color.as_deref()
        && color != "nocolor"
    {
        args.push("--color".to_string());
        args.push(color.to_string());
    }
    if let Some(motion) = &launch.motion {
        args.push("--motion".to_string());
        args.push(motion.clone());
    }
    args
}

fn prepared_launch(launch: &Launch, home: &Path) -> LaunchOptions {
    let mut options = LaunchOptions::new(launch.bin.as_os_str())
        .with_args(child_args(launch))
        .with_cwd(home)
        .with_env_mode(LaunchEnvMode::Cleared);
    for (key, value) in effective_launch_env(launch, home) {
        options = options.with_env(&key, &value);
    }
    options
}

fn write_binary_record(directory: &Path, path: &Path, sha256: &str) {
    let record = serde_json::json!({
        "path": path,
        "sha256": sha256,
    });
    std::fs::write(
        directory.join("binary.json"),
        serde_json::to_vec_pretty(&record).expect("binary record"),
    )
    .expect("write binary.json");
}

struct Manifest {
    reference_sha: String,
    lockfile_sha256: String,
    comparator_name: String,
    comparator_rev: String,
    session_file: String,
    binaries: Vec<(String, String)>,
    binary_provenance: Vec<(String, String, String)>,
}

impl Manifest {
    fn provenance(&self, sha256: &str) -> Option<(&str, &str)> {
        self.binary_provenance
            .iter()
            .find(|(digest, _, _)| digest == sha256)
            .map(|(_, source, lockfile)| (source.as_str(), lockfile.as_str()))
    }
}

fn manifest_path() -> PathBuf {
    workspace_root().join("docs/testing/reference-manifest.json")
}

fn required_str(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("reference manifest field {key}"))
        .to_string()
}

fn push_digest(binaries: &mut Vec<(String, String)>, name: &str, sha: &str) {
    if binaries
        .iter()
        .any(|(entry, digest)| entry == name && digest == sha)
    {
        return;
    }
    binaries.push((name.to_string(), sha.to_string()));
}

pub fn host_target_triple() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        (os, arch) => panic!("no reference binary for {os} {arch}"),
    }
}

pub fn reference_binary(name: &str) -> PathBuf {
    reference_binary_from(&manifest_path(), name)
}

pub fn reference_binary_from(manifest: &Path, name: &str) -> PathBuf {
    let value = manifest_value(manifest);
    let host = host_target_triple();
    let path = value["binaries"][name]["platforms"][host]["path"]
        .as_str()
        .unwrap_or_else(|| panic!("reference path for {name} on {host}"));
    workspace_root().join(path)
}

pub fn reference_digest(name: &str) -> String {
    reference_digest_from(&manifest_path(), name)
}

pub fn reference_digest_from(manifest: &Path, name: &str) -> String {
    let value = manifest_value(manifest);
    let host = host_target_triple();
    value["binaries"][name]["platforms"][host]["sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("reference sha256 for {name} on {host}"))
        .to_string()
}

fn manifest_value(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read reference manifest {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("parse reference manifest {}: {error}", path.display()))
}

fn read_manifest(path: &Path) -> Manifest {
    let value = manifest_value(path);
    let reference_sha = required_str(&value, "reference_sha");
    let lockfile_sha256 = required_str(&value, "lockfile_sha256");
    let session_file = required_str(&value["acceptance"], "session_file");
    assert_eq!(
        session_file, "session.json",
        "reference manifest session file"
    );
    let binaries_value = value["binaries"]
        .as_object()
        .expect("reference manifest binaries");
    let mut binaries = Vec::with_capacity(binaries_value.len());
    let mut binary_provenance = Vec::new();
    for (name, entry) in binaries_value {
        let sha = entry["sha256"]
            .as_str()
            .unwrap_or_else(|| panic!("reference manifest sha256 for {name}"));
        let identity_name = entry
            .get("identity_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(name);
        let source = entry
            .get("reference_sha")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&reference_sha);
        let lockfile = entry
            .get("lockfile_sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&lockfile_sha256);
        push_digest(&mut binaries, identity_name, sha);
        binary_provenance.push((sha.to_owned(), source.to_owned(), lockfile.to_owned()));
        let Some(platforms) = entry
            .get("platforms")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for (target, platform) in platforms {
            let platform_sha = platform
                .get("sha256")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("reference manifest sha256 for {name} {target}"));
            let platform_source = platform
                .get("reference_sha")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(source);
            let platform_lockfile = platform
                .get("lockfile_sha256")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(lockfile);
            push_digest(&mut binaries, identity_name, platform_sha);
            binary_provenance.push((
                platform_sha.to_owned(),
                platform_source.to_owned(),
                platform_lockfile.to_owned(),
            ));
        }
    }
    Manifest {
        reference_sha,
        lockfile_sha256,
        comparator_name: required_str(&value["comparator"], "name"),
        comparator_rev: required_str(&value["comparator"], "rev"),
        session_file,
        binaries,
        binary_provenance,
    }
}

fn resolve_bin(bin: &Path) -> PathBuf {
    let path = if bin.is_absolute() {
        bin.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory")
            .join(bin)
    };
    path.canonicalize().unwrap_or(path)
}

fn identity_refusal(
    launch: &Launch,
    binary: &Path,
    sha256: Option<&str>,
    manifest: &Manifest,
) -> Option<String> {
    match launch.mode {
        SessionMode::Candidate => {
            let sha256 = sha256?;
            let matched = manifest.binaries.iter().any(|(_, digest)| digest == sha256);
            if matched {
                Some(format!(
                    "reference binary refused in candidate mode: {} sha256 {sha256}",
                    binary.display()
                ))
            } else {
                None
            }
        }
        SessionMode::Reference => {
            let name = binary
                .file_name()
                .and_then(|file| file.to_str())
                .unwrap_or("");
            let matched = sha256.is_some_and(|actual| {
                manifest
                    .binaries
                    .iter()
                    .any(|(entry, digest)| entry == name && digest == actual)
            });
            if matched {
                None
            } else {
                Some(format!(
                    "reference identity mismatch for {name}: {}",
                    binary.display()
                ))
            }
        }
    }
}

fn refuse_launch(directory: PathBuf, message: String) -> Result<App, LaunchError> {
    std::fs::write(directory.join("spawn-error.txt"), &message).expect("write spawn-error.txt");
    Err(LaunchError {
        evidence: directory,
        message,
    })
}

fn write_session(
    directory: &Path,
    launch: &Launch,
    binary: &Path,
    sha256: &str,
    manifest: &Manifest,
) {
    let (reference_sha, lockfile_sha256) = manifest
        .provenance(sha256)
        .unwrap_or((&manifest.reference_sha, &manifest.lockfile_sha256));
    let record = serde_json::json!({
        "mode": launch.mode.as_str(),
        "reference_sha": reference_sha,
        "lockfile_sha256": lockfile_sha256,
        "binary_path": binary,
        "binary_sha256": sha256,
        "args": launch.args,
        "cols": launch.cols,
        "rows": launch.rows,
        "color": launch.color,
        "platform": launch.platform,
        "history": launch.history,
        "motion": launch.motion,
        "clock": launch.clock,
        "TERM": launch.term,
        "env": effective_launch_env(launch, &directory.join("home")),
        "comparator_name": manifest.comparator_name,
        "comparator_rev": manifest.comparator_rev,
    });
    let mut bytes = serde_json::to_vec_pretty(&record).expect("session record");
    bytes.push(b'\n');
    let path = directory.join(&manifest.session_file);
    std::fs::write(&path, bytes)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn effective_launch_env(launch: &Launch, home: &Path) -> BTreeMap<String, String> {
    let home_text = env_path(home);
    let config_home = env_path(&home.join("config"));
    let cache_home = env_path(&home.join("cache"));
    let data_home = env_path(&home.join("data"));
    let mut environment = BTreeMap::from([
        ("HOME".to_string(), home_text),
        ("XDG_CONFIG_HOME".to_string(), config_home),
        ("XDG_CACHE_HOME".to_string(), cache_home),
        ("XDG_DATA_HOME".to_string(), data_home),
        ("SHELL".to_string(), "/bin/sh".to_string()),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("TERM".to_string(), launch.term.clone()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
        ("LANG".to_string(), "C.UTF-8".to_string()),
        ("LC_ALL".to_string(), "C.UTF-8".to_string()),
        ("TZ".to_string(), "UTC".to_string()),
        (
            "JUNIE_TUI_CLIENT_PLATFORM".to_string(),
            launch.platform.clone(),
        ),
    ]);
    if launch.clock {
        environment.insert("JUNIE_TUI_TEST_CLOCK".to_string(), "1".to_string());
    }
    if fenced(launch) {
        environment.insert(TestRequest::FENCE_ENV.to_string(), "1".to_string());
    }
    if let Some(lag) = draw_lag_ms() {
        environment.insert(DRAW_LAG_ENV.to_string(), lag.to_string());
    }
    if !launch.history {
        environment.insert("HOLLA_NO_HISTORY".to_string(), "1".to_string());
    }
    if launch.color.as_deref() == Some("nocolor") {
        environment.insert("NO_COLOR".to_string(), "1".to_string());
    }
    for (key, value) in &launch.env {
        environment.insert(key.clone(), value.clone());
    }
    environment
}

/// The app's draw-lag knob (`src/runtime.rs`). The harness forwards the value
/// it is run with, so `JUNIE_TUI_DRAW_LAG_MS=300 python3 scripts/run_functional.py`
/// holds every frame of every candidate back by 300 ms. Only whole
/// milliseconds pass, so no other text reaches an application environment.
const DRAW_LAG_ENV: &str = "JUNIE_TUI_DRAW_LAG_MS";

/// Run with `TERMROCK_TEST_LEGACY_WAITS=1`, [`App::settle`] and
/// [`App::flash_expired`] wait the way they did before the fence: a quiet
/// interval and a fixed 200 ms sleep. It exists to prove, under
/// [`DRAW_LAG_ENV`], that a case fails on the old wait and passes on the new.
const LEGACY_WAITS_ENV: &str = "TERMROCK_TEST_LEGACY_WAITS";

fn draw_lag_ms() -> Option<u64> {
    std::env::var(DRAW_LAG_ENV).ok()?.parse().ok()
}

fn legacy_waits() -> bool {
    std::env::var_os(LEGACY_WAITS_ENV).is_some_and(|value| value == "1")
}

fn env_path(path: &Path) -> String {
    path.to_str()
        .expect("functional test environment paths must be valid UTF-8")
        .to_owned()
}

#[cfg(all(test, unix))]
mod env_path_tests {
    use super::env_path;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::Path};

    #[test]
    #[should_panic(expected = "functional test environment paths must be valid UTF-8")]
    fn rejects_non_utf8_environment_paths_instead_of_rewriting_them() {
        let path = Path::new(OsString::from_vec(vec![b'/', 0xff]).as_os_str()).to_path_buf();
        let _ = env_path(&path);
    }
}
