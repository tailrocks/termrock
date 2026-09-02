// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/grid.rs (MIT).

//! Typed cells, a pending-change queue, paging and local sort.

use std::collections::{BTreeSet, HashMap};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::widgets::{
    Action, ActionVariant, ActivationOutcome, ButtonState, ButtonVariant, ColumnKind, ColumnModel,
    DataColumn, DataColumnWidth, DataTable, DataTableNavMode, DataTableOutcome, DataTableState,
    Dialog, DialogOutcome, DialogSize, DialogState, Hint as KeyHint, LoadState, PanelChrome, Prop,
    SortSpec,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::tablepro::paint;
use crate::text;

const ID: WidgetId = WidgetId::of("grid");
const GRID: WidgetId = ID.sub("grid");
const PREVIEW: WidgetId = ID.sub("preview");
const DISCARD: WidgetId = ID.sub("discard");
const SAVE: WidgetId = ID.sub("save");
const PREVIEW_HINTS: &[KeyHint<'static>] = &[
    KeyHint {
        chord: "← →",
        label: "Choose",
        priority: 10,
        visible: true,
    },
    KeyHint {
        chord: "Enter",
        label: "Confirm",
        priority: 20,
        visible: true,
    },
    KeyHint {
        chord: "Esc",
        label: "Cancel",
        priority: 30,
        visible: true,
    },
];
const PAGE: usize = 40;
const ALL: usize = 96;

const NAMES: &[&str] = &[
    "Northwind Traders",
    "Blue Yonder Airlines",
    "Contoso Pharmaceuticals",
    "Fabrikam Robotics",
    "Litware Analytics",
    "Tailspin Toys",
    "Wide World Importers",
    "Adventure Works",
    "Proseware Studio",
    "Woodgrove Bank",
    "Alpine Ski House",
    "Coho Winery",
    "Lucerne Publishing",
    "Margie's Travel",
    "Trey Research",
    "Humongous Insurance",
];
const PLANS: &[&str] = &["free", "pro", "team", "enterprise"];
const OWNERS: &[&str] = &["mira", "jonas", "ana", "kai"];
const COLS: [&str; 8] = [
    "id",
    "customer",
    "plan",
    "seats",
    "mrr",
    "active",
    "renewed_at",
    "notes",
];

#[derive(Debug, Clone, PartialEq)]
enum CellVal {
    Null,
    Default,
    Text(String),
    Int(i64),
    Num(f64),
    Bool(bool),
    Json(String),
}

impl CellVal {
    fn text(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Default => "DEFAULT".into(),
            Self::Text(s) | Self::Json(s) => s.clone(),
            Self::Int(i) => i.to_string(),
            Self::Num(n) => format!("{n:.2}"),
            Self::Bool(b) => b.to_string(),
        }
    }
    fn edit_text(&self) -> String {
        match self {
            Self::Null | Self::Default => String::new(),
            v => v.text(),
        }
    }
}

fn row(i: usize) -> Vec<CellVal> {
    let plan = PLANS[(i * 7 + 3) % PLANS.len()];
    let seats = [1, 3, 5, 12, 25, 40, 80, 150][(i * 5 + 1) % 8];
    let mrr = match plan {
        "free" => 0.0,
        "pro" => 29.0 * seats as f64,
        "team" => 24.0 * seats as f64,
        _ => 19.0 * seats as f64,
    };
    let suffix = if i >= NAMES.len() {
        format!(" {}", i / NAMES.len() + 1)
    } else {
        String::new()
    };
    let renewed = if plan == "free" {
        CellVal::Null
    } else {
        CellVal::Text(format!("2026-{:02}-{:02}", 1 + i % 12, 1 + (i * 3) % 28))
    };
    let notes = if i.is_multiple_of(4) {
        CellVal::Json(format!(
            "{{\"owner\":\"{}\",\"seats\":{seats}}}",
            OWNERS[i % OWNERS.len()]
        ))
    } else {
        CellVal::Null
    };
    vec![
        CellVal::Int(1001 + i as i64),
        CellVal::Text(format!("{}{suffix}", NAMES[i % NAMES.len()])),
        CellVal::Text(plan.to_owned()),
        CellVal::Int(seats),
        CellVal::Num(mrr),
        CellVal::Bool(i % 5 != 3),
        renewed,
        notes,
    ]
}

fn literal(v: &CellVal) -> String {
    match v {
        CellVal::Null => "NULL".into(),
        CellVal::Default => "DEFAULT".into(),
        CellVal::Text(s) | CellVal::Json(s) => format!("'{}'", s.replace('\'', "''")),
        other => other.text(),
    }
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn editable(col: usize) -> bool {
    !matches!(col, 0 | 4)
}

fn parse_edit(col: usize, s: &str) -> Result<CellVal, String> {
    let t = s.trim();
    match col {
        1 => {
            if t.is_empty() {
                Err("customer is NOT NULL".into())
            } else {
                Ok(CellVal::Text(s.to_owned()))
            }
        }
        2 => {
            if PLANS.contains(&t) {
                Ok(CellVal::Text(t.to_owned()))
            } else {
                Err(format!("Must be one of: {}", PLANS.join(", ")))
            }
        }
        3 => t
            .parse::<i64>()
            .map(CellVal::Int)
            .map_err(|_| "seats must be an integer".into()),
        5 => match t {
            "true" | "1" => Ok(CellVal::Bool(true)),
            "false" | "0" => Ok(CellVal::Bool(false)),
            _ => Err("active must be true or false".into()),
        },
        6 => {
            if t.is_empty() {
                Ok(CellVal::Null)
            } else {
                Ok(CellVal::Text(t.to_owned()))
            }
        }
        7 => {
            if t.is_empty() {
                Ok(CellVal::Null)
            } else {
                Ok(CellVal::Json(t.to_owned()))
            }
        }
        _ => Err("Column is read-only".into()),
    }
}

fn mouse_down(pos: ratatui::layout::Position) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        position: pos,
        modifiers: KeyModifiers::NONE,
    }
}

#[derive(Clone)]
enum UndoAction {
    Cell {
        row: usize,
        col: usize,
        before: Option<CellVal>,
    },
    Delete {
        row: usize,
        was_deleted: bool,
    },
    Insert {
        row: usize,
    },
}

struct Pending {
    cells: HashMap<(usize, usize), CellVal>,
    inserted: BTreeSet<usize>,
    deleted: BTreeSet<usize>,
}

impl Pending {
    fn new() -> Self {
        Self {
            cells: HashMap::new(),
            inserted: BTreeSet::new(),
            deleted: BTreeSet::new(),
        }
    }
    fn dirty_rows(&self) -> BTreeSet<usize> {
        self.cells
            .keys()
            .map(|(r, _)| *r)
            .filter(|r| !self.inserted.contains(r))
            .collect()
    }
    fn counts(&self) -> (usize, usize, usize) {
        (
            self.dirty_rows().len(),
            self.inserted.len(),
            self.deleted.len(),
        )
    }
    fn total(&self) -> usize {
        let (u, i, d) = self.counts();
        u + i + d
    }
    fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.inserted.is_empty() && self.deleted.is_empty()
    }
    fn label(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let (u, i, d) = self.counts();
        let mut parts = Vec::new();
        if u > 0 {
            parts.push(format!("{u} update{}", if u == 1 { "" } else { "s" }));
        }
        if i > 0 {
            parts.push(format!("{i} insert{}", if i == 1 { "" } else { "s" }));
        }
        if d > 0 {
            parts.push(format!("{d} delete{}", if d == 1 { "" } else { "s" }));
        }
        Some(parts.join(" · "))
    }
}

pub struct GridPage {
    table: DataTableState<usize, &'static str>,
    columns: ColumnModel<&'static str>,
    rows: Vec<Vec<CellVal>>,
    order: Vec<usize>,
    loaded: usize,
    pending: Pending,
    undo: Vec<UndoAction>,
    commit_ticks: u8,
    saved: u32,
    preview: bool,
    preview_facts: Vec<Prop>,
    preview_code: Vec<String>,
    dialog: DialogState<&'static str>,
    save: ButtonState,
    label_view: u16,
    discard: ButtonState,
    preview_btn: ButtonState,
    row_error: Option<(usize, String)>,
}

impl GridPage {
    #[must_use]
    pub fn new() -> Self {
        // Source DataGrid `sample_widths` on the 40-row page:
        // p95.max(header).clamp(kind.min, kind.max), header = name + primary 2 + 2.
        let columns = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(9))
                .kind(ColumnKind::Id)
                .primary()
                .sortable(),
            DataColumn::new("customer", "customer", DataColumnWidth::Fixed(25))
                .editable()
                .sortable(),
            DataColumn::new("plan", "plan", DataColumnWidth::Fixed(10))
                .editable()
                .sortable(),
            DataColumn::new("seats", "seats", DataColumnWidth::Fixed(7))
                .kind(ColumnKind::Numeric)
                .editable()
                .sortable(),
            DataColumn::new("mrr", "mrr", DataColumnWidth::Fixed(7))
                .kind(ColumnKind::Numeric)
                .sortable(),
            DataColumn::new("active", "active", DataColumnWidth::Fixed(5)).editable(),
            DataColumn::new("renewed_at", "renewed_at", DataColumnWidth::Fixed(12)).editable(),
            DataColumn::new("notes", "notes", DataColumnWidth::Fixed(27)).editable(),
        ]);
        let loaded = PAGE.min(ALL);
        let rows: Vec<Vec<CellVal>> = (0..loaded).map(row).collect();
        let order: Vec<usize> = (0..loaded).collect();
        let mut table = DataTableState::new();
        table.nav_mode = DataTableNavMode::Cell;
        table.striped = false;
        table.set_logical_rows(loaded as u64);
        table.load = LoadState::Partial {
            resident: loaded as u64,
            total: Some(4_812),
        };
        Self {
            table,
            columns,
            rows,
            order,
            loaded,
            pending: Pending::new(),
            undo: Vec::new(),
            commit_ticks: 0,
            saved: 0,
            preview: false,
            preview_facts: Vec::new(),
            preview_code: Vec::new(),
            dialog: DialogState::confirm("copy", "cancel"),
            save: ButtonState::new(),
            label_view: 0,
            discard: ButtonState::new(),
            preview_btn: ButtonState::new(),
            row_error: None,
        }
    }

    fn stored(&self, r: usize, c: usize) -> CellVal {
        self.rows
            .get(r)
            .and_then(|row| row.get(c))
            .cloned()
            .unwrap_or(CellVal::Null)
    }

    fn value(&self, r: usize, c: usize) -> CellVal {
        self.pending
            .cells
            .get(&(r, c))
            .cloned()
            .unwrap_or_else(|| self.stored(r, c))
    }

    fn display_cell(&self, r: usize, c: usize) -> String {
        let v = self.value(r, c);
        if matches!(v, CellVal::Json(_)) {
            v.text().split_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            v.text()
        }
    }

    fn cursor_src(&self) -> Option<usize> {
        self.order.get(self.table.cursor_row).copied()
    }

    fn cursor_col(&self) -> usize {
        self.table.cursor_col.min(COLS.len().saturating_sub(1))
    }

    fn sync_load(&mut self) {
        self.table.set_logical_rows(self.order.len() as u64);
        if self.commit_ticks > 0 {
            self.table.load = LoadState::Loading {
                message: Some("Saving…".into()),
            };
        } else if self.loaded < ALL {
            self.table.load = LoadState::Partial {
                resident: self.loaded as u64,
                total: Some(4_812),
            };
        } else {
            self.table.load = LoadState::Ready {
                count: self.order.len() as u64,
            };
        }
    }

    fn ids(&self) -> Vec<usize> {
        self.order.clone()
    }

    fn statements(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut cells: Vec<_> = self.pending.cells.iter().collect();
        cells.sort_by_key(|((r, c), _)| (*r, *c));
        for ((r, c), v) in cells {
            if self.pending.inserted.contains(r) {
                continue;
            }
            out.push(format!(
                "UPDATE customers SET {} = {} WHERE id = {};",
                COLS[*c],
                literal(v),
                self.value(*r, 0).text()
            ));
        }
        for r in &self.pending.inserted {
            let cols: Vec<&str> = COLS
                .iter()
                .enumerate()
                .filter(|(c, _)| self.pending.cells.contains_key(&(*r, *c)))
                .map(|(_, n)| *n)
                .collect();
            if cols.is_empty() {
                out.push("INSERT INTO customers DEFAULT VALUES;".into());
            } else {
                let vals: Vec<String> = COLS
                    .iter()
                    .enumerate()
                    .filter_map(|(c, _)| self.pending.cells.get(&(*r, c)).map(literal))
                    .collect();
                out.push(format!(
                    "INSERT INTO customers ({}) VALUES ({});",
                    cols.join(", "),
                    vals.join(", ")
                ));
            }
        }
        for r in &self.pending.deleted {
            out.push(format!(
                "DELETE FROM customers WHERE id = {};",
                self.value(*r, 0).text()
            ));
        }
        out
    }

    fn record_cell(&mut self, r: usize, c: usize, value: CellVal) {
        let before = self.pending.cells.get(&(r, c)).cloned();
        let stored = self.stored(r, c);
        if value == stored && !self.pending.inserted.contains(&r) {
            self.pending.cells.remove(&(r, c));
        } else {
            self.pending.cells.insert((r, c), value);
        }
        self.undo.push(UndoAction::Cell {
            row: r,
            col: c,
            before,
        });
    }

    fn begin_edit(&mut self) {
        let Some(src) = self.cursor_src() else {
            return;
        };
        let c = self.cursor_col();
        if !editable(c) || self.pending.deleted.contains(&src) {
            return;
        }
        self.table.editing = true;
        self.table.edit_draft = self.value(src, c).edit_text();
    }

    fn fetch_more(&mut self, cx: &mut PageCtx<'_>) {
        let from = self.loaded;
        let to = (from + PAGE).min(ALL);
        for i in from..to {
            self.rows.push(row(i));
            self.order.push(i);
        }
        self.loaded = to;
        self.sync_load();
        cx.status(format!("Fetched rows {}–{}", from + 1, self.loaded));
    }

    fn refresh(&mut self, cx: &mut PageCtx<'_>) {
        let loaded = PAGE.min(ALL);
        self.rows = (0..loaded).map(row).collect();
        self.order = (0..loaded).collect();
        self.loaded = loaded;
        self.pending = Pending::new();
        self.undo.clear();
        self.row_error = None;
        self.table = DataTableState::new();
        self.table.nav_mode = DataTableNavMode::Cell;
        self.table.striped = false;
        self.sync_load();
        cx.status("Reloaded from the source");
    }

    fn insert_row(&mut self, cx: &mut PageCtx<'_>) {
        let mut rec = vec![CellVal::Null; COLS.len()];
        rec[0] = CellVal::Default;
        rec[4] = CellVal::Default;
        self.rows.push(rec);
        let src = self.rows.len() - 1;
        self.order.push(src);
        self.pending.inserted.insert(src);
        self.undo.push(UndoAction::Insert { row: src });
        self.loaded += 1;
        self.sync_load();
        if let Some(disp) = self.order.iter().position(|&r| r == src) {
            self.table.cursor_row = disp;
            self.table.cursor_col = 1;
        }
        cx.status("Row inserted · fill it in, then Save");
    }

    fn toggle_delete(&mut self, cx: &mut PageCtx<'_>) {
        let Some(src) = self.cursor_src() else {
            return;
        };
        if self.pending.inserted.contains(&src) {
            self.remove_inserted(src);
            cx.status("Row queued for deletion · u undoes");
            return;
        }
        let was = self.pending.deleted.contains(&src);
        if was {
            self.pending.deleted.remove(&src);
        } else {
            self.pending.deleted.insert(src);
            self.pending.cells.retain(|(r, _), _| *r != src);
        }
        self.undo.push(UndoAction::Delete {
            row: src,
            was_deleted: was,
        });
        cx.status("Row queued for deletion · u undoes");
    }

    fn remove_inserted(&mut self, src: usize) {
        if src >= self.rows.len() {
            return;
        }
        self.rows.remove(src);
        self.order.retain(|&r| r != src);
        for r in &mut self.order {
            if *r > src {
                *r -= 1;
            }
        }
        self.pending.inserted.remove(&src);
        self.pending.cells.retain(|(r, _), _| *r != src);
        let shift = |s: &mut BTreeSet<usize>| {
            *s = s.iter().map(|&r| if r > src { r - 1 } else { r }).collect();
        };
        shift(&mut self.pending.inserted);
        shift(&mut self.pending.deleted);
        self.pending.cells = self
            .pending
            .cells
            .drain()
            .map(|((r, c), v)| ((if r > src { r - 1 } else { r }, c), v))
            .collect();
        if self.loaded > 0 {
            self.loaded -= 1;
        }
        self.table.cursor_row = self
            .table
            .cursor_row
            .min(self.order.len().saturating_sub(1));
        self.sync_load();
    }

    fn undo_last(&mut self) {
        let Some(a) = self.undo.pop() else {
            return;
        };
        match a {
            UndoAction::Cell { row, col, before } => match before {
                Some(v) => {
                    self.pending.cells.insert((row, col), v);
                }
                None => {
                    self.pending.cells.remove(&(row, col));
                }
            },
            UndoAction::Delete { row, was_deleted } => {
                if was_deleted {
                    self.pending.deleted.insert(row);
                } else {
                    self.pending.deleted.remove(&row);
                }
            }
            UndoAction::Insert { row } => {
                if self.pending.inserted.contains(&row) {
                    self.remove_inserted(row);
                }
            }
        }
    }

    fn discard(&mut self, cx: &mut PageCtx<'_>) {
        let inserted: Vec<usize> = self.pending.inserted.iter().copied().collect();
        for src in inserted.into_iter().rev() {
            self.remove_inserted(src);
        }
        self.pending = Pending::new();
        self.undo.clear();
        self.row_error = None;
        cx.status("Changes discarded");
    }

    fn request_commit(&mut self, cx: &mut PageCtx<'_>) {
        if self.pending.is_empty() {
            cx.status("Nothing to save");
            return;
        }
        self.commit_ticks = 4;
        self.sync_load();
        cx.status("Saving…");
    }

    fn open_preview(&mut self) {
        let code = self.statements();
        let (changed, inserted, deleted) = self.pending.counts();
        self.preview_facts = vec![
            Prop::new("Statements", code.len().to_string()),
            Prop::new(
                "Rows",
                format!("{changed} changed · {inserted} inserted · {deleted} deleted"),
            ),
            Prop::new("Target", "customers"),
        ];
        self.preview_code = code;
        self.dialog = DialogState::destructive("close", "cancel");
        self.preview = true;
    }

    fn sort_by(&mut self, spec: SortSpec<&'static str>) {
        let Some(c) = COLS.iter().position(|n| *n == spec.column) else {
            return;
        };
        let values: Vec<(usize, String)> = self
            .order
            .iter()
            .map(|&r| (r, self.display_cell(r, c)))
            .collect();
        let mut idxs: Vec<usize> = (0..self.order.len()).collect();
        idxs.sort_by(|&ia, &ib| {
            let (ra, va) = &values[ia];
            let (rb, vb) = &values[ib];
            let ord = match (self.value(*ra, c), self.value(*rb, c)) {
                (CellVal::Int(a), CellVal::Int(b)) => a.cmp(&b),
                (CellVal::Num(a), CellVal::Num(b)) => {
                    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
                }
                _ => va.cmp(vb),
            };
            if spec.ascending { ord } else { ord.reverse() }
        });
        self.order = idxs.into_iter().map(|i| values[i].0).collect();
        self.table.sort = Some(spec);
    }

    fn finish_commit(&mut self, cx: &mut PageCtx<'_>) {
        let bad = self
            .pending
            .cells
            .iter()
            .find(|((_, c), v)| *c == 3 && matches!(v, CellVal::Int(n) if *n > 500));
        match bad.map(|((r, _), _)| *r) {
            Some(r) => {
                self.row_error = Some((r, "seats above the plan limit (500)".into()));
                self.commit_ticks = 0;
                self.sync_load();
                cx.status("Save failed · the row is marked");
            }
            None => {
                let n = self.pending.total();
                for ((r, c), v) in self.pending.cells.drain() {
                    if let Some(row) = self.rows.get_mut(r)
                        && let Some(cell) = row.get_mut(c)
                    {
                        *cell = v;
                    }
                }
                let deleted = std::mem::take(&mut self.pending.deleted);
                let mut keep = Vec::new();
                for (i, rec) in self.rows.drain(..).enumerate() {
                    if !deleted.contains(&i) {
                        keep.push(rec);
                    }
                }
                self.rows = keep;
                self.order = (0..self.rows.len()).collect();
                self.loaded = self.rows.len();
                self.pending = Pending::new();
                self.undo.clear();
                self.saved += n as u32;
                self.row_error = None;
                self.commit_ticks = 0;
                self.sync_load();
                cx.status(format!("Saved {n} changes"));
            }
        }
    }

    fn rows_label(&self) -> String {
        if self.order.is_empty() {
            return "0 rows".into();
        }
        let start = self.table.window.offset as usize;
        let vp = usize::from(self.label_view);
        let a = start.saturating_add(1);
        let b = (start + vp).min(self.order.len());
        let total = if self.loaded < ALL {
            format!(
                "{} loaded · ~{} total",
                thousands(self.loaded),
                thousands(4_812)
            )
        } else {
            thousands(self.order.len())
        };
        format!("rows {}–{} of {total}", thousands(a), thousands(b))
    }

    fn cols_label(&self) -> Option<String> {
        let n = self.columns.columns.len();
        let vis = self.table.header_regions.len();
        if vis == 0 || vis >= n {
            return None;
        }
        Some(format!("cols 1–{vis} of {n}"))
    }

    fn position_label(&self) -> String {
        match self.cols_label() {
            Some(c) => format!("{} · {c}", self.rows_label()),
            None => self.rows_label(),
        }
    }

    fn on_table(
        &mut self,
        ev: DataTableOutcome<usize, &'static str>,
        cx: &mut PageCtx<'_>,
    ) -> Route {
        match ev {
            DataTableOutcome::Ignored => Route::Ignored,
            DataTableOutcome::EditStarted { .. } => {
                // `e` sets `editing` before this outcome; Activate already
                // called `begin_edit`. Do not refill after Backspace-to-empty.
                if !self.table.editing {
                    self.begin_edit();
                }
                Route::Changed
            }
            DataTableOutcome::EditCommitted { row, column, text } => {
                if let Some(c) = COLS.iter().position(|n| *n == column) {
                    match parse_edit(c, &text) {
                        Ok(v) => {
                            self.record_cell(row, c, v);
                            cx.status(format!("{} pending", self.pending.total()));
                        }
                        Err(e) => cx.status(e),
                    }
                }
                Route::Changed
            }
            DataTableOutcome::EditCancelled => Route::Changed,
            DataTableOutcome::SortSpec(spec) => {
                self.sort_by(spec);
                Route::Changed
            }
            DataTableOutcome::SortRequested(col) => {
                let ascending = match &self.table.sort {
                    Some(s) if s.column == col => !s.ascending,
                    _ => true,
                };
                self.sort_by(SortSpec {
                    column: col,
                    ascending,
                });
                Route::Changed
            }
            DataTableOutcome::Activate(src) => {
                if self.pending.deleted.contains(&src) {
                    cx.status(format!(
                        "Would follow the reference on row {}",
                        self.table.cursor_row + 1
                    ));
                    return Route::Changed;
                }
                if editable(self.cursor_col()) {
                    self.begin_edit();
                } else {
                    cx.status(format!("Row {} activated", self.table.cursor_row + 1));
                }
                Route::Changed
            }
            DataTableOutcome::Copy(p) => {
                let n = match p {
                    termrock::widgets::CopyPayload::Cell { text } => text.len(),
                    _ => 0,
                };
                cx.status(format!("Copied {n} chars"));
                Route::Changed
            }
            DataTableOutcome::FilterChanged(_) => {
                cx.status("The filter editor belongs to the app");
                Route::Changed
            }
            DataTableOutcome::RetryLoad => {
                self.refresh(cx);
                Route::Changed
            }
            DataTableOutcome::CursorMoved
            | DataTableOutcome::Scrolled
            | DataTableOutcome::HoverChanged
            | DataTableOutcome::SelectionChanged
            | DataTableOutcome::ToggleRow(_) => Route::Changed,
            _ => Route::Changed,
        }
    }

    fn preview_actions() -> [Action<'static, &'static str>; 2] {
        [
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "close",
                label: "Close",
                enabled: true,
                variant: ActionVariant::Primary,
            },
        ]
    }

    fn apply_preview(&mut self, out: DialogOutcome<&'static str>, _cx: &mut PageCtx<'_>) -> Route {
        match out {
            DialogOutcome::Ignored | DialogOutcome::LoadingBlocked => Route::Consumed,
            DialogOutcome::Activated("close") | DialogOutcome::DefaultActivated("close") => {
                self.preview = false;
                Route::Changed
            }
            DialogOutcome::Activated(_) | DialogOutcome::Cancelled => {
                self.preview = false;
                Route::Changed
            }
            _ => Route::Changed,
        }
    }

    fn paint_change_marks(
        &self,
        body: Rect,
        buf: &mut Buffer,
        t: &termrock::style::JunieTheme,
        bg: ratatui::style::Color,
    ) {
        let off = usize::try_from(self.table.window.offset).unwrap_or(0);
        let vp = usize::from(self.table.window.viewport.max(1));
        for vis in 0..vp {
            let idx = off.saturating_add(vis);
            let Some(&src) = self.order.get(idx) else {
                break;
            };
            let y = body
                .y
                .saturating_add(1)
                .saturating_add(u16::try_from(vis).unwrap_or(u16::MAX));
            if y >= body.bottom() {
                break;
            }
            let (glyph, style) = if self.row_error.as_ref().is_some_and(|(r, _)| *r == src) {
                (
                    "!",
                    Style::new().fg(t.error).add_modifier(Modifier::BOLD).bg(bg),
                )
            } else if self.pending.deleted.contains(&src) {
                ("−", t.muted().bg(bg))
            } else if self.pending.inserted.contains(&src) {
                ("+", t.secondary().bg(bg))
            } else if self.pending.cells.keys().any(|(r, _)| *r == src) {
                ("•", t.primary().fg(t.warning).bg(bg))
            } else {
                continue;
            };
            buf.set_string(body.x.saturating_add(2), y, glyph, style);
        }
    }

    fn paint_dirty_underlines(&self, buf: &mut Buffer) {
        for region in &self.table.cell_regions {
            let Some(c) = COLS.iter().position(|n| *n == region.column) else {
                continue;
            };
            if !self.pending.cells.contains_key(&(region.row, c)) {
                continue;
            }
            if self.pending.inserted.contains(&region.row) {
                continue;
            }
            for x in region.area.x..region.area.right() {
                if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                    cell.set_style(cell.style().add_modifier(Modifier::UNDERLINED));
                }
            }
        }
    }
}

impl Page for GridPage {
    fn title(&self) -> &'static str {
        "Data grid"
    }
    fn blurb(&self) -> &'static str {
        "Typed cells, a pending-change queue, paging and local sort"
    }
    fn editing(&self) -> bool {
        self.table.editing
    }
    fn animating(&self) -> bool {
        self.commit_ticks > 0
    }
    fn overlaying(&self) -> bool {
        self.preview
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let overlay = self.preview;
        let saved_inert = ctx.inert;
        ctx.inert = saved_inert || overlay;

        let bar_focus = ctx.interaction.focused(PREVIEW)
            || ctx.interaction.focused(DISCARD)
            || ctx.interaction.focused(SAVE);
        let focused = ctx.interaction.focused(GRID) || bar_focus;
        let h = area.height.min(30);
        let pending = !self.pending.is_empty();
        // Card pad 2 + title 1; table header 1. Source DataGrid `bar_h` is 2
        // when pending (one blank + the action row).
        let body_h = h
            .saturating_sub(3)
            .saturating_sub(if pending { 2 } else { 0 });
        let seeded = body_h.saturating_sub(1);
        if seeded > 0 {
            self.label_view = seeded;
        }
        let meta = self.position_label();
        let (inner, bg) = layout::card(
            Rect::new(area.x, area.y, area.width, h),
            buf,
            t,
            Some("customers"),
            Some(&meta),
            focused && !overlay,
        );
        let body = if pending {
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(2),
            )
        } else {
            inner
        };
        let owned: Vec<(usize, Vec<String>)> = self
            .order
            .iter()
            .map(|&r| {
                let cells: Vec<String> = (0..COLS.len()).map(|c| self.display_cell(r, c)).collect();
                (r, cells)
            })
            .collect();
        let refs: Vec<(usize, Vec<&str>)> = owned
            .iter()
            .map(|(r, cells)| (*r, cells.iter().map(String::as_str).collect()))
            .collect();
        let projected: Vec<(usize, &[&str])> =
            refs.iter().map(|(r, c)| (*r, c.as_slice())).collect();
        self.table
            .set_accepts_input(ctx.interaction.focused(GRID) && !overlay);
        StatefulWidget::render(
            &DataTable::new(ctx.system, &self.columns, &projected)
                .focused(ctx.interaction.focused(GRID) && !overlay)
                .row_numbers(true)
                .datagrid(true),
            body,
            buf,
            &mut self.table,
        );
        let hidden = self
            .columns
            .columns
            .len()
            .saturating_sub(self.table.header_regions.len());
        if hidden > 0 {
            let lbl = format!("{hidden}›");
            let w = u16::try_from(lbl.chars().count()).unwrap_or(2);
            // junie DataGrid: `{n}›` sits at cols_area.right()+1.
            let x = body.right().saturating_sub(w.saturating_add(2));
            if let Some(cell) = buf.cell_mut((body.right().saturating_sub(1), body.y)) {
                if cell.symbol() == "…" {
                    cell.set_symbol(" ");
                }
            }
            buf.set_string(x, body.y, &lbl, t.faint().bg(bg));
        }
        ctx.control(GRID, body, overlay);
        ctx.scrollable(GRID, body);
        self.label_view = self.table.window.viewport;
        self.paint_change_marks(body, buf, t, bg);
        self.paint_dirty_underlines(buf);

        if pending {
            let by = inner.bottom().saturating_sub(1);
            let count = self.pending.total();
            let text_s = format!("• {count} pending");
            buf.set_string(inner.x + 1, by, &text_s, t.primary().fg(t.warning).bg(bg));
            let detail = match &self.row_error {
                Some((r, msg)) if Some(*r) == self.cursor_src() => format!("· {msg}"),
                _ => self.pending.label().unwrap_or_default(),
            };
            let ds = if self
                .row_error
                .as_ref()
                .is_some_and(|(r, _)| Some(*r) == self.cursor_src())
            {
                t.error_fg().bg(bg)
            } else {
                t.muted().bg(bg)
            };
            buf.set_string(
                inner.x + 2 + text::width(&text_s) as u16,
                by,
                text::truncate(&detail, 24),
                ds,
            );
            let labels = ["Preview SQL", "Discard", "Save"];
            let vars = [
                ButtonVariant::Quiet,
                ButtonVariant::Quiet,
                ButtonVariant::Primary,
            ];
            let ids = [PREVIEW, DISCARD, SAVE];
            let widths = [
                paint::button_width(labels[0]),
                paint::button_width(labels[1]),
                paint::button_width(labels[2]),
            ];
            let rects = layout::row_layout_right(
                Rect::new(inner.x, by, inner.width.saturating_sub(1), 1),
                &widths,
                1,
            );
            let states = [&mut self.preview_btn, &mut self.discard, &mut self.save];
            for i in 0..3 {
                if rects.get(i).is_none() {
                    break;
                }
                paint::button(
                    labels[i], vars[i], ids[i], rects[i], buf, ctx, states[i], false, bg,
                );
            }
        }

        let hy = area.y.saturating_add(h).saturating_add(1);
        if hy < area.bottom() {
            let help = if pending {
                format!(
                    "Enter edits · Space selects · s sorts · + inserts · - deletes · p previews SQL · Ctrl+S saves · seats over 500 are rejected on save · saved so far: {}",
                    self.saved
                )
            } else {
                format!(
                    "p previews SQL · Ctrl+S saves · seats over 500 are rejected on save · saved so far: {}",
                    self.saved
                )
            };
            buf.set_string(
                area.x.saturating_add(2),
                hy,
                &text::truncate(&help, usize::from(area.width.saturating_sub(2))),
                t.muted(),
            );
        }
        ctx.inert = saved_inert;
        if overlay {
            self.dialog.set_open(true);
            self.dialog.set_accepts_input(true);
            let actions = Self::preview_actions();
            Dialog::new("Pending changes", Text::default(), ctx.system)
                .emphasis(PanelChrome::Focused)
                .preferred_size(DialogSize {
                    width: 66,
                    height: 14,
                })
                .facts(&self.preview_facts, &self.preview_code)
                .hints(PREVIEW_HINTS)
                .paint_modal(*buf.area(), buf, &mut self.dialog, &actions);
            ctx.control(ID.sub("modal"), *buf.area(), false);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if self.preview {
            let actions = Self::preview_actions();
            return match ev {
                PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    let out = self.dialog.handle_key(*key, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        self.apply_preview(out, cx)
                    }
                }
                PageEvent::Click { pos, .. } => {
                    let out = self.dialog.handle_click(*pos, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        self.apply_preview(out, cx)
                    }
                }
                _ => Route::Consumed,
            };
        }
        match ev {
            PageEvent::Tick => {
                if self.commit_ticks == 0 {
                    return Route::Ignored;
                }
                self.commit_ticks -= 1;
                if self.commit_ticks == 0 {
                    self.finish_commit(cx);
                }
                Route::Changed
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == PREVIEW {
                    return match self.preview_btn.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.open_preview();
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == DISCARD {
                    return match self.discard.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.discard(cx);
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == SAVE {
                    return match self.save.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.request_commit(cx);
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f != GRID {
                    return Route::Ignored;
                }
                if self.table.editing && matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    let Some(src) = self.cursor_src() else {
                        return Route::Consumed;
                    };
                    let c = self.cursor_col();
                    let text = self.table.edit_draft.clone();
                    match parse_edit(c, &text) {
                        Ok(v) => {
                            self.record_cell(src, c, v);
                            self.table.editing = false;
                            self.table.edit_draft.clear();
                            let ids = self.ids();
                            let _ = self.table.handle_key(
                                termrock::input::KeyEvent::new(
                                    if matches!(key.code, KeyCode::BackTab) {
                                        KeyCode::Left
                                    } else {
                                        KeyCode::Right
                                    },
                                    KeyModifiers::NONE,
                                ),
                                &ids,
                                &self.columns,
                            );
                            if editable(self.cursor_col()) {
                                self.begin_edit();
                            } else if matches!(key.code, KeyCode::BackTab) {
                                cx.focus_prev();
                            } else {
                                cx.focus_next();
                            }
                            Route::Changed
                        }
                        Err(e) => {
                            cx.status(e);
                            Route::Changed
                        }
                    }
                } else {
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    if ctrl && matches!(key.code, KeyCode::Char('s' | 'S')) {
                        self.request_commit(cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('p')) && key.modifiers.is_empty() {
                        self.open_preview();
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('U')) {
                        self.discard(cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('u')) && key.modifiers.is_empty() {
                        self.undo_last();
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('+')) {
                        self.insert_row(cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('-')) {
                        self.toggle_delete(cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('r')) && !ctrl {
                        self.refresh(cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('f')) && key.modifiers.is_empty() {
                        if let Some(src) = self.cursor_src() {
                            let c = self.cursor_col();
                            cx.status(format!(
                                "Would filter {} = {}",
                                COLS[c],
                                self.value(src, c).text()
                            ));
                        }
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('F')) {
                        cx.status("No filters to clear");
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('/')) {
                        cx.status("The filter editor belongs to the app");
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Down | KeyCode::Char('j' | 'J'))
                        && key.modifiers.is_empty()
                        && !self.table.editing
                    {
                        let last = self.order.len().saturating_sub(1);
                        if self.table.cursor_row >= last && self.loaded < ALL {
                            self.fetch_more(cx);
                            return Route::Changed;
                        }
                    }
                    let ids = self.ids();
                    let ev = self.table.handle_key(*key, &ids, &self.columns);
                    self.on_table(ev, cx)
                }
            }
            PageEvent::Paste(text) => {
                if self.table.editing {
                    self.table.edit_draft.push_str(text);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == PREVIEW {
                    cx.set_focus(*id);
                    self.open_preview();
                    return Route::Changed;
                }
                if *id == DISCARD {
                    self.discard(cx);
                    return Route::Changed;
                }
                if *id == SAVE {
                    self.request_commit(cx);
                    return Route::Changed;
                }
                if *id == GRID {
                    cx.set_focus(*id);
                    let ids = self.ids();
                    let was_row = self.table.cursor_row;
                    let was_col = self.table.cursor_col;
                    let ev = self
                        .table
                        .handle_mouse(mouse_down(*pos), &ids, &mut self.columns);
                    if matches!(ev, DataTableOutcome::CursorMoved)
                        && self.table.cursor_row == was_row
                        && self.table.cursor_col == was_col
                    {
                        self.begin_edit();
                        return Route::Changed;
                    }
                    return self.on_table(ev, cx);
                }
                Route::Ignored
            }
            PageEvent::Drag { pressed, pos } if *pressed == GRID => {
                let ids = self.ids();
                let ev = self.table.handle_mouse(
                    MouseEvent {
                        kind: MouseEventKind::Drag(MouseButton::Left),
                        position: *pos,
                        modifiers: KeyModifiers::NONE,
                    },
                    &ids,
                    &mut self.columns,
                );
                self.on_table(ev, cx)
            }
            PageEvent::Wheel { id, delta } if *id == GRID => {
                let ids = self.ids();
                let ev = self.table.handle_mouse(
                    MouseEvent {
                        kind: if *delta < 0 {
                            MouseEventKind::ScrollUp
                        } else {
                            MouseEventKind::ScrollDown
                        },
                        position: self
                            .table
                            .cell_regions
                            .first()
                            .map(|r| ratatui::layout::Position {
                                x: r.area.x,
                                y: r.area.y,
                            })
                            .unwrap_or_default(),
                        modifiers: KeyModifiers::NONE,
                    },
                    &ids,
                    &mut self.columns,
                );
                self.on_table(ev, cx)
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.table.editing {
            return vec![("Enter", "Commit"), ("Esc", "Cancel"), ("Tab", "Next cell")];
        }
        if focus.is_some_and(|f| [PREVIEW, DISCARD, SAVE].contains(&f)) {
            return vec![("Enter", "Activate"), ("Tab", "Next")];
        }
        vec![
            ("↑↓←→", "Cell"),
            ("Enter", "Edit"),
            ("s", "Sort"),
            ("Space", "Select row"),
            ("+ -", "Insert / delete"),
            ("u", "Undo"),
        ]
    }
}
