// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Five-artifact capture + junie-reference text parity vs live source frames.

use std::path::PathBuf;

use termrock_catalog::capture;
use termrock_catalog::catalog::{CatalogProfile, PageId, SOURCE_NAV};

fn source_headless_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify/junie/source-headless")
}

fn trimmed_eq(ours: &str, src: &str) -> bool {
    let a: Vec<_> = ours.lines().map(str::trim_end).collect();
    let b: Vec<_> = src.lines().map(str::trim_end).collect();
    a == b
}

#[test]
fn overview_txt_matches_source_shot() {
    let art = capture::catalog_page(CatalogProfile::JunieReference, PageId::OVERVIEW, 120, 40);
    let src = std::fs::read_to_string(source_headless_dir().join("overview_120x40.txt"))
        .expect("source overview txt");
    assert!(
        trimmed_eq(&art.txt(), &src),
        "overview 120x40 txt drifted from source shot"
    );
}

#[test]
fn buttons_txt_matches_source_shot() {
    let art = capture::catalog_page(CatalogProfile::JunieReference, PageId::BUTTONS, 120, 40);
    let src = std::fs::read_to_string(source_headless_dir().join("buttons_120x40.txt"))
        .expect("source buttons txt");
    assert!(
        trimmed_eq(&art.txt(), &src),
        "buttons 120x40 txt drifted from source shot"
    );
}

#[test]
fn five_artifacts_for_overview() {
    let dir = std::env::temp_dir().join("termrock-catalog-capture-overview");
    let _ = std::fs::remove_dir_all(&dir);
    let art = capture::catalog_page(CatalogProfile::JunieReference, PageId::OVERVIEW, 120, 40);
    art.write_five(&dir.join("f_overview"))
        .expect("write five artifacts");
    for ext in ["ansi", "cursor", "txt", "html", "png"] {
        let p = dir.join(format!("f_overview.{ext}"));
        let meta = std::fs::metadata(&p).unwrap_or_else(|_| panic!("missing {p:?}"));
        assert!(meta.len() > 0, "{p:?} empty");
    }
    let png = std::fs::read(dir.join("f_overview.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "png signature");
    let cursor = std::fs::read_to_string(dir.join("f_overview.cursor")).unwrap();
    let parts: Vec<_> = cursor.split_whitespace().collect();
    assert_eq!(parts.len(), 3, "cursor format x y flag: {cursor}");
}

#[test]
fn every_source_page_captures_five_artifacts() {
    let dir = std::env::temp_dir().join("termrock-catalog-capture-all");
    let _ = std::fs::remove_dir_all(&dir);
    for e in SOURCE_NAV {
        let art = capture::catalog_page(CatalogProfile::JunieReference, e.id, 120, 40);
        let stem = dir.join(normalize_for_file(e.label));
        art.write_five(&stem).expect("write");
        assert!(!art.txt_trimmed().trim().is_empty(), "{} empty", e.label);
        assert!(art.ansi().contains("\u{1b}["), "{} missing SGR", e.label);
        assert!(art.html().contains("<pre"), "{} missing html", e.label);
    }
}

#[test]
fn tablepro_connect_production_capture_is_workbench() {
    let art = capture::tablepro(Some("Production"), 120, 40);
    let t = art.txt();
    assert!(t.contains("Production") || t.contains("production"), "{t}");
    assert!(
        !t.contains("Local PostgreSQL"),
        "still on connections list:\n{t}"
    );
}

fn normalize_for_file(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
