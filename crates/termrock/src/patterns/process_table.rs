// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ProcessTable** — process / task monitor with tree and flat modes.
//!
//! **Mission.** PID, command, CPU, memory, status, user, elapsed time,
//! hierarchy, search, sort, filters, details, signals, and refresh cadence.
//! Selection survives refresh/churn via **stable identity** ([`ProcessKey`]:
//! pid + start marker — never PID alone). Safe signal/terminate/kill
//! confirmation. Host owns process enumeration and kill/signal syscalls.
//!
//! Research: btop, bottom, htop, procs, process explorers.
//!
//! **vs [`super::TreeTable`].** TreeTable is generic hierarchy+columns.
//! ProcessTable is process-domain projection, sort/filter policy, signal
//! confirm chrome, and paint tuned for live monitors.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::ColumnModel,
    widgets::DataColumn,
    widgets::DataColumnWidth,
    widgets::LoadState,
    widgets::SortSpec,
    widgets::VirtualWindow,
};

// ── Identity ────────────────────────────────────────────────────────────────

/// Stable process identity across PID reuse.
///
/// Host must supply a start marker (boot-relative ms, starttime ticks, or
/// unique generation). Selection keys on `(pid, start_ms)`, never PID alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ProcessKey {
    /// OS process id.
    pub pid: u32,
    /// Host-defined start marker (ms or ticks); must change if PID is recycled.
    pub start_ms: u64,
}

impl ProcessKey {
    /// Construct.
    #[must_use]
    pub const fn new(pid: u32, start_ms: u64) -> Self {
        Self { pid, start_ms }
    }
}

// ── Domain model ────────────────────────────────────────────────────────────

/// Process run state (host classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ProcessStatus {
    /// Runnable / running.
    #[default]
    Running,
    /// Interruptible sleep.
    Sleeping,
    /// Idle / uninterruptible wait.
    Idle,
    /// Stopped (job control).
    Stopped,
    /// Zombie.
    Zombie,
    /// Dead / exiting.
    Dead,
    /// Unknown.
    Unknown,
}

impl ProcessStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Sleeping => "sleeping",
            Self::Idle => "idle",
            Self::Stopped => "stopped",
            Self::Zombie => "zombie",
            Self::Dead => "dead",
            Self::Unknown => "unknown",
        }
    }

    /// Short status letter (htop-class).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Running => "R",
            Self::Sleeping => "S",
            Self::Idle => "D",
            Self::Stopped => "T",
            Self::Zombie => "Z",
            Self::Dead => "X",
            Self::Unknown => "?",
        }
    }

    /// Semantic role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Running => Role::Success,
            Self::Sleeping | Self::Idle => Role::TextMuted,
            Self::Stopped => Role::Warning,
            Self::Zombie | Self::Dead => Role::Danger,
            Self::Unknown => Role::TextDisabled,
        }
    }
}

/// Flat list vs process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProcessViewMode {
    /// Flat list (default for sort-heavy monitors).
    #[default]
    Flat,
    /// Parent/child hierarchy.
    Tree,
}

impl ProcessViewMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Tree => "tree",
        }
    }

    /// Toggle flat ↔ tree.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Flat => Self::Tree,
            Self::Tree => Self::Flat,
        }
    }
}

/// Sortable columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProcessSortKey {
    /// PID.
    Pid,
    /// CPU % (default for live monitors).
    #[default]
    Cpu,
    /// Memory bytes.
    Memory,
    /// Elapsed time.
    Elapsed,
    /// Command name.
    Command,
    /// User.
    User,
    /// Status.
    Status,
}

impl ProcessSortKey {
    /// Stable id (also column id).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pid => "pid",
            Self::Cpu => "cpu",
            Self::Memory => "mem",
            Self::Elapsed => "time",
            Self::Command => "cmd",
            Self::User => "user",
            Self::Status => "stat",
        }
    }

    /// Header label.
    #[must_use]
    pub const fn header(self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Cpu => "CPU%",
            Self::Memory => "MEM",
            Self::Elapsed => "TIME",
            Self::Command => "COMMAND",
            Self::User => "USER",
            Self::Status => "S",
        }
    }

    /// Cycle for `s` chord.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Pid,
            Self::Pid => Self::Elapsed,
            Self::Elapsed => Self::Command,
            Self::Command => Self::User,
            Self::User => Self::Status,
            Self::Status => Self::Cpu,
        }
    }

    /// Parse column id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "pid" => Some(Self::Pid),
            "cpu" => Some(Self::Cpu),
            "mem" => Some(Self::Memory),
            "time" => Some(Self::Elapsed),
            "cmd" => Some(Self::Command),
            "user" => Some(Self::User),
            "stat" => Some(Self::Status),
            _ => None,
        }
    }
}

/// Unix-style signal request (host maps to OS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProcessSignal {
    /// SIGTERM (polite terminate).
    Term,
    /// SIGKILL.
    Kill,
    /// SIGINT.
    Int,
    /// SIGHUP.
    Hup,
    /// SIGQUIT.
    Quit,
    /// SIGSTOP.
    Stop,
    /// SIGCONT.
    Cont,
    /// SIGUSR1.
    Usr1,
    /// SIGUSR2.
    Usr2,
    /// Host-defined signal number.
    Custom(i32),
}

impl ProcessSignal {
    /// Stable id.
    #[must_use]
    pub fn id(self) -> String {
        match self {
            Self::Term => "TERM".into(),
            Self::Kill => "KILL".into(),
            Self::Int => "INT".into(),
            Self::Hup => "HUP".into(),
            Self::Quit => "QUIT".into(),
            Self::Stop => "STOP".into(),
            Self::Cont => "CONT".into(),
            Self::Usr1 => "USR1".into(),
            Self::Usr2 => "USR2".into(),
            Self::Custom(n) => format!("SIG{n}"),
        }
    }

    /// Safe confirm verb (no “destroy” / casual “kill all”).
    #[must_use]
    pub const fn safe_verb(self) -> &'static str {
        match self {
            Self::Term => "request terminate (TERM) on",
            Self::Kill => "force kill (KILL) on",
            Self::Int => "interrupt (INT)",
            Self::Hup => "hangup (HUP)",
            Self::Quit => "quit (QUIT)",
            Self::Stop => "stop (STOP)",
            Self::Cont => "continue (CONT)",
            Self::Usr1 | Self::Usr2 | Self::Custom(_) => "signal",
        }
    }

    /// Whether multi-target or single destructive needs confirm.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Term | Self::Kill | Self::Quit)
    }
}

/// One process row (host projection).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessRow<'a> {
    /// Stable key.
    pub key: ProcessKey,
    /// Parent key when known (tree mode).
    pub parent: Option<ProcessKey>,
    /// Depth in tree (0 root); ignored in flat sort projection.
    pub depth: u16,
    /// Has children (tree).
    pub branch: bool,
    /// Expanded (host-owned).
    pub expanded: bool,
    /// Command / cmdline summary.
    pub command: &'a str,
    /// CPU percent 0..=100+.
    pub cpu_pct: f64,
    /// RSS or similar bytes.
    pub mem_bytes: u64,
    /// Status.
    pub status: ProcessStatus,
    /// User name.
    pub user: &'a str,
    /// Elapsed wall ms.
    pub elapsed_ms: u64,
    /// Interaction enabled.
    pub enabled: bool,
}

impl<'a> ProcessRow<'a> {
    /// Construct leaf/process.
    #[must_use]
    pub fn new(key: ProcessKey, command: &'a str) -> Self {
        Self {
            key,
            parent: None,
            depth: 0,
            branch: false,
            expanded: false,
            command,
            cpu_pct: 0.0,
            mem_bytes: 0,
            status: ProcessStatus::Running,
            user: "",
            elapsed_ms: 0,
            enabled: true,
        }
    }

    /// Parent.
    #[must_use]
    pub const fn parent(mut self, parent: ProcessKey) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Depth.
    #[must_use]
    pub const fn depth(mut self, d: u16) -> Self {
        self.depth = d;
        self
    }

    /// Branch.
    #[must_use]
    pub const fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// CPU %.
    #[must_use]
    pub const fn cpu(mut self, pct: f64) -> Self {
        self.cpu_pct = pct;
        self
    }

    /// Memory bytes.
    #[must_use]
    pub const fn mem(mut self, bytes: u64) -> Self {
        self.mem_bytes = bytes;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: ProcessStatus) -> Self {
        self.status = s;
        self
    }

    /// User.
    #[must_use]
    pub const fn user(mut self, user: &'a str) -> Self {
        self.user = user;
        self
    }

    /// Elapsed.
    #[must_use]
    pub const fn elapsed_ms(mut self, ms: u64) -> Self {
        self.elapsed_ms = ms;
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// ── Formatting ──────────────────────────────────────────────────────────────

/// Format memory for table cell.
#[must_use]
pub fn format_mem_bytes(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b < K {
        format!("{bytes}B")
    } else if b < K * K {
        format!("{:.1}K", b / K)
    } else if b < K * K * K {
        format!("{:.1}M", b / (K * K))
    } else {
        format!("{:.1}G", b / (K * K * K))
    }
}

/// Format CPU %.
#[must_use]
pub fn format_cpu_pct(pct: f64) -> String {
    if !pct.is_finite() {
        return "—".into();
    }
    if pct < 10.0 {
        format!("{pct:.1}")
    } else {
        format!("{pct:.0}")
    }
}

/// Format elapsed ms as h:mm:ss or m:ss.
#[must_use]
pub fn format_elapsed_ms(ms: u64) -> String {
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

// ── Sort / filter ───────────────────────────────────────────────────────────

/// Compare two rows by sort key.
#[must_use]
pub fn cmp_process(
    a: &ProcessRow<'_>,
    b: &ProcessRow<'_>,
    key: ProcessSortKey,
    asc: bool,
) -> Ordering {
    let ord = match key {
        ProcessSortKey::Pid => a.key.pid.cmp(&b.key.pid),
        ProcessSortKey::Cpu => a.cpu_pct.partial_cmp(&b.cpu_pct).unwrap_or(Ordering::Equal),
        ProcessSortKey::Memory => a.mem_bytes.cmp(&b.mem_bytes),
        ProcessSortKey::Elapsed => a.elapsed_ms.cmp(&b.elapsed_ms),
        ProcessSortKey::Command => a.command.cmp(b.command),
        ProcessSortKey::User => a.user.cmp(b.user),
        ProcessSortKey::Status => a.status.cmp(&b.status),
    };
    if asc { ord } else { ord.reverse() }
}

/// Filter rows by search query (command/user/pid) and optional user/status.
#[must_use]
pub fn filter_processes<'a>(
    rows: &'a [ProcessRow<'a>],
    query: &str,
    user_filter: Option<&str>,
    status_filter: Option<ProcessStatus>,
) -> Vec<&'a ProcessRow<'a>> {
    let q = query.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| {
            if let Some(u) = user_filter
                && r.user != u
            {
                return false;
            }
            if let Some(s) = status_filter
                && r.status != s
            {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            let pid = r.key.pid.to_string();
            let hay = format!("{} {} {}", r.command, r.user, pid).to_ascii_lowercase();
            hay.contains(&q)
        })
        .collect()
}

/// Sort a filtered flat list (does not preserve tree order).
#[must_use]
pub fn sort_processes_flat<'a>(
    mut rows: Vec<&'a ProcessRow<'a>>,
    key: ProcessSortKey,
    ascending: bool,
) -> Vec<&'a ProcessRow<'a>> {
    rows.sort_by(|a, b| cmp_process(a, b, key, ascending));
    rows
}

/// Keep filtered rows plus ancestors so tree mode stays navigable.
#[must_use]
pub fn filter_tree_preserve<'a>(
    all: &'a [ProcessRow<'a>],
    filtered: &[&'a ProcessRow<'a>],
) -> Vec<&'a ProcessRow<'a>> {
    if filtered.len() == all.len() {
        return all.iter().collect();
    }
    let mut keep: BTreeSet<ProcessKey> = filtered.iter().map(|p| p.key).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for p in all {
            if keep.contains(&p.key)
                && let Some(par) = p.parent
                && keep.insert(par)
            {
                changed = true;
            }
        }
    }
    all.iter().filter(|p| keep.contains(&p.key)).collect()
}

/// Build default column model (documentation / host layout).
#[must_use]
pub fn process_column_model() -> ColumnModel<&'static str> {
    ColumnModel::new(vec![
        DataColumn::new("cmd", "COMMAND", DataColumnWidth::Min(12)).priority(100),
        DataColumn::new("pid", "PID", DataColumnWidth::Fixed(7))
            .priority(95)
            .sortable(),
        DataColumn::new("cpu", "CPU%", DataColumnWidth::Fixed(6))
            .priority(90)
            .sortable(),
        DataColumn::new("mem", "MEM", DataColumnWidth::Fixed(7))
            .priority(80)
            .sortable(),
        DataColumn::new("user", "USER", DataColumnWidth::Min(6)).priority(60),
        DataColumn::new("time", "TIME", DataColumnWidth::Fixed(8))
            .priority(70)
            .sortable(),
        DataColumn::new("stat", "S", DataColumnWidth::Fixed(2)).priority(50),
    ])
}

// ── Confirm / outcomes ──────────────────────────────────────────────────────

/// Pending signal confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSignalConfirm {
    /// Signal.
    pub signal: ProcessSignal,
    /// Subject text.
    pub subject: String,
    /// Target keys.
    pub targets: Vec<ProcessKey>,
}

/// Outcomes — host owns kill/signal/refresh syscalls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProcessTableOutcome {
    /// No change.
    Ignored,
    /// Selection moved.
    SelectionChanged(ProcessKey),
    /// Multi-check toggled.
    CheckToggled(ProcessKey),
    /// Tree expand/collapse (host updates projection).
    ExpandToggled(ProcessKey),
    /// Open details.
    DetailsRequested(ProcessKey),
    /// Sort changed.
    SortChanged {
        /// Key.
        key: ProcessSortKey,
        /// Ascending.
        ascending: bool,
    },
    /// View mode flat/tree.
    ViewModeChanged(ProcessViewMode),
    /// Filter query.
    FilterChanged(String),
    /// User filter.
    UserFilterChanged(Option<String>),
    /// Status filter.
    StatusFilterChanged(Option<ProcessStatus>),
    /// Signal request (after confirm if needed).
    SignalRequested {
        /// Targets.
        keys: Vec<ProcessKey>,
        /// Signal.
        signal: ProcessSignal,
    },
    /// Confirm banner.
    ConfirmRequired(ProcessSignalConfirm),
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Host should re-enumerate processes.
    RefreshRequested,
    /// Copy selected command lines.
    CopyCommand {
        /// Text.
        text: String,
    },
    /// Cancelled filter.
    Cancelled,
    /// Viewport scrolled.
    Scrolled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Process table state.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessTableState {
    /// Cursor selection (stable key).
    selected: Option<ProcessKey>,
    /// Multi-check set.
    checked: Vec<ProcessKey>,
    /// Multi-select enabled.
    multi: bool,
    /// Virtual window over visible rows.
    pub window: VirtualWindow,
    /// Flat vs tree.
    pub view_mode: ProcessViewMode,
    /// Sort key.
    pub sort_key: ProcessSortKey,
    /// Sort ascending.
    pub sort_asc: bool,
    /// Search query.
    pub filter: Option<String>,
    /// Optional user filter.
    pub user_filter: Option<String>,
    /// Optional status filter.
    pub status_filter: Option<ProcessStatus>,
    /// Pending signal confirm.
    pub pending_confirm: Option<ProcessSignalConfirm>,
    /// Desired refresh interval ms (host timer; chrome only).
    pub refresh_ms: u32,
    /// Generation counter host may bump on full rescan.
    pub generation: u64,
    /// Load chrome.
    pub load: LoadState,
    /// ASCII glyphs.
    pub ascii: bool,
    /// Host grants input.
    accepts_input: bool,
    /// Previous index for nearest-neighbor restore.
    previous_index: Option<usize>,
    /// Body hit regions (last paint).
    row_regions: Vec<(ProcessKey, Rect)>,
}

impl Default for ProcessTableState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTableState {
    /// Fresh (CPU desc default).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            checked: Vec::new(),
            multi: false,
            window: VirtualWindow::default(),
            view_mode: ProcessViewMode::Flat,
            sort_key: ProcessSortKey::Cpu,
            sort_asc: false,
            filter: None,
            user_filter: None,
            status_filter: None,
            pending_confirm: None,
            refresh_ms: 1000,
            generation: 0,
            load: LoadState::Ready { count: 0 },
            ascii: false,
            accepts_input: true,
            previous_index: None,
            row_regions: Vec::new(),
        }
    }

    /// With initial selection.
    #[must_use]
    pub fn with_selected(key: Option<ProcessKey>) -> Self {
        let mut s = Self::new();
        s.selected = key;
        s
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Selected process key.
    #[must_use]
    pub const fn selected(&self) -> Option<ProcessKey> {
        self.selected
    }

    /// Select key.
    pub fn select(&mut self, key: Option<ProcessKey>) {
        self.selected = key;
    }

    /// Multi-check membership.
    #[must_use]
    pub fn checked(&self) -> &[ProcessKey] {
        &self.checked
    }

    /// Whether multi-select is enabled.
    #[must_use]
    pub const fn multi_enabled(&self) -> bool {
        self.multi
    }

    /// Enable multi-select.
    pub fn enable_multi_select(&mut self) {
        self.multi = true;
    }

    /// Active sort as [`SortSpec`].
    #[must_use]
    pub fn sort_spec(&self) -> SortSpec<&'static str> {
        SortSpec {
            column: self.sort_key.id(),
            ascending: self.sort_asc,
        }
    }

    /// Reconcile selection after host refresh (drops dead keys; keeps stable matches).
    pub fn reconcile(&mut self, live: &[ProcessRow<'_>]) {
        let keys: Vec<ProcessKey> = live.iter().map(|r| r.key).collect();
        if let Some(sel) = self.selected {
            if let Some(idx) = keys.iter().position(|k| *k == sel) {
                self.previous_index = Some(idx);
            } else {
                // PID reuse: old key gone — nearest neighbor
                let anchor = self.previous_index.unwrap_or(0);
                if keys.is_empty() {
                    self.selected = None;
                    self.previous_index = None;
                } else {
                    let idx = keys
                        .iter()
                        .enumerate()
                        .min_by_key(|(i, _)| i.abs_diff(anchor))
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.selected = Some(keys[idx]);
                    self.previous_index = Some(idx);
                }
            }
        }
        self.checked.retain(|k| keys.contains(k));
        self.generation = self.generation.saturating_add(1);
        self.load = LoadState::Ready {
            count: live.len() as u64,
        };
        self.window.logical_len = live.len() as u64;
        self.window.clamp();
    }

    /// Visible ordered processes for paint/nav.
    #[must_use]
    pub fn visible_processes<'a>(&self, all: &'a [ProcessRow<'a>]) -> Vec<&'a ProcessRow<'a>> {
        let filtered = filter_processes(
            all,
            self.filter.as_deref().unwrap_or(""),
            self.user_filter.as_deref(),
            self.status_filter,
        );
        match self.view_mode {
            ProcessViewMode::Flat => sort_processes_flat(filtered, self.sort_key, self.sort_asc),
            ProcessViewMode::Tree => filter_tree_preserve(all, &filtered),
        }
    }

    /// Sync window length from visible projection.
    pub fn sync_window(&mut self, visible_len: usize) {
        self.window.logical_len = visible_len as u64;
        self.window.clamp();
        self.load = LoadState::Ready {
            count: visible_len as u64,
        };
    }

    fn reveal_index(&mut self, idx: usize) {
        let _ = self.window.reveal(idx as u64);
        self.previous_index = Some(idx);
    }

    fn targets(&self) -> Vec<ProcessKey> {
        if self.multi && !self.checked.is_empty() {
            return self.checked.clone();
        }
        self.selected.into_iter().collect()
    }

    fn request_signal(
        &mut self,
        processes: &[ProcessRow<'_>],
        signal: ProcessSignal,
    ) -> ProcessTableOutcome {
        let keys = self.targets();
        if keys.is_empty() {
            return ProcessTableOutcome::Ignored;
        }
        let multi = keys.len() > 1;
        let subject = if multi {
            format!("{} processes", keys.len())
        } else {
            processes
                .iter()
                .find(|p| Some(p.key) == self.selected)
                .map(|p| format!("{} ({})", p.command, p.key.pid))
                .unwrap_or_else(|| format!("pid {}", keys[0].pid))
        };
        if multi || signal.is_destructive() {
            let conf = ProcessSignalConfirm {
                signal,
                subject: subject.clone(),
                targets: keys.clone(),
            };
            self.pending_confirm = Some(conf.clone());
            ProcessTableOutcome::ConfirmRequired(conf)
        } else {
            ProcessTableOutcome::SignalRequested { keys, signal }
        }
    }

    fn copy_commands(&self, processes: &[ProcessRow<'_>]) -> ProcessTableOutcome {
        let mut lines = Vec::new();
        let keys = self.targets();
        for k in keys {
            if let Some(p) = processes.iter().find(|p| p.key == k) {
                lines.push(p.command.to_string());
            }
        }
        if lines.is_empty() {
            ProcessTableOutcome::Ignored
        } else {
            ProcessTableOutcome::CopyCommand {
                text: lines.join("\n"),
            }
        }
    }

    /// Primary key handler (nav + process chords).
    pub fn handle_key(
        &mut self,
        processes: &[ProcessRow<'_>],
        key: KeyEvent,
    ) -> ProcessTableOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return ProcessTableOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        if self.pending_confirm.is_some() && is_press {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    let conf = self.pending_confirm.take().expect("pending");
                    return ProcessTableOutcome::SignalRequested {
                        keys: conf.targets,
                        signal: conf.signal,
                    };
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.pending_confirm = None;
                    return ProcessTableOutcome::ConfirmCancelled;
                }
                _ => return ProcessTableOutcome::Ignored,
            }
        }

        // Filter typing
        if let Some(q) = self.filter.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return ProcessTableOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return ProcessTableOutcome::FilterChanged(
                        self.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c)
                    if !c.is_control()
                        && !matches!(c, 'j' | 'k' | 'h' | 'l' | 'J' | 'K' | 'H' | 'L') =>
                {
                    q.push(c);
                    return ProcessTableOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        if is_press {
            match key.code {
                KeyCode::Char('/') if key.modifiers.is_empty() => {
                    self.filter = Some(String::new());
                    return ProcessTableOutcome::FilterChanged(String::new());
                }
                KeyCode::Char('s') if key.modifiers.is_empty() => {
                    self.sort_key = self.sort_key.next();
                    return ProcessTableOutcome::SortChanged {
                        key: self.sort_key,
                        ascending: self.sort_asc,
                    };
                }
                KeyCode::Char('S') => {
                    self.sort_asc = !self.sort_asc;
                    return ProcessTableOutcome::SortChanged {
                        key: self.sort_key,
                        ascending: self.sort_asc,
                    };
                }
                KeyCode::Char('t') if key.modifiers.is_empty() => {
                    self.view_mode = self.view_mode.toggle();
                    return ProcessTableOutcome::ViewModeChanged(self.view_mode);
                }
                KeyCode::Char('r') if key.modifiers.is_empty() => {
                    return ProcessTableOutcome::RefreshRequested;
                }
                KeyCode::Char('K') => {
                    return self.request_signal(processes, ProcessSignal::Kill);
                }
                KeyCode::Char('x') if key.modifiers.is_empty() => {
                    return self.request_signal(processes, ProcessSignal::Term);
                }
                KeyCode::Char('9') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return self.request_signal(processes, ProcessSignal::Kill);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return self.request_signal(processes, ProcessSignal::Int);
                }
                KeyCode::Char('y') if key.modifiers.is_empty() => {
                    return self.copy_commands(processes);
                }
                KeyCode::Char('i') if key.modifiers.is_empty() => {
                    if let Some(k) = self.selected {
                        return ProcessTableOutcome::DetailsRequested(k);
                    }
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.refresh_ms = (self.refresh_ms.saturating_mul(2)).min(10_000);
                    return ProcessTableOutcome::Ignored;
                }
                KeyCode::Char('-') => {
                    self.refresh_ms = (self.refresh_ms / 2).max(100);
                    return ProcessTableOutcome::Ignored;
                }
                KeyCode::Char(' ') if self.multi && key.modifiers.is_empty() => {
                    if let Some(k) = self.selected {
                        if let Some(pos) = self.checked.iter().position(|c| *c == k) {
                            self.checked.remove(pos);
                        } else {
                            self.checked.push(k);
                        }
                        return ProcessTableOutcome::CheckToggled(k);
                    }
                }
                _ => {}
            }
        }

        // Navigation on visible list
        let visible = self.visible_processes(processes);
        self.sync_window(visible.len());
        if visible.is_empty() {
            return ProcessTableOutcome::Ignored;
        }

        // Tree expand/collapse
        if is_press && matches!(self.view_mode, ProcessViewMode::Tree) && key.modifiers.is_empty() {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if let Some(k) = self.selected
                        && let Some(p) = visible.iter().find(|p| p.key == k)
                        && p.branch
                        && p.expanded
                    {
                        return ProcessTableOutcome::ExpandToggled(k);
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let Some(k) = self.selected
                        && let Some(p) = visible.iter().find(|p| p.key == k)
                        && p.branch
                        && !p.expanded
                    {
                        return ProcessTableOutcome::ExpandToggled(k);
                    }
                }
                _ => {}
            }
        }

        if !is_press {
            return ProcessTableOutcome::Ignored;
        }

        let idx = visible
            .iter()
            .position(|p| Some(p.key) == self.selected)
            .unwrap_or(0);
        let vh = usize::from(self.window.viewport.max(1));

        match key.code {
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                let next = (idx + 1).min(visible.len() - 1);
                let k = visible[next].key;
                self.selected = Some(k);
                self.reveal_index(next);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                let next = idx.saturating_sub(1);
                let k = visible[next].key;
                self.selected = Some(k);
                self.reveal_index(next);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::Home => {
                let k = visible[0].key;
                self.selected = Some(k);
                self.reveal_index(0);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::End => {
                let next = visible.len() - 1;
                let k = visible[next].key;
                self.selected = Some(k);
                self.reveal_index(next);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::PageDown => {
                let next = (idx + vh).min(visible.len() - 1);
                let k = visible[next].key;
                self.selected = Some(k);
                self.reveal_index(next);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::PageUp => {
                let next = idx.saturating_sub(vh);
                let k = visible[next].key;
                self.selected = Some(k);
                self.reveal_index(next);
                ProcessTableOutcome::SelectionChanged(k)
            }
            KeyCode::Enter => {
                if let Some(k) = self.selected {
                    ProcessTableOutcome::DetailsRequested(k)
                } else {
                    ProcessTableOutcome::Ignored
                }
            }
            _ => ProcessTableOutcome::Ignored,
        }
    }

    /// Alias for [`Self::handle_key`] (nav-focused name).
    pub fn handle_key_nav(
        &mut self,
        processes: &[ProcessRow<'_>],
        key: KeyEvent,
    ) -> ProcessTableOutcome {
        self.handle_key(processes, key)
    }

    /// Mouse: click row to select; wheel scroll.
    pub fn handle_mouse(
        &mut self,
        processes: &[ProcessRow<'_>],
        event: MouseEvent,
    ) -> ProcessTableOutcome {
        if !self.accepts_input {
            return ProcessTableOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                let before = self.window.offset;
                let _ = self.window.scroll_by(3);
                if self.window.offset != before {
                    ProcessTableOutcome::Scrolled
                } else {
                    ProcessTableOutcome::Ignored
                }
            }
            MouseEventKind::ScrollUp => {
                let before = self.window.offset;
                let _ = self.window.scroll_by(-3);
                if self.window.offset != before {
                    ProcessTableOutcome::Scrolled
                } else {
                    ProcessTableOutcome::Ignored
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                for (key, rect) in &self.row_regions {
                    if rect.contains(event.position) {
                        let k = *key;
                        self.selected = Some(k);
                        if let Some(idx) = self
                            .visible_processes(processes)
                            .iter()
                            .position(|p| p.key == k)
                        {
                            self.reveal_index(idx);
                        }
                        return ProcessTableOutcome::SelectionChanged(k);
                    }
                }
                ProcessTableOutcome::Ignored
            }
            _ => ProcessTableOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Process monitor table.
#[derive(Debug, Clone, Copy)]
pub struct ProcessTable<'a> {
    processes: &'a [ProcessRow<'a>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
    ascii: bool,
}

impl<'a> ProcessTable<'a> {
    /// Processes + system.
    #[must_use]
    pub const fn new(processes: &'a [ProcessRow<'a>], system: &'a DesignSystem) -> Self {
        Self {
            processes,
            system,
            focused: true,
            title: None,
            ascii: false,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ProcessTableState) {
        if area.is_empty() {
            return;
        }
        let ascii = self.ascii || state.ascii;
        let mut y = area.y;
        let mut h = area.height;
        state.row_regions.clear();

        if let Some(title) = self.title
            && h > 0
        {
            let mode = state.view_mode.id();
            let sort_mark = if state.sort_asc { "^" } else { "v" };
            let line = format!(
                "{title} · {mode} · {}{sort_mark} · {}ms · {} procs",
                state.sort_key.id(),
                state.refresh_ms,
                self.processes.len()
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextStrong),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if state.filter.is_some() && h > 0 {
            let q = state.filter.as_deref().unwrap_or("");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&format!("/{q}_"), usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Accent),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        let confirm_h = u16::from(state.pending_confirm.is_some() && h >= 2);
        let body_h = h.saturating_sub(confirm_h).max(1);

        let header_h = u16::from(body_h >= 2);
        if header_h > 0 {
            // The leading space stands in for the row's selection mark, so the
            // header sits over its own data instead of one cell to the right.
            let hdr = format!(
                " {:<2} {:>7} {:>5} {:>7} {:<8} {:>8} {}",
                "S", "PID", "CPU%", "MEM", "USER", "TIME", "COMMAND"
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&hdr, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        let visible = state.visible_processes(self.processes);
        let rows_h = body_h.saturating_sub(header_h).max(1);
        state.window.viewport = rows_h;
        state.sync_window(visible.len());

        let start = state.window.offset as usize;
        let end = (start + usize::from(rows_h)).min(visible.len());
        let mut py = y;
        let bottom = y.saturating_add(rows_h);

        if visible.is_empty() {
            buffer.set_stringn(
                area.x,
                py,
                take_display_cols("(no processes)", usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            for p in visible.iter().skip(start).take(end.saturating_sub(start)) {
                if py >= bottom {
                    break;
                }
                let selected = state.selected == Some(p.key);
                let checked = state.multi && state.checked.contains(&p.key);
                let indent = if matches!(state.view_mode, ProcessViewMode::Tree) {
                    "  ".repeat(usize::from(p.depth))
                } else {
                    String::new()
                };
                let disc = if matches!(state.view_mode, ProcessViewMode::Tree) && p.branch {
                    if p.expanded {
                        if ascii { "v " } else { "▾ " }
                    } else if ascii {
                        "> "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let mark = if selected {
                    if ascii { ">" } else { "›" }
                } else if checked {
                    if ascii { "*" } else { "★" }
                } else {
                    " "
                };
                let line = format!(
                    "{mark}{:<2} {:>7} {:>5} {:>7} {:<8} {:>8} {}{}{}",
                    p.status.label(),
                    p.key.pid,
                    format_cpu_pct(p.cpu_pct),
                    format_mem_bytes(p.mem_bytes),
                    take_display_cols(p.user, 8),
                    format_elapsed_ms(p.elapsed_ms),
                    indent,
                    disc,
                    p.command
                );
                let style = if selected && self.focused {
                    self.system.style(Role::Focus)
                } else if !p.enabled {
                    self.system.style(Role::TextDisabled)
                } else {
                    self.system.style(Role::Text)
                };
                buffer.set_stringn(
                    area.x,
                    py,
                    take_display_cols(&line, usize::from(area.width)),
                    usize::from(area.width),
                    style,
                );
                if !selected {
                    buffer.set_stringn(
                        area.x.saturating_add(1),
                        py,
                        p.status.label(),
                        1,
                        self.system.style(p.status.role()),
                    );
                }
                state.row_regions.push((
                    p.key,
                    Rect {
                        x: area.x,
                        y: py,
                        width: area.width,
                        height: 1,
                    },
                ));
                py = py.saturating_add(1);
            }
        }

        if let Some(conf) = &state.pending_confirm {
            let cy = area.bottom().saturating_sub(1);
            let msg = format!(
                "! {} {}? Enter=yes Esc=no",
                conf.signal.safe_verb(),
                conf.subject
            );
            buffer.set_stringn(
                area.x,
                cy,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Danger),
            );
        }
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Frequent update / large set targets.
pub mod bench {
    /// Processes in a large host snapshot.
    pub const PROCESS_COUNT: usize = 5_000;
    /// Refresh cadence ms.
    pub const REFRESH_MS: u32 = 1000;
    /// Viewport rows.
    pub const VIEWPORT: u16 = 40;
    /// Frames for streaming paint smoke.
    pub const STREAM_FRAMES: u32 = 120;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> Vec<ProcessRow<'static>> {
        vec![
            ProcessRow::new(ProcessKey::new(1, 100), "systemd")
                .cpu(0.1)
                .mem(4_000_000)
                .user("root")
                .elapsed_ms(86_400_000)
                .branch()
                .expanded(),
            ProcessRow::new(ProcessKey::new(482, 200), "sshd")
                .parent(ProcessKey::new(1, 100))
                .depth(1)
                .cpu(0.0)
                .mem(8_000_000)
                .user("root")
                .elapsed_ms(3_600_000)
                .branch()
                .expanded(),
            ProcessRow::new(ProcessKey::new(1204, 300), "bash")
                .parent(ProcessKey::new(482, 200))
                .depth(2)
                .cpu(1.2)
                .mem(12_000_000)
                .user("alice")
                .elapsed_ms(600_000)
                .status(ProcessStatus::Sleeping),
            ProcessRow::new(ProcessKey::new(1888, 400), "cargo test")
                .parent(ProcessKey::new(1204, 300))
                .depth(3)
                .cpu(42.0)
                .mem(640_000_000)
                .user("alice")
                .elapsed_ms(30_000)
                .branch()
                .expanded(),
            ProcessRow::new(ProcessKey::new(1902, 500), "rustc")
                .parent(ProcessKey::new(1888, 400))
                .depth(4)
                .cpu(88.4)
                .mem(1_100_000_000)
                .user("alice")
                .elapsed_ms(12_000),
        ]
    }

    #[test]
    fn stable_key_survives_pid_reuse() {
        let a = ProcessKey::new(100, 1);
        let b = ProcessKey::new(100, 2); // same pid, new start
        assert_ne!(a, b);
        let mut state = ProcessTableState::new();
        state.select(Some(a));
        let live = [
            ProcessRow::new(b, "new").cpu(1.0).user("u"),
            ProcessRow::new(ProcessKey::new(101, 1), "other").user("u"),
        ];
        state.reconcile(&live);
        assert_ne!(state.selected(), Some(a));
        assert!(state.selected().is_some());
    }

    #[test]
    fn sort_cpu_desc() {
        let rows = sample();
        let refs: Vec<_> = rows.iter().collect();
        let sorted = sort_processes_flat(refs, ProcessSortKey::Cpu, false);
        assert!(sorted[0].cpu_pct >= sorted[1].cpu_pct);
    }

    #[test]
    fn filter_command_and_user() {
        let rows = sample();
        let v = filter_processes(&rows, "cargo", None, None);
        assert_eq!(v.len(), 1);
        let v2 = filter_processes(&rows, "", Some("alice"), None);
        assert!(v2.iter().all(|p| p.user == "alice"));
    }

    #[test]
    fn format_helpers() {
        assert!(format_mem_bytes(1500).contains('K') || format_mem_bytes(1500).contains('B'));
        assert_eq!(format_cpu_pct(42.0), "42");
        assert!(format_elapsed_ms(65_000).contains('1'));
    }

    #[test]
    fn signal_confirm_and_term() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1902, 500)));
        let out = state.handle_key(&rows, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(matches!(out, ProcessTableOutcome::ConfirmRequired(_)));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ProcessTableOutcome::SignalRequested {
                signal: ProcessSignal::Term,
                ..
            }
        ));
    }

    #[test]
    fn kill_confirm() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1888, 400)));
        let out = state.handle_key(&rows, KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE));
        assert!(matches!(
            out,
            ProcessTableOutcome::ConfirmRequired(c) if c.signal == ProcessSignal::Kill
        ));
    }

    #[test]
    fn nav_sort_mode_refresh() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1, 100)));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ProcessTableOutcome::SelectionChanged(_)
        ));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            ProcessTableOutcome::SortChanged { .. }
        ));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)),
            ProcessTableOutcome::ViewModeChanged(ProcessViewMode::Tree)
        ));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            ProcessTableOutcome::RefreshRequested
        ));
    }

    #[test]
    fn paint_flat_and_tree() {
        let system = DesignSystem::default();
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1888, 400)));
        let area = Rect::new(0, 0, 72, 12);
        let mut buf = Buffer::empty(area);
        ProcessTable::new(&rows, &system)
            .title("Procs")
            .render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("cargo") || text.contains("PID") || text.contains("procs"),
            "{text}"
        );

        state.view_mode = ProcessViewMode::Tree;
        ProcessTable::new(&rows, &system).render(area, &mut buf, &mut state);
    }

    #[test]
    fn copy_command() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1888, 400)));
        assert!(matches!(
            state.handle_key(
                &rows,
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)
            ),
            ProcessTableOutcome::CopyCommand { text } if text.contains("cargo")
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ProcessTableOutcome::Ignored
        ));
    }

    #[test]
    fn selection_preserved_across_refresh() {
        let mut state = ProcessTableState::new();
        let key = ProcessKey::new(42, 900);
        state.select(Some(key));
        let live = [
            ProcessRow::new(ProcessKey::new(1, 1), "init").user("root"),
            ProcessRow::new(key, "worker").cpu(5.0).user("alice"),
            ProcessRow::new(ProcessKey::new(99, 2), "other").user("bob"),
        ];
        state.reconcile(&live);
        assert_eq!(state.selected(), Some(key));
    }

    #[test]
    fn large_snapshot_sort_and_paint() {
        let system = DesignSystem::default();
        // static command names for 'static rows via leak-free approach: use fixed labels
        let cmds: Vec<String> = (0..bench::PROCESS_COUNT)
            .map(|i| format!("proc-{i}"))
            .collect();
        let rows: Vec<ProcessRow<'_>> = cmds
            .iter()
            .enumerate()
            .map(|(i, c)| {
                ProcessRow::new(ProcessKey::new(i as u32 + 1, i as u64), c)
                    .cpu((i % 100) as f64)
                    .mem((i as u64) * 1024)
                    .user(if i % 2 == 0 { "alice" } else { "bob" })
                    .elapsed_ms(i as u64 * 1000)
            })
            .collect();
        let mut state = ProcessTableState::new();
        state.window.viewport = bench::VIEWPORT;
        state.select(Some(ProcessKey::new(2500, 2499)));
        state.reconcile(&rows);
        assert_eq!(state.selected(), Some(ProcessKey::new(2500, 2499)));
        let vis = state.visible_processes(&rows);
        assert_eq!(vis.len(), bench::PROCESS_COUNT);
        assert!(vis[0].cpu_pct >= vis[1].cpu_pct);

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        for _ in 0..8 {
            ProcessTable::new(&rows, &system).render(area, &mut buf, &mut state);
            let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
    }

    #[test]
    fn never_mentions_syscall_execution() {
        // Guard: production body must not spawn/kill.
        let src = include_str!("process_table.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "libc::kill",
            "nix::sys::signal",
            "Command::new",
            "std::process::Command",
        ] {
            assert!(
                !body.contains(forbidden),
                "process_table must not contain {forbidden}"
            );
        }
    }

    #[test]
    fn confirm_cancel() {
        let rows = sample();
        let mut state = ProcessTableState::new();
        state.select(Some(ProcessKey::new(1902, 500)));
        let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(matches!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ProcessTableOutcome::ConfirmCancelled
        ));
        assert!(state.pending_confirm.is_none());
    }

    #[test]
    fn header_columns_sit_over_their_own_data() {
        let system = DesignSystem::default();
        let rows = sample();
        let mut state = ProcessTableState::new();
        let area = Rect::new(0, 0, 72, 12);
        let mut buf = Buffer::empty(area);
        ProcessTable::new(&rows, &system).render(area, &mut buf, &mut state);
        let row_text = |y: u16| -> String {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        };
        let header = (0..area.height)
            .map(row_text)
            .find(|line| line.contains("PID"))
            .expect("header row");
        let header_y = (0..area.height)
            .find(|y| row_text(*y).contains("PID"))
            .expect("header row index");
        // `PID` is right-aligned in a 7-cell column; the first data row's pid
        // must end in the same column, not one cell to its left.
        let pid_end = header.find("PID").expect("PID header") + "PID".len();
        let data = row_text(header_y + 1);
        assert_eq!(
            data[..pid_end].trim_end().len(),
            pid_end,
            "pid column drifted left of its header: {data:?}"
        );
        assert_eq!(
            data.as_bytes()[pid_end],
            b' ',
            "pid column overruns its header: {data:?}"
        );
    }

    #[test]
    fn column_model_has_sortable_cpu() {
        let m = process_column_model();
        assert!(m.index_of(&"cpu").is_some());
    }
}
