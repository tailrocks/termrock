// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Structural proof that the archived shadcn-TUI direction brief remains in-repo.

const BRIEF: &str = include_str!("../../../docs/design/shadcn-tui-direction.md");

#[test]
fn direction_brief_is_substantial_prose() {
    assert!(BRIEF.len() > 8_000, "brief too short ({} bytes)", BRIEF.len());
    assert!(BRIEF.lines().count() > 100);
}

#[test]
fn direction_brief_surveys_termrock_and_landscape() {
    for needle in [
        "Structural gaps",
        "Grok Build",
        "Amp",
        "awesome-tuis",
        "lazygit",
        "Ranked opportunities",
        "ToolCard",
        "WorkSurface",
        "Breaking changes",
    ] {
        assert!(BRIEF.contains(needle), "missing: {needle}");
    }
}

#[test]
fn direction_brief_records_execution_status() {
    assert!(
        BRIEF.contains("Archived direction brief") || BRIEF.contains("0029"),
        "brief should record that the research was executed"
    );
}
