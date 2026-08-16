// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! PNG baselines for the Jackin-used subset stories.
//!
//! Every comparison decodes PNGs and compares RGBA pixels at zero tolerance.

#![cfg(feature = "native")]

use std::{
    fs,
    path::{Path, PathBuf},
};

use termrock_lookbook::png::{render_story_png, story_png_filename, subset_stories};
use termrock_raster::compare_png_pixels;

fn baselines_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("baselines")
        .join("png")
}

#[test]
fn subset_stories_match_their_png_baselines() {
    let bless = std::env::var("TERMROCK_BLESS_PNGS").is_ok();
    if bless {
        fs::create_dir_all(baselines_dir()).expect("create baselines dir");
    }

    let subset = subset_stories();
    assert!(
        subset.len() >= 87,
        "the subset filter has rotted: only {} stories match the 16 subset components (>= 87 expected)",
        subset.len()
    );

    let mut drifted = Vec::new();
    for story in subset {
        let id = story.id;
        let first = render_story_png(story);
        let second = render_story_png(story);
        assert!(
            compare_png_pixels(&first, &second).is_ok(),
            "{id}: render-twice mismatch — the raster pipeline produced two different raw RGBA outputs in one process. This is a PIPELINE BUG (W1 failure point b), not design drift: do NOT resolve it by blessing. See ledger assumption A3 (plans/jackin-termrock-parity/coverage.md). Fix the rasterizer, then re-run."
        );

        let path = baselines_dir().join(story_png_filename(story));
        if bless {
            fs::write(&path, first).expect("write PNG baseline");
            continue;
        }

        match fs::read(&path) {
            Err(_) => drifted.push(format!(
                "{id}: no baseline at {} — run `mise run bless-pngs`",
                path.display()
            )),
            Ok(committed) => {
                if let Err(diff) = compare_png_pixels(&first, &committed) {
                    drifted.push(format!("{id}: {diff}"));
                }
            }
        }
    }

    assert!(
        drifted.is_empty(),
        "subset PNG previews drifted from their baselines. Review the change, then `mise run bless-pngs` if it is intended, and commit the rewritten PNGs in the same PR. If this fails only on Linux CI against a macOS-blessed baseline with no intended paint change, ledger assumption A3 is falsified: use its pinned-Linux or CI-produced bless fallback instead of re-blessing on macOS:\n{}",
        drifted.join("\n")
    );
}
