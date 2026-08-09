// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Design-system tokens beyond role colors: spacing, glyphs, recipes.

use super::{Density, Motion, Role, Theme};
use ratatui_core::style::Style;

/// Glyph policy for borders, disclosure, and status markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum GlyphSet {
    /// Unicode box-drawing and status glyphs (default).
    #[default]
    Unicode,
    /// ASCII-safe substitutes.
    Ascii,
}

impl GlyphSet {
    /// Expansion / disclosure open marker.
    #[must_use]
    pub const fn disclosure_open(self) -> &'static str {
        match self {
            Self::Unicode => "▾",
            Self::Ascii => "v",
        }
    }

    /// Expansion / disclosure closed marker.
    #[must_use]
    pub const fn disclosure_closed(self) -> &'static str {
        match self {
            Self::Unicode => "▸",
            Self::Ascii => ">",
        }
    }

    /// Selected-row gutter marker (non-color cue).
    #[must_use]
    pub const fn selection_gutter(self) -> &'static str {
        match self {
            Self::Unicode => "▌",
            Self::Ascii => ">",
        }
    }

    /// Horizontal rule unit.
    #[must_use]
    pub const fn rule(self) -> &'static str {
        match self {
            Self::Unicode => "─",
            Self::Ascii => "-",
        }
    }
}

/// How list/menu selection is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectionChrome {
    /// Full-row fill using `Role::Selection`.
    #[default]
    Fill,
    /// Leading gutter glyph only (quieter).
    Gutter,
    /// Tint via `Role::Focus` without full fill.
    Tint,
}

/// Cell-scale spacing resolved from density (and optional overrides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpacingScale {
    /// Horizontal padding inside chrome.
    pub pad_x: u16,
    /// Vertical padding inside chrome.
    pub pad_y: u16,
    /// Gap between sibling regions.
    pub gap: u16,
    /// Minimum interactive row height in cells.
    pub min_row_height: u16,
}

impl SpacingScale {
    /// Resolves spacing from a density preset.
    #[must_use]
    pub const fn from_density(density: Density) -> Self {
        Self {
            pad_x: density.padding_x(),
            pad_y: density.padding_y(),
            gap: density.gap(),
            min_row_height: match density {
                Density::Comfortable => 1,
                Density::Compact | Density::Dashboard => 1,
            },
        }
    }
}

/// Complete design-system token bundle for one frame or app shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignTokens {
    /// Color/style theme.
    pub theme: Theme,
    /// Layout density.
    pub density: Density,
    /// Motion preference.
    pub motion: Motion,
    /// Glyph policy.
    pub glyphs: GlyphSet,
    /// Resolved spacing.
    pub spacing: SpacingScale,
    /// Default list/menu selection chrome.
    pub selection: SelectionChrome,
}

impl Default for DesignTokens {
    fn default() -> Self {
        Self::new(Theme::default(), Density::default())
    }
}

impl DesignTokens {
    /// Builds tokens with density-derived spacing.
    #[must_use]
    pub fn new(theme: Theme, density: Density) -> Self {
        Self {
            theme,
            density,
            motion: Motion::default(),
            glyphs: GlyphSet::default(),
            spacing: SpacingScale::from_density(density),
            selection: SelectionChrome::default(),
        }
    }

    /// Overrides motion.
    #[must_use]
    pub const fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// Overrides glyph set.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: GlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Overrides selection chrome recipe.
    #[must_use]
    pub const fn selection(mut self, selection: SelectionChrome) -> Self {
        self.selection = selection;
        self
    }

    /// Resolves styles for a **list row** chrome recipe (one vertical slice).
    #[must_use]
    pub fn list_row_recipe(&self, selected: bool, focused: bool, enabled: bool) -> ListRowRecipe {
        let label = if !enabled {
            self.theme.style(Role::TextDisabled)
        } else if selected && matches!(self.selection, SelectionChrome::Fill) {
            self.theme.style(Role::Selection)
        } else if selected {
            self.theme.style(Role::TextStrong)
        } else {
            self.theme.style(Role::Text)
        };
        let gutter = if selected {
            Some((
                self.glyphs.selection_gutter(),
                self.theme.style(Role::Accent),
            ))
        } else {
            None
        };
        let trailing = self.theme.style(if enabled {
            Role::TextMuted
        } else {
            Role::TextDisabled
        });
        ListRowRecipe {
            label,
            trailing,
            gutter,
            pad_x: self.spacing.pad_x,
            use_fill: selected && matches!(self.selection, SelectionChrome::Fill),
            show_focus_underline: focused && selected,
            focus: self.theme.style(Role::Focus),
        }
    }
}

/// Resolved paint recipe for one list/menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListRowRecipe {
    /// Primary label style.
    pub label: Style,
    /// Trailing metadata style.
    pub trailing: Style,
    /// Optional leading gutter glyph + style.
    pub gutter: Option<(&'static str, Style)>,
    /// Horizontal padding cells.
    pub pad_x: u16,
    /// Whether the row background uses selection fill.
    pub use_fill: bool,
    /// Whether to paint a focus underline cue.
    pub show_focus_underline: bool,
    /// Focus accent style.
    pub focus: Style,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_differs_across_density() {
        let comfortable = SpacingScale::from_density(Density::Comfortable);
        let dashboard = SpacingScale::from_density(Density::Dashboard);
        assert!(comfortable.pad_x > dashboard.pad_x);
        assert!(comfortable.gap >= dashboard.gap);
    }

    #[test]
    fn list_row_recipe_changes_with_selection_chrome() {
        let fill = DesignTokens::new(Theme::default(), Density::Compact)
            .selection(SelectionChrome::Fill)
            .list_row_recipe(true, true, true);
        let gutter = DesignTokens::new(Theme::default(), Density::Compact)
            .selection(SelectionChrome::Gutter)
            .list_row_recipe(true, true, true);
        assert!(fill.use_fill);
        assert!(!gutter.use_fill);
        assert!(gutter.gutter.is_some());
        assert_ne!(fill.label, gutter.label);
    }

    #[test]
    fn ascii_glyphs_differ_from_unicode() {
        assert_ne!(
            GlyphSet::Unicode.disclosure_closed(),
            GlyphSet::Ascii.disclosure_closed()
        );
    }

    #[test]
    fn reduced_motion_is_distinct() {
        let full = DesignTokens::default();
        let reduced = DesignTokens::default().motion(Motion::Reduced);
        assert!(full.motion.animate_spinners());
        assert!(!Motion::Off.animate_spinners());
        assert_ne!(full.motion, reduced.motion);
    }
}
