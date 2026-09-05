// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Fail-first five-artifact parity against the frozen replay snapshot.
//!
//! `verify/junie/reference/scenes/` holds the checked-in export of the
//! canonical catalog replay (see `reference/manifest.json` provenance). This
//! gate is the drift tripwire: any render, scenario, or manifest change that
//! is not a deliberate snapshot regeneration fails here. Live source
//! anchoring lives in `tests/parity.rs` against `verify/junie/source-headless`.
//!
//! Golden PNGs are required and decoded with the same vendored raster as the
//! target. Pixel parity re-rasterizes the golden ANSI grid with
//! termrock-raster and compares it against the rendered target PNG.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use termrock::style::RolePalette;
use termrock_catalog::ansi_grid::{first_txt_diff, from_snapshot, parse_ansi, parse_html};
use termrock_catalog::capture;
use termrock_catalog::scenarios::{self, Scenario};

const ARTIFACTS: [&str; 5] = ["ansi", "cursor", "txt", "html", "png"];
const SNAPSHOT_SCENARIO_COUNT: usize = 63;

#[derive(serde::Deserialize)]
struct SnapshotManifest {
    artifact_set: Vec<String>,
    scenes: BTreeMap<String, SnapshotScene>,
}

#[derive(serde::Deserialize)]
struct SnapshotScene {
    cols: u16,
    rows: u16,
    sha256: BTreeMap<String, String>,
}

fn shots_dir() -> PathBuf {
    if let Ok(p) = std::env::var("JUNIE_SHOTS") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify/junie/reference/scenes")
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("manifest.json")
}

fn snapshot_manifest(dir: &Path) -> SnapshotManifest {
    let path = manifest_path(dir);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read replay manifest {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("cannot parse replay manifest {}: {e}", path.display()))
}

fn snapshot_ids() -> BTreeSet<&'static str> {
    scenarios::ALL.iter().map(|s| s.id).collect()
}

fn validate_manifest(dir: &Path, manifest: &SnapshotManifest) {
    let artifact_set: Vec<&str> = manifest.artifact_set.iter().map(String::as_str).collect();
    assert_eq!(
        artifact_set, ARTIFACTS,
        "replay manifest artifact set changed"
    );
    assert_eq!(
        scenarios::ALL.len(),
        SNAPSHOT_SCENARIO_COUNT,
        "replay inventory count changed"
    );
    assert_eq!(
        manifest.scenes.len(),
        SNAPSHOT_SCENARIO_COUNT,
        "replay manifest scene count changed"
    );

    let manifest_ids: BTreeSet<&str> = manifest.scenes.keys().map(String::as_str).collect();
    assert_eq!(
        manifest_ids,
        snapshot_ids(),
        "manifest/replay scene IDs differ"
    );

    for scenario in scenarios::ALL {
        let scene = manifest
            .scenes
            .get(scenario.id)
            .unwrap_or_else(|| panic!("replay manifest missing {}", scenario.id));
        assert_eq!(
            (scene.cols, scene.rows),
            (scenario.cols, scenario.rows),
            "replay manifest dimensions differ for {}",
            scenario.id
        );

        let digest_ids: BTreeSet<&str> = scene.sha256.keys().map(String::as_str).collect();
        let expected_digest_ids: BTreeSet<&str> = ARTIFACTS.into_iter().collect();
        assert_eq!(
            digest_ids, expected_digest_ids,
            "replay manifest digests incomplete for {}",
            scenario.id
        );
        for artifact in ARTIFACTS {
            let digest = &scene.sha256[artifact];
            assert!(
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid SHA-256 for {}.{} in replay manifest",
                scenario.id,
                artifact
            );
        }
    }

    for (id, scene) in &manifest.scenes {
        let scenario = scenarios::ALL
            .iter()
            .find(|candidate| candidate.id == id)
            .unwrap_or_else(|| panic!("replay manifest scene has no scenario: {id}"));
        for artifact in ARTIFACTS {
            let bytes = read_bytes(dir, id, artifact);
            assert!(
                !bytes.is_empty(),
                "snapshot artifact {}.{} is empty",
                id,
                artifact
            );
            if artifact == "png" {
                if let Err(error) = termrock_raster::compare_png_pixels(&bytes, &bytes) {
                    panic!("snapshot artifact {id}.png is not a decodable PNG: {error}");
                }
            }
        }
        assert_eq!(
            (scene.cols, scene.rows),
            (scenario.cols, scenario.rows),
            "replay manifest dimensions differ for {id}"
        );
    }
}

fn read_bytes(dir: &Path, id: &str, ext: &str) -> Vec<u8> {
    let p = dir.join(format!("{id}.{ext}"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("missing {}: {e}", p.display()))
}

fn read(dir: &Path, id: &str, ext: &str) -> String {
    let p = dir.join(format!("{id}.{ext}"));
    let bytes = read_bytes(dir, id, ext);
    assert!(
        !bytes.is_empty(),
        "snapshot artifact {} is empty",
        p.display()
    );
    String::from_utf8(bytes)
        .unwrap_or_else(|e| panic!("snapshot artifact {} is not UTF-8: {e}", p.display()))
}

fn fail(s: &Scenario, kind: &str, msg: String) -> ! {
    panic!(
        "first mismatch scenario {} {} {}x{}: {msg}",
        s.id, kind, s.cols, s.rows
    );
}

fn compare_one(dir: &Path, s: &Scenario, compare_png: bool) {
    let art = capture::replay(s);

    let golden_txt = read(dir, s.id, "txt");
    if art.txt().as_bytes() != golden_txt.as_bytes() {
        let detail = first_txt_diff(&art.txt(), &golden_txt)
            .map(|(x, y, expected, actual)| {
                format!(
                    "first visible difference at ({x},{y}): {expected:?} != {actual:?}; ours={:?}; golden={:?}",
                    art.txt().lines().nth(usize::from(y)).unwrap_or(""),
                    golden_txt.lines().nth(usize::from(y)).unwrap_or("")
                )
            })
            .unwrap_or_else(|| "line endings or trailing cells differ".to_owned());
        fail(s, "txt", format!("byte-exact mismatch: {detail}"));
    }

    let golden_cursor = read(dir, s.id, "cursor");
    let ours_c = art.cursor();
    if ours_c.as_bytes() != golden_cursor.as_bytes() {
        fail(
            s,
            "cursor",
            format!("expected {:?} actual {:?}", golden_cursor, ours_c),
        );
    }

    let golden_ansi = read(dir, s.id, "ansi");
    let golden_grid = parse_ansi(&golden_ansi, s.cols, s.rows);
    let ours_grid = from_snapshot(&art.snapshot);
    if let Some((x, y, why)) = golden_grid.first_strict_diff(&ours_grid) {
        fail(s, "ansi", format!("cell ({x},{y}) {why}"));
    }

    let golden_html = read(dir, s.id, "html");
    let html_grid = parse_html(&golden_html, s.cols, s.rows);
    if let Some((x, y, why)) = html_grid.first_strict_diff(&ours_grid) {
        fail(s, "html", format!("cell ({x},{y}) {why}"));
    }

    let golden_png = read_bytes(dir, s.id, "png");
    if let Err(diff) = termrock_raster::compare_png_pixels(&golden_png, &golden_png) {
        fail(s, "png", format!("decode golden PNG: {diff}"));
    }
    let ours_png = art
        .png()
        .unwrap_or_else(|e| fail(s, "png", format!("raster ours: {e}")));
    if let Err(diff) = termrock_raster::compare_png_pixels(&ours_png, &ours_png) {
        fail(s, "png", format!("decode target PNG: {diff}"));
    }
    if !compare_png {
        return;
    }

    let rerasterized =
        termrock_raster::render_png(&golden_grid.for_raster().to_buffer(), &RolePalette::junie())
            .unwrap_or_else(|e| fail(s, "png", format!("raster golden ANSI: {e}")));
    if let Err(diff) = termrock_raster::compare_png_pixels(&rerasterized, &ours_png) {
        fail(s, "png", diff.to_string());
    }
}

#[test]
fn canonical_replay_manifest_contains_sixty_three_stems_and_five_artifacts() {
    let dir = shots_dir();
    let manifest = snapshot_manifest(&dir);
    validate_manifest(&dir, &manifest);
}

#[test]
fn s_chips_idle_cell_and_cursor_match_frozen_snapshot() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "s_chips")
        .expect("s_chips");
    let art = capture::replay(s);
    let golden_txt = read(&dir, s.id, "txt");
    assert_eq!(
        art.txt().as_bytes(),
        golden_txt.as_bytes(),
        "snapshot text drifted"
    );
    let golden_cursor = read(&dir, s.id, "cursor");
    assert_eq!(
        art.cursor().as_bytes(),
        golden_cursor.as_bytes(),
        "snapshot cursor drifted"
    );
    let golden = parse_ansi(&read(&dir, s.id, "ansi"), s.cols, s.rows);
    let got = from_snapshot(&art.snapshot);
    assert!(
        golden.first_strict_diff(&got).is_none(),
        "snapshot ANSI grid drifted"
    );
}

#[test]
fn fail_first_shots_five_artifacts() {
    let dir = shots_dir();
    let manifest = snapshot_manifest(&dir);
    validate_manifest(&dir, &manifest);
    let compare_png = match std::env::var("TERMROCK_SHOTS_SKIP_PNG") {
        Ok(value) if value == "1" => {
            eprintln!(
                "TERMROCK_SHOTS_SKIP_PNG=1: explicitly skipping cross-raster PNG parity; \
                 golden and target PNGs are still required and decoded"
            );
            false
        }
        Ok(value) => panic!(
            "TERMROCK_SHOTS_SKIP_PNG must be exactly 1 for the documented raster opt-out, got {value:?}"
        ),
        Err(_) => true,
    };
    for id in manifest.scenes.keys() {
        let s = scenarios::ALL
            .iter()
            .find(|scenario| scenario.id == id)
            .unwrap_or_else(|| panic!("replay manifest scene has no scenario: {id}"));
        compare_one(&dir, s, compare_png);
    }
}

#[test]
fn f_80x24_taskrunner_matches_frozen_snapshot() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "f_80x24_taskrunner")
        .expect("f_80x24_taskrunner");
    let art = capture::replay(s);
    let golden = std::fs::read_to_string(dir.join("f_80x24_taskrunner.txt")).expect("golden txt");
    assert_eq!(art.txt().as_bytes(), golden.as_bytes());
}
