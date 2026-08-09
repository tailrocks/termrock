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

    /// Multi-select checked marker (without trailing space).
    #[must_use]
    pub const fn check_on(self) -> &'static str {
        match self {
            Self::Unicode => "☑",
            Self::Ascii => "[x]",
        }
    }

    /// Multi-select unchecked marker (without trailing space).
    #[must_use]
    pub const fn check_off(self) -> &'static str {
        match self {
            Self::Unicode => "☐",
            Self::Ascii => "[ ]",
        }
    }

    /// Loading / busy glyph for composed leading slots.
    #[must_use]
    pub const fn loading(self) -> &'static str {
        match self {
            Self::Unicode => "…",
            Self::Ascii => "...",
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

    /// Quiet-canvas / bright-intent design system alias (same token bundle).
    #[must_use]
    pub fn design_system(self) -> DesignSystem {
        DesignSystem { tokens: self }
    }

    /// Panel chrome recipe for single-line borders and title hierarchy.
    #[must_use]
    pub fn panel_recipe(&self, emphasis: PanelChrome) -> PanelRecipe {
        let (border_role, title_role) = match emphasis {
            PanelChrome::Normal => (Role::Border, Role::TextStrong),
            PanelChrome::Focused => (Role::BorderFocused, Role::TextStrong),
            PanelChrome::Danger => (Role::Danger, Role::TextStrong),
        };
        PanelRecipe {
            border: self.theme.style(border_role),
            title: self.theme.style(title_role),
            pad_x: self.spacing.pad_x,
            pad_y: self.spacing.pad_y,
            surface: self.theme.style(Role::Surface),
        }
    }

    /// Resolves styles for a **list row** chrome recipe (one vertical slice).
    ///
    /// Prefer [`Self::resolve_list_row`] with a full [`ListRowVisualState`] when
    /// hover/loading/checked affect paint.
    #[must_use]
    pub fn list_row_recipe(&self, selected: bool, focused: bool, enabled: bool) -> ListRowRecipe {
        self.resolve_list_row(ListRowVisualState {
            selected,
            focused,
            hovered: false,
            enabled,
            loading: false,
            checked: false,
        })
    }

    /// Full part×state list row recipe (quiet canvas, bright intent).
    #[must_use]
    pub fn resolve_list_row(&self, state: ListRowVisualState) -> ListRowRecipe {
        let disabled = !state.enabled;
        let label = if disabled {
            self.theme.style(Role::TextDisabled)
        } else if state.selected && matches!(self.selection, SelectionChrome::Fill) {
            self.theme.style(Role::Selection)
        } else if state.selected {
            self.theme.style(Role::TextStrong)
        } else if state.loading {
            self.theme.style(Role::TextMuted)
        } else {
            self.theme.style(Role::Text)
        };
        let secondary = self.theme.style(if disabled {
            Role::TextDisabled
        } else {
            Role::TextMuted
        });
        let shortcut = secondary;
        let gutter = if state.selected {
            // Accent gutter for selection; focus-visible is underline on primary.
            Some((
                self.glyphs.selection_gutter(),
                self.theme.style(Role::Accent),
            ))
        } else {
            None
        };
        let use_fill = state.selected && matches!(self.selection, SelectionChrome::Fill);
        let use_tint = state.selected && matches!(self.selection, SelectionChrome::Tint);
        let hover_fill = state.hovered && !state.selected && !disabled;
        ListRowRecipe {
            label,
            secondary,
            shortcut,
            trailing: secondary,
            gutter,
            pad_x: self.spacing.pad_x,
            use_fill,
            use_tint,
            hover_fill,
            show_focus_underline: state.focused && state.selected && !disabled,
            focus: self.theme.style(Role::Focus),
            hover: self.theme.style(Role::LinkHover),
            tint: self.theme.style(Role::Focus),
            check_on: self.glyphs.check_on(),
            check_off: self.glyphs.check_off(),
            loading_glyph: self.glyphs.loading(),
            show_gutter_slot: matches!(
                self.selection,
                SelectionChrome::Gutter | SelectionChrome::Tint | SelectionChrome::Fill
            ),
            checked: state.checked,
            loading: state.loading,
        }
    }
}

/// Runtime visual facts for one list row (widget state + row projection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ListRowVisualState {
    /// Cursor / keyboard selection.
    pub selected: bool,
    /// List owns focus and this row is the cursor.
    pub focused: bool,
    /// Pointer hover (enabled item only).
    pub hovered: bool,
    /// Row accepts interaction.
    pub enabled: bool,
    /// Row is loading (leading spinner/ellipsis).
    pub loading: bool,
    /// Multi-select membership.
    pub checked: bool,
}

/// Semantic panel chrome emphasis for recipes (mirrors widget emphasis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelChrome {
    /// Inactive / background panel.
    #[default]
    Normal,
    /// Interaction owner.
    Focused,
    /// Destructive / risk surface.
    Danger,
}

/// Resolved paint plan for a panel chrome surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelRecipe {
    /// Single-line border style (weight never encodes focus).
    pub border: Style,
    /// Title text style.
    pub title: Style,
    /// Horizontal content pad (cells).
    pub pad_x: u16,
    /// Vertical content pad (cells).
    pub pad_y: u16,
    /// Optional surface fill style.
    pub surface: Style,
}

/// Token-driven design system surface (quiet canvas, bright intent).
///
/// Wraps [`DesignTokens`] as the preferred public name from the terminal design
/// system spec. Prefer constructing via [`DesignTokens::design_system`] or
/// [`DesignSystem::phosphor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignSystem {
    /// Underlying tokens.
    pub tokens: DesignTokens,
}

impl Default for DesignSystem {
    fn default() -> Self {
        Self::phosphor()
    }
}

impl DesignSystem {
    /// Default phosphor Obsidian tokens.
    #[must_use]
    pub fn phosphor() -> Self {
        Self {
            tokens: DesignTokens::default().selection(SelectionChrome::Gutter),
        }
    }

    /// Builds from an existing theme + density.
    #[must_use]
    pub fn new(theme: Theme, density: Density) -> Self {
        Self {
            tokens: DesignTokens::new(theme, density).selection(SelectionChrome::Gutter),
        }
    }

    /// List row recipe (quiet selection via gutter by default).
    #[must_use]
    pub fn list_row_recipe(&self, selected: bool, focused: bool, enabled: bool) -> ListRowRecipe {
        self.tokens.list_row_recipe(selected, focused, enabled)
    }

    /// Full list row recipe from visual state.
    #[must_use]
    pub fn resolve_list_row(&self, state: ListRowVisualState) -> ListRowRecipe {
        self.tokens.resolve_list_row(state)
    }

    /// Panel chrome recipe.
    #[must_use]
    pub fn panel_recipe(&self, emphasis: PanelChrome) -> PanelRecipe {
        self.tokens.panel_recipe(emphasis)
    }

    /// Theme borrow.
    #[must_use]
    pub const fn theme(&self) -> &Theme {
        &self.tokens.theme
    }
}

/// Resolved paint recipe for one list/menu row (part×state plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListRowRecipe {
    /// Primary label style.
    pub label: Style,
    /// Secondary metadata style.
    pub secondary: Style,
    /// Shortcut hint style.
    pub shortcut: Style,
    /// Trailing metadata style (legacy alias of secondary tone).
    pub trailing: Style,
    /// Optional leading gutter glyph + style.
    pub gutter: Option<(&'static str, Style)>,
    /// Horizontal padding cells.
    pub pad_x: u16,
    /// Whether the row background uses selection fill.
    pub use_fill: bool,
    /// Whether selection uses tint (Focus role) without full Selection fill.
    pub use_tint: bool,
    /// Whether hover should tint the row background.
    pub hover_fill: bool,
    /// Whether to paint a focus underline cue on the primary label.
    pub show_focus_underline: bool,
    /// Focus accent style (underline / border role).
    pub focus: Style,
    /// Hover style when not selected.
    pub hover: Style,
    /// Tint style for [`SelectionChrome::Tint`].
    pub tint: Style,
    /// Multi-select checked glyph.
    pub check_on: &'static str,
    /// Multi-select unchecked glyph.
    pub check_off: &'static str,
    /// Loading leading glyph.
    pub loading_glyph: &'static str,
    /// Reserve leading gutter columns even when unselected (stable alignment).
    pub show_gutter_slot: bool,
    /// Multi-select membership for check paint.
    pub checked: bool,
    /// Loading flag for leading glyph override.
    pub loading: bool,
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
