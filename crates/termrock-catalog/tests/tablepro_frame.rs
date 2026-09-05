// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

#![allow(missing_docs)]

use termrock_catalog::canonical_tablepro_frame_json;
use termrock_catalog::cli::{Command, TableProFrameOptions, parse_command};
use termrock_catalog::host::TableProFrameSession;

#[test]
fn tablepro_frame_command_is_strict() {
    assert_eq!(
        parse_command([
            "frame",
            "--application",
            "tablepro",
            "--connect",
            "Production",
            "--keys",
            "Ctrl+T,Enter",
            "--cols",
            "80",
            "--rows",
            "24",
        ]),
        Ok(Command::TableProFrame(TableProFrameOptions {
            connect: Some("Production".to_owned()),
            cols: 80,
            rows: 24,
            keys: vec!["Ctrl+T".to_owned(), "Enter".to_owned()],
        }))
    );
    assert!(parse_command(["frame", "--application", "tablepro", "--cols", "80",]).is_err());
    assert!(
        parse_command([
            "frame",
            "--application",
            "other",
            "--cols",
            "80",
            "--rows",
            "24",
        ])
        .is_err()
    );
    assert!(
        parse_command([
            "frame",
            "--application",
            "tablepro",
            "--page",
            "overview",
            "--cols",
            "80",
            "--rows",
            "24",
        ])
        .is_err()
    );
    assert!(parse_command(["frame", "--page", "overview", "--connect", "Production",]).is_err());
}

#[test]
fn tablepro_frame_serializes_real_app_cells_and_cursor_metadata() {
    let json = canonical_tablepro_frame_json(Some("Production"), 80, 24, &[]).expect("frame");
    let unconnected = canonical_tablepro_frame_json(None, 80, 24, &[]).expect("frame");
    let value: serde_json::Value = serde_json::from_str(&json).expect("frame JSON");
    assert_eq!(value["story_id"], "tablepro");
    assert_eq!(value["title"], "TablePro");
    assert_eq!(value["component"], "Applications");
    assert_eq!(value["cols"], 80);
    assert_eq!(value["rows"], 24);
    assert_eq!(value["story_cols"], 80);
    assert_eq!(value["story_rows"], 24);
    assert_eq!(value["cells"].as_array().map(Vec::len), Some(80 * 24));
    assert_eq!(value["cursor_visible"], false);
    assert_ne!(json, unconnected);
}

#[test]
fn tablepro_frame_keys_drive_the_shared_application() {
    let mut session = TableProFrameSession::mount(Some("Production"), 80, 24).expect("mount");
    let before = session.frame();
    session.dispatch_key("Ctrl+T").expect("new query key");
    let after = session.frame();
    assert_ne!(before, after);
}
