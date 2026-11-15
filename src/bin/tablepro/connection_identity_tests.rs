//! Regression tests for stable connection identity (#85). They drive the real
//! `App::handle_input` path with keys and ticks.

use junie_tui::core::event::{Input, Key};
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use clap::error::ErrorKind;
use junie_tui::core::id::{ItemKey, WidgetId};

use crate::app::{App, Route};
use crate::connections::{ConnState, ConnectionEntry, ConnectionsScreen};
use crate::db::{ConnectOutcome, Connection, SafeMode};

fn conn(name: &str, host: &str) -> Connection {
    let mut c = crate::db::connections().remove(0);
    c.name = name.into();
    c.host = host.into();
    c.group = "Personal".into();
    c.outcome = ConnectOutcome::Ok;
    c
}

fn app_with(connections: Vec<Connection>) -> App {
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    app.connections = ConnectionsScreen::new(connections);
    app
}

fn key(code: KeyCode, mods: KeyModifiers) -> Input {
    Input::Key(Key { code, mods })
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_input(key(code, KeyModifiers::NONE));
}

fn ctrl(app: &mut App, c: char) {
    app.handle_input(key(KeyCode::Char(c), KeyModifiers::CONTROL));
}

fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.handle_input(Input::Tick);
    }
}

fn alt(app: &mut App, c: char) {
    app.handle_input(key(KeyCode::Char(c), KeyModifiers::ALT));
}

fn select_row(app: &mut App, downs: usize) {
    for _ in 0..downs {
        press(app, KeyCode::Down);
    }
}

fn draw(app: &mut App) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| junie_tui::shell::render(app, f)).unwrap();
}

fn delete_selected(app: &mut App) {
    let before = app.connections.len();
    press(app, KeyCode::Char('d'));
    draw(app);
    press(app, KeyCode::Tab);
    press(app, KeyCode::Enter);
    assert_eq!(
        app.connections.len(),
        before - 1,
        "the dialog's Delete action ran"
    );
}

#[test]
fn connection_identity_duplicate_names() {
    let mut a = conn("Dup", "host-a");
    a.outcome = ConnectOutcome::AuthFailed;
    let mut app = app_with(vec![a, conn("Dup", "host-b")]);
    let before = entries(&app);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 12);
    assert_eq!(app.route(), Route::Workbench);
    let wb = app.workbench.as_ref().expect("workbench opens");
    assert_eq!(wb.connection.host, "host-b");
    assert_eq!(wb.connection.outcome, ConnectOutcome::Ok);
    assert_ne!(
        wb.connection.outcome,
        before[0].value().outcome,
        "A and B differ in outcome"
    );
    assert_eq!(wb.connection_key, Some(before[1].key()));
    assert_ne!(wb.connection_key, Some(before[0].key()));
    assert_eq!(entries(&app)[0], before[0], "A is untouched by B's attempt");
}

#[test]
fn connection_identity_rename_delete_cancels() {
    let mut app = app_with(vec![conn("Dup", "host-a"), conn("Dup", "host-b")]);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 3);
    delete_selected(&mut app);
    tick(&mut app, 12);
    assert_eq!(
        app.route(),
        Route::Connections,
        "no substitution with host-a"
    );
    assert!(app.workbench.is_none());
}

fn rename_selected(app: &mut App) {
    press(app, KeyCode::Char('e'));
    press(app, KeyCode::Enter);
    press(app, KeyCode::Char('Z'));
    ctrl(app, 's');
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn entries(app: &App) -> Vec<ConnectionEntry> {
    app.connections.iter().cloned().collect()
}

fn pending_generation(app: &App) -> u64 {
    match &app.connections.state {
        ConnState::Connecting(req) => req.generation,
        other => panic!("expected a pending attempt, found {other:?}"),
    }
}

#[test]
fn connection_identity_rename_cancels() {
    let mut app = app_with(vec![conn("Dup", "host-a"), conn("Dup", "host-b")]);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 3);
    rename_selected(&mut app);
    tick(&mut app, 12);
    assert_eq!(
        app.route(),
        Route::Connections,
        "renamed target does not connect"
    );
    assert!(app.workbench.is_none());
    assert!(status(&app).contains("changed"), "{}", status(&app));
}

#[test]
fn connection_identity_delete_reports_deleted() {
    let mut app = app_with(vec![conn("Dup", "host-a"), conn("Dup", "host-b")]);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    delete_selected(&mut app);
    tick(&mut app, 12);
    assert!(status(&app).contains("deleted"), "{}", status(&app));
}

#[test]
fn connection_identity_reorder_preserves_target() {
    let mut app = app_with(vec![conn("Dup", "host-a"), conn("Dup", "host-b")]);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 2);
    press(&mut app, KeyCode::Up);
    alt(&mut app, 'd');
    assert_eq!(
        app.connections.len(),
        3,
        "a copy of host-a was inserted before host-b"
    );
    tick(&mut app, 12);
    assert_eq!(app.route(), Route::Workbench);
    assert_eq!(app.workbench.as_ref().unwrap().connection.host, "host-b");
}

#[test]
fn connection_identity_repeat_start_rejects_old() {
    let mut app = app_with(vec![conn("Dup", "host-a"), conn("Other", "host-b")]);
    select_row(&mut app, 1);
    press(&mut app, KeyCode::Enter);
    let old = pending_generation(&app);
    tick(&mut app, 4);
    select_row(&mut app, 1);
    press(&mut app, KeyCode::Enter);
    let current = pending_generation(&app);
    assert!(current > old);
    let before = app.connections.state.clone();
    assert!(
        app.connections.complete(old).is_none(),
        "late completion emits nothing"
    );
    assert_eq!(
        app.connections.state, before,
        "the newer attempt stays busy"
    );
    tick(&mut app, 12);
    assert_eq!(app.workbench.as_ref().unwrap().connection.host, "host-b");
    assert!(
        app.connections.complete(current).is_none(),
        "each completion is admitted once"
    );
}

#[test]
fn connection_identity_last_used_is_not_settings_revision() {
    let mut app = app_with(vec![conn("Dup", "host-a")]);
    select_row(&mut app, 1);
    let key = app.connections.selected_key().unwrap();
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 12);
    let entry = app.connections.get(key).unwrap();
    assert_eq!(entry.value().last_used, "just now");
    assert_eq!(entry.revision(), 0, "bookkeeping is not a settings edit");
    assert_eq!(
        app.connections.set_safe_mode(key, SafeMode::ReadOnly),
        Ok(true)
    );
    assert_eq!(app.connections.get(key).unwrap().revision(), 1);
    assert_eq!(
        app.connections.set_safe_mode(key, SafeMode::ReadOnly),
        Ok(false)
    );
    assert_eq!(
        app.connections.get(key).unwrap().revision(),
        1,
        "equal edit is a no-op"
    );
}

#[test]
fn connection_identity_unchanged_form_save_keeps_revision() {
    let mut app = app_with(vec![conn("Dup", "host-a")]);
    select_row(&mut app, 1);
    press(&mut app, KeyCode::Char('e'));
    ctrl(&mut app, 's');
    let entry = app.connections.iter().next().unwrap();
    assert_eq!(entry.revision(), 0);
    assert!(!app.connections.form_dirty());
}

#[test]
fn connection_identity_cli_duplicate_name_is_ambiguous() {
    let screen = ConnectionsScreen::new(vec![
        conn("Dup", "host-a"),
        conn("dup", "host-b"),
        conn("Unique", "host-c"),
    ]);
    let ambiguous = crate::connection_key(&screen, "DUP").unwrap_err();
    assert_eq!(ambiguous.kind(), ErrorKind::ValueValidation);
    let missing = crate::connection_key(&screen, "absent").unwrap_err();
    assert_eq!(missing.kind(), ErrorKind::ValueValidation);
    let key = crate::connection_key(&screen, "unique").unwrap();
    assert_eq!(screen.snapshot(key).unwrap().host, "host-c");

    let mut app = app_with(vec![
        conn("Dup", "host-a"),
        conn("Dup", "host-b"),
        conn("Unique", "host-c"),
    ]);
    let key = crate::connection_key(&app.connections, "UNIQUE").unwrap();
    app.connect_key(key);
    assert_eq!(app.workbench.as_ref().unwrap().connection.host, "host-c");
}

#[test]
fn connection_identity_duplicate_allocates_fresh_key() {
    let mut app = app_with(vec![conn("Dup", "host-a")]);
    select_row(&mut app, 1);
    let mut seen: Vec<ItemKey> = app.connections.iter().map(|e| e.key()).collect();
    for _ in 0..3 {
        alt(&mut app, 'd');
    }
    let copies: Vec<_> = app.connections.iter().skip(1).collect();
    assert_eq!(copies.len(), 3);
    assert!(copies.iter().all(|e| e.value().name == "Dup (Copy)"));
    seen.extend(copies.iter().map(|e| e.key()));
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "labels coincide, keys do not");

    let victim = copies[2].key();
    assert!(app.connections.select_key(victim));
    delete_selected(&mut app);
    alt(&mut app, 'd');
    let fresh = app
        .connections
        .iter()
        .map(|e| e.key())
        .find(|k| !seen.contains(k));
    assert!(
        fresh.is_some(),
        "a recreated entry never reuses a deleted key"
    );
}

#[test]
fn connection_identity_counter_exhaustion_is_atomic() {
    let two = || vec![conn("Dup", "host-a"), conn("Dup", "host-b")];

    // Key allocation exhausted: Duplicate is refused.
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    app.connections = ConnectionsScreen::with_counters(two(), u64::MAX, 0, 0);
    select_row(&mut app, 1);
    let (before, selected) = (entries(&app), app.connections.selected_key());
    alt(&mut app, 'd');
    assert_eq!(entries(&app), before);
    assert_eq!(app.connections.selected_key(), selected);
    assert!(status(&app).contains("not applied"), "{}", status(&app));

    // Attempt generation exhausted: Connect is refused and nothing is pending.
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    app.connections = ConnectionsScreen::with_counters(two(), 2, u64::MAX, 0);
    select_row(&mut app, 1);
    let (before, selected) = (entries(&app), app.connections.selected_key());
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.connections.state, ConnState::Idle);
    assert_eq!(entries(&app), before);
    assert_eq!(app.connections.selected_key(), selected);
    tick(&mut app, 12);
    assert!(app.workbench.is_none(), "no Connected event");

    // A pending attempt survives a refused edit; the entry is untouched.
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    app.connections = ConnectionsScreen::with_counters(two(), 2, 0, u64::MAX);
    select_row(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    let (before, state, selected) = (
        entries(&app),
        app.connections.state.clone(),
        app.connections.selected_key(),
    );
    rename_selected(&mut app);
    assert_eq!(entries(&app), before);
    assert_eq!(app.connections.state, state);
    assert_eq!(app.connections.selected_key(), selected);
    assert!(
        app.connections.form_dirty(),
        "the form stays open with the edit"
    );
    assert!(status(&app).contains("not applied"), "{}", status(&app));
    assert_eq!(
        app.connections
            .set_safe_mode(selected.unwrap(), SafeMode::ReadOnly),
        Err(crate::connections::ConnectionMutationError::IdentityExhausted)
    );
    assert_eq!(entries(&app), before);
    tick(&mut app, 12);
    assert_eq!(app.workbench.as_ref().unwrap().connection.host, "host-b");
}

#[test]
fn connection_identity_safe_mode_route_is_atomic_on_exhaustion() {
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    app.connections = ConnectionsScreen::with_counters(vec![conn("Dup", "host-a")], 1, 0, u64::MAX);
    let key = app.connections.iter().next().unwrap().key();
    app.connect_key(key);
    assert_eq!(
        app.workbench.as_ref().unwrap().connection.safe_mode,
        SafeMode::Silent
    );
    let before = entries(&app);

    ctrl(&mut app, 'l');
    draw(&mut app);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        app.workbench.as_ref().unwrap().connection.safe_mode,
        SafeMode::Silent,
        "workbench keeps the stored level when the save is refused"
    );
    assert_eq!(entries(&app), before, "stored connection is unchanged");
    assert!(status(&app).contains("not applied"), "{}", status(&app));
}

#[test]
fn connection_selection_follows_the_entry_when_its_group_changes() {
    let a = conn("A", "host-a");
    let mut b = conn("B", "host-b");
    b.group = "Acme".into();
    let c = conn("C", "host-c");
    let mut app = app_with(vec![a, b, c]);
    // Rows: Personal, A, C, Acme, B.
    select_row(&mut app, 2);
    let c_key = app.connections.selected_key().expect("C is selected");
    assert_eq!(
        app.connections.get(c_key).map(|e| e.value().name.as_str()),
        Some("C")
    );

    press(&mut app, KeyCode::Char('e'));
    draw(&mut app);
    app.shell
        .focus
        .focus(WidgetId::of("connections").sub("form").sub("group"));
    press(&mut app, KeyCode::Char('A'));
    ctrl(&mut app, 's');
    draw(&mut app);
    assert_eq!(
        app.connections.selected_key(),
        Some(c_key),
        "saving keeps C selected"
    );

    // Rows are now Personal, A, Acme, B, C. C sits last; the cursor follows it.
    press(&mut app, KeyCode::Up);
    let above = app.connections.selected_key().expect("a neighbour");
    assert_eq!(
        app.connections.get(above).map(|e| e.value().name.as_str()),
        Some("B"),
        "moving up from C reaches B, not whatever now sits at C's old row"
    );
}
