// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The one design system the catalog paints with.
//!
//! The catalog paints junie only. `junie_theme()` ignores custom palettes, so
//! there is no honest theme swap: every preview is the shipped system.
use termrock::style::{DesignSystem, RolePalette};

/// The catalog's design system.
///
/// `theme` is unused: the catalog does not swap palettes.
#[must_use]
pub fn lookbook_system(_theme: RolePalette) -> DesignSystem {
    DesignSystem::junie()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_paints_the_preset_it_ships() {
        let system = lookbook_system(RolePalette::junie());
        let preset = DesignSystem::junie();
        assert_eq!(system, preset, "catalog paint is the shipped junie system");
        assert_eq!(
            system.style(termrock::style::Role::Canvas).bg,
            preset.style(termrock::style::Role::Canvas).bg
        );
    }
}
