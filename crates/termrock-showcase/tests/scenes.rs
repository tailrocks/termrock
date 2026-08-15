// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The showcase obeys the same design law as the library.
//!
//! A flagship that fails the gates the library enforces would be evidence
//! against the design, not for it. These are the same assertions
//! `design_gate.rs` makes about widgets, applied to whole application frames
//! (plans/019).

use std::process::Command;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use termrock::runtime::{FrameTick, Instant};
use termrock::style::Role;

use termrock_showcase::app::App;
use termrock_showcase::demo_runtime::Scenario;

fn tick(ms: u64) -> FrameTick {
    FrameTick::manual(
        Instant::now(),
        std::time::Duration::from_millis(ms),
        std::time::Duration::from_millis(16),
    )
}

/// One painted frame of `scenario`, advanced `steps` ticks in.
fn frame(scenario: Scenario, area: Rect, steps: u64) -> Buffer {
    let mut app = App::new();
    app.scenario = scenario;
    app.submit("show me".into(), 0);
    for step in 0..steps {
        app.pump(tick(step * 25));
    }
    let mut buffer = Buffer::empty(area);
    app.render(&mut buffer, area);
    buffer
}

/// Whether a glyph is chrome rather than content.
///
/// In the phosphor palette `Role::Accent` and `Role::BorderFocused` are the
/// same green, so a border would otherwise read as an accent region. Frames
/// are measured by the border gate; this one measures *content*.
fn is_border_glyph(symbol: &str) -> bool {
    symbol.chars().all(|c| {
        matches!(
            c,
            '─' | '│'
                | '┌'
                | '┐'
                | '└'
                | '┘'
                | '├'
                | '┤'
                | '┬'
                | '┴'
                | '┼'
                | '━'
                | '┃'
                | '╭'
                | '╮'
                | '╰'
                | '╯'
                | '-'
                | '|'
                | '+'
        )
    })
}

#[test]
fn every_scene_spends_the_accent_on_few_rows() {
    let system = termrock::style::DesignSystem::from_palette(
        termrock::style::RolePalette::tailrocks_phosphor(),
    );
    let accent = system.style(Role::Accent).fg;
    let area = Rect::new(0, 0, 120, 32);
    for scenario in Scenario::ALL {
        let buffer = frame(scenario, area, 40);
        // Rows, not cells or runs: a live thing occupies a row (a selected
        // step, a chip). Counting cells punishes long labels; counting runs
        // splits one label at every space.
        let accent_rows: Vec<u16> = (0..area.height)
            .filter(|y| {
                (0..area.width).any(|x| {
                    let cell = &buffer[(x, *y)];
                    Some(cell.fg) == accent
                        && !cell.symbol().trim().is_empty()
                        && !is_border_glyph(cell.symbol())
                })
            })
            .collect();
        assert!(
            accent_rows.len() <= 3,
            "{} paints the accent on {} rows ({accent_rows:?}); it marks what is \
             live, not what exists",
            scenario.id(),
            accent_rows.len()
        );
    }
}

#[test]
fn every_scene_has_at_most_one_focused_border() {
    let system = termrock::style::DesignSystem::from_palette(
        termrock::style::RolePalette::tailrocks_phosphor(),
    );
    let focused = system.style(Role::BorderFocused).fg;
    let area = Rect::new(0, 0, 120, 32);
    for scenario in Scenario::ALL {
        let buffer = frame(scenario, area, 40);
        // A frame shows as two long horizontal runs (its top and bottom
        // edges); vertical edges are one cell wide and are not counted. Two
        // long runs = one focused container.
        let mut long_runs = 0usize;
        for y in 0..area.height {
            let mut run = 0usize;
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                let is_edge = Some(cell.fg) == focused && is_border_glyph(cell.symbol());
                if is_edge {
                    run += 1;
                } else {
                    if run >= 8 {
                        long_runs += 1;
                    }
                    run = 0;
                }
            }
            if run >= 8 {
                long_runs += 1;
            }
        }
        assert!(
            long_runs <= 2,
            "{} paints {long_runs} focused frame edges; only the container that \
             owns the keyboard may claim one",
            scenario.id()
        );
    }
}

#[test]
fn every_scene_paints_at_every_size_the_law_names() {
    for (w, h) in [(120, 32), (80, 24), (40, 16), (20, 5)] {
        for scenario in Scenario::ALL {
            let area = Rect::new(0, 0, w, h);
            let buffer = frame(scenario, area, 20);
            assert!(
                buffer
                    .content()
                    .iter()
                    .any(|cell| !cell.symbol().trim().is_empty()),
                "{} painted nothing at {w}x{h}",
                scenario.id()
            );
        }
    }
}

#[test]
fn the_showcase_imports_only_public_termrock() {
    let source_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(source_dir).expect("read showcase src") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let body = std::fs::read_to_string(&path).expect("read source");
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            // The banned surfaces from SKD-5, and any reach past the public API.
            for needle in [
                "ApprovalCard",
                "PromptBox",
                "pub(crate) use termrock",
                "doc(hidden)",
            ] {
                if trimmed.contains(needle) {
                    offenders.push(format!("{}:{}: {needle}", path.display(), i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the showcase must compose public TermRock only:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_readme_documents_a_run_command_that_exists() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("showcase README");
    assert!(
        readme.contains("cargo run -p termrock-showcase"),
        "the README has to say how to run it"
    );
    // The package name in the README is the package cargo knows.
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .expect("cargo metadata");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("termrock-showcase"),
        "the crate is a workspace member"
    );
}
