// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Design-system tokens beyond role colors: spacing, glyphs, recipes, packages.
use super::{
    BadgeKind, ButtonKind, ColorCapability, Glyph, JunieTheme, MotionPolicy, Role, RolePalette,
    VisualState,
};
use crate::runtime::FrameTick;
use ratatui_core::style::{Color, Modifier, Style};

/// Glyph vocabulary marker.
///
/// junie has exactly one vocabulary, so there is no profile to choose. This
/// type is the accessor surface (`selection_gutter()`, `rule()`, …) widgets
/// already call. State must survive monochrome through glyphs and modifiers,
/// which this one vocabulary guarantees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GlyphSet;

impl GlyphSet {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        "junie"
    }

    /// Resolve a semantic glyph in the one vocabulary.
    #[must_use]
    pub const fn resolve(self, glyph: super::glyph::Glyph) -> super::glyph::GlyphResolved {
        glyph.resolve()
    }

    /// Expansion / disclosure open marker.
    #[must_use]
    pub const fn disclosure_open(self) -> &'static str {
        super::glyph::Glyph::DisclosureOpen.resolve().text
    }

    /// Expansion / disclosure closed marker.
    #[must_use]
    pub const fn disclosure_closed(self) -> &'static str {
        super::glyph::Glyph::DisclosureClosed.resolve().text
    }

    /// Selected-row gutter marker (non-color cue).
    #[must_use]
    pub const fn selection_gutter(self) -> &'static str {
        super::glyph::Glyph::SelectionGutter.resolve().text
    }

    /// Selected-row marker (`›` chosen item; same encoding as chevron-right).
    #[must_use]
    pub const fn selection_marker(self) -> &'static str {
        super::glyph::Glyph::SelectionMarker.resolve().text
    }

    /// Horizontal rule unit.
    #[must_use]
    pub const fn rule(self) -> &'static str {
        super::glyph::Glyph::RuleH.resolve().text
    }

    /// Strong horizontal rule (H1 underlines, focus zones).
    #[must_use]
    pub const fn rule_strong(self) -> &'static str {
        super::glyph::Glyph::RuleHStrong.resolve().text
    }

    /// Vertical rule unit.
    #[must_use]
    pub const fn rule_v(self) -> &'static str {
        super::glyph::Glyph::RuleV.resolve().text
    }

    /// Multi-select checked marker (without trailing space).
    #[must_use]
    pub const fn check_on(self) -> &'static str {
        super::glyph::Glyph::CheckOn.resolve().text
    }

    /// Multi-select unchecked marker (without trailing space).
    #[must_use]
    pub const fn check_off(self) -> &'static str {
        super::glyph::Glyph::CheckOff.resolve().text
    }

    /// Loading / busy glyph for composed leading slots.
    #[must_use]
    pub const fn loading(self) -> &'static str {
        super::glyph::Glyph::Loading.resolve().text
    }

    /// Overflow / more ellipsis.
    #[must_use]
    pub const fn ellipsis(self) -> &'static str {
        super::glyph::Glyph::Ellipsis.resolve().text
    }

    /// Bullet for lists.
    #[must_use]
    pub const fn bullet(self) -> &'static str {
        super::glyph::Glyph::Bullet.resolve().text
    }

    /// Inline separator between adjacent facts on one row.
    ///
    /// The middle dot is reserved for this job: it separates meta content
    /// inside a row, while [`Self::bullet`] introduces a row of its own.
    #[must_use]
    pub const fn meta_separator(self) -> &'static str {
        super::glyph::Glyph::MetaSeparator.resolve().text
    }

    /// [`Self::meta_separator`] with its surrounding breathing space.
    ///
    /// Hint rows, status slots, and keyboard help all join with this exact
    /// string, so the rhythm between facts is identical wherever they appear.
    #[must_use]
    pub const fn meta_join(self) -> &'static str {
        " · "
    }

    /// Disabled mark (label suffix).
    #[must_use]
    pub const fn disabled_mark(self) -> &'static str {
        // junie states "disabled" through the faint text tier; the glyph is
        // kept for callers that need a non-color mark.
        super::glyph::Glyph::Remove.resolve().text
    }
}

/// How a family of surfaces says "the keyboard is here".
///
/// Plans 005-008 gave each family its cue; this names them so a theme can
/// state the vocabulary instead of every widget improvising one (audit F2).
/// The rule the names encode: a container brightens its border, a row tints
/// and takes the gutter, a cell reverses, a chip marks its bracket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FocusEmphasis {
    /// `Role::BorderFocused` on the container's own edge (panels, inputs).
    #[default]
    BrightBorder,
    /// Full selection fill — opt-in only, never a resting default.
    SelectionFill,
    /// `Role::SelectionTint` behind the row plus its gutter glyph.
    FocusTint,
    /// Reversed cell — a cell cursor is a cell (tables, grids).
    Reversed,
    /// Weight on the key itself (keycaps, hint chords).
    BoldKey,
    /// The token's bracket carries focus; the mark keeps stating membership.
    PillGlyph,
}

impl FocusEmphasis {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BrightBorder => "bright-border",
            Self::SelectionFill => "selection-fill",
            Self::FocusTint => "focus-tint",
            Self::Reversed => "reversed",
            Self::BoldKey => "bold-key",
            Self::PillGlyph => "pill-glyph",
        }
    }

    /// The cue a surface family wears by default.
    #[must_use]
    pub const fn for_family(family: SurfaceFamily) -> Self {
        match family {
            SurfaceFamily::Container | SurfaceFamily::Field => Self::BrightBorder,
            SurfaceFamily::Row => Self::FocusTint,
            SurfaceFamily::Cell => Self::Reversed,
            SurfaceFamily::Token => Self::PillGlyph,
            SurfaceFamily::Chord => Self::BoldKey,
        }
    }
}

/// The families whose focus cue differs, for [`FocusEmphasis::for_family`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SurfaceFamily {
    /// Panels, dialogs, drawers.
    #[default]
    Container,
    /// Text inputs and other typed fields.
    Field,
    /// List, tree and table rows.
    Row,
    /// Individual table or grid cells.
    Cell,
    /// Tags, chips, token-field entries.
    Token,
    /// Keycaps and hint chords.
    Chord,
}

impl SurfaceFamily {
    /// Position in a per-family table.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Container => 0,
            Self::Field => 1,
            Self::Row => 2,
            Self::Cell => 3,
            Self::Token => 4,
            Self::Chord => 5,
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Field => "field",
            Self::Row => "row",
            Self::Cell => "cell",
            Self::Token => "token",
            Self::Chord => "chord",
        }
    }
}

/// Component recipe families governed by one semantic visual contract.
///
/// This is deliberately coarser than individual widgets. A new control joins
/// one family and inherits its surface, hierarchy, focus, accent, motion, and
/// non-color cue instead of inventing a private visual language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecipeFamily {
    /// Buttons and other commit controls.
    Action,
    /// Editable fields and pickers.
    Input,
    /// Lists, trees, menus, and row-based tables.
    Collection,
    /// Dialogs, popovers, drawers, and transient layers.
    Overlay,
    /// Notices, progress, validation, and live state.
    Status,
    /// Metrics, key-value facts, tables, and charts.
    Data,
    /// Shell regions, panes, panels, and structural dividers.
    Layout,
}

impl RecipeFamily {
    /// Every enforced family, in stable inspector order.
    pub const ALL: [Self; 7] = [
        Self::Action,
        Self::Input,
        Self::Collection,
        Self::Overlay,
        Self::Status,
        Self::Data,
        Self::Layout,
    ];

    /// Stable id for inspectors and design gates.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Input => "input",
            Self::Collection => "collection",
            Self::Overlay => "overlay",
            Self::Status => "status",
            Self::Data => "data",
            Self::Layout => "layout",
        }
    }
}

/// Structural cue that keeps a recipe legible without hue.
///
/// No `None` variant exists: joining a recipe family requires choosing a cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NonColorCue {
    /// Focus/commit state changes label weight or reverse.
    WeightedLabel,
    /// A focused field carries a prompt glyph in its reserved leading cell.
    PromptGlyph,
    /// Selection carries a stable leading gutter or marker glyph.
    SelectionGlyph,
    /// Overlay ownership is stated by a frame and title marker.
    FramedTitle,
    /// Status pairs a semantic glyph with a written label.
    GlyphAndLabel,
    /// Primary, secondary, and metadata text occupy distinct tiers.
    TieredText,
    /// Structure is expressed through boundaries and spacing.
    BorderedRegion,
}

impl NonColorCue {
    /// Stable id for design inspection.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WeightedLabel => "weighted-label",
            Self::PromptGlyph => "prompt-glyph",
            Self::SelectionGlyph => "selection-glyph",
            Self::FramedTitle => "framed-title",
            Self::GlyphAndLabel => "glyph-and-label",
            Self::TieredText => "tiered-text",
            Self::BorderedRegion => "bordered-region",
        }
    }
}

/// Where the brand accent may appear inside a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccentUsage {
    /// Reserved for the primary commit action and its current intent.
    PrimaryIntent,
    /// Reserved for the one focus indicator; never ambient decoration.
    FocusOnly,
    /// Reserved for a small semantic mark, not whole sentences or surfaces.
    SemanticMark,
    /// This family does not spend brand accent.
    None,
}

/// Motion owned by a semantic recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MotionSemantics {
    /// Short state transition; retained (capped) under reduced motion.
    StateTransition,
    /// Ambient/activity motion; static under reduced motion.
    Activity,
    /// No motion is part of this family's meaning.
    Static,
}

impl MotionSemantics {
    /// Whether this channel may animate under `policy`.
    #[must_use]
    pub const fn animates(self, policy: MotionPolicy) -> bool {
        match self {
            Self::StateTransition => policy.allows_transitions(),
            Self::Activity => policy.animate_spinners(),
            Self::Static => false,
        }
    }
}

/// Semantic contract shared by every component in one [`RecipeFamily`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyRecipe {
    /// Family represented by this recipe.
    pub family: RecipeFamily,
    /// Default surface role.
    pub surface: Role,
    /// Primary content role.
    pub primary: Role,
    /// Secondary/meta content role.
    pub secondary: Role,
    /// Resting boundary role.
    pub border: Role,
    /// Focus vocabulary when the family can own interaction.
    pub focus: Option<FocusEmphasis>,
    /// Required structure that survives monochrome projection.
    pub non_color_cue: NonColorCue,
    /// Restriction on brand-accent use.
    pub accent: AccentUsage,
    /// Motion class owned by the family.
    pub motion: MotionSemantics,
}

/// Single-line border corner family.
///
/// junie draws rounded corners `╭╮╰╯` everywhere; there is no second shape to
/// choose. The type survives as the explicit marker [`DesignSystem`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BorderShape {
    /// Rounded corners `╭╮╰╯` (the junie shape).
    #[default]
    Rounded,
}

/// junie's named spacing tokens, in cells.
///
/// Every value is a const of the reference layout scale; there is no density
/// to tune. `gutter`/`inline` are the one-cell seams, `gap`/`column_gap` the
/// two-cell section seams, `form_gap` the four-cell form section break, and
/// the insets place content inside cards, frames, and dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpacingScale {
    /// Leading focus/selection seam (the `▎` column and its air).
    pub gutter: u16,
    /// One-cell inline pad inside a chip or control.
    pub inline: u16,
    /// Two-cell seam between sibling sections.
    pub gap: u16,
    /// Two-cell seam between table/grid columns.
    pub column_gap: u16,
    /// Four-cell seam between form sections.
    pub form_gap: u16,
    /// Inset inside a card / borderless filled surface.
    pub card_inset: u16,
    /// Inset inside a bordered frame (border + 2).
    pub frame_inset: u16,
    /// Horizontal inset inside a dialog.
    pub dialog_inset: u16,
    /// Cells of indent per hierarchy depth for tree rows.
    pub tree_indent: u16,
    /// Preferred input field height in rows.
    pub field_height: u16,
    /// Preferred tab strip height in rows.
    pub tabs_height: u16,
    /// Smallest usable viewport width.
    pub min_width: u16,
    /// Smallest usable viewport height.
    pub min_height: u16,
}

impl SpacingScale {
    /// The junie scale.
    #[must_use]
    pub const fn junie() -> Self {
        Self {
            gutter: 1,
            inline: 1,
            gap: 2,
            column_gap: 2,
            form_gap: 4,
            card_inset: 2,
            frame_inset: 3,
            dialog_inset: 3,
            tree_indent: 2,
            field_height: 3,
            tabs_height: 2,
            min_width: 72,
            min_height: 20,
        }
    }

    /// Blank rows that separate two content sections.
    ///
    /// junie's section break is one blank row: the first thing surrendered
    /// under height pressure — see [`SpacerBand::resolve`].
    #[must_use]
    pub const fn band(self) -> SpacerBand {
        SpacerBand { rows: 1 }
    }
}

impl Default for SpacingScale {
    fn default() -> Self {
        Self::junie()
    }
}

/// Cells reserved between chrome edges and the content they contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ContentInset {
    /// Columns reserved on each horizontal edge.
    pub x: u16,
    /// Rows reserved on each vertical edge.
    pub y: u16,
}

impl ContentInset {
    /// Shrinks `area` by this inset on all four edges, never past empty.
    #[must_use]
    pub fn apply(self, area: ratatui_core::layout::Rect) -> ratatui_core::layout::Rect {
        let x = area.x.saturating_add(self.x);
        let y = area.y.saturating_add(self.y);
        let width = area.width.saturating_sub(self.x.saturating_mul(2));
        let height = area.height.saturating_sub(self.y.saturating_mul(2));
        ratatui_core::layout::Rect {
            x: x.min(area.x.saturating_add(area.width)),
            y: y.min(area.y.saturating_add(area.height)),
            width,
            height,
        }
    }
}

/// Blank rows painted between two content sections to give a surface rhythm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SpacerBand {
    /// Preferred band height in rows.
    pub rows: u16,
}

impl SpacerBand {
    /// Band height that actually fits: rhythm is dropped before content is.
    ///
    /// `available` is the rows the surface owns, `content` the rows its
    /// sections need. The band survives only when every content row still
    /// fits with it painted.
    #[must_use]
    pub const fn resolve(self, available: u16, content: u16) -> u16 {
        if available >= content.saturating_add(self.rows) {
            self.rows
        } else {
            0
        }
    }
}

/// Separator painted between a key and its value in key-value surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvSeparator {
    /// Two-cell gutter — alignment carries the pairing (default).
    #[default]
    Gutter,
    /// Explicit ` : ` rule for ragged, non-aligned pairs.
    Colon,
}

impl KvSeparator {
    /// Literal painted between the key and the value.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Gutter => "  ",
            Self::Colon => " : ",
        }
    }

    /// Display columns the separator occupies.
    #[must_use]
    pub const fn cols(self) -> u16 {
        match self {
            Self::Gutter => 2,
            Self::Colon => 3,
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
    /// Row states a failure (the label steps into the error colour).
    pub error: bool,
    /// Pointer is holding the row down (the explicit reversal).
    pub pressed: bool,
    /// Row is loading (leading spinner/ellipsis).
    pub loading: bool,
    /// Multi-select membership.
    pub checked: bool,
}

impl ListRowVisualState {
    /// Whether this row's hidden affordances are revealed.
    ///
    /// A row reveals its actions when the operator is on it — by cursor, by
    /// focus, or by pointer. Everything else keeps them out of the way.
    #[must_use]
    pub const fn revealed(self) -> bool {
        self.enabled && (self.selected || self.focused || self.hovered)
    }
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

impl PanelChrome {
    /// The chrome a container wears for a focus flag.
    ///
    /// Every surface that owns focus reaches for the same two-armed
    /// conditional; stating it once keeps the focus-visible hierarchy from
    /// drifting one container at a time.
    #[must_use]
    pub const fn for_focus(focused: bool) -> Self {
        if focused { Self::Focused } else { Self::Normal }
    }
}

/// Resolved paint plan for a panel chrome surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelRecipe {
    /// Single-line border style; focus adds weight as a non-color cue.
    pub border: ratatui_core::style::Style,
    /// Title text style.
    pub title: ratatui_core::style::Style,
    /// Horizontal content pad (cells).
    pub pad_x: u16,
    /// Vertical content pad (cells).
    pub pad_y: u16,
    /// Optional surface fill style.
    pub surface: ratatui_core::style::Style,
    /// Glyph the title carries when the chrome itself is a warning.
    ///
    /// Danger chrome must not rely on a red border alone: colorless and
    /// low-color terminals need the mark in the title.
    pub title_prefix: Option<&'static str>,
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
    ///
    /// junie owns exactly one elevated plane: cards, dialogs, and popovers all
    /// sit on `#18181b`. `Overlay` therefore shares the rung with [`Self::Raised`]
    /// — an overlay is told apart by its frame and its backdrop, not by a
    /// lighter fill.
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
    /// Leading busy prefix and its style while the control is loading.
    ///
    /// junie says activity with an accent spinner cell, not by re-tinting the
    /// label; `None` when the control is not busy.
    pub busy_glyph: Option<(&'static str, Style)>,
}

/// Resolved text-input paint plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputRecipe {
    /// Value text style.
    pub value: Style,
    /// Placeholder style.
    pub placeholder: Style,
    /// Border style.
    pub border: Style,
    /// Fill style.
    pub fill: Style,
    /// Cursor style.
    pub cursor: Style,
    /// Field-local focus cue: always the `▎` bar. Unfocused paints fg=bg so
    /// the column stays reserved without a visible bar.
    pub prompt: Option<(&'static str, Style)>,
    /// Horizontal pad.
    pub pad_x: u16,
}

/// User-owned theme package (source-install / product brand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePackage {
    /// Stable package id (`junie`, `acme-brand`, …).
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

    /// The single canonical package TermRock ships.
    #[must_use]
    pub fn builtins() -> Vec<Self> {
        vec![Self::new("junie", "Junie", DesignSystem::junie())]
    }
}

/// Sole paint authority for a frame or app shell (pre-1.0 Break B).
///
/// One object owns palette, glyphs, spacing, capability, motion, and
/// breakpoints. Widgets take `&DesignSystem` only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignSystem {
    /// Role → Style map.
    pub palette: RolePalette,
    /// Motion tier for animated chrome.
    pub motion: MotionPolicy,
    /// Glyph vocabulary.
    pub glyphs: GlyphSet,
    /// Resolved spacing.
    pub spacing: SpacingScale,
    /// Color depth used for quantize-at-edge.
    pub capability: ColorCapability,
    /// Width breakpoints for contraction hosts.
    pub breakpoints: BreakpointScale,
    /// Separator painted between a key and its value.
    pub kv_separator: KvSeparator,
    /// Focus cue per surface family, in [`SurfaceFamily`] order.
    focus: [FocusEmphasis; 6],
    /// Frame time for animated chrome, when a host supplies one.
    ///
    /// Motion policy already rides the design system into every widget, but the
    /// *clock* did not, so anything wanting time needed it threaded through its
    /// own signature. Carrying the tick here gives all ~143 widgets access with
    /// no signature churn.
    ///
    /// `None` means "no time was injected": every animated surface paints its
    /// settled frame. That keeps snapshot tests deterministic without a special
    /// test mode, and it is the honest default — a widget must never invent a
    /// clock during paint.
    tick: Option<FrameTick>,
}

impl Default for DesignSystem {
    fn default() -> Self {
        Self::junie()
    }
}

impl DesignSystem {
    /// The canonical junie system.
    #[must_use]
    pub fn junie() -> Self {
        Self::new(RolePalette::junie()).capability(ColorCapability::Truecolor)
    }

    /// The canonical system resolved for the operator's terminal capability.
    #[must_use]
    pub fn adaptive() -> Self {
        Self::junie().quantize(ColorCapability::detect_from_env())
    }

    /// The junie theme resolved for this system's colour capability.
    ///
    /// Widgets that need a resolver (`lift`, `backdrop`, `button`, …) call the
    /// reference implementation here instead of re-deriving hover or pressed
    /// planes from roles.
    #[must_use]
    pub fn junie_theme(&self) -> JunieTheme {
        JunieTheme::for_level(self.capability)
    }

    /// junie hover ladder: exactly one plane above the container ground.
    ///
    /// canvas → elevated, surface/elevated → popover, field → field hover,
    /// anything else → popover.
    #[must_use]
    pub fn lift(&self, bg: Color) -> Color {
        self.junie_theme().lift(bg)
    }

    /// The explicit reversal — a cell cursor, a pressed row, a pressed quiet
    /// control. [`Modifier::REVERSED`] is banned; see [`JunieTheme::reversed`].
    #[must_use]
    pub fn reversed(&self) -> Style {
        self.junie_theme().reversed()
    }

    /// Selected text or range: body text on the popover plane.
    #[must_use]
    pub fn selected_text(&self) -> Style {
        self.junie_theme().selection()
    }

    /// Row-like control paint (nav item, list item, table row, tree node).
    ///
    /// The one row resolver: widgets that paint a row reach it here instead of
    /// re-deriving tint, hover, weight, and reversal per widget.
    #[must_use]
    pub fn row(&self, state: VisualState, bg: Color) -> Style {
        self.junie_theme().row(state, bg)
    }

    /// Focus-gutter glyph style for the control that owns the keyboard.
    #[must_use]
    pub fn gutter(&self, state: VisualState, bg: Color, on_accent: bool) -> Style {
        self.junie_theme().gutter(state, bg, on_accent)
    }

    /// Unoccupied scrollbar track.
    #[must_use]
    pub fn scrollbar_track(&self) -> Style {
        self.junie_theme().scrollbar_track()
    }

    /// Scrollbar thumb; the interaction owner's thumb is the brightest.
    #[must_use]
    pub fn scrollbar_thumb(&self, focused: bool, hovered: bool) -> Style {
        self.junie_theme().scrollbar_thumb(focused, hovered)
    }

    /// Key chord in an interaction hint.
    #[must_use]
    pub fn key_hint_key(&self) -> Style {
        self.junie_theme().key_hint_key()
    }

    /// Action label paired with a hint key.
    #[must_use]
    pub fn key_hint_action(&self) -> Style {
        self.junie_theme().key_hint_action()
    }

    /// Badge paint. [`BadgeKind::Edit`] is the only badge in the system.
    #[must_use]
    pub fn badge(&self, kind: BadgeKind) -> Style {
        self.junie_theme().badge(kind)
    }

    /// One step down the junie text ladder.
    ///
    /// [`Role::Text`] → [`Role::TextSecondary`] → [`Role::TextMuted`] →
    /// [`Role::TextFaint`] → [`Role::TextGhost`]. Any role off the ladder —
    /// semantic colours, surfaces — returns itself: de-emphasis never invents
    /// a new tone, and it never reaches for a DIM modifier.
    #[must_use]
    pub const fn lower_text(role: Role) -> Role {
        match role {
            Role::Text => Role::TextSecondary,
            Role::TextSecondary => Role::TextMuted,
            Role::TextMuted => Role::TextFaint,
            Role::TextFaint | Role::TextGhost => Role::TextGhost,
            other => other,
        }
    }

    /// Builds from a palette with the junie spacing, glyphs, and motion.
    #[must_use]
    pub fn new(palette: RolePalette) -> Self {
        Self {
            palette,
            motion: MotionPolicy::default(),
            glyphs: GlyphSet::default(),
            spacing: SpacingScale::junie(),
            capability: ColorCapability::default(),
            breakpoints: BreakpointScale::default(),
            kv_separator: KvSeparator::default(),
            focus: [
                FocusEmphasis::for_family(SurfaceFamily::Container),
                FocusEmphasis::for_family(SurfaceFamily::Field),
                FocusEmphasis::for_family(SurfaceFamily::Row),
                FocusEmphasis::for_family(SurfaceFamily::Cell),
                FocusEmphasis::for_family(SurfaceFamily::Token),
                FocusEmphasis::for_family(SurfaceFamily::Chord),
            ],
            tick: None,
        }
    }

    /// The focus cue this system gives a surface family.
    #[must_use]
    pub const fn focus_emphasis(&self, family: SurfaceFamily) -> FocusEmphasis {
        self.focus[family.index()]
    }

    /// Resolves the enforced semantic contract for a component family.
    #[must_use]
    pub const fn family_recipe(&self, family: RecipeFamily) -> FamilyRecipe {
        match family {
            RecipeFamily::Action => FamilyRecipe {
                family,
                surface: Role::Surface,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: Some(FocusEmphasis::BrightBorder),
                non_color_cue: NonColorCue::WeightedLabel,
                accent: AccentUsage::PrimaryIntent,
                motion: MotionSemantics::StateTransition,
            },
            RecipeFamily::Input => FamilyRecipe {
                family,
                surface: Role::Sunken,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: Some(self.focus_emphasis(SurfaceFamily::Field)),
                non_color_cue: NonColorCue::PromptGlyph,
                accent: AccentUsage::FocusOnly,
                motion: MotionSemantics::StateTransition,
            },
            RecipeFamily::Collection => FamilyRecipe {
                family,
                surface: Role::Surface,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: Some(self.focus_emphasis(SurfaceFamily::Row)),
                non_color_cue: NonColorCue::SelectionGlyph,
                accent: AccentUsage::FocusOnly,
                motion: MotionSemantics::StateTransition,
            },
            RecipeFamily::Overlay => FamilyRecipe {
                family,
                surface: Role::Elevated,
                primary: Role::TextStrong,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: Some(self.focus_emphasis(SurfaceFamily::Container)),
                non_color_cue: NonColorCue::FramedTitle,
                accent: AccentUsage::FocusOnly,
                motion: MotionSemantics::StateTransition,
            },
            RecipeFamily::Status => FamilyRecipe {
                family,
                surface: Role::StatusBar,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: None,
                non_color_cue: NonColorCue::GlyphAndLabel,
                accent: AccentUsage::SemanticMark,
                motion: MotionSemantics::Activity,
            },
            RecipeFamily::Data => FamilyRecipe {
                family,
                surface: Role::Surface,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::ChartGrid,
                focus: Some(self.focus_emphasis(SurfaceFamily::Cell)),
                non_color_cue: NonColorCue::TieredText,
                accent: AccentUsage::SemanticMark,
                motion: MotionSemantics::Static,
            },
            RecipeFamily::Layout => FamilyRecipe {
                family,
                surface: Role::Canvas,
                primary: Role::Text,
                secondary: Role::TextMuted,
                border: Role::Border,
                focus: Some(self.focus_emphasis(SurfaceFamily::Container)),
                non_color_cue: NonColorCue::BorderedRegion,
                accent: AccentUsage::None,
                motion: MotionSemantics::StateTransition,
            },
        }
    }

    /// Overrides one family's focus cue (themes state the vocabulary).
    #[must_use]
    pub const fn with_focus_emphasis(
        mut self,
        family: SurfaceFamily,
        emphasis: FocusEmphasis,
    ) -> Self {
        self.focus[family.index()] = emphasis;
        self
    }

    /// Supplies this frame's time to every widget painted with this system.
    ///
    /// Call once per frame in the host's render function:
    ///
    /// ```rust,ignore
    /// let system = base_system.clone().at(tick);
    /// ```
    #[must_use]
    pub const fn at(mut self, tick: FrameTick) -> Self {
        self.tick = Some(tick);
        self
    }

    /// This frame's time, if the host supplied one.
    #[must_use]
    pub const fn tick(&self) -> Option<FrameTick> {
        self.tick
    }

    /// Milliseconds since the runner started, or `0` when no tick was supplied.
    ///
    /// The phase source for ambient loops, which must ride wall clock rather
    /// than frame count.
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.tick.map_or(0, FrameTick::elapsed_ms)
    }

    /// Overrides motion.
    #[must_use]
    pub const fn motion(mut self, motion: MotionPolicy) -> Self {
        self.motion = motion;
        self
    }

    /// Resolves the border symbols: junie's rounded corners everywhere.
    #[must_use]
    pub const fn border_set(&self) -> ratatui_core::symbols::border::Set<'static> {
        ratatui_core::symbols::border::ROUNDED
    }

    /// Cells that separate chrome from the content inside it.
    ///
    /// Bordered chrome always reserves at least one column so text never sits
    /// flush against a border glyph — at every terminal width, including the
    /// narrow ones where padding used to collapse. Vertical rhythm inside a
    /// border comes from the border itself, so bordered chrome insets no rows.
    #[must_use]
    pub const fn content_inset(&self, bordered: bool) -> ContentInset {
        if bordered {
            ContentInset { x: 1, y: 0 }
        } else {
            ContentInset {
                x: self.spacing.card_inset,
                y: 1,
            }
        }
    }

    /// Separator this system paints between a key and its value.
    #[must_use]
    pub const fn kv_separator(&self) -> KvSeparator {
        self.kv_separator
    }

    /// Overrides the key-value separator family.
    #[must_use]
    pub const fn with_kv_separator(mut self, separator: KvSeparator) -> Self {
        self.kv_separator = separator;
        self
    }

    /// Overrides color capability (call before quantize).
    #[must_use]
    pub const fn capability(mut self, capability: ColorCapability) -> Self {
        self.capability = capability;
        self
    }

    /// Whether this system must carry meaning without color.
    ///
    /// True on a monochrome projection: the paint has to say what it means
    /// through weight, reverse, and glyph.
    #[must_use]
    pub const fn mono(&self) -> bool {
        matches!(self.capability, ColorCapability::Monochrome)
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
    }

    /// Role style lookup.
    #[must_use]
    pub fn style(&self, role: Role) -> Style {
        self.palette.style(role)
    }

    /// Paints one contracted row of text.
    ///
    /// The sanctioned single-row painter for titles, labels and values: it
    /// never splits a grapheme cluster, never leaves a silent hard cut, and
    /// takes its ellipsis from the active glyph profile. Surfaces that reach
    /// for `Buffer::set_stringn` instead lose all three.
    pub fn paint_row(
        &self,
        buffer: &mut ratatui_core::buffer::Buffer,
        area: ratatui_core::layout::Rect,
        text: &str,
        style: Style,
    ) {
        crate::text::paint_text(buffer, area, text, style, self.glyphs.ellipsis());
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

    /// Resolves this system onto a colour capability (or an override).
    #[must_use]
    pub fn quantize(mut self, capability: ColorCapability) -> Self {
        self.capability = capability;
        self.palette = self.palette.quantized(capability);
        self
    }

    /// Panel chrome recipe for single-line borders and title hierarchy.
    ///
    /// `elevation` selects the fill rung, so an overlay panel recesses the
    /// content behind it instead of repainting the same ordinary surface.
    /// Focus snaps: junie has no cross-fade, the frame simply states owner.
    #[must_use]
    pub fn panel_recipe(&self, emphasis: PanelChrome, elevation: Elevation) -> PanelRecipe {
        let theme = self.junie_theme();
        let focused = matches!(emphasis, PanelChrome::Focused);
        // junie framed-panel law: the frame is `border(focused)` — the strong
        // hairline when the panel owns focus, the subtle one otherwise — and
        // the title is `title()` when focused, `secondary()` when not. Danger
        // keeps the body text and states the risk through the frame and the
        // `!` mark, the only red chrome junie allows a container.
        let border = match emphasis {
            PanelChrome::Normal => self.style(Role::Border),
            PanelChrome::Focused => self.style(Role::BorderFocused),
            PanelChrome::Danger => self.style(Role::Danger),
        };
        PanelRecipe {
            border,
            title: if focused {
                theme.title()
            } else {
                theme.secondary()
            },
            pad_x: self.spacing.card_inset,
            pad_y: 1,
            surface: self.style(elevation.role()),
            title_prefix: match emphasis {
                PanelChrome::Danger => Some(self.glyphs.resolve(Glyph::Error).text),
                PanelChrome::Focused | PanelChrome::Normal => None,
            },
        }
    }

    /// Button part×state recipe — literal port of the junie `button` resolver.
    ///
    /// The public variants collapse onto junie's kinds: `Primary`, `Secondary`
    /// (also `Outline` — junie has no border-only action), `Quiet` → `Subtle`,
    /// `Destructive` → `Danger`. `Link` is the one derived form: junie has no
    /// link button, so it wears the link law instead (secondary text plus the
    /// underline affordance, never a fill).
    ///
    /// junie buttons carry no box border: a button is the `▎` gutter, its
    /// label, and its fill. Focus is the gutter plus weight, pressed is the
    /// explicit reversal `fg(canvas).bg(text_primary)`, and busy keeps the
    /// idle fill while the label loses its weight and drops one text tier —
    /// [`ButtonRecipe::busy_glyph`] carries the accent spinner prefix.
    #[must_use]
    pub fn button_recipe(
        &self,
        variant: ButtonRecipeVariant,
        state: ControlState,
        container: Color,
    ) -> ButtonRecipe {
        let theme = self.junie_theme();
        // The container supplies the plane the button sits on: a dialog is not
        // the chrome surface, and only `Subtle` reads the ground directly.
        let ground = container;
        let kind = match variant {
            ButtonRecipeVariant::Primary => ButtonKind::Primary,
            ButtonRecipeVariant::Destructive => ButtonKind::Danger,
            ButtonRecipeVariant::Quiet => ButtonKind::Subtle,
            ButtonRecipeVariant::Secondary | ButtonRecipeVariant::Outline => ButtonKind::Secondary,
            ButtonRecipeVariant::Link => ButtonKind::Subtle,
        };
        let visual = VisualState {
            hovered: matches!(state, ControlState::Hovered),
            focused: matches!(state, ControlState::Focused),
            pressed: matches!(state, ControlState::Pressed),
            disabled: matches!(state, ControlState::Disabled),
            busy: matches!(state, ControlState::Loading),
            ..VisualState::default()
        };
        let (label, fill) = if matches!(variant, ButtonRecipeVariant::Link) {
            // Link law: the underline is the affordance, hover brightens the
            // text one tier, focus adds weight. No fill, no reversal.
            let label = match state {
                ControlState::Hovered | ControlState::Pressed => self.style(Role::LinkHover),
                ControlState::Focused => self.style(Role::Link).add_modifier(Modifier::BOLD),
                ControlState::Loading => Style::new().fg(theme.text_secondary),
                _ => self.style(Role::Link),
            };
            (label, Style::new())
        } else if matches!(state, ControlState::Loading) {
            // Busy: the accent spinner prefix says the control is working and
            // the label loses its weight. The fill stays the idle one —
            // activity is not a second surface — so the label keeps the
            // fill's own contrast pair and only steps down off the accent
            // fill, where `text_secondary` would read at 1.2:1.
            let idle = theme.button(kind, VisualState::default(), ground);
            let label = Style::new().fg(if kind == ButtonKind::Primary {
                theme.text_on_accent
            } else {
                theme.text_secondary
            });
            let fill = Style::new().bg(idle.bg.unwrap_or(ground));
            (label, fill)
        } else {
            let painted = theme.button(kind, visual, ground);
            let label = Style::new()
                .fg(painted.fg.unwrap_or(theme.text_primary))
                .add_modifier(painted.add_modifier);
            let fill = Style::new().bg(painted.bg.unwrap_or(ground));
            (label, fill)
        };
        ButtonRecipe {
            label,
            fill,
            border: Style::new(),
            bordered: false,
            pad_x: self.spacing.inline.max(1),
            busy_glyph: matches!(state, ControlState::Loading)
                .then(|| (self.glyphs.loading(), Style::new().fg(theme.accent))),
        }
    }

    /// Text input part×state recipe.
    ///
    /// Keyboard focus is the gutter. `editing` is the insert session: accent
    /// underline. A focused-but-not-editing field is the well plus `▎` only.
    #[must_use]
    pub fn input_recipe(&self, state: ControlState, invalid: bool, editing: bool) -> InputRecipe {
        let theme = self.junie_theme();
        let focused = matches!(state, ControlState::Focused);
        let disabled = matches!(state, ControlState::Disabled);
        let visual_editing = editing && focused && !disabled;
        let visual = VisualState {
            hovered: matches!(state, ControlState::Hovered) && !visual_editing,
            disabled,
            focused,
            editing: visual_editing,
            ..VisualState::default()
        };
        // junie field law: the well is `field`, hover lifts to `field_hover`
        // while the field is not being edited, and the value is always body
        // text. Disabled keeps the well and steps the text to the disabled
        // tier. An invalid value keeps its tone: the underline says "error",
        // repainting the whole value would say nothing.
        let field = theme.field_style(visual);
        let value = Style::new().fg(if matches!(state, ControlState::Disabled) {
            theme.disabled
        } else {
            field.fg.unwrap_or(theme.text_primary)
        });
        let fill = Style::new().bg(field.bg.unwrap_or(theme.field));
        let placeholder = Style::new().fg(theme.placeholder(visual).fg.unwrap_or(theme.text_muted));
        // A field has no frame; the border slot carries the underline
        // affordance. Editing underlines in accent and an invalid value moves
        // that underline to the error colour — the 3-colour underline law.
        // Resting is the subtle hairline. Nav-focus is the gutter, not an
        // underline. Focus snaps; there is no cross-fade.
        let border = if invalid {
            Style::new()
                .add_modifier(Modifier::UNDERLINED)
                .underline_color(theme.error)
        } else if visual_editing {
            Style::new()
                .add_modifier(Modifier::UNDERLINED)
                .underline_color(theme.accent)
        } else {
            theme.border(false)
        };
        // junie edits with the hardware cursor; the recipe paints the cell it
        // occupies as the explicit reversal so a cell cursor reads as a cell.
        let cursor = Style::new().fg(theme.canvas).bg(theme.text_primary);
        // Col 0 is always the focus bar; idle paints fg=bg.
        let prompt = Some((
            self.glyphs.selection_gutter(),
            self.gutter(visual, fill.bg.unwrap_or(theme.field), false),
        ));
        InputRecipe {
            value,
            placeholder,
            border,
            fill,
            cursor,
            prompt,
            pad_x: self.spacing.inline,
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
            ..ListRowVisualState::default()
        })
    }

    /// Full part×state list row recipe — literal port of the junie `row`
    /// resolver on the chrome-plane ground.
    ///
    /// Universal row: col 0 is always the focus bar `▎` via
    /// [`Self::gutter`]; col 1 is the membership marker (`›` / `✓` / space).
    /// Tint is `selected && focused` and hover replaces it with exactly one
    /// plane up. The label never takes the accent — the marker does.
    ///
    /// Modifier order is the reference's: disabled, tint, hover, error, busy,
    /// focus weight, pressed replacement. Busy therefore lands *before* the
    /// focus weight, so a busy row keeps its weight instead of losing it.
    #[must_use]
    pub fn resolve_list_row(&self, state: ListRowVisualState) -> ListRowRecipe {
        let theme = self.junie_theme();
        let ground = theme.surface;
        let disabled = !state.enabled;
        let hovered = state.hovered && !disabled;
        let focused = state.focused && !disabled;
        let visual = VisualState {
            focused,
            hovered,
            pressed: state.pressed,
            selected: state.selected,
            disabled,
            error: state.error,
            busy: state.loading,
            ..VisualState::default()
        };
        // The reference `row` resolver owns the whole ladder; the recipe only
        // re-states its output in the part vocabulary the widgets paint with.
        let painted = self.row(visual, ground);
        let painted_bg = painted.bg.unwrap_or(ground);
        let label = painted;
        let secondary = Style::new().fg(if disabled {
            theme.disabled
        } else {
            theme.text_muted
        });
        let shortcut = secondary;
        let gutter = (
            self.glyphs.selection_gutter(),
            self.gutter(visual, painted_bg, false),
        );
        let membership = (state.selected || state.checked) && !disabled;
        let marker_glyph = if state.checked {
            Glyph::Success.resolve().text
        } else if state.selected {
            self.glyphs.selection_marker()
        } else {
            " "
        };
        let marker_style = if membership {
            painted.fg(if focused || hovered {
                theme.accent
            } else {
                theme.text_secondary
            })
        } else {
            painted
        };
        ListRowRecipe {
            label,
            secondary,
            shortcut,
            gutter,
            marker: (marker_glyph, marker_style),
            pad_x: self.spacing.inline,
            use_tint: state.selected && focused && !hovered,
            hover_fill: hovered,
            focus: Style::new().fg(theme.focus),
            hover: Style::new().fg(theme.text_primary),
            hover_wash: Style::new().bg(theme.lift(ground)),
            tint: Style::new().bg(theme.accent_bg),
            check_on: self.glyphs.check_on(),
            check_off: self.glyphs.check_off(),
            loading_glyph: self.glyphs.loading(),
            show_gutter_slot: true,
            checked: state.checked,
            loading: state.loading,
            // Law P6: a row's actions appear when the row is the one you are
            // on. Idle rows keep a faint marker so the affordance is still
            // discoverable.
            show_actions: state.revealed(),
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
    /// Leading focus-bar `▎` + style. Always present; unfocused paints fg=bg.
    pub gutter: (&'static str, Style),
    /// Col-1 membership glyph (`›` chosen, `✓` checked, space otherwise).
    pub marker: (&'static str, Style),
    /// Horizontal padding cells.
    pub pad_x: u16,
    /// Whether selection uses tint (Focus role) without full Selection fill.
    pub use_tint: bool,
    /// Whether hover should tint the row background.
    pub hover_fill: bool,
    /// Focus accent style for non-border focus cues.
    pub focus: Style,
    /// Hover label style when not selected.
    ///
    /// Hover never borrows link styling: the row lifts with
    /// [`Self::hover_wash`] behind a strong label, so a hovered row is not
    /// mistaken for a clickable hyperlink.
    pub hover: Style,
    /// Background wash for hovered rows.
    pub hover_wash: Style,
    /// Tint style for a focused selected row.
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
    /// Whether row actions are revealed this frame (law P6).
    pub show_actions: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_is_the_junie_scale() {
        // The tokens are consts: exactly one scale, no density to tune.
        let spacing = SpacingScale::junie();
        assert_eq!((spacing.gutter, spacing.inline), (1, 1));
        assert_eq!((spacing.gap, spacing.column_gap), (2, 2));
        assert_eq!(spacing.form_gap, 4);
        assert_eq!(
            (
                spacing.card_inset,
                spacing.frame_inset,
                spacing.dialog_inset
            ),
            (2, 3, 3)
        );
        assert_eq!(spacing.tree_indent, 2);
        assert_eq!((spacing.field_height, spacing.tabs_height), (3, 2));
        assert_eq!((spacing.min_width, spacing.min_height), (72, 20));
        assert_eq!(spacing, SpacingScale::default());
    }

    #[test]
    fn focused_selected_recipe_pins_gutter_and_checked_uses_success() {
        let system = DesignSystem::junie();
        let focused = system.list_row_recipe(true, true, true);
        assert_eq!(focused.gutter.0, Glyph::SelectionGutter.resolve().text);
        assert_eq!(focused.gutter.0, "▎");
        assert_eq!(focused.marker.0, system.glyphs.selection_marker());
        assert_eq!(focused.marker.0, "›");
        let checked = system.resolve_list_row(ListRowVisualState {
            selected: false,
            focused: true,
            checked: true,
            enabled: true,
            ..Default::default()
        });
        assert_eq!(checked.marker.0, Glyph::Success.resolve().text);
        assert_eq!(checked.marker.0, "✓");
        assert_ne!(
            checked.marker.0,
            Glyph::CheckOn.resolve().text,
            "list membership is ✓, not the checkbox [✓]"
        );
    }

    #[test]
    fn focused_row_tints_and_parked_row_only_marks() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let owned = system.resolve_list_row(ListRowVisualState {
            selected: true,
            focused: true,
            enabled: true,
            ..Default::default()
        });
        let parked = system.resolve_list_row(ListRowVisualState {
            selected: true,
            focused: false,
            enabled: true,
            ..Default::default()
        });
        assert_eq!(owned.label.bg, Some(theme.accent_bg), "tint needs focus");
        assert!(owned.use_tint);
        assert_eq!(
            parked.label.bg,
            Some(theme.surface),
            "a parked selection carries no tint: the row ground only"
        );
        assert!(!parked.use_tint);
        let (bar, _) = parked.gutter;
        assert_eq!(bar, system.glyphs.selection_gutter());
        let (marker, marker_style) = parked.marker;
        assert_eq!(marker, system.glyphs.selection_marker());
        assert_eq!(marker_style.fg, Some(theme.text_secondary));
        assert!(!owned.label.add_modifier.contains(Modifier::REVERSED));
        assert_ne!(
            owned.label.fg,
            Some(theme.accent),
            "label never uses accent"
        );
    }

    #[test]
    fn hover_replaces_the_selection_tint_with_one_plane_up() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let hovered = system.resolve_list_row(ListRowVisualState {
            selected: true,
            focused: true,
            hovered: true,
            enabled: true,
            ..Default::default()
        });
        assert!(hovered.hover_fill);
        assert!(!hovered.use_tint, "hover wins over the tint");
        assert_eq!(hovered.label.bg, Some(theme.lift(theme.surface)));
        assert_eq!(
            hovered.hover_wash.bg,
            Some(theme.surface_overlay),
            "hover is exactly one plane above the chrome plane"
        );
    }

    #[test]
    fn selection_gutter_tone_tracks_collection_focus() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let focused = system.resolve_list_row(ListRowVisualState {
            selected: true,
            focused: true,
            enabled: true,
            ..Default::default()
        });
        let parked = system.resolve_list_row(ListRowVisualState {
            selected: true,
            focused: false,
            enabled: true,
            ..Default::default()
        });
        let idle = system.resolve_list_row(ListRowVisualState {
            selected: false,
            focused: false,
            enabled: true,
            ..Default::default()
        });
        let (bar, bar_style) = focused.gutter;
        assert_eq!(bar, system.glyphs.selection_gutter());
        assert_eq!(bar_style.fg, Some(theme.focus));
        let (parked_bar, parked_bar_style) = parked.gutter;
        assert_eq!(parked_bar, system.glyphs.selection_gutter());
        assert_eq!(
            parked_bar_style.fg, parked_bar_style.bg,
            "unfocused bar is invisible: fg=bg"
        );
        let (marker, marker_style) = focused.marker;
        assert_eq!(marker, system.glyphs.selection_marker());
        assert_eq!(marker_style.fg, Some(theme.accent));
        let (parked_marker, parked_marker_style) = parked.marker;
        assert_eq!(parked_marker, system.glyphs.selection_marker());
        assert_eq!(parked_marker_style.fg, Some(theme.text_secondary));
        let (idle_bar, idle_bar_style) = idle.gutter;
        assert_eq!(idle_bar, system.glyphs.selection_gutter());
        assert_eq!(idle_bar_style.fg, idle_bar_style.bg);
        assert_eq!(idle.marker.0, " ");
        let checked = system.resolve_list_row(ListRowVisualState {
            selected: false,
            focused: true,
            checked: true,
            enabled: true,
            ..Default::default()
        });
        assert_eq!(checked.marker.0, "✓");
        assert_eq!(checked.marker.1.fg, Some(theme.accent));
        assert_ne!(focused.label.fg, Some(theme.accent));
    }

    #[test]
    fn reduced_motion_is_distinct() {
        let full = DesignSystem::default();
        assert!(full.motion.animate_spinners());
        assert!(!MotionPolicy::Off.animate_spinners());
        assert_ne!(full.motion, MotionPolicy::Off);
    }

    #[test]
    fn presets_are_distinct() {
        // junie is the only palette TermRock ships: distinctness is now a
        // capability question, not a palette question.
        let truecolor = DesignSystem::junie();
        let ansi = DesignSystem::junie().quantize(ColorCapability::Ansi16);
        assert_ne!(truecolor.style(Role::Accent), ansi.style(Role::Accent));
        assert_eq!(ansi.capability, ColorCapability::Ansi16);
        assert!(
            truecolor
                .style(Role::TextStrong)
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn with_role_and_merge_partial_package() {
        let base_palette = RolePalette::junie();
        let patched_palette = base_palette.clone().with_role(
            Role::Accent,
            Style::new().fg(ratatui_core::style::Color::Cyan),
        );
        let base = DesignSystem::new(base_palette);
        let patched = DesignSystem::new(patched_palette);
        assert_ne!(base.style(Role::Accent), patched.style(Role::Accent));
        let package = RolePalette::from_fn(|_| Style::new()); // empty → no change
        let merged = DesignSystem::new(RolePalette::junie().merge(&package));
        assert_eq!(
            merged.style(Role::Text),
            DesignSystem::junie().style(Role::Text)
        );
    }

    #[test]
    fn tick_defaults_to_none_so_snapshots_stay_deterministic() {
        let system = DesignSystem::default();
        assert_eq!(
            system.tick(),
            None,
            "a design system must not invent a clock"
        );
        assert_eq!(system.elapsed_ms(), 0);

        let start = crate::runtime::Instant::now();
        let tick = FrameTick::manual(
            start + std::time::Duration::from_millis(750),
            std::time::Duration::from_millis(750),
            std::time::Duration::from_millis(16),
        );
        let timed = system.at(tick);
        assert_eq!(timed.tick(), Some(tick));
        assert_eq!(timed.elapsed_ms(), 750);
    }

    #[test]
    fn no_color_forces_mono() {
        let system = DesignSystem::junie().no_color();
        assert_eq!(system.capability, ColorCapability::Monochrome);
    }

    #[test]
    fn button_recipe_follows_the_junie_button_table() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        // Primary is the accent fill with on-accent text.
        let primary = system.button_recipe(
            ButtonRecipeVariant::Primary,
            ControlState::Default,
            theme.surface,
        );
        assert_eq!(primary.fill.bg, Some(theme.accent));
        assert_eq!(primary.label.fg, Some(theme.text_on_accent));
        assert!(primary.label.add_modifier.contains(Modifier::BOLD));
        assert_eq!(
            system
                .button_recipe(
                    ButtonRecipeVariant::Primary,
                    ControlState::Hovered,
                    theme.surface
                )
                .fill
                .bg,
            Some(theme.accent_hover)
        );
        assert_eq!(
            system
                .button_recipe(
                    ButtonRecipeVariant::Primary,
                    ControlState::Pressed,
                    theme.surface
                )
                .fill
                .bg,
            Some(theme.accent_pressed)
        );

        // Secondary rests on the overlay plane, hovers to popover, and presses
        // into the explicit reversal — never Modifier::REVERSED.
        let secondary = system.button_recipe(
            ButtonRecipeVariant::Secondary,
            ControlState::Default,
            theme.surface,
        );
        assert_eq!(secondary.fill.bg, Some(theme.surface_overlay));
        assert_eq!(secondary.label.fg, Some(theme.text_primary));
        assert_eq!(
            system
                .button_recipe(
                    ButtonRecipeVariant::Secondary,
                    ControlState::Hovered,
                    theme.surface
                )
                .fill
                .bg,
            Some(theme.popover)
        );
        let pressed = system.button_recipe(
            ButtonRecipeVariant::Secondary,
            ControlState::Pressed,
            theme.surface,
        );
        assert_eq!(pressed.fill.bg, Some(theme.text_primary));
        assert_eq!(pressed.label.fg, Some(theme.canvas));
        assert!(!pressed.label.add_modifier.contains(Modifier::REVERSED));

        // Quiet rests on the container, hovers one plane up, brightens text.
        let quiet = system.button_recipe(
            ButtonRecipeVariant::Quiet,
            ControlState::Default,
            theme.surface,
        );
        assert_eq!(quiet.fill.bg, Some(theme.surface));
        assert_eq!(quiet.label.fg, Some(theme.text_secondary));
        let quiet_hover = system.button_recipe(
            ButtonRecipeVariant::Quiet,
            ControlState::Hovered,
            theme.surface,
        );
        assert_eq!(quiet_hover.fill.bg, Some(theme.surface_overlay));
        assert_eq!(quiet_hover.label.fg, Some(theme.text_primary));

        // Danger labels in error colour and presses solid.
        let danger = system.button_recipe(
            ButtonRecipeVariant::Destructive,
            ControlState::Default,
            theme.surface,
        );
        assert_eq!(danger.label.fg, Some(theme.error));
        assert_eq!(danger.fill.bg, Some(theme.surface_overlay));
        let danger_press = system.button_recipe(
            ButtonRecipeVariant::Destructive,
            ControlState::Pressed,
            theme.surface,
        );
        assert_eq!(danger_press.fill.bg, Some(theme.error));
        assert_eq!(danger_press.label.fg, Some(theme.text_primary));

        // Focus is weight, never a border and never a fill swap.
        let focused = system.button_recipe(
            ButtonRecipeVariant::Secondary,
            ControlState::Focused,
            theme.surface,
        );
        assert!(focused.label.add_modifier.contains(Modifier::BOLD));
        assert_eq!(focused.fill.bg, Some(theme.surface_overlay));
        assert!(!focused.bordered);

        // Busy is the idle pair verbatim: the spinner owns "working".
        let busy = system.button_recipe(
            ButtonRecipeVariant::Primary,
            ControlState::Loading,
            theme.surface,
        );
        assert_eq!(busy.fill.bg, Some(theme.accent));
        assert_eq!(busy.label.fg, Some(theme.text_on_accent));
        let disabled = system.button_recipe(
            ButtonRecipeVariant::Primary,
            ControlState::Disabled,
            theme.surface,
        );
        assert_eq!(disabled.label.fg, Some(theme.disabled));
        assert_eq!(
            system
                .button_recipe(
                    ButtonRecipeVariant::Primary,
                    ControlState::Disabled,
                    theme.surface
                )
                .fill
                .bg,
            system
                .button_recipe(
                    ButtonRecipeVariant::Primary,
                    ControlState::Disabled,
                    theme.surface
                )
                .fill
                .bg,
            "disabled state is computed from kind alone"
        );

        // junie buttons have no box border in any variant.
        for variant in [
            ButtonRecipeVariant::Primary,
            ButtonRecipeVariant::Secondary,
            ButtonRecipeVariant::Quiet,
            ButtonRecipeVariant::Destructive,
        ] {
            assert!(
                !system
                    .button_recipe(variant, ControlState::Focused, theme.surface)
                    .bordered,
                "{variant:?} grew a border"
            );
        }
    }

    #[test]
    fn link_recipe_is_the_underline_affordance() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let idle = system.button_recipe(
            ButtonRecipeVariant::Link,
            ControlState::Default,
            theme.surface,
        );
        assert_eq!(idle.label.fg, Some(theme.text_secondary));
        assert!(idle.label.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(idle.fill.bg, None, "a link never fills");
        let hovered = system.button_recipe(
            ButtonRecipeVariant::Link,
            ControlState::Hovered,
            theme.surface,
        );
        assert_eq!(hovered.label.fg, Some(theme.text_primary));
    }

    #[test]
    fn input_recipe_follows_the_junie_field() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let idle = system.input_recipe(ControlState::Default, false, false);
        assert_eq!(idle.fill.bg, Some(theme.field));
        assert_eq!(idle.value.fg, Some(theme.text_primary));
        assert_eq!(idle.placeholder.fg, Some(theme.text_muted));
        assert_eq!(idle.border.fg, Some(theme.border_subtle));
        let (idle_glyph, idle_prompt) = idle.prompt.expect("col 0 is always the focus bar");
        assert_eq!(idle_glyph, system.glyphs.selection_gutter());
        assert_eq!(
            idle_prompt.fg, idle_prompt.bg,
            "idle gutter is reserved, fg=bg"
        );

        let nav = system.input_recipe(ControlState::Focused, false, false);
        assert!(
            !nav.border.add_modifier.contains(Modifier::UNDERLINED),
            "nav-focus does not underline"
        );
        assert_eq!(
            nav.prompt.expect("focused field carries the gutter").1.fg,
            Some(theme.focus)
        );

        let editing = system.input_recipe(ControlState::Focused, false, true);
        assert!(
            editing.border.add_modifier.contains(Modifier::UNDERLINED),
            "editing underlines"
        );
        assert_eq!(
            editing.border.underline_color,
            Some(theme.accent),
            "the editing underline is the accent, not a brighter border"
        );
        assert_eq!(
            editing
                .prompt
                .expect("editing field carries the gutter")
                .1
                .fg,
            Some(theme.focus)
        );

        let hovered = system.input_recipe(ControlState::Hovered, false, false);
        assert_eq!(hovered.fill.bg, Some(theme.field_hover));

        let bad = system.input_recipe(ControlState::Default, true, false);
        assert!(
            bad.border.add_modifier.contains(Modifier::UNDERLINED),
            "an invalid field underlines"
        );
        assert_eq!(
            bad.border.underline_color,
            Some(theme.error),
            "invalid states underline in error"
        );
        assert_ne!(bad.border, idle.border);
    }

    #[test]
    fn elevation_maps_roles() {
        let system = DesignSystem::junie();
        assert_eq!(
            system.elevation(Elevation::Raised),
            system.style(Role::Elevated)
        );
        // junie has one elevated plane; overlays share it and are told apart
        // by frame and backdrop, not by a lighter fill.
        assert_eq!(
            system.elevation(Elevation::Overlay),
            system.elevation(Elevation::Raised)
        );
        assert_eq!(Elevation::Canvas.role(), Role::Canvas);
        assert_ne!(
            system.elevation(Elevation::Surface),
            system.elevation(Elevation::Raised)
        );
    }

    #[test]
    fn theme_package_builtins_is_the_single_junie_package() {
        let packs = ThemePackage::builtins();
        assert_eq!(packs.len(), 1, "junie is the only shipped package");
        assert_eq!(packs[0].id, "junie");
        assert_eq!(packs[0].system, DesignSystem::junie());
    }

    #[test]
    fn quantize_ansi_preserves_structure() {
        let system = DesignSystem::junie().quantize(ColorCapability::Ansi16);
        let _ = system.style(Role::Accent);
        assert_eq!(system.capability, ColorCapability::Ansi16);
    }

    #[test]
    fn bordered_chrome_always_reserves_a_column() {
        let system = DesignSystem::junie();
        let bordered = system.content_inset(true);
        assert!(bordered.x >= 1, "bordered inset collapsed");
        assert_eq!(bordered.y, 0, "the border owns vertical rhythm");
        let plain = system.content_inset(false);
        assert_eq!(plain.x, system.spacing.card_inset);
    }

    #[test]
    fn content_inset_shrinks_a_rect_symmetrically() {
        let area = ratatui_core::layout::Rect::new(4, 2, 10, 6);
        let inset = ContentInset { x: 2, y: 1 };
        let inner = inset.apply(area);
        assert_eq!((inner.x, inner.y, inner.width, inner.height), (6, 3, 6, 4));
        // Never past empty on a rect narrower than the inset.
        let tiny = ContentInset { x: 3, y: 3 }.apply(ratatui_core::layout::Rect::new(0, 0, 2, 2));
        assert_eq!((tiny.width, tiny.height), (0, 0));
    }

    #[test]
    fn rhythm_band_is_surrendered_before_content_rows() {
        let band = DesignSystem::junie().spacing.band();
        // junie's section break is one blank row, at every height.
        assert_eq!(band.rows, 1);
        assert_eq!(band.resolve(10, 4), 1);
        assert_eq!(band.resolve(5, 5), 0);
        assert_eq!(SpacerBand { rows: 2 }.resolve(6, 4), 2);
    }

    #[test]
    fn focus_borders_snap_with_no_cross_fade() {
        // junie has no transition vocabulary left: the focused frame is the
        // strong hairline at every moment, not the end of a blend.
        let system = DesignSystem::default();
        let settled = system.panel_recipe(PanelChrome::Focused, Elevation::Surface);
        assert_eq!(
            settled.border.fg,
            Some(system.junie_theme().border_strong),
            "focus is the strong hairline"
        );
        let unfocused = system.panel_recipe(PanelChrome::Normal, Elevation::Surface);
        assert_eq!(
            unfocused.border.fg,
            Some(system.junie_theme().border_subtle)
        );
        // `Off` paints exactly the settled frame.
        let off = DesignSystem::default().motion(MotionPolicy::Off);
        assert_eq!(
            off.panel_recipe(PanelChrome::Focused, Elevation::Surface)
                .border
                .fg,
            settled.border.fg
        );
    }

    #[test]
    fn key_value_surfaces_share_one_separator_token() {
        let system = DesignSystem::junie();
        assert_eq!(system.kv_separator(), KvSeparator::Gutter);
        assert_eq!(system.kv_separator().text(), "  ");
        let colon = system.with_kv_separator(KvSeparator::Colon);
        assert_eq!(colon.kv_separator().text(), " : ");
        for separator in [KvSeparator::Gutter, KvSeparator::Colon] {
            assert_eq!(
                usize::from(separator.cols()),
                crate::text::display_cols(separator.text())
            );
        }
    }
}
