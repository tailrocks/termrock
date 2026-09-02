// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/app_tests.rs (MIT).

//! Drive the shipped catalog App through TestBackend.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use termrock::input::{Event, KeyCode, KeyEvent, KeyModifiers};
use termrock::runtime::FrameTick;
use termrock::style::ColorCapability;
use termrock_catalog::catalog::{CatalogProfile, PageId, SOURCE_NAV};
use termrock_catalog::cli::parse_args;
use termrock_catalog::shell::{App, MIN_HEIGHT, MIN_WIDTH};

struct Harness {
    app: App,
    term: Terminal<TestBackend>,
}

impl Harness {
    fn new(w: u16, h: u16, page: Option<PageId>) -> Self {
        let mut app = App::new(CatalogProfile::JunieReference, ColorCapability::Truecolor);
        if let Some(p) = page {
            app.goto(p);
        }
        let term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut h = Self { app, term };
        h.draw();
        h
    }

    fn draw(&mut self) {
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(0),
            std::time::Duration::from_millis(16),
        );
        self.term.draw(|f| self.app.render(f, tick)).unwrap();
    }

    fn key(&mut self, code: KeyCode) {
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(0),
            std::time::Duration::from_millis(16),
        );
        let _ = self
            .app
            .handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)), tick);
        self.draw();
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
fn launches_overview() {
    let h = Harness::new(120, 40, None);
    let t = h.text();
    assert!(t.contains("Junie"), "junie-reference identity: {t}");
    assert!(t.contains("Overview"));
    assert!(t.contains("Tokens"));
    assert!(t.contains("Foundations"));
    assert!(t.contains("Buttons"));
    assert!(!h.app.quit);
    if let Ok(path) = std::env::var("TERMROCK_DUMP_OVERVIEW") {
        std::fs::write(&path, &t).expect("write overview dump");
    }
}

#[test]
fn source_prefix_visible_in_order() {
    let h = Harness::new(120, 40, None);
    let t = h.text();
    let mut last = 0;
    for label in SOURCE_NAV.iter().map(|e| e.label) {
        let pos = t
            .find(label)
            .unwrap_or_else(|| panic!("missing {label} in:\n{t}"));
        assert!(pos >= last, "{label} out of order");
        last = pos;
    }
}

#[test]
fn default_page_is_overview() {
    let h = Harness::new(120, 40, None);
    assert_eq!(h.app.page, PageId::OVERVIEW);
}

#[test]
fn termrock_default_identity_and_tablepro_nav() {
    let mut app = App::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let tick = FrameTick::manual(
        termrock::runtime::Instant::now(),
        std::time::Duration::from_millis(0),
        std::time::Duration::from_millis(16),
    );
    term.draw(|f| app.render(f, tick)).unwrap();
    let buf = term.backend().buffer();
    let mut t = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            t.push_str(buf[(x, y)].symbol());
        }
        t.push('\n');
    }
    assert!(t.contains("TermRock"), "{t}");
    assert!(t.contains("TablePro"), "{t}");
    app.goto(PageId::TABLEPRO);
    term.draw(|f| app.render(f, tick)).unwrap();
    let buf = term.backend().buffer();
    t.clear();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            t.push_str(buf[(x, y)].symbol());
        }
        t.push('\n');
    }
    assert!(t.contains("Connections") || t.contains("Production"), "{t}");
}

#[test]
fn dialogs_page_title() {
    let h = Harness::new(120, 40, Some(PageId::DIALOGS));
    let t = h.text();
    assert!(t.contains("Dialogs"), "{t}");
    assert!(t.contains("Focus is trapped") || t.contains("Esc"), "{t}");
}

#[test]
fn panels_page_title() {
    let h = Harness::new(120, 40, Some(PageId::PANELS));
    let t = h.text();
    assert!(t.contains("Panels"), "{t}");
    assert!(
        t.contains("card") || t.contains("Card") || t.contains("surface"),
        "{t}"
    );
}

#[test]
fn tables_page_has_source_tasks() {
    let h = Harness::new(120, 40, Some(PageId::TABLES));
    let t = h.text();
    assert!(t.contains("Task"), "{t}");
    assert!(t.contains("rate limiting") || t.contains("mira"), "{t}");
    assert!(t.contains("No checks have run yet"), "{t}");
}

#[test]
fn lists_page_has_source_data() {
    let h = Harness::new(120, 40, Some(PageId::LISTS));
    let t = h.text();
    assert!(t.contains("Language"), "{t}");
    assert!(t.contains("Rust"), "{t}");
    assert!(t.contains("No results for"), "{t}");
}

#[test]
fn inputs_page_has_source_fields() {
    let h = Harness::new(120, 40, Some(PageId::INPUTS));
    let t = h.text();
    assert!(t.contains("Project name"), "{t}");
    assert!(t.contains("payments-gateway"), "{t}");
    assert!(t.contains("Owner email"), "{t}");
    assert!(t.contains("State reference"), "{t}");
}

#[test]
fn page_flag_datagrid() {
    let h = Harness::new(120, 40, Some(PageId::GRID));
    let t = h.text();
    assert!(t.contains("Data grid"), "{t}");
}

#[test]
fn every_source_page_renders() {
    for e in SOURCE_NAV {
        for (w, h) in [(72, 20), (80, 24), (120, 40)] {
            let harness = Harness::new(w, h, Some(e.id));
            let t = harness.text();
            assert!(
                t.contains(e.label)
                    || t.contains(harness.app.page.index(SOURCE_NAV).to_string().as_str()),
                "page {} at {w}x{h} empty/missing label. got:\n{t}",
                e.label
            );
            assert!(!t.trim().is_empty());
        }
    }
}

#[test]
fn too_small_state() {
    let h = Harness::new(MIN_WIDTH - 1, MIN_HEIGHT - 1, None);
    let t = h.text();
    assert!(
        t.contains("too small") || t.contains("Too small") || t.contains("Terminal too small"),
        "{t}"
    );
    assert!(
        t.contains(&format!("{MIN_WIDTH}×{MIN_HEIGHT}")) || t.contains("72"),
        "{t}"
    );
}

#[test]
fn help_key_opens_dialog() {
    let mut h = Harness::new(120, 40, None);
    h.key(KeyCode::Char('?'));
    let t = h.text();
    assert!(
        t.contains("Keyboard & mouse") || t.contains("Tab / Shift+Tab"),
        "{t}"
    );
}

#[test]
fn cli_aliases_match_source() {
    let o = parse_args(["--page", "chipsselects"]).unwrap();
    assert_eq!(o.page, Some(PageId::CHIPS));
    let o = parse_args(["--color", "24bit"]).unwrap();
    assert_eq!(o.level, ColorCapability::Truecolor);
    let o = parse_args(["-c", "mono"]).unwrap();
    assert_eq!(o.level, ColorCapability::Monochrome);
}

/// junie-reference: every source page is live and contains source copy.
#[test]
fn junie_reference_source_pages_have_source_copy() {
    let cases: &[(PageId, &[&str])] = &[
        (PageId::OVERVIEW, &["Overview", "Tokens", "One hue"]),
        (PageId::BUTTONS, &["Buttons", "Run task", "State matrix"]),
        (
            PageId::INPUTS,
            &["Inputs", "Project name", "payments-gateway"],
        ),
        (PageId::TEXT_AREAS, &["Text areas"]),
        (PageId::FORMS, &["Forms", "New task", "Create task"]),
        (PageId::LISTS, &["Lists", "Language", "Rust"]),
        (PageId::TREES, &["Trees", "Project", "Cargo.toml"]),
        (PageId::TABLES, &["Tables", "No checks have run yet"]),
        (PageId::EDITABLE, &["Editable tables"]),
        (PageId::PANELS, &["Panels"]),
        (PageId::SIDEBARS, &["Sidebars"]),
        (PageId::DIALOGS, &["Dialogs"]),
        (PageId::PROGRESS, &["Progress"]),
        (PageId::SCROLLING, &["Scrolling"]),
        (PageId::EDITOR, &["Code editor", "fetch"]),
        (PageId::GRID, &["Data grid"]),
        (PageId::CHIPS, &["Chips & selects"]),
        (PageId::PICKERS, &["Pickers"]),
        (PageId::SETTINGS, &["Project settings"]),
        (
            PageId::TASK_RUNNER,
            &["Task runner", "Run pipeline", "compile"],
        ),
    ];
    for (id, needles) in cases {
        let h = Harness::new(120, 40, Some(*id));
        let t = h.text();
        for needle in *needles {
            assert!(
                t.contains(needle),
                "junie-reference page {:?} missing {needle:?}\n{t}",
                id
            );
        }
        assert!(
            !t.contains("Activate") || t.contains(needles[0]),
            "page {:?} still a stub\n{t}",
            id
        );
    }
    let h = Harness::new(120, 40, None);
    let t = h.text();
    assert!(
        !t.contains("TablePro"),
        "junie-reference must hide Applications/TablePro:\n{t}"
    );
    if let Ok(dir) = std::env::var("TERMROCK_DUMP_PAGES") {
        let _ = std::fs::create_dir_all(&dir);
        for (id, _) in cases {
            let h = Harness::new(120, 40, Some(*id));
            let name = format!("{id:?}");
            std::fs::write(format!("{dir}/{name}.txt"), h.text()).expect("dump page");
        }
    }
}
