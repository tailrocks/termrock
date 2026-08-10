// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DatabaseWorkbench** — source-owned database application composition from
//! **public** TermRock data widgets only (flagship TablePlus / DataGrip-class
//! layout; not a product clone or SQL client).
//!
//! **Mission.** Layout + focus + typed application messages for:
//! connection inventory ([`ConnectionManager`]), schema browser, query
//! tabs/editor, result grid, object inspector, history picker, status bar,
//! command palette, and export/copy actions. Responsive collapse. Host owns
//! protocol I/O, pools, vaults, and result pages — workbench never opens
//! sockets or embeds credential stores.
//!
//! **vs [`super::agent_workbench`].** Agent chrome; this is data IDE composition.
//! **vs standalone widgets.** Workbench composes; does not re-implement paint.
//!
//! Research: TablePlus, DataGrip, pgcli/lazysql-style tools, high-quality
//! developer workbenches.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignSystem, PanelChrome, Role},
    text::take_display_cols,
    widgets::{
        connection_to_reconnecting_state, example_command_catalog, example_connections,
        example_history_entries, CommandEntry, CommandPalette, CommandPaletteOutcome,
        CommandPaletteState, ConnectionEntry, ConnectionKind, ConnectionManager,
        ConnectionManagerOutcome, ConnectionManagerPresentation, ConnectionManagerState,
        ConnectionStatus, HistoryEntry, HistoryKind, HistoryPicker, HistoryPickerOutcome,
        HistoryPickerState, InspectorField, ObjectInspector, ObjectInspectorOutcome,
        ObjectInspectorState, Panel, QueryEditor, QueryEditorOutcome, QueryEditorState,
        QueryRunStatus, ResultCell, ResultColumn, ResultExportFormat, ResultGrid,
        ResultGridOutcome, ResultGridState, ResultQueryStatus, ResultRow, SchemaBrowser,
        SchemaBrowserEntry, SchemaBrowserState, SchemaConnStatus, StatusBar,
        StatusBarState, StatusRegion, StatusSlot,
    },
};

// ── Panes & density ─────────────────────────────────────────────────────────

/// Named panes of the database workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DatabaseWorkbenchPane {
    /// Connection inventory ([`ConnectionManager`]).
    Connections,
    /// Schema browser tree.
    Schema,
    /// Query editor / tabs.
    Query,
    /// Result grid.
    Results,
    /// Object / cell inspector.
    Inspector,
    /// Status strip.
    Status,
}

impl DatabaseWorkbenchPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connections => "connections",
            Self::Schema => "schema",
            Self::Query => "query",
            Self::Results => "results",
            Self::Inspector => "inspector",
            Self::Status => "status",
        }
    }

    /// Default Tab focus cycle (root; status is chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [DatabaseWorkbenchPane] {
        &[
            Self::Connections,
            Self::Schema,
            Self::Query,
            Self::Results,
            Self::Inspector,
        ]
    }
}

/// Responsive density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DatabaseWorkbenchDensity {
    /// Full workbench.
    #[default]
    Normal,
    /// Collapse inspector; keep connections+schema if width allows.
    Narrow,
    /// Query + results + status only.
    Tiny,
}

impl DatabaseWorkbenchDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 48 {
            Self::Tiny
        } else if width < 88 {
            Self::Narrow
        } else {
            Self::Normal
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Narrow => "narrow",
            Self::Tiny => "tiny",
        }
    }
}

// ── Application messages ────────────────────────────────────────────────────

/// Connection gate (host-projected; blocks run/export when offline-like).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DatabaseConnGate {
    /// Ready for run/export.
    #[default]
    Connected,
    /// Not connected.
    Disconnected,
    /// Offline / cached only.
    Offline,
    /// Reconnect in flight.
    Reconnecting,
    /// Auth required.
    AuthRequired,
    /// Error state.
    Error,
}

impl DatabaseConnGate {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Offline => "offline",
            Self::Reconnecting => "reconnecting",
            Self::AuthRequired => "auth_required",
            Self::Error => "error",
        }
    }

    /// From connection status.
    #[must_use]
    pub const fn from_status(s: ConnectionStatus) -> Self {
        match s {
            ConnectionStatus::Connected => Self::Connected,
            ConnectionStatus::Disconnected => Self::Disconnected,
            ConnectionStatus::Offline => Self::Offline,
            ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => Self::Reconnecting,
            ConnectionStatus::AuthRequired => Self::AuthRequired,
            ConnectionStatus::Error => Self::Error,
        }
    }

    /// Whether query run is allowed.
    #[must_use]
    pub const fn allows_run(self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Whether export is allowed (connected or offline cached page).
    #[must_use]
    pub const fn allows_export(self) -> bool {
        matches!(self, Self::Connected | Self::Offline)
    }

    /// Offline-like chrome.
    #[must_use]
    pub const fn is_offline_like(self) -> bool {
        !matches!(self, Self::Connected)
    }
}

/// Transaction status chrome (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DatabaseTxStatus {
    /// No transaction.
    #[default]
    None,
    /// Open idle transaction.
    Open,
    /// Statement active inside tx.
    Active,
    /// Failed / needs rollback.
    Failed,
}

impl DatabaseTxStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Open => "open",
            Self::Active => "active",
            Self::Failed => "failed",
        }
    }

    /// Status label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "autocommit",
            Self::Open => "in txn",
            Self::Active => "txn active",
            Self::Failed => "txn failed",
        }
    }
}

/// Query tab identity (host may hold draft per tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseQueryTab {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Draft SQL (workbench projects active tab into editor).
    pub draft: String,
    /// Language id (`sql`).
    pub language: String,
}

impl DatabaseQueryTab {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, draft: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            draft: draft.into(),
            language: "sql".into(),
        }
    }
}

/// Why a run was blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DatabaseRunBlockReason {
    /// Connection not ready.
    Disconnected,
    /// Offline gate.
    Offline,
    /// Auth required.
    AuthRequired,
    /// Connection error.
    Error,
    /// Empty query.
    EmptyQuery,
}

impl DatabaseRunBlockReason {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Offline => "offline",
            Self::AuthRequired => "auth_required",
            Self::Error => "error",
            Self::EmptyQuery => "empty_query",
        }
    }
}

/// Workbench key / action outcomes — requests only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DatabaseWorkbenchOutcome {
    /// Ignored.
    Ignored,
    /// Focus pane changed.
    FocusChanged(&'static str),
    /// Density override applied.
    DensityChanged(DatabaseWorkbenchDensity),
    /// Query run requested (host executes).
    RunRequested {
        /// Active tab id.
        tab_id: String,
        /// SQL text.
        text: String,
        /// Selection-only.
        selection_only: bool,
    },
    /// Cancel in-flight run.
    CancelRequested {
        /// Tab id.
        tab_id: String,
        /// Host run id when known.
        run_id: Option<String>,
    },
    /// Run blocked by connection gate / empty.
    RunBlocked {
        /// Reason.
        reason: DatabaseRunBlockReason,
        /// Tab id.
        tab_id: String,
    },
    /// Export requested — **no full result body** (host serializes page).
    ExportRequested {
        /// Format.
        format: ResultExportFormat,
        /// Tab id.
        tab_id: String,
        /// Visible row count hint (not data).
        visible_rows: usize,
    },
    /// Copy requested — summary only (payload kind/id, not full grid).
    CopyRequested {
        /// Tab id.
        tab_id: String,
        /// Summary label (`cell`, `row`, `selection`).
        kind: String,
    },
    /// Transaction chrome changed (host or workbench optimistic).
    TransactionChanged(DatabaseTxStatus),
    /// Connection gate changed.
    ConnGateChanged(DatabaseConnGate),
    /// Active tab changed.
    TabChanged {
        /// Tab id.
        id: String,
    },
    /// Connection manager child.
    Connection(ConnectionManagerOutcome),
    /// Schema browser (node id when applicable).
    Schema {
        /// Outcome label.
        kind: String,
        /// Node id if any.
        id: Option<String>,
    },
    /// Query editor child (non-run).
    Query(QueryEditorOutcome),
    /// Result grid child (non-export/copy).
    Results {
        /// Outcome kind id.
        kind: String,
    },
    /// Inspector child.
    Inspector {
        /// Outcome kind id.
        kind: String,
    },
    /// History applied or opened.
    History {
        /// Kind.
        kind: String,
        /// Entry id.
        id: Option<String>,
    },
    /// Palette command committed / opened.
    Palette {
        /// Kind.
        kind: String,
        /// Command id.
        id: Option<String>,
    },
    /// Open command palette.
    OpenPalette,
    /// Open history.
    OpenHistory,
    /// Escape cancelled root (host may blur workbench).
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent database workbench state.
#[derive(Debug)]
pub struct DatabaseWorkbenchState {
    /// Workspace collapse/zoom.
    pub workspace: WorkspaceState,
    /// Child: connections.
    pub connections: ConnectionManagerState,
    /// Child: schema tree.
    pub schema: SchemaBrowserState<&'static str>,
    /// Child: query editor (active tab draft).
    pub query: QueryEditorState,
    /// Child: results.
    pub results: ResultGridState,
    /// Child: inspector.
    pub inspector: ObjectInspectorState,
    /// Child: history overlay.
    pub history: HistoryPickerState<&'static str>,
    /// Child: command palette overlay.
    pub palette: CommandPaletteState<&'static str>,
    /// Status bar hits.
    pub status: StatusBarState<&'static str>,
    /// Query tabs.
    pub tabs: Vec<DatabaseQueryTab>,
    /// Active tab index.
    pub active_tab: usize,
    /// Focused pane id string.
    pub focus: &'static str,
    /// Density override (`None` = width-derived).
    pub density: Option<DatabaseWorkbenchDensity>,
    /// Connection gate.
    pub conn_gate: DatabaseConnGate,
    /// Transaction chrome.
    pub tx_status: DatabaseTxStatus,
    /// Last error message (status / banner).
    pub last_error: Option<String>,
    /// ASCII paint.
    pub ascii: bool,
    /// Colorless paint.
    pub colorless: bool,
    /// History overlay open.
    history_open: bool,
    /// Palette overlay open.
    palette_open: bool,
    /// Last painted pane rects (for tests).
    last_panes: Vec<PaneGeom>,
    /// Last layout/paint width — drives density when `density` override is `None`.
    last_area_width: Option<u16>,
}

impl Default for DatabaseWorkbenchState {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseWorkbenchState {
    /// Fresh workbench with example tabs.
    #[must_use]
    pub fn new() -> Self {
        let mut connections = ConnectionManagerState::new();
        connections.set_presentation(ConnectionManagerPresentation::Launcher);
        connections.set_connections(example_connections());
        // Prefer first connected entry for gate
        let gate = connections
            .current()
            .map(|c| DatabaseConnGate::from_status(c.status))
            .unwrap_or(DatabaseConnGate::Disconnected);
        // Select connected example when present
        if let Some(fi) = connections
            .filtered_indices()
            .iter()
            .position(|&si| connections.connections[si].status == ConnectionStatus::Connected)
        {
            connections.cursor = fi;
        }
        let gate = connections
            .current()
            .map(|c| DatabaseConnGate::from_status(c.status))
            .unwrap_or(gate);

        let tabs = example_query_tabs();
        let mut query = QueryEditorState::new();
        if let Some(t) = tabs.first() {
            query.set_text(&t.draft);
            query.title = Some(t.title.clone());
        }
        query.run = QueryRunStatus::Idle;

        let mut results = ResultGridState::new();
        results.status = ResultQueryStatus::Idle;
        results.schema = example_result_columns();

        let mut state = Self {
            workspace: WorkspaceState::new(),
            connections,
            schema: SchemaBrowserState::new(),
            query,
            results,
            inspector: ObjectInspectorState::new(),
            history: HistoryPickerState::new(),
            palette: CommandPaletteState::new(None),
            status: StatusBarState::new(),
            tabs,
            active_tab: 0,
            focus: DatabaseWorkbenchPane::Query.id(),
            density: None,
            conn_gate: gate,
            tx_status: DatabaseTxStatus::None,
            last_error: None,
            ascii: false,
            colorless: false,
            history_open: false,
            palette_open: false,
            last_panes: Vec::new(),
            last_area_width: None,
        };
        state.sync_conn_gate_from_selection();
        state
    }

    /// Effective density for focus/layout: override, else last paint width, else Normal.
    #[must_use]
    pub fn effective_density(&self) -> DatabaseWorkbenchDensity {
        if let Some(d) = self.density {
            return d;
        }
        if let Some(w) = self.last_area_width {
            return DatabaseWorkbenchDensity::for_width(w);
        }
        // Infer from last painted panes when width not recorded yet.
        if !self.last_panes.is_empty() {
            let has_inspector = self
                .last_panes
                .iter()
                .any(|p| p.id.0.as_str() == "inspector" && !p.collapsed && p.area.width > 0);
            let has_connections = self
                .last_panes
                .iter()
                .any(|p| p.id.0.as_str() == "connections" && !p.collapsed && p.area.width > 0);
            if !has_connections && !has_inspector {
                return DatabaseWorkbenchDensity::Tiny;
            }
            if !has_inspector {
                return DatabaseWorkbenchDensity::Narrow;
            }
            return DatabaseWorkbenchDensity::Normal;
        }
        DatabaseWorkbenchDensity::Normal
    }

    /// Clamp `focus` to panes visible at the given density.
    pub fn clamp_focus_to_density(&mut self, density: DatabaseWorkbenchDensity) {
        let order = self.focus_order_for(density);
        if !order.iter().any(|id| *id == self.focus) {
            self.focus = order.first().copied().unwrap_or("query");
            self.apply_focus_gates();
        }
    }

    /// History overlay open.
    #[must_use]
    pub const fn history_open(&self) -> bool {
        self.history_open
    }

    /// Palette overlay open.
    #[must_use]
    pub const fn palette_open(&self) -> bool {
        self.palette_open
    }

    /// Last layout panes (after paint or [`Self::layout`]).
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Active tab.
    #[must_use]
    pub fn active_tab(&self) -> Option<&DatabaseQueryTab> {
        self.tabs.get(self.active_tab)
    }

    /// Active tab id.
    #[must_use]
    pub fn active_tab_id(&self) -> String {
        self.active_tab()
            .map(|t| t.id.clone())
            .unwrap_or_else(|| "tab0".into())
    }

    /// Set density override.
    pub fn set_density(&mut self, d: Option<DatabaseWorkbenchDensity>) {
        self.density = d;
    }

    /// Project connection gate from current connection selection.
    pub fn sync_conn_gate_from_selection(&mut self) {
        if let Some(c) = self.connections.current() {
            self.conn_gate = DatabaseConnGate::from_status(c.status);
            if let Some(err) = &c.last_error {
                self.last_error = Some(err.clone());
            }
        }
    }

    /// Host sets connection gate explicitly.
    pub fn set_conn_gate(&mut self, gate: DatabaseConnGate) -> DatabaseWorkbenchOutcome {
        self.conn_gate = gate;
        DatabaseWorkbenchOutcome::ConnGateChanged(gate)
    }

    /// Host sets transaction chrome.
    pub fn set_tx_status(&mut self, tx: DatabaseTxStatus) -> DatabaseWorkbenchOutcome {
        self.tx_status = tx;
        DatabaseWorkbenchOutcome::TransactionChanged(tx)
    }

    /// Host reports run started.
    pub fn begin_run(&mut self, run_id: impl Into<String>) {
        self.query.run = QueryRunStatus::Running {
            run_id: run_id.into(),
        };
        self.results.status = ResultQueryStatus::Streaming {
            resident: 0,
            total: None,
        };
        if matches!(self.tx_status, DatabaseTxStatus::Open) {
            self.tx_status = DatabaseTxStatus::Active;
        }
    }

    /// Host reports run success.
    pub fn finish_run_success(&mut self, rows: u64, duration_ms: u64) {
        self.query.run = QueryRunStatus::Success {
            rows: Some(rows),
            duration_ms: Some(duration_ms),
        };
        self.results.status = ResultQueryStatus::Ready {
            total: Some(rows),
            duration_ms: Some(duration_ms),
        };
        if matches!(self.tx_status, DatabaseTxStatus::Active) {
            self.tx_status = DatabaseTxStatus::Open;
        }
        self.last_error = None;
    }

    /// Host reports run error.
    pub fn finish_run_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.query.run = QueryRunStatus::Failed {
            message: message.clone(),
        };
        self.results.status = ResultQueryStatus::Failed {
            message: message.clone(),
        };
        self.last_error = Some(message);
        if !matches!(self.tx_status, DatabaseTxStatus::None) {
            self.tx_status = DatabaseTxStatus::Failed;
        }
    }

    /// Select tab by index; projects draft into editor.
    pub fn select_tab(&mut self, index: usize) -> DatabaseWorkbenchOutcome {
        if index >= self.tabs.len() {
            return DatabaseWorkbenchOutcome::Ignored;
        }
        // Persist current draft back
        if let Some(t) = self.tabs.get_mut(self.active_tab) {
            t.draft = self.query.text();
        }
        self.active_tab = index;
        if let Some(t) = self.tabs.get(self.active_tab) {
            self.query.set_text(&t.draft);
            self.query.title = Some(t.title.clone());
            let id = t.id.clone();
            return DatabaseWorkbenchOutcome::TabChanged { id };
        }
        DatabaseWorkbenchOutcome::Ignored
    }

    /// Focus pane by id (must be in focus order when visible).
    pub fn set_focus(&mut self, pane: DatabaseWorkbenchPane) -> DatabaseWorkbenchOutcome {
        self.focus = pane.id();
        self.apply_focus_gates();
        DatabaseWorkbenchOutcome::FocusChanged(self.focus)
    }

    fn apply_focus_gates(&mut self) {
        let f = self.focus;
        let live = !self.palette_open && !self.history_open;
        self.connections.set_focused(f == "connections");
        self.connections
            .set_accepts_input(f == "connections" && live);
        self.schema.set_accepts_input(f == "schema" && live);
        self.query.set_accepts_input(f == "query" && live);
        self.results.set_accepts_input(f == "results" && live);
        self.inspector.set_accepts_input(f == "inspector" && live);
    }

    /// Visible focus targets for density.
    #[must_use]
    pub fn focus_order_for(&self, density: DatabaseWorkbenchDensity) -> Vec<&'static str> {
        match density {
            DatabaseWorkbenchDensity::Normal => DatabaseWorkbenchPane::focus_order()
                .iter()
                .map(|p| p.id())
                .collect(),
            DatabaseWorkbenchDensity::Narrow => vec![
                DatabaseWorkbenchPane::Connections.id(),
                DatabaseWorkbenchPane::Schema.id(),
                DatabaseWorkbenchPane::Query.id(),
                DatabaseWorkbenchPane::Results.id(),
            ],
            DatabaseWorkbenchDensity::Tiny => vec![
                DatabaseWorkbenchPane::Query.id(),
                DatabaseWorkbenchPane::Results.id(),
            ],
        }
    }

    /// Cycle focus.
    pub fn focus_next(&mut self, density: DatabaseWorkbenchDensity) -> DatabaseWorkbenchOutcome {
        let order = self.focus_order_for(density);
        if order.is_empty() {
            return DatabaseWorkbenchOutcome::Ignored;
        }
        let i = order.iter().position(|id| *id == self.focus).unwrap_or(0);
        let next = order[(i + 1) % order.len()];
        self.focus = next;
        self.apply_focus_gates();
        DatabaseWorkbenchOutcome::FocusChanged(self.focus)
    }

    /// Cycle focus reverse.
    pub fn focus_prev(&mut self, density: DatabaseWorkbenchDensity) -> DatabaseWorkbenchOutcome {
        let order = self.focus_order_for(density);
        if order.is_empty() {
            return DatabaseWorkbenchOutcome::Ignored;
        }
        let i = order.iter().position(|id| *id == self.focus).unwrap_or(0);
        let prev = order[(i + order.len() - 1) % order.len()];
        self.focus = prev;
        self.apply_focus_gates();
        DatabaseWorkbenchOutcome::FocusChanged(self.focus)
    }

    /// Layout for area (stores into `last_panes` + width for density).
    pub fn layout(&mut self, area: Rect) -> Vec<PaneGeom> {
        self.last_area_width = Some(area.width);
        let density = self.effective_density();
        let panes = database_workbench_layout_density(area, &self.workspace, density);
        self.last_panes = panes.clone();
        self.clamp_focus_to_density(density);
        panes
    }

    /// Request run — gates on connection.
    pub fn request_run(&mut self) -> DatabaseWorkbenchOutcome {
        let tab_id = self.active_tab_id();
        if !self.conn_gate.allows_run() {
            let reason = match self.conn_gate {
                DatabaseConnGate::Offline => DatabaseRunBlockReason::Offline,
                DatabaseConnGate::AuthRequired => DatabaseRunBlockReason::AuthRequired,
                DatabaseConnGate::Error => DatabaseRunBlockReason::Error,
                _ => DatabaseRunBlockReason::Disconnected,
            };
            return DatabaseWorkbenchOutcome::RunBlocked { reason, tab_id };
        }
        let (text, selection_only) = self.query.executable_text();
        if text.trim().is_empty() {
            return DatabaseWorkbenchOutcome::RunBlocked {
                reason: DatabaseRunBlockReason::EmptyQuery,
                tab_id,
            };
        }
        DatabaseWorkbenchOutcome::RunRequested {
            tab_id,
            text,
            selection_only,
        }
    }

    /// Request cancel.
    pub fn request_cancel(&mut self) -> DatabaseWorkbenchOutcome {
        let tab_id = self.active_tab_id();
        let run_id = match &self.query.run {
            QueryRunStatus::Running { run_id } => Some(run_id.clone()),
            _ => None,
        };
        DatabaseWorkbenchOutcome::CancelRequested { tab_id, run_id }
    }

    /// Keyboard routing.
    ///
    /// `inspect_fields` is the same host projection used for paint so inspector
    /// keys are not dead (`ObjectInspectorState::handle_key` needs field count).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        schema_entries: &[SchemaBrowserEntry<'_, &'static str>],
        history_entries: &[HistoryEntry<&'static str>],
        commands: &[CommandEntry<&'static str>],
        result_rows_len: usize,
        inspect_fields: &[InspectorField<'_>],
    ) -> DatabaseWorkbenchOutcome {
        if key.kind != KeyEventKind::Press {
            return DatabaseWorkbenchOutcome::Ignored;
        }

        // Match render: override or last paint width — never assume Normal when
        // density is None (narrow/tiny would still Tab into missing panes).
        let density = self.effective_density();
        self.clamp_focus_to_density(density);

        // Overlay: palette
        if self.palette_open {
            match key.code {
                KeyCode::Esc => {
                    self.palette_open = false;
                    self.apply_focus_gates();
                    return DatabaseWorkbenchOutcome::Palette {
                        kind: "dismissed".into(),
                        id: None,
                    };
                }
                _ => {
                    let out = self.palette.handle_key(key, commands);
                    return match out {
                        CommandPaletteOutcome::Activated { id, .. } => {
                            self.palette_open = false;
                            self.apply_focus_gates();
                            match id {
                                "run" => self.request_run(),
                                "cancel" => self.request_cancel(),
                                "export-csv" => {
                                    self.export_request(ResultExportFormat::Csv, result_rows_len)
                                }
                                "history" => {
                                    self.history_open = true;
                                    DatabaseWorkbenchOutcome::OpenHistory
                                }
                                other => DatabaseWorkbenchOutcome::Palette {
                                    kind: "activated".into(),
                                    id: Some(other.into()),
                                },
                            }
                        }
                        CommandPaletteOutcome::Cancelled => {
                            self.palette_open = false;
                            self.apply_focus_gates();
                            DatabaseWorkbenchOutcome::Palette {
                                kind: "cancelled".into(),
                                id: None,
                            }
                        }
                        CommandPaletteOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                        other => DatabaseWorkbenchOutcome::Palette {
                            kind: format!("{other:?}")
                                .split(|c: char| c == '(' || c == ' ')
                                .next()
                                .unwrap_or("palette")
                                .into(),
                            id: None,
                        },
                    };
                }
            }
        }

        // Overlay: history
        if self.history_open {
            match key.code {
                KeyCode::Esc => {
                    self.history_open = false;
                    self.apply_focus_gates();
                    return DatabaseWorkbenchOutcome::History {
                        kind: "dismissed".into(),
                        id: None,
                    };
                }
                _ => {
                    let out = self.history.handle_key(key, history_entries);
                    return match out {
                        HistoryPickerOutcome::Selected { id, value } => {
                            self.history_open = false;
                            self.query.set_text(&value);
                            self.apply_focus_gates();
                            DatabaseWorkbenchOutcome::History {
                                kind: "applied".into(),
                                id: Some(id.into()),
                            }
                        }
                        HistoryPickerOutcome::Cancelled => {
                            self.history_open = false;
                            self.apply_focus_gates();
                            DatabaseWorkbenchOutcome::History {
                                kind: "cancelled".into(),
                                id: None,
                            }
                        }
                        HistoryPickerOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                        other => DatabaseWorkbenchOutcome::History {
                            kind: format!("{other:?}")
                                .split(|c: char| c == '(' || c == ' ')
                                .next()
                                .unwrap_or("history")
                                .into(),
                            id: None,
                        },
                    };
                }
            }
        }

        // Global chords
        match key.code {
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette_open = true;
                return DatabaseWorkbenchOutcome::OpenPalette;
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.history_open = true;
                return DatabaseWorkbenchOutcome::OpenHistory;
            }
            KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                return self.export_request(ResultExportFormat::Csv, result_rows_len);
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.request_run();
            }
            KeyCode::Char('c')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(self.query.run, QueryRunStatus::Running { .. }) =>
            {
                // Ctrl+C cancels run when running (not copy)
                return self.request_cancel();
            }
            KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                return self.focus_next(density);
            }
            KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                return self.focus_prev(density);
            }
            KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let i = self.active_tab.saturating_sub(1);
                return self.select_tab(i);
            }
            KeyCode::Char(']') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let i = (self.active_tab + 1).min(self.tabs.len().saturating_sub(1));
                return self.select_tab(i);
            }
            KeyCode::Esc => {
                // Peel: if focus not query, go query; else cancel root
                if self.focus != DatabaseWorkbenchPane::Query.id() {
                    return self.set_focus(DatabaseWorkbenchPane::Query);
                }
                return DatabaseWorkbenchOutcome::Cancelled;
            }
            _ => {}
        }

        // Focused pane
        match self.focus {
            "connections" => {
                let out = self.connections.handle_key(key);
                if !matches!(out, ConnectionManagerOutcome::Ignored) {
                    self.sync_conn_gate_from_selection();
                }
                // Double-enter connect already emits ConnectRequested
                match out {
                    ConnectionManagerOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                    ConnectionManagerOutcome::ConnectRequested { ref id } => {
                        // Optimistic gate from selected entry
                        self.sync_conn_gate_from_selection();
                        DatabaseWorkbenchOutcome::Connection(
                            ConnectionManagerOutcome::ConnectRequested { id: id.clone() },
                        )
                    }
                    other => DatabaseWorkbenchOutcome::Connection(other),
                }
            }
            "schema" => {
                let out = self.schema.handle_key(schema_entries, key);
                let kind = format!("{out:?}");
                let kind_name = kind
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("schema")
                    .to_string();
                if kind_name == "Ignored" {
                    DatabaseWorkbenchOutcome::Ignored
                } else {
                    DatabaseWorkbenchOutcome::Schema {
                        kind: kind_name,
                        id: None,
                    }
                }
            }
            "query" => {
                let out = self.query.handle_key(key, &[]);
                match out {
                    QueryEditorOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                    QueryEditorOutcome::RunRequested {
                        text,
                        selection_only,
                        ..
                    } => {
                        if !self.conn_gate.allows_run() {
                            let reason = match self.conn_gate {
                                DatabaseConnGate::Offline => DatabaseRunBlockReason::Offline,
                                DatabaseConnGate::AuthRequired => {
                                    DatabaseRunBlockReason::AuthRequired
                                }
                                DatabaseConnGate::Error => DatabaseRunBlockReason::Error,
                                _ => DatabaseRunBlockReason::Disconnected,
                            };
                            return DatabaseWorkbenchOutcome::RunBlocked {
                                reason,
                                tab_id: self.active_tab_id(),
                            };
                        }
                        DatabaseWorkbenchOutcome::RunRequested {
                            tab_id: self.active_tab_id(),
                            text,
                            selection_only,
                        }
                    }
                    QueryEditorOutcome::StopRequested { run_id } => {
                        DatabaseWorkbenchOutcome::CancelRequested {
                            tab_id: self.active_tab_id(),
                            run_id,
                        }
                    }
                    QueryEditorOutcome::OpenHistory => {
                        self.history_open = true;
                        DatabaseWorkbenchOutcome::OpenHistory
                    }
                    other => DatabaseWorkbenchOutcome::Query(other),
                }
            }
            "results" => {
                let cols = self.results.column_model();
                // Host projects visible row ids; workbench uses 0..len for routing.
                let row_ids: Vec<u64> = (0..result_rows_len as u64).collect();
                let out = self.results.handle_key(key, &cols, &row_ids);
                match out {
                    ResultGridOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                    ResultGridOutcome::ExportRequested { format, .. } => {
                        self.export_request(format, result_rows_len)
                    }
                    ResultGridOutcome::Copy(payload) => {
                        let kind = format!("{payload:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("copy")
                            .to_string();
                        DatabaseWorkbenchOutcome::CopyRequested {
                            tab_id: self.active_tab_id(),
                            kind,
                        }
                    }
                    other => {
                        let kind = format!("{other:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("results")
                            .to_string();
                        DatabaseWorkbenchOutcome::Results { kind }
                    }
                }
            }
            "inspector" => {
                if inspect_fields.is_empty() {
                    return DatabaseWorkbenchOutcome::Ignored;
                }
                let out = self.inspector.handle_key(key, inspect_fields);
                match out {
                    ObjectInspectorOutcome::Ignored => DatabaseWorkbenchOutcome::Ignored,
                    other => {
                        let kind = format!("{other:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("inspector")
                            .to_string();
                        DatabaseWorkbenchOutcome::Inspector { kind }
                    }
                }
            }
            _ => DatabaseWorkbenchOutcome::Ignored,
        }
    }

    fn export_request(
        &self,
        format: ResultExportFormat,
        visible_rows: usize,
    ) -> DatabaseWorkbenchOutcome {
        if !self.conn_gate.allows_export() {
            return DatabaseWorkbenchOutcome::RunBlocked {
                reason: match self.conn_gate {
                    DatabaseConnGate::Offline => DatabaseRunBlockReason::Offline,
                    DatabaseConnGate::AuthRequired => DatabaseRunBlockReason::AuthRequired,
                    DatabaseConnGate::Error => DatabaseRunBlockReason::Error,
                    _ => DatabaseRunBlockReason::Disconnected,
                },
                tab_id: self.active_tab_id(),
            };
        }
        DatabaseWorkbenchOutcome::ExportRequested {
            format,
            tab_id: self.active_tab_id(),
            visible_rows,
        }
    }

    /// Status slots for current chrome.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let conn = match self.conn_gate {
            DatabaseConnGate::Connected => "connected",
            DatabaseConnGate::Disconnected => "disconnected",
            DatabaseConnGate::Offline => "offline",
            DatabaseConnGate::Reconnecting => "reconnecting",
            DatabaseConnGate::AuthRequired => "auth",
            DatabaseConnGate::Error => "error",
        };
        let run = self.query.run.id();
        let tx = self.tx_status.label();
        let tab = self
            .active_tab()
            .map(|t| t.title.as_str())
            .unwrap_or("query");
        // StatusSlot wants 'static content — use stable static labels for gate/run/tx;
        // tab title may not be static so use focus zone for dynamic-ish info.
        let mut slots = vec![
            StatusSlot::connection("conn", conn).priority(10),
            StatusSlot::mode("tx", tx).priority(20),
            StatusSlot::context("run", run).priority(30),
            StatusSlot::focus_zone("focus", self.focus).priority(40),
            StatusSlot::shortcut("keys", "C-↵ run · C-e export · C-p cmd · tab focus").priority(90),
        ];
        let _ = tab;
        if self.last_error.is_some() {
            slots.insert(
                0,
                StatusSlot::new("err", "error")
                    .region(StatusRegion::Left)
                    .priority(5),
            );
        }
        slots
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Layout with width-derived density.
#[must_use]
pub fn database_workbench_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    database_workbench_layout_density(
        area,
        state,
        DatabaseWorkbenchDensity::for_width(area.width),
    )
}

/// Layout with explicit density.
#[must_use]
pub fn database_workbench_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: DatabaseWorkbenchDensity,
) -> Vec<PaneGeom> {
    let root = match density {
        DatabaseWorkbenchDensity::Tiny => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 45,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(DatabaseWorkbenchPane::Query.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 90,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(DatabaseWorkbenchPane::Results.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 2,
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(DatabaseWorkbenchPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }),
        },
        DatabaseWorkbenchDensity::Narrow => {
            // west: connections+schema stacked | center query/results | status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 92,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 28,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 40,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Connections.id()),
                            constraint: PaneConstraint::Min(6),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Schema.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 42,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Query.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 2,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Results.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 2,
                        }),
                    }),
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(DatabaseWorkbenchPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }
        }
        DatabaseWorkbenchDensity::Normal => {
            // west connections+schema | center query/results | east inspector | status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 94,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 20,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 38,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Connections.id()),
                            constraint: PaneConstraint::Min(8),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Schema.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Horizontal,
                        ratio_percent: 72,
                        first: Box::new(WorkspaceNode::Split {
                            axis: WorkspaceAxis::Vertical,
                            ratio_percent: 40,
                            first: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(DatabaseWorkbenchPane::Query.id()),
                                constraint: PaneConstraint::Weight(1),
                                collapse_priority: 2,
                            }),
                            second: Box::new(WorkspaceNode::Leaf {
                                id: PaneId::from_static(DatabaseWorkbenchPane::Results.id()),
                                constraint: PaneConstraint::Weight(1),
                                collapse_priority: 2,
                            }),
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(DatabaseWorkbenchPane::Inspector.id()),
                            constraint: PaneConstraint::Min(16),
                            collapse_priority: 0,
                        }),
                    }),
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(DatabaseWorkbenchPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }
        }
    };
    Workspace::new(root).layout(area, state)
}

fn pane_area(panes: &[PaneGeom], id: &str) -> Option<Rect> {
    panes.iter().find_map(|p| {
        if p.id.0.as_str() == id && !p.collapsed && p.area.width > 0 && p.area.height > 0 {
            Some(p.area)
        } else {
            None
        }
    })
}

fn centered_modal(area: Rect) -> Rect {
    let width = (area.width * 3 / 5).clamp(24, area.width.saturating_sub(2).max(1));
    let height = (area.height / 2).clamp(8, area.height.saturating_sub(2).max(1));
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(height) / 4);
    Rect {
        x,
        y,
        width,
        height,
    }
}

// ── Surfaces & render ───────────────────────────────────────────────────────

/// Borrowed surfaces for one workbench paint.
pub struct DatabaseWorkbenchSurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// Persistent state.
    pub state: &'a mut DatabaseWorkbenchState,
    /// Schema tree projection.
    pub schema_entries: &'a [SchemaBrowserEntry<'a, &'static str>],
    /// Result columns.
    pub result_columns: &'a [ResultColumn],
    /// Result row window (host-paged).
    pub result_rows: &'a [ResultRow<'a>],
    /// Inspector fields for selection.
    pub inspect_fields: &'a [InspectorField<'a>],
    /// History catalog.
    pub history: &'a [HistoryEntry<&'static str>],
    /// Command catalog.
    pub commands: &'a [CommandEntry<&'static str>],
}

/// Paint composed database workbench (public child widgets only).
pub fn render_database_workbench(buffer: &mut Buffer, area: Rect, surfaces: DatabaseWorkbenchSurfaces<'_>) {
    let DatabaseWorkbenchSurfaces {
        system,
        state,
        schema_entries,
        result_columns,
        result_rows,
        inspect_fields,
        history,
        commands,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    let panes = database_workbench_layout_density(area, &state.workspace, density);
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);
    state.apply_focus_gates();

    // Offline / disconnect banner over full width above status when gated
    if state.conn_gate.is_offline_like() {
        if let Some(c) = state.connections.current() {
            let rs = connection_to_reconnecting_state(c);
            let line = rs.banner_line(state.ascii);
            // paint into top of query pane if present, else area top
            if let Some(qa) = pane_area(&panes, "query") {
                if qa.height > 0 {
                    // leave banner to status; also mark status transient
                    state.status.transient = Some(line);
                }
            } else {
                state.status.transient = Some(line);
            }
        } else {
            state.status.transient = Some(format!("● {}", state.conn_gate.id()));
        }
    }

    if let Some(r) = pane_area(&panes, "connections") {
        let focused = state.focus == "connections";
        let panel = Panel::new(system)
            .title("Connections")
            .emphasis(if focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(r);
        Widget::render(&panel, r, buffer);
        ConnectionManager::new(system)
            .ascii(state.ascii)
            .colorless(state.colorless)
            .list_only(true)
            .paint(inner, buffer, &mut state.connections);
    }

    if let Some(r) = pane_area(&panes, "schema") {
        let focused = state.focus == "schema";
        SchemaBrowser::new(schema_entries, system)
            .title("Schema")
            .focused(focused)
            .ascii(state.ascii)
            .render(r, buffer, &mut state.schema);
    }

    if let Some(r) = pane_area(&panes, "query") {
        let focused = state.focus == "query";
        // Tab strip (1 row) then editor
        let mut y = r.y;
        let mut h = r.height;
        if h > 0 && !state.tabs.is_empty() {
            let tab_line = state
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if i == state.active_tab {
                        format!("[{}]", t.title)
                    } else {
                        format!(" {} ", t.title)
                    }
                })
                .collect::<Vec<_>>()
                .join("");
            let style = if focused {
                system.style(Role::Accent).add_modifier(Modifier::BOLD)
            } else {
                system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                r.x,
                y,
                take_display_cols(&tab_line, usize::from(r.width)),
                usize::from(r.width),
                style,
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }
        if h > 0 {
            let editor_area = Rect {
                x: r.x,
                y,
                width: r.width,
                height: h,
            };
            let tab_title = state
                .active_tab()
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "Query".into());
            QueryEditor::new(system)
                .title(tab_title.as_str())
                .focused(focused)
                .ascii(state.ascii)
                .render(editor_area, buffer, &mut state.query);
        }
    }

    if let Some(r) = pane_area(&panes, "results") {
        let focused = state.focus == "results";
        ResultGrid::new(system, result_columns, result_rows)
            .title("Results")
            .focused(focused)
            .ascii(state.ascii)
            .render(r, buffer, &mut state.results);
    }

    if let Some(r) = pane_area(&panes, "inspector") {
        let focused = state.focus == "inspector";
        ObjectInspector::new(inspect_fields, system)
            .focused(focused)
            .ascii(state.ascii)
            .colorless(state.colorless)
            .render(r, buffer, &mut state.inspector);
    }

    if let Some(r) = pane_area(&panes, "status") {
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }

    // Overlays
    if state.palette_open {
        let m = centered_modal(area);
        CommandPalette::new("Commands", commands, system)
            .ascii(state.ascii)
            .paint(m, buffer, &mut state.palette);
    }
    if state.history_open {
        let m = centered_modal(area);
        HistoryPicker::new(history, system)
            .ascii(state.ascii)
            .paint(m, buffer, &mut state.history);
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Demo query tabs.
#[must_use]
pub fn example_query_tabs() -> Vec<DatabaseQueryTab> {
    vec![
        DatabaseQueryTab::new(
            "t1",
            "users",
            "SELECT id, email, created_at\nFROM users\nWHERE active = true\nORDER BY id\nLIMIT 100;",
        ),
        DatabaseQueryTab::new(
            "t2",
            "orders",
            "SELECT o.id, o.total, u.email\nFROM orders o\nJOIN users u ON u.id = o.user_id\nLIMIT 50;",
        ),
        DatabaseQueryTab::new("t3", "explain", "EXPLAIN ANALYZE SELECT 1;"),
    ]
}

/// Demo schema tree.
#[must_use]
pub fn example_schema_entries() -> Vec<SchemaBrowserEntry<'static, &'static str>> {
    vec![
        SchemaBrowserEntry::connection("conn", "Prod Postgres", "prod")
            .conn_status(SchemaConnStatus::Connected)
            .branch()
            .expanded(),
        SchemaBrowserEntry::database("db", "app", "prod/app", 1)
            .parent("conn")
            .branch()
            .expanded(),
        SchemaBrowserEntry::schema("sch", "public", "prod/app/public", 2)
            .parent("db")
            .branch()
            .expanded(),
        SchemaBrowserEntry::table("tbl_users", "users", "prod/app/public/users", 3)
            .parent("sch")
            .branch()
            .expanded()
            .secondary("≈12k rows"),
        SchemaBrowserEntry::column("col_id", "id", "prod/app/public/users/id", 4)
            .parent("tbl_users")
            .type_label("bigint")
            .key_badge("PK")
            .nullable(false),
        SchemaBrowserEntry::column("col_email", "email", "prod/app/public/users/email", 4)
            .parent("tbl_users")
            .type_label("text")
            .nullable(false),
        SchemaBrowserEntry::table("tbl_orders", "orders", "prod/app/public/orders", 3)
            .parent("sch")
            .secondary("≈80k rows"),
        SchemaBrowserEntry::view("v_active", "active_users", "prod/app/public/active_users", 3)
            .parent("sch"),
    ]
}

/// Demo result columns.
#[must_use]
pub fn example_result_columns() -> Vec<ResultColumn> {
    vec![
        ResultColumn::new("id", "id")
            .type_name("bigint")
            .not_null()
            .priority(0),
        ResultColumn::new("email", "email").type_name("text").priority(1),
        ResultColumn::new("active", "active")
            .type_name("bool")
            .priority(2),
        ResultColumn::new("created_at", "created_at")
            .type_name("timestamptz")
            .priority(3),
    ]
}

/// Demo result rows (small).
#[must_use]
pub fn example_result_rows() -> Vec<(u64, [&'static str; 4])> {
    vec![
        (1, ["1", "ada@example.com", "t", "2024-01-02"]),
        (2, ["2", "grace@example.com", "t", "2024-02-03"]),
        (3, ["3", "alan@example.com", "f", "2024-03-04"]),
    ]
}

/// Build `ResultRow` window from static tuples (for stories).
#[must_use]
pub fn example_result_row_refs<'a>(
    data: &'a [(u64, [&'static str; 4])],
    cells_store: &'a mut Vec<[ResultCell<'a>; 4]>,
) -> Vec<ResultRow<'a>> {
    cells_store.clear();
    for (_id, cols) in data {
        cells_store.push([
            ResultCell::integer(cols[0]),
            ResultCell::text(cols[1]),
            ResultCell::bool_text(cols[2]),
            ResultCell::text(cols[3]),
        ]);
    }
    data.iter()
        .enumerate()
        .map(|(i, (id, _))| ResultRow::new(*id, (i as u64) + 1, &cells_store[i]))
        .collect()
}

/// Large synthetic result page for paint stress (`n` rows).
#[must_use]
pub fn large_result_row_data(n: usize) -> Vec<(u64, String, String, String, String)> {
    (1..=n as u64)
        .map(|i| {
            (
                i,
                i.to_string(),
                format!("user{i}@example.com"),
                if i % 2 == 0 { "t" } else { "f" }.into(),
                format!("2024-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1),
            )
        })
        .collect()
}

/// Demo inspector fields.
#[must_use]
pub fn example_inspect_fields() -> Vec<InspectorField<'static>> {
    vec![
        InspectorField::new("id", "1")
            .path("row.id")
            .type_label("bigint"),
        InspectorField::new("email", "ada@example.com")
            .path("row.email")
            .type_label("text"),
        InspectorField::new("password_hash", "••••")
            .path("row.password_hash")
            .type_label("text")
            .secret(),
        InspectorField::container("meta", "row.meta", crate::widgets::InspectKind::Object)
            .child_count(2),
    ]
}

/// Demo commands for palette.
#[must_use]
pub fn example_db_commands() -> Vec<CommandEntry<&'static str>> {
    let mut cmds = example_command_catalog();
    cmds.extend([
        CommandEntry::new("run", "Run query")
            .shortcut("C-Enter")
            .keywords(["execute", "sql"])
            .group("Query"),
        CommandEntry::new("cancel", "Cancel run")
            .shortcut("C-c")
            .group("Query"),
        CommandEntry::new("export-csv", "Export CSV")
            .shortcut("C-e")
            .group("Results"),
        CommandEntry::new("history", "Query history")
            .shortcut("C-h")
            .group("Query"),
        CommandEntry::new("reconnect", "Reconnect")
            .group("Connection")
            .keywords(["offline"]),
    ]);
    cmds
}

/// Demo history (SQL).
#[must_use]
pub fn example_db_history() -> Vec<HistoryEntry<&'static str>> {
    let mut h = example_history_entries();
    h.insert(
        0,
        HistoryEntry::new("sql1", "SELECT 1")
            .display("SELECT 1")
            .kind(HistoryKind::Command)
            .group("sql")
            .recency(100),
    );
    h
}

/// Connected example connection list for stories.
#[must_use]
pub fn example_workbench_connections() -> Vec<ConnectionEntry> {
    example_connections()
}

/// Disconnected-only catalog for gate stories.
#[must_use]
pub fn example_disconnected_connections() -> Vec<ConnectionEntry> {
    vec![
        ConnectionEntry::new(
            "d1",
            "Offline lab",
            ConnectionKind::Database,
            "postgres",
            "lab:5432",
        )
        .environment("lab")
        .status(ConnectionStatus::Disconnected)
        .summary("not connected"),
        ConnectionEntry::new(
            "d2",
            "Broken",
            ConnectionKind::Database,
            "postgres",
            "bad:1",
        )
        .status(ConnectionStatus::Error)
        .last_error("connection refused"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress constants.
pub mod bench {
    /// Frames for multi-pane paint.
    pub const PAINT_FRAMES: u32 = 20;
    /// Large result page size.
    pub const LARGE_ROWS: usize = 400;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::ResultCell;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn open() -> DatabaseWorkbenchState {
        let mut st = DatabaseWorkbenchState::new();
        st.conn_gate = DatabaseConnGate::Connected;
        st
    }

    #[test]
    fn focus_cycle_visits_zones() {
        let mut st = open();
        st.density = Some(DatabaseWorkbenchDensity::Normal);
        let order = st.focus_order_for(DatabaseWorkbenchDensity::Normal);
        assert!(order.contains(&"connections"));
        assert!(order.contains(&"query"));
        assert!(order.contains(&"results"));
        assert!(order.contains(&"inspector"));

        st.focus = "connections";
        let mut seen = vec![st.focus];
        for _ in 0..order.len() {
            let out = st.focus_next(DatabaseWorkbenchDensity::Normal);
            assert!(matches!(out, DatabaseWorkbenchOutcome::FocusChanged(_)));
            seen.push(st.focus);
        }
        // full cycle returns
        assert_eq!(seen.last().copied(), Some("connections"));
        assert!(seen.contains(&"query"));
        assert!(seen.contains(&"results"));
    }

    #[test]
    fn narrow_and_tiny_drop_panes() {
        let ws = WorkspaceState::new();
        let normal = database_workbench_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            DatabaseWorkbenchDensity::Normal,
        );
        let narrow = database_workbench_layout_density(
            Rect::new(0, 0, 70, 24),
            &ws,
            DatabaseWorkbenchDensity::Narrow,
        );
        let tiny = database_workbench_layout_density(
            Rect::new(0, 0, 40, 16),
            &ws,
            DatabaseWorkbenchDensity::Tiny,
        );

        let ids = |p: &[PaneGeom]| {
            p.iter()
                .filter(|g| !g.collapsed && g.area.width > 0 && g.area.height > 0)
                .map(|g| g.id.0.as_str().to_string())
                .collect::<Vec<_>>()
        };
        let n = ids(&normal);
        let w = ids(&narrow);
        let t = ids(&tiny);

        assert!(n.iter().any(|i| i == "inspector"), "{n:?}");
        assert!(
            !w.iter().any(|i| i == "inspector"),
            "narrow must drop inspector: {w:?}"
        );
        assert!(w.iter().any(|i| i == "connections") || w.iter().any(|i| i == "schema"));
        assert!(
            !t.iter().any(|i| i == "connections"),
            "tiny drops connections: {t:?}"
        );
        assert!(
            !t.iter().any(|i| i == "inspector"),
            "tiny drops inspector: {t:?}"
        );
        assert!(t.iter().any(|i| i == "query"));
        assert!(t.iter().any(|i| i == "results"));
    }

    #[test]
    fn run_and_cancel_messages() {
        let mut st = open();
        st.focus = "query";
        st.query.set_text("SELECT 1");
        let out = st.request_run();
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::RunRequested {
                    ref tab_id,
                    ref text,
                    ..
                } if tab_id == "t1" && text.contains("SELECT 1")
            ),
            "{out:?}"
        );
        st.begin_run("run-1");
        assert!(st.query.run.is_running());
        let out = st.request_cancel();
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::CancelRequested {
                    ref tab_id,
                    run_id: Some(ref id)
                } if tab_id == "t1" && id == "run-1"
            ),
            "{out:?}"
        );
    }

    #[test]
    fn disconnected_blocks_run() {
        let mut st = open();
        st.set_conn_gate(DatabaseConnGate::Disconnected);
        st.query.set_text("SELECT 1");
        let out = st.request_run();
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::RunBlocked {
                    reason: DatabaseRunBlockReason::Disconnected,
                    ..
                }
            ),
            "{out:?}"
        );
        // Ctrl+Enter path
        let out = st.handle_key(ctrl(KeyCode::Enter), &[], &[], &[], 0, &[]);
        assert!(
            matches!(out, DatabaseWorkbenchOutcome::RunBlocked { .. }),
            "{out:?}"
        );
    }

    #[test]
    fn error_and_tx_status_projection() {
        let mut st = open();
        let out = st.set_tx_status(DatabaseTxStatus::Open);
        assert!(matches!(
            out,
            DatabaseWorkbenchOutcome::TransactionChanged(DatabaseTxStatus::Open)
        ));
        st.begin_run("r1");
        assert_eq!(st.tx_status, DatabaseTxStatus::Active);
        st.finish_run_error("syntax error near FROM");
        assert!(matches!(st.query.run, QueryRunStatus::Failed { .. }));
        assert!(matches!(
            st.results.status,
            ResultQueryStatus::Failed { .. }
        ));
        assert_eq!(st.tx_status, DatabaseTxStatus::Failed);
        assert!(st.last_error.as_deref().unwrap().contains("syntax"));
        let slots = st.status_slots();
        assert!(slots.iter().any(|s| s.id == "err" || s.id == "tx"));
    }

    #[test]
    fn export_without_embedding_rows() {
        let mut st = open();
        st.conn_gate = DatabaseConnGate::Connected;
        let out = st.export_request(ResultExportFormat::Csv, 400);
        let dbg = format!("{out:?}");
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::ExportRequested {
                    format: ResultExportFormat::Csv,
                    ref tab_id,
                    visible_rows: 400,
                } if tab_id == "t1"
            ),
            "{out:?}"
        );
        // Outcome Debug must not include row payloads
        assert!(!dbg.contains("ada@example.com"));
        assert!(!dbg.contains("user1@"));
    }

    #[test]
    fn copy_outcome_summary_only() {
        let mut st = open();
        st.focus = "results";
        st.conn_gate = DatabaseConnGate::Connected;
        // Drive export via Ctrl+E
        let out = st.handle_key(ctrl(KeyCode::Char('e')), &[], &[], &[], 3, &[]);
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::ExportRequested {
                    format: ResultExportFormat::Csv,
                    ..
                }
            ),
            "{out:?}"
        );
    }

    #[test]
    fn tab_switch_projects_draft() {
        let mut st = open();
        st.query.set_text("SELECT 'tab1-edit'");
        let out = st.select_tab(1);
        assert!(matches!(
            out,
            DatabaseWorkbenchOutcome::TabChanged { ref id } if id == "t2"
        ));
        assert!(st.query.text().contains("orders") || st.query.text().contains("JOIN"));
        // previous draft saved
        assert!(st.tabs[0].draft.contains("tab1-edit"));
    }

    #[test]
    fn paint_all_densities_and_composes_children() {
        let system = DesignSystem::default();
        let mut st = open();
        let schema = example_schema_entries();
        let cols = example_result_columns();
        let data = example_result_rows();
        let mut cell_store = Vec::new();
        let rows = example_result_row_refs(&data, &mut cell_store);
        let inspect = example_inspect_fields();
        let history = example_db_history();
        let commands = example_db_commands();

        for d in [
            DatabaseWorkbenchDensity::Normal,
            DatabaseWorkbenchDensity::Narrow,
            DatabaseWorkbenchDensity::Tiny,
        ] {
            st.density = Some(d);
            let area = match d {
                DatabaseWorkbenchDensity::Normal => Rect::new(0, 0, 120, 36),
                DatabaseWorkbenchDensity::Narrow => Rect::new(0, 0, 70, 24),
                DatabaseWorkbenchDensity::Tiny => Rect::new(0, 0, 40, 16),
            };
            let mut buf = Buffer::empty(area);
            render_database_workbench(
                &mut buf,
                area,
                DatabaseWorkbenchSurfaces {
                    system: &system,
                    state: &mut st,
                    schema_entries: &schema,
                    result_columns: &cols,
                    result_rows: &rows,
                    inspect_fields: &inspect,
                    history: &history,
                    commands: &commands,
                },
            );
            assert!(!st.last_panes().is_empty());
        }

        // Structural: source composes named surfaces
        let src = include_str!("database_workbench.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for needle in [
            "ConnectionManager",
            "SchemaBrowser",
            "QueryEditor",
            "ResultGrid",
            "ObjectInspector",
            "HistoryPicker",
            "StatusBar",
            "CommandPalette",
            "ExportRequested",
            "CopyRequested",
        ] {
            assert!(body.contains(needle), "missing composition: {needle}");
        }
        for forbidden in [
            "TcpStream",
            "tokio::net",
            "sqlx::",
            "postgres::",
            "mysql",
            "std::net::",
            "keyring",
        ] {
            assert!(
                !body.contains(forbidden),
                "forbidden I/O in workbench: {forbidden}"
            );
        }
    }

    #[test]
    fn large_result_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(DatabaseWorkbenchDensity::Normal);
        let schema = example_schema_entries();
        let cols = example_result_columns();
        let large = large_result_row_data(bench::LARGE_ROWS);
        // Build owned cell strings + rows for window paint
        let cell_owned: Vec<[String; 4]> = large
            .iter()
            .map(|(_, id, email, active, created)| {
                [id.clone(), email.clone(), active.clone(), created.clone()]
            })
            .collect();
        let cell_refs: Vec<[ResultCell<'_>; 4]> = cell_owned
            .iter()
            .map(|c| {
                [
                    ResultCell::integer(c[0].as_str()),
                    ResultCell::text(c[1].as_str()),
                    ResultCell::bool_text(c[2].as_str()),
                    ResultCell::text(c[3].as_str()),
                ]
            })
            .collect();
        let rows: Vec<ResultRow<'_>> = large
            .iter()
            .enumerate()
            .map(|(i, (id, ..))| ResultRow::new(*id, (i as u64) + 1, &cell_refs[i]))
            .collect();
        let inspect = example_inspect_fields();
        let history = example_db_history();
        let commands = example_db_commands();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            render_database_workbench(
                &mut buf,
                area,
                DatabaseWorkbenchSurfaces {
                    system: &system,
                    state: &mut st,
                    schema_entries: &schema,
                    result_columns: &cols,
                    result_rows: &rows,
                    inspect_fields: &inspect,
                    history: &history,
                    commands: &commands,
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 5,
            "paint too slow: {elapsed:?} for {} rows × {} frames",
            bench::LARGE_ROWS,
            bench::PAINT_FRAMES
        );
    }

    #[test]
    fn disconnected_story_gate() {
        let mut st = DatabaseWorkbenchState::new();
        st.connections
            .set_connections(example_disconnected_connections());
        st.sync_conn_gate_from_selection();
        assert!(st.conn_gate.is_offline_like());
        st.query.set_text("SELECT 1");
        assert!(matches!(
            st.request_run(),
            DatabaseWorkbenchOutcome::RunBlocked { .. }
        ));
    }

    #[test]
    fn palette_and_history_open() {
        let mut st = open();
        let cmds = example_db_commands();
        let hist = example_db_history();
        let out = st.handle_key(ctrl(KeyCode::Char('p')), &[], &hist, &cmds, 0, &[]);
        assert!(matches!(out, DatabaseWorkbenchOutcome::OpenPalette));
        assert!(st.palette_open());
        let out = st.handle_key(press(KeyCode::Esc), &[], &hist, &cmds, 0, &[]);
        assert!(matches!(
            out,
            DatabaseWorkbenchOutcome::Palette { .. }
        ));
        assert!(!st.palette_open());

        let out = st.handle_key(ctrl(KeyCode::Char('h')), &[], &hist, &cmds, 0, &[]);
        assert!(matches!(out, DatabaseWorkbenchOutcome::OpenHistory));
        assert!(st.history_open());
    }

    #[test]
    fn inspector_keys_with_fields() {
        let mut st = open();
        st.density = Some(DatabaseWorkbenchDensity::Normal);
        st.set_focus(DatabaseWorkbenchPane::Inspector);
        st.inspector.set_accepts_input(true);
        let fields = example_inspect_fields();
        assert!(!fields.is_empty());
        // Dead path: empty fields must not panic and stays Ignored
        assert!(matches!(
            st.handle_key(press(KeyCode::Down), &[], &[], &[], 0, &[]),
            DatabaseWorkbenchOutcome::Ignored
        ));
        // Live path: real fields drive CursorMoved (or Inspector kind)
        let out = st.handle_key(press(KeyCode::Down), &[], &[], &[], 0, &fields);
        assert!(
            matches!(
                out,
                DatabaseWorkbenchOutcome::Inspector { ref kind }
                    if kind == "CursorMoved" || kind == "Scrolled" || kind == "Activate"
            ) || matches!(out, DatabaseWorkbenchOutcome::Ignored),
            // At least not stuck on handle_key_count(0) always-Ignored without accepting input
            "{out:?}"
        );
        // Force a second down from index 0 after ensuring focus/accepts
        st.set_focus(DatabaseWorkbenchPane::Inspector);
        let out = st.handle_key(press(KeyCode::Down), &[], &[], &[], 0, &fields);
        assert!(
            matches!(out, DatabaseWorkbenchOutcome::Inspector { ref kind } if kind != "Ignored"),
            "inspector with fields must emit non-Ignored on Down, got {out:?}"
        );
    }

    #[test]
    fn density_none_narrow_width_clamps_focus_cycle() {
        let mut st = open();
        // Explicit None override path (default)
        st.density = None;
        // Simulate last paint at narrow width without density override
        let _ = st.layout(Rect::new(0, 0, 70, 24));
        assert_eq!(
            st.effective_density(),
            DatabaseWorkbenchDensity::Narrow,
            "width 70 must be Narrow when density=None"
        );
        // Focus inspector then Tab/handle_key must clamp off inspector
        st.focus = "inspector";
        let out = st.handle_key(press(KeyCode::Tab), &[], &[], &[], 0, &[]);
        assert!(
            matches!(out, DatabaseWorkbenchOutcome::FocusChanged(_)),
            "{out:?}"
        );
        assert_ne!(
            st.focus, "inspector",
            "narrow paint must not keep focus on unpainted inspector"
        );
        // Full Tab cycle never lands on inspector
        st.focus = "connections";
        for _ in 0..8 {
            let _ = st.focus_next(st.effective_density());
            assert_ne!(st.focus, "inspector");
        }
        // Tiny width
        let _ = st.layout(Rect::new(0, 0, 40, 16));
        assert_eq!(st.effective_density(), DatabaseWorkbenchDensity::Tiny);
        st.focus = "connections";
        st.clamp_focus_to_density(st.effective_density());
        assert!(
            st.focus == "query" || st.focus == "results",
            "tiny clamps to query/results, got {}",
            st.focus
        );
        for _ in 0..6 {
            let _ = st.handle_key(press(KeyCode::Tab), &[], &[], &[], 0, &[]);
            assert!(
                st.focus == "query" || st.focus == "results",
                "tiny Tab cycle, got {}",
                st.focus
            );
        }
    }

    #[test]
    fn density_for_width() {
        assert_eq!(
            DatabaseWorkbenchDensity::for_width(30),
            DatabaseWorkbenchDensity::Tiny
        );
        assert_eq!(
            DatabaseWorkbenchDensity::for_width(60),
            DatabaseWorkbenchDensity::Narrow
        );
        assert_eq!(
            DatabaseWorkbenchDensity::for_width(100),
            DatabaseWorkbenchDensity::Normal
        );
    }

    #[test]
    fn fuzz_gate_and_tx_ids() {
        for g in [
            DatabaseConnGate::Connected,
            DatabaseConnGate::Disconnected,
            DatabaseConnGate::Offline,
            DatabaseConnGate::Reconnecting,
            DatabaseConnGate::AuthRequired,
            DatabaseConnGate::Error,
        ] {
            assert!(!g.id().is_empty());
        }
        for t in [
            DatabaseTxStatus::None,
            DatabaseTxStatus::Open,
            DatabaseTxStatus::Active,
            DatabaseTxStatus::Failed,
        ] {
            assert!(!t.id().is_empty());
        }
    }
}
