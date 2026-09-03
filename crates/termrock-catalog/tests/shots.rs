// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Inventoried source `shots/` helpers and the chips MATCH ratchet.
//!
//! The checked-in reference directory mirrors the canonical source `shots/`
//! artifact set. `JUNIE_SHOTS` may point at an independently fetched source
//! checkout when refreshing evidence.

use std::path::{Path, PathBuf};

use termrock_catalog::ansi_grid::{first_txt_diff, from_snapshot, parse_ansi, parse_html};
use termrock_catalog::capture;
use termrock_catalog::scenarios::{self, Scenario};

fn shots_dir() -> PathBuf {
    if let Ok(p) = std::env::var("JUNIE_SHOTS") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify/junie/reference/scenes")
}

fn read(dir: &Path, id: &str, ext: &str) -> String {
    let p = dir.join(format!("{id}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("missing {}: {e}", p.display()))
}

fn fail(s: &Scenario, kind: &str, msg: String) -> ! {
    panic!(
        "first mismatch scenario {} {} {}x{}: {msg}",
        s.id, kind, s.cols, s.rows
    );
}

fn compare_one(dir: &Path, s: &Scenario) {
    let art = capture::replay(s);

    let src_txt = read(dir, s.id, "txt");
    if art.txt().as_bytes() != src_txt.as_bytes() {
        let detail = first_txt_diff(&art.txt(), &src_txt)
            .map(|(x, y, expected, actual)| {
                format!(
                    "first visible difference at ({x},{y}): {expected:?} != {actual:?}; ours={:?}; source={:?}",
                    art.txt().lines().nth(usize::from(y)).unwrap_or(""),
                    src_txt.lines().nth(usize::from(y)).unwrap_or("")
                )
            })
            .unwrap_or_else(|| "line endings or trailing cells differ".to_owned());
        fail(s, "txt", format!("byte-exact mismatch: {detail}"));
    }

    let src_cursor = read(dir, s.id, "cursor");
    let ours_c = art.cursor();
    if ours_c.as_bytes() != src_cursor.as_bytes() {
        fail(
            s,
            "cursor",
            format!("expected {:?} actual {:?}", src_cursor, ours_c),
        );
    }

    let src_ansi = read(dir, s.id, "ansi");
    let src_grid = parse_ansi(&src_ansi, s.cols, s.rows);
    let ours_grid = from_snapshot(&art.snapshot);
    if let Some((x, y, why)) = src_grid.first_strict_diff(&ours_grid) {
        fail(s, "ansi", format!("cell ({x},{y}) {why}"));
    }

    let src_html = read(dir, s.id, "html");
    let html_grid = parse_html(&src_html, s.cols, s.rows);
    if let Some((x, y, why)) = html_grid.first_strict_diff(&ours_grid) {
        fail(s, "html", format!("cell ({x},{y}) {why}"));
    }

    let src_png = std::fs::read(dir.join(format!("{}.png", s.id)))
        .unwrap_or_else(|e| fail(s, "png", format!("read source PNG: {e}")));
    let ours_png = art
        .png()
        .unwrap_or_else(|e| fail(s, "png", format!("raster ours: {e}")));
    if let Err(diff) = termrock_raster::compare_png_pixels(&src_png, &ours_png) {
        fail(s, "png", diff.to_string());
    }
}

#[test]
fn inventoried_count_is_sixty_three() {
    assert_eq!(scenarios::ALL.len(), 63);
    let dir = shots_dir();
    assert!(
        dir.join("f_overview.txt").is_file(),
        "source shots missing at {} (set JUNIE_SHOTS)",
        dir.display()
    );
    for s in scenarios::ALL {
        for ext in ["txt", "cursor", "ansi", "html", "png"] {
            let p = dir.join(format!("{}.{ext}", s.id));
            assert!(p.is_file(), "missing {}", p.display());
        }
    }
}

#[test]
fn s_chips_idle_cell_and_cursor_match_source_shot() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "s_chips")
        .expect("s_chips");
    let art = capture::replay(s);
    let src_txt = read(&dir, s.id, "txt");
    assert_eq!(
        art.txt().as_bytes(),
        src_txt.as_bytes(),
        "source text drifted"
    );
    let src_cursor = read(&dir, s.id, "cursor");
    assert_eq!(
        art.cursor().as_bytes(),
        src_cursor.as_bytes(),
        "source cursor drifted"
    );
    let src = parse_ansi(&read(&dir, s.id, "ansi"), s.cols, s.rows);
    let got = from_snapshot(&art.snapshot);
    assert!(
        src.first_strict_diff(&got).is_none(),
        "source ANSI grid drifted"
    );
}

#[test]
fn opt_in_shots_five_artifacts_against_stale_captures() {
    let Some(prefix) = std::env::var("TERMROCK_SHOTS_ONLY").ok() else {
        // Default `cargo nextest` stays green. Live catalog goldens are
        // source-headless (`parity.rs`). `shots/` is the stale ratchet.
        return;
    };
    let dir = shots_dir();
    assert!(
        dir.join("f_overview.txt").is_file(),
        "source shots missing at {} (set JUNIE_SHOTS)",
        dir.display()
    );
    let mut ran = 0usize;
    for s in scenarios::ALL {
        if !s.id.starts_with(prefix.as_str()) {
            continue;
        }
        compare_one(&dir, s);
        ran += 1;
    }
    assert!(
        ran > 0,
        "TERMROCK_SHOTS_ONLY={prefix} matched no inventoried shots"
    );
}

#[test]
fn f_80x24_taskrunner_stale_nav_is_settings_versus_code_editor() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "f_80x24_taskrunner")
        .expect("f_80x24_taskrunner");
    let art = capture::replay(s);
    let src = std::fs::read_to_string(dir.join("f_80x24_taskrunner.txt")).expect("src txt");
    let Some((x, y, expected, actual)) = first_txt_diff(&art.txt(), &src) else {
        panic!("expected stale 16-page nav diff; grids matched");
    };
    assert_eq!((x, y), (3, 16), "first stale-nav cell");
    assert_eq!(expected, 'S');
    assert_eq!(actual, 'C');
}
