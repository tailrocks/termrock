// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/app_tests.rs (MIT).

//! Shared TablePro workbench: standalone binary + Applications mount.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use termrock::input::{Event, KeyCode, KeyEvent, KeyModifiers};
use termrock::runtime::FrameTick;
use termrock::style::ColorCapability;
use termrock_catalog::catalog::{CatalogProfile, PageId};
use termrock_catalog::shell::App as CatalogApp;
use termrock_catalog::tablepro::db::Catalog;
use termrock_catalog::tablepro::workbench::WorkTab;
use termrock_catalog::tablepro::workbench::Workbench;
use termrock_catalog::tablepro::{App, ParseError, Screen, connections, help_text, parse_args};

fn tick() -> FrameTick {
    FrameTick::manual(
        termrock::runtime::Instant::now(),
        std::time::Duration::from_millis(0),
        std::time::Duration::from_millis(16),
    )
}

struct Harness {
    app: App,
    term: Terminal<TestBackend>,
}

impl Harness {
    fn new(w: u16, h: u16) -> Self {
        let app = App::new(ColorCapability::Truecolor);
        let term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut h = Self { app, term };
        h.draw();
        h
    }

    fn connected(w: u16, h: u16) -> Self {
        let mut h = Self::new(w, h);
        h.app.connect_named("Production").unwrap();
        h.draw();
        h
    }

    fn draw(&mut self) {
        let t = tick();
        self.term.draw(|f| self.app.render(f, t)).unwrap();
    }

    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let _ = self
            .app
            .handle_event(Event::Key(KeyEvent::new(code, modifiers)), tick());
        self.draw();
    }

    fn paste(&mut self, text: &str) {
        let _ = self.app.handle_event(Event::Paste(text.to_owned()), tick());
        self.draw();
    }

    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.key(KeyCode::Char(ch), KeyModifiers::NONE);
        }
    }

    fn text(&self) -> String {
        let buf = self.term.backend().buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }
}

#[test]
fn help_twice_matches_source_usage() {
    let a = parse_args(["--help"]).unwrap_err();
    let b = parse_args(["-h"]).unwrap_err();
    let t = help_text();
    assert!(matches!(a, ParseError::Help(_)));
    assert_eq!(a.to_string(), b.to_string());
    assert_eq!(a.to_string(), t);
    assert!(t.contains("Keys: Ctrl+O open quickly"));
    assert!(t.contains("--connect NAME"));
    assert!(t.contains("USAGE: tablepro [--color truecolor|256|16|none] [--connect NAME]"));
}

#[test]
fn bin_help_prints_source_usage() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_tablepro"))
        .arg("--help")
        .output()
        .expect("tablepro --help");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let t = String::from_utf8_lossy(&out.stdout);
    assert!(
        t.contains("Keys: Ctrl+O open quickly · Ctrl+T new query · Ctrl+R run · Ctrl+Y history")
    );
    assert!(t.contains("--connect NAME"));
}

#[test]
fn unknown_connection_is_named_error() {
    let mut app = App::new(ColorCapability::Truecolor);
    let err = app.connect_named("Nope").unwrap_err();
    assert!(err.contains("no connection named"), "{err}");
}

#[test]
fn connections_include_production() {
    assert!(
        connections().iter().any(|c| c.name == "Production"),
        "seed catalog must include Production"
    );
}

#[test]
fn connections_screen_lists_saved() {
    let h = Harness::new(120, 40);
    assert_eq!(h.app.screen, Screen::Connections);
    let t = h.text();
    assert!(t.contains("Local PostgreSQL"), "{t}");
    assert!(t.contains("Production"), "{t}");
    assert!(t.contains("Connections"), "{t}");
    assert!(!t.contains("organizations") || t.contains("saved"), "{t}");
}

#[test]
fn connect_production_is_workbench_not_list() {
    let h = Harness::connected(120, 40);
    assert_eq!(h.app.screen, Screen::Workbench);
    let name = h
        .app
        .workbench
        .as_ref()
        .expect("workbench after connect")
        .connection
        .name
        .as_str();
    assert_eq!(name, "Production");
    let t = h.text();
    assert!(t.contains("production") || t.contains("Production"), "{t}");
    assert!(t.contains("TablePro"), "{t}");
    assert!(
        !t.contains("Local PostgreSQL"),
        "connections list must not remain after --connect Production:\n{t}"
    );
    assert!(
        t.contains("Query") || t.contains("explorer") || t.contains("public"),
        "workbench chrome missing:\n{t}"
    );
}

#[test]
fn connect_named_is_case_insensitive() {
    let mut app = App::new(ColorCapability::Truecolor);
    app.connect_named("production").unwrap();
    assert_eq!(app.screen, Screen::Workbench);
    assert_eq!(
        app.workbench.as_ref().unwrap().connection.name,
        "Production"
    );
}

#[test]
fn new_query_and_history_tabs() {
    let mut h = Harness::connected(120, 40);
    h.app.workbench.as_mut().unwrap().new_query("select 1");
    h.draw();
    assert!(matches!(
        h.app.workbench.as_ref().unwrap().active_tab(),
        Some(WorkTab::Query(_))
    ));
    h.app.workbench.as_mut().unwrap().open_history();
    h.draw();
    assert!(matches!(
        h.app.workbench.as_ref().unwrap().active_tab(),
        Some(WorkTab::History(_))
    ));
    let t = h.text();
    assert!(t.contains("History") || t.contains("Query"), "{t}");
}

#[test]
fn quick_switcher_matches_source_query_clear_and_navigation() {
    let mut h = Harness::connected(120, 40);

    h.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(h.text().contains("Open quickly"), "{}", h.text());
    assert!(h.text().contains("All · Tab scope"), "{}", h.text());

    h.type_text("cust");
    assert!(h.text().contains("customers"), "{}", h.text());
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(
        h.app.workbench.as_ref().unwrap().active_tab(),
        Some(WorkTab::Table(t)) if t.name == "customers"
    ));

    h.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    h.type_text("x");
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(
        h.text().contains("Filter tables, tabs, queries"),
        "{}",
        h.text()
    );
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.text().contains("Open quickly"), "{}", h.text());

    h.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    h.type_text("ord");
    h.key(KeyCode::Down, KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    let active = h
        .app
        .workbench
        .as_ref()
        .unwrap()
        .active_tab()
        .map(WorkTab::label);
    assert_eq!(
        active.as_deref(),
        Some("orders"),
        "active={active:?}\n{}",
        h.text()
    );
}

#[test]
fn quick_switcher_tab_cycles_source_scope() {
    let mut h = Harness::connected(120, 40);
    h.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    h.key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(h.text().contains("Tables · Tab scope"), "{}", h.text());
    h.type_text("cust");
    assert!(h.text().contains("customers"), "{}", h.text());
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    h.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(h.text().contains("All · Tab scope"), "{}", h.text());
}

#[test]
fn catalog_applications_mounts_same_workbench() {
    let mut app = CatalogApp::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
    app.goto(PageId::TABLEPRO);
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let t = tick();
    term.draw(|f| app.render(f, t)).unwrap();
    let buf = term.backend().buffer();
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    assert!(s.contains("TablePro"), "{s}");
    assert!(
        s.contains("Applications") || s.contains("Connections"),
        "{s}"
    );
    assert!(
        s.contains("Production") || s.contains("Local PostgreSQL"),
        "{s}"
    );
}

#[test]
fn termrock_profile_shows_extensions_and_identity() {
    let mut app = CatalogApp::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let t = tick();
    term.draw(|f| app.render(f, t)).unwrap();
    let buf = term.backend().buffer();
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    assert!(s.contains("TermRock"), "default catalog identity:\n{s}");
    assert!(
        s.contains("TablePro"),
        "Applications after source prefix:\n{s}"
    );
    assert!(s.contains("Applications"), "{s}");
}

#[test]
fn safety_acknowledgement_renders_facts_and_refuses_wrong_token() {
    let mut h = Harness::connected(120, 40);
    h.app.seed_active_query("DELETE FROM orders");
    h.key(KeyCode::Char('r'), KeyModifiers::CONTROL);

    let dialog = h.text();
    for fact in [
        "Action",
        "Target",
        "Scope",
        "Risk",
        "Reversible",
        "Safe Mode",
        "DELETE FROM orders",
        "Type orders to confirm",
    ] {
        assert!(
            dialog.contains(fact),
            "missing {fact:?} in dialog:\n{dialog}"
        );
    }
    assert!(!h.app.workbench.as_ref().unwrap().running().is_some());

    // Enter starts the acknowledgement editor; it must not activate Execute.
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.app.workbench.as_ref().unwrap().running().is_none());
    h.paste("wrong");
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        h.text().contains("Type orders to confirm"),
        "wrong token must leave the dialog open:\n{}",
        h.text()
    );
    assert!(h.app.workbench.as_ref().unwrap().running().is_none());
}

#[test]
fn safety_acknowledgement_exact_token_executes_query() {
    let mut h = Harness::connected(120, 40);
    h.app
        .seed_active_query("UPDATE orders SET status = 'paid' WHERE id = 'x'");
    h.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(h.text().contains("Type orders to confirm"));

    h.key(KeyCode::Enter, KeyModifiers::NONE);
    for character in ['o', 'r', 'd', 'e', 'r', 'x'] {
        h.key(KeyCode::Char(character), KeyModifiers::NONE);
    }
    h.key(KeyCode::Backspace, KeyModifiers::NONE);
    h.key(KeyCode::Char('s'), KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    h.key(KeyCode::Right, KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);

    assert!(
        h.app.workbench.as_ref().unwrap().running().is_some(),
        "exact target token must start the query:\n{}",
        h.text()
    );
    assert!(!h.text().contains("Type orders to confirm"));
}

#[test]
fn render_does_not_advance_query_ticks() {
    let mut h = Harness::connected(120, 40);
    h.app.seed_active_query("SELECT * FROM orders");
    h.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(h.app.workbench.as_ref().unwrap().running().is_some());

    h.draw();
    assert!(
        h.app.workbench.as_ref().unwrap().running().is_some(),
        "render must not consume a query tick"
    );

    h.app.on_tick(tick());
    h.draw();
    assert!(h.app.workbench.as_ref().unwrap().running().is_some());
    h.app.on_tick(tick());
    h.draw();
    assert!(h.app.workbench.as_ref().unwrap().running().is_none());
}

#[test]
fn ctrl_c_cancels_running_query_before_quit() {
    let mut h = Harness::connected(120, 40);
    h.app.seed_active_query("SELECT * FROM orders");
    h.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(h.app.workbench.as_ref().unwrap().running().is_some());

    let flow = h.app.handle_event(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        tick(),
    );
    assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
    assert!(!h.app.quit);
    assert!(h.app.workbench.as_ref().unwrap().running().is_none());

    let flow = h.app.handle_event(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        tick(),
    );
    assert!(matches!(flow, std::ops::ControlFlow::Break(())));
    assert!(h.app.quit);
}

#[test]
fn dirty_quit_requires_confirmation() {
    let mut h = Harness::connected(120, 40);
    let query = h
        .app
        .workbench
        .as_mut()
        .unwrap()
        .active_query_mut()
        .unwrap();
    query.editor.set_editing(true);
    query
        .editor
        .set_text("SELECT * FROM orders WHERE status = 'pending'");
    query.editor.set_editing(false);
    assert!(query.dirty());

    h.key(KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(!h.app.quit);
    assert!(h.text().contains("Quit TablePro?"), "{}", h.text());
    assert!(h.text().contains("unsaved query"), "{}", h.text());

    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.quit);
    h.key(KeyCode::Char('q'), KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.app.quit);
}

#[test]
fn dirty_close_tab_requires_confirmation() {
    let mut h = Harness::connected(120, 40);
    {
        let wb = h.app.workbench.as_mut().unwrap();
        wb.new_query("SELECT 1");
        wb.new_query("SELECT 2");
        let query = wb.active_query_mut().unwrap();
        query.editor.set_editing(true);
        query.editor.set_text("SELECT 2 -- changed");
        query.editor.set_editing(false);
    }
    let tab_count = h.app.workbench.as_ref().unwrap().tabs.len();

    h.key(KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(h.app.workbench.as_ref().unwrap().tabs.len(), tab_count);
    assert!(
        h.text().contains("Close tab with unsaved work?"),
        "{}",
        h.text()
    );

    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.workbench.as_ref().unwrap().tabs.len(), tab_count - 1);
}

#[test]
fn close_tab_before_active_tab_preserves_active_tab() {
    let connection = connections().into_iter().next().unwrap();
    let mut wb = Workbench::new(connection, Catalog::acme_prod());
    wb.new_query("SELECT 1");
    wb.new_query("SELECT 2");
    wb.new_query("SELECT 3");
    wb.active = 2;

    wb.close_tab(0);

    assert_eq!(wb.active, 1);
    assert_eq!(wb.active_tab().unwrap().label(), "Query 3");
}

#[test]
fn deleting_connection_requires_confirmation() {
    let mut h = Harness::new(120, 40);
    let count = h.app.connections.connections.len();
    // Initial focus is the tree. The connection detail buttons follow it in
    // the focus ring: Connect, Edit, Duplicate, Delete.
    for _ in 0..4 {
        h.key(KeyCode::Tab, KeyModifiers::NONE);
    }
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.connections.connections.len(), count);
    assert!(h.text().contains("Delete connection?"), "{}", h.text());

    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.connections.connections.len(), count - 1);
}
