use termrock::core::id::{ItemKey, WidgetId};
use termrock::widgets::grid::{
    CellValue, ColumnSpec, DataGrid, GridChangeSet, GridCommitReceipt, GridRows, RowTotal,
};

use super::*;
use crate::tabs::cell_kind;

const SKU: usize = 1;
const NAME: usize = 2;
const UNIT_PRICE: usize = 4;
const CURRENCY: usize = 5;
const IS_ACTIVE: usize = 6;
const CREATED_AT: usize = 8;

fn catalog() -> Catalog {
    Catalog::acme_prod()
}

fn table(cat: &Catalog, name: &str) -> Table {
    cat.find(Some("public"), name)
        .expect("a sample table")
        .clone()
}

fn table_id(cat: &Catalog, table: &Table) -> TableId {
    TableId::new(&cat.database, &table.schema, &table.name)
}

fn columns(table: &Table) -> Vec<(String, ColType)> {
    table
        .columns
        .iter()
        .map(|c| (c.name.clone(), c.ty))
        .collect()
}

fn context() -> CommitContext {
    CommitContext::new("2025-07-04 08:09:10+00")
}

/// A grid showing `keys` of the table the way a table tab shows them.
fn grid_of(store: &SessionStore, id: &TableId, table: &Table, keys: &[ItemKey]) -> DataGrid {
    let specs = table
        .columns
        .iter()
        .map(|c| {
            let mut spec = ColumnSpec::new(&c.name, cell_kind(c.ty)).nullable(c.nullable);
            if c.primary {
                spec = spec.primary();
            }
            if c.generated {
                spec = spec.read_only();
            }
            spec
        })
        .collect();
    let mut grid = DataGrid::new(WidgetId::of("store-test"), specs);
    grid.editable = true;
    let rows = keys
        .iter()
        .map(|&k| {
            store
                .row(id, table, k)
                .expect("the key names a live row")
                .iter()
                .map(to_cell)
                .collect()
        })
        .collect();
    grid.try_set_rows_keyed(
        GridRows {
            rows,
            total: RowTotal::Exact(keys.len()),
            more: false,
        },
        keys.to_vec(),
    )
    .expect("keys are unique");
    grid
}

fn first_rows(store: &SessionStore, id: &TableId, table: &Table, n: usize) -> DataGrid {
    let keys: Vec<ItemKey> = store
        .scan(id, table)
        .rows
        .iter()
        .take(n)
        .map(|(k, _)| *k)
        .collect();
    grid_of(store, id, table, &keys)
}

fn prepare(
    store: &SessionStore,
    table: &Table,
    id: &TableId,
    revision: u64,
    changes: &GridChangeSet,
) -> Result<PreparedCommit, StoreError> {
    store.prepare_commit(table, id, revision, &columns(table), changes, &context())
}

fn new_product(grid: &mut DataGrid, sku: &str, name: &str) -> usize {
    grid.insert_row().expect("inserts");
    let src = grid.rows().len() - 1;
    for (col, value) in [
        (SKU, CellValue::Text(sku.into())),
        (NAME, CellValue::Text(name.into())),
        (UNIT_PRICE, CellValue::Num(4.5)),
        (CURRENCY, CellValue::Default),
        (IS_ACTIVE, CellValue::Default),
        (CREATED_AT, CellValue::Default),
    ] {
        grid.record_cell(src, col, value).expect("fills");
    }
    src
}

fn text(v: &Value) -> &str {
    match v {
        Value::Text(s) => s,
        other => panic!("text expected, got {other:?}"),
    }
}

#[test]
fn issue51_prepared_plan_is_bound_to_source() {
    let cat = catalog();
    let products = table(&cat, "products");
    let customers = table(&cat, "customers");
    let (pid, cid) = (table_id(&cat, &products), table_id(&cat, &customers));
    let mut a = SessionStore::new(1);
    let mut b = SessionStore::new(2);

    let mut grid = first_rows(&a, &pid, &products, 2);
    grid.record_cell(0, NAME, CellValue::Text("Edited".into()))
        .expect("edit");
    let changes = grid.snapshot_changes();
    let plan = prepare(&a, &products, &pid, 0, &changes).expect("prepares");
    let other_generation = prepare(&a, &products, &pid, 0, &changes).expect("prepares again");

    let before = b.scan(&pid, &products).rows;
    assert_eq!(b.apply_prepared(plan.clone()), Err(StoreError::WrongStore));
    assert_eq!(b.scan(&pid, &products).rows, before, "B is untouched");
    assert_eq!(b.revision(&pid), 0);
    assert_eq!(b.session_generation(), 2);

    a.apply_prepared(plan).expect("the store it was made for");
    assert_eq!(a.revision(&pid), 1);
    assert_eq!(
        a.apply_prepared(other_generation),
        Err(StoreError::StaleRevision {
            expected: 0,
            actual: 1
        }),
        "a plan made against an older table revision is refused"
    );
    assert_eq!(a.revision(&pid), 1, "the refusal changed nothing");
    assert_eq!(
        a.revision(&cid),
        0,
        "another table at the same revision is untouched"
    );
    assert_eq!(
        a.scan(&cid, &customers).rows,
        SessionStore::new(1).scan(&cid, &customers).rows
    );
}

#[test]
fn issue51_equal_generation_other_store_rejects_plan() {
    let cat = catalog();
    let products = table(&cat, "products");
    let id = table_id(&cat, &products);
    let a = SessionStore::new(1);
    let mut b = SessionStore::new(1);

    let mut grid = first_rows(&a, &id, &products, 1);
    grid.record_cell(0, NAME, CellValue::Text("Only in A".into()))
        .expect("edit");
    let plan = prepare(&a, &products, &id, 0, &grid.snapshot_changes()).expect("prepares");

    let rows = b.scan(&id, &products).rows;
    assert_eq!(b.revision(&id), a.revision(&id));
    assert_eq!(b.apply_prepared(plan), Err(StoreError::WrongStore));
    assert_eq!(b.scan(&id, &products).rows, rows, "B's rows are unchanged");
    assert_eq!(b.revision(&id), 0, "B's revision is unchanged");
    assert_eq!(b.overlay_len(&id), 0);
}

#[test]
fn issue51_batch_failure_is_atomic() {
    let cat = catalog();
    let products = table(&cat, "products");
    let id = table_id(&cat, &products);
    let mut store = SessionStore::new(1);

    let mut grid = first_rows(&store, &id, &products, 3);
    grid.record_cell(0, NAME, CellValue::Text("Fine".into()))
        .expect("edit");
    new_product(&mut grid, "SKU-OK", "Valid insert");
    grid.record_cell(1, SKU, CellValue::Null).expect("edit");
    let bad = grid.snapshot_changes();
    let rows = store.scan(&id, &products).rows;

    let error = prepare(&store, &products, &id, 0, &bad).expect_err("a later row is invalid");
    assert!(
        matches!(error, StoreError::Required { ref column, .. } if column == "sku"),
        "{error:?}"
    );
    assert!(error.row().is_some(), "the refusal names the row");
    assert_eq!(store.scan(&id, &products).rows, rows);
    assert_eq!(store.revision(&id), 0);
    assert_eq!(store.overlay_len(&id), 0);

    let stale = prepare(&store, &products, &id, 5, &bad).expect_err("wrong revision");
    assert!(matches!(
        stale,
        StoreError::StaleRevision {
            expected: 5,
            actual: 0
        }
    ));

    let mut ok = first_rows(&store, &id, &products, 1);
    let src = new_product(&mut ok, "SKU-NEXT", "After the failure");
    let plan = prepare(&store, &products, &id, 0, &ok.snapshot_changes()).expect("prepares");
    let receipt = store.apply_prepared(plan).expect("applies");
    let row = &receipt.rows[0];
    assert_eq!(
        row.committed_key,
        ItemKey::num(3),
        "the failed batch consumed no insert counter"
    );
    assert_eq!(
        row.values[0],
        CellValue::Text("00000000-0000-8000-8000-000000000001".into()),
        "nor a generated id"
    );
    assert_eq!(
        ok.rows()[src][0],
        CellValue::Default,
        "the draft still holds its marker"
    );
}

#[test]
fn issue51_defaults_are_typed_and_transactional() {
    let cat = catalog();
    let orders = table(&cat, "orders");
    let id = table_id(&cat, &orders);
    let seed = orders.row_count;
    let mut store = SessionStore::new(1);

    let mut grid = first_rows(&store, &id, &orders, 1);
    let new_order = |grid: &mut DataGrid, total: CellValue| -> usize {
        grid.insert_row().expect("inserts");
        let src = grid.rows().len() - 1;
        let uuid = |n: u8| CellValue::Text(format!("11111111-1111-4111-8111-{n:012}"));
        for (col, value) in [
            (2, uuid(1)),
            (3, uuid(2)),
            (4, CellValue::Default),
            (5, total),
            (6, CellValue::Default),
            (7, CellValue::Null),
            (8, CellValue::Null),
            (11, CellValue::Default),
            (12, CellValue::Default),
        ] {
            grid.record_cell(src, col, value).expect("fills");
        }
        src
    };
    let first = new_order(&mut grid, CellValue::Num(10.0));
    let second = new_order(&mut grid, CellValue::Num(20.0));
    let plan = prepare(&store, &orders, &id, 0, &grid.snapshot_changes()).expect("prepares");
    let receipt = store.apply_prepared(plan).expect("applies");
    assert_eq!(receipt.rows.len(), 2);

    let stored: Vec<Vec<Value>> = receipt
        .rows
        .iter()
        .map(|r| store.row(&id, &orders, r.committed_key).expect("stored"))
        .collect();
    for (row, values) in receipt.rows.iter().zip(&stored) {
        assert_eq!(
            row.values,
            values.iter().map(to_cell).collect::<Vec<_>>(),
            "the receipt describes what the store holds"
        );
    }
    assert_eq!(text(&stored[0][0]), "00000000-0000-8000-8000-000000000001");
    assert_eq!(text(&stored[1][0]), "00000000-0000-8000-8000-000000000002");
    assert_eq!(stored[0][1], Value::Int(10_000 + seed as i64));
    assert_eq!(stored[1][1], Value::Int(10_001 + seed as i64));
    assert_eq!(text(&stored[0][4]), "pending");
    assert_eq!(text(&stored[0][6]), "USD");
    assert_eq!(stored[0][7], Value::Null, "an explicit NULL stays NULL");
    assert_eq!(stored[0][11], Value::Bool(false));
    assert_eq!(
        text(&stored[0][12]),
        "2025-07-04 08:09:10+00",
        "now() is the one captured time"
    );
    assert_eq!(stored[0][12], stored[1][12], "for the whole save");
    assert_eq!(
        stored[0][13],
        Value::Null,
        "an unset nullable column without a default is NULL"
    );
    let _ = (first, second);

    let mut grid = first_rows(&store, &id, &orders, 1);
    new_order(&mut grid, CellValue::Null);
    let error = prepare(&store, &orders, &id, 1, &grid.snapshot_changes()).expect_err("NULL total");
    assert_eq!(
        error.to_string(),
        "total_amount is required",
        "no column default to type"
    );
    assert!(matches!(error, StoreError::Required { ref column, .. } if column == "total_amount"));

    let mut grid = first_rows(&store, &id, &orders, 1);
    let src = new_order(&mut grid, CellValue::Num(1.0));
    grid.record_cell(src, 2, CellValue::Default).expect("edit");
    let error = prepare(&store, &orders, &id, 1, &grid.snapshot_changes())
        .expect_err("DEFAULT of a column without one");
    assert!(matches!(error, StoreError::Required { ref column, .. } if column == "customer_id"));
    assert_eq!(
        error.to_string(),
        "customer_id is required",
        "no column default to type"
    );

    let mut grid = first_rows(&store, &id, &orders, 1);
    grid.record_cell(0, 1, CellValue::Int(7)).expect("edit");
    let error = prepare(&store, &orders, &id, 1, &grid.snapshot_changes())
        .expect_err("a generated column cannot be changed");
    assert!(
        matches!(error, StoreError::GeneratedColumn { ref column, .. } if column == "order_number")
    );

    let mut grid = first_rows(&store, &id, &orders, 1);
    grid.record_cell(0, 4, CellValue::Text("lost".into()))
        .expect("edit");
    let error =
        prepare(&store, &orders, &id, 1, &grid.snapshot_changes()).expect_err("not an enum member");
    assert!(matches!(error, StoreError::NotInEnum { .. }));

    let mut grid = first_rows(&store, &id, &orders, 1);
    grid.record_cell(0, 5, CellValue::Num(f64::NAN))
        .expect("edit");
    let error = prepare(&store, &orders, &id, 1, &grid.snapshot_changes()).expect_err("not finite");
    assert!(matches!(error, StoreError::NonFinite { .. }));

    let mut grid = first_rows(&store, &id, &orders, 1);
    grid.record_cell(0, 5, CellValue::Text("many".into()))
        .expect("edit");
    let error = prepare(&store, &orders, &id, 1, &grid.snapshot_changes())
        .expect_err("a number column holds numbers");
    assert!(matches!(error, StoreError::WrongKind { .. }));

    let mut custom = orders.clone();
    custom.columns[11].default = Some("random()".into());
    let mut grid = first_rows(&store, &id, &custom, 1);
    new_order(&mut grid, CellValue::Num(5.0));
    let error = store
        .prepare_commit(
            &custom,
            &id,
            1,
            &columns(&custom),
            &grid.snapshot_changes(),
            &context(),
        )
        .expect_err("an expression the simulator does not know");
    assert!(
        matches!(error, StoreError::UnsupportedDefault { ref expression, .. } if expression == "random()")
    );

    let mut grid = first_rows(&store, &id, &orders, 1);
    new_order(&mut grid, CellValue::Num(5.0));
    let changes = grid.snapshot_changes();
    let error = store
        .prepare_commit(
            &orders,
            &id,
            1,
            &columns(&orders),
            &changes,
            &CommitContext::new("yesterday"),
        )
        .expect_err("a timestamp outside the grammar");
    assert!(matches!(error, StoreError::BadTimestamp(_)));
    assert_eq!(store.revision(&id), 1, "no failure advanced the table");
}

#[test]
fn issue51_large_fixture_is_sparse_and_session_scoped() {
    let cat = catalog();
    let mut big = table(&cat, "products");
    big.row_count = 1_000_000;
    let id = table_id(&cat, &big);
    let mut store = SessionStore::new(1);

    let far = ItemKey::num(500_000 * 2);
    let mut grid = first_rows(&store, &id, &big, 3);
    grid.record_cell(0, NAME, CellValue::Text("Changed".into()))
        .expect("edit");
    grid.toggle_delete(1).expect("delete");
    new_product(&mut grid, "SKU-BIG", "Inserted");
    let plan = prepare(&store, &big, &id, 0, &grid.snapshot_changes()).expect("prepares");
    store.apply_prepared(plan).expect("applies");

    let mut grid = grid_of(&store, &id, &big, &[far]);
    grid.record_cell(0, NAME, CellValue::Text("Far away".into()))
        .expect("edit");
    let plan = prepare(&store, &big, &id, 1, &grid.snapshot_changes()).expect("prepares");
    store.apply_prepared(plan).expect("applies");

    assert_eq!(store.overlay_len(&id), 4, "only the changes are held");
    let scan = store.scan(&id, &big);
    assert_eq!(
        scan.rows.len(),
        SAMPLE_CAP - 1 + 1,
        "a bounded baseline sample less the deletion, plus the insert"
    );
    assert_eq!(scan.sampled_baseline, SAMPLE_CAP - 1);
    assert_eq!(scan.live_baseline, 999_999);
    assert_eq!(
        store.row_count(&id, &big),
        1_000_000,
        "one delete, one insert"
    );
    assert_eq!(
        text(&store.row(&id, &big, far).expect("lazy read")[NAME]),
        "Far away"
    );
    assert!(
        scan.rows.iter().all(|(k, _)| *k != far),
        "a row past the sample is read by key, not scanned"
    );

    assert_eq!(
        estimate_total(0, 0, 0, 0),
        0,
        "an empty table has no denominator"
    );
    assert_eq!(estimate_total(1_000_000, 0, 0, 3), 3);
    assert_eq!(estimate_total(1_000_000, 2_000, 200, 2), 100_002);
    let none = Scan {
        rows: vec![],
        sampled_baseline: 0,
        live_baseline: 0,
        revision: 0,
    };
    assert_eq!(none.estimate_total(&[]), 0);

    let fresh = SessionStore::new(2);
    assert_eq!(
        fresh.revision(&id),
        0,
        "a new connection generation starts over"
    );
    assert_eq!(fresh.row_count(&id, &big), 1_000_000);
    assert_eq!(fresh.overlay_len(&id), 0);
    assert_ne!(
        text(&fresh.row(&id, &big, ItemKey::num(0)).expect("seed")[NAME]),
        "Changed"
    );
}

#[test]
fn issue51_clock_and_timestamp_grammar() {
    assert!(CommitClock::new(SIMULATOR_TIMESTAMP).is_ok());
    assert!(CommitClock::new("2025-06-01 12:30:00-05:30").is_ok());
    assert!(CommitClock::new("2025-06-01 12:30:00Z").is_ok());
    for bad in [
        "",
        "2025-06-01",
        "2025-13-01 12:30:00+00",
        "2025-06-01 24:30:00+00",
        "2025-06-01 12:30:00",
        "2025-06-01 12:30:00+0",
    ] {
        assert!(CommitClock::new(bad).is_err(), "{bad:?}");
    }
    assert_eq!(
        SessionStore::new(1).commit_context().timestamp(),
        SIMULATOR_TIMESTAMP
    );
}

#[test]
fn issue51_receipt_matches_the_grid_it_answers() {
    let cat = catalog();
    let products = table(&cat, "products");
    let id = table_id(&cat, &products);
    let mut store = SessionStore::new(1);
    let mut grid = first_rows(&store, &id, &products, 2);
    grid.record_cell(0, NAME, CellValue::Text("Edited".into()))
        .expect("edit");
    grid.toggle_delete(1).expect("delete");
    new_product(&mut grid, "SKU-R", "Receipt");
    let changes = grid.snapshot_changes();
    let token = grid.begin_submission(changes.clone()).expect("locks");
    let plan = prepare(&store, &products, &id, 0, &changes).expect("prepares");
    grid.validate_submission_receipt(token, plan.receipt())
        .expect("the receipt covers the captured rows");
    let receipt: GridCommitReceipt = store.apply_prepared(plan).expect("applies");
    grid.complete_submission(token, Ok(receipt))
        .expect("acknowledges");
    assert!(grid.pending().is_empty());
    let keys: Vec<ItemKey> = (0..grid.rows().len())
        .filter_map(|i| grid.row_key(i))
        .collect();
    for key in keys {
        assert!(
            store.row(&id, &products, key).is_some(),
            "{key:?} is a stored row"
        );
    }
}
