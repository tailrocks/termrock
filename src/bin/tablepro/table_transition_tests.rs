//! A table's rows and controls change only through one gate. Unsaved edits and
//! the shown sort, filters and chips survive Cancel, a failed read and a stale
//! answer, and change together, once, when a change is installed.

use super::*;
use junie_tui::core::event::Key;
use junie_tui::widgets::document_workspace::CloseDecision;
use junie_tui::widgets::grid::{CellEditResult, GridChangeSet, GridRevision, PendingChanges};
use junie_tui::widgets::table::SortDir;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::tabs::{TableTab, TableViewState};

const NAME: usize = 2;

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
    let w = app.workbench.as_mut().expect("connected");
    w.open_table("public", table, preview).expect("opens")
}

fn tab(app: &App, key: ItemKey) -> &TableTab {
    app.workbench
        .as_ref()
        .and_then(|w| w.table(key))
        .unwrap_or_else(|| panic!("{key:?} is a table tab"))
}

/// Mutates a table the way the app does, through the workspace's gateway.
fn with_tab<R>(app: &mut App, key: ItemKey, f: impl FnOnce(&mut TableTab) -> R) -> R {
    app.workbench
        .as_mut()
        .and_then(|w| w.modify_table(key, |t, _| f(t)))
        .unwrap_or_else(|| panic!("{key:?} is a table tab no save holds"))
}

/// Closes `key` the way a confirmed Discard does.
fn force_close(app: &mut App, key: ItemKey) {
    let w = app.workbench.as_mut().expect("connected");
    for token in w.request_close(key).expect("the tab may close") {
        let resp =
            w.ws.resolve_close(token, CloseDecision::Discard)
                .expect("a fresh token");
        let _ = w.settle(resp);
    }
    assert!(w.tab(key).is_none(), "the tab closed");
}

fn is_preview(app: &App, key: ItemKey) -> bool {
    app.workbench.as_ref().expect("connected").is_preview(key)
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_input(Input::Key(Key {
        code,
        mods: KeyModifiers::NONE,
    }));
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn focus_grid(app: &mut App, key: ItemKey) {
    let id = tab(app, key).grid.id;
    app.shell.focus.focus(id);
}

fn focus_chips(app: &mut App, key: ItemKey) {
    let id = tab(app, key).chips.id;
    app.shell.focus.focus(id);
}

fn status(app: &App) -> String {
    app.shell
        .status
        .get()
        .map(|(text, _)| text.to_owned())
        .unwrap_or_default()
}

fn filter(column: &str, value: &str, enabled: bool) -> Filter {
    Filter {
        column: column.into(),
        op: FilterOp::Contains,
        value: value.into(),
        value2: String::new(),
        enabled,
    }
}

/// Shows `view` on a clean table the way a finished transition does.
fn show(app: &mut App, key: ItemKey, view: TableViewState) {
    let w = app.workbench.as_mut().expect("connected");
    w.modify_table(key, |t, cat| {
        let loaded = t.prepare_load(cat, &view).expect("the view reads");
        t.install_load(loaded, view).expect("installs");
    })
    .expect("a table");
}

#[derive(Debug, Clone, Copy)]
enum Draft {
    Update,
    Insert,
    Delete,
}

fn stage(app: &mut App, key: ItemKey, draft: Draft) {
    with_tab(app, key, |t| {
        let g = &mut t.grid;
        match draft {
            Draft::Update => {
                g.record_cell(0, NAME, CellValue::Text("staged".into()))
                    .expect("edit");
            }
            Draft::Insert => {
                g.insert_row().expect("insert");
            }
            Draft::Delete => {
                g.toggle_delete(0).expect("delete");
            }
        }
    });
}

#[derive(Debug, PartialEq)]
struct Checkpoint {
    view: TableViewState,
    view_revision: u64,
    sort_marker: Option<(usize, SortDir)>,
    chips: Vec<String>,
    pending: PendingChanges,
    changes: GridChangeSet,
    revision: GridRevision,
    rows: usize,
    selection: usize,
}

fn checkpoint(app: &mut App, key: ItemKey) -> Checkpoint {
    let t = tab(app, key);
    Checkpoint {
        view: t.view().clone(),
        view_revision: t.view_revision(),
        sort_marker: t.grid.sort,
        chips: t.chips.chips.iter().map(|c| format!("{c:?}")).collect(),
        pending: t.grid.pending().clone(),
        changes: t.grid.snapshot_changes(),
        revision: t.grid.revision(),
        rows: t.grid.rows().len(),
        selection: t.grid.selection.len(),
    }
}

fn guard_open(app: &mut App) -> bool {
    app.table_guard.pending().is_some()
}

/// Answers the guard's dialog with the keys a user presses: Save has focus, and
/// Discard is one button to its left.
fn answer_guard(app: &mut App, action: usize) {
    assert_eq!(app.top_kind(), Some("dialog"), "the guard's dialog is up");
    match action {
        1 => {
            press(app, KeyCode::Left);
            press(app, KeyCode::Enter);
        }
        2 => press(app, KeyCode::Enter),
        _ => press(app, KeyCode::Esc),
    }
}

fn activate(app: &mut App, key: ItemKey) {
    assert!(app.workbench.as_mut().expect("connected").activate(key));
}

/// Every gesture that asks for a new view, driven the way a user drives it.
#[derive(Debug, Clone, Copy)]
enum Gesture {
    SortKey,
    RefreshKey,
    ChipRemove,
    ChipToggle,
    ChipClearAll,
    ChipMatchMode,
    FilterAdded,
    FilterEdited,
}

const GESTURES: [Gesture; 8] = [
    Gesture::SortKey,
    Gesture::RefreshKey,
    Gesture::ChipRemove,
    Gesture::ChipToggle,
    Gesture::ChipClearAll,
    Gesture::ChipMatchMode,
    Gesture::FilterAdded,
    Gesture::FilterEdited,
];

fn two_filters() -> TableViewState {
    TableViewState {
        sort: None,
        filters: vec![filter("name", "a", true), filter("sku", "", false)],
        match_all: true,
    }
}

fn do_gesture(app: &mut App, key: ItemKey, gesture: Gesture) {
    match gesture {
        Gesture::SortKey => {
            focus_grid(app, key);
            with_tab(app, key, |t| t.grid.cursor = (0, NAME));
            press(app, KeyCode::Char('s'));
        }
        Gesture::RefreshKey => {
            focus_grid(app, key);
            press(app, KeyCode::Char('r'));
        }
        Gesture::ChipRemove => {
            focus_chips(app, key);
            press(app, KeyCode::Char('x'));
        }
        Gesture::ChipToggle => {
            focus_chips(app, key);
            press(app, KeyCode::Char(' '));
        }
        Gesture::ChipClearAll => {
            focus_chips(app, key);
            press(app, KeyCode::Char('X'));
        }
        Gesture::ChipMatchMode => {
            focus_chips(app, key);
            press(app, KeyCode::Char('m'));
        }
        Gesture::FilterAdded | Gesture::FilterEdited => {
            let index = matches!(gesture, Gesture::FilterEdited).then_some(0);
            app.act(Act::EditFilter(index));
            assert_eq!(app.top_kind(), Some("filter"), "the editor opened");
            app.apply_filter(index, filter("name", "b", true));
        }
    }
}

#[test]
fn issue12_cancel_preserves_every_transition() {
    for draft in [Draft::Update, Draft::Insert, Draft::Delete] {
        for gesture in GESTURES {
            let mut app = connected();
            let key = open(&mut app, "products", false);
            show(&mut app, key, two_filters());
            stage(&mut app, key, draft);
            let before = checkpoint(&mut app, key);
            assert!(before.pending.total() > 0, "{draft:?} staged a change");

            do_gesture(&mut app, key, gesture);

            let what = format!("{draft:?} then {gesture:?}");
            assert!(guard_open(&mut app), "{what}: the guard asks first");
            assert!(app.top_kind() == Some("dialog"), "{what}: dialog is shown");
            assert_eq!(
                checkpoint(&mut app, key),
                before,
                "{what}: nothing changes before the answer"
            );

            press(&mut app, KeyCode::Esc);

            assert!(!guard_open(&mut app), "{what}: the guard closed");
            assert_eq!(
                checkpoint(&mut app, key),
                before,
                "{what}: Cancel keeps the draft and the shown controls"
            );
        }
    }
}

#[test]
fn issue12_sort_with_pending_edit_discard_installs_view_and_rows_once() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    stage(&mut app, key, Draft::Update);
    let revision = tab(&app, key).view_revision();

    do_gesture(&mut app, key, Gesture::SortKey);
    assert_eq!(tab(&app, key).view_revision(), revision);
    answer_guard(&mut app, 1);

    let t = tab(&app, key);
    assert_eq!(t.view_revision(), revision + 1, "one install");
    assert_eq!(t.view().sort, Some((NAME, SortDir::Asc)));
    assert_eq!(t.grid.sort, t.view().sort, "the marker follows the view");
    assert!(
        t.grid.pending().is_empty(),
        "the replacement dropped the draft"
    );
    assert!(status(&app).starts_with("Sorted by"), "{}", status(&app));
}

#[test]
fn issue12_clean_table_transitions_without_asking() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    do_gesture(&mut app, key, Gesture::SortKey);
    assert!(!guard_open(&mut app));
    assert_eq!(tab(&app, key).view().sort, Some((NAME, SortDir::Asc)));
}

#[test]
fn issue12_discard_prepares_before_mutating() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    show(&mut app, key, two_filters());
    stage(&mut app, key, Draft::Update);
    let before = checkpoint(&mut app, key);

    let mut broken = two_filters();
    broken.filters.push(filter("no_such_column", "x", true));
    let change = {
        let t = tab(&app, key);
        TableChange::filters(t, broken, None)
    };
    app.request_table_change(change, Review::Guard);
    assert!(guard_open(&mut app));
    answer_guard(&mut app, 1);

    assert_eq!(
        checkpoint(&mut app, key),
        before,
        "a read that fails changes nothing"
    );
    assert!(
        status(&app).contains("no_such_column"),
        "the failure is shown: {}",
        status(&app)
    );

    do_gesture(&mut app, key, Gesture::ChipClearAll);
    answer_guard(&mut app, 1);
    let t = tab(&app, key);
    assert_eq!(t.view_revision(), before.view_revision + 1);
    assert!(t.view().filters.is_empty());
    assert!(t.grid.pending().is_empty());
}

#[test]
fn issue12_install_rejection_after_prepare_preserves_draft() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    stage(&mut app, key, Draft::Update);
    let before = checkpoint(&mut app, key);

    let w = app.workbench.as_mut().expect("connected");
    w.modify_table(key, |t, cat| {
        let mut next = t.view().clone();
        next.sort = Some((NAME, SortDir::Desc));
        let loaded = t.prepare_load(cat, &next).expect("the view reads");
        let snapshot = t.grid.snapshot_changes();
        let token = t.grid.begin_submission(snapshot).expect("locks");
        assert_eq!(
            t.install_load(loaded, next),
            Err(junie_tui::widgets::grid::GridDataError::Busy)
        );
        t.grid.cancel_submission(token).expect("unlocks");
    })
    .expect("a table");

    assert_eq!(checkpoint(&mut app, key), before);
    assert!(!tab(&app, key).grid.is_editing());
}

#[test]
fn issue12_stale_answer_is_rejected_before_any_effect() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    stage(&mut app, key, Draft::Update);
    do_gesture(&mut app, key, Gesture::SortKey);
    assert!(guard_open(&mut app));

    app.invalidate_connection();
    let before = checkpoint(&mut app, key);
    answer_guard(&mut app, 1);

    assert_eq!(
        checkpoint(&mut app, key),
        before,
        "an answer from an ended connection touches nothing"
    );
}

#[test]
fn issue12_second_request_while_reviewing_is_refused() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    stage(&mut app, key, Draft::Update);
    do_gesture(&mut app, key, Gesture::SortKey);
    assert!(guard_open(&mut app));

    let change = {
        let t = tab(&app, key);
        TableChange::sort(t, Some((1, SortDir::Desc)))
    };
    app.request_table_change(change, Review::Guard);
    assert_eq!(status(&app), table_transition::PENDING_NOTICE);

    answer_guard(&mut app, 1);
    assert_eq!(
        tab(&app, key).view().sort,
        Some((NAME, SortDir::Asc)),
        "the reviewed destination was kept"
    );
}

#[test]
fn issue12_save_then_transition_uses_request_owner() {
    let mut app = connected();
    let a = open(&mut app, "products", false);
    let b = open(&mut app, "customers", false);
    stage(&mut app, a, Draft::Update);
    stage(&mut app, b, Draft::Update);
    activate(&mut app, a);
    let b_before = checkpoint(&mut app, b);

    do_gesture(&mut app, a, Gesture::SortKey);
    assert!(guard_open(&mut app));
    answer_guard(&mut app, 2);
    assert!(matches!(app.commit, Some(CommitState::Reviewing { .. })));
    assert!(app.transition_wait.is_some());
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Action(1));
    activate(&mut app, b);
    for _ in 0..SAVE_TICKS {
        app.advance();
    }

    let t = tab(&app, a);
    assert!(t.grid.pending().is_empty(), "A saved");
    assert_eq!(
        t.view().sort,
        Some((NAME, SortDir::Asc)),
        "A's next view installed"
    );
    assert_eq!(t.grid.sort, t.view().sort);
    assert!(status(&app).contains("Saved 1 change"), "{}", status(&app));
    assert!(status(&app).contains("Sorted by"), "{}", status(&app));
    assert_eq!(checkpoint(&mut app, b), b_before, "B is untouched");
    assert!(app.transition_wait.is_none());
}

#[test]
fn issue12_cancelled_save_drops_the_transition() {
    let mut app = connected();
    let a = open(&mut app, "products", false);
    stage(&mut app, a, Draft::Update);
    do_gesture(&mut app, a, Gesture::SortKey);
    let before = checkpoint(&mut app, a);
    answer_guard(&mut app, 2);
    app.dialog_closed(COMMIT_DIALOG, DialogResult::Cancelled);

    assert!(app.transition_wait.is_none());
    assert!(app.commit.is_none());
    for _ in 0..SAVE_TICKS {
        app.advance();
    }
    let after = checkpoint(&mut app, a);
    assert_eq!(after.view, before.view, "no sort was installed");
    assert_eq!(after.pending, before.pending, "the draft is still pending");
}

#[test]
fn issue12_save_refused_keeps_draft_and_drops_the_transition() {
    let mut app = connected();
    let a = open(&mut app, "products", false);
    app.workbench
        .as_mut()
        .expect("connected")
        .connection
        .safe_mode = SafeMode::ReadOnly;
    stage(&mut app, a, Draft::Update);
    do_gesture(&mut app, a, Gesture::SortKey);
    let before = checkpoint(&mut app, a);
    answer_guard(&mut app, 2);

    assert!(
        app.transition_wait.is_none(),
        "nothing waits on a refused save"
    );
    assert!(app.commit.is_none());
    assert_eq!(checkpoint(&mut app, a), before);
    assert!(status(&app).contains("Safe Mode"), "{}", status(&app));
}

#[test]
fn issue12_dirty_preview_promotes_and_survives() {
    let mut app = connected();
    let a = open(&mut app, "products", true);
    assert!(is_preview(&app, a));
    focus_grid(&mut app, a);
    with_tab(&mut app, a, |t| t.grid.cursor = (0, NAME));

    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(
        is_preview(&app, a),
        "an edit that changes nothing does not promote"
    );
    assert_eq!(tab(&app, a).dirty_count(), 0);

    press(&mut app, KeyCode::Enter);
    type_str(&mut app, "x");
    press(&mut app, KeyCode::Enter);
    assert_eq!(tab(&app, a).dirty_count(), 1);
    assert!(!is_preview(&app, a), "the first real edit promotes");

    let b = open(&mut app, "customers", true);
    assert_ne!(a, b, "the new preview is a new key");
    let t = tab(&app, a);
    assert_eq!(t.dirty_count(), 1, "A keeps its draft");
    assert!(!is_preview(&app, a));
    assert!(is_preview(&app, b));

    with_tab(&mut app, a, |t| t.grid.discard().expect("discards"));
    assert!(!is_preview(&app, a), "promotion outlives the draft");

    let c = open(&mut app, "orders", true);
    assert_ne!(c, b, "a clean preview is replaced by a new key");
    let w = app.workbench.as_ref().expect("connected");
    assert!(w.tab(b).is_none(), "the clean preview was replaced");
    assert!(w.tab(a).is_some());
}

#[test]
fn issue12_open_table_never_replaces_a_dirty_preview() {
    let mut app = connected();
    let a = open(&mut app, "products", true);
    stage(&mut app, a, Draft::Update);
    assert!(
        !is_preview(&app, a),
        "the workspace promotes a preview the moment a mutation leaves it dirty"
    );

    let b = open(&mut app, "customers", true);

    assert_ne!(a, b);
    assert_eq!(tab(&app, a).dirty_count(), 1, "the draft survived");
    assert!(!is_preview(&app, a), "the dirty preview stays pinned");
    assert!(is_preview(&app, b));
}

#[test]
fn issue12_read_only_refusal_does_not_promote() {
    let mut app = connected();
    let a = open(&mut app, "products", true);
    with_tab(&mut app, a, |t| t.grid.editable = false);
    focus_grid(&mut app, a);
    with_tab(&mut app, a, |t| t.grid.cursor = (0, NAME));
    press(&mut app, KeyCode::Enter);
    type_str(&mut app, "x");
    press(&mut app, KeyCode::Enter);
    assert!(is_preview(&app, a));
}

#[test]
fn issue12_filter_modal_stale_owner_rejected() {
    // The view the editor was opened against changes before it answers.
    let mut app = connected();
    let a = open(&mut app, "products", false);
    app.act(Act::EditFilter(None));
    assert_eq!(app.top_kind(), Some("filter"));
    show(
        &mut app,
        a,
        TableViewState {
            sort: Some((NAME, SortDir::Desc)),
            ..TableViewState::default()
        },
    );
    let newer = checkpoint(&mut app, a);
    app.apply_filter(None, filter("name", "late", true));
    assert_eq!(checkpoint(&mut app, a), newer, "newer settings survive");
    assert!(status(&app).contains("older view"), "{}", status(&app));

    // The tab closes and the table reopens: the answer cannot find it.
    app.act(Act::EditFilter(None));
    force_close(&mut app, a);
    let b = open(&mut app, "products", false);
    assert_ne!(a, b);
    let b_before = checkpoint(&mut app, b);
    app.apply_filter(None, filter("name", "late", true));
    assert_eq!(
        checkpoint(&mut app, b),
        b_before,
        "the reopened tab is untouched"
    );

    // The connection is replaced: the answer is from an ended era.
    app.act(Act::EditFilter(None));
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    let c = open(&mut app, "products", false);
    let c_before = checkpoint(&mut app, c);
    app.apply_filter(None, filter("name", "late", true));
    assert_eq!(checkpoint(&mut app, c), c_before);
}

#[test]
fn issue12_filter_answer_applies_to_the_table_it_was_opened_on() {
    let mut app = connected();
    let a = open(&mut app, "products", false);
    let b = open(&mut app, "customers", false);
    activate(&mut app, a);
    app.act(Act::EditFilter(None));
    activate(&mut app, b);
    let b_before = checkpoint(&mut app, b);

    app.apply_filter(None, filter("name", "a", true));

    assert_eq!(tab(&app, a).view().filters.len(), 1, "A got the filter");
    assert_eq!(checkpoint(&mut app, b), b_before, "B did not");
}

#[test]
fn issue12_equal_view_finishes_editor_without_reload() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    show(
        &mut app,
        key,
        TableViewState {
            sort: Some((NAME, SortDir::Asc)),
            ..TableViewState::default()
        },
    );
    focus_grid(&mut app, key);
    with_tab(&mut app, key, |t| {
        t.grid.cursor = (0, NAME);
        let _ = t.grid.begin_edit();
    });
    type_str(&mut app, "live");
    let (view_revision, source) = {
        let t = tab(&app, key);
        (t.view_revision(), t.grid.revision().source)
    };

    let change = {
        let t = tab(&app, key);
        TableChange::sort(t, Some((NAME, SortDir::Asc)))
    };
    app.request_table_change(change, Review::Guard);

    let t = tab(&app, key);
    assert!(!t.grid.is_editing(), "the valid edit was recorded");
    assert_eq!(t.grid.pending().total(), 1, "recorded once");
    assert_eq!(t.view_revision(), view_revision, "no view was installed");
    assert_eq!(t.grid.revision().source, source, "no rows were replaced");
    assert!(!guard_open(&mut app), "no leave dialog");
    assert!(app.layers.is_empty());
    assert_eq!(
        tab(&app, key).grid.value(0, NAME),
        &CellValue::Text("Cable traylive".into())
    );
}

#[test]
fn issue12_invalid_live_edit_stops_the_request_and_stays_open() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    focus_grid(&mut app, key);
    with_tab(&mut app, key, |t| {
        let g = &mut t.grid;
        g.validator = junie_tui::widgets::grid::cell_validator(|_, _| Err("no".to_owned()));
        g.cursor = (0, NAME);
        let _ = g.begin_edit();
    });
    type_str(&mut app, "bad");
    let revision = tab(&app, key).view_revision();

    let change = {
        let t = tab(&app, key);
        TableChange::sort(t, Some((NAME, SortDir::Asc)))
    };
    app.request_table_change(change, Review::Guard);

    let t = tab(&app, key);
    assert!(t.grid.is_editing(), "the input is kept");
    assert_eq!(t.view_revision(), revision);
    assert!(!guard_open(&mut app));
    assert!(
        status(&app).contains("Fix the open cell first"),
        "{}",
        status(&app)
    );
    let _ = CellEditResult::Changed;
}

#[test]
fn issue12_discard_gesture_uses_the_gate() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    show(&mut app, key, two_filters());
    stage(&mut app, key, Draft::Update);
    let before = checkpoint(&mut app, key);
    focus_grid(&mut app, key);

    press(&mut app, KeyCode::Char('U'));
    assert!(
        app.transition_wait.is_some(),
        "the discard question is open"
    );
    assert_eq!(checkpoint(&mut app, key), before);
    press(&mut app, KeyCode::Esc);
    assert_eq!(checkpoint(&mut app, key), before, "Cancel keeps everything");

    press(&mut app, KeyCode::Char('U'));
    app.dialog_closed(DISCARD_DIALOG, DialogResult::Action(1));
    let after = checkpoint(&mut app, key);
    assert!(after.pending.is_empty());
    assert_eq!(after.view, before.view, "the view stays");
    assert_eq!(after.view_revision, before.view_revision + 1);
    assert_eq!(status(&app), "Changes discarded");
}

#[test]
fn issue12_open_table_filtered_asks_before_replacing_a_draft() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    stage(&mut app, key, Draft::Update);
    let before = checkpoint(&mut app, key);

    app.act(Act::OpenTableFiltered(
        "products".into(),
        "name".into(),
        "a".into(),
    ));

    assert!(guard_open(&mut app));
    assert_eq!(checkpoint(&mut app, key), before);
    press(&mut app, KeyCode::Esc);
    assert_eq!(checkpoint(&mut app, key), before);
}

fn draw(app: &mut App) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 40))
        .expect("a test terminal");
    terminal
        .draw(|f| junie_tui::shell::render(app, f))
        .expect("draws");
}

/// Clicks the middle of a widget the way the terminal reports it: press, then release.
fn click_widget(app: &mut App, id: WidgetId) {
    use junie_tui::core::event::{Mouse, MouseKind};
    let area = app
        .shell
        .hits
        .area_of(id)
        .unwrap_or_else(|| panic!("{id:?} is on screen"));
    let at = ratatui::layout::Position::new(area.x + area.width / 2, area.y + area.height / 2);
    for kind in [MouseKind::Move, MouseKind::Down, MouseKind::Up] {
        app.handle_input(Input::Mouse(Mouse::new(kind, at)));
    }
}

#[test]
fn issue12_pointer_sort_and_match_mode_ask_before_replacing_a_draft() {
    let mut app = connected();
    let key = open(&mut app, "products", false);
    show(&mut app, key, two_filters());
    stage(&mut app, key, Draft::Update);
    let before = checkpoint(&mut app, key);
    draw(&mut app);

    let header = tab(&app, key).grid.header_id(NAME);
    click_widget(&mut app, header);
    assert!(
        guard_open(&mut app),
        "a header click asks: {}",
        status(&app)
    );
    assert_eq!(checkpoint(&mut app, key), before);
    answer_guard(&mut app, 0);
    assert_eq!(checkpoint(&mut app, key), before, "Cancel keeps the draft");

    draw(&mut app);
    let lead = tab(&app, key).chips.lead_id();
    click_widget(&mut app, lead);
    assert!(
        guard_open(&mut app),
        "a match mode click asks: {}",
        status(&app)
    );
    assert_eq!(checkpoint(&mut app, key), before);
    answer_guard(&mut app, 1);
    let t = tab(&app, key);
    assert!(!t.view().match_all, "Discard installed the new match mode");
    assert!(t.grid.pending().is_empty());
}

#[test]
fn issue12_schema_switch_reveals_the_explorer_it_focuses() {
    let mut app = connected();
    app.workbench.as_mut().expect("connected").explorer_visible = false;
    app.open_switcher();
    type_str(&mut app, "analytics");
    let row = app
        .switcher_targets
        .iter()
        .position(|t| matches!(t, crate::model::SwitchTarget::Schema(s) if s == "analytics"))
        .expect("the schema is offered");
    for _ in 0..row {
        press(&mut app, KeyCode::Down);
    }
    press(&mut app, KeyCode::Enter);

    let w = app.workbench.as_ref().expect("connected");
    assert_eq!(w.schema, "analytics");
    assert!(w.explorer_visible, "the focused explorer is shown");
    assert!(app.shell.focus.is(crate::workbench::EXPLORER));
}
