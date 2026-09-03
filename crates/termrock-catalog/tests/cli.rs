// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

#![allow(missing_docs)]

use std::path::Path;

use termrock_catalog::catalog::PageId;
use termrock_catalog::cli::{Command, FrameOptions, RenderOptions, parse_command};
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
            page: PageId::CHIPS,
            cols: 80,
            rows: 24,
        }))
    );
    assert_eq!(
        parse_command(["frame", "--page", "datagrid"]),
        Ok(Command::Frame(FrameOptions {
            page: PageId::GRID,
            cols: DEFAULT_FRAME_COLS,
            rows: DEFAULT_FRAME_ROWS,
        }))
    );
}

#[test]
fn render_command_requires_output_directory() {
    assert_eq!(
        parse_command(["render", "--out", "tmp/catalog"]),
        Ok(Command::Render(RenderOptions {
            out: Path::new("tmp/catalog").to_owned(),
        }))
    );
    assert!(parse_command(["render"]).is_err());
    assert!(parse_command(["frame"]).is_err());
}

#[test]
fn canonical_frame_json_is_byte_deterministic() {
    let first = canonical_frame_json(PageId::GRID, 80, 24).expect("frame");
    let second = canonical_frame_json(PageId::GRID, 80, 24).expect("frame");
    assert_eq!(first, second);
    assert!(first.ends_with('}'));
    assert!(first.contains("\"story_id\": \"datagrid\""));
}
