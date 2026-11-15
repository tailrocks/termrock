use std::collections::BTreeSet;

use crate::domain::context::Os;
use crate::domain::manifest::{Json, json_i64, json_str, json_u64, parse_json};
use crate::sim::fs::{Fs, NodeKind, RemovalStats};

pub const CACHE_SCHEMA: u32 = 3;
pub const CACHE_TTL_SECS: i64 = 7 * 86_400;
pub const CACHE_DEPTH: usize = 2;
pub const REPORT_DETAIL: usize = 6;
pub const ARTIFACT_DEPTH: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Safety {
    Rebuildable,

    OldOnly,

    ReviewFirst,
}

impl Safety {
    pub fn label(self) -> &'static str {
        match self {
            Safety::Rebuildable => "rebuilt on demand",
            Safety::OldOnly => "safe if old",
            Safety::ReviewFirst => "review first",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detect {
    Dir,

    Tool(&'static str),

    ToolOrDir(&'static str),
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Category {
    pub id: &'static str,
    pub label: &'static str,
    pub safety: Safety,

    pub min_age_days: i64,
    pub detect: Detect,

    pub roots: &'static [&'static str],
    pub macos_only: bool,

    pub children_of_root: bool,

    pub guard_process: Option<&'static str>,
    pub note: &'static str,
}

pub const CATEGORIES: [Category; 18] = [
    Category {
        id: "xcode.derived-data",
        label: "Xcode DerivedData",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Dir,
        roots: &["Library/Developer/Xcode/DerivedData"],
        macos_only: true,
        children_of_root: false,
        guard_process: Some("Xcode"),
        note: "rebuilt by the next build",
    },
    Category {
        id: "xcode.device-support",
        label: "Xcode device support",
        safety: Safety::OldOnly,
        min_age_days: 90,
        detect: Detect::Dir,
        roots: &[
            "Library/Developer/Xcode/iOS DeviceSupport",
            "Library/Developer/Xcode/watchOS DeviceSupport",
            "Library/Developer/Xcode/tvOS DeviceSupport",
        ],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "re-downloaded when a device of that version connects",
    },
    Category {
        id: "xcode.archives",
        label: "Xcode archives",
        safety: Safety::ReviewFirst,
        min_age_days: 0,
        detect: Detect::Dir,
        roots: &["Library/Developer/Xcode/Archives"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "signed archives may be needed for symbolication or redistribution",
    },
    Category {
        id: "simulator.caches",
        label: "Simulator caches",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Dir,
        roots: &["Library/Developer/CoreSimulator/Caches"],
        macos_only: true,
        children_of_root: false,
        guard_process: Some("Simulator"),
        note: "recreated by the Simulator",
    },
    Category {
        id: "brew.cache",
        label: "Homebrew cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Tool("brew"),
        roots: &["Library/Caches/Homebrew"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "downloads are fetched again on install",
    },
    Category {
        id: "npm.cache",
        label: "npm cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Tool("npm"),
        roots: &[".npm/_cacache", ".npm/_logs"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "packages are fetched again on install",
    },
    Category {
        id: "pnpm.store",
        label: "pnpm store",
        safety: Safety::OldOnly,
        min_age_days: 30,
        detect: Detect::Tool("pnpm"),
        roots: &["Library/pnpm/store"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "content-addressable store · resolved through pnpm store path",
    },
    Category {
        id: "yarn.cache",
        label: "Yarn cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Tool("yarn"),
        roots: &[".yarn/cache", "Library/Caches/Yarn"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "packages are fetched again on install",
    },
    Category {
        id: "bun.cache",
        label: "Bun cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Tool("bun"),
        roots: &[".bun/install/cache"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "packages are fetched again on install",
    },
    Category {
        id: "cargo.registry-cache",
        label: "Cargo registry cache",
        safety: Safety::OldOnly,
        min_age_days: 30,
        detect: Detect::Tool("cargo"),
        roots: &[".cargo/registry/cache", ".cargo/git"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "crates are downloaded again on build",
    },
    Category {
        id: "gradle.caches",
        label: "Gradle caches",
        safety: Safety::OldOnly,
        min_age_days: 30,
        detect: Detect::Dir,
        roots: &[".gradle/caches", ".gradle/daemon", ".gradle/wrapper/dists"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "gradle --stop runs first · dependencies are downloaded again",
    },
    Category {
        id: "maven.repository",
        label: "Maven repository",
        safety: Safety::ReviewFirst,
        min_age_days: 0,
        detect: Detect::Dir,
        roots: &[".m2/repository"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "snapshots and offline builds may need it",
    },
    Category {
        id: "pip.cache",
        label: "pip cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::ToolOrDir("pip3"),
        roots: &["Library/Caches/pip"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "wheels are downloaded again on install",
    },
    Category {
        id: "uv.cache",
        label: "uv cache",
        safety: Safety::Rebuildable,
        min_age_days: 0,
        detect: Detect::Tool("uv"),
        roots: &[".cache/uv"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "$XDG_CACHE_HOME/uv or ~/.cache/uv",
    },
    Category {
        id: "user.caches",
        label: "User caches",
        safety: Safety::ReviewFirst,
        min_age_days: 30,
        detect: Detect::Always,
        roots: &["Library/Caches"],
        macos_only: true,
        children_of_root: true,
        guard_process: None,
        note: "each application cache is reviewed on its own · the root stays",
    },
    Category {
        id: "user.logs",
        label: "User logs",
        safety: Safety::Rebuildable,
        min_age_days: 7,
        detect: Detect::Always,
        roots: &["Library/Logs"],
        macos_only: true,
        children_of_root: true,
        guard_process: None,
        note: "older than a week · the root stays",
    },
    Category {
        id: "ide.jetbrains-logs",
        label: "JetBrains logs",
        safety: Safety::Rebuildable,
        min_age_days: 7,
        detect: Detect::Dir,
        roots: &["Library/Logs/JetBrains"],
        macos_only: true,
        children_of_root: false,
        guard_process: None,
        note: "older than a week",
    },
    Category {
        id: "project.artifacts",
        label: "Project artifacts",
        safety: Safety::ReviewFirst,
        min_age_days: 7,
        detect: Detect::Always,
        roots: &["Projects"],
        macos_only: false,
        children_of_root: false,
        guard_process: None,
        note: "generated directories beside a project indicator · rebuilt by the project's own tooling",
    },
];

pub fn category(id: &str) -> Option<&'static Category> {
    CATEGORIES.iter().find(|c| c.id == id)
}

pub const ARTIFACT_NAMES: [&str; 12] = [
    "node_modules",
    "target",
    "build",
    "dist",
    ".venv",
    "venv",
    ".next",
    ".turbo",
    ".gradle",
    "DerivedData",
    "Pods",
    "__pycache__",
];

pub const INDICATORS: [&str; 8] = [
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    ".git",
];

pub const NOISE_NAMES: [&str; 14] = [
    "node_modules",
    ".git",
    "target",
    "build",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    "DerivedData",
    "Pods",
    ".gradle",
    ".next",
    ".turbo",
    ".cache",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessObservation {
    Running,
    NotRunning,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eligibility {
    Preselected,

    Selectable,

    Ineligible(String),
}

impl Eligibility {
    pub fn selectable(&self) -> bool {
        !matches!(self, Eligibility::Ineligible(_))
    }
}

pub fn age_days(mtime: i64, now_secs: i64) -> Option<i64> {
    if mtime > now_secs {
        return None;
    }
    Some((now_secs - mtime) / 86_400)
}

pub fn eligibility(cat: &Category, age: Option<i64>, process: &ProcessObservation) -> Eligibility {
    if let Some(name) = cat.guard_process {
        match process {
            ProcessObservation::Running => {
                return Eligibility::Ineligible(format!("{name} is running"));
            }
            ProcessObservation::Unknown(why) => {
                return Eligibility::Ineligible(format!("{name} state unknown · {why}"));
            }
            ProcessObservation::NotRunning => {}
        }
    }
    if cat.min_age_days > 0 {
        match age {
            None => return Eligibility::Ineligible("age unknown · treated as recent".into()),
            Some(d) if d < cat.min_age_days => {
                return Eligibility::Ineligible(format!(
                    "too recent · {d} d < {} d",
                    cat.min_age_days
                ));
            }
            _ => {}
        }
    }
    match cat.safety {
        Safety::ReviewFirst => Eligibility::Selectable,
        Safety::Rebuildable | Safety::OldOnly => Eligibility::Preselected,
    }
}

pub fn detected(cat: &Category, os: Os, home: &str, fs: &Fs, tools: &BTreeSet<String>) -> bool {
    if cat.macos_only && os != Os::MacOs {
        return false;
    }
    let root_exists = cat.roots.iter().any(|r| fs.is_dir(&format!("{home}/{r}")));
    match cat.detect {
        Detect::Always => true,
        Detect::Dir => root_exists,
        Detect::Tool(t) => tools.contains(t),
        Detect::ToolOrDir(t) => tools.contains(t) || root_exists,
    }
}

pub fn pnpm_store_root(home: &str, resolved: Result<&str, &str>) -> (String, Option<String>) {
    let fallback = format!("{home}/Library/pnpm/store");
    let allowed = [
        format!("{home}/Library/pnpm/store"),
        format!("{home}/.local/share/pnpm/store"),
        format!("{home}/.pnpm-store"),
    ];
    match resolved {
        Ok(p)
            if allowed
                .iter()
                .any(|a| p == a || p.starts_with(&format!("{a}/"))) =>
        {
            (p.to_owned(), None)
        }
        Ok(p) => (
            fallback,
            Some(format!(
                "pnpm store path {p} is outside the known roots · using the default"
            )),
        ),
        Err(e) => (
            fallback,
            Some(format!("pnpm store path failed: {e} · using the default")),
        ),
    }
}

pub fn is_artifact(fs: &Fs, path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or("");
    if !ARTIFACT_NAMES.contains(&name) {
        return false;
    }
    let Some(parent) = Fs::parent(path) else {
        return false;
    };
    INDICATORS
        .iter()
        .any(|i| fs.exists(&format!("{parent}/{i}")))
}

pub fn find_artifacts_report(fs: &Fs, roots: &[String]) -> (Vec<String>, Vec<String>) {
    let mut uniq: Vec<String> = vec![];
    for r in roots {
        if !uniq.contains(r) && !uniq.iter().any(|u| r.starts_with(&format!("{u}/"))) {
            uniq.push(r.clone());
        }
    }
    let mut out = vec![];
    let mut unreadable = vec![];
    for r in uniq {
        walk_artifacts(fs, &r, 0, &mut out, &mut unreadable);
    }
    out.sort();
    out.dedup();
    unreadable.sort();
    unreadable.dedup();
    (out, unreadable)
}

fn walk_artifacts(
    fs: &Fs,
    dir: &str,
    depth: usize,
    out: &mut Vec<String>,
    unreadable: &mut Vec<String>,
) {
    if depth > ARTIFACT_DEPTH {
        return;
    }
    let children = match fs.list(dir) {
        Ok(c) => c,
        Err(_) => {
            unreadable.push(dir.to_owned());
            return;
        }
    };
    for c in children {
        if !c.is_dir() {
            continue;
        }
        if is_artifact(fs, &c.path) {
            out.push(c.path.clone());
            continue;
        }
        walk_artifacts(fs, &c.path, depth + 1, out, unreadable);
    }
}

pub fn walk_candidates(fs: &Fs, root: &str, select: &dyn Fn(&str, bool) -> bool) -> Vec<String> {
    walk_candidates_report(fs, root, select).0
}

pub fn walk_candidates_report(
    fs: &Fs,
    root: &str,
    select: &dyn Fn(&str, bool) -> bool,
) -> (Vec<String>, Vec<String>) {
    crate::domain::projection::record(|w| w.candidate_scans += 1);
    let mut out = vec![];
    let mut unreadable = vec![];
    walk_sel(fs, root, 0, select, &mut out, &mut unreadable);
    out.sort();
    out.dedup();
    unreadable.sort();
    unreadable.dedup();
    (out, unreadable)
}

fn walk_sel(
    fs: &Fs,
    dir: &str,
    depth: usize,
    select: &dyn Fn(&str, bool) -> bool,
    out: &mut Vec<String>,
    unreadable: &mut Vec<String>,
) {
    if depth > 5 {
        return;
    }
    let children = match fs.list(dir) {
        Ok(c) => c,
        Err(_) => {
            unreadable.push(dir.to_owned());
            return;
        }
    };
    for c in children {
        if matches!(c.kind, NodeKind::Symlink { .. }) {
            continue;
        }
        if c.name() == "node_modules" {
            continue;
        }
        if select(&c.path, c.is_dir()) {
            out.push(c.path.clone());
            continue;
        }
        if c.is_dir() {
            if c.readable {
                walk_sel(fs, &c.path, depth + 1, select, out, unreadable);
            } else {
                unreadable.push(c.path.clone());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deny(pub String);

fn lexical(path: &str) -> Result<Vec<&str>, Deny> {
    if !path.starts_with('/') {
        return Err(Deny("path must be absolute".into()));
    }
    if path == "/" {
        return Err(Deny("the filesystem root is protected".into()));
    }
    if path.ends_with('/') {
        return Err(Deny("trailing slash".into()));
    }
    let comps: Vec<&str> = path[1..].split('/').collect();
    for c in &comps {
        if c.is_empty() {
            return Err(Deny("empty path component (repeated slash)".into()));
        }
        if *c == ".." || *c == "." {
            return Err(Deny(format!("relative component `{c}`")));
        }
    }
    Ok(comps)
}

const PROTECTED_SUBTREES: [&str; 8] = [
    "/bin",
    "/sbin",
    "/etc",
    "/private/etc",
    "/System",
    "/var/db",
    "/private/var/db",
    "/usr",
];
const PROTECTED_EXACT: [&str; 5] = ["/usr/local", "/Library", "/Applications", "/Users", "/home"];

fn under(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

pub fn system_rule(path: &str) -> Option<Deny> {
    for s in PROTECTED_SUBTREES {
        if under(path, s) {
            if s == "/usr" && path.starts_with("/usr/local/") {
                return None;
            }
            return Some(Deny(format!("{s} is protected")));
        }
    }
    if PROTECTED_EXACT.contains(&path) {
        return Some(Deny(format!("{path} itself is protected")));
    }
    None
}

pub fn user_rule(path: &str, home: &str) -> Option<Deny> {
    if path == home {
        return Some(Deny("your home directory is protected".into()));
    }
    let rel = path.strip_prefix(&format!("{home}/"))?;
    let lower = rel.to_lowercase();
    let first = rel.split('/').next().unwrap_or("");
    if first == ".Trash" {
        return Some(Deny("the Trash is managed by the system".into()));
    }
    if rel == "Library/Caches" || rel == "Library/Logs" {
        return Some(Deny(format!(
            "~/{rel} itself is protected · its children may be cleaned"
        )));
    }
    if let Some(lib) = rel.strip_prefix("Library/") {
        let seg = lib.split('/').next().unwrap_or("");
        if seg.starts_with("Mobile Documents") {
            return Some(Deny("iCloud Drive data is never a cleanup target".into()));
        }
        if seg == "Keychains" || seg == "Application Support" || seg == "Safari" || seg == "WebKit"
        {
            return Some(Deny(format!("~/Library/{seg} holds user data")));
        }
        if seg == "Containers" || seg == "Group Containers" {
            let sub = lower.split('/').nth(2).unwrap_or("");
            if sub.contains("safari")
                || sub.contains("webkit")
                || sub.contains("docker")
                || sub.contains("chrome")
                || sub.contains("firefox")
            {
                return Some(Deny(format!(
                    "{seg} for a browser or Docker holds user data"
                )));
            }
        }
    }
    None
}

pub fn validate(path: &str, home: &str, os: Os, fs: &Fs) -> Result<String, Deny> {
    lexical(path)?;
    if let Some(d) = system_rule(path) {
        return Err(d);
    }
    if let Some(d) = user_rule(path, home) {
        return Err(d);
    }

    let (parent, links) = match fs.canonical_parent(path) {
        Ok(v) => v,
        Err(crate::sim::fs::FsError::NotFound(_)) => {
            (Fs::parent(path).unwrap_or("/".into()), vec![])
        }
        Err(e) => return Err(Deny(e.message())),
    };
    for l in &links {
        let alias = os == Os::MacOs && (l == "/tmp" || l == "/var");
        if !alias {
            return Err(Deny(format!("{l} is a symbolic link on the path")));
        }
    }
    let canonical = Fs::join(&parent, path.rsplit('/').next().unwrap_or(""));
    if let Some(d) = system_rule(&canonical) {
        return Err(d);
    }
    if let Some(d) = user_rule(&canonical, home) {
        return Err(d);
    }
    Ok(canonical)
}

pub fn protected_descendant(path: &str, home: &str, fs: &Fs) -> Option<String> {
    fs.subtree(path)
        .iter()
        .find(|n| user_rule(&n.path, home).is_some() || system_rule(&n.path).is_some())
        .map(|n| n.path.clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Trash,
    Permanent,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Trash => "trash",
            Mode::Permanent => "permanent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteItem {
    pub path: String,
    pub category: String,

    pub estimate: u64,
    pub guard: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePlan {
    pub host: String,
    pub root: String,
    pub items: Vec<DeleteItem>,
    pub mode: Mode,
    pub dry_run: bool,
    pub revision: u64,
}

impl DeletePlan {
    pub fn new(host: &str, root: &str, items: Vec<DeleteItem>, mode: Mode, dry_run: bool) -> Self {
        let mut p = Self {
            host: host.into(),
            root: root.into(),
            items,
            mode,
            dry_run,
            revision: 0,
        };
        p.revision = p.fingerprint();
        p
    }

    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |s: &str| {
            for b in s.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h ^= 0xff;
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        feed(&self.host);
        feed(&self.root);
        feed(self.mode.label());
        feed(if self.dry_run { "dry" } else { "live" });
        for i in &self.items {
            feed(&i.path);
            feed(&i.category);
        }
        h
    }

    pub fn estimate(&self) -> u64 {
        self.items.iter().map(|i| i.estimate).sum()
    }

    pub fn phrase(&self) -> String {
        format!(
            "{} {} UNDER {} ON {}",
            if self.mode == Mode::Permanent {
                "PERMANENTLY DELETE"
            } else {
                "TRASH"
            },
            self.items.len(),
            self.root,
            self.host
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Removed,
    Trashed,
    WouldRemove,
    Failed,
    Skipped,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Removed => "removed",
            Outcome::Trashed => "trashed",
            Outcome::WouldRemove => "would_remove",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemResult {
    pub path: String,
    pub outcome: Outcome,
    /// Processed allocation that left the tree. Two links to one inode can
    /// both count here, so this is not disk space returned.
    pub bytes: u64,
    /// Blocks returned to the volume by this item. Zero for Trash, dry runs,
    /// and any removal that left another reference to the inode alive.
    pub reclaimed_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub timestamp_ms: i64,
    pub mode: Mode,
    pub dry_run: bool,
    pub path: String,
    /// Processed allocation (the `size` key, kept for old readers).
    pub size: u64,
    /// Blocks actually returned to the volume. `None` marks a record written
    /// before this field existed; it is unknown, never inferred from `size`.
    pub reclaimed_size: Option<u64>,
    pub outcome: Outcome,
    pub error: Option<String>,
}

impl LogRecord {
    pub fn json(&self) -> String {
        let mut line = serde_json::json!({
            "v": 1,
            "timestamp_ms": self.timestamp_ms,
            "mode": self.mode.label(),
            "dry_run": self.dry_run,
            "path": self.path,
            "size": self.size,
            "outcome": self.outcome.label(),
        });
        if let Some(reclaimed) = self.reclaimed_size {
            line["reclaimed_size"] = Json::from(reclaimed);
        }
        if let Some(error) = &self.error {
            line["error"] = Json::String(error.clone());
        }
        line.to_string()
    }

    pub fn parse(line: &str) -> Option<Self> {
        let value = parse_json(line).ok()?;
        if value.get("v")?.as_u64()? != 1 {
            return None;
        }
        let mode = match value.get("mode")?.as_str()? {
            "trash" => Mode::Trash,
            "permanent" => Mode::Permanent,
            _ => return None,
        };
        let outcome = match value.get("outcome")?.as_str()? {
            "removed" => Outcome::Removed,
            "trashed" => Outcome::Trashed,
            "would_remove" => Outcome::WouldRemove,
            "failed" => Outcome::Failed,
            "skipped" => Outcome::Skipped,
            _ => return None,
        };
        let error = match value.get("error") {
            None | Some(Json::Null) => None,
            Some(Json::String(e)) => Some(e.clone()),
            Some(_) => return None,
        };
        Some(LogRecord {
            timestamp_ms: value.get("timestamp_ms")?.as_i64()?,
            mode,
            dry_run: value.get("dry_run")?.as_bool()?,
            path: value.get("path")?.as_str()?.to_owned(),
            size: value.get("size")?.as_u64()?,
            reclaimed_size: match value.get("reclaimed_size") {
                None | Some(Json::Null) => None,
                Some(v) => Some(v.as_u64()?),
            },
            outcome,
            error,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpsLog {
    pub path: String,
    pub lines: Vec<String>,

    pub write_failure: Option<String>,
    pub failures: u32,
}

impl OpsLog {
    pub fn append(&mut self, r: &LogRecord) {
        if let Some(_e) = &self.write_failure {
            self.failures += 1;
            return;
        }
        self.lines.push(r.json());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub plan_revision: u64,
    pub mode: Mode,
    pub dry_run: bool,
    pub items: Vec<ItemResult>,
    pub log_failures: u32,
    pub log_path: String,

    /// Processed allocation across all items, including Trash moves.
    pub bytes: u64,

    /// Blocks returned to the volume by permanent removals, counted once per
    /// inode. Only this value may reduce the volume's used space.
    pub freed_now: u64,
}

impl Report {
    pub fn count(&self, o: Outcome) -> usize {
        self.items.iter().filter(|i| i.outcome == o).count()
    }

    pub fn details(&self) -> (Vec<&ItemResult>, usize) {
        let all: Vec<&ItemResult> = self
            .items
            .iter()
            .filter(|i| matches!(i.outcome, Outcome::Failed | Outcome::Skipped))
            .collect();
        let over = all.len().saturating_sub(REPORT_DETAIL);
        (all.into_iter().take(REPORT_DETAIL).collect(), over)
    }
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        for o in [
            Outcome::Trashed,
            Outcome::Removed,
            Outcome::WouldRemove,
            Outcome::Failed,
            Outcome::Skipped,
        ] {
            let n = self.count(o);
            if n > 0 {
                parts.push(format!(
                    "{n} {}",
                    match o {
                        Outcome::WouldRemove => "would be removed",
                        other => other.label(),
                    }
                ));
            }
        }
        if parts.is_empty() {
            parts.push("nothing to do".into());
        }
        if self.log_failures > 0 {
            parts.push(format!("{} log write failures", self.log_failures));
        }
        parts.join(" · ")
    }
    pub fn incomplete(&self) -> bool {
        self.count(Outcome::Failed) + self.count(Outcome::Skipped) > 0
    }
}

pub struct ExecContext<'a> {
    pub home: &'a str,
    pub os: Os,
    pub now_secs: i64,
    pub processes: &'a dyn Fn(&str) -> ProcessObservation,
}

#[derive(Debug, Clone, Copy, Default)]
struct Amounts {
    processed: u64,
    reclaimed: u64,
}

#[derive(Debug, Clone)]
pub struct Execution {
    plan: DeletePlan,
    report: Report,
    done: BTreeSet<String>,
    processes: std::collections::BTreeMap<String, ProcessObservation>,
    next: usize,
}

impl Execution {
    pub fn start(
        plan: &DeletePlan,
        expected_revision: u64,
        log_path: &str,
    ) -> Result<Self, String> {
        if plan.revision != expected_revision || plan.fingerprint() != plan.revision {
            return Err("the reviewed plan changed · confirm again".into());
        }
        Ok(Self {
            plan: plan.clone(),
            report: Report {
                plan_revision: plan.revision,
                mode: plan.mode,
                dry_run: plan.dry_run,
                items: vec![],
                log_failures: 0,
                log_path: log_path.to_owned(),
                bytes: 0,
                freed_now: 0,
            },
            done: BTreeSet::new(),
            processes: Default::default(),
            next: 0,
        })
    }

    pub fn total(&self) -> usize {
        self.plan.items.len()
    }

    pub fn completed(&self) -> usize {
        self.next
    }

    pub fn finished(&self) -> bool {
        self.next >= self.plan.items.len()
    }

    pub fn report(&self) -> &Report {
        &self.report
    }

    fn record(
        &mut self,
        path: &str,
        outcome: Outcome,
        bytes: u64,
        error: Option<String>,
        log: &mut OpsLog,
        now_secs: i64,
    ) {
        let amounts = Amounts {
            processed: bytes,
            reclaimed: 0,
        };
        self.record_amounts(path, outcome, amounts, error, log, now_secs);
    }

    fn record_amounts(
        &mut self,
        path: &str,
        outcome: Outcome,
        amounts: Amounts,
        error: Option<String>,
        log: &mut OpsLog,
        now_secs: i64,
    ) {
        log.append(&LogRecord {
            timestamp_ms: now_secs * 1000,
            mode: self.plan.mode,
            dry_run: self.plan.dry_run,
            path: path.into(),
            size: amounts.processed,
            reclaimed_size: Some(amounts.reclaimed),
            outcome,
            error: error.clone(),
        });
        self.report.items.push(ItemResult {
            path: path.into(),
            outcome,
            bytes: amounts.processed,
            reclaimed_bytes: amounts.reclaimed,
            error,
        });
    }

    pub fn step(&mut self, fs: &mut Fs, log: &mut OpsLog, cx: &ExecContext) -> bool {
        let Some(item) = self.plan.items.get(self.next).cloned() else {
            return false;
        };
        self.next += 1;
        let path = item.path.as_str();
        let now = cx.now_secs;

        if !self.done.insert(path.to_owned()) {
            self.record(
                path,
                Outcome::Skipped,
                0,
                Some("duplicate of an earlier item".into()),
                log,
                now,
            );
            return true;
        }
        let ancestor = self
            .plan
            .items
            .iter()
            .map(|i| i.path.as_str())
            .find(|a| *a != path && path.starts_with(&format!("{a}/")))
            .map(str::to_owned);
        if let Some(anc) = ancestor {
            self.record(
                path,
                Outcome::Skipped,
                0,
                Some(format!("covered by its ancestor {anc}")),
                log,
                now,
            );
            return true;
        }

        if let Some(g) = item.guard {
            let obs = self
                .processes
                .entry(g.to_owned())
                .or_insert_with(|| (cx.processes)(g))
                .clone();
            match obs {
                ProcessObservation::Running => {
                    self.record(
                        path,
                        Outcome::Skipped,
                        0,
                        Some(format!("{g} is running")),
                        log,
                        now,
                    );
                    return true;
                }
                ProcessObservation::Unknown(why) => {
                    self.record(
                        path,
                        Outcome::Skipped,
                        0,
                        Some(format!("{g} state unknown · {why}")),
                        log,
                        now,
                    );
                    return true;
                }
                ProcessObservation::NotRunning => {}
            }
        }

        if let Err(Deny(why)) = validate(path, cx.home, cx.os, fs) {
            self.record(path, Outcome::Skipped, 0, Some(why), log, now);
            return true;
        }
        if let Some(child) = protected_descendant(path, cx.home, fs) {
            self.record(
                path,
                Outcome::Skipped,
                0,
                Some(format!("contains protected {child}")),
                log,
                now,
            );
            return true;
        }
        if !fs.exists(path) {
            self.record(
                path,
                Outcome::Failed,
                0,
                Some("no such file or directory".into()),
                log,
                now,
            );
            return true;
        }
        let scan = fs.scan(
            path,
            &crate::sim::fs::ScanOptions {
                include_hidden: true,
                ..Default::default()
            },
        );
        if scan.errors() > 0 {
            self.record(
                path,
                Outcome::Failed,
                0,
                Some("size traversal failed · unreadable entry".into()),
                log,
                now,
            );
            return true;
        }
        let bytes = scan.allocated;
        if let Err(Deny(why)) = validate(path, cx.home, cx.os, fs) {
            self.record(
                path,
                Outcome::Skipped,
                0,
                Some(format!("changed after sizing · {why}")),
                log,
                now,
            );
            return true;
        }
        if self.plan.dry_run {
            self.report.bytes = self.report.bytes.saturating_add(bytes);
            self.record(path, Outcome::WouldRemove, bytes, None, log, now);
            return true;
        }
        match self.plan.mode {
            Mode::Trash => match fs.trash(path, now) {
                Ok(stats) => {
                    self.report.bytes = self
                        .report
                        .bytes
                        .saturating_add(stats.removed_allocated_bytes);
                    self.record_removal(path, Outcome::Trashed, stats, log, now);
                }
                Err(e) => self.record(path, Outcome::Failed, 0, Some(e), log, now),
            },
            Mode::Permanent => match fs.remove_permanent(path) {
                Ok(stats) => {
                    self.report.bytes = self
                        .report
                        .bytes
                        .saturating_add(stats.removed_allocated_bytes);
                    self.report.freed_now =
                        self.report.freed_now.saturating_add(stats.reclaimed_bytes);
                    self.record_removal(path, Outcome::Removed, stats, log, now);
                }
                Err(e) => self.record(path, Outcome::Failed, 0, Some(e), log, now),
            },
        }
        true
    }

    fn record_removal(
        &mut self,
        path: &str,
        outcome: Outcome,
        stats: RemovalStats,
        log: &mut OpsLog,
        now_secs: i64,
    ) {
        let amounts = Amounts {
            processed: stats.removed_allocated_bytes,
            reclaimed: stats.reclaimed_bytes,
        };
        self.record_amounts(path, outcome, amounts, None, log, now_secs);
    }

    pub fn finish(mut self, log: &mut OpsLog) -> Report {
        self.report.log_failures = log.failures;
        log.failures = 0;
        self.report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub path: String,
    pub allocated: u64,
    pub mtime: i64,
    pub recorded_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SizeCache {
    pub entries: Vec<CacheEntry>,
    pub load_error: Option<String>,
    pub save_error: Option<String>,
}

impl SizeCache {
    pub fn hint(&self, path: &str, mtime: i64, now_secs: i64) -> Option<&CacheEntry> {
        self.entries.iter().find(|e| {
            e.path == path
                && e.mtime == mtime
                && now_secs.saturating_sub(e.recorded_secs) <= CACHE_TTL_SECS
        })
    }

    pub fn record(
        &mut self,
        root: &str,
        tree: &crate::sim::fs::ScanNode,
        snapshot: &[(String, i64)],
        fs: &Fs,
        now_secs: i64,
    ) {
        fn walk(n: &crate::sim::fs::ScanNode, depth: usize, out: &mut Vec<(String, u64)>) {
            if depth > CACHE_DEPTH {
                return;
            }
            if n.is_dir && n.error.is_none() {
                out.push((n.path.clone(), n.allocated));
            }
            for c in &n.children {
                walk(c, depth + 1, out);
            }
        }
        let mut found = vec![];
        walk(tree, 0, &mut found);
        for (p, allocated) in found {
            let Some(node) = fs.get(&p) else { continue };
            let changed = snapshot
                .iter()
                .find(|(sp, _)| *sp == p)
                .is_some_and(|(_, m)| *m != node.mtime);
            if changed {
                continue;
            }
            self.entries.retain(|e| e.path != p);
            self.entries.push(CacheEntry {
                path: p,
                allocated,
                mtime: node.mtime,
                recorded_secs: now_secs,
            });
        }
        let _ = root;
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
    }

    pub fn serialize(&self) -> String {
        let entries: Vec<Json> = self
            .entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "allocated": e.allocated,
                    "mtime": e.mtime,
                    "at": e.recorded_secs,
                })
            })
            .collect();
        serde_json::json!({ "v": CACHE_SCHEMA, "entries": entries }).to_string()
    }

    fn parse_entry(e: &Json) -> Result<CacheEntry, String> {
        Ok(CacheEntry {
            path: json_str(e, "path")?.to_owned(),
            allocated: json_u64(e, "allocated")?,
            mtime: json_i64(e, "mtime")?,
            recorded_secs: json_i64(e, "at")?,
        })
    }

    pub fn load(text: Option<&str>, now_secs: i64) -> Self {
        let Some(text) = text else {
            return Self::default();
        };
        let json = match parse_json(text) {
            Ok(j) => j,
            Err(e) => {
                return Self {
                    load_error: Some(format!("sizes.json unreadable: {e}")),
                    ..Default::default()
                };
            }
        };
        if json.get("v").and_then(Json::as_u64) != Some(u64::from(CACHE_SCHEMA)) {
            return Self {
                load_error: Some(format!(
                    "sizes.json schema {} ignored",
                    json.get("v")
                        .map_or_else(|| "missing".into(), |v| v.to_string())
                )),
                ..Default::default()
            };
        }
        let mut c = Self::default();
        match json.get("entries") {
            None => {}
            Some(Json::Array(es)) => {
                for (i, e) in es.iter().enumerate() {
                    match Self::parse_entry(e) {
                        Ok(entry) => {
                            if now_secs.saturating_sub(entry.recorded_secs) > CACHE_TTL_SECS {
                                continue;
                            }
                            c.entries.push(entry);
                        }
                        Err(why) => {
                            if c.load_error.is_none() {
                                c.load_error = Some(format!("sizes.json entry {i} invalid: {why}"));
                            }
                        }
                    }
                }
            }
            Some(_) => c.load_error = Some("sizes.json entries is not an array".into()),
        }
        c
    }

    pub fn merge(&mut self, other: &SizeCache, now_secs: i64) {
        for o in &other.entries {
            if now_secs.saturating_sub(o.recorded_secs) > CACHE_TTL_SECS {
                continue;
            }
            match self.entries.iter_mut().find(|e| e.path == o.path) {
                Some(e) if e.recorded_secs < o.recorded_secs => *e = o.clone(),
                Some(_) => {}
                None => self.entries.push(o.clone()),
            }
        }
        self.entries
            .retain(|e| now_secs.saturating_sub(e.recorded_secs) <= CACHE_TTL_SECS);
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/alex";
    const BLOCK: u64 = crate::sim::fs::BLOCK;

    fn run(fs: &mut Fs, paths: &[&str], mode: Mode, dry_run: bool) -> Report {
        let items = paths
            .iter()
            .map(|p| DeleteItem {
                path: (*p).into(),
                category: "test".into(),
                estimate: fs.size_of(p),
                guard: None,
            })
            .collect();
        let plan = DeletePlan::new("mbp", HOME, items, mode, dry_run);
        let mut exec = Execution::start(&plan, plan.revision, "/log").unwrap();
        let mut log = OpsLog::default();
        let processes = |_: &str| ProcessObservation::NotRunning;
        let cx = ExecContext {
            home: HOME,
            os: Os::MacOs,
            now_secs: 1_000,
            processes: &processes,
        };
        while exec.step(fs, &mut log, &cx) {}
        exec.finish(&mut log)
    }

    fn two_links() -> Fs {
        let mut fs = Fs::new();
        fs.dir("/Users/alex/Downloads", 1)
            .file("/Users/alex/Downloads/a", 100, 1)
            .hardlink("/Users/alex/Downloads/b", "/Users/alex/Downloads/a")
            .volume("/", 1 << 40, 1 << 30);
        fs
    }

    #[test]
    fn cleanup_freed_now_matches_last_link_reclamation() {
        let mut live = two_links();
        let report = run(
            &mut live,
            &["/Users/alex/Downloads/a", "/Users/alex/Downloads/b"],
            Mode::Permanent,
            false,
        );
        assert_eq!(report.freed_now, BLOCK, "the inode is reclaimed once");
        assert_eq!(report.bytes, 2 * BLOCK, "processed sizes stay distinct");
        assert_eq!(report.items[0].reclaimed_bytes, 0);
        assert_eq!(report.items[1].reclaimed_bytes, BLOCK);
        assert_eq!(report.items[0].bytes, BLOCK);
        assert_eq!(report.items[1].bytes, BLOCK);

        let mut dry = two_links();
        let estimate = run(
            &mut dry,
            &["/Users/alex/Downloads/a", "/Users/alex/Downloads/b"],
            Mode::Permanent,
            true,
        );
        assert_eq!(estimate.freed_now, 0, "dry run reclaims nothing");
        assert_eq!(
            estimate.bytes,
            2 * BLOCK,
            "dry run totals its processed estimate"
        );
        assert_eq!(
            estimate.items.iter().map(|i| i.bytes).collect::<Vec<_>>(),
            vec![BLOCK, BLOCK],
            "dry run keeps each item's processed estimate"
        );

        let mut trash = two_links();
        let trashed = run(
            &mut trash,
            &["/Users/alex/Downloads/a", "/Users/alex/Downloads/b"],
            Mode::Trash,
            false,
        );
        assert_eq!(trashed.freed_now, 0, "trash never reclaims");
        assert_eq!(trashed.bytes, 2 * BLOCK);
    }

    #[test]
    fn cleanup_log_records_reclaimed_size_round_trip() {
        let record = LogRecord {
            timestamp_ms: 1_000,
            mode: Mode::Permanent,
            dry_run: false,
            path: "/Users/alex/Downloads/big".into(),
            size: u64::MAX - 7,
            reclaimed_size: Some(u64::MAX - 9),
            outcome: Outcome::Removed,
            error: None,
        };
        assert_eq!(LogRecord::parse(&record.json()), Some(record));
    }

    #[test]
    fn cleanup_legacy_log_reclamation_is_unknown() {
        let legacy = r#"{"v":1,"timestamp_ms":1000,"mode":"permanent","dry_run":false,"path":"/Users/alex/Downloads/old","size":8192,"outcome":"removed"}"#;
        let parsed = LogRecord::parse(legacy).unwrap();
        assert_eq!(parsed.size, 8192);
        assert_eq!(
            parsed.reclaimed_size, None,
            "a record without reclaimed_size never reads as measured reclamation"
        );
    }

    #[test]
    fn cleanup_dry_run_report_bytes_sums_estimates() {
        let mut fs = two_links();
        fs.file("/Users/alex/Downloads/c", 3 * BLOCK, 1);
        let before = fs.clone();
        let paths = [
            "/Users/alex/Downloads/a",
            "/Users/alex/Downloads/b",
            "/Users/alex/Downloads/c",
        ];
        let report = run(&mut fs, &paths, Mode::Permanent, true);
        assert_eq!(
            report.bytes,
            5 * BLOCK,
            "a dry run totals every item's estimate, a shared inode once per item"
        );
        assert_eq!(
            report.bytes,
            report.items.iter().map(|item| item.bytes).sum::<u64>(),
            "the total is the sum of the per-item estimates"
        );
        assert_eq!(report.freed_now, 0);
        assert!(report.items.iter().all(|item| item.reclaimed_bytes == 0));
        assert_eq!(fs, before, "a dry run removes nothing");
    }

    #[test]
    fn cleanup_report_totals_saturate_instead_of_overflowing() {
        let huge = u64::MAX / 2 + 1;
        let mut fs = Fs::new();
        fs.dir("/Users/alex/Downloads", 1)
            .sparse("/Users/alex/Downloads/a", huge, huge, 1)
            .sparse("/Users/alex/Downloads/b", huge, huge, 1)
            .volume("/", u64::MAX, 1 << 30);
        let paths = ["/Users/alex/Downloads/a", "/Users/alex/Downloads/b"];
        let dry = run(&mut fs.clone(), &paths, Mode::Permanent, true);
        assert_eq!(dry.bytes, u64::MAX, "dry-run estimates saturate");
        assert_eq!(dry.items.len(), 2, "both items are still recorded");
        let live = run(&mut fs, &paths, Mode::Permanent, false);
        assert_eq!(live.bytes, u64::MAX, "processed bytes saturate");
        assert_eq!(live.freed_now, u64::MAX, "reclaimed bytes saturate");
    }

    const NOW: i64 = 1_800_000_000;

    fn entry(path: &str, allocated: u64, mtime: i64, at: i64) -> CacheEntry {
        CacheEntry {
            path: path.into(),
            allocated,
            mtime,
            recorded_secs: at,
        }
    }

    #[test]
    fn holla_json_strings_roundtrip_all_escape_classes() {
        let cache = SizeCache {
            entries: vec![entry("/w/a\"b\\nc\nline\ttab\u{1}ctl 🚀", 4096, 10, NOW)],
            ..Default::default()
        };
        let loaded = SizeCache::load(Some(&cache.serialize()), NOW);
        assert_eq!(loaded.load_error, None);
        assert_eq!(loaded.entries, cache.entries);
    }

    #[test]
    fn holla_json_legacy_size_cache_with_bare_backslash_degrades_to_empty() {
        // The pre-#63 writer escaped only quotes, so `\d` reached disk raw and
        // is an invalid escape now. The cache is regenerable: drop it with a
        // diagnostic instead of guessing the path.
        let legacy = format!(
            r#"{{"v":{CACHE_SCHEMA},"entries":[{{"path":"/w/a\d","allocated":4096,"mtime":1,"at":{NOW}}}]}}"#
        );
        let loaded = SizeCache::load(Some(&legacy), NOW);
        assert!(loaded.entries.is_empty());
        assert!(
            loaded
                .load_error
                .as_deref()
                .is_some_and(|e| e.starts_with("sizes.json unreadable"))
        );
    }

    #[test]
    fn holla_json_integer_fields_are_exact_and_checked() {
        let exact = entry("/exact", u64::MAX, 9_007_199_254_740_993, NOW);
        let cache = SizeCache {
            entries: vec![exact.clone()],
            ..Default::default()
        };
        let loaded = SizeCache::load(Some(&cache.serialize()), NOW);
        assert_eq!(loaded.entries, vec![exact]);

        let text = format!(
            r#"{{"v":{CACHE_SCHEMA},"entries":[{{"path":"/frac","allocated":1.5,"mtime":1,"at":{NOW}}},{{"path":"/exp","allocated":1e3,"mtime":1,"at":{NOW}}},{{"path":"/neg","allocated":-1,"mtime":1,"at":{NOW}}}]}}"#
        );
        let rejected = SizeCache::load(Some(&text), NOW);
        assert!(rejected.entries.is_empty());
        assert!(rejected.load_error.is_some());
    }

    #[test]
    fn holla_json_invalid_entries_keep_valid_siblings_with_diagnostic() {
        let text = format!(
            r#"{{"v":{CACHE_SCHEMA},"entries":[{{"path":"/a","allocated":1,"mtime":1,"at":{NOW}}},{{"path":"/bad","allocated":"x","mtime":1,"at":{NOW}}},{{"path":"/c","allocated":3,"mtime":3,"at":{NOW}}}]}}"#
        );
        let loaded = SizeCache::load(Some(&text), NOW);
        let paths: Vec<&str> = loaded.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["/a", "/c"]);
        assert!(
            loaded
                .load_error
                .as_deref()
                .is_some_and(|e| e.contains("entry 1"))
        );
    }

    #[test]
    fn holla_json_extreme_timestamps_do_not_overflow() {
        let text = format!(
            r#"{{"v":{CACHE_SCHEMA},"entries":[{{"path":"/old","allocated":1,"mtime":1,"at":{min}}},{{"path":"/future","allocated":1,"mtime":1,"at":{max}}}]}}"#,
            min = i64::MIN,
            max = i64::MAX
        );
        let mut loaded = SizeCache::load(Some(&text), NOW);
        let paths: Vec<&str> = loaded.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["/future"]);
        assert!(loaded.hint("/future", 1, NOW).is_some());
        let other = SizeCache {
            entries: vec![entry("/old", 1, 1, i64::MIN)],
            ..Default::default()
        };
        loaded.merge(&other, NOW);
        assert_eq!(loaded.entries.len(), 1);
    }

    #[test]
    fn holla_ops_log_escaped_field_names_are_data() {
        let record = LogRecord {
            timestamp_ms: 42,
            mode: Mode::Trash,
            dry_run: false,
            path: "/x/\",\"outcome\":\"failed\",\"size\":7,\"y".into(),
            size: 9,
            reclaimed_size: Some(3),
            outcome: Outcome::Removed,
            error: Some("\"mode\":\"permanent\"\nboom".into()),
        };
        assert_eq!(LogRecord::parse(&record.json()), Some(record));
    }

    #[test]
    fn holla_ops_log_reclaimed_size_goes_through_the_json_codec() {
        let hostile = "/x/\",\"reclaimed_size\":5,\"y\n";
        let record = |reclaimed_size| LogRecord {
            timestamp_ms: i64::MIN,
            mode: Mode::Permanent,
            dry_run: false,
            path: hostile.into(),
            size: 9_007_199_254_740_993,
            reclaimed_size,
            outcome: Outcome::Removed,
            error: None,
        };

        let exact = record(Some(9_007_199_254_740_993));
        let line = exact.json();
        let value = parse_json(&line).expect("the writer emits strict JSON");
        assert_eq!(
            json_u64(&value, "reclaimed_size"),
            Ok(9_007_199_254_740_993),
            "reclaimed_size is written as an exact JSON integer"
        );
        assert_eq!(LogRecord::parse(&line), Some(exact));

        let unknown = record(None);
        let line = unknown.json();
        assert!(
            parse_json(&line).unwrap().get("reclaimed_size").is_none(),
            "an unknown reclamation is omitted, not written as zero"
        );
        let parsed = LogRecord::parse(&line).expect("round trip");
        assert_eq!(
            parsed.reclaimed_size, None,
            "key-like text inside the path is data, never the reclaimed field"
        );
        assert_eq!(parsed, unknown);
    }

    #[test]
    fn holla_ops_log_rejects_ill_typed_reclaimed_size() {
        let line = |field: &str| {
            format!(
                r#"{{"v":1,"timestamp_ms":42,"mode":"trash","dry_run":false,"path":"/x","size":9,"reclaimed_size":{field},"outcome":"removed"}}"#
            )
        };
        assert_eq!(
            LogRecord::parse(&line("null")).map(|r| r.reclaimed_size),
            Some(None)
        );
        for bad in ["1.0", "1e3", "-1", "\"7\"", "true", "18446744073709551616"] {
            assert_eq!(LogRecord::parse(&line(bad)), None, "reclaimed_size {bad}");
        }
    }

    #[test]
    fn holla_ops_log_rejects_non_boolean_dry_run() {
        let line = r#"{"v":1,"timestamp_ms":42,"mode":"trash","dry_run":"true","path":"/x","size":9,"outcome":"removed"}"#;
        assert_eq!(LogRecord::parse(line), None);
    }
}
