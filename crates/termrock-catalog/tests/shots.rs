// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Fail-first five-artifact parity against the canonical source manifest.
//!
//! Source PNGs are required and decoded, but their Pillow/FreeType raster is
//! not compared directly with TermRock's vendored raster. Pixel parity uses
//! the source ANSI grid re-rasterized by termrock-raster.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use termrock::style::RolePalette;
use termrock_catalog::ansi_grid::{first_txt_diff, from_snapshot, parse_ansi, parse_html};
use termrock_catalog::capture;
use termrock_catalog::scenarios::{self, Scenario};

const ARTIFACTS: [&str; 5] = ["ansi", "cursor", "txt", "html", "png"];
const SOURCE_SCENARIO_COUNT: usize = 63;

#[derive(serde::Deserialize)]
struct SourceManifest {
    artifact_set: Vec<String>,
    scenes: BTreeMap<String, SourceScene>,
}

#[derive(serde::Deserialize)]
struct SourceScene {
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

fn source_manifest(dir: &Path) -> SourceManifest {
    let path = manifest_path(dir);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read source manifest {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("cannot parse source manifest {}: {e}", path.display()))
}

fn source_ids() -> BTreeSet<&'static str> {
    scenarios::ALL.iter().map(|s| s.id).collect()
}

fn validate_manifest(dir: &Path, manifest: &SourceManifest) {
    let artifact_set: Vec<&str> = manifest.artifact_set.iter().map(String::as_str).collect();
    assert_eq!(
        artifact_set, ARTIFACTS,
        "source manifest artifact set changed"
    );
    assert_eq!(
        scenarios::ALL.len(),
        SOURCE_SCENARIO_COUNT,
        "replay inventory count changed"
    );
    assert_eq!(
        manifest.scenes.len(),
        SOURCE_SCENARIO_COUNT,
        "source manifest scene count changed"
    );

    let manifest_ids: BTreeSet<&str> = manifest.scenes.keys().map(String::as_str).collect();
    assert_eq!(
        manifest_ids,
        source_ids(),
        "manifest/replay scene IDs differ"
    );

    for scenario in scenarios::ALL {
        let scene = manifest
            .scenes
            .get(scenario.id)
            .unwrap_or_else(|| panic!("source manifest missing {}", scenario.id));
        assert_eq!(
            (scene.cols, scene.rows),
            (scenario.cols, scenario.rows),
            "source manifest dimensions differ for {}",
            scenario.id
        );

        let digest_ids: BTreeSet<&str> = scene.sha256.keys().map(String::as_str).collect();
        let expected_digest_ids: BTreeSet<&str> = ARTIFACTS.into_iter().collect();
        assert_eq!(
            digest_ids, expected_digest_ids,
            "source manifest digests incomplete for {}",
            scenario.id
        );
        for artifact in ARTIFACTS {
            let digest = &scene.sha256[artifact];
            assert!(
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid SHA-256 for {}.{} in source manifest",
                scenario.id,
                artifact
            );
        }
    }

    for (id, scene) in &manifest.scenes {
        let scenario = scenarios::ALL
            .iter()
            .find(|candidate| candidate.id == id)
            .unwrap_or_else(|| panic!("source manifest scene has no replay: {id}"));
        for artifact in ARTIFACTS {
            let bytes = read_bytes(dir, id, artifact);
            assert!(
                !bytes.is_empty(),
                "source artifact {}.{} is empty",
                id,
                artifact
            );
            if artifact == "png" {
                if let Err(error) = termrock_raster::compare_png_pixels(&bytes, &bytes) {
                    panic!("source artifact {id}.png is not a decodable PNG: {error}");
                }
            }
        }
        assert_eq!(
            (scene.cols, scene.rows),
            (scenario.cols, scenario.rows),
            "source manifest dimensions differ for {id}"
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
        "source artifact {} is empty",
        p.display()
    );
    String::from_utf8(bytes)
        .unwrap_or_else(|e| panic!("source artifact {} is not UTF-8: {e}", p.display()))
}

fn fail(s: &Scenario, kind: &str, msg: String) -> ! {
    panic!(
        "first mismatch scenario {} {} {}x{}: {msg}",
        s.id, kind, s.cols, s.rows
    );
}

fn compare_one(dir: &Path, s: &Scenario, compare_png: bool) {
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

    let src_png = read_bytes(dir, s.id, "png");
    if let Err(diff) = termrock_raster::compare_png_pixels(&src_png, &src_png) {
        fail(s, "png", format!("decode source PNG: {diff}"));
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

    let source_png =
        termrock_raster::render_png(&src_grid.for_raster().to_buffer(), &RolePalette::junie())
            .unwrap_or_else(|e| fail(s, "png", format!("raster source ANSI: {e}")));
    if let Err(diff) = termrock_raster::compare_png_pixels(&source_png, &ours_png) {
        fail(s, "png", diff.to_string());
    }
}

#[test]
fn canonical_source_manifest_contains_sixty_three_stems_and_five_artifacts() {
    let dir = shots_dir();
    let manifest = source_manifest(&dir);
    validate_manifest(&dir, &manifest);
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
fn fail_first_shots_five_artifacts() {
    let dir = shots_dir();
    let manifest = source_manifest(&dir);
    validate_manifest(&dir, &manifest);
    let compare_png = match std::env::var("TERMROCK_SHOTS_SKIP_PNG") {
        Ok(value) if value == "1" => {
            eprintln!(
                "TERMROCK_SHOTS_SKIP_PNG=1: explicitly skipping cross-raster PNG parity; \
                 source and target PNGs are still required and decoded"
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
            .unwrap_or_else(|| panic!("source manifest scene has no replay: {id}"));
        compare_one(&dir, s, compare_png);
    }
}

#[test]
fn f_80x24_taskrunner_uses_historical_source_navigation() {
    let dir = shots_dir();
    let s = scenarios::ALL
        .iter()
        .find(|s| s.id == "f_80x24_taskrunner")
        .expect("f_80x24_taskrunner");
    let art = capture::replay(s);
    let src = std::fs::read_to_string(dir.join("f_80x24_taskrunner.txt")).expect("src txt");
    assert_eq!(art.txt().as_bytes(), src.as_bytes());
}
