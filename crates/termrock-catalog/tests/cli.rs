// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

#![allow(missing_docs)]

use std::path::Path;

use termrock_catalog::catalog::PageId;
use termrock_catalog::cli::{CaptureOptions, Command, FrameOptions, RenderOptions, parse_command};
use termrock_catalog::{DEFAULT_FRAME_COLS, DEFAULT_FRAME_ROWS, canonical_frame_json};

#[test]
fn frame_command_parses_page_and_optional_dimensions() {
    assert_eq!(
        parse_command([
            "frame",
            "--page",
            "chips-selects",
            "--cols",
            "80",
            "--rows",
            "24"
        ]),
        Ok(Command::Frame(FrameOptions {
            page: Some(PageId::CHIPS),
            scenario: None,
            cols: 80,
            rows: 24,
            keys: Vec::new(),
        }))
    );
    assert_eq!(
        parse_command(["frame", "--page", "datagrid"]),
        Ok(Command::Frame(FrameOptions {
            page: Some(PageId::GRID),
            scenario: None,
            cols: DEFAULT_FRAME_COLS,
            rows: DEFAULT_FRAME_ROWS,
            keys: Vec::new(),
        }))
    );
}

#[test]
fn render_command_requires_output_directory() {
    assert_eq!(
        parse_command(["render", "--out", "tmp/catalog"]),
        Ok(Command::Render(RenderOptions {
            out: Path::new("tmp/catalog").to_owned(),
            scenarios: false,
        }))
    );
    assert!(parse_command(["render"]).is_err());
    assert!(parse_command(["frame"]).is_err());
    assert!(parse_command(["frame", "--scenario", "button/activation"]).is_ok());
}

#[test]
fn frame_command_parses_scenario_keys() {
    assert_eq!(
        parse_command([
            "frame",
            "--scenario",
            "button/activation",
            "--keys",
            "Tab,Enter"
        ]),
        Ok(Command::Frame(FrameOptions {
            page: None,
            scenario: Some("button/activation".to_owned()),
            cols: DEFAULT_FRAME_COLS,
            rows: DEFAULT_FRAME_ROWS,
            keys: vec!["Tab".to_owned(), "Enter".to_owned()],
        }))
    );
}

#[test]
fn capture_command_requires_output_and_accepts_one_known_scenario() {
    assert_eq!(
        parse_command(["capture", "--out", "target/capture", "--scenario", "t_100",]),
        Ok(Command::Capture(CaptureOptions {
            out: Path::new("target/capture").to_owned(),
            scenario: Some("t_100".to_owned()),
        }))
    );
    assert!(parse_command(["capture", "--out", "target/capture", "--all"]).is_ok());
    assert!(parse_command(["capture"]).is_err());
    assert!(
        parse_command([
            "capture",
            "--out",
            "target/capture",
            "--scenario",
            "missing"
        ])
        .is_err()
    );
}

#[test]
fn canonical_frame_json_is_byte_deterministic() {
    let first = canonical_frame_json(PageId::GRID, 80, 24).expect("frame");
    let second = canonical_frame_json(PageId::GRID, 80, 24).expect("frame");
    assert_eq!(first, second);
    assert!(first.ends_with('}'));
    assert!(first.contains("\"story_id\": \"datagrid\""));
}
