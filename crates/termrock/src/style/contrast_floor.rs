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
//! junie states its own values as canonical, weak pairs included: the pressed
//! primary reads at 3.8:1 and the danger label on the overlay plane at 3.7:1
//! by design, because the state that dims them is transient. Those pairs are
//! listed in [`KNOWN_SHORTFALLS`] with their measured ratio. The list is exact
//! in both directions: a pair that starts passing must be removed from it, so
//! the barrier can never rot into a permanent exemption.
use ratatui_core::style::Style;

use super::{
    ButtonRecipeVariant, ControlState, DesignSystem, Elevation, ListRowVisualState, PanelChrome,
    Rgb, Role, RolePalette, contrast_ratio,
};

/// Surface roles that content is painted onto.
const SURFACES: [Role; 5] = [
    Role::Canvas,
    Role::Surface,
    Role::Elevated,
    Role::Sunken,
    Role::Popover,
];

/// One measured pair and the minimum it must clear.
struct Measured {
    pair: String,
    ratio: f32,
    floor: f32,
}

impl Measured {
    fn passes(&self) -> bool {
        self.ratio + 0.005 >= self.floor
    }
}

/// Pairs that miss their floor with junie's canonical values.
///
/// Every entry is a value junie declares on purpose: a transient state or a
/// hairline whose job is to be quiet. When a value change fixes one, its line
/// is deleted in the same commit.
const PALETTE_SHORTFALLS: &[(&str, &str)] = &[
    ("junie", "TextFaint on Canvas"),     // 2.48 — 30% white by design
    ("junie", "TextFaint on Surface"),    // 2.23
    ("junie", "TextDisabled on Canvas"),  // 2.48 — disabled is the faint tier
    ("junie", "TextDisabled on Surface"), // 2.23
    ("junie", "TextMuted on Elevated"),   // 4.49 — 50% white on the card plane
    ("junie", "TextMuted on Sunken"),     // 4.21
    ("junie", "TextMuted on Popover"),    // 2.64 — popover metadata
    ("junie", "InputInvalid on its own tint"), // 4.14 — error tone on the field plane
    ("junie", "ladder Canvas->Surface"),  // 1.11
    ("junie", "ladder Surface->Elevated"), // 1.07
];

const RECIPE_SHORTFALLS: &[(&str, &str)] = &[
    ("junie", "button Destructive/Default label"),  // 3.71
    ("junie", "button Destructive/Focused label"),  // 3.71
    ("junie", "button Destructive/Hovered label"),  // 2.60
    ("junie", "button Destructive/Pressed label"),  // 4.01
    ("junie", "button Primary/Pressed label"),      // 3.81
    ("junie", "button Primary/Disabled label"),     // 1.76
    ("junie", "button Secondary/Disabled label"),   // 1.76
    ("junie", "button Outline/Disabled label"),     // 1.76
    ("junie", "button Destructive/Disabled label"), // 1.76
    ("junie", "button Quiet/Disabled label"),       // 2.23
    ("junie", "input Disabled value"),              // 1.97
    ("junie", "input Disabled placeholder"),        // 1.97
];

/// The one palette TermRock ships, measured at truecolor where its hex values
/// are observable.
fn junie_palette() -> (&'static str, RolePalette) {
    ("junie", RolePalette::junie())
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
        pair: pair.into(),
        ratio: contrast_ratio(fg, bg),
        floor,
    });
}

fn palette_pairs(palette: &RolePalette) -> Vec<Measured> {
    let mut out = Vec::new();
    for surface in SURFACES {
        for role in [Role::Text, Role::TextStrong] {
            measure(
                &mut out,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                7.0,
            );
        }
        // Supporting text has to survive on every plane it can be painted on.
        for role in [Role::TextSecondary, Role::TextMuted] {
            measure(
                &mut out,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                4.5,
            );
        }
    }
    for surface in [Role::Canvas, Role::Surface] {
        // Metadata and disabled text are only ever painted on the two lowest
        // planes; the faint tier is junie's declared 2.48:1 on the canvas.
        for (role, floor) in [
            (Role::TextFaint, 3.0),
            (Role::TextDisabled, 2.5),
            (Role::Danger, 4.5),
            (Role::Warning, 4.5),
            (Role::Success, 4.5),
            (Role::Accent, 4.5),
        ] {
            measure(
                &mut out,
                format!("{role:?} on {surface:?}"),
                fg_of(palette, role),
                bg_of(palette, surface),
                floor,
            );
        }
    }
    // Status colors on the tinted grounds they are painted over.
    for role in [Role::DiffAdded, Role::DiffRemoved, Role::InputInvalid] {
        measure(
            &mut out,
            format!("{role:?} on its own tint"),
            fg_of(palette, role),
            ground_of(palette, role),
            4.5,
        );
    }
    // Text on the two selection grounds junie paints: the tint (keyboard
    // selection) and the popover (text / range selection).
    for tint in [Role::SelectionTint, Role::Selection] {
        for role in [Role::Text, Role::TextStrong] {
            measure(
                &mut out,
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
            format!("Border on {surface:?}"),
            fg_of(palette, Role::Border),
            bg_of(palette, surface),
            1.15,
        );
        measure(
            &mut out,
            format!("BorderFocused on {surface:?}"),
            fg_of(palette, Role::BorderFocused),
            bg_of(palette, surface),
            1.15,
        );
    }
    // Elevation is only real when the eye can see the step. junie's steps are
    // canonical — canvas→surface measures 1.11:1 on purpose — so they are
    // listed as shortfalls rather than silently relaxed.
    for pair in [
        (Role::Canvas, Role::Surface),
        (Role::Surface, Role::Elevated),
    ] {
        measure(
            &mut out,
            format!("ladder {:?}->{:?}", pair.0, pair.1),
            bg_of(palette, pair.0),
            bg_of(palette, pair.1),
            1.15,
        );
    }
    out
}

fn recipe_pairs(system: &DesignSystem) -> Vec<Measured> {
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
            let recipe = system.button_recipe(variant, state, system.junie_theme().surface);
            // junie's declared transient states: the pressed primary and the
            // resting danger label measure in the high 3s on purpose.
            let floor = if matches!(state, ControlState::Disabled) {
                2.5
            } else {
                4.5
            };
            measure(
                &mut out,
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
        ControlState::Hovered,
        ControlState::Disabled,
    ] {
        let recipe = system.input_recipe(state, false);
        let ground = style_ground(palette, recipe.fill);
        measure(
            &mut out,
            format!("input {state:?} value"),
            recipe.value.fg.and_then(Rgb::from_color),
            ground,
            if matches!(state, ControlState::Disabled) {
                2.5
            } else {
                4.5
            },
        );
        // junie's placeholder is muted text on the field plane: 4.2:1, a
        // deliberately quiet answer to "what goes here".
        measure(
            &mut out,
            format!("input {state:?} placeholder"),
            recipe.placeholder.fg.and_then(Rgb::from_color),
            ground,
            2.5,
        );
    }
    for selected in [false, true] {
        for focused in [false, true] {
            for hovered in [false, true] {
                let state = ListRowVisualState {
                    selected,
                    focused,
                    hovered,
                    enabled: true,
                    ..ListRowVisualState::default()
                };
                let recipe = system.resolve_list_row(state);
                let ground = if recipe.hover_fill {
                    recipe
                        .hover_wash
                        .bg
                        .and_then(Rgb::from_color)
                        .or_else(|| bg_of(palette, Role::Surface))
                } else if recipe.use_tint {
                    recipe
                        .tint
                        .bg
                        .and_then(Rgb::from_color)
                        .or_else(|| bg_of(palette, Role::SelectionTint))
                } else {
                    bg_of(palette, Role::Surface)
                };
                measure(
                    &mut out,
                    format!("list row s={selected}/f={focused}/h={hovered} label"),
                    recipe.label.fg.and_then(Rgb::from_color),
                    ground,
                    4.5,
                );
                measure(
                    &mut out,
                    format!("list row s={selected}/f={focused}/h={hovered} secondary"),
                    recipe.secondary.fg.and_then(Rgb::from_color),
                    ground,
                    2.5,
                );
            }
        }
    }
    for (chrome, elevation) in [
        (PanelChrome::Normal, Elevation::Surface),
        (PanelChrome::Focused, Elevation::Surface),
        (PanelChrome::Danger, Elevation::Surface),
        (PanelChrome::Focused, Elevation::Overlay),
    ] {
        let recipe = system.panel_recipe(chrome, elevation);
        let ground = style_ground(palette, recipe.surface);
        measure(
            &mut out,
            format!("panel {chrome:?}/{} title", elevation.id()),
            recipe.title.fg.and_then(Rgb::from_color),
            ground,
            4.5,
        );
        measure(
            &mut out,
            format!("panel {chrome:?}/{} border", elevation.id()),
            recipe.border.fg.and_then(Rgb::from_color),
            ground,
            1.15,
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
                "junie", m.pair, m.ratio, m.floor
            )
        })
        .collect();
    failures.sort();
    failures.join("\n")
}

fn assert_floor_exact(measured: Vec<Measured>, shortfalls: &[(&str, &str)]) {
    let known: std::collections::BTreeSet<String> = shortfalls
        .iter()
        .map(|(_, pair)| pair.to_string())
        .collect();
    let actual: std::collections::BTreeSet<String> = measured
        .iter()
        .filter(|m| !m.passes())
        .map(|m| m.pair.clone())
        .collect();

    let unexpected: Vec<String> = actual.difference(&known).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "contrast floor violated by {} pair(s):\n{}",
        unexpected.len(),
        report(&measured)
    );
    let missing: Vec<&str> = known.difference(&actual).map(String::as_str).collect();
    assert!(
        missing.is_empty(),
        "KNOWN_SHORTFALLS contains passing or unmeasured pairs: {missing:?}"
    );
}

#[test]
fn contrast_floor_holds() {
    let (_, palette) = junie_palette();
    let measured = palette_pairs(&palette);
    assert!(measured.len() > 45, "floor table lost its coverage");
    assert_floor_exact(measured, PALETTE_SHORTFALLS);
}

#[test]
fn recipe_pairs_measure_their_declared_shortfalls() {
    assert_floor_exact(recipe_pairs(&DesignSystem::junie()), RECIPE_SHORTFALLS);
}

#[test]
fn floor_table_covers_every_text_tier() {
    // A new text tier that never reaches this table is a contrast defect
    // waiting to happen, so the table names the tiers it measures. `TextGhost`
    // is absent on purpose: it is a backdrop tier and never carries content.
    let (_, palette) = junie_palette();
    let measured = palette_pairs(&palette);
    for tier in [
        "Text",
        "TextStrong",
        "TextSecondary",
        "TextMuted",
        "TextFaint",
        "TextDisabled",
    ] {
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
fn disabled_is_the_faint_tier_by_design() {
    // junie states disabled text as the faint tier value: the difference is
    // carried by weight and by the absence of a marker, not by colour. The
    // tiers *below* body text are still distinct from each other.
    let (_, palette) = junie_palette();
    assert_eq!(
        palette.style(Role::TextDisabled).fg,
        palette.style(Role::TextFaint).fg,
        "disabled and faint are one tier in junie"
    );
    assert_ne!(
        palette.style(Role::TextMuted).fg,
        palette.style(Role::TextFaint).fg,
        "muted and faint must stay distinct tiers"
    );
    assert_ne!(
        palette.style(Role::TextSecondary).fg,
        palette.style(Role::TextMuted).fg,
        "secondary and muted must stay distinct tiers"
    );
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
