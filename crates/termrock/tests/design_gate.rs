// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Design gates for the canonical junie system.
//!
//! Two mechanisms live here, and both read only public API or the crate's own
//! painted source:
//!
//! 1. *Behaviour gates* drive [`termrock::style::DesignSystem`], its recipes,
//!    and the widgets into a `Buffer`, then assert what the junie contract
//!    says the paint must be.
//! 2. *Scan gates* read `src/widgets` and `src/patterns` (painted half only —
//!    everything from the first `#[cfg(test)]` on is fixture text, and
//!    comment lines are prose) so a reviewer does not have to catch a rule by
//!    eye across two hundred files.
//!
//! Every gate cites its authority: `research/junie-campaign/phase3-decision.md`
//! (`D1`–`D9`) and `research/junie-campaign/reference-spec.md`. The decision
//! file is the binding contract; the reference spec is the fidelity record.
//!
//! Old gates that encoded the pre-junie law (named ANSI baseline, Density
//! profiles, hover/`DIM` roles, edge fades, hover roles) were rewritten to the
//! junie invariant that carries the same property, or deleted with a one-line
//! justification at the site.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};
use ratatui_core::text::Line;
use ratatui_core::widgets::{StatefulWidget, Widget};
use termrock::runtime::{FrameTick, Instant};
use termrock::scroll::{
    SCROLLBAR_TRACK, ScrollAxis, ScrollbarGeometry, ScrollbarSpec, ScrollbarStyle,
    paint_overflow_scrollbar, render_scrollbar,
};
use termrock::style::{
    ACTION_FLASH_MS, AccentUsage, ActionFlash, BadgeKind, BorderShape, ButtonKind,
    ButtonRecipeVariant, ColorCapability, ControlState, DesignSystem, Elevation, Glyph, GlyphSet,
    JunieTheme, ListRowVisualState, MotionPolicy, NonColorCue, PanelChrome, RecipeFamily, Role,
    RolePalette, SpacingScale, SurfaceFamily, SyntaxTone, Tone, VisualState, contrast_ratio,
    downgrade, nearest_16, nearest_256,
};
use termrock::text::display_cols;

// ── Scan helpers ─────────────────────────────────────────────────────────────
//
// The scans are mechanical on purpose: they read the painted half of a file,
// never its fixtures, and they treat comment lines as documentation prose.

/// A painted source file, already trimmed of its test module.
struct PaintedSource {
    path: PathBuf,
    /// `(1-based line number, line)` for the painted half only.
    lines: Vec<(usize, String)>,
    /// Indices into `lines` that sit inside an `example_*` payload function.
    ///
    /// Those functions carry *simulated third-party output* — git porcelain,
    /// cargo test lines — which the copy rules do not govern. A literal `...`
    /// there is data, not microcopy.
    payload: Vec<bool>,
}

fn crate_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Every painted source under `widgets/` and `patterns/`.
fn painted_sources() -> Vec<PaintedSource> {
    let src = crate_src();
    let mut out = Vec::new();
    for dir in ["widgets", "patterns"] {
        for path in rust_files(&src.join(dir)) {
            let body = fs::read_to_string(&path).expect("read source");
            let lines: Vec<(usize, String)> = body
                .lines()
                .enumerate()
                .map(|(i, l)| (i + 1, l.to_string()))
                .take_while(|(_, l)| !l.trim_start().starts_with("#[cfg(test)]"))
                .filter(|(_, l)| !l.trim_start().starts_with("//"))
                .collect();
            let payload = payload_mask(&lines);
            out.push(PaintedSource {
                path,
                lines,
                payload,
            });
        }
    }
    assert!(out.len() > 100, "painted source scan found too few files");
    out
}

/// Marks the lines that belong to an `example_*` payload function.
///
/// Top-level `fn`/`pub fn` items start at column zero, so a run ends at the
/// next such item — good enough for a lint, and it never spans a file.
fn payload_mask(lines: &[(usize, String)]) -> Vec<bool> {
    let mut out = Vec::with_capacity(lines.len());
    let mut inside = false;
    for (_, line) in lines {
        let starts_item = line.starts_with("fn ") || line.starts_with("pub fn ");
        if starts_item {
            inside = line.contains(" example_");
        }
        out.push(inside);
    }
    out
}

/// String literals on one line, quotes included.
///
/// Deliberately simple: it walks the line and respects `\"` escapes. Raw
/// strings and multi-line literals are not painted copy in this codebase.
fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.char_indices();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut escaped = false;
        for (end, c) in chars.by_ref() {
            if escaped {
                escaped = false;
                continue;
            }
            match c {
                '\\' => escaped = true,
                '"' => {
                    out.push(line[start..=end].to_string());
                    break;
                }
                _ => {}
            }
        }
    }
    out
}

/// Whether the literal's ASCII form is explicitly gated nearby.
///
/// The law allows an ASCII twin of a Unicode string; it forbids a bare `...`
/// that no profile switch selects. Gating shows up as an `ascii` flag test or
/// an `_ASCII` constant within the surrounding few lines.
fn ascii_gated(lines: &[(usize, String)], index: usize) -> bool {
    let start = index.saturating_sub(4);
    lines[start..=index]
        .iter()
        .any(|(_, l)| l.to_ascii_lowercase().contains("ascii"))
}

#[test]
fn gates_detect_their_own_violations() {
    // The scans are only worth their runtime if they actually fire.
    let bare = [(1usize, r#"    let msg = "loading...";"#.to_string())];
    assert!(!ascii_gated(&bare, 0));
    assert_eq!(string_literals(&bare[0].1), vec!["\"loading...\""]);

    let gated = [
        (1usize, "    let msg = if self.ascii {".to_string()),
        (2, r#"        "loading...""#.to_string()),
    ];
    assert!(ascii_gated(&gated, 1));

    let escaped = r#"println!("a \"b\" c", "d");"#;
    assert_eq!(
        string_literals(escaped),
        vec![r#""a \"b\" c""#.to_string(), "\"d\"".to_string()]
    );

    let lines = [
        (
            1usize,
            "pub fn example_terminal_lines() -> Vec<Line> {".to_string(),
        ),
        (2, r#"    line("test x ... ok")"#.to_string()),
        (3, "}".to_string()),
        (4, "pub fn paint() {".to_string()),
        (5, r#"    let msg = "loading...";"#.to_string()),
    ];
    assert_eq!(payload_mask(&lines), vec![true, true, true, false, false]);

    // And the painted-source scan itself must keep finding the corpus.
    assert!(painted_sources().len() > 100);
}

// ── Paint helpers ────────────────────────────────────────────────────────────

/// Renders one painter into a fresh buffer.
fn painted(area: Rect, paint: impl FnOnce(&mut Buffer)) -> Buffer {
    let mut buffer = Buffer::empty(area);
    paint(&mut buffer);
    buffer
}

fn count_cells(buffer: &Buffer, keep: impl Fn(&Cell) -> bool) -> usize {
    buffer.content().iter().filter(|cell| keep(cell)).count()
}

// ── A. Colour truth (D1, D2, D9) ─────────────────────────────────────────────
//
// junie is the only palette. The tables below restate the reference hexes as
// literals here, so a token is measured against the reference rather than
// against the port of itself.

const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

// The reference raw palette (`junie-tui` `theme.rs`, module `palette`).
const BLACK: Color = rgb(0x00_00_00);
const CHROME: Color = rgb(0x11_11_11);
const CARD: Color = rgb(0x18_18_1b);
const INPUT: Color = rgb(0x1e_1e_22);
const INPUT_HOVER: Color = rgb(0x23_23_28);
const OVERLAY: Color = rgb(0x27_27_2a);
const POPOVER: Color = rgb(0x3f_3f_46);
const WHITE: Color = rgb(0xff_ff_ff);
const WHITE_70: Color = rgb(0xb3_b3_b3);
const WHITE_50: Color = rgb(0x80_80_80);
const WHITE_30: Color = rgb(0x4d_4d_4d);
const WHITE_15: Color = rgb(0x26_26_26);
const GREEN: Color = rgb(0x48_e0_54);
const GREEN_80: Color = rgb(0x3a_b3_43);
const GREEN_60: Color = rgb(0x2b8632);
const GREEN_20: Color = rgb(0x0f_2e_13);
const ON_GREEN: Color = rgb(0x19_19_1c);
const RED: Color = rgb(0xe4_45_45);
const AMBER: Color = rgb(0xf5_9e_09);
const HIGHLIGHT: Color = rgb(0x2f_5a_a8);
const HIGHLIGHT_DANGER: Color = rgb(0x7a_2a_2a);
const ERROR_SOFT: Color = rgb(0xd9_8a_8a);

/// The 24 active junie tokens, by name and canonical hex (D2 table).
const JUNIE_TOKEN_TABLE: &[(&str, Color)] = &[
    ("canvas", BLACK),
    ("surface", CHROME),
    ("surface_elevated", CARD),
    ("surface_overlay", OVERLAY),
    ("field", INPUT),
    ("field_hover", INPUT_HOVER),
    ("popover", POPOVER),
    ("border_subtle", WHITE_15),
    ("border_strong", WHITE_30),
    ("text_primary", WHITE),
    ("text_secondary", WHITE_70),
    ("text_muted", WHITE_50),
    ("text_faint", WHITE_30),
    ("text_ghost", WHITE_15),
    ("text_on_accent", ON_GREEN),
    ("accent", GREEN),
    ("accent_hover", GREEN_80),
    ("accent_pressed", GREEN_60),
    ("accent_bg", GREEN_20),
    ("focus", GREEN),
    ("disabled", WHITE_30),
    ("error", RED),
    ("warning", AMBER),
    ("success", GREEN),
];

/// `(role, fg, bg, modifiers)` the canonical palette must resolve to (D2).
const JUNIE_ROLE_TABLE: &[(Role, Option<Color>, Option<Color>, &[Modifier])] = &[
    (Role::Canvas, None, Some(BLACK), &[]),
    (Role::Surface, None, Some(CHROME), &[]),
    (Role::Elevated, None, Some(CARD), &[]),
    (Role::Sunken, None, Some(INPUT), &[]),
    (Role::Popover, None, Some(POPOVER), &[]),
    // `Role::Backdrop` is deliberately absent from this literal table: D2
    // defines it as "per `backdrop()`", so it is pinned compositionally against
    // the resolver in `backdrop_role_is_the_dimmed_page`, not against a hex.
    (Role::Text, Some(WHITE), None, &[]),
    (Role::TextStrong, Some(WHITE), None, &[Modifier::BOLD]),
    (Role::TextSecondary, Some(WHITE_70), None, &[]),
    (Role::TextMuted, Some(WHITE_50), None, &[]),
    (Role::TextFaint, Some(WHITE_30), None, &[]),
    (Role::TextGhost, Some(WHITE_15), None, &[]),
    (Role::TextDisabled, Some(WHITE_30), None, &[]),
    (Role::TextOnAccent, Some(ON_GREEN), None, &[]),
    (Role::Border, Some(WHITE_15), None, &[]),
    (Role::BorderFocused, Some(WHITE_30), None, &[]),
    (Role::Selection, Some(WHITE), Some(POPOVER), &[]),
    (Role::SelectionTint, None, Some(GREEN_20), &[]),
    (Role::Focus, Some(GREEN), None, &[]),
    (Role::Accent, Some(GREEN), None, &[]),
    (Role::Success, Some(GREEN), None, &[]),
    (Role::Warning, Some(AMBER), None, &[]),
    (Role::Danger, Some(RED), None, &[]),
    (Role::Link, Some(WHITE_70), None, &[Modifier::UNDERLINED]),
    (Role::LinkHover, Some(WHITE), None, &[Modifier::UNDERLINED]),
    (Role::Input, Some(WHITE), Some(INPUT), &[]),
    (Role::InputInvalid, Some(RED), Some(INPUT), &[]),
    (Role::ScrollTrack, Some(WHITE_15), None, &[]),
    (Role::ScrollThumb, Some(WHITE_50), None, &[]),
    (Role::TabActive, Some(WHITE), None, &[Modifier::BOLD]),
    (Role::TabInactive, Some(WHITE_70), None, &[]),
    (Role::HintKey, Some(WHITE), None, &[Modifier::BOLD]),
    (Role::HintText, Some(WHITE_50), None, &[]),
    (Role::HintDim, Some(WHITE_30), None, &[]),
    (Role::HintSeparator, Some(WHITE_15), None, &[]),
    (Role::ActionFocused, Some(GREEN), None, &[Modifier::BOLD]),
    (Role::ActionDisabled, Some(WHITE_30), None, &[]),
    (Role::StatusBar, Some(WHITE_70), Some(BLACK), &[]),
    // D3 derivations: diff, syntax, actors, charts walk the ladder — never hue.
    (Role::DiffAdded, Some(WHITE_70), None, &[]),
    (Role::DiffRemoved, Some(WHITE_50), None, &[]),
    (Role::SyntaxKeyword, Some(WHITE), None, &[Modifier::BOLD]),
    (Role::SyntaxString, Some(WHITE_70), None, &[]),
    (
        Role::SyntaxComment,
        Some(WHITE_30),
        None,
        &[Modifier::ITALIC],
    ),
    (Role::SyntaxNumber, Some(WHITE_70), None, &[]),
    (Role::SyntaxFunction, Some(WHITE), None, &[]),
    (Role::ActorUser, Some(WHITE), None, &[]),
    (Role::ActorAssistant, Some(WHITE), None, &[]),
    (Role::ActorThinking, Some(WHITE_50), None, &[]),
    (Role::ActorTool, Some(WHITE), None, &[]),
    (Role::ActorPlan, Some(WHITE), None, &[]),
    (Role::ActorSystem, Some(WHITE_50), None, &[]),
    (Role::ChartSeries1, Some(WHITE), None, &[]),
    (Role::ChartSeries2, Some(WHITE_70), None, &[]),
    (Role::ChartSeries3, Some(WHITE_50), None, &[]),
    (Role::ChartSeries4, Some(WHITE_30), None, &[]),
    (Role::ChartAxis, Some(WHITE_50), None, &[]),
    (Role::ChartGrid, Some(WHITE_15), None, &[]),
    (
        Role::Highlight,
        Some(WHITE),
        Some(HIGHLIGHT),
        &[Modifier::BOLD],
    ),
    (
        Role::HighlightDanger,
        Some(WHITE),
        Some(HIGHLIGHT_DANGER),
        &[Modifier::BOLD],
    ),
    (Role::ErrorSoft, Some(ERROR_SOFT), None, &[]),
];

/// `phosphor_baseline_uses_named_ansi_only` rewritten (D9): junie is the only
/// palette, and its truecolor token table is the reference table exactly.
#[test]
fn junie_truecolor_tokens_exact() {
    let theme = JunieTheme::junie();
    assert_eq!(theme.level, ColorCapability::Truecolor);

    let actual = [
        ("canvas", theme.canvas),
        ("surface", theme.surface),
        ("surface_elevated", theme.surface_elevated),
        ("surface_overlay", theme.surface_overlay),
        ("field", theme.field),
        ("field_hover", theme.field_hover),
        ("popover", theme.popover),
        ("border_subtle", theme.border_subtle),
        ("border_strong", theme.border_strong),
        ("text_primary", theme.text_primary),
        ("text_secondary", theme.text_secondary),
        ("text_muted", theme.text_muted),
        ("text_faint", theme.text_faint),
        ("text_ghost", theme.text_ghost),
        ("text_on_accent", theme.text_on_accent),
        ("accent", theme.accent),
        ("accent_hover", theme.accent_hover),
        ("accent_pressed", theme.accent_pressed),
        ("accent_bg", theme.accent_bg),
        ("focus", theme.focus),
        ("disabled", theme.disabled),
        ("error", theme.error),
        ("warning", theme.warning),
        ("success", theme.success),
    ];
    assert_eq!(actual.len(), JUNIE_TOKEN_TABLE.len());
    for ((name, actual), (expected_name, expected)) in actual.iter().zip(JUNIE_TOKEN_TABLE) {
        assert_eq!(*name, *expected_name, "token table drifted out of order");
        assert_eq!(*actual, *expected, "token {name} left the reference value");
    }

    // And the shipped system is that theme, not a restyled derivative.
    let system = DesignSystem::default();
    assert_eq!(system.junie_theme(), JunieTheme::junie());
    assert_eq!(system.palette(), &RolePalette::junie());
}

/// The palette's 57 roles are pinned exactly: junie tokens plus the derived
/// buckets of D3 (diff, syntax, actors, charts, links) which walk the ladder
/// instead of inventing hue.
#[test]
fn junie_role_palette_exact() {
    let palette = RolePalette::junie();
    for (role, fg, bg, mods) in JUNIE_ROLE_TABLE {
        let style = palette.style(*role);
        assert_eq!(style.fg, *fg, "{role:?} foreground");
        assert_eq!(style.bg, *bg, "{role:?} background");
        let expected = mods.iter().fold(Modifier::empty(), |acc, m| acc | *m);
        assert_eq!(style.add_modifier, expected, "{role:?} modifiers");
    }
    // No role escapes the table: the palette is exactly the canonical set,
    // except the one role D2 defines compositionally.
    assert_eq!(JUNIE_ROLE_TABLE.len(), RolePalette::roles().len() - 1);
    for role in RolePalette::roles() {
        if role == Role::Backdrop {
            continue;
        }
        assert!(
            JUNIE_ROLE_TABLE.iter().any(|(r, _, _, _)| r == &role),
            "{role:?} is not pinned by the gate table"
        );
    }
}

/// D2: `Backdrop` is "per `backdrop()`" — the role is the base page run
/// through the dimming resolver, not an independent hex.
#[test]
fn backdrop_role_is_the_dimmed_page() {
    let theme = JunieTheme::junie();
    let palette = RolePalette::junie();
    assert_eq!(
        palette.style(Role::Backdrop),
        theme.backdrop(theme.base()),
        "Backdrop must be backdrop() applied to the base page"
    );
}

/// `canvas_uses_terminal_default_fill` rewritten (D9): the canvas is junie's
/// `#000000` ground and base text is white on it.
#[test]
fn canvas_is_junie_black() {
    let theme = JunieTheme::junie();
    assert_eq!(theme.canvas, BLACK);
    let base = theme.base();
    assert_eq!(base.fg, Some(WHITE));
    assert_eq!(base.bg, Some(BLACK));
    // Every surface the system ships rests on that ground unless it says so.
    let palette = RolePalette::junie();
    assert_eq!(palette.style(Role::Canvas).bg, Some(BLACK));
    assert_eq!(palette.style(Role::StatusBar).bg, Some(BLACK));
}

/// Capability vectors (D1/D9): every token is downgraded by the reference
/// algorithm at construction time, and the algorithm's output is exact.
#[test]
fn capability_downgrade_vectors_exact() {
    use ColorCapability::{Ansi16, Indexed256, Monochrome, Truecolor};

    // (name, truecolor token, 256 index, 16 name, mono bucket).
    let vectors: &[(&str, Color, u8, Color, Color)] = &[
        ("BLACK", BLACK, 16, Color::Black, Color::Black),
        ("CHROME", CHROME, 232, Color::Black, Color::Black),
        ("CARD", CARD, 233, Color::Black, Color::Black),
        ("INPUT", INPUT, 234, Color::Black, Color::Black),
        (
            "INPUT_HOVER",
            INPUT_HOVER,
            234,
            Color::DarkGray,
            Color::Black,
        ),
        ("OVERLAY", OVERLAY, 235, Color::DarkGray, Color::Black),
        ("POPOVER", POPOVER, 237, Color::DarkGray, Color::DarkGray),
        ("WHITE", WHITE, 231, Color::White, Color::White),
        ("WHITE_70", WHITE_70, 249, Color::Gray, Color::Gray),
        ("WHITE_50", WHITE_50, 244, Color::Gray, Color::Gray),
        ("WHITE_30", WHITE_30, 238, Color::DarkGray, Color::DarkGray),
        ("WHITE_15", WHITE_15, 235, Color::DarkGray, Color::Black),
        ("GREEN", GREEN, 78, Color::LightGreen, Color::Gray),
        ("GREEN_80", GREEN_80, 77, Color::Green, Color::DarkGray),
        ("GREEN_60", GREEN_60, 238, Color::Green, Color::DarkGray),
        ("GREEN_20", GREEN_20, 233, Color::DarkGray, Color::Black),
        ("ON_GREEN", ON_GREEN, 233, Color::Black, Color::Black),
        ("RED", RED, 167, Color::LightRed, Color::Gray),
        ("AMBER", AMBER, 214, Color::Yellow, Color::Gray),
    ];
    for (name, token, indexed, ansi, mono) in vectors {
        assert_eq!(
            downgrade(*token, Indexed256),
            Color::Indexed(*indexed),
            "{name} @256"
        );
        assert_eq!(downgrade(*token, Ansi16), *ansi, "{name} @16");
        assert_eq!(downgrade(*token, Monochrome), *mono, "{name} in mono");
        assert_eq!(downgrade(*token, Truecolor), *token, "{name} in truecolor");
    }

    // The decision file's named vectors stay exact.
    assert_eq!(downgrade(WHITE_15, Ansi16), Color::DarkGray);
    assert_eq!(downgrade(AMBER, Ansi16), Color::Yellow);
    assert_eq!(downgrade(GREEN, Indexed256), Color::Indexed(78));
    assert_eq!(downgrade(CHROME, Indexed256), Color::Indexed(232));

    // Named and indexed colours are already resolved by the terminal; the
    // algorithm must leave them alone.
    for already in [Color::Green, Color::Indexed(78), Color::Reset] {
        for level in [Truecolor, Indexed256, Ansi16, Monochrome] {
            assert_eq!(downgrade(already, level), already);
        }
    }

    // The whole theme is downgraded, not a hand-picked subset.
    let sixteen = JunieTheme::for_level(Ansi16);
    assert_eq!(sixteen.accent, Color::LightGreen);
    assert_eq!(sixteen.error, Color::LightRed);
    assert_eq!(sixteen.canvas, Color::Black);
    let twofiftysix = JunieTheme::for_level(Indexed256);
    assert_eq!(twofiftysix.accent, Color::Indexed(78));
    assert_eq!(twofiftysix.canvas, Color::Indexed(16));
    let mono = JunieTheme::for_level(Monochrome);
    assert_eq!(mono.text_primary, Color::White);
    assert_eq!(mono.canvas, Color::Black);
    assert_eq!(JunieTheme::for_level(Truecolor), JunieTheme::junie());
}

/// `nearest_256` / `nearest_16` are the reference algorithms, so a re-derivation
/// from the raw channels agrees with the token-level vectors above.
#[test]
fn downgrade_helpers_agree_with_the_reference_math() {
    assert_eq!(nearest_256(0x48, 0xe0, 0x54), 78);
    assert_eq!(nearest_256(0x11, 0x11, 0x11), 232);
    assert_eq!(nearest_256(0xff, 0xff, 0xff), 231);
    assert_eq!(nearest_16(0x26, 0x26, 0x26), Color::DarkGray);
    assert_eq!(nearest_16(0xf5, 0x9e, 0x09), Color::Yellow);
    assert_eq!(nearest_16(0x48, 0xe0, 0x54), Color::LightGreen);
    assert_eq!(nearest_16(0x00, 0x00, 0x00), Color::Black);
}

/// The monochrome rung is four grey buckets — no `REVERSED` substitution, no
/// per-call-site special case (D1/D9).
#[test]
fn mono_palette_is_four_grey_buckets() {
    let palette = RolePalette::junie_for(ColorCapability::Monochrome);
    let buckets = [Color::Black, Color::DarkGray, Color::Gray, Color::White];
    for role in RolePalette::roles() {
        let style = palette.style(role);
        for color in [style.fg, style.bg].into_iter().flatten() {
            assert!(
                buckets.contains(&color),
                "{role:?} paints {color:?} in monochrome; only the four grey buckets exist"
            );
        }
    }
    // Hierarchy survives the collapse: the ladder stays ordered.
    assert_ne!(
        palette.style(Role::Text).fg,
        palette.style(Role::TextMuted).fg
    );
    assert_ne!(
        palette.style(Role::TextMuted).fg,
        palette.style(Role::TextGhost).fg
    );
}

/// `phosphor_surfaces_keep_semantic_elevation` / the ladder floor gate
/// rewritten (D9): the rungs are *canonically* close, so the gate pins the
/// measured ratios instead of demanding a floor.
#[test]
fn surface_ladder_steps_are_canonical() {
    let measure = |a: Color, b: Color| {
        let (Some(x), Some(y)) = (rgb_of(a), rgb_of(b)) else {
            panic!("ladder rungs must be truecolor to be measured");
        };
        contrast_ratio(x, y)
    };
    let expected = [
        ("canvas -> surface", BLACK, CHROME, 1.11),
        ("surface -> elevated", CHROME, CARD, 1.07),
        ("elevated -> overlay", CARD, OVERLAY, 1.19),
        ("overlay -> popover", OVERLAY, POPOVER, 1.43),
        ("surface -> field", CHROME, INPUT, 1.14),
        ("field -> field hover", INPUT, INPUT_HOVER, 1.06),
    ];
    for (name, a, b, exact) in expected {
        let measured = measure(a, b);
        assert!(
            (measured - exact).abs() < 0.01,
            "{name} measures {measured:.2}, the canonical step is {exact}"
        );
    }

    // Text tiers keep their distances too: white is the maximum contrast the
    // medium has, and every lower tier is exactly one alpha step down.
    let tiers = [
        ("primary on canvas", WHITE, BLACK, 21.00),
        ("secondary on canvas", WHITE_70, BLACK, 10.02),
        ("muted on canvas", WHITE_50, BLACK, 5.32),
        ("faint on canvas", WHITE_30, BLACK, 2.48),
        ("secondary on surface", WHITE_70, CHROME, 9.01),
        ("muted on surface", WHITE_50, CHROME, 4.78),
        ("white on field", WHITE, INPUT, 16.61),
        ("white on popover", WHITE, POPOVER, 10.44),
    ];
    for (name, fg, bg, exact) in tiers {
        let measured = measure(fg, bg);
        assert!(
            (measured - exact).abs() < 0.01,
            "{name} measures {measured:.2}, the canonical value is {exact}"
        );
    }
}

fn rgb_of(color: Color) -> Option<termrock::style::Rgb> {
    termrock::style::Rgb::from_color(color)
}

/// `contrast_floor KNOWN_SHORTFALLS` rewritten (D9): junie declares four sub-AA
/// pairings and the gate measures them in-repo, so a token that drifts moves a
/// measured number rather than silently entering the exemption list.
#[test]
fn contrast_known_shortfalls_are_the_declared_four() {
    const AA_TEXT: f32 = 4.5;
    let shortfalls = [
        // Primary pressed: on-accent text over the pressed accent.
        ("primary pressed", ON_GREEN, GREEN_60, 3.81),
        // Danger label resting: error tone over the overlay plane.
        ("danger label", RED, OVERLAY, 3.71),
        // Danger pressed: body text over the error fill.
        ("danger pressed", WHITE, RED, 4.01),
        // Placeholder: muted text in the field body.
        ("placeholder", WHITE_50, INPUT, 4.21),
    ];
    for (name, fg, bg, measured) in shortfalls {
        let ratio = contrast_ratio(
            rgb_of(fg).expect("truecolor"),
            rgb_of(bg).expect("truecolor"),
        );
        assert!(
            ratio < AA_TEXT,
            "{name} measures {ratio:.2}; a declared shortfall must stay below AA"
        );
        assert!(
            (ratio - measured).abs() < 0.01,
            "{name} measures {ratio:.2}, the declared value is {measured}"
        );
    }

    // Everything a reader must read clears AA: body, secondary, titles.
    let clear = [
        ("body on canvas", WHITE, BLACK),
        ("body on surface", WHITE, CHROME),
        ("body on field", WHITE, INPUT),
        ("secondary on surface", WHITE_70, CHROME),
        ("selection text", WHITE, POPOVER),
        ("selected row on tint", WHITE, GREEN_20),
    ];
    for (name, fg, bg) in clear {
        let ratio = contrast_ratio(
            rgb_of(fg).expect("truecolor"),
            rgb_of(bg).expect("truecolor"),
        );
        assert!(ratio >= AA_TEXT, "{name} measures {ratio:.2} < AA");
    }
}

/// The resolvers are the reference resolvers: tone never borrows the accent,
/// syntax is the ladder plus weight, and the ` EDIT ` badge is the only badge.
#[test]
fn junie_resolvers_keep_the_reference_behaviour() {
    let theme = JunieTheme::junie();

    // `tone()` maps to the ladder and the three semantic colours, never accent.
    assert_eq!(theme.tone(Tone::Normal), WHITE);
    assert_eq!(theme.tone(Tone::Secondary), WHITE_70);
    assert_eq!(theme.tone(Tone::Muted), WHITE_50);
    assert_eq!(theme.tone(Tone::Faint), WHITE_30);
    assert_eq!(theme.tone(Tone::Error), RED);
    assert_eq!(theme.tone(Tone::Warning), AMBER);
    assert_eq!(theme.tone(Tone::Success), GREEN);

    // Syntax: structure through weight, comment in the faint italic tier.
    let keyword = theme.syntax(SyntaxTone::Keyword);
    assert_eq!(keyword.fg, Some(WHITE));
    assert!(keyword.add_modifier.contains(Modifier::BOLD));
    assert_eq!(theme.syntax(SyntaxTone::Ident).fg, Some(WHITE));
    assert_eq!(theme.syntax(SyntaxTone::Str).fg, Some(WHITE_70));
    assert_eq!(theme.syntax(SyntaxTone::Number).fg, Some(WHITE_70));
    assert_eq!(theme.syntax(SyntaxTone::Operator).fg, Some(WHITE_50));
    assert_eq!(theme.syntax(SyntaxTone::Punct).fg, Some(WHITE_50));
    let comment = theme.syntax(SyntaxTone::Comment);
    assert_eq!(comment.fg, Some(WHITE_30));
    assert_eq!(comment.add_modifier, Modifier::ITALIC);

    // Badge: one badge, on-accent, weight carries the "you are editing". The
    // match has no wildcard arm on purpose: a second `BadgeKind` variant stops
    // compiling here instead of passing a gate it never read.
    const BADGE_KIND_COUNT: usize = match BadgeKind::Edit {
        BadgeKind::Edit => 1,
    };
    assert_eq!(
        BADGE_KIND_COUNT, 1,
        "a second badge kind entered the system; junie has exactly one"
    );
    let badge = theme.badge(BadgeKind::Edit);
    assert_eq!(badge.fg, Some(ON_GREEN));
    assert_eq!(badge.bg, Some(GREEN));
    assert_eq!(badge.add_modifier, Modifier::BOLD);

    // Text helpers: weight is the title, the focused label, and the hint key.
    assert_eq!(theme.title().add_modifier, Modifier::BOLD);
    assert_eq!(theme.label(true).add_modifier, Modifier::BOLD);
    assert_eq!(theme.label(false).fg, Some(WHITE_70));
    assert_eq!(theme.key_hint_key().add_modifier, Modifier::BOLD);
    assert_eq!(theme.key_hint_action().fg, Some(WHITE_50));

    // Scrollbar: track subtle, thumb brightens with interaction.
    assert_eq!(theme.scrollbar_track().fg, Some(WHITE_15));
    assert_eq!(theme.scrollbar_thumb(true, false).fg, Some(WHITE));
    assert_eq!(theme.scrollbar_thumb(false, true).fg, Some(WHITE_70));
    assert_eq!(theme.scrollbar_thumb(false, false).fg, Some(WHITE_50));

    // Modal dimming keeps the page's shape and collapses the ladder.
    assert_eq!(theme.backdrop(theme.base()).bg, Some(BLACK));
    assert_eq!(
        theme
            .backdrop(ratatui_core::style::Style::new().bg(theme.field))
            .bg,
        Some(CARD)
    );
    assert_eq!(
        theme
            .backdrop(ratatui_core::style::Style::new().fg(theme.accent))
            .fg,
        Some(WHITE_50)
    );
}

// ── B. Modifier law (D5) ─────────────────────────────────────────────────────

/// D5: the only modifiers in the system are BOLD, ITALIC (comments), UNDERLINED
/// (the three-colour law), and CROSSED_OUT (deleted rows). `DIM` is banned, and
/// `Modifier::REVERSED` is banned with it — a reversal is painted explicitly as
/// `fg(canvas).bg(text_primary)`.
///
/// This scan is the enforcement arm of that law. A defensive
/// `remove_modifier` strip is not a paint site — it can never add a modifier —
/// and a legal ITALIC/CROSSED_OUT site must name its class on the line so the
/// scan can hold it there.
#[test]
fn modifiers_are_the_junie_set() {
    let mut offenders: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (line_no, line) in &source.lines {
            if line.contains("remove_modifier") {
                continue;
            }
            if line.contains("Modifier::DIM") {
                offenders.push(format!("{name}:{line_no}: DIM (D5 ban)"));
            }
            if line.contains("Modifier::REVERSED") {
                offenders.push(format!(
                    "{name}:{line_no}: REVERSED — paint the reversal as fg(canvas).bg(text_primary)"
                ));
            }
            // ITALIC is comments only; CROSSED_OUT is deleted rows only.
            for (glyph, class) in [("ITALIC", "comment"), ("CROSSED_OUT", "deleted row")] {
                if line.contains(glyph) && !line.to_lowercase().contains(class) {
                    offenders.push(format!(
                        "{name}:{line_no}: {glyph} must name its D5 class ({class})"
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "modifier law violations (phase3-decision.md D5):\n{}",
        offenders.join("\n")
    );
}

/// The palette carries the law too: weight and the comment italic are the only
/// modifiers it ships, and the one underline belongs to the link affordance.
#[test]
fn palette_modifiers_are_bold_underline_and_comment_italic() {
    let palette = RolePalette::junie();
    for role in RolePalette::roles() {
        let style = palette.style(role);
        let mods = style.add_modifier;
        assert!(!mods.contains(Modifier::DIM), "{role:?} dims (D5 ban)");
        assert!(
            !mods.contains(Modifier::REVERSED),
            "{role:?} uses Modifier::REVERSED; a reversal is painted explicitly"
        );
        assert!(
            !mods.contains(Modifier::CROSSED_OUT),
            "{role:?} strikes through; CROSSED_OUT belongs to deleted rows"
        );
        if mods.contains(Modifier::ITALIC) {
            assert_eq!(
                role,
                Role::SyntaxComment,
                "{role:?} is italic; ITALIC is the comment tier"
            );
        }
        if mods.contains(Modifier::UNDERLINED) {
            assert!(
                matches!(role, Role::Link | Role::LinkHover),
                "{role:?} underlines; the palette reserves underline for the link affordance"
            );
        }
    }
    // Crossed out stays available to the diff chrome and nothing else.
    assert!(
        !palette
            .style(Role::DiffRemoved)
            .add_modifier
            .contains(Modifier::CROSSED_OUT)
    );
}

/// D8/D5: pressed is the explicit reversal `fg(canvas).bg(text_primary)` —
/// never a modifier, and never a per-kind improvisation.
#[test]
fn pressed_is_an_explicit_reversal() {
    let theme = JunieTheme::junie();
    for ground in [BLACK, CHROME, CARD, INPUT] {
        let pressed = theme.row(
            VisualState {
                pressed: true,
                ..VisualState::default()
            },
            ground,
        );
        assert_eq!(pressed.fg, Some(BLACK), "row pressed on {ground:?}");
        assert_eq!(pressed.bg, Some(WHITE), "row pressed on {ground:?}");
        assert!(pressed.add_modifier.contains(Modifier::BOLD));
        assert!(!pressed.add_modifier.contains(Modifier::REVERSED));
    }

    // Secondary, toggle and subtle buttons reverse; danger presses into its own
    // fill; primary presses down the accent ramp.
    let system = DesignSystem::junie();
    let reversed = [
        ButtonKind::Secondary,
        ButtonKind::Toggle,
        ButtonKind::Subtle,
    ];
    for kind in reversed {
        let pressed = theme.button(
            kind,
            VisualState {
                pressed: true,
                ..VisualState::default()
            },
            CHROME,
        );
        assert_eq!(pressed.fg, Some(BLACK), "{kind:?} pressed");
        assert_eq!(pressed.bg, Some(WHITE), "{kind:?} pressed");
        assert!(!pressed.add_modifier.contains(Modifier::REVERSED));
    }
    let danger = theme.button(
        ButtonKind::Danger,
        VisualState {
            pressed: true,
            ..VisualState::default()
        },
        CHROME,
    );
    assert_eq!(danger.fg, Some(WHITE));
    assert_eq!(danger.bg, Some(RED));
    let primary = theme.button(
        ButtonKind::Primary,
        VisualState {
            pressed: true,
            ..VisualState::default()
        },
        CHROME,
    );
    assert_eq!(primary.fg, Some(ON_GREEN));
    assert_eq!(primary.bg, Some(GREEN_60));

    // The recipe the widget reads agrees, and the field cursor is a cell.
    for variant in [ButtonRecipeVariant::Secondary, ButtonRecipeVariant::Quiet] {
        let recipe = system.button_recipe(variant, ControlState::Pressed, CHROME);
        assert_eq!(recipe.label.fg, Some(BLACK), "{variant:?} pressed");
        assert_eq!(recipe.fill.bg, Some(WHITE), "{variant:?} pressed");
        assert!(!recipe.label.add_modifier.contains(Modifier::REVERSED));
    }
    let input = system.input_recipe(ControlState::Focused, false, false);
    assert_eq!(input.cursor.fg, Some(BLACK));
    assert_eq!(input.cursor.bg, Some(WHITE));
}

// ── C. Glyph law (D6) ────────────────────────────────────────────────────────

/// The one vocabulary, pinned glyph by glyph. A second vocabulary (`Ascii`,
/// `Enhanced`) is deleted, so there is nothing to switch between.
#[test]
fn glyph_vocabulary_is_the_junie_table() {
    const TABLE: &[&str] = &[
        // directional
        "▾", "▸", "‹", "›", "▴", "▾", "→", "↓", // status / choice
        "✓", "•", "!", "·", "⠋", "[✓]", "[ ]", "●", "○", // action
        "×", "+", "−", // rules
        "─", "│", "━", // selection chrome
        "▎", "›", // meta
        "•", "·", "…", // marks
        "●", "○", "◆", "┃", "◇", "●", // slider
        "●", "━", "─", // dividers
        "│", "┃", "┃", "─", "━", "━",
    ];
    assert_eq!(
        Glyph::ALL.len(),
        TABLE.len(),
        "the glyph catalog changed size"
    );
    for (glyph, expected) in Glyph::ALL.iter().zip(TABLE) {
        assert_eq!(glyph.resolve().text, *expected, "{} encoding", glyph.id());
        assert!(
            !glyph.meaning().is_empty(),
            "{} must carry a meaning",
            glyph.id()
        );
    }
    // One glyph, one concept: the checkbox pair is the only multi-cell entry.
    for glyph in Glyph::ALL {
        let resolved = glyph.resolve();
        if matches!(glyph, Glyph::CheckOn | Glyph::CheckOff) {
            assert_eq!(resolved.cols, 3);
        } else {
            assert_eq!(resolved.cols, 1, "{} must be one column", glyph.id());
            assert_eq!(display_cols(resolved.text), 1);
        }
    }
    // `GlyphSet` is the one junie vocabulary, not a profile switch.
    assert_eq!(GlyphSet.id(), "junie");
    assert_eq!(GlyphSet::default().id(), "junie");
    // junie's context law: co-occurring glyphs are documented, not unique.
    assert!(!termrock::style::GLYPH_CONTEXTS.is_empty());
    for (context, glyphs) in termrock::style::GLYPH_CONTEXTS {
        assert!(!context.is_empty());
        assert!(!glyphs.is_empty(), "{context} lists no glyphs");
    }
    assert!(termrock::style::glyph_by_id("selection-gutter").is_some());
}

/// The focus gutter is the haired bar `▎` (D6: `▌`→`▎`, `❯` deleted).
#[test]
fn focus_bar_is_the_haired_gutter() {
    let system = DesignSystem::junie();
    assert_eq!(system.glyphs.selection_gutter(), "▎");
    let theme = JunieTheme::junie();
    // Unfocused gutter is invisible (painted in its own ground); the focused
    // gutter is the focus green; on an accent fill it is body text.
    let idle = theme.gutter(VisualState::default(), CHROME, false);
    assert_eq!(idle.fg, Some(CHROME));
    let owned = theme.gutter(
        VisualState {
            focused: true,
            ..VisualState::default()
        },
        CHROME,
        false,
    );
    assert_eq!(owned.fg, Some(GREEN));
    let on_accent = theme.gutter(
        VisualState {
            focused: true,
            ..VisualState::default()
        },
        GREEN,
        true,
    );
    assert_eq!(on_accent.fg, Some(WHITE));
}

/// Deleted glyph variants must not come back in paint (D6 delete list).
#[test]
fn deleted_glyphs_are_gone_from_paint() {
    const BANNED: [(&str, &str); 6] = [
        ("▌", "heavy block — the focus bar is ▎"),
        ("❯", "prompt chevron — deleted, use ▎"),
        ("✕", "ballot × — the close glyph is ×"),
        ("☑", "ballot box — the checkbox pair is [✓]/[ ]"),
        ("☐", "ballot box — the checkbox pair is [✓]/[ ]"),
        ("═", "double rule — the rule is ─/━"),
    ];
    let mut offenders: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (line_no, line) in &source.lines {
            for (glyph, why) in BANNED {
                if line.contains(glyph) {
                    offenders.push(format!("{name}:{line_no}: {glyph} — {why}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "glyphs with no junie counterpart are still painted (D6):\n{}",
        offenders.join("\n")
    );
}

/// `BorderShape::Rounded` is canonical and there is no second shape (D6/D7).
#[test]
fn borders_are_rounded() {
    let system = DesignSystem::junie();
    let set = system.border_set();
    assert_eq!(set.top_left, "╭");
    assert_eq!(set.top_right, "╮");
    assert_eq!(set.bottom_left, "╰");
    assert_eq!(set.bottom_right, "╯");
    assert_eq!(set.horizontal_top, "─");
    assert_eq!(set.horizontal_bottom, "─");
    assert_eq!(set.vertical_left, "│");
    assert_eq!(set.vertical_right, "│");

    match BorderShape::Rounded {
        BorderShape::Rounded => {}
        _ => panic!("a second border shape reappeared (D7 deletes Square)"),
    }
}

/// `collections_share_one_gutter_glyph` rewritten to the junie vocabulary (D9):
/// the shared marker is `▎`, and the gutter column is the cursor's voice.
#[test]
fn collections_share_one_gutter_glyph() {
    use termrock::widgets::{
        Column, ColumnWidth, Table, TableRow, TableState, Timeline, TimelineEvent, Tree, TreeNode,
        TreeState,
    };

    let system = DesignSystem::junie();
    let gutter = system.glyphs.selection_gutter();
    assert_eq!(
        gutter, "▎",
        "the vocabulary is pinned above; keep this in sync"
    );
    let area = Rect::new(0, 0, 28, 4);

    // List
    let rows = rows();
    let mut list_state = ListState::new(Some("beta"));
    let mut list_buffer = Buffer::empty(area);
    (&List::new(&rows, &system)).render(area, &mut list_buffer, &mut list_state);
    let list_row = list_state
        .regions()
        .iter()
        .find(|r| r.id == "beta")
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        list_buffer[(list_row.x, list_row.y)].symbol(),
        gutter,
        "List"
    );

    // Table
    let columns = [Column::new("name", "Name", ColumnWidth::Fixed(10))];
    let alpha = [Line::from("alpha")];
    let beta = [Line::from("beta")];
    let table_rows = [TableRow::new(0u8, &alpha), TableRow::new(1u8, &beta)];
    let mut table_state = TableState::new(Some(1u8));
    let mut table_buffer = Buffer::empty(area);
    (&Table::new(&columns, &table_rows, &system)).render(area, &mut table_buffer, &mut table_state);
    let table_row = table_state
        .row_regions
        .iter()
        .find(|r| r.id == 1u8)
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        table_buffer[(table_row.x, table_row.y)].symbol(),
        gutter,
        "Table"
    );

    // Tree
    let nodes = vec![
        TreeNode::new("root", Line::from("Workspace"), 0),
        TreeNode::new("leaf", Line::from("File"), 1),
    ];
    let mut tree_state = TreeState::new(Some("leaf"));
    let mut tree_buffer = Buffer::empty(area);
    Tree::new(&nodes, &system).render(area, &mut tree_buffer, &mut tree_state);
    let tree_row = tree_state
        .regions()
        .iter()
        .find(|r| r.id == "leaf")
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        tree_buffer[(tree_row.x, tree_row.y)].symbol(),
        gutter,
        "Tree"
    );

    // Timeline
    let events = [
        TimelineEvent::new("12:01", "Started"),
        TimelineEvent::new("12:02", "Running"),
    ];
    let mut timeline_buffer = Buffer::empty(area);
    Widget::render(&Timeline::new(&events, &system), area, &mut timeline_buffer);
    assert_eq!(
        timeline_buffer[(area.x, area.y)].symbol(),
        gutter,
        "Timeline"
    );
}

// ── D. Spacing and motion (D7) ───────────────────────────────────────────────

/// `Density` is deleted; the scale is junie's named tokens as constants.
#[test]
fn spacing_is_the_junie_scale() {
    let expected = [
        ("gutter", 1u16),
        ("inline", 1),
        ("gap", 2),
        ("column_gap", 2),
        ("form_gap", 4),
        ("card_inset", 2),
        ("frame_inset", 3),
        ("dialog_inset", 3),
        ("tree_indent", 2),
        ("field_height", 3),
        ("tabs_height", 2),
        ("min_width", 72),
        ("min_height", 20),
    ];
    let scale = SpacingScale::junie();
    let actual = [
        ("gutter", scale.gutter),
        ("inline", scale.inline),
        ("gap", scale.gap),
        ("column_gap", scale.column_gap),
        ("form_gap", scale.form_gap),
        ("card_inset", scale.card_inset),
        ("frame_inset", scale.frame_inset),
        ("dialog_inset", scale.dialog_inset),
        ("tree_indent", scale.tree_indent),
        ("field_height", scale.field_height),
        ("tabs_height", scale.tabs_height),
        ("min_width", scale.min_width),
        ("min_height", scale.min_height),
    ];
    for ((name, expected), (actual_name, actual)) in expected.iter().zip(actual) {
        assert_eq!(*name, actual_name);
        assert_eq!(*expected, actual, "spacing token {name} drifted");
    }
    // There is no second scale to pick.
    assert_eq!(SpacingScale::default(), SpacingScale::junie());
    assert_eq!(DesignSystem::default().spacing, SpacingScale::junie());
    // Rhythm is surrendered before content is.
    assert_eq!(scale.band().rows, 1);
    assert_eq!(scale.band().resolve(10, 9), 1);
    assert_eq!(scale.band().resolve(10, 10), 0);
}

/// `{Full, Off}` is the whole motion vocabulary, and `Off` never animates
/// (D7/D9) — not even a "brief transition".
#[test]
fn motion_policy_has_two_states_and_off_never_animates() {
    match MotionPolicy::Full {
        MotionPolicy::Full | MotionPolicy::Off => {}
        _ => panic!("a third motion tier reappeared (D7)"),
    }
    assert!(MotionPolicy::Full.animate_spinners());
    assert!(MotionPolicy::Full.allows_transitions());
    assert!(!MotionPolicy::Off.animate_spinners());
    assert!(!MotionPolicy::Off.allows_transitions());
    // The frame index itself parks on frame zero.
    let start = Instant::now();
    let late = FrameTick::manual(
        start + Duration::from_millis(4_000),
        Duration::from_millis(4_000),
        Duration::from_millis(16),
    );
    assert_eq!(
        late.spinner_step(SPINNER_FRAMES.len(), SPINNER_PERIOD_MS, MotionPolicy::Off),
        0
    );

    // And no recipe family claims an animation `Off` may run.
    let system = DesignSystem::junie();
    for family in RecipeFamily::ALL {
        let motion = system.family_recipe(family).motion;
        assert!(
            !motion.animates(MotionPolicy::Off),
            "{family:?} animates under MotionPolicy::Off"
        );
    }
}

/// `family_motion_semantics_respect_reduced_motion` rewritten (D9): motion is
/// per family under one policy — activity moves when `Full` is allowed, data
/// never moves at all.
#[test]
fn family_motion_semantics_respect_reduced_motion() {
    let system = DesignSystem::junie();
    let action = system.family_recipe(RecipeFamily::Action).motion;
    let status = system.family_recipe(RecipeFamily::Status).motion;
    let data = system.family_recipe(RecipeFamily::Data).motion;

    assert!(action.animates(MotionPolicy::Full));
    assert!(status.animates(MotionPolicy::Full));
    assert!(!data.animates(MotionPolicy::Full));
    // `Off` answers are already proven above; repeat the family law here so a
    // new family cannot quietly join the moving set.
    for family in RecipeFamily::ALL {
        assert!(
            !system
                .family_recipe(family)
                .motion
                .animates(MotionPolicy::Off)
        );
    }
    // Families state their class; the class states the policy answer.
    assert_eq!(
        system.family_recipe(RecipeFamily::Data).motion,
        termrock::style::MotionSemantics::Static
    );
    let _ = action;
    let _ = status;
}

/// The `Off` tier paints a settled frame: nothing on screen moves between
/// ticks, however far apart they are.
#[test]
fn motion_policy_off_paints_static_frames() {
    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 24, 3);
    let (first, second) = two_ticks(950);

    let spinner = Spinner::new(&system).label("working");
    let state = SpinnerState::new();
    let a = painted(area, |b| {
        spinner.paint(area, b, &state, first, MotionPolicy::Off)
    });
    let c = painted(area, |b| {
        spinner.paint(area, b, &state, second, MotionPolicy::Off)
    });
    assert_eq!(a, c, "Spinner animated under MotionPolicy::Off");

    let skeleton = Skeleton::new(2, &system);
    let skeleton_state = SkeletonState::new();
    let a = painted(area, |b| {
        skeleton.paint_with_state(area, b, &skeleton_state, first, MotionPolicy::Off);
    });
    let c = painted(area, |b| {
        skeleton.paint_with_state(area, b, &skeleton_state, second, MotionPolicy::Off);
    });
    assert_eq!(a, c, "Skeleton animated under MotionPolicy::Off");

    let a = painted(area, |b| {
        ProgressBar::new(
            ProgressKind::indeterminate_from(first, MotionPolicy::Off),
            &system,
        )
        .paint(area, b);
    });
    let c = painted(area, |b| {
        ProgressBar::new(
            ProgressKind::indeterminate_from(second, MotionPolicy::Off),
            &system,
        )
        .paint(area, b);
    });
    assert_eq!(a, c, "ProgressBar animated under MotionPolicy::Off");
}

/// `motion_policy_full_actually_animates` + `spinner_frames_one_column`
/// rewritten (D9): the spinner is the ten braille frames at an 80 ms tick, and
/// `Full` must actually advance it or the `Off` gate proves nothing.
#[test]
fn spinner_is_the_braille_cadence_at_80ms() {
    const REFERENCE: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    assert_eq!(
        SPINNER_FRAMES, REFERENCE,
        "the braille cadence is canonical"
    );
    assert_eq!(SPINNER_PERIOD_MS, 80);
    for frame in SPINNER_FRAMES {
        assert_eq!(display_cols(frame), 1, "frame {frame:?} is not one column");
    }
    // The loading glyph is the first frame of the same cadence.
    assert_eq!(DesignSystem::junie().glyphs.loading(), "⠋");

    // Ten frames, one period: a tick one ms short of the period stays put.
    let start = Instant::now();
    let at = |ms: u64| {
        FrameTick::manual(
            start + Duration::from_millis(ms),
            Duration::from_millis(ms),
            Duration::from_millis(16),
        )
    };
    assert_eq!(
        at(79).spinner_step(SPINNER_FRAMES.len(), SPINNER_PERIOD_MS, MotionPolicy::Full),
        0
    );
    assert_eq!(
        at(80).spinner_step(SPINNER_FRAMES.len(), SPINNER_PERIOD_MS, MotionPolicy::Full),
        1
    );
    assert_eq!(
        at(800).spinner_step(SPINNER_FRAMES.len(), SPINNER_PERIOD_MS, MotionPolicy::Full),
        0,
        "the cadence wraps after ten frames"
    );

    // And the widget really moves under Full, so the static gate above bites.
    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 24, 1);
    let mut state = SpinnerState::new();
    let a = painted(area, |b| {
        Spinner::new(&system)
            .label("work")
            .paint(area, b, &mut state, at(0), MotionPolicy::Full);
    });
    let c = painted(area, |b| {
        Spinner::new(&system)
            .label("work")
            .paint(area, b, &mut state, at(80), MotionPolicy::Full);
    });
    assert_ne!(a, c, "the spinner is static even under MotionPolicy::Full");
}

/// D7: the pressed/acknowledged flash is one 140 ms binary mark.
#[test]
fn action_flash_is_a_140ms_binary_mark() {
    assert_eq!(ACTION_FLASH_MS, 140);
    let mut flash = ActionFlash::new();
    assert!(!flash.is_lit(0));
    flash.fire(1_000);
    assert!(flash.is_lit(1_139), "139 ms is inside the mark");
    assert!(!flash.is_lit(1_140), "the window closes hard at 140 ms");
    assert_eq!(flash.alpha(MotionPolicy::Full, 1_000), 1.0);
    assert_eq!(flash.alpha(MotionPolicy::Full, 1_139), 1.0);
    assert_eq!(flash.alpha(MotionPolicy::Full, 1_140), 0.0);
    // `Off` suppresses the repaint churn, not the honesty of the mark.
    assert_eq!(flash.alpha(MotionPolicy::Off, 1_000), 0.0);
    assert!(flash.is_lit(1_000));
    assert_eq!(flash.next_deadline_ms(1_000), Some(1_140));
    flash.clear();
    assert!(!flash.is_lit(1_000));
}

/// Two ticks `gap` ms apart, for gates that need motion to have had its chance.
fn two_ticks(gap: u64) -> (FrameTick, FrameTick) {
    let start = Instant::now();
    (
        FrameTick::manual(start, Duration::ZERO, Duration::ZERO),
        FrameTick::manual(
            start + Duration::from_millis(gap),
            Duration::from_millis(gap),
            Duration::from_millis(16),
        ),
    )
}

// ── E. Selection law (D8) ────────────────────────────────────────────────────

/// `no_widget_paints_selection_fill_by_default` replaced by the D8 invariant:
/// the tint is the keyboard's, hover wins over it, a parked selection is a
/// marker, and a text selection is the popover plane.
#[test]
fn selection_tint_requires_focus_and_hover_wins() {
    let theme = JunieTheme::junie();
    let ground = theme.surface;

    let state = |selected: bool, focused: bool, hovered: bool| VisualState {
        selected,
        focused,
        hovered,
        ..VisualState::default()
    };

    // 1. tint iff selected && focused
    assert_eq!(
        theme.row(state(true, false, false), ground).bg,
        Some(ground)
    );
    assert_eq!(
        theme.row(state(true, true, false), ground).bg,
        Some(GREEN_20)
    );

    // 2. selected && !focused: no tint — the row keeps its own ground. Col 0
    // is always the focus bar (invisible: fg=bg); col 1 carries `›`.
    let recipe = DesignSystem::junie().resolve_list_row(row_state(true, false, false));
    assert_ne!(recipe.label.bg, Some(GREEN_20), "a parked row never tints");
    assert_eq!(recipe.use_tint, false, "a parked row never claims the tint");
    let (bar, bar_style) = recipe.gutter;
    assert_eq!(bar, DesignSystem::junie().glyphs.selection_gutter());
    assert_eq!(bar_style.fg, bar_style.bg, "unfocused bar is invisible");
    let (marker, marker_style) = recipe.marker;
    assert_eq!(
        marker,
        DesignSystem::junie().glyphs.selection_marker(),
        "a parked row speaks with the marker, not the focus gutter"
    );
    assert_eq!(marker_style.fg, Some(WHITE_70));

    // 3. hovered: the lift plane wins, tint or not
    assert_eq!(theme.row(state(true, true, true), ground).bg, Some(OVERLAY));
    assert_eq!(
        theme.row(state(false, false, true), ground).bg,
        Some(OVERLAY)
    );
    assert_eq!(theme.lift(ground), OVERLAY);

    // 4. pressed: #000000 on #ffffff (proven in full in `pressed_is_an_explicit_reversal`)
    let pressed = theme.row(
        VisualState {
            pressed: true,
            ..VisualState::default()
        },
        ground,
    );
    assert_eq!((pressed.fg, pressed.bg), (Some(BLACK), Some(WHITE)));

    // 5. text/range selection is the popover plane, not the tint
    let selection = theme.selection();
    assert_eq!(selection.fg, Some(WHITE));
    assert_eq!(selection.bg, Some(POPOVER));

    // The recipe and the resolver agree, so a widget cannot fork the law.
    let system = DesignSystem::junie();
    let focused = system.resolve_list_row(row_state(true, true, false));
    assert_eq!(focused.label.bg, Some(GREEN_20));
    assert!(focused.use_tint);
    let hovered = system.resolve_list_row(row_state(true, true, true));
    assert_eq!(hovered.label.bg, Some(OVERLAY));
    assert!(!hovered.use_tint, "hover replaces the tint");
    assert_eq!(system.style(Role::SelectionTint).bg, Some(GREEN_20));
    assert_eq!(system.style(Role::Selection).bg, Some(POPOVER));
}

/// The theme is the only selection authority: production paint never forces a
/// chrome on the system, it asks the system.
#[test]
fn selection_chrome_is_not_overridden_in_widget_paint() {
    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&widgets).expect("widgets directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("widget source is readable");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a head");
        if production.contains("SelectionChrome") {
            offenders.push(
                path.file_name()
                    .expect("file has a name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "leftover SelectionChrome identifiers in production widget paint: {offenders:?}"
    );
}

/// Examples do not invent selection chrome (D8: the row recipe owns it).
///
/// The old gate allowed a `REVERSED` slab inside a "colorless" branch; D5/D6
/// delete that second system (state survives monochrome through glyphs and
/// weight), so the reversed clause moved to `modifiers_are_the_junie_set`.
#[test]
fn patterns_compose_chrome() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read pattern");
        let lines: Vec<(usize, &str)> = body
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .take_while(|(_, l)| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|(_, l)| !l.trim_start().starts_with("//"))
            .collect();
        for (line_no, line) in lines {
            if line.contains("Role::Selection") {
                offenders.push(format!(
                    "{}:{line_no}: Role::Selection — the row recipe owns selection paint",
                    path.display()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an example invented its own selection paint:\n{}",
        offenders.join("\n")
    );
}

// ── F. Fields and the three-colour underline law (D5, D9) ────────────────────

/// `a_focused_field_says_so` rewritten to the junie field law: the well is
/// always painted, focus is the `▎` prompt in the focus green, and the label
/// carries the weight.
#[test]
fn a_focused_field_says_so() {
    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 20, 2);

    let mut resting = Buffer::empty(area);
    let mut resting_state = TextInputState::new("value");
    TextInput::new("Name", &system).paint(area, &mut resting, &mut resting_state);

    let mut focused = Buffer::empty(area);
    let mut focused_state = TextInputState::new("value");
    focused_state.set_focused(true);
    TextInput::new("Name", &system).paint(area, &mut focused, &mut focused_state);

    assert_ne!(
        resting, focused,
        "a focused field must differ from a resting one by more than its caret"
    );

    // The well is the field plane in every state.
    let well = system
        .style(Role::Sunken)
        .bg
        .expect("the well carries a fill");
    assert_eq!(
        well,
        Some(INPUT).map(|c| c).expect("field plane"),
        "the well is junie's field body"
    );
    assert!(
        resting.content().iter().any(|c| c.bg == well),
        "the well is painted while resting, not only when focused"
    );
    assert!(focused.content().iter().any(|c| c.bg == well));

    // Focus is the ▎ prompt in the focus green, and it is the only green here.
    assert!(
        focused
            .content()
            .iter()
            .any(|c| c.symbol() == "▎" && c.fg == GREEN),
        "the focused field paints its ▎ prompt in the focus green"
    );
    // Junie reserves the gutter column always. Resting paints `▎` with fg=bg
    // so the slot is present but invisible.
    assert!(
        resting
            .content()
            .iter()
            .any(|c| c.symbol() == "▎" && c.fg == c.bg),
        "a resting field keeps the prompt slot reserved (fg=bg)"
    );

    // The label takes the weight while the field owns the keyboard.
    assert_eq!(
        system.junie_theme().label(true).add_modifier,
        Modifier::BOLD
    );
    assert_eq!(system.junie_theme().label(false).fg, Some(WHITE_70));
    let recipe = system.input_recipe(ControlState::Focused, false, false);
    assert!(recipe.prompt.is_some(), "the recipe ships the prompt glyph");
    assert!(
        !recipe.border.add_modifier.contains(Modifier::UNDERLINED),
        "nav-focus does not underline"
    );
    let editing = system.input_recipe(ControlState::Focused, false, true);
    assert!(
        editing.border.add_modifier.contains(Modifier::UNDERLINED),
        "editing underlines"
    );
}

/// Every field in the input family wears the same chrome (junie: one field
/// body, one prompt glyph, one well).
#[test]
fn inputs_share_field_chrome() {
    use termrock::widgets::{
        NumberInput, NumberInputState, PasswordInput, PasswordInputState, SearchInput,
        SearchInputState, TokenField, TokenFieldState,
    };

    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 24, 2);
    let well = system
        .style(Role::Sunken)
        .bg
        .expect("the field well carries a fill");
    assert_eq!(well, INPUT);

    let mut frames: Vec<(&str, Buffer)> = Vec::new();

    let mut text = TextInputState::new("value");
    text.set_focused(true);
    frames.push((
        "TextInput",
        painted(area, |buffer| {
            TextInput::new("Name", &system).paint(area, buffer, &mut text);
        }),
    ));

    let mut number = NumberInputState::new().with_value(7.0);
    number.set_focused(true);
    frames.push((
        "NumberInput",
        painted(area, |buffer| {
            let _ = NumberInput::new("Count", &system).paint(area, buffer, &mut number);
        }),
    ));

    let mut search = SearchInputState::new();
    search.set_focused(true);
    frames.push((
        "SearchInput",
        painted(area, |buffer| {
            SearchInput::new(&system).paint(area, buffer, &mut search);
        }),
    ));

    let mut password = PasswordInputState::new();
    password.set_focused(true);
    frames.push((
        "PasswordInput",
        painted(area, |buffer| {
            let _ = PasswordInput::new("Secret", &system).paint(area, buffer, &mut password);
        }),
    ));

    let mut tokens = TokenFieldState::new();
    tokens.set_focused(true);
    frames.push((
        "TokenField",
        painted(area, |buffer| {
            TokenField::new(&system).paint(area, buffer, &mut tokens);
        }),
    ));

    for (name, buffer) in &frames {
        assert!(
            buffer.content().iter().any(|cell| cell.bg == well),
            "{name} does not paint the shared field well (#1e1e22)"
        );
        assert!(
            buffer
                .content()
                .iter()
                .any(|cell| cell.symbol() == "▎" && cell.fg == GREEN),
            "{name} does not paint the shared ▎ focus prompt"
        );
        let underlined: Vec<_> = buffer
            .content()
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.style().add_modifier.contains(Modifier::UNDERLINED))
            .map(|(i, cell)| {
                (
                    i as u16 % area.width,
                    i as u16 / area.width,
                    cell.symbol().to_string(),
                )
            })
            .collect();
        assert!(
            underlined.is_empty(),
            "{name} idle focused field must not underline: {underlined:?}"
        );
    }
}

/// `interaction_underline_is_dead` rewritten to the D5/D9 three-colour law:
/// underline is no longer dead, it is *owned* — accent = editing here,
/// `#4d4d4d` = quiet affordance, error/warning = a diagnostic range, plus the
/// link affordance the law itself states. Every painted site must name its
/// class, or the gate fails.
#[test]
fn interaction_underline_is_three_color() {
    /// file -> the D5 class its underline belongs to. Files that reach the
    /// underline through a recipe role (`Role::Link` in `link.rs`) paint no
    /// literal here and stay off the list; the recipe gates pin their
    /// affordance instead.
    const LAW_CLASSES: &[(&str, &str)] = &[
        ("markdown.rs", "link affordance (MarkdownInlineKind::Link)"),
        ("citation.rs", "link affordance (a citation is a link)"),
        ("key_value_list.rs", "link affordance (href values)"),
        (
            "text.rs",
            "author-set content span, not an interaction state",
        ),
        ("code_block.rs", "diagnostic range (squiggle substitute)"),
        (
            "text_input.rs",
            "accent = editing here; idle invalid is bold ! not underline",
        ),
        (
            "field_row.rs",
            "accent = editing here; idle invalid is bold ! not underline",
        ),
        (
            "picker.rs",
            "accent = editing here (query field while filtering)",
        ),
    ];

    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets");
    let mut offenders = Vec::new();
    for path in rust_files(&widgets) {
        let name = path
            .file_name()
            .expect("file has a name")
            .to_string_lossy()
            .into_owned();
        let body = fs::read_to_string(&path).expect("widget source is readable");
        let painted_half = body
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a head");
        let underlines = painted_half.contains("Modifier::UNDERLINED")
            || painted_half.contains(".underlined()")
            || painted_half.contains(".underline(true)");
        match LAW_CLASSES.iter().find(|(file, _)| *file == name) {
            Some((_, class)) => {
                if !underlines {
                    offenders.push(format!("{name}: claims class {class:?} but paints none"));
                }
            }
            None => {
                if underlines {
                    offenders.push(format!(
                        "{name}: paints UNDERLINED without naming its D5 class"
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the three-colour underline law is violated:\n{}",
        offenders.join("\n")
    );

    // The palette side: links are affordances and nothing else underlines.
    let system = DesignSystem::junie();
    for (role, fg) in [(Role::Link, WHITE_70), (Role::LinkHover, WHITE)] {
        let style = system.style(role);
        assert!(
            style.add_modifier.contains(Modifier::UNDERLINED),
            "{role:?}"
        );
        assert_eq!(style.fg, Some(fg), "{role:?} stays on the ladder");
    }

    // The field side: underline is the insert session (accent). Idle invalid
    // is a trailing bold `!` (widget paint), not a red underline — junie
    // `input.rs`. Nav-focus and resting do not underline. Editing wins even
    // when the value is invalid.
    let nav = system.input_recipe(ControlState::Focused, false, false);
    assert!(
        !nav.border.add_modifier.contains(Modifier::UNDERLINED),
        "nav-focus does not underline"
    );
    let editing = system.input_recipe(ControlState::Focused, false, true);
    assert!(editing.border.add_modifier.contains(Modifier::UNDERLINED));
    let invalid = system.input_recipe(ControlState::Focused, true, false);
    assert!(
        !invalid.border.add_modifier.contains(Modifier::UNDERLINED),
        "input_recipe(Focused, true, false) must not be UNDERLINED"
    );
    let editing_invalid = system.input_recipe(ControlState::Focused, true, true);
    assert!(
        editing_invalid
            .border
            .add_modifier
            .contains(Modifier::UNDERLINED),
        "input_recipe(Focused, true, true) is underlined: editing wins"
    );
    assert_eq!(
        editing_invalid.border.underline_color, editing.border.underline_color,
        "editing invalid stays accent, not error"
    );
    assert!(
        !system
            .input_recipe(ControlState::Default, false, false)
            .border
            .add_modifier
            .contains(Modifier::UNDERLINED),
        "a resting field does not underline"
    );
}

/// The three-colour law's first colour: the editing underline is the accent,
/// carried in the underline colour so the value text keeps its tier.
#[test]
fn editing_underline_is_accent() {
    let system = DesignSystem::junie();
    let editing = system.input_recipe(ControlState::Focused, false, true);
    assert_eq!(
        editing.border.underline_color,
        Some(GREEN),
        "the field that owns the keyboard underlines in the accent"
    );
    // The quiet affordance colour is the one the law gives the hover-editable
    // and current-line cases, never the field that owns the keyboard.
    assert_eq!(system.style(Role::BorderFocused).fg, Some(WHITE_30));
}

/// D2/D4.12: the active document tab is marked by the one accent `━` rule —
/// not by a fill, and not by the link underline.
#[test]
fn tab_active_underline_is_an_accent_rule() {
    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 24, 2);
    let tabs = [Tab::new("a", "First"), Tab::new("b", "Second")];
    let mut state = TabsState::new().with_selected("a");
    let buffer = painted(area, |buffer| {
        Tabs::new(&tabs, &system).paint(area, buffer, &mut state);
    });

    // Exactly one accent rule, on the strip's baseline, under the active tab.
    let rules: Vec<u16> = (0..area.width)
        .filter(|x| buffer[(area.x + x, area.y + 1)].symbol() == "━")
        .collect();
    // Source Tabs: `x+1 .. x+w-1` (gutter and trailing pad stay baseline `─`).
    // "First" is gutter+5+pad2 = 8 wide, so 6 accent cells.
    assert_eq!(rules.len(), 6, "one tab width of ━, painted once");
    for x in rules {
        assert_eq!(
            buffer[(area.x + x, area.y + 1)].fg,
            GREEN,
            "the active-tab rule is the accent"
        );
    }
    // The active label carries weight, never a tint fill and never a link
    // underline.
    assert!(!buffer.content().iter().any(|cell| cell.bg == GREEN_20));
    let active = system.style(Role::TabActive);
    assert!(!active.add_modifier.contains(Modifier::UNDERLINED));
    assert!(active.add_modifier.contains(Modifier::BOLD));
}

// ── G. State table and buttons (D9) ─────────────────────────────────────────

/// `state_matrix_distinct` rewritten to the junie state table: default, hover,
/// focus, selected±focus, pressed, disabled, error, busy, editing.
#[test]
fn state_matrix_distinct() {
    let theme = JunieTheme::junie();
    let ground = theme.surface;

    let states: [(&str, VisualState); 9] = [
        ("default", VisualState::default()),
        (
            "hovered",
            VisualState {
                hovered: true,
                ..VisualState::default()
            },
        ),
        (
            "focused",
            VisualState {
                focused: true,
                ..VisualState::default()
            },
        ),
        (
            "selected+focused",
            VisualState {
                selected: true,
                focused: true,
                ..VisualState::default()
            },
        ),
        (
            "pressed",
            VisualState {
                pressed: true,
                ..VisualState::default()
            },
        ),
        (
            "disabled",
            VisualState {
                disabled: true,
                ..VisualState::default()
            },
        ),
        (
            "error",
            VisualState {
                error: true,
                ..VisualState::default()
            },
        ),
        (
            "busy",
            VisualState {
                busy: true,
                ..VisualState::default()
            },
        ),
        (
            "editing (field)",
            VisualState {
                editing: true,
                focused: true,
                ..VisualState::default()
            },
        ),
    ];
    for (i, (name, state)) in states.iter().enumerate() {
        // `row` is the universal resolver; the field state rides `field_style`.
        let style = if *name == "editing (field)" {
            theme.field_style(*state)
        } else {
            theme.row(*state, ground)
        };
        let key = (style.fg, style.bg, style.add_modifier);
        for (other_name, other) in states.iter().skip(i + 1) {
            let other_style = if *other_name == "editing (field)" {
                theme.field_style(*other)
            } else {
                theme.row(*other, ground)
            };
            assert_ne!(
                key,
                (other_style.fg, other_style.bg, other_style.add_modifier),
                "the junie state table resolves {name} and {other_name} identically"
            );
        }
    }

    // A parked selection is the one deliberate equal: no fill, marker only.
    assert_eq!(
        theme
            .row(
                VisualState {
                    selected: true,
                    ..VisualState::default()
                },
                ground
            )
            .bg,
        theme.row(VisualState::default(), ground).bg
    );

    // The recipe layer agrees with the resolver layer, per control state.
    let system = DesignSystem::junie();
    let control_states = [
        ControlState::Default,
        ControlState::Hovered,
        ControlState::Pressed,
        ControlState::Focused,
        ControlState::Disabled,
        ControlState::Loading,
    ];
    // `Primary` is deliberately absent from this loop: junie's primary focus
    // cue is the `▎` gutter, which `ButtonRecipe` cannot carry (no gutter
    // field), so Default and Focused resolve to the same (label, fill) pair by
    // construction. The missing cue is the defect tracked by the ignored gate
    // `button_focus_is_the_gutter`; weakening this loop to admit it would hide
    // the collapse instead of naming it.
    for variant in [
        ButtonRecipeVariant::Secondary,
        ButtonRecipeVariant::Quiet,
        ButtonRecipeVariant::Destructive,
    ] {
        let resolved: Vec<_> = control_states
            .iter()
            .map(|state| system.button_recipe(variant, *state, CHROME))
            .collect();
        for (i, recipe) in resolved.iter().enumerate() {
            for (j, other) in resolved.iter().enumerate().skip(i + 1) {
                // Weight and the busy prefix are part of the state's address:
                // focus is BOLD where the fill does not move, and busy is the
                // accent spinner cell, not a second fill.
                assert_ne!(
                    (
                        recipe.label.fg,
                        recipe.label.add_modifier,
                        recipe.fill.bg,
                        recipe.busy_glyph.is_some(),
                    ),
                    (
                        other.label.fg,
                        other.label.add_modifier,
                        other.fill.bg,
                        other.busy_glyph.is_some(),
                    ),
                    "{variant:?} resolves {:?} and {:?} identically",
                    control_states[i],
                    control_states[j]
                );
            }
        }
    }

    // A disabled control ignores hover: unavailability is not a hover target.
    let disabled = VisualState {
        disabled: true,
        ..VisualState::default()
    };
    let disabled_hover = VisualState {
        disabled: true,
        hovered: true,
        ..VisualState::default()
    };
    assert_eq!(
        theme.row(disabled, ground),
        theme.row(disabled_hover, ground),
        "a disabled row must not react to hover"
    );
    for kind in [
        ButtonKind::Primary,
        ButtonKind::Secondary,
        ButtonKind::Subtle,
        ButtonKind::Danger,
    ] {
        assert_eq!(
            theme.button(kind, disabled, ground),
            theme.button(kind, disabled_hover, ground),
            "{kind:?} must not react to hover while disabled"
        );
    }
}

/// The button table, kind for kind, against the reference (D9).
#[test]
fn button_table_matches_the_reference() {
    let theme = JunieTheme::junie();
    let idle = VisualState::default();
    let hovered = VisualState {
        hovered: true,
        ..VisualState::default()
    };
    let pressed = VisualState {
        pressed: true,
        ..VisualState::default()
    };
    let focused = VisualState {
        focused: true,
        ..VisualState::default()
    };

    // Primary: on-accent text, accent fill, hover and press walk the ramp.
    let primary = theme.button(ButtonKind::Primary, idle, theme.surface);
    assert_eq!(primary.fg, Some(ON_GREEN));
    assert_eq!(primary.bg, Some(GREEN));
    assert!(primary.add_modifier.contains(Modifier::BOLD));
    assert_eq!(
        theme.button(ButtonKind::Primary, hovered, theme.surface).bg,
        Some(GREEN_80)
    );
    assert_eq!(
        theme.button(ButtonKind::Primary, pressed, theme.surface).bg,
        Some(GREEN_60)
    );

    // Secondary and toggle share the overlay plane; press reverses.
    for kind in [ButtonKind::Secondary, ButtonKind::Toggle] {
        let style = theme.button(kind, idle, theme.surface);
        assert_eq!(style.fg, Some(WHITE));
        assert_eq!(style.bg, Some(OVERLAY));
        assert_eq!(theme.button(kind, hovered, theme.surface).bg, Some(POPOVER));
        assert_eq!(theme.button(kind, pressed, theme.surface).fg, Some(BLACK));
        assert_eq!(theme.button(kind, pressed, theme.surface).bg, Some(WHITE));
        assert!(
            theme
                .button(kind, focused, theme.surface)
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    // Subtle reads the container ground and lifts one plane on hover.
    let subtle = theme.button(ButtonKind::Subtle, idle, theme.surface);
    assert_eq!(subtle.fg, Some(WHITE_70));
    assert_eq!(subtle.bg, Some(theme.surface));
    let subtle_hover = theme.button(ButtonKind::Subtle, hovered, theme.surface);
    assert_eq!(subtle_hover.fg, Some(WHITE));
    assert_eq!(subtle_hover.bg, Some(theme.lift(theme.surface)));
    assert!(
        theme
            .button(ButtonKind::Subtle, focused, theme.surface)
            .add_modifier
            .contains(Modifier::BOLD)
    );

    // Danger keeps its tone and presses into its own fill.
    let danger = theme.button(ButtonKind::Danger, idle, theme.surface);
    assert_eq!(danger.fg, Some(RED));
    assert_eq!(danger.bg, Some(OVERLAY));
    let danger_pressed = theme.button(ButtonKind::Danger, pressed, theme.surface);
    assert_eq!(danger_pressed.fg, Some(WHITE));
    assert_eq!(danger_pressed.bg, Some(RED));
    assert!(
        !danger.add_modifier.contains(Modifier::BOLD),
        "danger carries no weight; the glyph carries the alarm"
    );

    // Disabled: the faint tier, lifted unless the control was already quiet.
    let disabled = VisualState {
        disabled: true,
        ..VisualState::default()
    };
    assert_eq!(
        theme
            .button(ButtonKind::Primary, disabled, theme.surface)
            .fg,
        Some(WHITE_30)
    );
    assert_eq!(
        theme
            .button(ButtonKind::Primary, disabled, theme.surface)
            .bg,
        Some(OVERLAY)
    );
    assert_eq!(
        theme.button(ButtonKind::Subtle, disabled, theme.surface).bg,
        Some(theme.surface)
    );

    // The public recipe vocabulary collapses onto these kinds.
    let system = DesignSystem::junie();
    let primary_recipe =
        system.button_recipe(ButtonRecipeVariant::Primary, ControlState::Default, CHROME);
    assert_eq!(primary_recipe.label.fg, Some(ON_GREEN));
    assert_eq!(primary_recipe.fill.bg, Some(GREEN));
    let destructive = system.button_recipe(
        ButtonRecipeVariant::Destructive,
        ControlState::Default,
        CHROME,
    );
    assert_eq!(destructive.label.fg, Some(RED));
    assert_eq!(destructive.fill.bg, Some(OVERLAY));
    // A link is the affordance law, never a fill.
    let link = system.button_recipe(ButtonRecipeVariant::Link, ControlState::Default, CHROME);
    assert_eq!(link.fill.bg, None);
    assert!(link.label.add_modifier.contains(Modifier::UNDERLINED));
}

/// `every_button_variant_paints_distinct_focus_without_color` rewritten (D5/D6):
/// monochrome focus must be structural — weight or an explicit plane — because
/// the glyph vocabulary and the modifiers are the only channels left.
#[test]
fn button_focus_is_structural_in_monochrome() {
    let mono = DesignSystem::junie().no_color();
    assert!(mono.mono());

    for variant in [
        ButtonRecipeVariant::Secondary,
        ButtonRecipeVariant::Quiet,
        ButtonRecipeVariant::Destructive,
    ] {
        let idle = mono.button_recipe(variant, ControlState::Default, CHROME);
        let focused = mono.button_recipe(variant, ControlState::Focused, CHROME);
        assert_ne!(
            focused.label, idle.label,
            "{variant:?} loses focus without colour"
        );
        assert!(
            focused.label.add_modifier.contains(Modifier::BOLD),
            "{variant:?} focus is not weight"
        );
    }

    // And the paint follows the recipe: a secondary button says focus through
    // weight alone, with no colour doing the work.
    use termrock::widgets::{Button, ButtonState, ButtonVariant};
    let area = Rect::new(0, 0, 18, 1);
    for variant in [ButtonVariant::Secondary, ButtonVariant::Quiet] {
        let mut idle_state = ButtonState::new();
        let idle = painted(area, |buffer| {
            Button::new("Run", &mono)
                .variant(variant)
                .paint(area, buffer, &mut idle_state);
        });
        let mut focused_state = ButtonState::new();
        focused_state.activation.set_accepts_input(true);
        // Focus is a fact the host supplies; accepting input only makes the
        // control focusable.
        focused_state.focused = true;
        let focused = painted(area, |buffer| {
            Button::new("Run", &mono)
                .variant(variant)
                .paint(area, buffer, &mut focused_state);
        });
        assert_ne!(
            focused.content(),
            idle.content(),
            "{variant:?} focus vanished in monochrome paint"
        );
        assert!(
            focused
                .content()
                .iter()
                .any(|cell| cell.modifier.contains(Modifier::BOLD)),
            "{variant:?} focus carries no weight"
        );
    }
}

/// junie's focus cue for an already-bold control is the `▎` gutter, so focus
/// survives mono and a page of pre-emphasised labels.
#[test]
fn button_focus_is_the_gutter() {
    let mono = DesignSystem::junie().no_color();
    let area = Rect::new(0, 0, 18, 1);
    for variant in [
        termrock::widgets::ButtonVariant::Primary,
        termrock::widgets::ButtonVariant::Destructive,
    ] {
        let mut idle_state = termrock::widgets::ButtonState::new();
        let idle = painted(area, |buffer| {
            termrock::widgets::Button::new("Run", &mono)
                .variant(variant)
                .paint(area, buffer, &mut idle_state);
        });
        let mut focused_state = termrock::widgets::ButtonState::new();
        focused_state.activation.set_accepts_input(true);
        focused_state.focused = true;
        let focused = painted(area, |buffer| {
            termrock::widgets::Button::new("Run", &mono)
                .variant(variant)
                .paint(area, buffer, &mut focused_state);
        });
        assert_ne!(
            focused.content(),
            idle.content(),
            "{variant:?} focus vanished in monochrome"
        );
        assert!(
            focused.content().iter().any(|cell| cell.symbol() == "▎"),
            "{variant:?} focus is not the gutter"
        );
    }
}

// ── H. Accent budget (D9) ────────────────────────────────────────────────────

/// `accent_budget` rewritten to junie's position classes (D9): green may spend
/// on a focus gutter cell, a primary fill cell, or a `›`/`✓` marker cell — and
/// nowhere else. The three classes are budgeted separately so a flood of
/// markers cannot hide behind a legitimate fill.
#[test]
fn junie_green_budget_is_position_based() {
    use termrock::widgets::{StatusBar, StatusBarState, StatusSlot};

    let system = DesignSystem::junie();
    let green = system
        .style(Role::Accent)
        .fg
        .expect("the accent carries a colour");
    let is_green = |cell: &Cell| cell.fg == green || cell.bg == green;
    const MARKERS: [&str; 2] = ["›", "✓"];

    // Collection: green is a gutter column and a marker, never a fill.
    let rows: Vec<ListRow<'static, usize>> = (0..8)
        .map(|id| ListRow {
            id,
            label: Line::from("a list row that says something"),
            leading: None,
            secondary: Some(Line::from("meta")),
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        })
        .collect();
    let area = Rect::new(0, 0, 60, 12);
    let mut list_state = ListState::new(Some(2));
    let list = painted(area, |buffer| {
        StatefulWidget::render(&List::new(&rows, &system), area, buffer, &mut list_state);
    });

    let mut gutter_cells = 0usize;
    let mut marker_cells = 0usize;
    let mut fill_cells = 0usize;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &list[(area.x + x, area.y + y)];
            if !is_green(cell) {
                continue;
            }
            if x == 0 && cell.symbol() == "▎" {
                gutter_cells += 1;
            } else if MARKERS.contains(&cell.symbol()) {
                marker_cells += 1;
            } else if cell.bg == green {
                fill_cells += 1;
            } else {
                panic!("list paints green at {x},{y} in no budgeted class");
            }
        }
    }
    assert!(
        gutter_cells <= usize::from(area.height),
        "{gutter_cells} gutter cells; one per visible row at most"
    );
    // Markers are a per-row budget too: at most one green marker cell per row.
    assert!(
        marker_cells <= usize::from(area.height),
        "{marker_cells} marker cells; one per visible row at most"
    );
    assert_eq!(fill_cells, 0, "green never fills a collection row");

    // Primary action: the fill class, and the whole control is the fill — the
    // gutter cell, the pad, and the label all speak the accent, nothing else.
    let button_area = Rect::new(0, 0, 10, 1);
    let mut button_state = termrock::widgets::ButtonState::new();
    button_state.activation.set_accepts_input(true);
    let button = painted(button_area, |buffer| {
        termrock::widgets::Button::new("Run", &system)
            .variant(termrock::widgets::ButtonVariant::Primary)
            .paint(button_area, buffer, &mut button_state);
    });
    // A cell the button never touched stays default; every cell it did touch
    // must speak the accent — the fill, the gutter, or the on-accent label.
    let on_green = system.style(Role::TextOnAccent).fg.expect("on-accent text");
    let untouched = |cell: &Cell| {
        cell.symbol().trim().is_empty() && cell.fg == Color::Reset && cell.bg == Color::Reset
    };
    let in_fill_block = |cell: &Cell| is_green(cell) || cell.fg == on_green;
    assert_eq!(
        count_cells(&button, |cell| !untouched(cell) && !in_fill_block(cell)),
        0,
        "a primary button is one fill block, not scattered accents"
    );
    assert!(
        count_cells(&button, is_green) > 0,
        "the primary button painted nothing for the gate to judge"
    );

    // Status chrome spends no green at all: status is a glyph and a label.
    let slots = [
        StatusSlot::new("mode", "edit"),
        StatusSlot::new("branch", "main"),
        StatusSlot::new("sel", "3 selected"),
    ];
    let bar_area = Rect::new(0, 0, 60, 1);
    let mut bar_state = StatusBarState::<&str>::new();
    let bar = painted(bar_area, |buffer| {
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], &system),
            bar_area,
            buffer,
            &mut bar_state,
        );
    });
    assert_eq!(
        count_cells(&bar, is_green),
        0,
        "the status bar spends no green"
    );
}

/// `accents_are_distinct` rewritten (D9): there is one accent, and it is the
/// same green as focus and success. Every other role is neutral or diagnostic.
#[test]
fn green_is_reserved_to_the_intent_roles() {
    let palette = RolePalette::junie();
    let green = palette.style(Role::Accent).fg.expect("accent");
    assert_eq!(
        palette.style(Role::Focus).fg,
        Some(green),
        "focus is the accent"
    );
    assert_eq!(
        palette.style(Role::Success).fg,
        Some(green),
        "success is the accent"
    );
    assert_eq!(palette.style(Role::SelectionTint).bg, Some(GREEN_20));

    // No other role may spend it. `ActionFocused` is exempt: it IS the focus
    // class (the `▎` gutter's colour), not a second accent (D9 budget).
    for role in RolePalette::roles() {
        if matches!(
            role,
            Role::Accent | Role::Focus | Role::Success | Role::SelectionTint | Role::ActionFocused
        ) {
            continue;
        }
        let style = palette.style(role);
        assert_ne!(
            style.fg,
            Some(green),
            "{role:?} paints the accent; green is reserved to intent"
        );
        assert_ne!(
            style.bg,
            Some(green),
            "{role:?} fills with the accent; green is reserved to intent"
        );
    }
}

/// D2/D4.14: the ` EDIT ` badge is the only badge in the system, and it spends
/// green on purpose — one badge, one intent.
#[test]
fn edit_badge_is_the_only_badge() {
    // Compile guard: no wildcard arm, so a second badge kind is a build error
    // rather than a green bar.
    const BADGE_KIND_COUNT: usize = match BadgeKind::Edit {
        BadgeKind::Edit => 1,
    };
    assert_eq!(BADGE_KIND_COUNT, 1, "junie has exactly one badge");
    let badge = JunieTheme::junie().badge(BadgeKind::Edit);
    assert_eq!(badge.fg, Some(ON_GREEN));
    assert_eq!(badge.bg, Some(GREEN));
    assert_eq!(badge.add_modifier, Modifier::BOLD);
}

/// D4.14: green is completion, not activity — a running bar is the ladder.
#[test]
fn progress_spends_green_only_when_complete() {
    let system = DesignSystem::junie();
    let green = system.style(Role::Accent).fg.expect("accent");
    let is_green = |cell: &Cell| cell.fg == green || cell.bg == green;

    let area = Rect::new(0, 0, 20, 1);
    let running = painted(area, |buffer| {
        ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system).paint(area, buffer);
    });
    assert_eq!(
        count_cells(&running, is_green),
        0,
        "a running bar is white 70%, never the accent"
    );

    let full_running = painted(area, |buffer| {
        ProgressBar::new(ProgressKind::Determinate { fraction: 1.0 }, &system).paint(area, buffer);
    });
    assert_eq!(
        count_cells(&full_running, is_green),
        0,
        "a 100% running bar is still the ladder; green is the Done status, not the fill amount"
    );

    let complete = painted(area, |buffer| {
        ProgressBar::new(ProgressKind::Determinate { fraction: 1.0 }, &system)
            .status(termrock::widgets::ProgressStatus::Complete)
            .paint(area, buffer);
    });
    assert!(
        count_cells(&complete, is_green) > 0,
        "completion is the one thing progress spends green on"
    );
}

// ── I. Information budget ────────────────────────────────────────────────────
//
// Copy and tone budgets are junie-neutral (one voice, one hue), so they carry
// over unchanged apart from the vocabulary they name.

#[test]
fn no_bare_ellipsis_in_paint() {
    let mut offenders = Vec::new();
    for source in painted_sources() {
        for (i, (line_no, line)) in source.lines.iter().enumerate() {
            if source.payload[i] {
                continue;
            }
            for literal in string_literals(line) {
                if !literal.contains("...") {
                    continue;
                }
                if ascii_gated(&source.lines, i) {
                    continue;
                }
                offenders.push(format!("{}:{line_no}: {literal}", source.path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "bare `...` in painted copy — resolve through GlyphSet::ellipsis() or pair it with an \
         ASCII-gated twin (law §4.1 rule 4):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn one_overflow_note() {
    // `text::more_note` is the one voice that says what a surface cut. A
    // hand-rolled copy is how plan 022 left four sites agreeing by accident
    // and a fifth (`integration_status`) clipping in silence.
    let mut offenders = Vec::new();
    for source in painted_sources() {
        for (line_no, line) in &source.lines {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("}more\"") || compact.contains("{hidden}more") {
                offenders.push(format!(
                    "{}:{line_no}: {}",
                    source.path.display(),
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "overflow notes must come from `text::more_note`, not a local format!:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn one_chord_notation() {
    // `kbd.rs` owns the spelled and symbolic renderings; it is the formatter,
    // not a caller.
    const FORMATTER: &str = "kbd.rs";
    // junie pickers.rs paints spelled `Alt+Enter` in the search footer.
    const JUNIE_SPELLED_FOOTER: &str = "picker.rs";
    const SPELLED: [&str; 5] = ["Ctrl+", "Control+", "Cmd+", "Alt+", "Shift+"];
    /// Mac modifier symbols. These double as resource badges (`⌘` marks an SSH
    /// host, `⌥` a branch), so only a symbol *bound to a key* is chord notation.
    const SYMBOLS: [char; 4] = ['⌘', '⌥', '⇧', '⌃'];

    let mut offenders = Vec::new();
    for source in painted_sources() {
        if source.path.ends_with(FORMATTER) || source.path.ends_with(JUNIE_SPELLED_FOOTER) {
            continue;
        }
        for (line_no, line) in &source.lines {
            for literal in string_literals(line) {
                let spelled = SPELLED.iter().find(|f| literal.contains(**f)).copied();
                let symbolic = SYMBOLS.iter().copied().find(|symbol| {
                    literal
                        .split(*symbol)
                        .skip(1)
                        .any(|rest| rest.chars().next().is_some_and(char::is_alphanumeric))
                });
                let found = spelled
                    .map(str::to_string)
                    .or_else(|| symbolic.map(|c| c.to_string()));
                if let Some(found) = found {
                    offenders.push(format!(
                        "{}:{line_no}: {found} in {literal}",
                        source.path.display()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "spelled chord in painted copy — use the `C-x` / `A-x` / `S-x` notation or \
         `widgets::kbd::format_chord` (law §4.1 rule 3):\n{}",
        offenders.join("\n")
    );
}

/// Chords a footer hint literal may advertise.
///
/// Sixteen chords in one row is a keymap dump wearing a hint row's clothes:
/// the eye reads none of them. The rest belong in the keyboard-help overlay.
const HINT_COPY_BUDGET: usize = 5;

/// Whether a ` · `-joined literal reads as a row of chords.
fn hint_segments(literal: &str) -> Option<usize> {
    let body = literal.trim_matches('"');
    let segments: Vec<&str> = body.split(" · ").map(str::trim).collect();
    if segments.len() < 2 {
        return None;
    }
    let chord_like = segments
        .iter()
        .filter(|segment| {
            let mut parts = segment.split_whitespace();
            let (Some(chord), Some(verb)) = (parts.next(), parts.next()) else {
                return false;
            };
            chord.len() <= 4
                && !chord.contains('{')
                && verb.chars().next().is_some_and(char::is_alphabetic)
        })
        .count();
    (chord_like * 2 >= segments.len()).then_some(segments.len())
}

/// Whether this line sits inside a `StatusSlot::shortcut(...)` call.
fn slot_shortcut_context(lines: &[(usize, String)], index: usize) -> bool {
    let start = index.saturating_sub(2);
    lines[start..=index]
        .iter()
        .any(|(_, line)| line.contains("StatusSlot::shortcut"))
}

#[test]
fn pattern_hint_copy_budget() {
    let mut over: Vec<String> = Vec::new();
    for source in painted_sources() {
        if !source.path.to_string_lossy().contains("patterns") {
            continue;
        }
        for (index, (number, line)) in source.lines.iter().enumerate() {
            if slot_shortcut_context(&source.lines, index) {
                continue;
            }
            for literal in string_literals(line) {
                if let Some(count) = hint_segments(&literal)
                    && count > HINT_COPY_BUDGET
                {
                    let name = source
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    over.push(format!("{name}:{number}: {count} chords in {literal}"));
                }
            }
        }
    }
    assert!(
        over.is_empty(),
        "footer hint rows over the {HINT_COPY_BUDGET}-chord budget:\n  {}",
        over.join("\n  ")
    );
}

/// Foreground colors that paint *content* in `buffer`, not single glyphs.
///
/// junie speaks in one hue plus a five-tier neutral ladder, so the hue budget
/// is a *structure* budget: a frame that needs a ninth foreground is ranking
/// nine things at once.
const GLYPH_CELL_ALLOWANCE: usize = 3;

fn content_foregrounds(buffer: &Buffer) -> Vec<Color> {
    let mut counts: Vec<(Color, usize)> = Vec::new();
    for cell in buffer.content() {
        if cell.symbol().trim().is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(color, _)| *color == cell.fg) {
            Some((_, seen)) => *seen += 1,
            None => counts.push((cell.fg, 1)),
        }
    }
    counts
        .into_iter()
        .filter(|(_, seen)| *seen > GLYPH_CELL_ALLOWANCE)
        .map(|(color, _)| color)
        .collect()
}

/// Footer hint rows in a frame, and how many chords each advertises.
fn hint_rows(buffer: &Buffer, system: &DesignSystem) -> Vec<usize> {
    let separator = system.glyphs.meta_separator();
    let track_fg = system.style(Role::ScrollTrack).fg;
    let footer_band = buffer.area.height.saturating_sub(3);
    (footer_band..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter(|x| {
                    let cell = &buffer[(buffer.area.x + x, buffer.area.y + y)];
                    cell.symbol() == separator && Some(cell.fg) != track_fg
                })
                .count()
        })
        .filter(|joins| *joins >= 2)
        .map(|joins| joins + 1)
        .collect()
}

/// The default frame of each priority pattern, painted with its own fixture.
fn priority_pattern_frames(system: &DesignSystem) -> Vec<(&'static str, Buffer)> {
    let area = Rect::new(0, 0, 72, 18);

    let mut agent = AgentStatusHeaderState::new();
    agent.snapshot = example_agent_status();
    agent.presentation = AgentStatusPresentation::Header;
    let agent_area = Rect::new(0, 0, 72, 3);

    let mut sessions = SessionPickerState::new();
    sessions.set_sessions(example_sessions());

    let mut connections = ConnectionManagerState::new();
    connections.set_connections(example_connections());

    let mut plan = PlanReviewState::new();
    plan.open(example_plan_document());

    let mut integrations = IntegrationStatusState::new();
    integrations.set_entries(example_integrations());
    integrations.presentation = IntegrationStatusPresentation::Panel;

    let wide = Rect::new(0, 0, 120, 32);
    let mut database = DatabaseWorkbenchState::new();
    let schema = example_schema_entries();
    let columns = example_result_columns();
    let raw_rows = example_result_rows();
    let mut cell_store = Vec::new();
    let rows = example_result_row_refs(&raw_rows, &mut cell_store);
    let db_fields = example_inspect_fields();
    let history = example_db_history();
    let commands = example_db_commands();

    let mut observability = ObservabilityDashboardState::new();
    let logs = example_observability_logs();
    let events = example_observability_events();
    let tiles = example_observability_tiles();
    let alerts = example_observability_alerts();
    let obs_fields = example_log_inspect_fields();

    vec![
        (
            "agent_status_header",
            painted(agent_area, |buffer| {
                AgentStatusHeader::new(system).paint(agent_area, buffer, &mut agent);
            }),
        ),
        (
            "session_picker",
            painted(area, |buffer| {
                SessionPicker::new(system).paint(area, buffer, &mut sessions);
            }),
        ),
        (
            "connection_manager",
            painted(area, |buffer| {
                ConnectionManager::new(system).paint(area, buffer, &mut connections);
            }),
        ),
        (
            "plan_review",
            painted(area, |buffer| {
                PlanReview::new(system).paint(area, buffer, &mut plan);
            }),
        ),
        (
            "integration_status",
            painted(area, |buffer| {
                IntegrationStatus::new(system).paint(area, buffer, &mut integrations);
            }),
        ),
        (
            "database_workbench",
            painted(wide, |buffer| {
                render_database_workbench(
                    buffer,
                    wide,
                    DatabaseWorkbenchSurfaces {
                        system,
                        state: &mut database,
                        schema_entries: &schema,
                        result_columns: &columns,
                        result_rows: &rows,
                        inspect_fields: &db_fields,
                        history: &history,
                        commands: &commands,
                    },
                );
            }),
        ),
        (
            "observability_dashboard",
            painted(wide, |buffer| {
                render_observability_dashboard(
                    buffer,
                    wide,
                    ObservabilityDashboardSurfaces {
                        system,
                        state: &mut observability,
                        logs: &logs,
                        events: &events,
                        tiles: &tiles,
                        alerts: &alerts,
                        inspect_fields: &obs_fields,
                    },
                );
            }),
        ),
    ]
}

/// Hues a default frame may speak in before it is shouting.
///
/// junie's whole vocabulary is five neutral tiers plus three semantic colours;
/// a frame over this budget is not painting in the system any more.
const STYLE_DIVERSITY_BUDGET: usize = 8;

#[test]
fn pattern_style_diversity() {
    let system = DesignSystem::junie();
    let mut over: Vec<String> = Vec::new();
    for (name, buffer) in priority_pattern_frames(&system) {
        let hues = content_foregrounds(&buffer);
        if hues.len() > STYLE_DIVERSITY_BUDGET {
            over.push(format!("{name}: {} hues {hues:?}", hues.len()));
        }
    }
    assert!(
        over.is_empty(),
        "default frames over the {STYLE_DIVERSITY_BUDGET}-hue budget:\n  {}",
        over.join("\n  ")
    );
}

/// Chords one footer row may advertise before it becomes a keymap dump.
const HINT_BUDGET: usize = 5;

#[test]
fn pattern_hint_budget() {
    let system = DesignSystem::junie();
    let mut over: Vec<String> = Vec::new();
    for (name, buffer) in priority_pattern_frames(&system) {
        let rows = hint_rows(&buffer, &system);
        if rows.len() > 1 {
            over.push(format!("{name}: {} hint rows", rows.len()));
        }
        for hints in &rows {
            if *hints > HINT_BUDGET {
                over.push(format!("{name}: {hints} hints on one row"));
            }
        }
    }
    assert!(
        over.is_empty(),
        "footer hint budget exceeded:\n  {}",
        over.join("\n  ")
    );
}

/// Data rows read as tiers, not as one tone (junie's alpha ladder).
#[test]
fn data_rows_have_ladder() {
    use termrock::widgets::{
        EventSeverity, EventStream, EventStreamState, LogLevel, LogLine, LogLineRecipe, LogStream,
        LogStreamState, StreamEvent, TraceSpan, TraceWaterfall, TraceWaterfallState,
    };

    let system = DesignSystem::junie();
    let area = Rect::new(0, 0, 96, 12);
    let mut frames: Vec<(&'static str, Buffer)> = Vec::new();

    let log_lines = vec![
        LogLine::new("1", LogLevel::Info, "boot complete")
            .timestamp("12:00:00")
            .source("main"),
        LogLine::new("2", LogLevel::Error, "connection refused")
            .timestamp("12:00:01")
            .source("net"),
    ];
    let mut log_state = LogStreamState::new();
    log_state.set_following(false);
    log_state.recipe = LogLineRecipe::Detailed;
    frames.push((
        "LogStream",
        painted(area, |buffer| {
            LogStream::new(&log_lines, &system)
                .focused(true)
                .render(area, buffer, &mut log_state);
        }),
    ));

    let events: Vec<StreamEvent<'_, ()>> = vec![
        StreamEvent::with_id((), "Normal", "12:01:00", "Scheduled pod")
            .severity(EventSeverity::Info)
            .source("scheduler")
            .fields("pod=api-7 node=n1"),
        StreamEvent::with_id((), "Failed", "12:01:04", "Back-off restarting")
            .severity(EventSeverity::Error)
            .source("kubelet")
            .fields("pod=api-7"),
    ];
    let mut event_state = EventStreamState::new();
    event_state.set_following(false);
    frames.push((
        "EventStream",
        painted(area, |buffer| {
            EventStream::new(&events, &system)
                .focused(true)
                .render(area, buffer, &mut event_state);
        }),
    ));

    let spans = vec![
        TraceSpan::new("root", "HTTP GET /api", 0, 420)
            .service("gateway")
            .branch()
            .expanded(),
        TraceSpan::new("db", "SELECT users", 50, 180)
            .parent("root")
            .service("postgres")
            .depth(1),
    ];
    let mut trace_state = TraceWaterfallState::new();
    frames.push((
        "TraceWaterfall",
        painted(area, |buffer| {
            let _ = TraceWaterfall::new(&spans, &system).focused(true).render(
                area,
                buffer,
                &mut trace_state,
            );
        }),
    ));

    for (name, buffer) in &frames {
        let rows = data_row_tones(buffer);
        assert!(
            !rows.is_empty(),
            "{name} painted no data row for the gate to judge"
        );
        for (y, tones) in rows {
            assert!(
                tones >= 2,
                "{name} paints row {y} in {tones} tone(s); a row of several \
                 facts must not arrive as several equals"
            );
        }
    }
}

/// Distinct foregrounds for every buffer row carrying a row of data.
fn data_row_tones(buffer: &Buffer) -> Vec<(u16, usize)> {
    (0..buffer.area.height)
        .filter_map(|y| {
            let mut seen: Vec<Color> = Vec::new();
            let mut content = 0usize;
            for x in 0..buffer.area.width {
                let cell = &buffer[(buffer.area.x + x, buffer.area.y + y)];
                if cell.symbol().trim().is_empty() {
                    continue;
                }
                content += 1;
                if !seen.contains(&cell.fg) {
                    seen.push(cell.fg);
                }
            }
            (content >= 16).then_some((y, seen.len()))
        })
        .collect()
}

#[test]
fn patterns_have_real_empty_states() {
    let mut offenders = Vec::new();
    for dir in ["patterns", "widgets"] {
        for path in rust_files(&crate_src().join(dir)) {
            let body = fs::read_to_string(&path).expect("read source");
            for (i, line) in body
                .lines()
                .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
                .enumerate()
            {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for literal in string_literals(line) {
                    if literal.starts_with("(no ") || literal.starts_with("(select ") {
                        offenders.push(format!("{}:{}: {literal}", path.display(), i + 1));
                    }
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "placeholder copy where an EmptyState belongs:\n{}",
        offenders.join("\n")
    );
}

/// Wide emoji never sit in a one-column slot.
#[test]
fn no_wide_emoji_in_chrome() {
    fn is_emoji_presentation(c: char) -> bool {
        matches!(c as u32, 0x1F000..=0x1FAFF)
    }

    let mut offenders = Vec::new();
    for source in painted_sources() {
        if source.path.ends_with("tests.rs") {
            continue;
        }
        for (i, (line_no, line)) in source.lines.iter().enumerate() {
            if source.payload[i] {
                continue;
            }
            for literal in string_literals(line) {
                if literal.chars().any(is_emoji_presentation) {
                    offenders.push(format!("{}:{line_no}: {literal}", source.path.display()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "wide emoji in painted chrome — use a one-column catalog glyph:\n{}",
        offenders.join("\n")
    );
}

// ── J. Geometry ──────────────────────────────────────────────────────────────

#[test]
fn text_never_touches_borders() {
    use termrock::widgets::{Surface, SurfaceRecipe};

    // The contract holds at every width, including the narrow ones where
    // padding used to collapse to zero — and the rounded corners reserve the
    // same column a square one did.
    let system = DesignSystem::junie();
    for recipe in [
        SurfaceRecipe::Overlay,
        SurfaceRecipe::OverlayFocused,
        SurfaceRecipe::Raised,
        SurfaceRecipe::Interactive,
    ] {
        for width in 3..40u16 {
            let area = Rect::new(0, 0, width, 5);
            let content = Surface::new(&system)
                .recipe(recipe)
                .bordered(true)
                .content_inset()
                .layout(area)
                .content;
            if content.width == 0 {
                continue;
            }
            assert!(
                content.x >= area.x + 2,
                "{recipe:?} at width {width}: content starts at {} — border at {} plus one",
                content.x,
                area.x
            );
            assert!(
                content.right() + 2 <= area.right(),
                "{recipe:?} at width {width}: content ends at {} against border {}",
                content.right(),
                area.right()
            );
        }
    }
}

/// Overlay widgets whose chrome must never let text touch a border glyph.
const BORDERED_OVERLAYS: &[&str] = &[
    "drawer.rs",
    "dropdown_menu.rs",
    "notification_center.rs",
    "preview_card.rs",
    "popover.rs",
    "menu_bar.rs",
    "fullscreen_viewer.rs",
    "image_surface.rs",
    "callout.rs",
];

#[test]
fn bordered_overlays_reserve_their_gutters() {
    // A `padding(0, 0)` on this family is how text ended up flush against the
    // border glyph; `content_inset()` is the sanctioned form.
    let mut flush: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !BORDERED_OVERLAYS.contains(&name.as_str()) {
            continue;
        }
        for (number, line) in &source.lines {
            if line.contains("padding(0, 0)") {
                flush.push(format!("{name}:{number}"));
            }
        }
    }
    assert!(
        flush.is_empty(),
        "bordered overlays painting flush against their border: {flush:?}"
    );
}

/// Truncation always says it truncated, with the one ellipsis glyph.
#[test]
fn truncation_has_ellipsis() {
    let system = DesignSystem::junie();
    let ellipsis = system.glyphs.ellipsis();
    assert_eq!(ellipsis, "…", "the junie ellipsis is canonical");
    for (name, panel) in [
        (
            "bordered",
            Panel::new(&system).title("a title far wider than the chrome it was given"),
        ),
        (
            "quiet",
            Panel::quiet(&system).title("a title far wider than the chrome it was given"),
        ),
    ] {
        let area = Rect::new(0, 0, 24, 4);
        let buffer = painted(area, |buffer| {
            Widget::render(&panel, area, buffer);
        });
        let painted_text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(
            painted_text.contains(ellipsis),
            "{name} clipped its title with no ellipsis: {painted_text:?}"
        );
    }
}

/// Deterministic pseudo-random sizes: a fuzz that reproduces.
fn lcg(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    *seed >> 33
}

#[test]
fn flagship_widgets_survive_tiny_and_random_geometry() {
    use termrock::widgets::{StatusBar, StatusBarState, StatusSlot, TextInput, TextInputState};

    let system = DesignSystem::junie();
    let mut seed = 0x5eed_1234_u64;
    for round in 0..200 {
        let (width, height) = if round < 4 {
            [(20u16, 5u16), (1, 1), (0, 4), (3, 2)][round]
        } else {
            (
                u16::try_from(lcg(&mut seed) % 60).unwrap_or(0),
                u16::try_from(lcg(&mut seed) % 20).unwrap_or(0),
            )
        };
        let area = Rect::new(0, 0, width, height);
        let _ = painted(area, |buffer| {
            Widget::render(
                &Panel::new(&system).title("panel").footer("esc close"),
                area,
                buffer,
            );
        });
        let _ = painted(area, |buffer| {
            let mut state = TextInputState::new("a value long enough to need contraction");
            let _ = TextInput::new("Label", &system).paint(area, buffer, &mut state);
        });
        let _ = painted(area, |buffer| {
            let slots = [
                StatusSlot::new("mode", "edit"),
                StatusSlot::new("branch", "main"),
            ];
            let mut state = StatusBarState::<&str>::new();
            StatefulWidget::render(
                &StatusBar::new(&slots, &[], &system),
                area,
                buffer,
                &mut state,
            );
        });
    }
}

/// Modal placement yields to the terminal instead of asserting a minimum.
#[test]
fn modal_geometry_never_escapes_its_terminal() {
    use termrock::layout::{ModalSpec, modal_rect};
    use termrock::patterns::{dialog_modal_rect, diff_modal_rect, permission_modal_rect};

    let specs = [
        ModalSpec::new(3, 4, 16).height(1, 3, 6),
        ModalSpec::new(3, 5, 28).height(1, 2, 8),
        ModalSpec::new(5, 6, 24).height(1, 3, 10),
        ModalSpec::new(9, 1, 400).height(9, 1, 400),
    ];
    for width in 0..=64u16 {
        for height in 0..=24u16 {
            let area = Rect::new(3, 2, width, height);
            let mut rects = vec![
                permission_modal_rect(area),
                dialog_modal_rect(area),
                diff_modal_rect(area),
            ];
            rects.extend(specs.iter().map(|spec| modal_rect(area, *spec)));
            for rect in rects {
                assert!(
                    rect.x >= area.x
                        && rect.y >= area.y
                        && rect.right() <= area.right()
                        && rect.bottom() <= area.bottom(),
                    "modal {rect:?} escaped {area:?}"
                );
            }
        }
    }
}

/// Composed patterns keep painting with an overlay open at any size.
#[test]
fn workbench_overlays_survive_tiny_and_random_geometry() {
    use termrock::patterns::{
        AgentWorkbenchState, WorkbenchSurfaces, default_modes, render_agent_workbench,
    };
    use termrock::widgets::{
        ListRow, PermissionPrompt, PermissionPromptState, PermissionRequest, PromptComposer,
        PromptComposerState, StatusBarState, StatusSlot, Transcript, TranscriptState,
    };

    let system = DesignSystem::junie();
    let mut seed = 0xfeed_9876_u64;
    for round in 0..80 {
        let (width, height) = if round < 5 {
            [(20u16, 5u16), (1, 1), (0, 4), (3, 2), (44, 9)][round]
        } else {
            (
                u16::try_from(lcg(&mut seed) % 90).unwrap_or(0),
                u16::try_from(lcg(&mut seed) % 30).unwrap_or(0),
            )
        };
        let area = Rect::new(0, 0, width, height);

        let mut workbench = AgentWorkbenchState::new();
        let mut permission_state = PermissionPromptState::new();
        let _ = permission_state.enqueue(
            PermissionRequest::new("req-1", "shell", "repository")
                .command("rm -rf build")
                .expected("nothing runs until you decide"),
        );
        let permission = PermissionPrompt::new(&system);
        let mut prompt_state = PromptComposerState::new();
        let prompt = PromptComposer::new(&system);
        let mut transcript_state = TranscriptState::<&str>::new();
        let blocks = [];
        let transcript = Transcript::new(&blocks, &system);
        let mut status_state = StatusBarState::<&str>::new();
        let slots = [StatusSlot::mode("mode", "busy")];
        let modes = default_modes("build");
        let tasks: [ListRow<'_, &'static str>; 0] = [];

        let _ = painted(area, |buffer| {
            render_agent_workbench(
                buffer,
                area,
                WorkbenchSurfaces {
                    system: &system,
                    state: &mut workbench,
                    task_models: None,
                    tasks: &tasks,
                    modes: &modes,
                    transcript: &transcript,
                    transcript_state: &mut transcript_state,
                    activities: None,
                    prompt: &prompt,
                    prompt_state: &mut prompt_state,
                    status_slots: &slots,
                    status_state: &mut status_state,
                    permission: Some((&permission, &mut permission_state)),
                    question: None,
                    plan: None,
                    diff: None,
                    session: None,
                    working: None,
                },
            );
        });
    }
}

// ── K. Scroll (D9) ───────────────────────────────────────────────────────────

#[test]
fn one_scrollbar_language() {
    // Thumb and track roles belong to `scroll::render`. A widget that resolves
    // them itself is painting a second scrollbar language.
    let mut local: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (number, line) in &source.lines {
            if line.contains("Role::ScrollThumb") || line.contains("Role::ScrollTrack") {
                local.push(format!("{name}:{number}"));
            }
        }
    }
    assert!(
        local.is_empty(),
        "widgets resolving scrollbar roles instead of calling scroll::render: {local:?}"
    );
}

/// `a_scrolled_region_says_it_continues` rewritten (D9): junie has no edge
/// fade. The scrollbar is the one continuation cue — a track and a thumb in
/// the scroll roles — and the row under the cut keeps its own tone.
#[test]
fn a_scrolled_region_says_it_continues() {
    let system = DesignSystem::junie();
    let track = system.style(Role::ScrollTrack);
    let thumb = system.style(Role::ScrollThumb);

    // One scrollbar language: a `┃` thumb over a dim track, both from the
    // shared painter, both in the scroll roles.
    let area = Rect::new(0, 0, 1, 5);
    let buffer = painted(area, |buffer| {
        render_scrollbar(
            buffer,
            area,
            ScrollbarSpec::new(ScrollAxis::Vertical, ScrollbarGeometry::new(20, 5, 2)),
            &system,
        );
    });
    let thumb_glyph = ScrollbarStyle::Line.vertical_thumb();
    let thumb_rows: Vec<u16> = (0..area.height)
        .filter(|y| buffer[(area.x, area.y + y)].symbol() == thumb_glyph)
        .collect();
    assert!(
        !thumb_rows.is_empty() && thumb_rows.len() < usize::from(area.height),
        "a scrolled region paints a thumb, never a full or empty track"
    );
    for y in 0..area.height {
        let cell = &buffer[(area.x, area.y + y)];
        let expected = if cell.symbol() == thumb_glyph {
            thumb.fg
        } else {
            track.fg
        };
        assert_eq!(
            Some(cell.fg),
            expected,
            "scrollbar row {y} speaks the wrong role"
        );
    }
    assert_eq!(track.fg, Some(WHITE_15));
    assert_eq!(thumb.fg, Some(WHITE_50));

    // Nothing is painted when the content fits: a reserved gutter stays blank
    // rather than showing a full-height thumb.
    let quiet = painted(area, |buffer| {
        paint_overflow_scrollbar(buffer, area, 5, 5, 0, false, &system);
    });
    assert!(
        quiet
            .content()
            .iter()
            .all(|cell| cell.symbol().trim().is_empty()),
        "a fitting region must not paint a scrollbar"
    );

    // No edge fade: the visible rows are identical whether or not more content
    // follows, which is the property the deleted `paint_scroll_edges` owned.
    let gutter = Rect::new(8, 0, 1, 3);
    let with_more = painted(Rect::new(0, 0, 10, 3), |buffer| {
        paint_overflow_scrollbar(buffer, gutter, 9, 3, 0, false, &system);
    });
    let at_end = painted(Rect::new(0, 0, 10, 3), |buffer| {
        paint_overflow_scrollbar(buffer, gutter, 3, 3, 0, false, &system);
    });
    for y in 0..3 {
        for x in 0..8 {
            let a = &with_more[(x, y)];
            let b = &at_end[(x, y)];
            assert_eq!(
                (a.symbol(), a.fg, a.bg, a.modifier),
                (b.symbol(), b.fg, b.bg, b.modifier),
                "the cut edge is faded: content at {x},{y} depends on what follows it"
            );
        }
    }

    // And the scrollbar painter is still the only entry point widgets reach.
    let mut bare: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let paints_bar = source.lines.iter().any(|(_, line)| {
            line.contains("paint_overflow_scrollbar(") || line.contains("render_scrollbar(")
        });
        let goes_through_the_authority = source
            .lines
            .iter()
            .any(|(_, line)| line.contains("paint_overflow_scrollbar("));
        if paints_bar && !goes_through_the_authority && name != "scroll_area.rs" {
            bare.push(name);
        }
    }
    assert!(
        bare.is_empty(),
        "scrollbar painters must go through the shared scroll authority: {bare:?}"
    );
    // `paint_scroll_edges` is deleted; the track constant is the vocabulary.
    assert_eq!(SCROLLBAR_TRACK, "│");
    assert_eq!(ScrollbarStyle::Line.vertical_thumb(), "┃");
}

// ── L. Architecture lints ────────────────────────────────────────────────────

#[test]
fn patterns_only_compose() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read pattern");
        let source: String = body
            .lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for needle in ["set_stringn(", "cell_mut("] {
            if source.contains(needle) {
                offenders.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "raw buffer paint in an example — compose a widget, or report the missing widget:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn patterns_have_charter_docs() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        if path.ends_with("mod.rs") {
            continue;
        }
        let body = fs::read_to_string(&path).expect("read pattern");
        if !body.contains("//! Teaches:") {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "example without a `//! Teaches:` header — say what assembly it teaches:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn widgets_never_import_patterns() {
    let dir = crate_src().join("widgets");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read widget");
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("crate::patterns") || trimmed.contains("super::patterns") {
                offenders.push(format!("{}:{}: {trimmed}", path.display(), i + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a widget depends on an example; the dependency runs the other way:\n{}",
        offenders.join("\n")
    );
}

/// One chip family: no second bracket-paint body.
#[test]
fn one_chip_recipe() {
    let src = crate_src();
    let mut offenders = Vec::new();
    for (file, allowance) in [("tag_chip.rs", 1usize), ("kbd.rs", 1usize)] {
        let body = fs::read_to_string(src.join("widgets").join(file)).expect("read widget");
        let paint_only: String = body
            .lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let bodies = paint_only.matches("String::from(\"[\"").count()
            + paint_only.matches("format!(\"[{").count();
        if bodies > allowance {
            offenders.push(format!(
                "{file}: {bodies} bracket-composing bodies (allowed {allowance})"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "a second chip/keycap painter appeared; route it through the shared one:\n{}",
        offenders.join("\n")
    );
}

// ── M. Recipes and monochrome ────────────────────────────────────────────────

/// Every public component family resolves through one semantic contract.
#[test]
fn recipe_families_are_complete_and_restrained() {
    let system = DesignSystem::junie();
    let ids = RecipeFamily::ALL.map(RecipeFamily::id);
    assert_eq!(
        ids,
        [
            "action",
            "input",
            "collection",
            "overlay",
            "status",
            "data",
            "layout"
        ]
    );

    for family in RecipeFamily::ALL {
        let recipe = system.family_recipe(family);
        assert_eq!(recipe.family, family);
        assert!(!recipe.non_color_cue.id().is_empty());
        assert_ne!(
            system.style(recipe.primary),
            system.style(recipe.secondary),
            "{family:?} flattened text hierarchy"
        );
    }

    // junie's accent classes: the primary intent, the focus indicator, a small
    // semantic mark, and nothing at all for structure.
    assert_eq!(
        system.family_recipe(RecipeFamily::Action).accent,
        AccentUsage::PrimaryIntent
    );
    for family in [
        RecipeFamily::Input,
        RecipeFamily::Collection,
        RecipeFamily::Overlay,
    ] {
        assert_eq!(system.family_recipe(family).accent, AccentUsage::FocusOnly);
    }
    for family in [RecipeFamily::Status, RecipeFamily::Data] {
        assert_eq!(
            system.family_recipe(family).accent,
            AccentUsage::SemanticMark
        );
    }
    assert_eq!(
        system.family_recipe(RecipeFamily::Layout).accent,
        AccentUsage::None
    );

    // Every family that can own interaction names its focus vocabulary.
    let emphasis = [
        (RecipeFamily::Action, NonColorCue::WeightedLabel),
        (RecipeFamily::Input, NonColorCue::PromptGlyph),
        (RecipeFamily::Collection, NonColorCue::SelectionGlyph),
        (RecipeFamily::Overlay, NonColorCue::FramedTitle),
        (RecipeFamily::Status, NonColorCue::GlyphAndLabel),
        (RecipeFamily::Data, NonColorCue::TieredText),
        (RecipeFamily::Layout, NonColorCue::BorderedRegion),
    ];
    for (family, cue) in emphasis {
        assert_eq!(system.family_recipe(family).non_color_cue, cue);
    }
    // The row family's cue is the gutter, and the theme states it once.
    assert_eq!(
        system.focus_emphasis(SurfaceFamily::Row),
        termrock::style::FocusEmphasis::FocusTint
    );
    assert_eq!(
        system.focus_emphasis(SurfaceFamily::Container),
        termrock::style::FocusEmphasis::BrightBorder
    );
}

/// Every exact public UI owner joins to one recipe and one monochrome proof.
#[test]
fn public_ui_inventory_has_exact_recipe_and_monochrome_evidence() {
    use std::collections::BTreeSet;

    use termrock::{
        registry::{ComponentFamily, DocumentationKind, public_ui_inventory},
        style::RecipeFamily,
    };

    fn recipe_family(family: ComponentFamily) -> RecipeFamily {
        #[allow(unreachable_patterns)]
        match family {
            ComponentFamily::Action => RecipeFamily::Action,
            ComponentFamily::Input => RecipeFamily::Input,
            ComponentFamily::Navigation => RecipeFamily::Collection,
            ComponentFamily::Data | ComponentFamily::Visualization | ComponentFamily::Content => {
                RecipeFamily::Data
            }
            ComponentFamily::Feedback => RecipeFamily::Status,
            ComponentFamily::Overlay => RecipeFamily::Overlay,
            ComponentFamily::Layout => RecipeFamily::Layout,
            other => panic!("unmapped public UI family: {other:?}"),
        }
    }

    let inventory = public_ui_inventory();
    let mono = DesignSystem::junie().no_color();
    let mut ids = BTreeSet::new();
    let mut documentation = BTreeSet::new();
    let mut evidence = Vec::with_capacity(inventory.len());

    for entry in inventory {
        assert!(ids.insert(entry.id.as_str()), "duplicate recipe evidence");
        documentation.insert(entry.documentation);
        let family = recipe_family(entry.family);
        let recipe = mono.family_recipe(family);
        assert_eq!(recipe.family, family);
        assert!(
            !recipe.non_color_cue.id().is_empty(),
            "{} / {} has no monochrome cue evidence",
            entry.id,
            entry.representative_story
        );
        assert_ne!(
            mono.style(recipe.primary),
            mono.style(recipe.secondary),
            "{} / {} loses hierarchy in monochrome",
            entry.id,
            entry.representative_story
        );
        assert!(
            entry.representative_story.split_once('/').is_some(),
            "{} lacks representative story evidence",
            entry.id
        );
        evidence.push((
            entry.id,
            entry.representative_story,
            recipe.family,
            recipe.non_color_cue,
        ));
    }

    assert_eq!(evidence.len(), inventory.len());
    assert_eq!(ids.len(), inventory.len());
    assert!(documentation.contains(&DocumentationKind::Component));
    assert!(documentation.contains(&DocumentationKind::Pattern));
}

/// `family_focus_and_selection_have_non_color_cues` rewritten (D5/D6): focus
/// and selection survive monochrome through the gutter glyph and weight, never
/// through a reversal or a dim.
#[test]
fn family_focus_and_selection_survive_monochrome() {
    use ratatui_core::style::Style;

    let system = DesignSystem::junie();
    let mono = system.clone().no_color();

    // Container: focus moves the border up the neutral ladder and adds weight.
    let panel = mono.panel_recipe(PanelChrome::Focused, Elevation::Overlay);
    let idle_panel = mono.panel_recipe(PanelChrome::Normal, Elevation::Overlay);
    assert!(!panel.border.add_modifier.contains(Modifier::BOLD));
    assert!(!idle_panel.border.add_modifier.contains(Modifier::BOLD));
    assert_ne!(panel.border, idle_panel.border);
    // junie's focused panel has no title marker: the frame is the cue.
    assert_eq!(panel.title_prefix, None);
    assert_eq!(
        mono.panel_recipe(PanelChrome::Danger, Elevation::Overlay)
            .title_prefix,
        Some("!"),
        "danger chrome states the risk in the title"
    );

    // Row: the gutter glyph and the weight are the non-colour cues.
    let selected = mono.resolve_list_row(row_state(true, true, false));
    let (glyph, _) = selected.gutter;
    assert_eq!(glyph, "▎");
    assert!(selected.label.add_modifier.contains(Modifier::BOLD));
    assert_ne!(
        mono.resolve_list_row(row_state(false, false, false)).gutter,
        selected.gutter
    );

    // Field: the prompt glyph survives the collapse.
    let field = mono.input_recipe(ControlState::Focused, false, false);
    assert_eq!(
        field.prompt.expect("prompt glyph").0,
        "▎",
        "the field focus cue is a glyph, not a colour"
    );

    // And no monochrome recipe leans on a banned modifier.
    let check = |style: Style, what: &str| {
        assert!(!style.add_modifier.contains(Modifier::REVERSED), "{what}");
        assert!(!style.add_modifier.contains(Modifier::DIM), "{what}");
    };
    check(panel.border, "panel border");
    check(selected.label, "list row label");
    check(field.border, "field border");
    for variant in [
        ButtonRecipeVariant::Primary,
        ButtonRecipeVariant::Secondary,
        ButtonRecipeVariant::Quiet,
        ButtonRecipeVariant::Destructive,
        ButtonRecipeVariant::Link,
    ] {
        for state in [
            ControlState::Default,
            ControlState::Hovered,
            ControlState::Focused,
            ControlState::Pressed,
            ControlState::Disabled,
        ] {
            let recipe = mono.button_recipe(variant, state, CHROME);
            check(recipe.label, "{variant:?} label");
            check(recipe.fill, "{variant:?} fill");
        }
    }
}

/// `bold_budget_per_row` rewritten (D9): one bold run per focused row. The
/// label carries it while the row owns the keyboard; metadata never does, and
/// title/keyword chrome is the named exemption.
#[test]
fn bold_budget_per_row() {
    use ratatui_core::style::Style;

    let system = DesignSystem::junie();
    for selected in [false, true] {
        for focused in [false, true] {
            for hovered in [false, true] {
                for enabled in [false, true] {
                    let state = ListRowVisualState {
                        selected,
                        focused,
                        hovered,
                        enabled,
                        error: false,
                        pressed: false,
                        loading: false,
                        checked: false,
                    };
                    let recipe = system.resolve_list_row(state);
                    // Exactly one bold run: the label, and only with focus.
                    assert_eq!(
                        recipe.label.add_modifier.contains(Modifier::BOLD),
                        focused && enabled,
                        "label weight for (selected={selected}, focused={focused}, \
                         hovered={hovered}, enabled={enabled})"
                    );
                    let meta = [
                        ("secondary", recipe.secondary),
                        ("shortcut", recipe.shortcut),
                    ];
                    for (name, style) in meta {
                        assert_eq!(
                            style.add_modifier,
                            Modifier::empty(),
                            "list row (selected={selected}, focused={focused}, \
                             hovered={hovered}) paints {name} with modifiers"
                        );
                    }
                    // Hover and focus never arrive as italics either.
                    let check = |style: Style, what: &str| {
                        assert!(!style.add_modifier.contains(Modifier::ITALIC), "{what}");
                    };
                    check(recipe.label, "label");
                    check(recipe.secondary, "secondary");
                }
            }
        }
    }

    // The named exemptions: titles and hint keys.
    let theme = JunieTheme::junie();
    for bold in [theme.title(), theme.label(true), theme.key_hint_key()] {
        assert!(bold.add_modifier.contains(Modifier::BOLD));
    }
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

use termrock::widgets::{
    List, ListRow, ListState, ProgressBar, ProgressKind, RowRole,
    SPINNER_BRAILLE_FRAMES as SPINNER_FRAMES, SPINNER_DEFAULT_PERIOD_MS as SPINNER_PERIOD_MS,
    Skeleton, SkeletonState, Spinner, SpinnerState, TextInput, TextInputState,
};

/// An enabled row in the three interaction states every gate varies.
fn row_state(selected: bool, focused: bool, hovered: bool) -> ListRowVisualState {
    ListRowVisualState {
        selected,
        focused,
        hovered,
        enabled: true,
        error: false,
        pressed: false,
        loading: false,
        checked: false,
    }
}

/// Three plain rows for the collection gates.
fn rows() -> [ListRow<'static, &'static str>; 3] {
    ["alpha", "beta", "gamma"].map(|id| ListRow {
        id,
        label: Line::from(id),
        leading: None,
        secondary: None,
        status: None,
        badge: None,
        shortcut: None,
        actions: None,
        custom: None,
        role: RowRole::Item,
        enabled: true,
        loading: false,
    })
}

use termrock::patterns::{
    AgentStatusHeader, AgentStatusHeaderState, AgentStatusPresentation, ConnectionManager,
    ConnectionManagerState, DatabaseWorkbenchState, DatabaseWorkbenchSurfaces, IntegrationStatus,
    IntegrationStatusPresentation, IntegrationStatusState, ObservabilityDashboardState,
    ObservabilityDashboardSurfaces, PlanReview, PlanReviewState, SessionPicker, SessionPickerState,
    example_agent_status, example_connections, example_db_commands, example_db_history,
    example_inspect_fields, example_integrations, example_log_inspect_fields,
    example_observability_alerts, example_observability_events, example_observability_logs,
    example_observability_tiles, example_plan_document, example_result_columns,
    example_result_row_refs, example_result_rows, example_schema_entries, example_sessions,
    render_database_workbench, render_observability_dashboard,
};
use termrock::widgets::{Panel, Tab, Tabs, TabsState};
