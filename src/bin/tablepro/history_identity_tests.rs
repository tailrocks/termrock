use std::sync::{Arc, Mutex};

use junie_tui::core::event::{Input, Key, Mouse, MouseKind};
use junie_tui::shell::{self, Clipboard, NativeClipboard};
use junie_tui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;

use crate::app::App;
use crate::model::{HistoryEntry, HistorySource};
use crate::workbench::WorkTab;

#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<String>>);

impl NativeClipboard for Recorder {
    fn write(&self, text: &str) -> bool {
        self.0.lock().expect("recorder").push(text.to_owned());
        true
    }
}

fn entry(sql: &str, connection: &str, failed: bool) -> HistoryEntry {
    HistoryEntry {
        id: 0,
        sql: sql.to_owned(),
        connection: connection.to_owned(),
        database: "acme".into(),
        schema: "public".into(),
        minutes_ago: 1,
        duration_ms: Some(4),
        rows: Some(1),
        error: failed.then(|| "boom".to_owned()),
        source: HistorySource::Editor,
    }
}

struct Fixture {
    app: App,
    clip: Arc<Recorder>,
}

impl Fixture {
    fn new(queries: &[(&str, bool)]) -> Self {
        let mut app = App::new(Theme::junie());
        app.connect(1);
        app.history = crate::model::History::default();
        let clip = Arc::new(Recorder::default());
        app.shell
            .set_clipboard(Clipboard::new().osc52(false).native(clip.clone()));
        let conn = app
            .workbench
            .as_ref()
            .expect("connected")
            .connection
            .name
            .clone();
        for (sql, failed) in queries {
            app.history.push(entry(sql, &conn, *failed));
        }
        let w = app.workbench.as_mut().expect("connected");
        w.open_history(&app.history);
        let mut f = Self { app, clip };
        f.focus_list();
        f.draw();
        f
    }

    fn conn(&self) -> String {
        self.app
            .workbench
            .as_ref()
            .expect("connected")
            .connection
            .name
            .clone()
    }

    fn focus_list(&mut self) {
        let id = self.tab().list.id;
        self.app.shell.focus.focus(id);
    }

    fn tab(&self) -> &crate::tabs::HistoryTab {
        match self.app.workbench.as_ref().expect("connected").active_tab() {
            Some(WorkTab::History(h)) => h,
            _ => panic!("the history tab is active"),
        }
    }

    fn draw(&mut self) {
        let mut term = ratatui::Terminal::new(TestBackend::new(140, 40)).expect("terminal");
        term.draw(|f| shell::render(&mut self.app, f))
            .expect("draws");
    }

    fn key(&mut self, code: KeyCode) {
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        shell::handle(&mut self.app, Input::Key(key));
        self.draw();
    }

    fn chars(&mut self, text: &str) {
        for c in text.chars() {
            self.key(KeyCode::Char(c));
        }
    }

    fn mouse(&mut self, kind: MouseKind, pos: Position) {
        shell::handle(&mut self.app, Input::Mouse(Mouse::new(kind, pos)));
        self.draw();
    }

    fn select(&mut self, sql: &str) {
        let at = self.row_of(sql);
        self.mouse(MouseKind::Down, at);
        self.mouse(MouseKind::Up, at);
        assert_eq!(self.selected_sql().as_deref(), Some(sql));
    }

    fn row_of(&self, sql: &str) -> Position {
        let list = self
            .app
            .shell
            .hits
            .area_of(self.tab().list.id)
            .expect("list area");
        let tab = self.tab();
        let index = tab
            .list
            .items
            .iter()
            .position(|item| item.label == sql)
            .unwrap_or_else(|| panic!("{sql} is listed"));
        let row = index - tab.list.scroll.offset;
        Position::new(list.x + 3, list.y + row as u16)
    }

    fn selected_sql(&self) -> Option<String> {
        self.tab()
            .current_entry(&self.app.history)
            .map(|e| e.sql.clone())
    }

    fn listed(&self) -> Vec<String> {
        self.tab()
            .list
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect()
    }

    fn complete(&mut self, sql: &str) {
        let conn = self.conn();
        let w = self.app.workbench.as_mut().expect("connected");
        w.record_history(&mut self.app.history, vec![entry(sql, &conn, false)]);
        self.draw();
    }

    fn open_menu(&mut self, sql: &str) {
        let at = self.row_of(sql);
        self.mouse(MouseKind::Secondary, at);
        assert!(!self.app.layers.is_empty(), "the context menu is open");
    }

    fn choose_menu_item(&mut self, index: usize) {
        for _ in 0..index {
            self.key(KeyCode::Down);
        }
        self.key(KeyCode::Enter);
    }

    fn query_tabs(&self) -> Vec<String> {
        self.app
            .workbench
            .as_ref()
            .expect("connected")
            .tabs
            .iter()
            .filter_map(|t| match t {
                WorkTab::Query(q) => Some(q.editor.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    fn copied(&self) -> Vec<String> {
        self.clip.0.lock().expect("recorder").clone()
    }
}

const FOUR: [(&str, bool); 4] = [("q1", false), ("q2", false), ("q3", false), ("q4", false)];

#[test]
fn history_identity_prepend_preserves_selected_sql() {
    let mut f = Fixture::new(&FOUR);
    assert_eq!(f.listed(), ["q4", "q3", "q2", "q1"]);
    f.select("q2");
    let detail = f.tab().detail.text().to_owned();
    assert_eq!(detail, "q2");

    f.complete("q5");
    assert_eq!(f.listed(), ["q5", "q4", "q3", "q2", "q1"]);
    assert_eq!(f.selected_sql().as_deref(), Some("q2"));
    assert_eq!(f.tab().list.cursor, 3);
    assert_eq!(f.tab().list.chosen, Some(3));
    assert_eq!(f.tab().detail.text(), detail);

    f.complete("q6");
    f.complete("q7");
    assert_eq!(f.selected_sql().as_deref(), Some("q2"));
    assert_eq!(f.tab().list.cursor, 5);
}

#[test]
fn history_identity_filter_fallback() {
    let mut f = Fixture::new(&[
        ("q1", false),
        ("q2", true),
        ("q3", true),
        ("q4", false),
        ("q5", true),
    ]);
    assert_eq!(f.listed(), ["q5", "q4", "q3", "q2", "q1"]);

    f.select("q3");
    f.chars("s");
    assert_eq!(f.listed(), ["q5", "q3", "q2"]);
    assert_eq!(
        f.selected_sql().as_deref(),
        Some("q3"),
        "a still-visible selection is restored at its new index"
    );
    assert_eq!(f.tab().list.cursor, 1);
    f.chars("s");

    f.select("q4");
    f.chars("s");
    assert_eq!(
        f.selected_sql().as_deref(),
        Some("q3"),
        "a hidden selection falls to the successor at its old position"
    );

    f.chars("s");
    f.select("q1");
    f.chars("s");
    assert_eq!(f.listed(), ["q5", "q3", "q2"]);
    assert_eq!(
        f.selected_sql().as_deref(),
        Some("q2"),
        "with no successor the predecessor is chosen"
    );

    f.key(KeyCode::Char('/'));
    f.chars("zzz");
    f.key(KeyCode::Enter);
    assert!(f.listed().is_empty());
    assert_eq!(f.selected_sql(), None);
    assert_eq!(f.tab().list.chosen, None);
    assert_eq!(f.tab().detail.text(), "");
}

#[test]
fn history_identity_pruned_selection_has_no_stale_lookup() {
    let mut f = Fixture::new(&FOUR);
    f.select("q3");
    let conn = f.conn();
    f.app.history.entries.retain(|e| e.sql != "q3");
    let w = f.app.workbench.as_mut().expect("connected");
    w.record_history(&mut f.app.history, vec![entry("q5", &conn, false)]);
    f.draw();
    assert_eq!(f.listed(), ["q5", "q4", "q2", "q1"]);
    assert_eq!(
        f.selected_sql().as_deref(),
        Some("q2"),
        "the entry that moved into the old visible position is selected"
    );
}

#[test]
fn history_identity_context_survives_background_append() {
    for (item, what) in [(0, "open"), (1, "rerun"), (2, "copy")] {
        let mut f = Fixture::new(&FOUR);
        f.open_menu("q2");
        f.complete("q5");
        f.complete("q6");
        assert!(!f.app.layers.is_empty(), "the menu stays open");
        let tabs_before = f.query_tabs().len();
        f.choose_menu_item(item);
        match what {
            "copy" => assert_eq!(f.copied(), ["q2"], "{what}"),
            _ => {
                let tabs = f.query_tabs();
                assert_eq!(tabs.len(), tabs_before + 1, "{what}: a tab opens");
                assert_eq!(tabs.last().map(String::as_str), Some("q2"), "{what}");
            }
        }
    }
}

#[test]
fn history_identity_context_refuses_a_pruned_target() {
    for item in [0, 1, 2] {
        let mut f = Fixture::new(&FOUR);
        f.open_menu("q2");
        f.app.history.entries.retain(|e| e.sql != "q2");
        f.complete("q5");
        let tabs_before = f.query_tabs().len();
        f.choose_menu_item(item);
        assert!(f.copied().is_empty(), "item {item} copies nothing");
        assert_eq!(
            f.query_tabs().len(),
            tabs_before,
            "item {item} opens nothing"
        );
        let status = f
            .app
            .shell
            .status
            .get()
            .map(|(text, _)| text.to_owned())
            .unwrap_or_default();
        assert!(status.contains("no longer available"), "{status}");
    }
}

#[test]
fn history_identity_detail_view_does_not_reset() {
    let long = format!("SELECT {}", "a_long_column_name, ".repeat(12));
    let mut f = Fixture::new(&[("q1", false), (long.as_str(), false), ("q3", false)]);
    f.key(KeyCode::Down);
    assert_eq!(f.selected_sql().as_deref(), Some(long.as_str()));
    let id = f.tab().detail.id;
    f.app.shell.focus.focus(id);
    f.key(KeyCode::End);
    f.key(KeyCode::Right);
    let caret = f.tab().detail.cursor_offset();
    let text = f.tab().detail.text().to_owned();
    assert!(caret > 0, "the caret moved");

    for sql in ["q4", "q5", "q6"] {
        f.complete(sql);
        assert_eq!(f.selected_sql().as_deref(), Some(long.as_str()));
        assert_eq!(f.tab().detail.cursor_offset(), caret);
        assert_eq!(f.tab().detail.text(), text);
    }
}

#[test]
fn history_identity_context_refuses_a_closed_owner() {
    let mut f = Fixture::new(&FOUR);
    f.open_menu("q2");
    let w = f.app.workbench.as_mut().expect("connected");
    let at = w
        .tabs
        .iter()
        .position(|t| matches!(t, WorkTab::History(_)))
        .expect("history tab");
    w.close_tab(at);
    let tabs_before = f.query_tabs();
    f.choose_menu_item(2);
    assert!(f.copied().is_empty());
    assert_eq!(f.query_tabs(), tabs_before);
    let status = f
        .app
        .shell
        .status
        .get()
        .map(|(text, _)| text.to_owned())
        .unwrap_or_default();
    assert!(status.contains("no longer available"), "{status}");
}

#[test]
fn history_identity_context_status_filter_keeps_the_selected_record() {
    let mut f = Fixture::new(&[("q1", false), ("q2", true), ("q3", true), ("q4", false)]);
    f.open_menu("q3");
    f.complete("q5");
    f.choose_menu_item(5);
    assert!(f.tab().failed_only);
    assert_eq!(f.listed(), ["q3", "q2"]);
    assert_eq!(f.selected_sql().as_deref(), Some("q3"));
}

#[test]
fn history_identity_viewport_anchor_survives_append() {
    let many: Vec<(String, bool)> = (1..=60).map(|n| (format!("q{n}"), false)).collect();
    let many: Vec<(&str, bool)> = many.iter().map(|(s, f)| (s.as_str(), *f)).collect();
    let mut f = Fixture::new(&many);
    for _ in 0..40 {
        f.key(KeyCode::Down);
    }
    assert_eq!(f.selected_sql().as_deref(), Some("q20"));
    let below_top = f.tab().list.cursor - f.tab().list.scroll.offset;
    assert!(f.tab().list.scroll.offset > 0, "the list is scrolled");

    for sql in ["q61", "q62", "q63"] {
        f.complete(sql);
        assert_eq!(f.selected_sql().as_deref(), Some("q20"));
        assert_eq!(
            f.tab().list.cursor - f.tab().list.scroll.offset,
            below_top,
            "the selection keeps its row in the viewport"
        );
    }
}
