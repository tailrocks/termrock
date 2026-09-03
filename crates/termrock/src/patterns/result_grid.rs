// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ResultGrid** — database / query result component on [`super::DataTable`].
//!
//! **Mission.** Typed cells, nulls, binary, large text, row numbers, copy/export,
//! column statistics, editable cells, pagination/streaming, and query status.
//! Cell detail and object inspection for structured values. Very wide schemas
//! and unknown totals. Safe display/redaction for secrets and binary. Integrates
//! with [`super::QueryEditor`] (results slot) and schema context.
//!
//! **Ownership.** Host owns query execution, paging, typed decoding, and
//! persistence of edits. TermRock owns projection paint, selection/nav chrome,
//! status/stats chrome, and typed request outcomes.
//!
//! Research: database clients, VisiData, TablePlus, SQL terminal tools.
//!
//! Teaches: how to compose database / query result component on
//! [`super::DataTable`].
//!
//! Composes: [`crate::widgets::ColumnModel`], [`crate::widgets::ColumnPin`],
//! [`crate::widgets::CopyPayload`], [`crate::widgets::DataColumn`],
//! [`crate::widgets::DataColumnWidth`], [`crate::widgets::DataTable`],
//! [`crate::widgets::DataTableNavMode`],
//! [`crate::widgets::DataTableOutcome`], and 8 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseEvent},
    patterns::QueryResultSummary,
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{
        ColumnModel, ColumnPin, CopyPayload, DataColumn, DataColumnWidth, DataTable,
        DataTableNavMode, DataTableOutcome, DataTableState, DataTableToolbar, FilterSpec,
        InspectKind, InspectorField, LoadState, SemanticStatus, SortSpec, StatusIndicator,
    },
};

// ── Cell kinds & values ─────────────────────────────────────────────────────

/// Semantic cell kind (host classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ResultCellKind {
    /// SQL NULL / missing.
    Null,
    /// Boolean.
    Bool,
    /// Integer.
    Integer,
    /// Floating.
    Float,
    /// UTF-8 text.
    #[default]
    Text,
    /// Binary / blob (never dump raw by default).
    Binary,
    /// Structured JSON / object / array text.
    Json,
    /// Timestamp / date-time display.
    Timestamp,
    /// UUID / opaque id.
    Uuid,
    /// Secret / credential (redact by default).
    Secret,
    /// Domain-unknown.
    Other,
}

impl ResultCellKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Json => "json",
            Self::Timestamp => "timestamp",
            Self::Uuid => "uuid",
            Self::Secret => "secret",
            Self::Other => "other",
        }
    }

    /// Map to object-inspector kind.
    #[must_use]
    pub const fn to_inspect_kind(self) -> InspectKind {
        match self {
            Self::Null => InspectKind::Null,
            Self::Bool => InspectKind::Bool,
            Self::Integer | Self::Float => InspectKind::Number,
            Self::Text | Self::Timestamp | Self::Uuid | Self::Other => InspectKind::String,
            Self::Binary | Self::Secret => InspectKind::Binary,
            Self::Json => InspectKind::Object,
        }
    }

    /// Paint role for non-selected cells.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Null => Role::TextDisabled,
            Self::Bool => Role::TextMuted,
            Self::Integer | Self::Float => Role::Text,
            Self::Text | Self::Timestamp | Self::Uuid | Self::Other => Role::Text,
            Self::Binary => Role::TextMuted,
            Self::Json => Role::TextSecondary,
            // Redaction is a value kind, not a warning state. Its literal
            // label carries the distinction without spending warning color.
            Self::Secret => Role::TextMuted,
        }
    }
}

/// One projected cell (borrowed display payload from host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultCell<'a> {
    /// Kind.
    pub kind: ResultCellKind,
    /// Display text when non-null / non-binary (host may pre-format numbers).
    pub text: &'a str,
    /// Binary length when kind is Binary (display uses length, not bytes).
    pub binary_len: Option<u64>,
    /// Whether host marks this cell secret (overrides kind for redaction).
    pub secret: bool,
    /// Truncated large text (full value available via detail request).
    pub truncated: bool,
}

impl<'a> ResultCell<'a> {
    /// Text cell.
    #[must_use]
    pub const fn text(text: &'a str) -> Self {
        Self {
            kind: ResultCellKind::Text,
            text,
            binary_len: None,
            secret: false,
            truncated: false,
        }
    }

    /// NULL.
    #[must_use]
    pub const fn null() -> Self {
        Self {
            kind: ResultCellKind::Null,
            text: "",
            binary_len: None,
            secret: false,
            truncated: false,
        }
    }

    /// Integer display.
    #[must_use]
    pub const fn integer(text: &'a str) -> Self {
        Self {
            kind: ResultCellKind::Integer,
            text,
            binary_len: None,
            secret: false,
            truncated: false,
        }
    }

    /// Bool display (`true`/`false`).
    #[must_use]
    pub const fn bool_text(text: &'a str) -> Self {
        Self {
            kind: ResultCellKind::Bool,
            text,
            binary_len: None,
            secret: false,
            truncated: false,
        }
    }

    /// Binary blob of `len` bytes.
    #[must_use]
    pub const fn binary(len: u64) -> Self {
        Self {
            kind: ResultCellKind::Binary,
            text: "",
            binary_len: Some(len),
            secret: false,
            truncated: false,
        }
    }

    /// JSON / structured text.
    #[must_use]
    pub const fn json(text: &'a str) -> Self {
        Self {
            kind: ResultCellKind::Json,
            text,
            binary_len: None,
            secret: false,
            truncated: false,
        }
    }

    /// Secret value (redacted unless revealed).
    #[must_use]
    pub const fn secret_value(text: &'a str) -> Self {
        Self {
            kind: ResultCellKind::Secret,
            text,
            binary_len: None,
            secret: true,
            truncated: false,
        }
    }

    /// Kind override.
    #[must_use]
    pub const fn kind(mut self, kind: ResultCellKind) -> Self {
        self.kind = kind;
        self
    }

    /// Truncated large text mark.
    #[must_use]
    pub const fn truncated(mut self) -> Self {
        self.truncated = true;
        self
    }

    /// Secret flag.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }
}

// ── Schema / columns ────────────────────────────────────────────────────────

/// Schema column for result projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultColumn {
    /// Stable column id (name).
    pub id: String,
    /// Header title.
    pub title: String,
    /// SQL/type label (`varchar`, `int8`, `bytea`).
    pub type_name: Option<String>,
    /// Nullable.
    pub nullable: bool,
    /// Secret column (all cells redacted by default).
    pub secret: bool,
    /// Binary column.
    pub binary: bool,
    /// Host allows inline edit.
    pub editable: bool,
    /// Preferred width policy.
    pub width: DataColumnWidth,
    /// Priority under narrow width (higher keeps longer).
    pub priority: u8,
    /// Pin start (row number column typically).
    pub pin_start: bool,
}

impl ResultColumn {
    /// Named column.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            type_name: None,
            nullable: true,
            secret: false,
            binary: false,
            editable: false,
            width: DataColumnWidth::Min(10),
            priority: 50,
            pin_start: false,
        }
    }

    /// Type label.
    #[must_use]
    pub fn type_name(mut self, t: impl Into<String>) -> Self {
        self.type_name = Some(t.into());
        self
    }

    /// Not null.
    #[must_use]
    pub const fn not_null(mut self) -> Self {
        self.nullable = false;
        self
    }

    /// Secret.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Binary.
    #[must_use]
    pub const fn binary(mut self) -> Self {
        self.binary = true;
        self
    }

    /// Editable.
    #[must_use]
    pub const fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Width.
    #[must_use]
    pub const fn width(mut self, w: DataColumnWidth) -> Self {
        self.width = w;
        self
    }

    /// Priority.
    #[must_use]
    pub const fn priority(mut self, p: u8) -> Self {
        self.priority = p;
        self
    }

    /// Pin start.
    #[must_use]
    pub const fn pin_start(mut self) -> Self {
        self.pin_start = true;
        self
    }
}

/// One projected result row (visible window only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRow<'a> {
    /// Stable row key (logical index or host id).
    pub id: u64,
    /// 1-based display row number (pagination-aware).
    pub row_number: u64,
    /// Cells in **schema column order** (not including optional row# column).
    pub cells: &'a [ResultCell<'a>],
}

impl<'a> ResultRow<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: u64, row_number: u64, cells: &'a [ResultCell<'a>]) -> Self {
        Self {
            id,
            row_number,
            cells,
        }
    }
}

// ── Query status / stats ────────────────────────────────────────────────────

/// Query run status for results chrome.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ResultQueryStatus {
    /// No result yet.
    #[default]
    Idle,
    /// Streaming / partial rows.
    Streaming {
        /// Resident rows.
        resident: u64,
        /// Optional known total.
        total: Option<u64>,
    },
    /// Complete page/window.
    Ready {
        /// Rows in full result or known total.
        total: Option<u64>,
        /// Duration ms.
        duration_ms: Option<u64>,
    },
    /// Failed.
    Failed {
        /// Message.
        message: String,
    },
    /// Cancelled.
    Cancelled,
}

impl ResultQueryStatus {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Streaming { .. } => "streaming",
            Self::Ready { .. } => "ready",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Operator-facing lifecycle verb.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Streaming { .. } => "streaming",
            Self::Ready { .. } => "ready",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Shared lifecycle projection for recipe-owned status chrome.
    #[must_use]
    pub fn semantic(&self) -> SemanticStatus {
        match self {
            Self::Idle => SemanticStatus::Idle,
            Self::Streaming { .. } => SemanticStatus::Running,
            Self::Ready { .. } => SemanticStatus::Success,
            Self::Failed { .. } => SemanticStatus::Failed,
            Self::Cancelled => SemanticStatus::Paused,
        }
    }

    /// Footer / QueryEditor summary line.
    #[must_use]
    pub fn summary_line(&self, columns: usize, visible: usize) -> String {
        match self {
            Self::Idle => "no results".into(),
            Self::Streaming { resident, total } => match total {
                Some(t) => format!("streaming {resident}/{t} · {columns} cols · showing {visible}"),
                None => format!("streaming {resident}+ · {columns} cols · showing {visible}"),
            },
            Self::Ready { total, duration_ms } => {
                let rows = total
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| format!("{visible}+"));
                match duration_ms {
                    Some(ms) => format!("{rows} rows · {columns} cols · {ms}ms"),
                    None => format!("{rows} rows · {columns} cols"),
                }
            }
            // Severity rides the Danger role and the status glyph, not the word.
            Self::Failed { message } => message.clone(),
            Self::Cancelled => "cancelled".into(),
        }
    }

    /// Bridge to QueryEditor results chrome.
    #[must_use]
    pub fn to_query_summary(&self, columns: usize) -> QueryResultSummary {
        let status = self.summary_line(columns, 0);
        let mut s = QueryResultSummary::new(status).columns(columns as u32);
        match self {
            Self::Ready { total, .. } => {
                if let Some(t) = total {
                    s = s.rows(*t);
                }
            }
            Self::Streaming { resident, total } => {
                s = s.rows(*resident);
                s.has_more = total.map(|t| t > *resident).unwrap_or(true);
            }
            _ => {}
        }
        s
    }

    /// Map to DataTable load chrome.
    #[must_use]
    pub fn to_load_state(&self, projected: usize) -> LoadState {
        match self {
            Self::Idle => LoadState::Empty {
                message: Some("No results".into()),
            },
            Self::Streaming { resident, total } => LoadState::Partial {
                resident: *resident,
                total: *total,
            },
            Self::Ready { total, .. } => LoadState::Ready {
                count: total.unwrap_or(projected as u64),
            },
            Self::Failed { message } => LoadState::Error {
                message: message.clone(),
                retryable: true,
            },
            Self::Cancelled => LoadState::Empty {
                message: Some("Cancelled".into()),
            },
        }
    }
}

/// Per-column aggregate stats (host computes; chrome displays).
#[derive(Debug, Clone, PartialEq)]
pub struct ResultColumnStats {
    /// Column id.
    pub column: String,
    /// Non-null count.
    pub non_null: u64,
    /// Null count.
    pub nulls: u64,
    /// Distinct (optional).
    pub distinct: Option<u64>,
    /// Min display.
    pub min: Option<String>,
    /// Max display.
    pub max: Option<String>,
    /// Mean / average display.
    pub mean: Option<String>,
    /// Sum display.
    pub sum: Option<String>,
}

impl ResultColumnStats {
    /// Construct.
    #[must_use]
    pub fn new(column: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            non_null: 0,
            nulls: 0,
            distinct: None,
            min: None,
            max: None,
            mean: None,
            sum: None,
        }
    }

    /// Compact one-line chrome.
    #[must_use]
    pub fn summary_line(&self) -> String {
        let mut parts = vec![self.column.clone()];
        parts.push(format!("n={}", self.non_null));
        if self.nulls > 0 {
            parts.push(format!("null={}", self.nulls));
        }
        if let Some(d) = self.distinct {
            parts.push(format!("uniq={d}"));
        }
        if let Some(m) = &self.min {
            parts.push(format!("min={m}"));
        }
        if let Some(m) = &self.max {
            parts.push(format!("max={m}"));
        }
        parts.join(" · ")
    }
}

/// Export format request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResultExportFormat {
    /// CSV.
    Csv,
    /// TSV.
    Tsv,
    /// JSON lines / array (host decides).
    Json,
    /// Markdown table.
    Markdown,
}

impl ResultExportFormat {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Json => "json",
            Self::Markdown => "markdown",
        }
    }
}

/// Redaction policy for secrets/binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ResultRedaction {
    /// Redact secrets and summarize binary (default).
    #[default]
    Safe,
    /// Reveal secrets (host must gate).
    RevealSecrets,
    /// Show short hex prefix for binary.
    BinaryHexPreview,
}

// ── Display helpers ─────────────────────────────────────────────────────────

/// NULL glyph.
pub const RESULT_NULL_GLYPH: &str = "∅";
/// ASCII null.
pub const RESULT_NULL_ASCII: &str = "NULL";
/// Secret mask.
pub const RESULT_SECRET_MASK: &str = "••••";
/// Large text ellipsis mark.
pub const RESULT_TRUNC_MARK: &str = "…";

/// Format a cell for grid display under redaction policy.
#[must_use]
pub fn format_result_cell(cell: &ResultCell<'_>, redaction: ResultRedaction) -> String {
    if cell.secret || matches!(cell.kind, ResultCellKind::Secret) {
        if matches!(redaction, ResultRedaction::RevealSecrets) {
            return with_trunc(cell.text, cell.truncated);
        }
        return RESULT_SECRET_MASK.into();
    }
    match cell.kind {
        ResultCellKind::Null => RESULT_NULL_GLYPH.into(),
        ResultCellKind::Binary => {
            let len = cell.binary_len.unwrap_or(0);
            match redaction {
                ResultRedaction::BinaryHexPreview if !cell.text.is_empty() => {
                    format!("blob({len}) {}", take_display_cols(cell.text, 16))
                }
                _ => format!("blob({len}B)"),
            }
        }
        ResultCellKind::Json
        | ResultCellKind::Text
        | ResultCellKind::Integer
        | ResultCellKind::Float
        | ResultCellKind::Bool
        | ResultCellKind::Timestamp
        | ResultCellKind::Uuid
        | ResultCellKind::Other
        | ResultCellKind::Secret => with_trunc(cell.text, cell.truncated),
    }
}

fn with_trunc(text: &str, truncated: bool) -> String {
    if truncated {
        format!("{text}{RESULT_TRUNC_MARK}")
    } else {
        text.to_string()
    }
}

/// Max cell paint width for very long text (display clamp; full via detail).
pub const RESULT_CELL_MAX_DISPLAY: usize = 120;

/// Clamp display string for grid paint.
#[must_use]
pub fn clamp_cell_display(s: &str, max_cols: usize) -> String {
    let max = max_cols.max(4);
    let t = take_display_cols(s, max);
    if t.len() < s.len() || display_wider(s, max) {
        format!("{t}{RESULT_TRUNC_MARK}")
    } else {
        t.to_string()
    }
}

fn display_wider(s: &str, max: usize) -> bool {
    crate::text::display_cols(s) > max
}

/// Build DataTable column model from schema (+ optional row number).
#[must_use]
pub fn result_column_model(columns: &[ResultColumn], row_numbers: bool) -> ColumnModel<String> {
    let mut cols = Vec::new();
    if row_numbers {
        cols.push(
            DataColumn::new("#".to_string(), "#", DataColumnWidth::Fixed(5))
                .priority(255)
                .pin(ColumnPin::Start),
        );
    }
    for c in columns {
        let mut dc = DataColumn::new(c.id.clone(), c.title.clone(), c.width).priority(c.priority);
        if c.pin_start {
            dc = dc.pin(ColumnPin::Start);
        }
        if c.editable {
            dc = dc.editable();
        }
        // Always sortable at chrome level (host re-sorts projection).
        dc = dc.sortable();
        cols.push(dc);
    }
    ColumnModel::new(cols)
}

/// Project typed rows into DataTable string cells (owned buffers).
///
/// Returns `(row_ids_with_cell_vecs, flat storage is inside each vec)`.
#[must_use]
pub fn project_result_rows(
    rows: &[ResultRow<'_>],
    columns: &[ResultColumn],
    redaction: ResultRedaction,
    row_numbers: bool,
) -> Vec<(u64, Vec<String>)> {
    rows.iter()
        .map(|r| {
            let mut cells = Vec::with_capacity(columns.len() + usize::from(row_numbers));
            if row_numbers {
                cells.push(r.row_number.to_string());
            }
            for (i, col) in columns.iter().enumerate() {
                let cell = r.cells.get(i).copied().unwrap_or(ResultCell::null());
                let mut c = cell;
                if col.secret {
                    c.secret = true;
                }
                if col.binary && matches!(c.kind, ResultCellKind::Text | ResultCellKind::Other) {
                    c.kind = ResultCellKind::Binary;
                }
                let formatted = format_result_cell(&c, redaction);
                cells.push(clamp_cell_display(&formatted, RESULT_CELL_MAX_DISPLAY));
            }
            (r.id, cells)
        })
        .collect()
}

/// Build ObjectInspector fields for a single row (cell detail).
#[must_use]
pub fn result_row_to_inspector_fields<'a>(
    columns: &'a [ResultColumn],
    row: &ResultRow<'a>,
    redaction: ResultRedaction,
) -> Vec<InspectorField<'a>> {
    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let cell = row.cells.get(i).copied().unwrap_or(ResultCell::null());
            let value = if cell.secret && !matches!(redaction, ResultRedaction::RevealSecrets) {
                RESULT_SECRET_MASK
            } else if matches!(cell.kind, ResultCellKind::Null) {
                RESULT_NULL_GLYPH
            } else if matches!(cell.kind, ResultCellKind::Binary) {
                // static-ish — use empty and type_label
                ""
            } else {
                cell.text
            };
            let mut f =
                InspectorField::new(col.id.as_str(), value).kind(cell.kind.to_inspect_kind());
            if let Some(t) = col.type_name.as_deref() {
                f = f.type_label(t);
            }
            if col.secret || cell.secret {
                f = f.secret();
            }
            if matches!(cell.kind, ResultCellKind::Json) {
                f = f.kind(InspectKind::Object);
            }
            f
        })
        .collect()
}

/// TSV export of projected window (safe display).
#[must_use]
pub fn export_result_window_tsv(
    columns: &[ResultColumn],
    rows: &[ResultRow<'_>],
    redaction: ResultRedaction,
    include_header: bool,
) -> String {
    let mut out = String::new();
    if include_header {
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            out.push_str(&c.title);
        }
        out.push('\n');
    }
    for r in rows {
        for (i, col) in columns.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            let mut cell = r.cells.get(i).copied().unwrap_or(ResultCell::null());
            if col.secret {
                cell.secret = true;
            }
            let s = format_result_cell(&cell, redaction);
            // Escape tabs/newlines lightly
            out.push_str(&s.replace(['\t', '\n', '\r'], " "));
        }
        out.push('\n');
    }
    out
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// ResultGrid outcomes — host owns query IO and clipboard policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultGridOutcome {
    /// No change.
    Ignored,
    /// Cursor / selection moved.
    CursorMoved,
    /// Scrolled.
    Scrolled,
    /// Sort requested.
    SortRequested {
        /// Column id.
        column: String,
        /// Ascending.
        ascending: bool,
    },
    /// Filter changed.
    FilterChanged(FilterSpec),
    /// Row activated.
    Activate {
        /// Row id.
        row: u64,
    },
    /// Selection changed.
    SelectionChanged,
    /// Select-all visible requested.
    SelectAllRequested,
    /// Copy request with payload.
    Copy(CopyPayload),
    /// Export request (host serializes full result or page).
    ExportRequested {
        /// Format.
        format: ResultExportFormat,
        /// When true, only projected window.
        selection_or_window: bool,
    },
    /// Open cell detail / large text.
    CellDetailRequested {
        /// Row.
        row: u64,
        /// Column id.
        column: String,
    },
    /// Open object inspector for structured cell/row.
    InspectRequested {
        /// Row.
        row: u64,
        /// Column when cell-scoped.
        column: Option<String>,
    },
    /// Inline edit started.
    EditStarted {
        /// Row.
        row: u64,
        /// Column.
        column: String,
    },
    /// Edit committed.
    EditCommitted {
        /// Row.
        row: u64,
        /// Column.
        column: String,
        /// Text.
        text: String,
    },
    /// Edit cancelled.
    EditCancelled,
    /// Next page / more rows (streaming).
    PageNext,
    /// Previous page.
    PagePrev,
    /// Refresh / re-run projection.
    RefreshRequested,
    /// Retry after error.
    RetryLoad,
    /// Column stats focus.
    StatsFocus {
        /// Column id.
        column: String,
    },
    /// Toggle secret reveal (host must authorize).
    RevealSecretsToggled {
        /// New policy is reveal.
        reveal: bool,
    },
    /// Context menu.
    ContextMenu {
        /// Row.
        row: u64,
        /// Column.
        column: Option<String>,
    },
    /// Fullscreen.
    FullscreenRequested,
    /// Nav mode changed.
    NavModeChanged(DataTableNavMode),
    /// Column resized.
    ColumnResized {
        /// Column.
        column: String,
        /// Width.
        width: u16,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Result grid state.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultGridState {
    /// Underlying interactive table (row id = u64, col id = String).
    pub table: DataTableState<u64, String>,
    /// Query status chrome.
    pub status: ResultQueryStatus,
    /// Redaction policy.
    pub redaction: ResultRedaction,
    /// Show row numbers column.
    pub row_numbers: bool,
    /// Show stats strip when host supplies stats.
    pub show_stats: bool,
    /// Column stats (host-filled).
    pub stats: Vec<ResultColumnStats>,
    /// Focused stats column index.
    pub stats_cursor: usize,
    /// Colorless.
    pub colorless: bool,
    /// Title.
    pub title: Option<String>,
    /// Schema columns (for chrome / model rebuild).
    pub schema: Vec<ResultColumn>,
    /// Last status line painted.
    pub last_status_line: String,
    accepts_input: bool,
}

impl Default for ResultGridState {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultGridState {
    /// Fresh grid.
    #[must_use]
    pub fn new() -> Self {
        let mut table = DataTableState::new();
        table.nav_mode = DataTableNavMode::Cell;
        table.striped = true;
        Self {
            table,
            status: ResultQueryStatus::Idle,
            redaction: ResultRedaction::Safe,
            row_numbers: true,
            show_stats: false,
            stats: Vec::new(),
            stats_cursor: 0,
            colorless: false,
            title: None,
            schema: Vec::new(),
            last_status_line: String::new(),
            accepts_input: true,
        }
    }

    /// With schema.
    #[must_use]
    pub fn with_schema(schema: Vec<ResultColumn>) -> Self {
        let mut s = Self::new();
        s.schema = schema;
        s
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.table.set_accepts_input(on);
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Set schema.
    pub fn set_schema(&mut self, schema: Vec<ResultColumn>) {
        self.schema = schema;
    }

    /// Set status + sync load chrome.
    pub fn set_status(&mut self, status: ResultQueryStatus, projected_len: usize) {
        self.table.load = status.to_load_state(projected_len);
        self.status = status;
    }

    /// Logical universe for virtual window (unknown → 0 or resident).
    pub fn set_logical_rows(&mut self, n: u64) {
        self.table.set_logical_rows(n);
    }

    /// QueryEditor summary bridge.
    #[must_use]
    pub fn query_summary(&self) -> QueryResultSummary {
        self.status.to_query_summary(self.schema.len())
    }

    /// Toggle stats strip.
    pub fn toggle_stats(&mut self) {
        self.show_stats = !self.show_stats;
    }

    /// Build column model from schema.
    #[must_use]
    pub fn column_model(&self) -> ColumnModel<String> {
        result_column_model(&self.schema, self.row_numbers)
    }

    /// Keys. `row_ids` is the projected window row id list (same order as paint).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        columns: &ColumnModel<String>,
        row_ids: &[u64],
    ) -> ResultGridOutcome {
        if !self.accepts_input || key.is_release() {
            return ResultGridOutcome::Ignored;
        }
        let is_press = key.is_press();

        if is_press {
            // Result-specific chords before DataTable
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
            {
                match key.code {
                    KeyCode::Char('e' | 'E') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        return ResultGridOutcome::ExportRequested {
                            format: ResultExportFormat::Csv,
                            selection_or_window: true,
                        };
                    }
                    KeyCode::Char('e' | 'E') => {
                        return ResultGridOutcome::ExportRequested {
                            format: ResultExportFormat::Tsv,
                            selection_or_window: true,
                        };
                    }
                    KeyCode::Char('j' | 'J') => {
                        return ResultGridOutcome::ExportRequested {
                            format: ResultExportFormat::Json,
                            selection_or_window: true,
                        };
                    }
                    KeyCode::Char('i' | 'I') => {
                        return self.inspect_cursor(row_ids, columns);
                    }
                    KeyCode::Char('d' | 'D') => {
                        return self.cell_detail_cursor(row_ids, columns);
                    }
                    KeyCode::Char('u' | 'U') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        let reveal = !matches!(self.redaction, ResultRedaction::RevealSecrets);
                        self.redaction = if reveal {
                            ResultRedaction::RevealSecrets
                        } else {
                            ResultRedaction::Safe
                        };
                        return ResultGridOutcome::RevealSecretsToggled { reveal };
                    }
                    KeyCode::Char('g' | 'G') => {
                        self.toggle_stats();
                        if self.show_stats && !self.stats.is_empty() {
                            let col = self.stats[self.stats_cursor.min(self.stats.len() - 1)]
                                .column
                                .clone();
                            return ResultGridOutcome::StatsFocus { column: col };
                        }
                        return ResultGridOutcome::CursorMoved;
                    }
                    KeyCode::Char('r' | 'R') => {
                        return ResultGridOutcome::RefreshRequested;
                    }
                    KeyCode::Char('n' | 'N') => {
                        return ResultGridOutcome::PageNext;
                    }
                    KeyCode::Char('p' | 'P') => {
                        return ResultGridOutcome::PagePrev;
                    }
                    _ => {}
                }
            }
            // ] [ page when not editing
            if key.modifiers.is_empty() && !self.table.editing {
                match key.code {
                    KeyCode::Char(']') => return ResultGridOutcome::PageNext,
                    KeyCode::Char('[') => return ResultGridOutcome::PagePrev,
                    _ => {}
                }
            }
        }

        let out = self.table.handle_key(key, row_ids, columns);
        map_table_outcome(out)
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        columns: &mut ColumnModel<String>,
        row_ids: &[u64],
    ) -> ResultGridOutcome {
        if !self.accepts_input {
            return ResultGridOutcome::Ignored;
        }
        let out = self.table.handle_mouse(event, row_ids, columns);
        map_table_outcome(out)
    }

    fn inspect_cursor(&self, row_ids: &[u64], columns: &ColumnModel<String>) -> ResultGridOutcome {
        let Some(row) = row_ids.get(self.table.cursor_row).copied() else {
            return ResultGridOutcome::Ignored;
        };
        let column = self.table.cursor_column_id(columns);
        ResultGridOutcome::InspectRequested { row, column }
    }

    fn cell_detail_cursor(
        &self,
        row_ids: &[u64],
        columns: &ColumnModel<String>,
    ) -> ResultGridOutcome {
        let Some(row) = row_ids.get(self.table.cursor_row).copied() else {
            return ResultGridOutcome::Ignored;
        };
        let Some(column) = self.table.cursor_column_id(columns) else {
            return ResultGridOutcome::Ignored;
        };
        // Skip row# column
        if column == "#" {
            return ResultGridOutcome::Ignored;
        }
        ResultGridOutcome::CellDetailRequested { row, column }
    }
}

fn map_table_outcome(out: DataTableOutcome<u64, String>) -> ResultGridOutcome {
    match out {
        DataTableOutcome::Ignored => ResultGridOutcome::Ignored,
        DataTableOutcome::Scrolled => ResultGridOutcome::Scrolled,
        DataTableOutcome::CursorMoved | DataTableOutcome::ToggleRow(_) => {
            ResultGridOutcome::CursorMoved
        }
        // A hover wash is chrome; the grid's host has nothing to do about it.
        DataTableOutcome::HoverChanged => ResultGridOutcome::Ignored,
        DataTableOutcome::SortRequested(col) => ResultGridOutcome::SortRequested {
            column: col,
            ascending: true,
        },
        DataTableOutcome::SortSpec(SortSpec { column, ascending }) => {
            ResultGridOutcome::SortRequested { column, ascending }
        }
        DataTableOutcome::FilterChanged(f) => ResultGridOutcome::FilterChanged(f),
        DataTableOutcome::Activate(row) => ResultGridOutcome::Activate { row },
        DataTableOutcome::SelectionChanged => ResultGridOutcome::SelectionChanged,
        DataTableOutcome::SelectAllRequested => ResultGridOutcome::SelectAllRequested,
        DataTableOutcome::Copy(p) => ResultGridOutcome::Copy(p),
        DataTableOutcome::ExpandToggled(_) | DataTableOutcome::GroupToggled(_) => {
            ResultGridOutcome::CursorMoved
        }
        DataTableOutcome::ContextMenu { row, column } => {
            ResultGridOutcome::ContextMenu { row, column }
        }
        DataTableOutcome::EditStarted { row, column } => {
            let Some(column) = column else {
                return ResultGridOutcome::Ignored;
            };
            ResultGridOutcome::EditStarted { row, column }
        }
        DataTableOutcome::EditCommitted { row, column, text } => {
            ResultGridOutcome::EditCommitted { row, column, text }
        }
        DataTableOutcome::EditCancelled => ResultGridOutcome::EditCancelled,
        DataTableOutcome::RetryLoad => ResultGridOutcome::RetryLoad,
        DataTableOutcome::ToolbarAction(i) => match i {
            0 => ResultGridOutcome::RefreshRequested,
            1 => ResultGridOutcome::ExportRequested {
                format: ResultExportFormat::Csv,
                selection_or_window: false,
            },
            _ => ResultGridOutcome::Ignored,
        },
        DataTableOutcome::ColumnResized { column, width } => {
            ResultGridOutcome::ColumnResized { column, width }
        }
        DataTableOutcome::ColumnVisibility { .. }
        | DataTableOutcome::ColumnReorderRequested { .. } => ResultGridOutcome::CursorMoved,
        DataTableOutcome::FullscreenRequested => ResultGridOutcome::FullscreenRequested,
        DataTableOutcome::NavModeChanged(m) => ResultGridOutcome::NavModeChanged(m),
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Result grid workbench chrome around DataTable.
#[derive(Debug, Clone)]
pub struct ResultGrid<'a> {
    system: &'a DesignSystem,
    columns: &'a [ResultColumn],
    rows: &'a [ResultRow<'a>],
    focused: bool,
    title: Option<&'a str>,
}

impl<'a> ResultGrid<'a> {
    /// Schema columns + projected window rows.
    #[must_use]
    pub const fn new(
        system: &'a DesignSystem,
        columns: &'a [ResultColumn],
        rows: &'a [ResultRow<'a>],
    ) -> Self {
        Self {
            system,
            columns,
            rows,
            focused: true,
            title: None,
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
    /// Paint status + optional stats + DataTable body.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ResultGridState) {
        if area.is_empty() {
            return;
        }
        // Keep schema on state for keys when host uses widget columns
        if state.schema.is_empty() && !self.columns.is_empty() {
            state.schema = self.columns.to_vec();
        }

        state.table.colorless = state.colorless;
        state
            .table
            .set_accepts_input(self.focused && state.accepts_input);

        let mut y = area.y;
        let mut h = area.height;

        // Status line
        if h > 0 {
            let title = self.title.or(state.title.as_deref()).unwrap_or("results");
            let line = format!(
                "· {title} · {}",
                state
                    .status
                    .summary_line(self.columns.len(), self.rows.len())
            );
            state.last_status_line = line.clone();
            let status = StatusIndicator::new(state.status.semantic(), self.system)
                .label(state.status.verb());
            let status_width = status.measure_width(None).min(area.width);
            status.paint(Rect::new(area.x, y, status_width, 1), buffer, None);
            let metadata_x = area.x.saturating_add(status_width.saturating_add(1));
            let metadata_width = area.right().saturating_sub(metadata_x);
            if metadata_width > 0 {
                self.system.paint_row(
                    buffer,
                    Rect::new(metadata_x, y, metadata_width, 1),
                    &line,
                    self.system.style(if self.focused {
                        Role::TextStrong
                    } else {
                        Role::TextMuted
                    }),
                );
            }
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        // Stats strip
        let stats_h = u16::from(state.show_stats && !state.stats.is_empty() && h >= 3);
        if stats_h > 0 {
            let st = &state.stats[state.stats_cursor.min(state.stats.len() - 1)];
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &format!("stats {}", st.summary_line()),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if h == 0 {
            return;
        }

        let body = Rect {
            x: area.x,
            y,
            width: area.width,
            height: h,
        };

        // Project strings for DataTable
        let projected_owned =
            project_result_rows(self.rows, self.columns, state.redaction, state.row_numbers);
        // Build col model
        let col_model = result_column_model(self.columns, state.row_numbers);

        // Sync load / logical
        state.table.load = state.status.to_load_state(self.rows.len());
        match &state.status {
            ResultQueryStatus::Ready { total: Some(t), .. } => {
                state.table.set_logical_rows(*t);
            }
            ResultQueryStatus::Streaming { resident, total } => {
                state.table.set_logical_rows(total.unwrap_or(*resident));
            }
            _ => {
                state.table.set_logical_rows(self.rows.len() as u64);
            }
        }

        // Lifetime bridge: hold Vec<Vec<String>> and build refs
        let cell_refs: Vec<(u64, Vec<&str>)> = projected_owned
            .iter()
            .map(|(id, cells)| (*id, cells.iter().map(String::as_str).collect()))
            .collect();
        let rows: Vec<(u64, &[&str])> = cell_refs
            .iter()
            .map(|(id, cells)| (*id, cells.as_slice()))
            .collect();

        let toolbar = DataTableToolbar {
            actions: &["Refresh", "Export"],
        };

        DataTable::new(self.system, &col_model, &rows)
            .toolbar(&toolbar)
            .focused(self.focused && state.accepts_input)
            .render(body, buffer, &mut state.table);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Wide schema / large page targets.
pub mod bench {
    /// Columns in a wide result.
    pub const WIDE_COLUMNS: usize = 64;
    /// Rows in a projected page.
    pub const PAGE_ROWS: usize = 500;
    /// Logical unknown-total stream size (host).
    pub const STREAM_RESIDENT: u64 = 50_000;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn schema() -> Vec<ResultColumn> {
        vec![
            ResultColumn::new("id", "ID")
                .type_name("int8")
                .not_null()
                .width(DataColumnWidth::Fixed(6))
                .priority(100)
                .pin_start(),
            ResultColumn::new("name", "Name")
                .type_name("text")
                .width(DataColumnWidth::Min(12))
                .priority(90)
                .editable(),
            ResultColumn::new("blob", "Blob")
                .type_name("bytea")
                .binary()
                .priority(40),
            ResultColumn::new("token", "Token")
                .type_name("text")
                .secret()
                .priority(30),
            ResultColumn::new("meta", "Meta")
                .type_name("jsonb")
                .priority(50),
        ]
    }

    fn sample_rows() -> (Vec<ResultColumn>, Vec<ResultRow<'static>>) {
        static R0: [ResultCell<'static>; 5] = [
            ResultCell::integer("1"),
            ResultCell::text("alpha"),
            ResultCell::binary(128),
            ResultCell::secret_value("s3cr3t"),
            ResultCell::json(r#"{"a":1}"#),
        ];
        static R1: [ResultCell<'static>; 5] = [
            ResultCell::integer("2"),
            ResultCell::text("beta"),
            ResultCell::null(),
            ResultCell::secret_value("other"),
            ResultCell::json("[]"),
        ];
        static R2: [ResultCell<'static>; 5] = [
            ResultCell::integer("3"),
            ResultCell {
                kind: ResultCellKind::Text,
                text: "very long name that should clamp in the grid display area",
                binary_len: None,
                secret: false,
                truncated: true,
            },
            ResultCell::binary(1_048_576),
            ResultCell::null(),
            ResultCell::json(r#"{"nested":true}"#),
        ];
        let cols = schema();
        let rows = vec![
            ResultRow::new(1, 1, &R0),
            ResultRow::new(2, 2, &R1),
            ResultRow::new(3, 3, &R2),
        ];
        (cols, rows)
    }

    #[test]
    fn format_null_secret_binary() {
        assert_eq!(
            format_result_cell(&ResultCell::null(), ResultRedaction::Safe),
            RESULT_NULL_GLYPH
        );
        assert_eq!(
            format_result_cell(&ResultCell::secret_value("x"), ResultRedaction::Safe),
            RESULT_SECRET_MASK
        );
        assert!(format_result_cell(&ResultCell::binary(99), ResultRedaction::Safe).contains("99"));
        assert_eq!(
            format_result_cell(
                &ResultCell::secret_value("open"),
                ResultRedaction::RevealSecrets
            ),
            "open"
        );
    }

    #[test]
    fn project_includes_row_numbers_and_redaction() {
        let (cols, rows) = sample_rows();
        let proj = project_result_rows(&rows, &cols, ResultRedaction::Safe, true);
        assert_eq!(proj[0].1[0], "1"); // row#
        assert!(proj[0].1.iter().any(|c| c == RESULT_SECRET_MASK));
        assert!(proj[0].1.iter().any(|c| c.contains("blob")));
    }

    #[test]
    fn export_tsv_header() {
        let (cols, rows) = sample_rows();
        let tsv = export_result_window_tsv(&cols, &rows, ResultRedaction::Safe, true);
        assert!(tsv.starts_with("ID\tName"));
        assert!(tsv.contains("alpha"));
        assert!(tsv.contains(RESULT_SECRET_MASK));
    }

    #[test]
    fn status_to_query_summary() {
        let st = ResultQueryStatus::Ready {
            total: Some(100),
            duration_ms: Some(12),
        };
        let sum = st.to_query_summary(5);
        assert_eq!(sum.columns, Some(5));
        assert_eq!(sum.rows, Some(100));
        assert!(sum.status.contains("12ms"));
    }

    #[test]
    fn column_model_wide_priority() {
        let mut cols = schema();
        for i in 0..bench::WIDE_COLUMNS {
            cols.push(
                ResultColumn::new(format!("c{i}"), format!("C{i}"))
                    .priority((50u8).saturating_sub((i % 40) as u8)),
            );
        }
        let model = result_column_model(&cols, true);
        assert!(model.index_of(&"#".to_string()).is_some());
    }

    #[test]
    fn paint_basic() {
        let system = DesignSystem::default();
        let (cols, rows) = sample_rows();
        let mut state = ResultGridState::with_schema(cols.clone());
        state.set_status(
            ResultQueryStatus::Ready {
                total: Some(3),
                duration_ms: Some(5),
            },
            rows.len(),
        );
        let area = Rect::new(0, 0, 80, 14);
        let mut buf = Buffer::empty(area);
        let _ = ResultGrid::new(&system, &cols, &rows)
            .title("q1")
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("alpha") || text.contains("results") || text.contains("q1"),
            "{text}"
        );
    }

    #[test]
    fn keys_export_inspect_page() {
        let (cols, rows) = sample_rows();
        let mut state = ResultGridState::with_schema(cols.clone());
        state.set_status(
            ResultQueryStatus::Ready {
                total: Some(3),
                duration_ms: None,
            },
            rows.len(),
        );
        let model = state.column_model();
        let row_ids: Vec<u64> = rows.iter().map(|r| r.id).collect();

        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
                &model,
                &row_ids
            ),
            ResultGridOutcome::ExportRequested {
                format: ResultExportFormat::Tsv,
                ..
            }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
                &model,
                &row_ids
            ),
            ResultGridOutcome::PageNext
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
                &model,
                &row_ids
            ),
            ResultGridOutcome::InspectRequested { .. }
        ));
    }

    #[test]
    fn reveal_secrets_toggle() {
        let mut state = ResultGridState::new();
        let model = ColumnModel::new(vec![DataColumn::new(
            "a".into(),
            "A",
            DataColumnWidth::Min(4),
        )]);
        let row_ids: [u64; 0] = [];
        let out = state.handle_key(
            KeyEvent::new(
                KeyCode::Char('u'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            &model,
            &row_ids,
        );
        assert!(matches!(
            out,
            ResultGridOutcome::RevealSecretsToggled { reveal: true }
        ));
        assert!(matches!(state.redaction, ResultRedaction::RevealSecrets));
    }

    #[test]
    fn accepts_input_gate() {
        let mut state = ResultGridState::new();
        state.set_accepts_input(false);
        let model = state.column_model();
        let row_ids: [u64; 0] = [];
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &model,
                &row_ids
            ),
            ResultGridOutcome::Ignored
        ));
    }

    #[test]
    fn inspector_bridge() {
        let (cols, rows) = sample_rows();
        let fields = result_row_to_inspector_fields(&cols, &rows[0], ResultRedaction::Safe);
        assert_eq!(fields.len(), cols.len());
        assert!(fields.iter().any(|f| f.secret));
    }

    #[test]
    fn large_page_paint() {
        let system = DesignSystem::default();
        let cols = vec![
            ResultColumn::new("id", "ID").width(DataColumnWidth::Fixed(8)),
            ResultColumn::new("v", "V").width(DataColumnWidth::Min(12)),
        ];
        // Build owned strings for cells
        let id_strs: Vec<String> = (0..bench::PAGE_ROWS).map(|i| i.to_string()).collect();
        let val_strs: Vec<String> = (0..bench::PAGE_ROWS).map(|i| format!("v{i}")).collect();
        let cell_pairs: Vec<[ResultCell<'_>; 2]> = (0..bench::PAGE_ROWS)
            .map(|i| {
                [
                    ResultCell::integer(&id_strs[i]),
                    ResultCell::text(&val_strs[i]),
                ]
            })
            .collect();
        let rows: Vec<ResultRow<'_>> = cell_pairs
            .iter()
            .enumerate()
            .map(|(i, cells)| ResultRow::new(i as u64, i as u64 + 1, cells))
            .collect();
        let mut state = ResultGridState::with_schema(cols.clone());
        state.set_status(
            ResultQueryStatus::Streaming {
                resident: bench::STREAM_RESIDENT,
                total: None,
            },
            rows.len(),
        );
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        for _ in 0..6 {
            let _ = ResultGrid::new(&system, &cols, &rows).paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn never_runs_queries() {
        let src = include_str!("result_grid.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "sqlx::",
            "tokio_postgres",
            "rusqlite",
            "std::process::Command",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn stats_summary() {
        let mut s = ResultColumnStats::new("age");
        s.non_null = 10;
        s.nulls = 2;
        s.min = Some("1".into());
        s.max = Some("99".into());
        assert!(s.summary_line().contains("age"));
        assert!(s.summary_line().contains("null=2"));
    }
}
