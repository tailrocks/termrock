// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The contrast floor — one computed barrier instead of case-by-case fixes.
//!
//! Contrast defects kept arriving one at a time (disabled text lost on the
//! canvas, ladder steps a rounding error apart, a label patched over an
//! unrelated fill) because nothing measured them. This module resolves every
//! foreground/background pair the design language actually paints, measures it
//! with [`super::contrast_ratio`], and holds it to a documented minimum.
//!
//! Pairs that cannot meet their floor with today's values are listed in
//! [`KNOWN_SHORTFALLS`] with the plan that fixes them. The list is exact in
//! both directions: a pair that starts passing must be removed from it, so the
//! barrier can never rot into a permanent exemption.

use ratatui_core::style::Style;

use super::{
    ButtonRecipeVariant, ControlState, DesignSystem, ListRowVisualState, PanelChrome, Rgb, Role,
    RolePalette, contrast_ratio,
};

/// Surface roles that content is painted onto.
const SURFACES: [Role; 5] = [
    Role::Canvas,
    Role::Surface,
    Role::Raised,
    Role::Elevated,
    Role::Sunken,
];

/// One measured pair and the minimum it must clear.
struct Measured {
    preset: &'static str,
    pair: String,
    ratio: f32,
    floor: f32,
}

impl Measured {
    fn passes(&self) -> bool {
        self.ratio + 0.005 >= self.floor
    }

    fn key(&self) -> (&'static str, &str) {
        (self.preset, self.pair.as_str())
    }
}

/// Pairs that miss their floor with the palette values on `main` today.
///
/// Every entry is a palette VALUE defect, and every one of them is retired by
/// the graphite ladder in `plans/002-role-palette-foundation.md` (phosphor +
/// high-contrast + paper presets). The floor test asserts these still fail:
/// when a palette change fixes one, its line must be deleted in the same
/// commit, and `plans/017` re-runs the whole table then.
///
const KNOWN_SHORTFALLS: &[(&str, &str)] = &[
    // The surface ladder cannot clear 1.15 per step while one `Border` tone
    // stays a visible hairline on every rung of it. A ladder that steps by
    // 1.15 spans 1.52 from Canvas to Elevated, and a border 1.3 clear of both
    // ends needs 1.69 — so the two rows of the floor table are mutually
    // unsatisfiable with a single border role. Candidate dark ladder measured
    // for the record: Canvas (10,12,10) → Surface (31,35,33) → Raised
    // (45,49,47) → Elevated (57,61,59) at 1.20 per step; it needs a second
    // border tone (or a per-elevation border) to keep hairlines visible.
    // Reported under plans/017 Part A STOP: palette identity is a design call.
    ("phosphor", "ladder Canvas->Surface"),
    ("phosphor", "ladder Surface->Raised"),
    ("phosphor", "ladder Raised->Elevated"),
    ("paper", "ladder Canvas->Surface"),
    ("paper", "ladder Surface->Raised"),
    ("paper", "ladder Raised->Elevated"),
    // The high-contrast preset paints Surface on Canvas deliberately: one
    // black ground, maximum text contrast, structure carried by borders.
    ("high_contrast", "ladder Canvas->Surface"),
    ("high_contrast", "ladder Surface->Raised"),
    ("high_contrast", "ladder Raised->Elevated"),
];
fn presets() -> Vec<(&'static str, RolePalette)> {
    vec![
        ("phosphor", RolePalette::tailrocks_phosphor()),
        ("slate", RolePalette::slate()),
        ("paper", RolePalette::paper()),
        ("high_contrast", RolePalette::high_contrast()),
    ]
}

fn fg_of(palette: &RolePalette, role: Role) -> Option<Rgb> {
    Rgb::from_color(palette.style(role).fg?)
}

fn bg_of(palette: &RolePalette, role: Role) -> Option<Rgb> {
    Rgb::from_color(palette.style(role).bg?)
}

/// Background a role paints on: its own fill, else the ordinary surface.
fn ground_of(palette: &RolePalette, role: Role) -> Option<Rgb> {
    bg_of(palette, role).or_else(|| bg_of(palette, Role::Surface))
}

fn style_ground(palette: &RolePalette, style: Style) -> Option<Rgb> {
    style
        .bg
        .and_then(Rgb::from_color)
        .or_else(|| bg_of(palette, Role::Surface))
}

fn measure(
    out: &mut Vec<Measured>,
    preset: &'static str,
    pair: impl Into<String>,
    fg: Option<Rgb>,
    bg: Option<Rgb>,
    floor: f32,
) {
    // Named ANSI and indexed colors resolve in the operator's terminal, so they
    // carry no value this crate can measure.
    let (Some(fg), Some(bg)) = (fg, bg) else {
        return;
    };
    out.push(Measured {
        preset,
        pair: pair.into(),
        ratio: contrast_ratio(fg, bg),
        floor,
    });
}

/// Text tiers are held to the strongest floor in the high-contrast preset.
fn text_floor(preset: &str, base: f32) -> f32 {
    if preset == "high_contrast" { 7.0 } else { base }
}

fn palette_pairs(preset: &'static str, palette: &RolePalette) -> Vec<Measured> {
    let mut out = Vec::new();
    for surface in SURFACES {
        for (role, base) in [(Role::Text, 7.0), (Role::TextStrong, 7.0)] {
            measure(
                &mut out,
                preset,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                text_floor(preset, base),
            );
        }
    }
    for surface in [Role::Canvas, Role::Surface] {
        for (role, base) in [
            (Role::TextMuted, 4.5),
            (Role::TextFaint, 3.0),
            (Role::TextDisabled, 2.5),
        ] {
            measure(
                &mut out,
                preset,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                text_floor(preset, base),
            );
        }
        for role in [Role::Danger, Role::Warning, Role::Info, Role::Success] {
            measure(
                &mut out,
                preset,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                4.5,
            );
        }
    }
    // Status colors on the tinted grounds they are painted over.
    for role in [Role::DiffAdded, Role::DiffRemoved, Role::InputInvalid] {
        measure(
            &mut out,
            preset,
            format!("{role:?} on its own tint"),
            fg_of(palette, role),
            ground_of(palette, role),
            4.5,
        );
    }
    for tint in [Role::SelectionTint, Role::HoverTint] {
        for role in [Role::Text, Role::TextStrong] {
            measure(
                &mut out,
                preset,
                format!("{role:?} on {tint:?}"),
                fg_of(palette, role),
                bg_of(palette, tint),
                4.5,
            );
        }
    }
    // A hairline still has to be visible.
    for surface in [Role::Canvas, Role::Surface, Role::Elevated] {
        measure(
            &mut out,
            preset,
            format!("Border on {surface:?}"),
            fg_of(palette, Role::Border),
            bg_of(palette, surface),
            1.3,
        );
    }
    // Elevation is only real when the eye can see the step.
    for pair in [
        (Role::Canvas, Role::Surface),
        (Role::Surface, Role::Raised),
        (Role::Raised, Role::Elevated),
    ] {
        measure(
            &mut out,
            preset,
            format!("ladder {:?}->{:?}", pair.0, pair.1),
            bg_of(palette, pair.0),
            bg_of(palette, pair.1),
            1.15,
        );
    }
    out
}

fn recipe_pairs(preset: &'static str, system: &DesignSystem) -> Vec<Measured> {
    let palette = &system.palette;
    let mut out = Vec::new();
    for variant in [
        ButtonRecipeVariant::Primary,
        ButtonRecipeVariant::Secondary,
        ButtonRecipeVariant::Destructive,
        ButtonRecipeVariant::Quiet,
        ButtonRecipeVariant::Outline,
        ButtonRecipeVariant::Link,
    ] {
        for state in [
            ControlState::Default,
            ControlState::Hovered,
            ControlState::Focused,
            ControlState::Pressed,
            ControlState::Disabled,
            ControlState::Loading,
        ] {
            let recipe = system.button_recipe(variant, state);
            let floor = if matches!(state, ControlState::Disabled | ControlState::Loading) {
                2.5
            } else {
                4.5
            };
            measure(
                &mut out,
                preset,
                format!("button {variant:?}/{state:?} label"),
                recipe.label.fg.and_then(Rgb::from_color),
                style_ground(palette, recipe.fill),
                floor,
            );
        }
    }
    for state in [
        ControlState::Default,
        ControlState::Focused,
        ControlState::Disabled,
    ] {
        for invalid in [false, true] {
            let recipe = system.input_recipe(state, invalid);
            let ground = style_ground(palette, recipe.fill);
            measure(
                &mut out,
                preset,
                format!("input {state:?}/invalid={invalid} value"),
                recipe.value.fg.and_then(Rgb::from_color),
                ground,
                if matches!(state, ControlState::Disabled) {
                    2.5
                } else {
                    4.5
                },
            );
            measure(
                &mut out,
                preset,
                format!("input {state:?}/invalid={invalid} placeholder"),
                recipe.placeholder.fg.and_then(Rgb::from_color),
                ground,
                2.5,
            );
        }
    }
    for selected in [false, true] {
        for focused in [false, true] {
            for hovered in [false, true] {
                let state = ListRowVisualState {
                    selected,
                    focused,
                    hovered,
                    enabled: true,
                    loading: false,
                    checked: false,
                };
                let recipe = system.resolve_list_row(state);
                let ground = if recipe.use_fill {
                    bg_of(palette, Role::Selection)
                } else if recipe.use_tint {
                    recipe
                        .tint
                        .bg
                        .and_then(Rgb::from_color)
                        .or_else(|| bg_of(palette, Role::SelectionTint))
                } else if hovered {
                    recipe
                        .hover_wash
                        .bg
                        .and_then(Rgb::from_color)
                        .or_else(|| bg_of(palette, Role::HoverTint))
                } else {
                    bg_of(palette, Role::Surface)
                };
                measure(
                    &mut out,
                    preset,
                    format!("list row s={selected}/f={focused}/h={hovered} label"),
                    recipe.label.fg.and_then(Rgb::from_color),
                    ground,
                    4.5,
                );
                measure(
                    &mut out,
                    preset,
                    format!("list row s={selected}/f={focused}/h={hovered} secondary"),
                    recipe.secondary.fg.and_then(Rgb::from_color),
                    ground,
                    2.5,
                );
            }
        }
    }
    for chrome in [
        PanelChrome::Normal,
        PanelChrome::Focused,
        PanelChrome::Danger,
    ] {
        let recipe = system.panel_recipe(chrome);
        let ground = style_ground(palette, recipe.surface);
        measure(
            &mut out,
            preset,
            format!("panel {chrome:?} title"),
            recipe.title.fg.and_then(Rgb::from_color),
            ground,
            4.5,
        );
        measure(
            &mut out,
            preset,
            format!("panel {chrome:?} border"),
            recipe.border.fg.and_then(Rgb::from_color),
            ground,
            1.3,
        );
    }
    out
}

fn report(measured: &[Measured]) -> String {
    let mut failures: Vec<String> = measured
        .iter()
        .filter(|m| !m.passes())
        .map(|m| {
            format!(
                "    (\"{}\", \"{}\"), // {:.2} < {:.2}",
                m.preset, m.pair, m.ratio, m.floor
            )
        })
        .collect();
    failures.sort();
    failures.join("\n")
}

fn assert_floor(measured: Vec<Measured>) {
    let known: std::collections::HashSet<(&str, &str)> = KNOWN_SHORTFALLS.iter().copied().collect();
    let unexpected: Vec<&Measured> = measured
        .iter()
        .filter(|m| !m.passes() && !known.contains(&m.key()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "contrast floor violated by {} pair(s):\n{}",
        unexpected.len(),
        report(&measured)
    );
    let fixed: Vec<(&str, &str)> = measured
        .iter()
        .filter(|m| m.passes() && known.contains(&m.key()))
        .map(Measured::key)
        .collect();
    assert!(
        fixed.is_empty(),
        "these pairs now clear their floor — delete them from KNOWN_SHORTFALLS: {fixed:?}"
    );
}

#[test]
fn contrast_floor_holds() {
    let mut measured = Vec::new();
    for (preset, palette) in presets() {
        measured.extend(palette_pairs(preset, &palette));
    }
    assert!(measured.len() > 100, "floor table lost its coverage");
    assert_floor(measured);
}

#[test]
fn recipe_pairs_pass_floor() {
    assert_floor(recipe_pairs("phosphor", &DesignSystem::phosphor()));
}

#[test]
fn floor_table_covers_every_text_tier() {
    // A new text tier that never reaches this table is a contrast defect
    // waiting to happen, so the table names the tiers it measures.
    let measured = palette_pairs("phosphor", &RolePalette::tailrocks_phosphor());
    for tier in ["Text", "TextStrong", "TextMuted", "TextFaint", "TextDisabled"] {
        assert!(
            measured
                .iter()
                .any(|m| m.pair.starts_with(&format!("{tier} on"))),
            "{tier} is not measured against any surface"
        );
    }
    for surface in SURFACES {
        assert!(
            measured
                .iter()
                .any(|m| m.pair.ends_with(&format!("on {surface:?}"))),
            "{surface:?} carries no measured text"
        );
    }
}

#[test]
fn disabled_and_faint_tiers_stay_distinguishable() {
    // Two tiers that measure the same are one tier with two names.
    for (preset, palette) in presets() {
        let ground = bg_of(&palette, Role::Canvas).expect("canvas fill");
        let Some(faint) = fg_of(&palette, Role::TextFaint) else {
            continue;
        };
        let Some(disabled) = fg_of(&palette, Role::TextDisabled) else {
            continue;
        };
        let gap = (contrast_ratio(faint, ground) - contrast_ratio(disabled, ground)).abs();
        assert!(
            gap >= 0.4,
            "{preset}: TextFaint and TextDisabled differ by only {gap:.2} against the canvas"
        );
    }
}

#[test]
fn contrast_ratio_matches_wcag_reference_points() {
    let black = Rgb::new(0, 0, 0);
    let white = Rgb::new(255, 255, 255);
    assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 0.001);
    // Order does not matter.
    assert!((contrast_ratio(black, white) - contrast_ratio(white, black)).abs() < f32::EPSILON);
    // WCAG reference: #767676 on white is the 4.5:1 boundary.
    let boundary = contrast_ratio(Rgb::new(0x76, 0x76, 0x76), white);
    assert!((4.4..=4.6).contains(&boundary), "{boundary}");
}
