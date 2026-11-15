//! A saved change is part of the connection session. Reloading, sorting,
//! filtering, closing and reopening a table, and another query, all read it.

use super::*;
use junie_tui::widgets::table::SortDir;

use crate::tabs::{TableTab, TableViewState};

const ID: usize = 0;
const SKU: usize = 1;
const NAME: usize = 2;
const UNIT_PRICE: usize = 4;
const CURRENCY: usize = 5;
const IS_ACTIVE: usize = 6;
const CREATED_AT: usize = 8;

fn connected() -> App {
    let mut app = App::new(Theme::junie());
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    app
}

fn open(app: &mut App, table: &str) -> ItemKey {
    let w = app.workbench.as_mut().expect("connected");
    w.open_table("public", table, false).expect("opens")
}

fn tab(app: &mut App, key: ItemKey) -> &TableTab {
    match app.workbench.as_ref().and_then(|w| w.table(key)) {
        Some(t) => t,
        None => panic!("{key:?} is a table tab"),
    }
}

fn edit(app: &mut App, key: ItemKey, row: usize, col: usize, value: CellValue) {
    with_tab(app, key, |t| {
        t.grid.record_cell(row, col, value).expect("edit");
    });
}

/// Runs `f` on a table tab through the workspace's mutation gateway.
fn with_tab<R>(app: &mut App, key: ItemKey, f: impl FnOnce(&mut TableTab) -> R) -> R {
    app.workbench
        .as_mut()
        .and_then(|w| w.modify_table(key, |t, _| f(t)))
        .unwrap_or_else(|| panic!("{key:?} is a table tab"))
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        app.advance();
    }
}

fn status(app: &App) -> String {
    app.shell
        .status
        .get()
        .map(|(text, _)| text.to_owned())
        .unwrap_or_default()
}

fn save(app: &mut App, key: ItemKey) {
    let w = app.workbench.as_mut().expect("connected");
    assert!(w.activate(key), "open");
    app.begin_commit();
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    ticks(app, 6);
    assert!(status(app).starts_with("Saved"), "{}", status(app));
}

fn reload(app: &mut App, key: ItemKey) {
    let change = TableChange::refresh(tab(app, key), "Refreshed");
    app.request_table_change(change, Review::Guard);
    assert_eq!(status(app), "Refreshed");
}

fn show(app: &mut App, key: ItemKey, view: TableViewState) {
    let t = tab(app, key);
    let change = TableChange::filters(t, view, None);
    app.request_table_change(change, Review::Guard);
}

fn row_with_id(grid: &DataGrid, id: &str) -> Option<Vec<CellValue>> {
    grid.rows()
        .iter()
        .find(|r| r[ID] == CellValue::Text(id.into()))
        .cloned()
}

fn id_of(grid: &DataGrid, src: usize) -> String {
    match &grid.rows()[src][ID] {
        CellValue::Text(id) => id.clone(),
        other => panic!("the id is text, not {other:?}"),
    }
}

/// The rows a query shows, found by running it through the editor tab.
fn select(app: &mut App, sql: &str) -> Vec<Vec<CellValue>> {
    let w = app.workbench.as_mut().expect("connected");
    let key = w.new_query(sql);
    w.modify(key, |tab| {
        let WorkTab::Query(q) = tab else {
            panic!("a query tab");
        };
        q.start(vec![(sql.to_owned(), 0..sql.len())], false, None);
    })
    .expect("the query tab is editable");
    ticks(app, 14);
    let w = app.workbench.as_ref().expect("connected");
    let Some(WorkTab::Query(q)) = w.tab(key) else {
        panic!("a query tab");
    };
    q.active_grid()
        .unwrap_or_else(|| panic!("the query returned rows"))
        .rows()
        .to_vec()
}

fn fill_new_row(grid: &mut DataGrid, src: usize, sku: &str, name: &str) {
    for (col, value) in [
        (SKU, CellValue::Text(sku.into())),
        (NAME, CellValue::Text(name.into())),
        (UNIT_PRICE, CellValue::Num(12.5)),
        (CURRENCY, CellValue::Default),
        (IS_ACTIVE, CellValue::Default),
        (CREATED_AT, CellValue::Default),
    ] {
        grid.record_cell(src, col, value).expect("fills the cell");
    }
}

#[test]
fn issue51_update_save_reload_query_persists() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let id = id_of(&tab(&mut app, key).grid, 0);
    edit(
        &mut app,
        key,
        0,
        NAME,
        CellValue::Text("Renamed once".into()),
    );
    save(&mut app, key);
    let saved = |grid: &DataGrid| row_with_id(grid, &id).map(|r| r[NAME].clone());
    assert_eq!(
        saved(&tab(&mut app, key).grid),
        Some(CellValue::Text("Renamed once".into())),
        "the saving grid shows the saved value"
    );

    reload(&mut app, key);
    assert_eq!(
        saved(&tab(&mut app, key).grid),
        Some(CellValue::Text("Renamed once".into())),
        "a refresh reads the saved value"
    );

    let t = tab(&mut app, key);
    let change = TableChange::sort(t, Some((NAME, SortDir::Asc)));
    app.request_table_change(change, Review::Guard);
    assert_eq!(
        saved(&tab(&mut app, key).grid),
        Some(CellValue::Text("Renamed once".into())),
        "a sort reads the saved value"
    );

    let mut view = tab(&mut app, key).view().clone();
    view.filters.push(Filter {
        column: "name".into(),
        op: FilterOp::Contains,
        value: "Renamed once".into(),
        value2: String::new(),
        enabled: true,
    });
    show(&mut app, key, view);
    let filtered = tab(&mut app, key).grid.rows().to_vec();
    assert!(
        filtered
            .iter()
            .any(|r| r[ID] == CellValue::Text(id.clone())),
        "a filter on the saved value finds the row"
    );

    let w = app.workbench.as_mut().expect("connected");
    assert!(
        w.request_close(key).expect("a clean tab closes").is_empty(),
        "a clean tab closes at once"
    );
    let reopened = open(&mut app, "products");
    assert_eq!(
        saved(&tab(&mut app, reopened).grid),
        Some(CellValue::Text("Renamed once".into())),
        "a reopened table reads the saved value"
    );

    let rows = select(
        &mut app,
        "SELECT * FROM public.products WHERE name = 'Renamed once'",
    );
    assert!(
        rows.iter().any(|r| r[ID] == CellValue::Text(id.clone())),
        "another SELECT reads the saved value"
    );
}

#[test]
fn issue51_insert_delete_survive_all_views() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let seed_total = tab(&mut app, key).total;
    let deleted_id = id_of(&tab(&mut app, key).grid, 1);
    with_tab(&mut app, key, |t| {
        let g = &mut t.grid;
        g.toggle_delete(1).expect("marks a delete");
        g.insert_row().expect("inserts");
        let src = g.rows().len() - 1;
        fill_new_row(g, src, "SKU-SAVED", "Brand new");
    });
    save(&mut app, key);

    let inserted_id = {
        let g = &tab(&mut app, key).grid;
        let row = g
            .rows()
            .iter()
            .find(|r| r[SKU] == CellValue::Text("SKU-SAVED".into()))
            .expect("the inserted row is shown");
        assert!(
            matches!(&row[ID], CellValue::Text(id) if !id.is_empty()),
            "the generated key is a value: {:?}",
            row[ID]
        );
        row[ID].clone()
    };
    assert!(
        tab(&mut app, key).grid.row_key(0).is_some(),
        "rows keep their keys"
    );

    reload(&mut app, key);
    let t = tab(&mut app, key);
    assert!(
        row_with_id(&t.grid, &deleted_id).is_none(),
        "a deleted row stays deleted after a refresh"
    );
    assert!(
        t.grid.rows().iter().any(|r| r[ID] == inserted_id),
        "an inserted row stays after a refresh"
    );
    assert_eq!(
        t.total, seed_total,
        "one insert and one delete keep the count"
    );

    let w = app.workbench.as_mut().expect("connected");
    assert!(
        w.request_close(key).expect("a clean tab closes").is_empty(),
        "a clean tab closes at once"
    );
    let reopened = open(&mut app, "products");
    let t = tab(&mut app, reopened);
    assert!(row_with_id(&t.grid, &deleted_id).is_none(), "still deleted");
    assert!(
        t.grid.rows().iter().any(|r| r[ID] == inserted_id),
        "still inserted"
    );

    let rows = select(
        &mut app,
        "SELECT * FROM public.products WHERE sku = 'SKU-SAVED'",
    );
    assert_eq!(rows.len(), 1, "another SELECT finds the inserted row");
    assert_eq!(rows[0][ID], inserted_id);
    let count = select(&mut app, "SELECT COUNT(*) FROM public.products");
    assert_eq!(count[0][0], CellValue::Int(seed_total as i64));
}

fn second_view(app: &mut App) -> ItemKey {
    app.workbench
        .as_mut()
        .expect("connected")
        .open_second_view("public", "products")
        .expect("opens")
}

fn name_at(app: &mut App, key: ItemKey, src: usize) -> CellValue {
    tab(app, key).grid.rows()[src][NAME].clone()
}

fn text_cell(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn strip_suffixes(app: &App) -> Vec<Option<String>> {
    app.workbench
        .as_ref()
        .expect("connected")
        .strip
        .items
        .iter()
        .map(|i| i.suffix.clone())
        .collect()
}

fn confirm_save(app: &mut App, key: ItemKey) {
    let w = app.workbench.as_mut().expect("connected");
    assert!(w.activate(key), "open");
    app.begin_commit();
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    ticks(app, 6);
}

#[test]
fn issue51_dirty_second_view_conflicts() {
    let mut app = connected();
    let a = open(&mut app, "products");
    let b = second_view(&mut app);
    let c = second_view(&mut app);
    edit(&mut app, b, 1, NAME, text_cell("B draft"));
    edit(&mut app, a, 0, NAME, text_cell("A saved"));
    save(&mut app, a);

    assert_eq!(
        name_at(&mut app, c, 0),
        text_cell("A saved"),
        "a clean peer reads it"
    );
    assert!(!tab(&mut app, c).source_changed());
    assert!(
        tab(&mut app, b).source_changed(),
        "the dirty peer is flagged"
    );
    assert_eq!(
        tab(&mut app, b).grid.pending().total(),
        1,
        "its draft stays"
    );
    assert_eq!(tab(&mut app, b).grid.value(1, NAME), &text_cell("B draft"));
    assert_ne!(
        name_at(&mut app, b, 0),
        text_cell("A saved"),
        "its rows are not replaced"
    );
    assert!(
        strip_suffixes(&app)
            .iter()
            .flatten()
            .any(|s| s == "source changed"),
        "the tab strip says so"
    );
    assert!(status(&app).contains("1 other view"), "{}", status(&app));

    let history = app.history.entries.len();
    confirm_save(&mut app, b);
    assert!(
        status(&app).contains("Source changed; save requires reload/review"),
        "{}",
        status(&app)
    );
    assert_eq!(app.history.entries.len(), history, "no success history");
    assert_eq!(
        tab(&mut app, b).grid.pending().total(),
        1,
        "B keeps its draft"
    );
    assert!(
        !tab(&mut app, b).grid.submission_pending(),
        "and is unlocked"
    );
    assert_eq!(
        tab(&mut app, b).grid.commit_failure(),
        Some("Source changed; save requires reload/review")
    );

    reload(&mut app, c);
    assert_eq!(
        name_at(&mut app, c, 0),
        text_cell("A saved"),
        "A's value was not overwritten"
    );
    let rows = select(
        &mut app,
        "SELECT * FROM public.products WHERE name = 'B draft'",
    );
    assert!(rows.is_empty(), "B's draft never reached the store");

    let change = TableChange::refresh(tab(&mut app, b), "Reloaded");
    app.request_table_change(change, Review::Discard);
    app.discard_review_closed(true);
    assert!(!tab(&mut app, b).source_changed(), "a reload resolves it");
    assert_eq!(name_at(&mut app, b, 0), text_cell("A saved"));
    assert_eq!(tab(&mut app, b).grid.pending().total(), 0);
}

#[test]
fn issue51_peer_save_preserves_uncommitted_editor_input() {
    let mut app = connected();
    let a = open(&mut app, "products");
    let b = second_view(&mut app);
    with_tab(&mut app, b, |t| {
        let g = &mut t.grid;
        g.cursor = (1, NAME);
        let _ = g.begin_edit();
        for c in "-typing".chars() {
            let _ = g.on_key(&junie_tui::core::event::Key {
                code: ratatui::crossterm::event::KeyCode::Char(c),
                mods: ratatui::crossterm::event::KeyModifiers::NONE,
            });
        }
    });
    let editor = format!("{:?}", tab(&mut app, b).grid.edit);
    let source = tab(&mut app, b).grid.revision().source;
    assert!(tab(&mut app, b).grid.is_editing());
    assert_eq!(
        tab(&mut app, b).grid.pending().total(),
        0,
        "nothing is staged"
    );

    edit(&mut app, a, 0, NAME, text_cell("A saved"));
    save(&mut app, a);

    let t = tab(&mut app, b);
    assert!(t.grid.is_editing(), "the editor is still open");
    assert_eq!(
        format!("{:?}", t.grid.edit),
        editor,
        "with exactly its text and selection"
    );
    assert!(t.source_changed());
    assert_eq!(
        t.grid.revision().source,
        source,
        "its rows were not replaced"
    );
    assert_eq!(t.grid.pending().total(), 0);
}

#[test]
fn issue51_failed_save_changes_neither_store_nor_draft() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let second = name_at(&mut app, key, 1);
    let first = name_at(&mut app, key, 0);
    with_tab(&mut app, key, |t| {
        let g = &mut t.grid;
        g.record_cell(1, NAME, text_cell("valid but unsaved"))
            .expect("edit");
        g.record_cell(0, SKU, CellValue::Null).expect("edit");
    });
    let history = app.history.entries.len();
    let draft = tab(&mut app, key).grid.snapshot_changes().counts();
    confirm_save(&mut app, key);

    assert!(status(&app).contains("sku is required"), "{}", status(&app));
    assert!(
        !status(&app).contains("DEFAULT"),
        "sku has no default to type: {}",
        status(&app)
    );
    assert_eq!(app.history.entries.len(), history, "no success history");
    let t = tab(&mut app, key);
    assert_eq!(
        t.grid.snapshot_changes().counts(),
        draft,
        "the whole draft is kept"
    );
    assert!(!t.grid.submission_pending(), "unlocked for another try");
    assert_eq!(t.grid.value(1, NAME), &text_cell("valid but unsaved"));
    assert!(
        t.grid.clone().undo().expect("undo is allowed").is_some(),
        "the undo history is kept"
    );
    assert!(!t.source_changed(), "a refused row is not a conflict");

    let rows = select(&mut app, "SELECT * FROM public.products LIMIT 3");
    assert_eq!(
        rows[0][NAME], first,
        "no row of the failed batch reached the store"
    );
    assert_eq!(rows[1][NAME], second);
}

#[test]
fn issue51_refused_save_does_not_advance_the_content_revision() {
    let mut app = connected();
    let key = open(&mut app, "products");
    edit(&mut app, key, 0, SKU, CellValue::Null);
    let revision = |app: &App| {
        app.workbench
            .as_ref()
            .expect("connected")
            .ws
            .revision(key)
            .expect("the tab is open")
    };
    let before = revision(&app);
    confirm_save(&mut app, key);

    assert!(status(&app).contains("sku is required"), "{}", status(&app));
    assert_eq!(
        revision(&app),
        before,
        "a refusal changes no content, so an open close request stays valid"
    );
}

#[test]
fn issue51_null_in_defaulted_column_offers_the_default_hint() {
    let mut app = connected();
    let key = open(&mut app, "products");
    edit(&mut app, key, 0, CURRENCY, CellValue::Null);
    confirm_save(&mut app, key);

    assert!(
        status(&app).contains("currency is required (type DEFAULT to use the column default)"),
        "{}",
        status(&app)
    );
}

#[test]
fn issue51_null_in_column_without_default_says_required() {
    let mut app = connected();
    let key = open(&mut app, "products");
    edit(&mut app, key, 0, SKU, CellValue::Null);
    confirm_save(&mut app, key);

    assert!(status(&app).contains("sku is required"), "{}", status(&app));
    assert!(
        !status(&app).contains("DEFAULT"),
        "sku has no default to type: {}",
        status(&app)
    );
}

#[test]
fn issue51_receipt_validation_happens_before_the_store_changes() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let original = name_at(&mut app, key, 0);
    edit(&mut app, key, 0, NAME, text_cell("never stored"));
    let older = tab(&mut app, key).grid.snapshot_changes();
    edit(&mut app, key, 0, SKU, text_cell("SKU-NEWER"));
    let w = app.workbench.as_mut().expect("connected");
    assert!(w.activate(key), "open");
    app.begin_commit();
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    let Some(CommitState::Submitted { request, .. }) = app.commit.as_mut() else {
        panic!("the save is submitted");
    };
    request.changes = older;
    ticks(&mut app, 6);

    assert!(
        app.layers.top_as::<Dialog>().is_some(),
        "the grid's refusal is shown"
    );
    assert!(
        tab(&mut app, key).grid.submission_pending(),
        "the draft stays locked"
    );
    assert!(!status(&app).starts_with("Saved"), "{}", status(&app));
    app.dialog_closed(COMMIT_FAILED_DIALOG, DialogResult::Cancelled);
    let rows = select(&mut app, "SELECT * FROM public.products LIMIT 1");
    assert_eq!(rows[0][NAME], original, "the store was not written");
    let draft = tab(&mut app, key).grid.snapshot_changes();
    assert_eq!(
        draft.updated()[0].cells().len(),
        2,
        "both edits are still pending"
    );
}

#[test]
fn issue51_query_results_stay_snapshots_and_say_so() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let sql = "SELECT * FROM public.products LIMIT 2";
    let before = select(&mut app, sql);
    edit(&mut app, key, 0, NAME, text_cell("after the query"));
    save(&mut app, key);

    let w = app.workbench.as_ref().expect("connected");
    let q = w
        .tabs()
        .find_map(|t| match t {
            WorkTab::Query(q) if !q.results.is_empty() => Some(q),
            _ => None,
        })
        .expect("the query tab");
    assert!(q.results[0].source_changed, "the result is marked");
    assert_eq!(
        q.active_grid().expect("rows").rows(),
        before.as_slice(),
        "and shows the rows as they were read"
    );
    let rerun = select(&mut app, sql);
    assert_eq!(
        rerun[0][NAME],
        text_cell("after the query"),
        "a new run reads the store"
    );
}

#[test]
fn issue51_new_connection_starts_from_the_seed_again() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let original = name_at(&mut app, key, 0);
    edit(&mut app, key, 0, NAME, text_cell("this session"));
    save(&mut app, key);

    let again = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(again);
    let fresh = open(&mut app, "products");
    assert_eq!(
        name_at(&mut app, fresh, 0),
        original,
        "the earlier session is gone"
    );
}

#[test]
fn issue51_saved_counts_reach_totals_and_explorer() {
    let mut app = connected();
    let key = open(&mut app, "products");
    let seed = tab(&mut app, key).total;
    with_tab(&mut app, key, |t| t.grid.toggle_delete(0).expect("delete"));
    save(&mut app, key);
    assert_eq!(tab(&mut app, key).total, seed - 1);
    assert_eq!(
        tab(&mut app, key).grid.total,
        junie_tui::widgets::grid::RowTotal::Exact(seed - 1)
    );
    let meta = app
        .workbench
        .as_ref()
        .expect("connected")
        .explorer
        .nodes
        .first()
        .and_then(|db| db.children.first())
        .map(|schema| format!("{:?}", schema.children));
    assert!(
        meta.is_some_and(|m| m.contains(&format!("{:?}", crate::sql::fmt_rows(seed - 1)))),
        "the explorer shows the live count"
    );
}
