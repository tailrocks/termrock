//! The selectable keys of one accepted result source, in visible order.

use super::{ResultRowKind, SearchError, SearchSource};
use crate::core::id::ItemKey;
use crate::core::list::{KeyedRowSource, RowSource, SourceStamp};
use crate::core::selection::ClickPolicy;

/// One selectable item or cell. Slots are sorted by `row`, then `cell`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Slot {
    pub key: ItemKey,
    pub row: usize,
    pub cell: usize,
    pub enabled: bool,
    pub in_cells: bool,
    pub policy: Option<ClickPolicy>,
}

/// Checks every row against its kind and lists the selectable keys. Nothing
/// is kept when a row is inconsistent.
pub(super) fn build<S: SearchSource + ?Sized>(source: &S) -> Result<Vec<Slot>, SearchError> {
    let mut slots = Vec::new();
    for row in 0..source.len() {
        let cells = source.cell_count(row);
        match source.row_kind(row) {
            ResultRowKind::Heading | ResultRowKind::Spacer => {
                if cells != 0 {
                    return Err(SearchError::InconsistentRow { row });
                }
            }
            ResultRowKind::Item => {
                if cells != 1 || source.cell_key(row, 0) != source.key(row) {
                    return Err(SearchError::InconsistentRow { row });
                }
                slots.push(Slot {
                    key: source.key(row),
                    row,
                    cell: 0,
                    enabled: source.enabled(row),
                    in_cells: false,
                    policy: source.cell_click_policy(row, 0),
                });
            }
            ResultRowKind::Cells => {
                let row_enabled = source.enabled(row);
                for cell in 0..cells {
                    slots.push(Slot {
                        key: source.cell_key(row, cell),
                        row,
                        cell,
                        enabled: row_enabled && source.cell_enabled(row, cell),
                        in_cells: true,
                        policy: source.cell_click_policy(row, cell),
                    });
                }
            }
        }
    }
    Ok(slots)
}

/// Presents the slots as the visible rows of a keyed source whose authoritative
/// keys are the source's own.
pub(super) struct Selectable<'a, S: ?Sized> {
    pub source: &'a S,
    pub slots: &'a [Slot],
}

impl<S: SearchSource + ?Sized> RowSource for Selectable<'_, S> {
    fn len(&self) -> usize {
        self.slots.len()
    }

    fn enabled(&self, i: usize) -> bool {
        self.slots[i].enabled
    }

    fn label(&self, i: usize) -> &str {
        let slot = &self.slots[i];
        self.source.cell_label(slot.row, slot.cell)
    }

    fn key(&self, i: usize) -> ItemKey {
        self.slots[i].key
    }
}

impl<S: SearchSource + ?Sized> KeyedRowSource for Selectable<'_, S> {
    fn stamp(&self) -> SourceStamp {
        self.source.stamp()
    }

    fn source_len(&self) -> usize {
        self.source.source_len()
    }

    fn source_key(&self, source_index: usize) -> ItemKey {
        self.source.source_key(source_index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Move {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp(usize),
    PageDown(usize),
}

fn row_start(slots: &[Slot], row: usize) -> usize {
    slots.partition_point(|s| s.row < row)
}

fn row_end(slots: &[Slot], row: usize) -> usize {
    slots.partition_point(|s| s.row <= row)
}

/// The enabled slot of `slots[start..end]` nearest to `column`, lower first.
fn pick(slots: &[Slot], start: usize, end: usize, column: usize) -> Option<usize> {
    (start..end)
        .filter(|&i| slots[i].enabled)
        .min_by_key(|&i| (slots[i].cell.abs_diff(column), slots[i].cell))
}

/// The first row at or after slot `from` that has an enabled slot.
fn scan_forward(slots: &[Slot], mut from: usize, column: usize) -> Option<usize> {
    while from < slots.len() {
        let end = row_end(slots, slots[from].row);
        if let Some(found) = pick(slots, from, end, column) {
            return Some(found);
        }
        from = end;
    }
    None
}

/// The last row ending at or before slot `until` that has an enabled slot.
fn scan_backward(slots: &[Slot], mut until: usize, column: usize) -> Option<usize> {
    while until > 0 {
        let start = row_start(slots, slots[until - 1].row);
        if let Some(found) = pick(slots, start, until, column) {
            return Some(found);
        }
        until = start;
    }
    None
}

/// Where `mv` lands from slot `from`, or `None` when it has nowhere to go.
/// `column` is the cell column vertical moves try to keep.
pub(super) fn target(
    slots: &[Slot],
    from: Option<usize>,
    column: usize,
    mv: Move,
) -> Option<usize> {
    let Some(at) = from else {
        return match mv {
            Move::Up | Move::End | Move::PageUp(_) => scan_backward(slots, slots.len(), column),
            _ => scan_forward(slots, 0, column),
        };
    };
    let row = slots[at].row;
    match mv {
        Move::Down => scan_forward(slots, row_end(slots, row), column),
        Move::Up => scan_backward(slots, row_start(slots, row), column),
        Move::PageDown(rows) => {
            let ahead = row_start(slots, row.saturating_add(rows.max(1)));
            scan_forward(slots, ahead, column).or_else(|| scan_backward(slots, slots.len(), column))
        }
        Move::PageUp(rows) => {
            let behind = row_end(slots, row.saturating_sub(rows.max(1)));
            scan_backward(slots, behind, column).or_else(|| scan_forward(slots, 0, column))
        }
        Move::Home => scan_forward(slots, 0, column),
        Move::End => scan_backward(slots, slots.len(), column),
        Move::Left => {
            let start = row_start(slots, row);
            (start..at).rev().find(|&i| slots[i].enabled)
        }
        Move::Right => {
            let end = row_end(slots, row);
            (at + 1..end).find(|&i| slots[i].enabled)
        }
    }
}
