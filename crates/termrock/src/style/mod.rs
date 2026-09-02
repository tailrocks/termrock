// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ratatui adapters for shared terminal design tokens.
//!
use ratatui_core::style::{Color, Modifier, Style};

#[cfg(test)]
mod contrast_floor;
mod glyph;
mod junie;
mod motion;
mod palette;
mod preview_host;
mod quantize;
mod tokens;

pub use glyph::{
    BLOCK_RAMP, BRAILLE_RAMP, GLYPH_CONTEXTS, Glyph, GlyphGroup, GlyphResolved, LEFT_BLOCK_RAMP,
    MASK_CELLS, SHADE_RAMP, SPINNER_BRAILLE_FRAMES, glyph_by_id,
};
pub use junie::{
    BadgeKind, ButtonKind, JunieTheme, SyntaxTone, Tone, VisualState, downgrade, nearest_16,
    nearest_256,
};
pub use motion::{ACTION_FLASH_MS, ActionFlash, MotionPolicy};
use palette::PREVIEW_CARD as PREVIEW_CARD_RGB;
pub use palette::{Rgb, contrast_ratio, relative_luminance};
pub use preview_host::{
    CapabilityPreviewHost, MediaSessionCommand, PreviewPresentation, PreviewSurface,
    PreviewSurfaceKind,
};
pub use quantize::{Ansi16Color, ColorCapability, quantize_color};
pub use tokens::{
    AccentUsage, BorderShape, BreakpointScale, ButtonRecipe, ButtonRecipeVariant, ContentInset,
    ControlState, DesignSystem, Elevation, FamilyRecipe, FocusEmphasis, GlyphSet, InputRecipe,
    KvSeparator, ListRowRecipe, ListRowVisualState, MotionSemantics, NonColorCue, PanelChrome,
    PanelRecipe, RecipeFamily, SelectionChrome, SpacerBand, SpacingScale, SurfaceFamily,
    ThemePackage,
};

#[must_use]
/// Converts this palette color into Ratatui color space.
pub const fn color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

/// Truecolor phosphor swatch for web/SVG export; runtime TUI recipes use
/// [`Ansi16Color`] names.
/// Truecolor preview-card swatch (the junie elevated plane) for web/SVG
/// export; never a runtime TUI authority.
pub const PREVIEW_CARD: Color = color(PREVIEW_CARD_RGB);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic visual roles resolved by a [`RolePalette`].
pub enum Role {
    /// Terminal-wide base background.
    Canvas,
    /// Ordinary component surface above the canvas.
    Surface,
    /// Card / dialog / popup surface.
    Elevated,
    /// Recessed well surface used by inputs and inset content.
    Sunken,
    /// Popover / dialog chrome surface.
    Popover,
    /// Occluding layer behind modal content.
    Backdrop,
    /// Ordinary body text (default weight).
    Text,
    /// Strong or heading text (bold).
    TextStrong,
    /// Secondary explanatory text (70% white).
    TextSecondary,
    /// Metadata text (50% white).
    TextMuted,
    /// Faint meta text (30% white).
    TextFaint,
    /// One step below faint: dimmed backdrops only.
    TextGhost,
    /// Unavailable or non-interactive text.
    TextDisabled,
    /// Text painted on an accent fill.
    TextOnAccent,
    /// Inactive component border.
    Border,
    /// Border of the component that owns focus.
    BorderFocused,
    /// Selected text or range.
    Selection,
    /// Selection tint behind the focused cursor row.
    SelectionTint,
    /// Non-border focus indicator.
    Focus,
    /// Brand-neutral visual accent.
    Accent,
    /// Successful or completed state.
    Success,
    /// Cautionary state that is not yet an error.
    Warning,
    /// Error, failure, or destructive state.
    Danger,
    /// Inactive hyperlink text.
    Link,
    /// Hyperlink text under the pointer.
    LinkHover,
    /// Editable input value and cursor surface.
    Input,
    /// Input that fails its validation contract.
    InputInvalid,
    /// Unoccupied scrollbar track.
    ScrollTrack,
    /// Scrollbar thumb representing the visible window.
    ScrollThumb,
    /// Selected tab label and fill.
    TabActive,
    /// Unselected tab label and fill.
    TabInactive,
    /// Key chord in an interaction hint.
    HintKey,
    /// Action label paired with a hint key.
    HintText,
    /// De-emphasized optional hint content.
    HintDim,
    /// Separator between adjacent interaction hints.
    HintSeparator,
    /// Focused dialog or action-bar control.
    ActionFocused,
    /// Disabled dialog or action-bar control.
    ActionDisabled,
    /// Status-bar background and ordinary text.
    StatusBar,
    /// Added line or segment in a diff.
    DiffAdded,
    /// Removed line or segment in a diff.
    DiffRemoved,
    /// Syntax: language keyword.
    SyntaxKeyword,
    /// Syntax: string literal.
    SyntaxString,
    /// Syntax: comment.
    SyntaxComment,
    /// Syntax: numeric literal.
    SyntaxNumber,
    /// Syntax: function / method name.
    SyntaxFunction,
    /// User actor accent for agent surfaces.
    ActorUser,
    /// Assistant actor accent for agent surfaces.
    ActorAssistant,
    /// Thinking actor accent for agent surfaces.
    ActorThinking,
    /// Tool actor accent for agent surfaces.
    ActorTool,
    /// Plan actor accent for agent surfaces.
    ActorPlan,
    /// System actor accent for agent surfaces.
    ActorSystem,
    /// Chart series 1 (primary series).
    ChartSeries1,
    /// Chart series 2.
    ChartSeries2,
    /// Chart series 3.
    ChartSeries3,
    /// Chart series 4.
    ChartSeries4,
    /// Chart axis labels and ticks.
    ChartAxis,
    /// Chart grid / guide lines.
    ChartGrid,
}

/// Number of [`Role`] variants (stable for palette array sizing).
pub const ROLE_COUNT: usize = 57;

macro_rules! every_role {
    ($macro:ident) => {
        $macro! {
            Canvas,
            Surface,
            Elevated,
            Sunken,
            Popover,
            Backdrop,
            Text,
            TextStrong,
            TextSecondary,
            TextMuted,
            TextFaint,
            TextGhost,
            TextDisabled,
            TextOnAccent,
            Border,
            BorderFocused,
            Selection,
            SelectionTint,
            Focus,
            Accent,
            Success,
            Warning,
            Danger,
            Link,
            LinkHover,
            Input,
            InputInvalid,
            ScrollTrack,
            ScrollThumb,
            TabActive,
            TabInactive,
            HintKey,
            HintText,
            HintDim,
            HintSeparator,
            ActionFocused,
            ActionDisabled,
            StatusBar,
            DiffAdded,
            DiffRemoved,
            SyntaxKeyword,
            SyntaxString,
            SyntaxComment,
            SyntaxNumber,
            SyntaxFunction,
            ActorUser,
            ActorAssistant,
            ActorThinking,
            ActorTool,
            ActorPlan,
            ActorSystem,
            ChartSeries1,
            ChartSeries2,
            ChartSeries3,
            ChartSeries4,
            ChartAxis,
            ChartGrid
        }
    };
}

macro_rules! role_array {
    ($($role:ident),+ $(,)?) => {
        [$(Role::$role),+]
    };
}

#[cfg(test)]
macro_rules! define_role_exhaustiveness_guard {
    ($($role:ident),+ $(,)?) => {
        const fn role_is_declared(role: Role) {
            match role {
                $(Role::$role => {}),+
            }
        }
    };
}

#[cfg(test)]
every_role!(define_role_exhaustiveness_guard);

#[derive(Debug, Clone, PartialEq, Eq)]
/// Semantic style roles used by every TermRock widget.
///
/// # Examples
///
/// ```
/// use ratatui_core::style::{Color, Style};
/// use termrock::style::{Role, RolePalette};
///
/// let theme = RolePalette::default().with_role(Role::Accent, Style::new().fg(Color::Cyan));
/// assert_eq!(theme.style(Role::Accent).fg, Some(Color::Cyan));
/// ```
pub struct RolePalette {
    roles: [Style; ROLE_COUNT],
}

impl RolePalette {
    /// Builds the canonical junie palette at truecolor.
    ///
    /// This is the only palette TermRock ships. Every role is either a direct
    /// junie token or a documented derivation (charts, actors, diff, link,
    /// syntax — see `research/junie-campaign/phase3-decision.md` D2/D3).
    ///
    /// Roles the resolvers own (hover, pressed, thumb focus) carry their
    /// resting value here; widgets call [`JunieTheme`] resolvers for the
    /// interactive forms instead of inventing a hover role.
    #[must_use]
    pub fn junie() -> Self {
        Self::junie_for(ColorCapability::Truecolor)
    }

    /// Builds the canonical junie palette resolved for a colour capability.
    ///
    /// Every token is downgraded by [`JunieTheme::for_level`]; nothing else in
    /// the crate quantizes colours after the fact.
    #[must_use]
    pub fn junie_for(level: ColorCapability) -> Self {
        let t = JunieTheme::for_level(level);
        Self::from_fn(move |role| match role {
            Role::Canvas => Style::new().bg(t.canvas),
            Role::Surface => Style::new().bg(t.surface),
            Role::Elevated => Style::new().bg(t.surface_elevated),
            // Recessed wells and fields share the junie field plane.
            Role::Sunken => Style::new().bg(t.field),
            Role::Popover => Style::new().bg(t.popover),
            // The dimmed page: the base run through `backdrop()`, so the role
            // can never drift from the resolver (D2).
            Role::Backdrop => t.backdrop(t.base()),
            Role::Text => Style::new().fg(t.text_primary),
            Role::TextStrong => Style::new().fg(t.text_primary).add_modifier(Modifier::BOLD),
            Role::TextSecondary => Style::new().fg(t.text_secondary),
            Role::TextMuted => Style::new().fg(t.text_muted),
            Role::TextFaint => Style::new().fg(t.text_faint),
            Role::TextGhost => Style::new().fg(t.text_ghost),
            Role::TextDisabled => Style::new().fg(t.disabled),
            Role::TextOnAccent => Style::new().fg(t.text_on_accent),
            Role::Border => Style::new().fg(t.border_subtle),
            Role::BorderFocused => Style::new().fg(t.border_strong),
            Role::Selection => Style::new().fg(t.text_primary).bg(t.popover),
            Role::SelectionTint => Style::new().bg(t.accent_bg),
            Role::Focus => Style::new().fg(t.focus),
            Role::Accent => Style::new().fg(t.accent),
            Role::Success => Style::new().fg(t.success),
            Role::Warning => Style::new().fg(t.warning),
            // Danger carries no weight; the glyph carries the alarm.
            Role::Danger => Style::new().fg(t.error),
            Role::Link => Style::new()
                .fg(t.text_secondary)
                .add_modifier(Modifier::UNDERLINED),
            // Hover is a plane, not a colour; the underline affordance stays.
            Role::LinkHover => Style::new()
                .fg(t.text_primary)
                .add_modifier(Modifier::UNDERLINED),
            Role::Input => Style::new().fg(t.text_primary).bg(t.field),
            Role::InputInvalid => Style::new().fg(t.error).bg(t.field),
            Role::ScrollTrack => Style::new().fg(t.border_subtle),
            Role::ScrollThumb => Style::new().fg(t.text_muted),
            // Active tab: weight plus the accent `━` underline the tabs widget
            // paints; the role itself carries no underline.
            Role::TabActive => Style::new().fg(t.text_primary).add_modifier(Modifier::BOLD),
            Role::TabInactive => Style::new().fg(t.text_secondary),
            Role::HintKey => Style::new().fg(t.text_primary).add_modifier(Modifier::BOLD),
            Role::HintText => Style::new().fg(t.text_muted),
            Role::HintDim => Style::new().fg(t.text_faint),
            Role::HintSeparator => Style::new().fg(t.text_ghost),
            // Focus is the `▎` gutter glyph, never a background fill.
            Role::ActionFocused => Style::new().fg(t.focus).add_modifier(Modifier::BOLD),
            // No DIM anywhere in the system.
            Role::ActionDisabled => Style::new().fg(t.disabled),
            Role::StatusBar => Style::new().fg(t.text_secondary).bg(t.canvas),
            // D3: diff/change semantics copied from the reference grid — the
            // ladder plus a change glyph, never green/red.
            Role::DiffAdded => Style::new().fg(t.text_secondary),
            Role::DiffRemoved => Style::new().fg(t.text_muted),
            Role::SyntaxKeyword => Style::new().fg(t.text_primary).add_modifier(Modifier::BOLD),
            Role::SyntaxString => Style::new().fg(t.text_secondary),
            Role::SyntaxComment => Style::new().fg(t.text_faint).add_modifier(Modifier::ITALIC),
            Role::SyntaxNumber => Style::new().fg(t.text_secondary),
            Role::SyntaxFunction => Style::new().fg(t.text_primary),
            // D3: actors are ladder + glyphs + labels, never hue.
            Role::ActorUser => Style::new().fg(t.text_primary),
            Role::ActorAssistant => Style::new().fg(t.text_primary),
            Role::ActorThinking => Style::new().fg(t.text_muted),
            Role::ActorTool => Style::new().fg(t.text_primary),
            Role::ActorPlan => Style::new().fg(t.text_primary),
            Role::ActorSystem => Style::new().fg(t.text_muted),
            // D3: charts are achromatic; series walk the ladder.
            Role::ChartSeries1 => Style::new().fg(t.text_primary),
            Role::ChartSeries2 => Style::new().fg(t.text_secondary),
            Role::ChartSeries3 => Style::new().fg(t.text_muted),
            Role::ChartSeries4 => Style::new().fg(t.text_faint),
            Role::ChartAxis => Style::new().fg(t.text_muted),
            Role::ChartGrid => Style::new().fg(t.text_ghost),
        })
    }

    /// Start from an existing theme and override one semantic role.
    #[must_use]
    pub fn with_role(mut self, role: Role, style: Style) -> Self {
        self.roles[role as usize] = style;
        self
    }

    /// Merge non-empty styles from `other` (partial package inheritance).
    ///
    /// Empty styles (no fg/bg/modifiers) leave the base role unchanged.
    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        for role in Self::roles() {
            let s = other.style(role);
            if s.fg.is_some() || s.bg.is_some() || s.add_modifier != Modifier::empty() {
                self.roles[role as usize] = s;
            }
        }
        self
    }

    /// Build a theme by answering every semantic role from a function.
    #[must_use]
    pub fn from_fn(f: impl Fn(Role) -> Style) -> Self {
        let mut roles = [Style::new(); ROLE_COUNT];
        for role in Self::roles() {
            roles[role as usize] = f(role);
        }
        Self { roles }
    }

    /// Return every semantic role in stable positional order.
    #[must_use]
    pub const fn roles() -> [Role; ROLE_COUNT] {
        every_role!(role_array)
    }

    #[must_use]
    /// Resolves the Ratatui style assigned to a semantic role.
    pub const fn style(&self, role: Role) -> Style {
        self.roles[role as usize]
    }
}

impl Default for RolePalette {
    fn default() -> Self {
        Self::junie()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use junie::palette as jp;

    #[test]
    fn roles_cover_the_positional_theme_array() {
        let roles = RolePalette::roles();
        assert_eq!(roles.len(), ROLE_COUNT);
        assert_eq!(Role::ChartGrid as usize, roles.len() - 1);
        for (index, role) in roles.into_iter().enumerate() {
            role_is_declared(role);
            assert_eq!(role as usize, index);
        }
    }

    #[test]
    fn builders_override_and_populate_every_role() {
        let blue = Style::new().bg(Color::Blue);
        let theme = RolePalette::default().with_role(Role::TabActive, blue);
        assert_eq!(theme.style(Role::TabActive), blue);

        let generated = RolePalette::from_fn(|role| Style::new().fg(Color::Indexed(role as u8)));
        for role in RolePalette::roles() {
            assert_eq!(generated.style(role).fg, Some(Color::Indexed(role as u8)));
        }
    }

    #[test]
    fn default_is_the_junie_palette() {
        assert_eq!(RolePalette::default(), RolePalette::junie());
    }

    #[test]
    fn default_carries_the_junie_text_ladder() {
        let theme = RolePalette::default();
        assert!(
            !theme
                .style(Role::Text)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert!(
            theme
                .style(Role::TextStrong)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        // Junie's alpha ladder: strictly dimming neutral tiers. Text is white;
        // each step down is a lower white alpha over black.
        let ladder = [
            (Role::Text, jp::WHITE),
            (Role::TextSecondary, jp::WHITE_70),
            (Role::TextMuted, jp::WHITE_50),
            (Role::TextFaint, jp::WHITE_30),
            (Role::TextGhost, jp::WHITE_15),
        ];
        for (index, (role, expected)) in ladder.into_iter().enumerate() {
            assert_eq!(theme.style(role).fg, Some(expected), "{role:?} tier");
            for (other_role, _) in ladder.into_iter().skip(index + 1) {
                assert_ne!(
                    theme.style(role),
                    theme.style(other_role),
                    "{role:?} and {other_role:?} share a complete style"
                );
            }
        }
        // Disabled IS the faint tier in junie (`WHITE_30`); weight and the
        // absence of a marker tell "unavailable" apart from "meta".
        assert_eq!(theme.style(Role::TextDisabled).fg, Some(jp::WHITE_30));
    }

    #[test]
    fn default_borders_use_subtle_then_strong_not_accent() {
        let theme = RolePalette::default();
        // Focus moves the border up the neutral ladder; it never borrows the
        // accent, which is reserved for the focus gutter glyph.
        assert_eq!(theme.style(Role::Border).fg, Some(jp::WHITE_15));
        assert_eq!(theme.style(Role::BorderFocused).fg, Some(jp::WHITE_30));
        assert_ne!(
            theme.style(Role::BorderFocused).fg,
            theme.style(Role::Accent).fg
        );
    }

    #[test]
    fn junie_green_is_reserved_for_intent() {
        let palette = RolePalette::default();
        let green = palette.style(Role::Accent).fg;
        // Green is the one accent: focus, success, and the accent itself.
        assert_eq!(palette.style(Role::Focus).fg, green);
        assert_eq!(palette.style(Role::Success).fg, green);
        // Nothing else may spend it — charts, diff, links, hints, borders, and
        // the text ladder are all neutral.
        for role in [
            Role::Warning,
            Role::Danger,
            Role::Link,
            Role::ScrollThumb,
            Role::ScrollTrack,
            Role::ChartSeries1,
            Role::ChartAxis,
            Role::DiffAdded,
            Role::DiffRemoved,
            Role::HintText,
            Role::TabActive,
            Role::TabInactive,
            Role::Border,
            Role::BorderFocused,
            Role::Text,
            Role::TextSecondary,
        ] {
            assert_ne!(
                palette.style(role).fg,
                green,
                "{role:?} still paints the brand accent"
            );
        }
    }

    #[test]
    fn every_role_is_populated() {
        let palette = RolePalette::default();
        for role in RolePalette::roles() {
            let style = palette.style(role);
            assert!(
                style.fg.is_some() || style.bg.is_some(),
                "{role:?} must be populated"
            );
        }
    }

    #[test]
    fn selection_tint_carries_the_keyboard_ground() {
        let palette = RolePalette::default();
        assert_eq!(palette.style(Role::SelectionTint).bg, Some(jp::GREEN_20));
        // Hover is not a role: it is `lift()`, resolved per container ground.
        assert_eq!(
            JunieTheme::junie().lift(jp::CHROME),
            jp::OVERLAY,
            "hover lifts one plane, it never borrows a tint role"
        );
    }

    #[test]
    fn junie_surfaces_carry_the_canonical_ladder() {
        let palette = RolePalette::default();
        let expected = [
            (Role::Canvas, jp::BLACK),
            (Role::Surface, jp::CHROME),
            (Role::Elevated, jp::CARD),
            (Role::Sunken, jp::INPUT),
            (Role::Popover, jp::POPOVER),
        ];
        for (role, fill) in expected {
            assert_eq!(palette.style(role).bg, Some(fill), "{role:?} fill");
        }
        // The status bar rests on the canvas, never on its own plane.
        assert_eq!(palette.style(Role::StatusBar).bg, Some(jp::BLACK));
    }

    #[test]
    fn action_states_speak_through_weight_and_the_gutter() {
        let palette = RolePalette::default();
        let focused = palette.style(Role::ActionFocused);
        let disabled = palette.style(Role::ActionDisabled);
        // Focus is the accent gutter plus weight; it is not a background fill.
        assert_eq!(focused.fg, Some(jp::GREEN));
        assert!(focused.add_modifier.contains(Modifier::BOLD));
        assert_eq!(focused.bg, None);
        // Disabled is the faint tier, and it never dims.
        assert_eq!(disabled.fg, Some(jp::WHITE_30));
        assert!(!disabled.add_modifier.contains(Modifier::DIM));
        assert!(!focused.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn derived_buckets_walk_the_ladder_instead_of_inventing_hue() {
        let palette = RolePalette::default();
        // Charts: series descend the text ladder, axis and grid are meta.
        let expected = [
            (Role::ChartSeries1, jp::WHITE),
            (Role::ChartSeries2, jp::WHITE_70),
            (Role::ChartSeries3, jp::WHITE_50),
            (Role::ChartSeries4, jp::WHITE_30),
            (Role::ChartAxis, jp::WHITE_50),
            (Role::ChartGrid, jp::WHITE_15),
            // Actors: identity is the glyph and the label, never a hue.
            (Role::ActorUser, jp::WHITE),
            (Role::ActorAssistant, jp::WHITE),
            (Role::ActorTool, jp::WHITE),
            (Role::ActorPlan, jp::WHITE),
            (Role::ActorThinking, jp::WHITE_50),
            (Role::ActorSystem, jp::WHITE_50),
            // Diff/change semantics copied from the reference grid.
            (Role::DiffAdded, jp::WHITE_70),
            (Role::DiffRemoved, jp::WHITE_50),
            // Links are affordances: secondary text plus the underline.
            (Role::Link, jp::WHITE_70),
            (Role::LinkHover, jp::WHITE),
        ];
        for (role, fg) in expected {
            assert_eq!(palette.style(role).fg, Some(fg), "{role:?}");
        }
        assert!(
            palette
                .style(Role::Link)
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn junie_palette_pins_load_bearing_role_values() {
        let palette = RolePalette::default();
        let expected = [
            (Role::Text, Style::new().fg(jp::WHITE)),
            (
                Role::TextStrong,
                Style::new().fg(jp::WHITE).add_modifier(Modifier::BOLD),
            ),
            (Role::TextSecondary, Style::new().fg(jp::WHITE_70)),
            (Role::TextMuted, Style::new().fg(jp::WHITE_50)),
            (Role::TextFaint, Style::new().fg(jp::WHITE_30)),
            (Role::TextDisabled, Style::new().fg(jp::WHITE_30)),
            (Role::TextOnAccent, Style::new().fg(jp::ON_GREEN)),
            (Role::Border, Style::new().fg(jp::WHITE_15)),
            (Role::BorderFocused, Style::new().fg(jp::WHITE_30)),
            (Role::Focus, Style::new().fg(jp::GREEN)),
            (Role::Accent, Style::new().fg(jp::GREEN)),
            (Role::Success, Style::new().fg(jp::GREEN)),
            (Role::Warning, Style::new().fg(jp::AMBER)),
            (Role::Danger, Style::new().fg(jp::RED)),
            (Role::Selection, Style::new().fg(jp::WHITE).bg(jp::POPOVER)),
            (Role::SelectionTint, Style::new().bg(jp::GREEN_20)),
            (Role::Input, Style::new().fg(jp::WHITE).bg(jp::INPUT)),
            (Role::InputInvalid, Style::new().fg(jp::RED).bg(jp::INPUT)),
            (Role::ScrollThumb, Style::new().fg(jp::WHITE_50)),
            (Role::ScrollTrack, Style::new().fg(jp::WHITE_15)),
            (
                Role::HintKey,
                Style::new().fg(jp::WHITE).add_modifier(Modifier::BOLD),
            ),
            (Role::HintText, Style::new().fg(jp::WHITE_50)),
            (
                Role::SyntaxComment,
                Style::new().fg(jp::WHITE_30).add_modifier(Modifier::ITALIC),
            ),
            (
                Role::SyntaxKeyword,
                Style::new().fg(jp::WHITE).add_modifier(Modifier::BOLD),
            ),
            (Role::SyntaxString, Style::new().fg(jp::WHITE_70)),
        ];
        for (role, expected) in expected {
            assert_eq!(palette.style(role), expected, "{role:?}");
        }
    }
}
