//! Regression tests for #105 in TablePro: a query editor selection that
//! survives blur is what Run executes. They drive the real `App::handle_input`
//! path with keys, pointer clicks and draws.

use junie_tui::core::event::{Input, Key, Mouse, MouseKind};
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;

use crate::app::App;
use crate::tabs::TOOLBAR_RUN;
use crate::workbench::{TABSTRIP, WorkTab};

const SQL: &str = "SELECT 1;\nSELECT 2;\nSELECT 3;";
const SELECTED: &str = "SELECT 1;\nSELECT 2;";

fn key(code: KeyCode, mods: KeyModifiers) -> Input {
    Input::Key(Key { code, mods })
}

fn draw(app: &mut App) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| junie_tui::shell::render(app, f)).unwrap();
}

fn click(app: &mut App, at: Position) {
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Down, at)));
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Up, at)));
}

fn query_tab(app: &mut App) -> &mut crate::tabs::QueryTab {
    match app.workbench.as_mut().and_then(|w| w.active_tab_mut()) {
        Some(WorkTab::Query(q)) => q,
        _ => panic!("a query tab is active"),
    }
}

/// Connect, type `SQL`, and select its first two statements from the start,
/// so the selection and the block under the caret differ.
fn app_with_selected_statements() -> App {
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    let connection = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(connection);
    draw(&mut app);

    let editor = query_tab(&mut app).editor.id;
    app.shell.focus.focus(editor);
    query_tab(&mut app).editor.begin_edit();
    app.handle_input(Input::Paste(SQL.into()));
    query_tab(&mut app).editor.jump_to(0);
    for _ in 0..SELECTED.chars().count() {
        app.handle_input(key(KeyCode::Right, KeyModifiers::SHIFT));
    }
    assert_eq!(
        query_tab(&mut app)
            .editor
            .selection_or_block()
            .map(|(text, _)| text)
            .as_deref(),
        Some(SELECTED),
        "the selection is in place before blur"
    );
    app
}

fn running_statements(app: &mut App) -> Vec<String> {
    query_tab(app)
        .running
        .as_ref()
        .map(|r| r.statements.iter().map(|(text, _)| text.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn run_after_strip_and_toolbar_click_executes_the_retained_selection() {
    let mut app = app_with_selected_statements();
    let editor = query_tab(&mut app).editor.id;

    draw(&mut app);
    let strip = app
        .shell
        .hits
        .area_of(TABSTRIP)
        .expect("the workbench tab strip is drawn");
    // Blank strip space past the tab labels: the strip takes focus and
    // activates nothing, so the editor blurs.
    let blank = Position::new(strip.x + strip.width / 2, strip.y);
    assert_eq!(
        app.shell.hits.hit(blank),
        Some(TABSTRIP),
        "the blank strip is its own hit region"
    );
    click(&mut app, blank);
    draw(&mut app);
    assert_ne!(
        app.shell.focus.current(),
        Some(editor),
        "the strip click moved focus off the editor"
    );
    assert_eq!(
        query_tab(&mut app)
            .editor
            .selection_or_block()
            .map(|(text, _)| text)
            .as_deref(),
        Some(SELECTED),
        "the selection survives the blur"
    );

    let run = app
        .shell
        .hits
        .area_of(TOOLBAR_RUN)
        .expect("the toolbar Run button is drawn");
    click(&mut app, Position::new(run.x + 1, run.y));

    assert_eq!(
        running_statements(&mut app),
        ["SELECT 1", "SELECT 2"],
        "Run executes the selection, not the statement under the caret"
    );
}
