use super::*;
use crate::core::id::WidgetId;
use crate::widgets::grid::{CellKind, ColumnSpec, GridRows, RowTotal};
use crate::widgets::table::SortDir;

const ID: usize = 0;
const NAME: usize = 1;
const QTY: usize = 2;
const FLAG: usize = 3;
const NOTE: usize = 4;

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new("id", CellKind::Id).primary().read_only(),
        ColumnSpec::new("name", CellKind::Text),
        ColumnSpec::new("qty", CellKind::Number),
        ColumnSpec::new("flag", CellKind::Bool),
        ColumnSpec::new("note", CellKind::Text).nullable(true),
    ]
}

fn source_row(i: usize) -> Vec<CellValue> {
    vec![
        CellValue::Int(100 + i as i64),
        text(&format!("name{i}")),
        CellValue::Num(1.5 + i as f64),
        CellValue::Bool(i % 2 == 0),
        if i % 3 == 0 {
            CellValue::Null
        } else {
            text(&format!("note{i}"))
        },
    ]
}

fn grid(n: usize) -> DataGrid {
    let mut g = DataGrid::new(WidgetId::of("issue50"), columns());
    g.set_rows(GridRows {
        rows: (0..n).map(source_row).collect(),
        total: RowTotal::Exact(n),
        more: false,
    });
    g
}

fn assert_valid(g: &DataGrid, set: &GridChangeSet) {
    let mut seen = std::collections::HashSet::new();
    let keys = set
        .inserted()
        .iter()
        .map(InsertedRow::key)
        .chain(set.updated().iter().map(UpdatedRow::key))
        .chain(set.deleted().iter().map(DeletedRow::key));
    for key in keys {
        assert!(seen.insert(key), "operation classes are disjoint: {key:?}");
        assert!(g.source_index(key).is_some(), "no nonexistent key");
    }
    let width = g.columns.len();
    assert!(set.inserted().iter().all(|r| r.values().len() == width));
    assert!(set.deleted().iter().all(|r| r.before().len() == width));
    for row in set.updated() {
        assert_eq!(row.before().len(), width);
        let cols: Vec<usize> = row.cells().iter().map(CellChange::column).collect();
        assert!(cols.windows(2).all(|w| w[0] < w[1]), "ascending columns");
        assert!(cols.iter().all(|&c| c < width), "no nonexistent column");
    }
    let order = |keys: Vec<ItemKey>| -> Vec<usize> {
        keys.into_iter()
            .map(|k| g.source_index(k).unwrap())
            .collect()
    };
    for rows in [
        order(set.inserted().iter().map(InsertedRow::key).collect()),
        order(set.updated().iter().map(UpdatedRow::key).collect()),
        order(set.deleted().iter().map(DeletedRow::key).collect()),
    ] {
        assert!(rows.windows(2).all(|w| w[0] < w[1]), "source order");
    }
}

#[test]
fn issue50_duplicate_snapshot_contains_complete_row() {
    let mut g = grid(2);
    g.cursor = (1, NAME);
    g.duplicate_row().expect("the row duplicates");

    assert_eq!(
        g.pending().cells().count(),
        0,
        "the copies live in the stored row, not in pending cells"
    );
    let set = g.snapshot_changes();
    assert_valid(&g, &set);
    assert_eq!(set.counts(), (0, 1, 0));
    let row = &set.inserted()[0];
    assert_eq!(
        row.values(),
        &[
            CellValue::Default,
            text("name1"),
            CellValue::Num(2.5),
            CellValue::Bool(false),
            text("note1"),
        ],
        "every copied writable value, the primary key reset to Default"
    );
    assert_eq!(Some(row.key()), g.row_key(2));

    let mut g = grid(1);
    g.duplicate_row().expect("the row duplicates");
    assert_eq!(
        g.snapshot_changes().inserted()[0].values()[NOTE],
        CellValue::Null,
        "an explicit Null stays Null"
    );
}

#[test]
fn issue50_fresh_insert_keeps_null_and_default_distinct() {
    let mut g = grid(1);
    g.insert_row().expect("the row inserts");
    let set = g.snapshot_changes();
    assert_eq!(
        set.inserted()[0].values(),
        &[
            CellValue::Default,
            CellValue::Null,
            CellValue::Null,
            CellValue::Null,
            CellValue::Null,
        ]
    );
}

#[test]
fn issue50_duplicate_override_and_dirty_source() {
    let mut g = grid(2);
    assert!(g.record_cell(0, NAME, text("edited")));
    g.cursor = (0, NAME);
    g.duplicate_row().expect("the row duplicates");
    let copy = g.len() - 1;

    let set = g.snapshot_changes();
    assert_valid(&g, &set);
    assert_eq!(
        set.inserted()[0].values()[NAME],
        text("edited"),
        "duplicating resolves the source's pending override"
    );
    assert_eq!(set.counts(), (1, 1, 0));
    let update = &set.updated()[0];
    assert_eq!(
        update.before()[NAME],
        text("name0"),
        "update keeps its base"
    );
    assert_eq!(update.cells().len(), 1);

    assert!(g.record_cell(copy, QTY, CellValue::Num(9.0)));
    let set = g.snapshot_changes();
    let values = set.inserted()[0].values();
    assert_eq!(values[QTY], CellValue::Num(9.0), "the override applies");
    assert_eq!(values[NAME], text("edited"), "the other copies remain");
    assert_eq!(values[FLAG], CellValue::Bool(true));
    assert_eq!(values[ID], CellValue::Default);
}

#[test]
fn issue50_deleted_draft_never_emits_update() {
    let mut g = grid(3);
    assert!(g.record_cell(1, NAME, text("draft")));
    g.toggle_delete(1);

    let set = g.snapshot_changes();
    assert_valid(&g, &set);
    assert_eq!(set.counts(), (0, 0, 1));
    assert_eq!(set.deleted()[0].before(), source_row(1).as_slice());

    g.undo().expect("undo the delete mark");
    let set = g.snapshot_changes();
    assert_eq!(set.counts(), (1, 0, 0));
    let change = &set.updated()[0].cells()[0];
    assert_eq!(change.column(), NAME);
    assert_eq!(change.before(), &text("name1"));
    assert_eq!(change.after(), &text("draft"));
}

#[test]
fn issue50_removed_inserted_row_is_absent() {
    let mut g = grid(1);
    g.insert_row().expect("the row inserts");
    g.toggle_delete(1);
    assert!(g.snapshot_changes().is_empty());
}

#[test]
fn issue50_semantic_noop_overrides_produce_no_update() {
    let mut g = grid(1);
    g.record_cell(0, QTY, CellValue::Num(9.0));
    g.record_cell(0, QTY, CellValue::Num(1.5));
    assert!(g.snapshot_changes().is_empty(), "back at the base value");

    let mut g = grid(1);
    g.rows[0][QTY] = CellValue::Num(0.0);
    g.pending
        .cells
        .insert((g.row_key(0).unwrap(), QTY), CellValue::Num(-0.0));
    g.pending
        .cells
        .insert((g.row_key(0).unwrap(), NAME), text("changed"));
    let set = g.snapshot_changes();
    assert_eq!(
        set.updated()[0]
            .cells()
            .iter()
            .map(CellChange::column)
            .collect::<Vec<_>>(),
        vec![NAME],
        "a legacy base-equal override is filtered with the edit equality"
    );
}

#[test]
fn issue50_snapshot_is_immutable_and_deterministic() {
    let mut g = grid(6);
    for row in [4, 1, 3] {
        for col in [NOTE, QTY, NAME] {
            g.record_cell(row, col, text(&format!("r{row}c{col}")));
        }
    }
    g.toggle_delete(5);
    g.insert_row().expect("the row inserts");
    g.cursor = (0, 0);
    let first = g.snapshot_changes();
    assert_valid(&g, &first);
    let rows: Vec<usize> = first
        .updated()
        .iter()
        .map(|r| g.source_index(r.key()).unwrap())
        .collect();
    assert_eq!(rows, vec![1, 3, 4]);
    for _ in 0..8 {
        assert_eq!(g.snapshot_changes(), first, "repeatable");
    }

    let kept = first.clone();
    g.apply_local_sort(NAME, SortDir::Desc);
    g.cursor = (3, QTY);
    assert_eq!(g.snapshot_changes(), first, "sort and cursor are not draft");
    assert_eq!(g.snapshot_changes().revision(), first.revision());

    g.record_cell(0, NAME, text("later"));
    g.toggle_delete(2);
    let later = g.snapshot_changes();
    assert_ne!(later, first);
    assert_ne!(later.revision(), first.revision());
    assert_eq!(first, kept, "the earlier capture does not change");
    assert!(!g.pending().is_empty(), "capture never clears the draft");
}

#[test]
fn issue50_clean_rows_do_not_clone_payloads() {
    let mut g = grid(2_000);
    assert!(g.record_cell(1_234, QTY, CellValue::Num(-4.0)));
    COPIED_CELLS.with(|n| n.set(0));
    let set = g.snapshot_changes();
    let copied = COPIED_CELLS.with(|n| n.get());
    let width = g.columns.len();
    assert_eq!(
        copied,
        width + 2,
        "one changed row's base plus its before and after, not {} cells",
        2_000 * width
    );
    assert_eq!(set.counts(), (1, 0, 0));

    COPIED_CELLS.with(|n| n.set(0));
    g.discard();
    assert!(g.snapshot_changes().is_empty());
    assert_eq!(
        COPIED_CELLS.with(|n| n.get()),
        0,
        "a clean grid copies nothing"
    );
}

#[test]
fn issue50_clone_gets_a_fresh_origin_with_the_same_draft() {
    let mut g = grid(2);
    g.record_cell(0, NAME, text("edited"));
    let copy = g.clone();
    assert_ne!(copy.instance_id(), g.instance_id());
    assert_eq!(g.instance_id(), g.instance_id());
    assert_eq!(copy.revision(), g.revision());

    let (a, b) = (g.snapshot_changes(), copy.snapshot_changes());
    assert_eq!(a.counts(), b.counts());
    assert_eq!(a.origin(), g.instance_id());
    assert_eq!(b.origin(), copy.instance_id());
    assert!(!a.same_capture(&b), "a capture belongs to one instance");
    assert_ne!(
        DataGrid::new(WidgetId::of("other"), columns()).instance_id(),
        g.instance_id()
    );
}

#[test]
fn issue50_same_capture_is_bit_exact_where_eq_is_not() {
    let mut g = grid(1);
    g.record_cell(0, QTY, CellValue::Num(f64::NAN));
    let nan = g.snapshot_changes();
    assert_ne!(nan, nan.clone(), "derived equality keeps NaN != NaN");
    assert!(nan.same_capture(&nan.clone()));

    let mut other_payload = nan.clone();
    other_payload.updated[0].cells[0].after =
        CellValue::Num(f64::from_bits(f64::NAN.to_bits() ^ 1));
    assert!(!nan.same_capture(&other_payload), "NaN payloads differ");

    let mut g = grid(1);
    g.record_cell(0, QTY, CellValue::Num(0.25));
    let plain = g.snapshot_changes();
    let mut zero = plain.clone();
    zero.updated[0].cells[0].before = CellValue::Num(0.0);
    let mut negative = zero.clone();
    negative.updated[0].cells[0].before = CellValue::Num(-0.0);
    assert_eq!(zero, negative, "0.0 == -0.0 under derived equality");
    assert!(!zero.same_capture(&negative), "the sign of zero counts");
}

#[test]
fn issue50_receipt_records_the_snapshot_it_answers() {
    let mut g = grid(1);
    g.insert_row().expect("the row inserts");
    let set = g.snapshot_changes();
    let key = set.inserted()[0].key();
    let receipt = GridCommitReceipt::new(
        &set,
        vec![CommittedRow {
            key,
            committed_key: ItemKey::num(9_000),
            values: set.inserted()[0].values().to_vec(),
        }],
    );
    assert_eq!(receipt.origin(), g.instance_id());
    assert_eq!(receipt.revision(), set.revision());
    assert_eq!(receipt.rows[0].key, key);
    assert_ne!(receipt.rows[0].committed_key, key);

    let other = grid(1).snapshot_changes();
    assert_ne!(
        GridCommitReceipt::new(&other, vec![]).origin(),
        receipt.origin()
    );
}
