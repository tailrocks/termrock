// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Pending-change model adapted from junie-tui widgets/grid.rs +
// src/bin/tablepro/tabs.rs (MIT).

//! Result grid and pending edits. Replaces the source DataGrid for TablePro
//! so preview SQL and row edits do not depend on a vendored widget.

use std::collections::HashMap;

use super::db::{ColType, Value};

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
    pub sort: Option<(usize, bool)>,
    pub editable: bool,
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
        Self {
            columns,
            rows,
            total,
            more: false,
            pending: Pending::default(),
            cursor_row: 0,
            cursor_col: 0,
            offset: 0,
            sort: None,
            editable,
        }
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
}
