// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Pending-change model adapted from junie-tui widgets/grid.rs +
// src/bin/tablepro/tabs.rs (MIT).

//! Result grid and pending edits. Replaces the source DataGrid for TablePro
//! so preview SQL and row edits do not depend on a vendored widget.

use std::collections::HashMap;

use super::db::{ColType, Table, Value};
use crate::text as ttext;

/// Cell stored in a result grid (source `CellValue`).
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,
    Text(String),
    Int(i64),
    Num(f64),
    Bool(bool),
    Json(String),
    Default,
}

impl CellValue {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Null | Self::Default => "NULL".into(),
            Self::Text(s) | Self::Json(s) => s.clone(),
            Self::Int(i) => i.to_string(),
            Self::Num(n) => format!("{n:.2}"),
            Self::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
        }
    }
}

#[must_use]
pub fn to_cell(v: &Value) -> CellValue {
    match v {
        Value::Null => CellValue::Null,
        Value::Text(s) => CellValue::Text(s.clone()),
        Value::Int(i) => CellValue::Int(*i),
        Value::Num(n) => CellValue::Num(*n),
        Value::Bool(b) => CellValue::Bool(*b),
        Value::Json(j) => CellValue::Json(j.clone()),
    }
}

#[must_use]
pub fn from_cell(v: &CellValue) -> Value {
    match v {
        CellValue::Null | CellValue::Default => Value::Null,
        CellValue::Text(s) => Value::Text(s.clone()),
        CellValue::Int(i) => Value::Int(*i),
        CellValue::Num(n) => Value::Num(*n),
        CellValue::Bool(b) => Value::Bool(*b),
        CellValue::Json(j) => Value::Json(j.clone()),
    }
}

/// Pending inserts / cell edits / deletes (source `DataGrid::pending`).
#[derive(Debug, Clone, Default)]
pub struct Pending {
    pub cells: HashMap<(usize, usize), CellValue>,
    pub inserted: Vec<usize>,
    pub deleted: Vec<usize>,
}

impl Pending {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.inserted.is_empty() && self.deleted.is_empty()
    }

    #[must_use]
    pub fn value(&self, row: usize, col: usize) -> Option<&CellValue> {
        self.cells.get(&(row, col))
    }

    /// Source rows that have at least one edited cell (not inserts).
    #[must_use]
    pub fn dirty_rows(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = self
            .cells
            .keys()
            .map(|(r, _)| *r)
            .filter(|r| !self.inserted.contains(r) && !self.deleted.contains(r))
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.dirty_rows().len() + self.inserted.len() + self.deleted.len()
    }
}

/// Browseable / editable result set.
#[derive(Debug, Clone)]
pub struct ResultGrid {
    pub columns: Vec<(String, ColType)>,
    pub rows: Vec<Vec<CellValue>>,
    pub total: usize,
    pub more: bool,
    pub pending: Pending,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub offset: usize,
    /// Source DataGrid `hscroll.offset` (absolute column index).
    pub hscroll: usize,
    pub sort: Option<(usize, bool)>,
    pub editable: bool,
    /// Per-column primary-key flag (source `ColumnSpec::primary`).
    pub primary: Vec<bool>,
    /// Per-column FK flag (source paints `→` on the last cell).
    pub references: Vec<bool>,
}

impl ResultGrid {
    #[must_use]
    pub fn from_values(
        columns: Vec<(String, ColType)>,
        rows: Vec<Vec<Value>>,
        total: usize,
        editable: bool,
    ) -> Self {
        let rows: Vec<Vec<CellValue>> = rows
            .into_iter()
            .map(|r| r.iter().map(to_cell).collect())
            .collect();
        let n = columns.len();
        Self {
            columns,
            rows,
            total,
            more: false,
            pending: Pending::default(),
            cursor_row: 0,
            cursor_col: 0,
            offset: 0,
            hscroll: 0,
            sort: None,
            editable,
            primary: vec![false; n],
            references: vec![false; n],
        }
    }

    /// Copy PK / FK flags from the live table (source `column_specs`).
    pub fn annotate(&mut self, table: &Table) {
        for (i, (name, _)) in self.columns.iter().enumerate() {
            let Some(col) = table.column(name) else {
                continue;
            };
            if let Some(slot) = self.primary.get_mut(i) {
                *slot = col.primary;
            }
            if let Some(slot) = self.references.get_mut(i) {
                *slot = col.references.is_some();
            }
        }
    }

    /// Source `CellKind::default_width`.
    #[must_use]
    pub fn kind_bounds(ty: ColType) -> (u16, u16) {
        match ty {
            ColType::Uuid => (9, 36),
            ColType::Text => (6, 40),
            ColType::Int | ColType::Numeric => (4, 22),
            ColType::Bool => (5, 5),
            ColType::Timestamp | ColType::Date => (10, 29),
            ColType::Json => (8, 40),
            ColType::Enum => (6, 16),
        }
    }

    /// Source `DataGrid::sample_widths` for one column.
    #[must_use]
    pub fn sampled_width(&self, col: usize) -> u16 {
        let Some((name, ty)) = self.columns.get(col) else {
            return 8;
        };
        let (min_width, max_width) = Self::kind_bounds(*ty);
        let primary = self.primary.get(col).copied().unwrap_or(false);
        let mut ws: Vec<usize> = self
            .rows
            .iter()
            .take(200)
            .map(|r| ttext::width(&r.get(col).map(CellValue::display).unwrap_or_default()))
            .collect();
        ws.sort_unstable();
        let p95 = u16::try_from(ws.get(ws.len() * 95 / 100).copied().unwrap_or(0)).unwrap_or(0);
        let sorted = self.sort.is_some_and(|(c, _)| c == col);
        // Source `fit_header_marks`: name + primary 2 + sorted 2 + 1.
        // Idle pad is +2; sorted adds one more so `"status ▴"` fits at 9.
        let header = u16::try_from(ttext::width(name)).unwrap_or(0)
            + if primary { 2 } else { 0 }
            + if sorted { 1 } else { 0 }
            + 2;
        let max = max_width.max(header.min(24));
        p95.max(header).clamp(min_width.min(max), max)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn cell(&self, row: usize, col: usize) -> CellValue {
        if let Some(v) = self.pending.value(row, col) {
            return v.clone();
        }
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or(CellValue::Null)
    }

    pub fn record_cell(&mut self, row: usize, col: usize, value: CellValue) {
        let original = self
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or(CellValue::Null);
        if original == value {
            self.pending.cells.remove(&(row, col));
        } else {
            self.pending.cells.insert((row, col), value);
        }
    }

    pub fn toggle_delete(&mut self, row: usize) {
        if let Some(i) = self.pending.deleted.iter().position(|&r| r == row) {
            self.pending.deleted.remove(i);
        } else {
            self.pending.deleted.push(row);
        }
    }

    pub fn move_cursor(&mut self, drow: isize, dcol: isize) {
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        let nr = self.rows.len() as isize;
        let nc = self.columns.len() as isize;
        let r = (self.cursor_row as isize + drow).clamp(0, nr - 1) as usize;
        let c = (self.cursor_col as isize + dcol).clamp(0, nc - 1) as usize;
        self.cursor_row = r;
        self.cursor_col = c;
    }

    /// Source `DataGrid::ensure_cursor_visible` for columns.
    pub fn ensure_hscroll(&mut self, viewport: usize) {
        let vp = viewport.max(1);
        let n = self.columns.len().max(1);
        if self.cursor_col < self.hscroll {
            self.hscroll = self.cursor_col;
        } else if self.cursor_col >= self.hscroll.saturating_add(vp) {
            self.hscroll = self.cursor_col.saturating_add(1).saturating_sub(vp);
        }
        self.hscroll = self.hscroll.min(n.saturating_sub(vp.min(n)));
    }
}
