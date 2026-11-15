//! Tab identity, close and task ownership run through the shared
//! `DocumentWorkspace`: a close names its tab by token, a save that closes its
//! tab is one close-save, and a query run is one operation of its tab.

use super::*;
use junie_tui::core::event::Key;
use junie_tui::widgets::grid::cell_validator;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

const PRODUCT_NAME: usize = 2;
const CUSTOMER_NAME: usize = 3;

fn connected() -> App {
    let mut app = App::new(Theme::junie());
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    app
}

fn open(app: &mut App, table: &str, preview: bool) -> ItemKey {
    app.workbench
        .as_mut()
        .expect("connected")
        .open_table("public", table, preview)
        .expect("opens")
}

fn with_grid<R>(
    app: &mut App,
    key: ItemKey,
    f: impl FnOnce(&mut junie_tui::widgets::grid::DataGrid) -> R,
) -> R {
    app.workbench
        .as_mut()
        .and_then(|w| w.modify_table(key, |t, _| f(&mut t.grid)))
        .expect("a table tab that no save holds")
}

fn edit(app: &mut App, key: ItemKey, col: usize, to: &str) {
    let _ = with_grid(app, key, |g| {
        g.record_cell(0, col, CellValue::Text(to.into()))
    });
}

fn w(app: &App) -> &Workbench {
    app.workbench.as_ref().expect("connected")
}

fn status(app: &App) -> String {
    app.shell
        .status
        .get()
        .map(|(text, _)| text.to_owned())
        .unwrap_or_default()
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_input(Input::Key(Key {
        code,
        mods: KeyModifiers::NONE,
    }));
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        app.advance();
    }
}

fn two_dirty_tables() -> (App, ItemKey, ItemKey) {
    let mut app = connected();
    let products = open(&mut app, "products", false);
    let customers = open(&mut app, "customers", false);
    edit(&mut app, products, PRODUCT_NAME, "A edit");
    edit(&mut app, customers, CUSTOMER_NAME, "B edit");
    (app, products, customers)
}

fn close_guard_open(app: &App) -> bool {
    app.close_guard.pending().is_some()
}

#[test]
fn workspace_dirty_close_asks_and_discard_closes_only_that_tab() {
    let (mut app, products, customers) = two_dirty_tables();
    let open = w(&app).tab_count();

    app.request_close_tab(products);

    assert!(close_guard_open(&app), "a dirty table asks first");
    assert_eq!(w(&app).tab_count(), open, "nothing closed yet");
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(1));

    assert!(w(&app).tab(products).is_none(), "the asked tab closed");
    assert!(
        w(&app).tab(customers).is_some(),
        "the other dirty tab stays"
    );
    assert_eq!(with_grid(&mut app, customers, |g| g.pending().total()), 1);
    assert_eq!(w(&app).active_key(), Some(customers));
}

#[test]
fn workspace_cancelled_close_keeps_the_tab_and_its_draft() {
    let (mut app, products, _) = two_dirty_tables();
    app.request_close_tab(products);
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Cancelled);

    assert!(w(&app).tab(products).is_some());
    assert_eq!(with_grid(&mut app, products, |g| g.pending().total()), 1);
    assert!(
        w(&app).ws.pending_close(products).is_none(),
        "the request was cleared"
    );
}

#[test]
fn workspace_query_close_confirmation_names_its_tab_by_token() {
    let mut app = connected();
    let wb = app.workbench.as_mut().expect("connected");
    let first = wb.new_query("select 1");
    let second = wb.new_query("select 2");
    wb.modify(second, |tab| {
        if let WorkTab::Query(q) = tab {
            q.editor.set_text("select 22");
        }
    });
    let _ = first;
    app.request_close_tab(second);
    assert_eq!(app.top_kind(), Some("dialog"));
    let token = app.close_pending.expect("the prompt names a token");
    assert_eq!(token.document, second);

    app.workbench.as_mut().expect("connected").activate(first);
    app.dialog_closed(CLOSE_DIALOG, DialogResult::Action(1));

    assert!(w(&app).tab(second).is_none(), "the token's tab closed");
    assert!(w(&app).tab(first).is_some(), "the active tab is untouched");
}

#[test]
fn workspace_stale_close_after_an_edit_asks_again_instead_of_closing() {
    let (mut app, products, _) = two_dirty_tables();
    app.request_close_tab(products);
    let asked = app.close_guard.pending().is_some();
    assert!(asked);

    edit(&mut app, products, PRODUCT_NAME, "changed while deciding");
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(1));

    assert!(
        w(&app).tab(products).is_some(),
        "an edit made after the request keeps the tab open"
    );
    assert!(
        status(&app).contains("changed while you decided"),
        "{}",
        status(&app)
    );
    assert!(close_guard_open(&app), "the prompt is shown again");
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(1));
    assert!(w(&app).tab(products).is_none(), "the fresh token closes it");
}

#[test]
fn workspace_save_then_close_saves_exactly_and_closes_only_that_tab() {
    let (mut app, products, customers) = two_dirty_tables();
    let history = app.history.entries.len();
    app.request_close_tab(products);
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(2));
    assert!(
        app.layers.top_as::<Dialog>().is_some(),
        "the table's own save review opens: {}",
        status(&app)
    );

    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    assert!(w(&app).ws.save_locked(products), "the save locks its tab");
    ticks(&mut app, 8);

    assert!(w(&app).tab(products).is_none(), "A closed after its save");
    assert!(w(&app).tab(customers).is_some());
    assert_eq!(
        with_grid(&mut app, customers, |g| g.pending().total()),
        1,
        "B's draft was neither saved nor dropped"
    );
    assert_eq!(
        app.history.entries.len(),
        history + 1,
        "one statement saved"
    );
    assert!(status(&app).contains("products"), "{}", status(&app));
    assert!(status(&app).contains("closed"), "{}", status(&app));
    assert!(app.commit.is_none());
}

#[test]
fn workspace_close_save_locks_edits_but_not_navigation() {
    let mut app = connected();
    let products = open(&mut app, "products", false);
    edit(&mut app, products, PRODUCT_NAME, "saving");
    app.request_close_tab(products);
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(2));
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    assert!(w(&app).ws.save_locked(products));
    while !app.layers.is_empty() {
        app.layers.pop(&mut app.shell.focus);
    }
    let grid = w(&app).table(products).expect("open").grid.id;
    app.shell.focus.focus(grid);
    let cursor = w(&app).table(products).expect("open").grid.cursor;

    press(&mut app, KeyCode::Down);
    assert_ne!(
        w(&app).table(products).expect("open").grid.cursor,
        cursor,
        "navigation still moves the cursor"
    );

    let revision = w(&app).ws.revision(products);
    press(&mut app, KeyCode::Delete);
    assert_eq!(status(&app), SAVING_NOTICE, "{}", status(&app));
    assert_eq!(
        w(&app).ws.revision(products),
        revision,
        "a refused mutation never reaches the document"
    );
    ticks(&mut app, 8);
    assert!(w(&app).tab(products).is_none());
}

#[test]
fn workspace_cancelled_save_review_keeps_the_tab_and_releases_the_close() {
    let (mut app, products, _) = two_dirty_tables();
    app.request_close_tab(products);
    app.dialog_closed(CLOSE_GUARD_DIALOG, DialogResult::Action(2));
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Cancelled);

    assert!(w(&app).tab(products).is_some());
    assert!(!w(&app).ws.save_locked(products));
    assert!(app.commit.is_none());
    assert!(app.transition_wait.is_none(), "nothing waits to close");
    assert_eq!(with_grid(&mut app, products, |g| g.pending().total()), 1);
    ticks(&mut app, 8);
    assert!(
        w(&app).tab(products).is_some(),
        "a cancelled save never closes"
    );
}

#[test]
fn workspace_close_resolves_editor_before_capture() {
    let mut app = connected();
    let products = open(&mut app, "products", false);
    let grid = w(&app).table(products).expect("open").grid.id;
    app.shell.focus.focus(grid);
    with_grid(&mut app, products, |g| {
        g.cursor = (0, PRODUCT_NAME);
        let _ = g.begin_edit();
    });
    for c in "-live".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert!(w(&app).table(products).expect("open").grid.is_editing());
    assert_eq!(w(&app).table(products).expect("open").dirty_count(), 0);

    app.request_close_tab(products);

    assert!(
        close_guard_open(&app),
        "the valid edit became part of the draft, so the tab asks: {}",
        status(&app)
    );
    let t = w(&app).table(products).expect("open");
    assert_eq!(t.dirty_count(), 1, "captured before the close revision");
    assert!(!t.grid.is_editing());
    let token = w(&app).ws.pending_close(products).expect("a close token");
    assert_eq!(Some(token.revision), w(&app).ws.revision(products));
}

#[test]
fn workspace_invalid_editor_input_keeps_the_tab_open_with_no_request() {
    let mut app = connected();
    let products = open(&mut app, "products", false);
    let grid = w(&app).table(products).expect("open").grid.id;
    app.shell.focus.focus(grid);
    with_grid(&mut app, products, |g| {
        g.validator = cell_validator(|_, _| Err("no".to_owned()));
        g.cursor = (0, PRODUCT_NAME);
        let _ = g.begin_edit();
    });
    press(&mut app, KeyCode::Char('x'));

    app.request_close_tab(products);

    assert!(w(&app).tab(products).is_some(), "the tab stays open");
    assert!(!close_guard_open(&app), "no close prompt");
    assert!(
        w(&app).ws.pending_close(products).is_none(),
        "no close token"
    );
    assert!(
        status(&app).contains("Fix the open cell first"),
        "{}",
        status(&app)
    );
    assert!(w(&app).table(products).expect("open").grid.is_editing());
}

#[test]
fn workspace_running_query_close_cancels_its_own_operation_only() {
    let mut app = connected();
    let wb = app.workbench.as_mut().expect("connected");
    let a = wb.new_query("select 1 from products");
    let b = wb.new_query("select 1 from customers");
    for key in [a, b] {
        let statements = match wb.tab(key) {
            Some(WorkTab::Query(q)) => q.statements_to_run(true),
            _ => panic!("a query tab"),
        };
        assert!(wb.start_query(key, statements, true, None));
    }
    let run_b = wb.ws.running(b).expect("B runs under an operation");
    assert!(wb.ws.running(a).is_some());

    app.request_close_tab(a);

    let wb = w(&app);
    assert!(
        wb.tab(a).is_none(),
        "a running, unedited query closes at once"
    );
    assert_eq!(wb.ws.running(b), Some(run_b), "B's operation is untouched");
    ticks(&mut app, 12);
    let wb = w(&app);
    assert!(
        wb.ws.running(b).is_none(),
        "B's run completed and was released"
    );
    match wb.tab(b) {
        Some(WorkTab::Query(q)) => assert!(!q.is_running() && !q.results.is_empty()),
        _ => panic!("B is still open"),
    }
}

#[test]
fn workspace_running_query_tick_keeps_close_token_valid() {
    let mut app = connected();
    let wb = app.workbench.as_mut().expect("connected");
    let key = wb.new_query("select 1 from products");
    wb.modify(key, |tab| {
        if let WorkTab::Query(q) = tab {
            q.editor.set_text("select 2 from products");
        }
    });
    let statements = match wb.tab(key) {
        Some(WorkTab::Query(q)) => q.statements_to_run(true),
        _ => panic!("a query tab"),
    };
    assert!(wb.start_query(key, statements, true, None));

    app.request_close_tab(key);
    let token = app.close_pending.expect("an edited running tab asks first");
    ticks(&mut app, 12);
    assert!(
        w(&app).ws.running(key).is_none(),
        "the run finished while the prompt was open"
    );
    assert_eq!(
        w(&app).ws.pending_close(key),
        Some(token),
        "operation progress is not a content change"
    );
    assert_eq!(
        w(&app).ws.revision(key),
        Some(token.revision),
        "polling and finishing a run leave the content revision alone"
    );

    app.dialog_closed(CLOSE_DIALOG, DialogResult::Action(1));

    assert!(w(&app).tab(key).is_none(), "Close anyway closes the tab");
    assert!(
        !status(&app).contains("changed while you decided"),
        "{}",
        status(&app)
    );
}

#[test]
fn workspace_preview_open_keeps_a_preview_with_an_open_editor() {
    let mut app = connected();
    let a = open(&mut app, "products", true);
    let grid = w(&app).table(a).expect("open").grid.id;
    app.shell.focus.focus(grid);
    with_grid(&mut app, a, |g| {
        g.cursor = (0, PRODUCT_NAME);
        let _ = g.begin_edit();
    });
    assert!(w(&app).is_preview(a));

    let b = open(&mut app, "customers", true);

    assert!(
        w(&app).tab(a).is_some(),
        "a preview with an open editor stays"
    );
    assert!(!w(&app).is_preview(a), "it was pinned");
    assert!(w(&app).is_preview(b));
}

#[test]
fn workspace_switcher_opens_a_tab_by_key_after_the_strip_reorders() {
    let mut app = connected();
    let a = open(&mut app, "products", false);
    let b = open(&mut app, "customers", false);
    let wb = app.workbench.as_mut().expect("connected");
    let resp = wb.ws.reorder(b, 0);
    let _ = wb.settle(resp);
    wb.activate(a);

    let open_tabs: Vec<(ItemKey, String)> = w(&app).tabs().map(|t| (t.key(), t.label())).collect();
    assert_eq!(open_tabs[0].0, b, "the strip order changed");
    let index = crate::model::SwitcherIndex::build(
        &w(&app).catalog,
        &w(&app).connection.name,
        &open_tabs,
        &app.history,
    );
    let target = index
        .items
        .iter()
        .find_map(|item| match &item.target {
            crate::model::SwitchTarget::OpenTab(key) if *key == b => Some(item.target.clone()),
            _ => None,
        })
        .expect("the open tab is offered under its key");
    assert_eq!(target, crate::model::SwitchTarget::OpenTab(b));
}

#[test]
fn workspace_strip_events_are_translated_to_keys_from_the_same_projection() {
    let mut app = connected();
    open(&mut app, "products", false);
    open(&mut app, "customers", false);
    let c = open(&mut app, "orders", false);
    let wb = app.workbench.as_mut().expect("connected");
    let resp = wb.ws.reorder(c, 0);
    let _ = wb.settle(resp);
    let order = wb.ws.ordered_keys().to_vec();
    assert_eq!(order[0], c, "the strip order changed");

    for (position, key) in order.iter().enumerate() {
        assert!(wb.activate_index(position), "position {position}");
        assert_eq!(wb.active_key(), Some(*key), "the key at that position");
        assert_eq!(wb.strip.active, position, "the strip follows the key");
    }
    assert!(
        !wb.activate_index(order.len()),
        "a position outside the projection"
    );
}
