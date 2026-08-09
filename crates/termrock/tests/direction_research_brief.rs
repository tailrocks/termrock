// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Structural proof that the durable shadcn-TUI research brief is present and
//! still carries the sections the direction goal requires.
//!
//! Drives the real in-repo artifact via `include_str!` — not a reimplementation
//! of the research content.

const BRIEF: &str = include_str!("../../../plans/038-direction-shadcn-tui-research.md");

#[test]
fn research_brief_is_substantial_prose() {
    assert!(
        BRIEF.len() > 8_000,
        "research brief unexpectedly short ({} bytes)",
        BRIEF.len()
    );
    assert!(
        BRIEF.lines().count() > 100,
        "research brief has too few lines"
    );
}

#[test]
fn research_brief_surveys_termrock_surface_and_gaps() {
    for needle in [
        "TermRock inventory",
        "Structural gaps",
        "Public widgets",
        "FocusRing",
        "shadcn",
    ] {
        assert!(
            BRIEF.contains(needle),
            "missing TermRock surface/gap coverage: {needle}"
        );
    }
}

#[test]
fn research_brief_covers_external_landscape() {
    for needle in [
        "Grok Build",
        "Amp",
        "awesome-tuis",
        "lazygit",
        "btop",
        "yazi",
        "posting",
        "OpenCode",
        "Charm",
        "Textual",
    ] {
        assert!(
            BRIEF.contains(needle),
            "missing landscape reference: {needle}"
        );
    }
}

#[test]
fn research_brief_has_prioritized_breaking_ok_roadmap() {
    for needle in [
        "Ranked opportunities",
        "Breaking changes",
        "ToolCard",
        "WorkSurface",
        "Density",
        "non-goals",
        "Tier 0",
        "Phase A",
        "quality over compatibility",
    ] {
        assert!(
            BRIEF.contains(needle),
            "missing prioritized roadmap content: {needle}"
        );
    }
}
