// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Machine-verifiable public visual-component catalog coverage.

use std::collections::BTreeSet;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use termrock::registry::public_ui_inventory;
use termrock::runtime::FrameTick;
use termrock::style::ColorCapability;
use termrock_catalog::catalog::{CatalogProfile, PageId, nav_entries};
use termrock_catalog::coverage::{BUCKETS, catalog_page_for, extras_on};
use termrock_catalog::host::catalog;
use termrock_catalog::shell::App;

fn draw(app: &mut App, cols: u16, rows: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
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
    t
}

#[test]
fn every_public_visual_maps_to_exactly_one_catalog_page() {
    let mut seen = BTreeSet::new();
    for &(page, ids) in BUCKETS {
        for id in ids {
            assert!(seen.insert(*id), "{id} mapped twice (second page {page:?})");
        }
    }
    for entry in public_ui_inventory() {
        assert!(
            seen.contains(&entry.id),
            "public UI {} has no catalog page",
            entry.id
        );
        let page = catalog_page_for(entry.id);
        let nav = nav_entries(CatalogProfile::TermRock);
        assert!(
            nav.iter().any(|e| e.id == page),
            "{} maps to {page:?} which is missing from TermRock nav",
            entry.id
        );
    }
    assert_eq!(
        seen.len(),
        public_ui_inventory().len(),
        "bucket size vs inventory"
    );
}

#[test]
fn catalog_nav_ids_are_unique() {
    let nav = nav_entries(CatalogProfile::TermRock);
    let mut ids = BTreeSet::new();
    let mut labels = BTreeSet::new();
    for e in nav {
        assert!(ids.insert(e.id.0), "duplicate page id {:?}", e.id);
        assert!(labels.insert(e.label), "duplicate label {}", e.label);
    }
}

#[test]
fn native_and_web_catalog_ids_match() {
    let nav = nav_entries(CatalogProfile::TermRock);
    let web = catalog();
    let nav_ids: Vec<String> = nav
        .iter()
        .map(|e| termrock_catalog::catalog::normalize(e.label))
        .collect();
    let web_ids: Vec<String> = web.iter().map(|d| d.id.clone()).collect();
    assert_eq!(nav_ids, web_ids, "web host catalog drifted from nav");
}

#[test]
fn junie_reference_hides_termrock_extensions() {
    let junie = nav_entries(CatalogProfile::JunieReference);
    assert!(junie.iter().all(|e| e.id.0 < 20));
    let termrock = nav_entries(CatalogProfile::TermRock);
    assert!(termrock.iter().any(|e| e.id == PageId::TABLEPRO));
    assert!(termrock.iter().any(|e| e.id == PageId::FEEDBACK));
}

#[test]
fn extras_pages_paint_owned_component_names() {
    for page in [
        PageId::FEEDBACK,
        PageId::OVERLAYS,
        PageId::CHARTS,
        PageId::STRUCTURE,
    ] {
        let mut app = App::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
        app.goto(page);
        let text = draw(&mut app, 120, 400);
        for id in extras_on(page) {
            assert!(
                text.contains(id.as_str()),
                "page {page:?} missing {} in:\n{}",
                id.as_str(),
                text.chars().take(800).collect::<String>()
            );
        }
    }
}

#[test]
fn every_mapped_page_renders() {
    let nav = nav_entries(CatalogProfile::TermRock);
    for e in nav {
        let mut app = App::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
        app.goto(e.id);
        let text = draw(&mut app, 120, 40);
        assert!(!text.trim().is_empty(), "blank page {}", e.label);
        assert!(
            text.contains(e.label) || text.contains("TermRock"),
            "page {} did not identify itself",
            e.label
        );
    }
}
