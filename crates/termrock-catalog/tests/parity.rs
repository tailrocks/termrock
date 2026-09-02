// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Fail-on-first cell-grid compare vs current source TestBackend frames
//! (SHA e43cf670). Committed `shots/` are older than that SHA's 20-page nav;
//! headless source frames are the live executable goldens.

use std::path::PathBuf;

use termrock_catalog::capture;
use termrock_catalog::catalog::{CatalogProfile, SOURCE_NAV};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify/junie/source-headless")
}

fn file_stem(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn first_mismatch(ours: &str, src: &str) -> Option<(u16, u16, char, char)> {
    let ol: Vec<&str> = ours.split_inclusive('\n').collect();
    let sl: Vec<&str> = src.split_inclusive('\n').collect();
    let rows = ol.len().max(sl.len());
    for y in 0..rows {
        let a = ol.get(y).copied().unwrap_or("");
        let b = sl.get(y).copied().unwrap_or("");
        let cols = a.chars().count().max(b.chars().count());
        let ac: Vec<char> = a.chars().collect();
        let bc: Vec<char> = b.chars().collect();
        for x in 0..cols {
            let ca = *ac.get(x).unwrap_or(&' ');
            let cb = *bc.get(x).unwrap_or(&' ');
            if ca != cb {
                return Some((x as u16, y as u16, cb, ca));
            }
        }
    }
    None
}

#[test]
fn junie_reference_120x40_matches_source_headless_fail_first() {
    let dir = golden_dir();
    for e in SOURCE_NAV {
        let stem = file_stem(e.label);
        let golden = dir.join(format!("{stem}_120x40.txt"));
        let src = std::fs::read_to_string(&golden)
            .unwrap_or_else(|err| panic!("missing source golden {}: {err}", golden.display()));
        let art = capture::catalog_page(CatalogProfile::JunieReference, e.id, 120, 40);
        if let Some((x, y, expected, actual)) = first_mismatch(&art.txt(), &src) {
            panic!(
                "first cell mismatch page {} at col {x} row {y}: expected {expected:?} actual {actual:?}\nours L{y}: {}\nsrc  L{y}: {}",
                e.label,
                art.txt().lines().nth(usize::from(y)).unwrap_or(""),
                src.lines().nth(usize::from(y)).unwrap_or(""),
            );
        }
    }
}
