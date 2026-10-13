// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The one design system the catalog paints with.
//!
//! Every story used to build its own with `DesignSystem::from_palette(theme)`,
//! which takes the palette's colours and *none* of the preset's other
//! decisions — so the catalog painted `SelectionChrome` defaults that no
//! shipped preset uses, and the selection language the design plans specify
//! never appeared in a single preview. One constructor answers instead, and
//! swapping a theme swaps colours only.

use termrock::style::{DesignSystem, RolePalette};

/// The catalog's design system for `theme`.
///
/// Shape (selection chrome, density, motion, glyphs) comes from the shipped
/// phosphor preset; only the palette changes with the theme picker.
#[must_use]
pub fn lookbook_system(theme: RolePalette) -> DesignSystem {
    let preset = DesignSystem::phosphor();
    DesignSystem::from_palette(theme)
        .selection(preset.selection)
        .density(preset.density)
        .motion(preset.motion)
        .glyphs(preset.glyphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_paints_the_preset_it_ships() {
        let system = lookbook_system(RolePalette::slate());
        let preset = DesignSystem::phosphor();
        assert_eq!(
            system.selection, preset.selection,
            "a preview must show the selection chrome the library ships"
        );
        assert_eq!(system.density, preset.density);
        assert_eq!(system.motion, preset.motion);
        // The palette is the one thing a theme swap changes.
        assert_ne!(
            system.style(termrock::style::Role::Canvas).bg,
            preset.style(termrock::style::Role::Canvas).bg
        );
    }
}
