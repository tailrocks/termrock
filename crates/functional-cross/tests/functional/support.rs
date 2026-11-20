#[path = "../../../../crates/termrock-functional/tests/functional/support.rs"]
mod shared;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tui_test::{SnapshotColor, TerminalSnapshot};

pub use shared::*;

pub fn showcase_bin() -> &'static str {
    app_binary("showcase")
}

pub fn tablepro_bin() -> &'static str {
    app_binary("tablepro")
}

pub fn holla_bin() -> &'static str {
    app_binary("holla")
}

/// Path of a sibling app binary.
///
/// `TCB_BIN_<NAME>` wins, which is how a nextest setup script or a developer
/// points at a specific build. Otherwise the binary is the sibling of the
/// `deps/` directory this test executable lives in, which holds for an
/// in-place run and for a nextest archive extracted with its target dir.
fn app_binary(name: &str) -> &'static str {
    static CACHE: OnceLock<Mutex<BTreeMap<String, &'static str>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(path) = cache.get(name) {
        return path;
    }
    let variable = format!("TCB_BIN_{}", name.to_ascii_uppercase());
    let path = match std::env::var_os(&variable) {
        Some(value) => PathBuf::from(value),
        None => std::env::current_exe()
            .expect("current test executable")
            .parent()
            .and_then(|deps| deps.parent())
            .expect("test executable sits in <target>/<profile>/deps")
            .join(name),
    };
    assert!(
        path.is_file(),
        "app binary `{name}` not found at {}. Build it first (`cargo build --workspace --bins`) or set {variable}.",
        path.display()
    );
    let leaked: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
    cache.insert(name.to_string(), leaked);
    leaked
}

/// Launch one app binary at 120 by 40 with truecolor and reduced motion, a
/// wall clock and no extra environment, then change what a case needs.
pub struct Open<'a> {
    name: &'a str,
    bin: &'a str,
    args: &'a [&'a str],
    cols: u16,
    rows: u16,
    color: Option<&'a str>,
    motion: Option<&'a str>,
    clock: bool,
    env: &'a [(&'a str, &'a str)],
}

impl<'a> Open<'a> {
    pub fn new(name: &'a str, bin: &'a str, args: &'a [&'a str]) -> Self {
        Self {
            name,
            bin,
            args,
            cols: 120,
            rows: 40,
            color: Some("truecolor"),
            motion: Some("reduced"),
            clock: false,
            env: &[],
        }
    }

    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }

    pub fn color(mut self, color: Option<&'a str>) -> Self {
        self.color = color;
        self
    }

    pub fn motion(mut self, motion: Option<&'a str>) -> Self {
        self.motion = motion;
        self
    }

    pub fn clock(mut self, clock: bool) -> Self {
        self.clock = clock;
        self
    }

    pub fn env(mut self, env: &'a [(&'a str, &'a str)]) -> Self {
        self.env = env;
        self
    }

    pub fn launch(self) -> App {
        let mut launch = Launch::new(self.name, self.bin);
        launch.clock = self.clock;
        launch.cols = self.cols;
        launch.rows = self.rows;
        launch.color = self.color.map(str::to_string);
        launch.motion = self.motion.map(str::to_string);
        launch.args = self.args.iter().copied().map(str::to_string).collect();
        launch.env = self
            .env
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
    }
}

/// The default launch: 120 by 40, truecolor, reduced motion, wall clock.
pub fn open(name: &str, bin: &str, args: &[&str]) -> App {
    Open::new(name, bin, args).launch()
}

pub fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

pub fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

pub fn kill(app: &App) {
    assert!(app.child_running(), "child exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "child still running");
}

pub const BOLD: u8 = 1;
pub const DIM: u8 = 2;
pub const INVERSE: u8 = 4;
pub const UNDER: u8 = 8;

/// The BOLD, DIM, INVERSE and UNDER bits of the cell at `point`.
pub fn mods_at(screen: &TerminalSnapshot, point: Point) -> u8 {
    use tui_test::SnapshotUnderline;
    use tui_test::terminal::cell::Attrs;
    let cell = screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell");
    let mut mods = 0;
    for (attr, bit) in [
        (Attrs::BOLD, BOLD),
        (Attrs::DIM, DIM),
        (Attrs::INVERSE, INVERSE),
    ] {
        if cell.has(attr) {
            mods |= bit;
        }
    }
    if cell.underline() != SnapshotUnderline::None {
        mods |= UNDER;
    }
    mods
}

pub fn rgb_cells(screen: &TerminalSnapshot) -> usize {
    let mut count = 0;
    for row in 0..screen.rows() {
        for cell in screen.row(row).expect("row") {
            if matches!(cell.fg(), SnapshotColor::Rgb(_, _, _))
                || matches!(cell.bg(), SnapshotColor::Rgb(_, _, _))
            {
                count += 1;
            }
        }
    }
    count
}

pub fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle}\n{}", text(screen)))
}
