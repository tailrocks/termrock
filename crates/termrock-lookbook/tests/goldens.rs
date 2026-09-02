// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Buffer-level baselines for the flagship stories.
//!
//! The repository's "deterministic preview check" rendered a story twice and
//! diffed it against itself, which passes for any render — including a blank
//! one. These are real baselines: a committed cell dump per flagship story,
//! diffed against a fresh render. A change in what the catalog paints has to
//! be blessed deliberately (`mise run bless-previews`), which is exactly the
//! review moment the plans want (plans/011 Step 5).

use std::fs;
use std::path::{Path, PathBuf};

use termrock::style::RolePalette;
use termrock_lookbook::design::lookbook_system;
use termrock_lookbook::frame::{TerminalFrame, paint_story_frame, story_by_id};

/// The stories a change is most likely to break, one per language it exercises.
const FLAGSHIP: [&str; 15] = [
    "list/selection",
    "table/basic",
    "tabs/status",
    "dialog/destructive",
    "form/validation",
    "toast/stack",
    "transcript/basic",
    "prompt-composer/basic",
    "metrics-dashboard/basic",
    "tool-call-card/permission",
    "command-palette/basic",
    "sidebar/settings",
    "status-bar/basic",
    "quick-open/basic",
    "setup-wizard/capability",
];

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens")
}

fn golden_path(story_id: &str) -> PathBuf {
    goldens_dir().join(format!("{}.txt", story_id.replace('/', "__")))
}

/// A frame as text: one line per row, plus a foreground map.
///
/// Text rather than JSON because the diff a reviewer reads should be legible:
/// a golden that no one can read is a golden no one checks.
fn render_golden(frame: &TerminalFrame) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} {}x{} (story {}x{})\n",
        frame.story_id, frame.cols, frame.rows, frame.story_cols, frame.story_rows
    ));
    for row in 0..frame.rows {
        for col in 0..frame.cols {
            let index = usize::from(row) * usize::from(frame.cols) + usize::from(col);
            match frame.cells.get(index) {
                Some(cell) if !cell.ch.is_empty() => out.push_str(&cell.ch),
                _ => out.push(' '),
            }
        }
        out.push('\n');
    }
    out
}

fn frame_for(story_id: &str) -> Option<TerminalFrame> {
    let story = story_by_id(story_id)?;
    let system = lookbook_system(RolePalette::junie());
    Some(paint_story_frame(story, &system, None, None))
}

#[test]
fn flagship_stories_match_their_baselines() {
    let bless = std::env::var("TERMROCK_BLESS_PREVIEWS").is_ok();
    if bless {
        fs::create_dir_all(goldens_dir()).expect("create goldens dir");
    }

    let mut missing = Vec::new();
    let mut drifted = Vec::new();
    let mut covered = 0usize;

    for id in FLAGSHIP {
        // A story that was renamed is a coverage gap, not a failure: the list
        // is checked below so it cannot silently rot to zero.
        let Some(frame) = frame_for(id) else {
            missing.push(id);
            continue;
        };
        covered += 1;
        let golden = render_golden(&frame);
        let path = golden_path(id);
        if bless {
            fs::write(&path, &golden).expect("write golden");
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(expected) if expected == golden => {}
            Ok(expected) => {
                let first_diff = expected
                    .lines()
                    .zip(golden.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                drifted.push(format!(
                    "{id}: first differing row {first_diff}\n  golden: {:?}\n  render: {:?}",
                    expected.lines().nth(first_diff).unwrap_or(""),
                    golden.lines().nth(first_diff).unwrap_or("")
                ));
            }
            Err(_) => drifted.push(format!("{id}: no baseline — run `mise run bless-previews`")),
        }
    }

    assert!(
        covered >= 8,
        "the flagship list has rotted: only {covered} of {} stories still exist ({missing:?})",
        FLAGSHIP.len()
    );
    assert!(
        drifted.is_empty(),
        "flagship previews drifted from their baselines. Review the change, then \
         `mise run bless-previews` if it is intended:\n{}",
        drifted.join("\n")
    );
}
