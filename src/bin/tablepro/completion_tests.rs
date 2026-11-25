//! Completion at Home on the rendered screen (#84). Keys go through
//! `App::handle_input`, the popup is read back from the drawn cells.

use junie_tui::core::event::{Input, Key};
use junie_tui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Route};
use crate::workbench::WorkTab;

const ORIGINAL: &str = " SELECT id FROM users";

fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.handle_input(Input::Key(Key { code, mods }));
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|f| junie_tui::shell::render(app, f))
        .unwrap();
    let buf = terminal.backend().buffer();
    let area = buf.area;
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn query_text(app: &App) -> String {
    let w = app.workbench.as_ref().expect("connected");
    match w.active_tab() {
        Some(WorkTab::Query(q)) => q.editor.text().to_owned(),
        _ => panic!("the active tab is the query"),
    }
}

#[test]
fn completion_prefix_home_popup_renders_and_accepts() {
    let mut app = App::new(Theme::junie());
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    let w = app.workbench.as_mut().expect("connected");
    w.new_query(ORIGINAL);
    let focus = w.primary_focus().expect("the query editor has focus");
    app.shell.focus.focus(focus);

    press(&mut app, KeyCode::Home, KeyModifiers::NONE);
    assert!(!screen(&app).contains("INSERT INTO"), "no popup before Ctrl+Space");

    press(&mut app, KeyCode::Char(' '), KeyModifiers::CONTROL);
    let open = screen(&app);
    assert!(open.contains("INSERT INTO"), "the Start popup is drawn:\n{open}");

    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    let text = query_text(&app);
    assert!(text.len() > ORIGINAL.len(), "{text:?}");
    assert!(text.ends_with(ORIGINAL), "the SQL after the caret is unchanged: {text:?}");
    assert!(!screen(&app).contains("INSERT INTO"), "the popup closed after accept");
    assert_eq!(app.route(), Route::Workbench, "the editor is still alive");
}
