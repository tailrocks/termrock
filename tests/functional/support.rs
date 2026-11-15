use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use junie_tui::clock::TestRequest;
use junie_tui::core::pointer::PointerConfig;
use junie_tui::shell::StatusLine;
use tui_test::{
    CheckedMouseAction, KeyAction, LaunchEnvMode, LaunchOptions, MouseOptions, OpenOptions,
    Operation, OperationResult, RunOptions, Session, SnapshotCell, SnapshotScope, TerminalSnapshot,
    WaitQuietOptions,
};

pub const SHOWCASE: &str = match option_env!("CARGO_BIN_EXE_showcase") {
    Some(path) => path,
    None => "",
};
pub const TABLEPRO: &str = match option_env!("CARGO_BIN_EXE_tablepro") {
    Some(path) => path,
    None => "",
};
pub const HOLLA: &str = match option_env!("CARGO_BIN_EXE_holla") {
    Some(path) => path,
    None => "",
};
const TIMEOUT: Duration = Duration::from_secs(10);

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
    pub fn new(name: &str, bin: &str) -> Self {
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

pub struct App {
    session: Session,
    directory: PathBuf,
    actions: RefCell<Vec<String>>,
    checkpoint: Cell<usize>,
    clock: bool,
}

impl App {
    pub fn new(name: &str, bin: &str, args: &[&str], cols: u16, rows: u16, color: &str) -> Self {
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

    pub fn wait_absent(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("absent {needle}"), |screen| {
            !text(screen).contains(needle)
        })
    }

    pub fn settle(&self) -> TerminalSnapshot {
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

    pub fn resize(&self, cols: u16, rows: u16) {
        self.log(format!("resize {cols}x{rows}"));
        self.session
            .execute(Operation::Resize { cols, rows })
            .expect("resize PTY");
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
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.child_running() {
            assert!(
                Instant::now() < deadline,
                "child survived Session::kill {}",
                self.directory.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
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
        std::fs::write(
            path.with_extension("json"),
            serde_json::to_vec(screen).unwrap(),
        )
        .expect("save terminal cells");
        std::fs::write(path.with_extension("txt"), text(screen)).expect("save terminal text");
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
            eprintln!("functional evidence: {}", self.directory.display());
        }
    }
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
                clock: launch.clock,
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

fn baseline_root(test_id: &str) -> PathBuf {
    if test_id.starts_with("pre_refactor/") {
        workspace_root().join("tests/functional/baselines")
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
