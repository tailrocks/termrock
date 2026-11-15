//! A save belongs to the tab, connection and exact changes it was reviewed
//! with. Switching or closing tabs, editing after review or reconnecting cannot
//! redirect or expand it.

use super::test_support::review_code;
use super::*;
use junie_tui::core::event::Key;
use junie_tui::widgets::grid::{CellEditResult, FinishEdit, GridEditError, cell_validator};

const PRODUCT_NAME: usize = 2;
const CUSTOMER_NAME: usize = 3;

fn app_with_two_tables() -> (App, usize, usize) {
    let mut app = App::new(Theme::junie());
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    let w = app.workbench.as_mut().expect("connected");
    let products = w.open_table("public", "products", false).expect("opens");
    let customers = w.open_table("public", "customers", false).expect("opens");
    (app, products, customers)
}

fn grid_at(app: &mut App, index: usize) -> &mut DataGrid {
    match app.workbench.as_mut().and_then(|w| w.tabs.get_mut(index)) {
        Some(WorkTab::Table(t)) => &mut t.grid,
        _ => panic!("tab {index} is a table"),
    }
}

fn edit(app: &mut App, index: usize, col: usize, to: &str) {
    let _ = grid_at(app, index).record_cell(0, col, CellValue::Text(to.into()));
}

fn activate(app: &mut App, index: usize) {
    app.workbench.as_mut().expect("connected").set_active(index);
}

fn confirm(app: &mut App) {
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        app.advance();
    }
}

/// Row-edit statements saved since the app started, oldest first. The seeded
/// history holds older entries of the same source, which these never include.
fn saved_sql(app: &App) -> Vec<String> {
    let mut saved: Vec<String> = app
        .history
        .entries
        .iter()
        .take_while(|e| e.minutes_ago == 0)
        .filter(|e| matches!(e.source, crate::model::HistorySource::RowEdits))
        .map(|e| e.sql.clone())
        .collect();
    saved.reverse();
    saved
}

fn status(app: &App) -> String {
    app.shell
        .status
        .get()
        .map(|(text, _)| text.to_owned())
        .unwrap_or_default()
}

#[test]
fn issue11_save_completion_targets_submitted_tab() {
    let (mut app, products, customers) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "A edit");
    edit(&mut app, customers, CUSTOMER_NAME, "B edit");
    activate(&mut app, products);
    app.begin_commit();
    let reviewed = review_code(&app);
    assert_eq!(reviewed.len(), 1, "{reviewed:?}");
    assert!(reviewed[0].contains("products"), "{}", reviewed[0]);
    confirm(&mut app);

    activate(&mut app, customers);
    ticks(&mut app, 5);

    assert!(grid_at(&mut app, products).pending().is_empty(), "A saved");
    assert_eq!(
        grid_at(&mut app, customers).pending().total(),
        1,
        "the active tab's draft was not saved or discarded"
    );
    assert_eq!(
        saved_sql(&app),
        reviewed,
        "history is exactly what was reviewed"
    );
    let status = status(&app);
    assert!(status.contains("products"), "{status}");
}

#[test]
fn issue11_edit_after_review_cannot_expand_the_save() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "reviewed");
    activate(&mut app, products);
    app.begin_commit();
    let reviewed = review_code(&app);
    confirm(&mut app);

    edit(&mut app, products, 1, "sneaked-in");
    ticks(&mut app, 5);

    assert_eq!(
        saved_sql(&app),
        reviewed,
        "an edit after review is not part of the save"
    );
    assert_eq!(reviewed.len(), 1);
    assert!(
        grid_at(&mut app, products).pending().is_empty(),
        "nothing outside the review was left behind"
    );
}

#[test]
fn issue11_close_target_while_submitted_is_refused() {
    let (mut app, products, customers) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "A edit");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);
    let open = app.workbench.as_ref().expect("connected").tabs.len();

    app.dialog_closed(CLOSE_DIALOG.child(products), DialogResult::Action(1));
    assert_eq!(
        app.workbench.as_ref().expect("connected").tabs.len(),
        open,
        "the saving tab stays open"
    );

    activate(&mut app, customers);
    ticks(&mut app, 5);
    assert!(grid_at(&mut app, products).pending().is_empty());
    assert_eq!(saved_sql(&app).len(), 1);
}

#[test]
fn issue11_stale_connection_and_request_ignored() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "old connection");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);

    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    let w = app.workbench.as_mut().expect("reconnected");
    let fresh = w.open_table("public", "products", false).expect("opens");
    edit(&mut app, fresh, PRODUCT_NAME, "new connection");
    ticks(&mut app, 8);

    assert_eq!(
        grid_at(&mut app, fresh).pending().total(),
        1,
        "the old save never touched the new connection's draft"
    );
    assert!(saved_sql(&app).is_empty(), "no old success was recorded");
    assert!(!status(&app).starts_with("Saved"), "{}", status(&app));
}

#[test]
fn issue11_review_cancel_unlocks_and_keeps_the_draft() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "kept");
    activate(&mut app, products);
    app.begin_commit();
    assert!(
        grid_at(&mut app, products).submission_pending(),
        "review locks"
    );
    assert_eq!(
        grid_at(&mut app, products).record_cell(0, 1, CellValue::Text("x".into())),
        Err(GridEditError::Busy)
    );

    app.begin_commit();
    assert!(
        status(&app).contains("already in progress"),
        "{}",
        status(&app)
    );

    app.dialog_closed(COMMIT_DIALOG, DialogResult::Cancelled);
    assert!(app.commit.is_none());
    let g = grid_at(&mut app, products);
    assert!(!g.submission_pending());
    assert_eq!(g.pending().total(), 1, "the draft was not lost");
    assert_eq!(
        g.record_cell(0, 1, CellValue::Text("again".into())),
        Ok(CellEditResult::Changed)
    );
    ticks(&mut app, 8);
    assert!(saved_sql(&app).is_empty(), "a cancelled review never saves");
}

#[test]
fn issue11_unrelated_tab_stays_editable_while_saving() {
    let (mut app, products, customers) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "saving");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);

    activate(&mut app, customers);
    assert_eq!(
        grid_at(&mut app, customers).record_cell(0, CUSTOMER_NAME, CellValue::Text("free".into())),
        Ok(CellEditResult::Changed)
    );
    ticks(&mut app, 5);
    assert_eq!(grid_at(&mut app, customers).pending().total(), 1);
}

#[test]
fn issue11_close_reopen_never_reuses_target() {
    let (mut app, products, customers) = app_with_two_tables();
    let w = app.workbench.as_mut().expect("connected");
    let key_products = w.tabs[products].key();
    let key_customers = w.tabs[customers].key();
    assert_ne!(key_products, key_customers);
    let ids: Vec<WidgetId> = w
        .tabs
        .iter()
        .filter_map(|t| match t {
            WorkTab::Table(t) => Some(t.id),
            _ => None,
        })
        .collect();

    assert!(w.close_tab(products));
    assert!(
        w.tab_mut(key_products).is_none(),
        "the closed key resolves nothing"
    );
    let reopened = w.open_table("public", "products", false).expect("opens");
    let key_reopened = w.tabs[reopened].key();
    assert_ne!(
        key_reopened, key_products,
        "a reopened table is a new document"
    );
    assert_ne!(key_reopened, key_customers);
    let WorkTab::Table(t) = &w.tabs[reopened] else {
        panic!("a table tab");
    };
    assert!(
        !ids.contains(&t.id),
        "its render id collides with no earlier tab"
    );
    assert_eq!(w.active_key(), Some(key_reopened));
    assert!(
        w.tab_mut(key_customers).is_some(),
        "other tabs keep their keys"
    );
}

#[test]
fn issue11_stale_completion_after_close_reopen_has_no_effect() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "reviewed");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);
    let Some(CommitState::Submitted { request, token, .. }) = app.commit.clone() else {
        panic!("the save is submitted");
    };

    app.commit = None;
    grid_at(&mut app, products)
        .cancel_submission(token)
        .expect("the lock is released");
    let w = app.workbench.as_mut().expect("connected");
    assert!(w.close_tab(products));
    let reopened = w.open_table("public", "products", false).expect("opens");
    edit(&mut app, reopened, PRODUCT_NAME, "new document");
    let history = app.history.entries.len();
    let note = status(&app);

    app.complete_commit(request, token);

    assert_eq!(grid_at(&mut app, reopened).pending().total(), 1);
    assert_eq!(
        app.history.entries.len(),
        history,
        "no history for a stale save"
    );
    assert_eq!(status(&app), note);
}

#[test]
fn issue11_duplicate_completion_has_no_second_effect() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "once");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);
    let Some(CommitState::Submitted { request, token, .. }) = app.commit.clone() else {
        panic!("the save is submitted");
    };
    ticks(&mut app, 5);
    let (history, note, revision) = (
        app.history.entries.len(),
        status(&app),
        grid_at(&mut app, products).revision(),
    );
    assert!(note.starts_with("Saved"), "{note}");

    app.complete_commit(request, token);

    assert_eq!(app.history.entries.len(), history);
    assert_eq!(status(&app), note);
    assert_eq!(grid_at(&mut app, products).revision(), revision);
}

#[test]
fn issue11_invalid_receipt_keeps_the_lock_until_cancelled() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "kept");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);
    let Some(CommitState::Submitted { request, token, .. }) = app.commit.take() else {
        panic!("the save is submitted");
    };
    let mut broken = request.changes.identity_receipt();
    broken.rows.clear();

    let outcome = grid_at(&mut app, products).complete_submission(token, Ok(broken));
    assert!(outcome.is_err());
    app.commit_failed_receipt(request, token, "missing rows".into());

    assert!(
        grid_at(&mut app, products).submission_pending(),
        "still locked"
    );
    assert_eq!(grid_at(&mut app, products).pending().total(), 1);
    ticks(&mut app, 8);
    assert!(saved_sql(&app).is_empty(), "no ticking and no save");
    assert!(
        app.layers.top_as::<Dialog>().is_some(),
        "the failure is shown"
    );

    app.dialog_closed(COMMIT_FAILED_DIALOG, DialogResult::Cancelled);
    assert!(app.commit.is_none());
    assert!(!grid_at(&mut app, products).submission_pending());
    assert_eq!(
        grid_at(&mut app, products).pending().total(),
        1,
        "draft retained"
    );
}

#[test]
fn issue11_save_finishes_live_editor_once() {
    let (mut app, products, _) = app_with_two_tables();
    activate(&mut app, products);
    let parsed = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = parsed.clone();
    let g = grid_at(&mut app, products);
    g.validator = cell_validator(move |spec, text| {
        seen.set(seen.get() + 1);
        junie_tui::widgets::grid::default_validator(spec, text)
    });
    g.cursor = (0, PRODUCT_NAME);
    let _ = g.begin_edit();
    for c in "-live".chars() {
        let _ = g.on_key(&Key {
            code: ratatui::crossterm::event::KeyCode::Char(c),
            mods: ratatui::crossterm::event::KeyModifiers::NONE,
        });
    }
    assert!(g.is_editing());
    assert_eq!(parsed.get(), 0);

    app.begin_commit();

    assert_eq!(
        parsed.get(),
        1,
        "the session reached the parser exactly once"
    );
    let sql = review_code(&app);
    assert!(sql[0].contains("-live"), "{}", sql[0]);
    assert!(!grid_at(&mut app, products).is_editing());
    assert!(grid_at(&mut app, products).submission_pending());
    assert_eq!(
        grid_at(&mut app, products).finish_edit_session(),
        Ok(FinishEdit::NoSession)
    );
}

#[test]
fn issue11_rejected_live_editor_stops_the_review() {
    let (mut app, products, _) = app_with_two_tables();
    activate(&mut app, products);
    let g = grid_at(&mut app, products);
    g.validator = cell_validator(|_, _| Err("no".to_owned()));
    g.cursor = (0, PRODUCT_NAME);
    let _ = g.begin_edit();
    let _ = g.on_key(&Key {
        code: ratatui::crossterm::event::KeyCode::Char('x'),
        mods: ratatui::crossterm::event::KeyModifiers::NONE,
    });

    app.begin_commit();

    assert!(app.layers.top_as::<Dialog>().is_none(), "no review opened");
    assert!(app.commit.is_none());
    assert!(
        grid_at(&mut app, products).is_editing(),
        "the input is kept"
    );
    assert!(!grid_at(&mut app, products).submission_pending());
    assert!(
        status(&app).contains("Fix the open cell first"),
        "{}",
        status(&app)
    );
}

fn press(app: &mut App, code: ratatui::crossterm::event::KeyCode) {
    app.handle_input(Input::Key(Key {
        code,
        mods: ratatui::crossterm::event::KeyModifiers::NONE,
    }));
}

/// Picks the schema through Open Quickly, the way a user switches schemas.
fn switch_schema(app: &mut App, schema: &str) {
    use ratatui::crossterm::event::KeyCode;
    app.open_switcher();
    for c in schema.chars() {
        press(app, KeyCode::Char(c));
    }
    let row = app
        .switcher_targets
        .iter()
        .position(|t| matches!(t, crate::model::SwitchTarget::Schema(s) if s == schema))
        .expect("the schema is offered");
    for _ in 0..row {
        press(app, KeyCode::Down);
    }
    press(app, KeyCode::Enter);
    assert_eq!(
        app.workbench.as_ref().expect("connected").schema,
        schema,
        "the switch applied"
    );
}

#[test]
fn issue11_schema_switch_never_reuses_tab_key() {
    let (mut app, products, customers) = app_with_two_tables();
    let w = app.workbench.as_mut().expect("connected");
    let query = w.tabs.len();
    w.new_query("select 1");
    assert_eq!(w.tabs.len(), query + 1);

    switch_schema(&mut app, "analytics");

    let w = app.workbench.as_mut().expect("connected");
    let opened = w.open_table("analytics", "events", false).expect("opens");
    let keys: Vec<ItemKey> = w.tabs.iter().map(WorkTab::key).collect();
    for (i, k) in keys.iter().enumerate() {
        assert!(
            !keys[..i].contains(k),
            "tab keys stay unique across a schema switch: {keys:?}"
        );
    }
    assert!(products < opened && customers < opened);

    edit(&mut app, opened, 1, "after-switch");
    activate(&mut app, opened);
    app.begin_commit();
    assert!(
        app.layers.top_as::<Dialog>().is_some(),
        "the review opened: {}",
        status(&app)
    );
    confirm(&mut app);
    ticks(&mut app, 8);

    assert!(
        !grid_at(&mut app, opened).submission_pending(),
        "the save released the grid: {}",
        status(&app)
    );
    assert!(grid_at(&mut app, opened).pending().is_empty());
    assert!(status(&app).starts_with("Saved"), "{}", status(&app));
}

#[test]
fn issue11_completion_for_a_mismatched_tab_releases_the_lock() {
    let (mut app, products, _) = app_with_two_tables();
    edit(&mut app, products, PRODUCT_NAME, "kept");
    activate(&mut app, products);
    app.begin_commit();
    confirm(&mut app);
    let Some(CommitState::Submitted {
        mut request, token, ..
    }) = app.commit.take()
    else {
        panic!("the save is submitted");
    };
    request.table = "customers".into();

    app.complete_commit(request, token);

    assert!(
        !grid_at(&mut app, products).submission_pending(),
        "the lock is not stranded"
    );
    assert_eq!(
        grid_at(&mut app, products).pending().total(),
        1,
        "the draft is kept"
    );
    assert!(status(&app).contains("kept pending"), "{}", status(&app));
    assert!(saved_sql(&app).is_empty());
}
