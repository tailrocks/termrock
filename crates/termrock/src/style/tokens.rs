// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Design-system tokens beyond role colors: spacing, glyphs, recipes, packages.

use super::{ColorCapability, Density, Motion, Role, RolePalette};
use ratatui_core::style::{Modifier, Style};

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

/// Semantic panel chrome emphasis for recipes.
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
    pub border: ratatui_core::style::Style,
    /// Title text style.
    pub title: ratatui_core::style::Style,
    /// Horizontal content pad (cells).
    pub pad_x: u16,
    /// Vertical content pad (cells).
    pub pad_y: u16,
    /// Optional surface fill style.
    pub surface: ratatui_core::style::Style,
}

/// Elevation token → surface role mapping (not border weight).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Elevation {
    /// Terminal underlay / canvas.
    Canvas,
    /// Default component surface.
    #[default]
    Surface,
    /// Raised card / dialog body.
    Raised,
    /// Overlay host above backdrop.
    Overlay,
}

impl Elevation {
    /// Maps elevation onto a semantic role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Canvas => Role::Canvas,
            Self::Surface => Role::Surface,
            Self::Raised | Self::Overlay => Role::Elevated,
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Surface => "surface",
            Self::Raised => "raised",
            Self::Overlay => "overlay",
        }
    }
}

/// Width breakpoints for density/contraction hosts (cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BreakpointScale {
    /// Essential-only / line mode (≤ this width).
    pub tiny: u16,
    /// Single-pane / drawer pressure.
    pub narrow: u16,
    /// Comfortable multi-pane.
    pub comfortable: u16,
    /// Wide workbench.
    pub wide: u16,
}

impl Default for BreakpointScale {
    fn default() -> Self {
        Self {
            tiny: 20,
            narrow: 40,
            comfortable: 80,
            wide: 120,
        }
    }
}

/// Button visual variant for recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonRecipeVariant {
    /// Brand primary.
    Primary,
    /// Default secondary.
    #[default]
    Secondary,
    /// Destructive / danger.
    Destructive,
    /// Quiet text-like.
    Quiet,
    /// Outline border only.
    Outline,
    /// Link style.
    Link,
}

/// Interaction state for button/input recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ControlState {
    /// Idle.
    #[default]
    Default,
    /// Pointer hover.
    Hovered,
    /// Focus-visible.
    Focused,
    /// Pressed / active.
    Pressed,
    /// Disabled.
    Disabled,
    /// Loading / busy.
    Loading,
}

/// Resolved button paint plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonRecipe {
    /// Label style.
    pub label: Style,
    /// Fill / surface style (may be empty).
    pub fill: Style,
    /// Border style when outlined.
    pub border: Style,
    /// Whether to paint a box border.
    pub bordered: bool,
    /// Leading/trailing pad cells.
    pub pad_x: u16,
}

/// Resolved text-input paint plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputRecipe {
    /// Value text style.
    pub value: Style,
    /// Placeholder style.
    pub placeholder: Style,
    /// Border / underline style.
    pub border: Style,
    /// Fill style.
    pub fill: Style,
    /// Cursor style.
    pub cursor: Style,
    /// Horizontal pad.
    pub pad_x: u16,
}

/// User-owned theme package (source-install / product brand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePackage {
    /// Stable package id (`phosphor`, `acme-brand`, …).
    pub id: String,
    /// Human label.
    pub label: String,
    /// Full design system snapshot.
    pub system: DesignSystem,
}

impl ThemePackage {
    /// Creates a package from an id, label, and system.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, system: DesignSystem) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            system,
        }
    }

    /// Built-in packages shipped by TermRock.
    #[must_use]
    pub fn builtins() -> Vec<Self> {
        vec![
            Self::new("phosphor", "Phosphor Obsidian", DesignSystem::phosphor()),
            Self::new("slate", "Slate", DesignSystem::slate()),
            Self::new("paper", "Paper", DesignSystem::paper()),
            Self::new("ansi", "ANSI 16", DesignSystem::ansi()),
            Self::new("high-contrast", "High Contrast", DesignSystem::high_contrast()),
            Self::new("adaptive", "Terminal Adaptive", DesignSystem::adaptive()),
        ]
    }
}

/// Sole paint authority for a frame or app shell (pre-1.0 Break B).
///
/// One object owns palette, density, glyphs, spacing, selection, capability,
/// motion, and breakpoints. Widgets take `&DesignSystem` only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignSystem {
    /// Role → Style map.
    pub palette: RolePalette,
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
    /// Color depth used for quantize-at-edge.
    pub capability: ColorCapability,
    /// Width breakpoints for contraction hosts.
    pub breakpoints: BreakpointScale,
}

impl Default for DesignSystem {
    fn default() -> Self {
        Self::phosphor()
    }
}

impl DesignSystem {
    /// Default phosphor Obsidian system (quiet gutter selection).
    #[must_use]
    pub fn phosphor() -> Self {
        Self::from_palette(RolePalette::tailrocks_phosphor()).selection(SelectionChrome::Gutter)
    }

    /// Alias for [`Self::phosphor`] (marketing name).
    #[must_use]
    pub fn obsidian() -> Self {
        Self::phosphor()
    }

    /// Cool-gray slate system.
    #[must_use]
    pub fn slate() -> Self {
        Self::from_palette(RolePalette::slate()).selection(SelectionChrome::Gutter)
    }

    /// Light paper system.
    #[must_use]
    pub fn paper() -> Self {
        Self::from_palette(RolePalette::paper()).selection(SelectionChrome::Fill)
    }

    /// ANSI 16-color native system (no truecolor dependency).
    #[must_use]
    pub fn ansi() -> Self {
        Self::from_palette(RolePalette::ansi())
            .capability(ColorCapability::Ansi16)
            .selection(SelectionChrome::Gutter)
    }

    /// High-contrast accessibility system.
    #[must_use]
    pub fn high_contrast() -> Self {
        Self::from_palette(RolePalette::high_contrast())
            .selection(SelectionChrome::Fill)
            .glyphs(GlyphSet::Unicode)
    }

    /// Adaptive: phosphor base quantized to env capability; mono → ASCII glyphs.
    #[must_use]
    pub fn adaptive() -> Self {
        let cap = ColorCapability::detect_from_env();
        let mut system = Self::phosphor().quantize(cap);
        if matches!(cap, ColorCapability::Monochrome) {
            system = system.glyphs(GlyphSet::Ascii).no_color();
        }
        system
    }

    /// Builds from palette + density-derived spacing.
    #[must_use]
    pub fn new(palette: RolePalette, density: Density) -> Self {
        Self {
            palette,
            density,
            motion: Motion::default(),
            glyphs: GlyphSet::default(),
            spacing: SpacingScale::from_density(density),
            selection: SelectionChrome::default(),
            capability: ColorCapability::default(),
            breakpoints: BreakpointScale::default(),
        }
    }

    /// Builds from a palette with default density.
    #[must_use]
    pub fn from_palette(palette: RolePalette) -> Self {
        Self::new(palette, Density::default())
    }

    /// Overrides density and recomputes spacing from density.
    #[must_use]
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self.spacing = SpacingScale::from_density(density);
        self
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

    /// ASCII glyph set only.
    #[must_use]
    pub const fn ascii(self) -> Self {
        self.glyphs(GlyphSet::Ascii)
    }

    /// Overrides selection chrome recipe.
    #[must_use]
    pub const fn selection(mut self, selection: SelectionChrome) -> Self {
        self.selection = selection;
        self
    }

    /// Overrides color capability (call before quantize).
    #[must_use]
    pub const fn capability(mut self, capability: ColorCapability) -> Self {
        self.capability = capability;
        self
    }

    /// Breakpoint scale.
    #[must_use]
    pub const fn breakpoints(mut self, breakpoints: BreakpointScale) -> Self {
        self.breakpoints = breakpoints;
        self
    }

    /// Force monochrome capability + quantize (NO_COLOR path).
    #[must_use]
    pub fn no_color(self) -> Self {
        self.quantize(ColorCapability::Monochrome)
            .glyphs(GlyphSet::Ascii)
    }

    /// Override one role style (partial theme package).
    #[must_use]
    pub fn with_role(mut self, role: Role, style: Style) -> Self {
        self.palette = self.palette.with_role(role, style);
        self
    }

    /// Merge non-empty styles from `other` onto this palette (inheritance).
    ///
    /// A style is applied when it has any fg, bg, or modifier — empty styles
    /// leave the base role untouched.
    #[must_use]
    pub fn merge(mut self, other: &RolePalette) -> Self {
        self.palette = self.palette.merge(other);
        self
    }

    /// Role style lookup.
    #[must_use]
    pub fn style(&self, role: Role) -> Style {
        self.palette.style(role)
    }

    /// Elevation → style.
    #[must_use]
    pub fn elevation(&self, elevation: Elevation) -> Style {
        self.style(elevation.role())
    }

    /// Palette borrow.
    #[must_use]
    pub const fn palette(&self) -> &RolePalette {
        &self.palette
    }

    /// Quantizes palette colors to this system's capability (or an override).
    #[must_use]
    pub fn quantize(self, capability: ColorCapability) -> Self {
        let mut out = self;
        out.capability = capability;
        out.palette = super::quantize_palette(&out.palette, capability);
        out
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
            border: self.style(border_role),
            title: self.style(title_role),
            pad_x: self.spacing.pad_x,
            pad_y: self.spacing.pad_y,
            surface: self.style(Role::Surface),
        }
    }

    /// Button part×state recipe (no hard-coded RGB).
    #[must_use]
    pub fn button_recipe(
        &self,
        variant: ButtonRecipeVariant,
        state: ControlState,
    ) -> ButtonRecipe {
        let disabled = matches!(state, ControlState::Disabled | ControlState::Loading);
        let base_role = match variant {
            ButtonRecipeVariant::Primary => Role::ActionFocused,
            ButtonRecipeVariant::Destructive => Role::Danger,
            ButtonRecipeVariant::Link => Role::Link,
            ButtonRecipeVariant::Secondary
            | ButtonRecipeVariant::Quiet
            | ButtonRecipeVariant::Outline => Role::Text,
        };
        let mut label = self.style(base_role);
        let mut fill = Style::new();
        let mut border = self.style(Role::Border);
        let mut bordered = matches!(
            variant,
            ButtonRecipeVariant::Outline | ButtonRecipeVariant::Primary | ButtonRecipeVariant::Destructive
        );

        if matches!(variant, ButtonRecipeVariant::Primary) {
            fill = self.style(Role::ActionFocused);
            label = self.style(Role::ActionFocused);
            border = self.style(Role::BorderFocused);
        }
        if matches!(variant, ButtonRecipeVariant::Destructive) {
            fill = self.style(Role::Danger);
            border = self.style(Role::Danger);
        }
        if matches!(variant, ButtonRecipeVariant::Quiet | ButtonRecipeVariant::Link) {
            bordered = false;
        }
        match state {
            ControlState::Focused => {
                border = self.style(Role::BorderFocused);
                if !matches!(variant, ButtonRecipeVariant::Quiet | ButtonRecipeVariant::Link) {
                    bordered = true;
                }
                label = label.add_modifier(Modifier::BOLD);
            }
            ControlState::Hovered => {
                label = self.style(Role::LinkHover);
            }
            ControlState::Pressed => {
                label = label.add_modifier(Modifier::BOLD);
            }
            ControlState::Disabled | ControlState::Loading => {
                label = self.style(Role::ActionDisabled);
                fill = Style::new();
                border = self.style(Role::Border);
            }
            ControlState::Default => {}
        }
        if disabled {
            label = self.style(Role::ActionDisabled);
        }
        ButtonRecipe {
            label,
            fill,
            border,
            bordered,
            pad_x: self.spacing.pad_x.max(1),
        }
    }

    /// Text input part×state recipe.
    #[must_use]
    pub fn input_recipe(&self, state: ControlState, invalid: bool) -> InputRecipe {
        let mut border = self.style(Role::Border);
        let mut value = self.style(Role::Input);
        let placeholder = self.style(Role::TextMuted);
        let fill = self.style(Role::Input);
        let cursor = self.style(Role::Focus);
        if invalid {
            border = self.style(Role::InputInvalid);
            value = self.style(Role::InputInvalid);
        }
        match state {
            ControlState::Focused => border = self.style(Role::BorderFocused),
            ControlState::Disabled => {
                value = self.style(Role::TextDisabled);
                border = self.style(Role::Border);
            }
            ControlState::Hovered => border = self.style(Role::Focus),
            _ => {}
        }
        InputRecipe {
            value,
            placeholder,
            border,
            fill,
            cursor,
            pad_x: self.spacing.pad_x,
        }
    }

    /// Resolves styles for a list row chrome recipe (one vertical slice).
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
            self.style(Role::TextDisabled)
        } else if state.selected && matches!(self.selection, SelectionChrome::Fill) {
            self.style(Role::Selection)
        } else if state.selected {
            self.style(Role::TextStrong)
        } else if state.loading {
            self.style(Role::TextMuted)
        } else {
            self.style(Role::Text)
        };
        let secondary = self.style(if disabled {
            Role::TextDisabled
        } else {
            Role::TextMuted
        });
        let shortcut = secondary;
        let gutter = if state.selected {
            Some((self.glyphs.selection_gutter(), self.style(Role::Accent)))
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
            focus: self.style(Role::Focus),
            hover: self.style(Role::LinkHover),
            tint: self.style(Role::Focus),
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
        let fill = DesignSystem::new(RolePalette::default(), Density::Compact)
            .selection(SelectionChrome::Fill)
            .list_row_recipe(true, true, true);
        let gutter = DesignSystem::new(RolePalette::default(), Density::Compact)
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
        let full = DesignSystem::default();
        let reduced = DesignSystem::default().motion(Motion::Reduced);
        assert!(full.motion.animate_spinners());
        assert!(!Motion::Off.animate_spinners());
        assert_ne!(full.motion, reduced.motion);
    }

    #[test]
    fn presets_are_distinct() {
        let phosphor = DesignSystem::phosphor();
        let slate = DesignSystem::slate();
        let paper = DesignSystem::paper();
        let ansi = DesignSystem::ansi();
        let hc = DesignSystem::high_contrast();
        assert_ne!(phosphor.style(Role::Accent), slate.style(Role::Accent));
        assert_ne!(paper.style(Role::Text).fg, phosphor.style(Role::Text).fg);
        assert_eq!(ansi.capability, ColorCapability::Ansi16);
        assert!(hc.style(Role::TextStrong).add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn with_role_and_merge_partial_package() {
        let base = DesignSystem::phosphor();
        let patched = base
            .clone()
            .with_role(Role::Accent, Style::new().fg(ratatui_core::style::Color::Cyan));
        assert_ne!(base.style(Role::Accent), patched.style(Role::Accent));
        let package = RolePalette::from_fn(|_| Style::new()); // empty → no change
        let merged = base.merge(&package);
        assert_eq!(merged.style(Role::Text), DesignSystem::phosphor().style(Role::Text));
    }

    #[test]
    fn no_color_forces_mono_and_ascii() {
        let system = DesignSystem::phosphor().no_color();
        assert_eq!(system.capability, ColorCapability::Monochrome);
        assert_eq!(system.glyphs, GlyphSet::Ascii);
    }

    #[test]
    fn button_recipe_focus_uses_border_focused() {
        let system = DesignSystem::phosphor();
        let focused = system.button_recipe(ButtonRecipeVariant::Secondary, ControlState::Focused);
        let idle = system.button_recipe(ButtonRecipeVariant::Secondary, ControlState::Default);
        assert_ne!(focused.border, idle.border);
        assert!(focused.bordered);
    }

    #[test]
    fn input_recipe_invalid_uses_input_invalid() {
        let system = DesignSystem::slate();
        let bad = system.input_recipe(ControlState::Default, true);
        let ok = system.input_recipe(ControlState::Default, false);
        assert_ne!(bad.border, ok.border);
    }

    #[test]
    fn elevation_maps_roles() {
        let system = DesignSystem::slate();
        assert_eq!(
            system.elevation(Elevation::Raised),
            system.style(Role::Elevated)
        );
        assert_eq!(Elevation::Canvas.role(), Role::Canvas);
    }

    #[test]
    fn theme_package_builtins_cover_presets() {
        let packs = ThemePackage::builtins();
        assert!(packs.iter().any(|p| p.id == "phosphor"));
        assert!(packs.iter().any(|p| p.id == "paper"));
        assert!(packs.iter().any(|p| p.id == "adaptive"));
        assert_eq!(packs.len(), 6);
    }

    #[test]
    fn quantize_ansi_preserves_structure() {
        let system = DesignSystem::paper().quantize(ColorCapability::Ansi16);
        let _ = system.style(Role::Accent);
        assert_eq!(system.capability, ColorCapability::Ansi16);
    }
}
