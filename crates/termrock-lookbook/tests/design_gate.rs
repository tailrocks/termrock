// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Executable semantic-recipe evidence for the canonical public UI inventory.

use std::collections::BTreeSet;

use ratatui::style::Modifier;
use termrock::{
    registry::{ComponentFamily, public_ui_inventory},
    style::{ColorCapability, NonColorCue, RecipeFamily, RolePalette},
};
use termrock_lookbook::{
    design::lookbook_system,
    frame::{STORY_PAD, paint_story_buffer, story_by_id},
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

/// Every inventory claim reaches the real painter with the no-color system.
/// The painted cells, rather than registry metadata, must retain the structure
/// promised by that entry's enforced family recipe.
#[test]
fn every_public_ui_representative_paints_its_recipe_cue_without_color() {
    let system = lookbook_system(RolePalette::default()).no_color();
    assert_eq!(system.capability, ColorCapability::Monochrome);
    // A second no-color paint tags every semantic role. Seeing this modifier
    // in the real output proves the fixture consumed the supplied design
    // system instead of painting a self-owned hard-coded style.
    let mut probe = system.clone();
    for role in RolePalette::roles() {
        let style = probe.style(role).add_modifier(Modifier::CROSSED_OUT);
        probe.palette = probe.palette.with_role(role, style);
    }
    let mut painted_ids = BTreeSet::new();
    let mut missing_cues = Vec::new();
    let mut missing_recipe_consumption = Vec::new();

    for entry in public_ui_inventory() {
        let story = story_by_id(entry.representative_story).unwrap_or_else(|| {
            panic!(
                "{} has no executable representative {}",
                entry.id, entry.representative_story
            )
        });
        assert_eq!(
            story.public_ui_id(),
            Some(entry.id),
            "{} representative is owned by another public surface",
            entry.id
        );

        let contract = system.family_recipe(recipe_family(entry.family));
        let buffer = paint_story_buffer(story, &system, None, None);
        let probe_buffer = paint_story_buffer(story, &probe, None, None);
        let mut has_label = false;
        let mut has_structural_glyph = false;
        let mut has_emphasis = false;
        let mut consumed_system_role = false;
        let mut modifiers = BTreeSet::new();
        let mut painted = 0usize;

        for y in STORY_PAD..STORY_PAD + story.height.max(1) {
            for x in STORY_PAD..STORY_PAD + story.width.max(1) {
                let cell = buffer.cell((x, y)).expect("story cell inside paint area");
                let symbol = cell.symbol();
                let has_symbol = !symbol.trim().is_empty();
                if !has_symbol && cell.modifier.is_empty() {
                    continue;
                }
                painted += 1;
                if has_symbol {
                    has_label |= symbol.chars().any(char::is_alphabetic);
                    has_structural_glyph |= symbol
                        .chars()
                        .any(|ch| !ch.is_alphanumeric() && !ch.is_whitespace());
                }
                has_emphasis |= cell.modifier.intersects(
                    Modifier::BOLD
                        | Modifier::DIM
                        | Modifier::ITALIC
                        | Modifier::UNDERLINED
                        | Modifier::REVERSED
                        | Modifier::CROSSED_OUT,
                );
                modifiers.insert(cell.modifier.bits());
                consumed_system_role |= probe_buffer
                    .cell((x, y))
                    .expect("probe cell inside paint area")
                    .modifier
                    .contains(Modifier::CROSSED_OUT);
            }
        }

        if painted == 0 {
            missing_cues.push(format!("{} / {} (empty paint)", entry.id, story.id));
            assert!(painted_ids.insert(entry.id), "duplicate paint evidence");
            continue;
        }
        if !consumed_system_role && story.id != "backdrop/basic" {
            // Junie backdrop() clears modifiers (D5). The CROSSED_OUT probe
            // cannot survive that resolver; the widget still reads JunieTheme.
            missing_recipe_consumption.push(format!("{} / {}", entry.id, story.id));
        }
        let cue_visible = match contract.non_color_cue {
            NonColorCue::WeightedLabel => has_emphasis || has_structural_glyph,
            NonColorCue::PromptGlyph => has_structural_glyph || has_label,
            NonColorCue::SelectionGlyph => has_structural_glyph || has_emphasis,
            NonColorCue::FramedTitle | NonColorCue::GlyphAndLabel => {
                has_structural_glyph || has_emphasis || has_label
            }
            NonColorCue::TieredText => has_label || modifiers.len() > 1,
            NonColorCue::BorderedRegion => has_structural_glyph || has_label,
            #[allow(unreachable_patterns)]
            other => panic!("unverified non-color cue: {other:?}"),
        };
        if !cue_visible {
            missing_cues.push(format!(
                "{} / {} ({})",
                entry.id,
                story.id,
                contract.non_color_cue.id()
            ));
        }
        assert!(painted_ids.insert(entry.id), "duplicate paint evidence");
    }

    assert_eq!(painted_ids.len(), public_ui_inventory().len());
    assert!(
        missing_recipe_consumption.is_empty(),
        "representatives that ignored the supplied semantic role palette:\n{}",
        missing_recipe_consumption.join("\n")
    );
    assert!(
        missing_cues.is_empty(),
        "representatives without painted recipe cues:\n{}",
        missing_cues.join("\n")
    );
}
