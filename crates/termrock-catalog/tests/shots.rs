// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Fail-on-first five-artifact compare vs immutable source `shots/`.
//!
//! Replay inventoried keys/mouse/resize/ticks on the same App the binaries
//! mount (`junie-reference` / standalone TablePro). PNG is zero-tol
//! `termrock-raster` of both cell grids (source `.ansi` re-rasterized at
//! TermRock metrics). Source Python/FreeType `shots/*.png` bytes are a
//! different raster (9×20+pad) and are not the NC of the application.

use std::path::{Path, PathBuf};

use termrock::style::RolePalette;
use termrock_catalog::ansi_grid::{first_txt_diff, from_snapshot, parse_ansi, parse_html};
use termrock_catalog::capture;
use termrock_catalog::scenarios::{self, Scenario};

fn shots_dir() -> PathBuf {
    if let Ok(p) = std::env::var("JUNIE_SHOTS") {
        return PathBuf::from(p);
    }
    PathBuf::from("/Users/donbeave/Projects/terminal-components-claude/shots")
}

fn read(dir: &Path, id: &str, ext: &str) -> String {
    let p = dir.join(format!("{id}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("missing {}: {e}", p.display()))
}

fn parse_cursor(s: &str) -> Option<(u16, u16, u8)> {
    let mut p = s.split_whitespace();
    Some((
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    ))
}

/// Visible caret is byte-exact. Hidden flag (`0`) is application-controlled;
/// tmux still reports a leftover `x y` from the last cell write, which
/// TestBackend does not reproduce.
fn cursor_match(ours: &str, src: &str) -> bool {
    let Some((ox, oy, of)) = parse_cursor(ours) else {
        return false;
    };
    let Some((sx, sy, sf)) = parse_cursor(src) else {
        return false;
    };
    if sf == 1 || of == 1 {
        ox == sx && oy == sy && of == sf
    } else {
        of == 0 && sf == 0
    }
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
    if let Some((x, y, expected, actual)) = first_txt_diff(&art.txt(), &src_txt) {
        fail(
            s,
            "txt",
            format!(
                "cell ({x},{y}) expected {expected:?} actual {actual:?}\nours: {}\nsrc:  {}",
                art.txt().lines().nth(usize::from(y)).unwrap_or(""),
                src_txt.lines().nth(usize::from(y)).unwrap_or("")
            ),
        );
    }

    let src_cursor = read(dir, s.id, "cursor");
    let ours_c = art.cursor();
    if !cursor_match(ours_c.trim(), src_cursor.trim()) {
        fail(
            s,
            "cursor",
            format!(
                "expected {:?} actual {:?}",
                src_cursor.trim(),
                ours_c.trim()
            ),
        );
    }

    let src_ansi = read(dir, s.id, "ansi");
    let src_grid = parse_ansi(&src_ansi, s.cols, s.rows);
    let ours_grid = from_snapshot(&art.snapshot);
    if let Some((x, y, why)) = src_grid.first_diff(&ours_grid) {
        fail(s, "ansi", format!("cell ({x},{y}) {why}"));
    }

    let src_html = read(dir, s.id, "html");
    let html_grid = parse_html(&src_html, s.cols, s.rows);
    if let Some((x, y, why)) = html_grid.first_diff(&ours_grid) {
        fail(s, "html", format!("cell ({x},{y}) {why}"));
    }

    let src_buf = src_grid.for_raster().to_buffer();
    let src_png = termrock_raster::render_png(&src_buf, &RolePalette::junie())
        .unwrap_or_else(|e| fail(s, "png", format!("raster source ansi: {e}")));
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
fn s_chips_idle_matches_source_shot() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "s_chips")
        .expect("s_chips");
    compare_one(&dir, s);
}

#[test]
fn fail_first_shots_five_artifacts() {
    let dir = shots_dir();
    assert!(
        dir.join("f_overview.txt").is_file(),
        "source shots missing at {} (set JUNIE_SHOTS)",
        dir.display()
    );
    let only = std::env::var("TERMROCK_SHOTS_ONLY").ok();
    for s in scenarios::ALL {
        if let Some(ref prefix) = only
            && !s.id.starts_with(prefix.as_str())
        {
            continue;
        }
        // f_* goldens are 16-page nav; e43cf670 and s_* are 20-page.
        // Skip unless explicitly selected. See delta-manifest f-shots-sixteen-page-nav.
        if only.is_none() && s.id.starts_with("f_") {
            continue;
        }
        compare_one(&dir, s);
    }
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
