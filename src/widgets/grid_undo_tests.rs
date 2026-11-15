use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::core::event::Key;
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::{ItemKey, WidgetId};
use crate::core::list::RowSource;
use crate::core::selection::SelectGesture;
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::widgets::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridCmd, GridDataError,
    GridEditError, GridKeyError, GridRevision, GridRows, GridUndo, GridUndoKind, PendingChanges,
    RowState, RowTotal,
};
use crate::widgets::table::SortDir;

fn commit(g: &mut DataGrid) {
    let snapshot = g.snapshot_changes();
    let receipt = snapshot.identity_receipt();
    let token = g.begin_submission(snapshot).unwrap();
    g.complete_submission(token, Ok(receipt)).unwrap();
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new("name", CellKind::Text),
        ColumnSpec::new("note", CellKind::Text),
    ]
}

fn page(n: usize, more: bool) -> GridRows {
    GridRows {
        rows: (0..n)
            .map(|i| vec![text(&format!("name{i}")), text(&format!("note{i}"))])
            .collect(),
        total: RowTotal::Exact(n),
        more,
    }
}

fn grid(n: usize) -> DataGrid {
    let mut g = DataGrid::new(WidgetId::of("issue33"), columns());
    g.try_set_rows(page(n, false)).unwrap();
    g
}

fn press(g: &mut DataGrid, code: KeyCode, mods: KeyModifiers) -> Option<GridAction> {
    g.on_key(&Key { code, mods })
        .try_into_single()
        .unwrap_or_else(|_| panic!("single action expected"))
        .1
}

fn undone(g: &mut DataGrid) -> GridUndo {
    match press(g, KeyCode::Char('u'), KeyModifiers::NONE) {
        Some(GridAction::Undone(u)) => u,
        other => panic!("undo reports Undone, got {other:?}"),
    }
}

fn key_of(g: &DataGrid, src: usize) -> ItemKey {
    g.row_key(src).expect("a live row")
}

fn render(g: &mut DataGrid) -> String {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    g.render(area, &mut buf, &mut ctx, theme.canvas);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_consistent(g: &DataGrid) {
    let n = g.len();
    let keys: HashSet<ItemKey> = (0..n).filter_map(|s| g.row_key(s)).collect();
    assert_eq!(keys.len(), n, "every row has a unique key");
    for s in 0..n {
        assert_eq!(g.source_index(key_of(g, s)), Some(s), "lookup is inverse");
    }
    let mut order: Vec<usize> = (0..n).map(|d| g.source_row(d)).collect();
    order.sort_unstable();
    assert_eq!(order, (0..n).collect::<Vec<_>>(), "order is a permutation");
    assert!(g.selection.iter().all(|s| s < n), "selection in range");
    assert!(g.cell_errors.keys().all(|(r, _)| *r < n), "cell errors");
    assert!(g.row_errors.keys().all(|r| *r < n), "row errors");
    for k in g.pending().inserted().chain(g.pending().deleted()) {
        assert!(g.source_index(k).is_some(), "pending key is live");
    }
    for (k, c, _) in g.pending().cells() {
        assert!(g.source_index(k).is_some(), "no phantom pending cell");
        assert!(c < g.columns().len());
    }
}

#[test]
fn issue33_delete_undelete_restores_staged_cells() {
    let mut g = grid(2);
    let k0 = key_of(&g, 0);
    assert_eq!(
        g.record_cell(0, 0, text("new")),
        Ok(CellEditResult::Changed)
    );

    g.toggle_delete(0).unwrap();
    assert_eq!(
        g.pending().counts(),
        (0, 0, 1),
        "deleted rows are not updates"
    );
    assert_eq!(g.pending().value(k0, 0), Some(&text("new")), "one store");
    assert_eq!(g.value(0, 0), &text("new"));

    let u = undone(&mut g);
    assert_eq!(
        u,
        GridUndo {
            key: k0,
            kind: GridUndoKind::DeleteMark
        }
    );
    assert_eq!(g.pending().counts(), (1, 0, 0));
    assert_eq!(g.value(0, 0), &text("new"), "undo restores the staged cell");

    g.toggle_delete(0).unwrap();
    g.toggle_delete(0).unwrap();
    assert_eq!(g.value(0, 0), &text("new"), "delete twice keeps the edit");
    assert_eq!(g.pending().counts(), (1, 0, 0));

    assert_eq!(
        g.undo().unwrap().map(|u| u.kind),
        Some(GridUndoKind::DeleteMark)
    );
    assert_eq!(
        g.undo().unwrap().map(|u| u.kind),
        Some(GridUndoKind::DeleteMark)
    );
    assert_eq!(g.undo().unwrap().map(|u| u.kind), Some(GridUndoKind::Cell));
    assert_eq!(g.value(0, 0), &text("name0"));
    assert_eq!(g.pending(), &PendingChanges::default());
    assert_eq!(g.undo().unwrap(), None);
}

#[test]
fn issue33_undo_repaints_staged_value() {
    let mut g = grid(2);
    g.record_cell(0, 0, text("fresh")).unwrap();
    g.toggle_delete(0).unwrap();
    let struck = render(&mut g);
    assert!(struck.contains("fresh"), "the struck row shows its draft");
    assert!(struck.contains("1 delete"), "{struck}");

    assert_eq!(undone(&mut g).kind, GridUndoKind::DeleteMark);
    let back = render(&mut g);
    assert!(back.contains("fresh"), "{back}");
    assert!(back.contains("1 update"), "{back}");
    assert_eq!(g.row_state(0), RowState::Modified);
}

#[test]
fn issue33_remove_inserted_before_edited_row() {
    let mut g = grid(2);
    g.insert_row().unwrap();
    g.insert_row().unwrap();
    let (ka, kb) = (key_of(&g, 2), key_of(&g, 3));
    assert_eq!(g.record_cell(3, 0, text("b1")), Ok(CellEditResult::Changed));
    assert_eq!(g.record_cell(3, 0, text("b2")), Ok(CellEditResult::Changed));

    g.toggle_delete(2).unwrap();
    assert_eq!(g.len(), 3);
    assert_eq!(g.source_index(kb), Some(2), "B shifted but kept its key");
    assert_eq!(g.value(2, 0), &text("b2"));
    assert_consistent(&g);

    let first = g.undo().unwrap().expect("restores the removed insert");
    assert_eq!(
        first,
        GridUndo {
            key: ka,
            kind: GridUndoKind::RemovedInsert
        }
    );
    assert_eq!((g.source_index(ka), g.source_index(kb)), (Some(2), Some(3)));
    assert_eq!(g.value(3, 0), &text("b2"));

    let second = g.undo().unwrap().expect("undoes the B edit");
    assert_eq!(
        second,
        GridUndo {
            key: kb,
            kind: GridUndoKind::Cell
        }
    );
    assert_eq!(g.value(3, 0), &text("b1"));
    assert_eq!(g.pending().cells().count(), 1, "no phantom pending cell");
    assert_consistent(&g);
}

#[test]
fn issue33_removed_insert_restores_complete_row() {
    let mut g = grid(3);
    g.cursor = (0, 0);
    g.duplicate_row().unwrap();
    let ka = key_of(&g, 3);
    g.insert_row().unwrap();
    let kb = key_of(&g, 4);
    g.record_cell(3, 1, text("dup-note")).unwrap();
    g.record_cell(4, 0, text("b")).unwrap();
    g.cell_errors.insert((3, 0), "bad cell".into());
    g.row_errors.insert(3, "bad row".into());

    g.cursor = (3, 0);
    press(&mut g, KeyCode::Down, KeyModifiers::SHIFT);
    g.select_rows(SelectGesture::Toggle, 3);
    g.select_rows(SelectGesture::Toggle, 4);
    let range = g.run(GridCmd::Copy).1;
    let marked = g.selection.to_vec();
    let before_rows = g.rows().to_vec();
    let before_cursor = g.cursor;
    let before_counts = g.pending().counts();

    assert!(g.begin_edit().actions().next().is_none() && g.is_editing());
    g.toggle_delete(3).unwrap();
    assert!(!g.is_editing(), "removal closes the editor");
    assert_eq!(g.selection.to_vec(), vec![3], "B keeps its mark, shifted");
    assert!(g.cell_errors.is_empty() && g.row_errors.is_empty());

    let u = g.undo().unwrap().expect("restores");
    assert_eq!(u.kind, GridUndoKind::RemovedInsert);
    assert!(!g.is_editing(), "undo does not resurrect an editor");
    assert_eq!(g.rows(), &before_rows[..]);
    assert_eq!((g.source_index(ka), g.source_index(kb)), (Some(3), Some(4)));
    assert_eq!(g.value(3, 1), &text("dup-note"));
    assert_eq!(g.pending().counts(), before_counts);
    assert_eq!(
        g.cell_errors.get(&(3, 0)).map(String::as_str),
        Some("bad cell")
    );
    assert_eq!(g.row_errors.get(&3).map(String::as_str), Some("bad row"));
    assert_eq!(g.selection.to_vec(), marked);
    assert_eq!(g.cursor, before_cursor);
    assert_eq!(
        g.run(GridCmd::Copy).1,
        range,
        "the cell range anchor is back"
    );
    assert_eq!(g.row_state(3), RowState::Error);
    assert_consistent(&g);
}

#[test]
fn issue33_undo_reports_affected_key() {
    let mut g = grid(3);
    let k1 = key_of(&g, 1);
    assert_eq!(g.record_cell(1, 0, text("x")), Ok(CellEditResult::Changed));
    let draft = g.revision().draft;
    assert_eq!(
        g.record_cell(1, 0, text("x")),
        Ok(CellEditResult::Unchanged),
        "an unchanged edit is ignored"
    );
    assert_eq!(g.revision().draft, draft, "and does not touch the revision");

    g.cursor = (2, 1);
    let u = undone(&mut g);
    assert_eq!(
        u,
        GridUndo {
            key: k1,
            kind: GridUndoKind::Cell
        },
        "the action names the edited row, not the cursor row"
    );
    assert_eq!(g.value(1, 0), &text("name1"));
    assert_eq!(
        press(&mut g, KeyCode::Char('u'), KeyModifiers::NONE),
        None,
        "the ignored edit left no history"
    );
}

#[test]
fn issue33_key_revision_lifecycle() {
    let mut g = grid(2);
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 1,
            draft: 0
        }
    );
    let mut seen: HashSet<ItemKey> = (0..2).map(|s| key_of(&g, s)).collect();

    g.try_append_rows(page(1, false)).unwrap();
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 2,
            draft: 0
        }
    );
    assert!(seen.insert(key_of(&g, 2)), "an appended row gets a new key");

    g.record_cell(0, 0, text("e")).unwrap();
    g.insert_row().unwrap();
    assert!(seen.insert(key_of(&g, 3)));
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 2,
            draft: 2
        }
    );

    g.toggle_delete(3).unwrap();
    g.insert_row().unwrap();
    assert!(
        seen.insert(key_of(&g, 3)),
        "a removed key is never reissued"
    );
    g.undo().unwrap();
    g.undo().unwrap();
    assert_eq!(g.revision().draft, 6);

    g.select_rows(SelectGesture::All, 0);
    g.cursor = (1, 1);
    g.local_sort = true;
    g.run(GridCmd::Sort);
    assert_eq!(g.revision().draft, 6, "view-only changes keep the revision");

    g.record_cell(0, 0, text("again")).unwrap();
    g.discard().unwrap();
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 2,
            draft: 8
        }
    );
    g.discard().unwrap();
    assert_eq!(g.revision().draft, 8, "an empty discard is not a change");

    g.record_cell(1, 0, text("kept")).unwrap();
    commit(&mut g);
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 3,
            draft: 10
        }
    );

    g.record_cell(1, 0, text("stale")).unwrap();
    g.try_set_rows(page(2, false)).unwrap();
    assert_eq!(
        g.revision(),
        GridRevision {
            source: 4,
            draft: 12
        }
    );
    for s in 0..2 {
        assert!(seen.insert(key_of(&g, s)), "replacement never reuses keys");
    }
    g.insert_row().unwrap();
    assert!(seen.insert(key_of(&g, 2)));
}

#[test]
fn issue33_keyed_setters_validate_before_install() {
    let mut g = grid(2);
    g.record_cell(0, 0, text("draft")).unwrap();
    let (rows, revision) = (g.rows().to_vec(), g.revision());
    let k = ItemKey::num(7);

    assert_eq!(
        g.try_set_rows_keyed(page(2, false), vec![k]),
        Err(GridDataError::Key(GridKeyError::KeyCount {
            expected: 2,
            actual: 1
        }))
    );
    assert_eq!(
        g.try_set_rows_keyed(page(2, false), vec![k, k]),
        Err(GridDataError::Key(GridKeyError::DuplicateKey(k)))
    );
    assert_eq!(
        g.try_set_model_keyed(
            vec![ColumnSpec::new("only", CellKind::Text)],
            page(2, false),
            vec![k, k]
        ),
        Err(GridDataError::Key(GridKeyError::DuplicateKey(k)))
    );
    assert_eq!((g.rows(), g.revision()), (&rows[..], revision), "untouched");
    assert_eq!(g.columns().len(), 2);
    assert_eq!(
        g.pending().counts(),
        (1, 0, 0),
        "the draft survives a rejection"
    );

    let keys = vec![ItemKey::num(2), ItemKey::text("b")];
    g.try_set_rows_keyed(page(2, false), keys.clone()).unwrap();
    assert_eq!((g.row_key(0), g.row_key(1)), (Some(keys[0]), Some(keys[1])));
    assert_eq!(g.source_index(keys[1]), Some(1));
    assert!(g.pending().is_empty(), "replacement clears the old epoch");

    g.insert_row().unwrap();
    let local = key_of(&g, 2);
    assert!(!keys.contains(&local), "a local key skips provider keys");

    let model = vec![ColumnSpec::new("only", CellKind::Text)];
    let one = GridRows {
        rows: vec![vec![text("x")]],
        total: RowTotal::Exact(1),
        more: false,
    };
    g.try_set_model_keyed(model, one, vec![ItemKey::text("solo")])
        .unwrap();
    assert_eq!(g.columns().len(), 1);
    assert_eq!(g.row_key(0), Some(ItemKey::text("solo")));
}

#[test]
fn issue33_append_between_remove_and_undo() {
    let mut g = grid(2);
    g.insert_row().unwrap();
    let ka = key_of(&g, 2);
    g.record_cell(2, 0, text("A")).unwrap();
    g.toggle_delete(2).unwrap();

    g.try_append_rows(page(1, false)).unwrap();
    let kc = key_of(&g, 2);
    g.cell_errors.insert((2, 1), "c broke".into());
    g.select_rows(SelectGesture::Only, 2);

    assert_eq!(g.undo().unwrap().map(|u| u.key), Some(ka));
    assert_eq!(g.len(), 4);
    assert_eq!(g.source_index(ka), Some(2));
    assert_eq!(
        g.source_index(kc),
        Some(3),
        "C keeps its key and moves down"
    );
    assert_eq!(g.value(3, 0), &text("name0"));
    assert_eq!(
        g.cell_errors.get(&(3, 1)).map(String::as_str),
        Some("c broke")
    );
    assert_eq!(g.selection.to_vec(), vec![3], "the live selection survives");
    let order: Vec<usize> = (0..4).map(|d| g.source_row(d)).collect();
    assert_eq!(
        order,
        vec![0, 1, 2, 3],
        "no duplicate or missing display row"
    );
    assert_consistent(&g);

    let mut s = grid(2);
    s.local_sort = true;
    s.sort = Some((0, SortDir::Desc));
    s.try_set_rows(page(2, false)).unwrap();
    s.insert_row().unwrap();
    let ka = key_of(&s, 2);
    s.toggle_delete(2).unwrap();
    s.try_append_rows(page(1, false)).unwrap();
    let kc = key_of(&s, 2);
    s.undo().unwrap();
    let by_display: Vec<ItemKey> = (0..4)
        .map(|d| s.row_key(s.source_row(d)).unwrap())
        .collect();
    assert_eq!(by_display.last(), Some(&ka), "the draft sits in the tail");
    assert!(
        by_display[..3].contains(&kc),
        "the fetched row sorts into the base"
    );
    assert_eq!(
        (0..3)
            .map(|d| s.value(s.source_row(d), 0).clone())
            .collect::<Vec<_>>(),
        vec![text("name1"), text("name0"), text("name0")],
        "base rows follow the sort"
    );
    assert_consistent(&s);
}

#[test]
fn issue33_sorted_repeated_undo_reaches_baseline() {
    let mut g = grid(4);
    g.local_sort = true;
    g.run(GridCmd::Sort);
    let baseline_rows = g.rows().to_vec();
    let baseline_keys: Vec<ItemKey> = (0..4).map(|s| key_of(&g, s)).collect();
    let baseline_order: Vec<usize> = (0..4).map(|d| g.source_row(d)).collect();

    g.record_cell(1, 0, text("zzz")).unwrap();
    g.insert_row().unwrap();
    g.record_cell(4, 0, text("ins")).unwrap();
    g.toggle_delete(2).unwrap();
    g.cursor = (0, 0);
    g.duplicate_row().unwrap();
    g.select_rows(SelectGesture::Toggle, 1);
    g.cell_errors.insert((4, 0), "err".into());
    g.toggle_delete(4).unwrap();
    g.record_cell(3, 1, text("tail")).unwrap();
    assert_consistent(&g);

    while g.undo().unwrap().is_some() {
        assert_consistent(&g);
    }
    assert_eq!(g.rows(), &baseline_rows[..]);
    assert_eq!(g.pending(), &PendingChanges::default());
    assert_eq!(
        (0..4).map(|s| key_of(&g, s)).collect::<Vec<_>>(),
        baseline_keys
    );
    assert_eq!(
        (0..4).map(|d| g.source_row(d)).collect::<Vec<_>>(),
        baseline_order
    );
    assert!(g.cursor.0 < g.len());
}

#[test]
fn issue33_invalid_row_mutation_leaves_history_unchanged() {
    let mut g = grid(2);
    let revision = g.revision();
    assert_eq!(g.toggle_delete(9), Ok(None));
    assert_eq!(
        g.record_cell(9, 0, text("x")),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(
        g.record_cell(0, 9, text("x")),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(g.undo().unwrap(), None);
    assert_eq!(g.pending(), &PendingChanges::default());
    assert_eq!(g.revision(), revision);

    g.record_cell(0, 0, text("ok")).unwrap();
    assert_eq!(g.toggle_delete(9), Ok(None));
    assert_eq!(g.undo().unwrap().map(|u| u.kind), Some(GridUndoKind::Cell));
    assert_eq!(
        g.undo().unwrap(),
        None,
        "the rejected calls added no history"
    );
    assert_consistent(&g);
}

#[test]
fn issue33_bulk_delete_of_marked_rows_follows_keys() {
    let mut g = grid(2);
    g.insert_row().unwrap();
    g.insert_row().unwrap();
    let kb = key_of(&g, 3);
    for src in [0, 2, 3] {
        g.selection.set(src, true);
    }
    g.run(GridCmd::ClearCell);
    assert_eq!(g.len(), 2, "both inserted rows are gone");
    assert_eq!(
        g.pending().counts(),
        (0, 0, 1),
        "only the original is marked"
    );
    assert!(g.pending().is_deleted(key_of(&g, 0)));
    assert_eq!(g.source_index(kb), None);
    assert_consistent(&g);
    let kinds: Vec<_> = (0..3)
        .filter_map(|_| g.undo().unwrap().map(|u| u.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            GridUndoKind::RemovedInsert,
            GridUndoKind::RemovedInsert,
            GridUndoKind::DeleteMark
        ]
    );
    assert_eq!(g.len(), 4);
    assert_eq!(g.pending().counts(), (0, 2, 0));
    assert_consistent(&g);
}

#[test]
fn issue33_commit_keeps_keys_and_order_of_survivors() {
    let mut g = grid(4);
    g.local_sort = true;
    g.run(GridCmd::Sort);
    g.run(GridCmd::Sort);
    let (k0, k2, k3) = (key_of(&g, 0), key_of(&g, 2), key_of(&g, 3));
    g.record_cell(2, 0, text("saved")).unwrap();
    g.record_cell(1, 0, text("gone-draft")).unwrap();
    g.toggle_delete(1).unwrap();
    g.insert_row().unwrap();
    let kn = key_of(&g, 4);
    g.selection.set(3, true);
    commit(&mut g);

    assert_eq!(g.len(), 4, "the deleted row left, the inserted row stayed");
    assert_eq!(
        (g.row_key(0), g.row_key(1), g.row_key(2), g.row_key(3)),
        (Some(k0), Some(k2), Some(k3), Some(kn))
    );
    assert_eq!(g.value(1, 0), &text("saved"));
    assert_eq!(g.selection.to_vec(), vec![2], "the mark followed its row");
    assert_eq!(g.pending(), &PendingChanges::default());
    assert_eq!(g.undo().unwrap(), None);
    assert_consistent(&g);
}

#[test]
fn issue33_row_source_key_follows_display_order() {
    let mut g = grid(3);
    g.more = true;
    g.local_sort = true;
    g.run(GridCmd::Sort);
    g.run(GridCmd::Sort);
    for d in 0..3 {
        assert_eq!(RowSource::key(&g, d), key_of(&g, g.source_row(d)));
    }
    let more = RowSource::key(&g, 3);
    assert!(
        (0..3).all(|s| key_of(&g, s) != more),
        "the fetch row is no record"
    );
}
