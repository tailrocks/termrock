// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ratatui adapters for shared terminal design tokens.
//!
//! Also exposes named `Style` constants for the most-repeated combinations
//! (`BOLD_WHITE`, `BOLD_GREEN`, `DIM`, `DANGER`) so callers avoid writing
//! `crate::style::BOLD_WHITE` inline.

#![allow(unused_variables, unused_mut)] // unit-test fixtures
use ratatui_core::style::{Color, Modifier, Style};

mod appearance;
mod density;
mod glyph;
mod palette;
mod preview_host;
mod quantize;
mod tokens;

pub use appearance::{Appearance, AppearanceThemeMap, palette_for_appearance};
pub use density::{Density, Motion};
pub use glyph::{BLOCK_RAMP, Glyph, GlyphGroup, GlyphResolved, glyph_by_id};
pub use palette::Rgb;
use palette::{
    ACTION_CONSTRUCTIVE as ACTION_CONSTRUCTIVE_RGB, ACTOR_ASSISTANT as ACTOR_ASSISTANT_RGB,
    ACTOR_PLAN as ACTOR_PLAN_RGB, ACTOR_SYSTEM as ACTOR_SYSTEM_RGB,
    ACTOR_THINKING as ACTOR_THINKING_RGB, ACTOR_TOOL as ACTOR_TOOL_RGB,
    ACTOR_USER as ACTOR_USER_RGB, BACKDROP_WASH as BACKDROP_WASH_RGB,
    BORDER_GRAY as BORDER_GRAY_RGB, CANVAS as CANVAS_RGB, CHART_GREEN as CHART_GREEN_RGB,
    CYAN as CYAN_RGB, DANGER_RED as DANGER_RED_RGB, DIALOG_SCROLL_THUMB as DIALOG_SCROLL_THUMB_RGB,
    DIALOG_SCROLL_TRACK as DIALOG_SCROLL_TRACK_RGB, DISCLOSURE_HEADER as DISCLOSURE_HEADER_RGB,
    ELEVATED as ELEVATED_RGB, HOVER_TINT as HOVER_TINT_RGB, INFO_DIM as INFO_DIM_RGB,
    INPUT_BG_DIM as INPUT_BG_DIM_RGB, LINK_FG as LINK_FG_RGB, LINK_FG_HOVER as LINK_FG_HOVER_RGB,
    PHOSPHOR_DARK as PHOSPHOR_DARK_RGB, PHOSPHOR_DIM as PHOSPHOR_DIM_RGB,
    PHOSPHOR_GREEN as PHOSPHOR_GREEN_RGB, PREVIEW_CARD as PREVIEW_CARD_RGB, RAISED as RAISED_RGB,
    SELECTION_TINT as SELECTION_TINT_RGB, SUCCESS_GREEN as SUCCESS_GREEN_RGB, SUNKEN as SUNKEN_RGB,
    SURFACE as SURFACE_RGB, TAB_BG_ACTIVE as TAB_BG_ACTIVE_RGB,
    TAB_BG_ACTIVE_HOVER as TAB_BG_ACTIVE_HOVER_RGB, TAB_BG_INACTIVE as TAB_BG_INACTIVE_RGB,
    TAB_BG_INACTIVE_HOVER as TAB_BG_INACTIVE_HOVER_RGB, WARNING_YELLOW as WARNING_YELLOW_RGB,
    WHITE as WHITE_RGB,
};
pub use preview_host::{
    CapabilityPreviewHost, MediaSessionCommand, PreviewPresentation, PreviewSurface,
    PreviewSurfaceKind,
};
pub use quantize::{ColorCapability, quantize_color, quantize_palette, rgb_to_xterm256};
pub use tokens::{
    BorderShape, BreakpointScale, ButtonRecipe, ButtonRecipeVariant, ControlState, DesignSystem,
    Elevation, GlyphSet, InputRecipe, ListRowRecipe, ListRowVisualState, PanelChrome, PanelRecipe,
    SelectionChrome, SpacingScale, ThemePackage,
};

#[must_use]
/// Converts this palette color into Ratatui color space.
pub const fn color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

/// Primary phosphor accent used by the default design language.
pub const PHOSPHOR_GREEN: Color = color(PHOSPHOR_GREEN_RGB);
pub(crate) const PHOSPHOR_DIM: Color = color(PHOSPHOR_DIM_RGB);
/// Dark phosphor surface used behind emphasized content.
pub const PHOSPHOR_DARK: Color = color(PHOSPHOR_DARK_RGB);
pub(crate) const INPUT_BG_DIM: Color = color(INPUT_BG_DIM_RGB);
// Dialog backdrops paint the terminal's DEFAULT background, not a
// fixed colour: `Color::Reset` emits `\x1b[49m`, so modal overlays match the
// operator's terminal theme instead of forcing pure black that stands out
// against a themed (non-black) default. Occlusion still holds — Reset cells
// overwrite the chrome behind them with a space on the default background.
pub(crate) const DIALOG_BACKDROP: Color = Color::Reset;
pub(crate) const DIALOG_SCROLL_THUMB: Color = color(DIALOG_SCROLL_THUMB_RGB);
pub(crate) const DIALOG_SCROLL_TRACK: Color = color(DIALOG_SCROLL_TRACK_RGB);
pub(crate) const WHITE: Color = color(WHITE_RGB);
/// Foreground for text on bright chips/buttons.
///
/// ANSI black by design so terminals map it consistently with their palette.
pub(crate) const INK: Color = Color::Black;
pub(crate) const TAB_BG_INACTIVE: Color = color(TAB_BG_INACTIVE_RGB);
pub(crate) const TAB_BG_INACTIVE_HOVER: Color = color(TAB_BG_INACTIVE_HOVER_RGB);
pub(crate) const TAB_BG_ACTIVE: Color = color(TAB_BG_ACTIVE_RGB);
pub(crate) const TAB_BG_ACTIVE_HOVER: Color = color(TAB_BG_ACTIVE_HOVER_RGB);
pub(crate) const LINK_FG: Color = color(LINK_FG_RGB);
pub(crate) const LINK_FG_HOVER: Color = color(LINK_FG_HOVER_RGB);
pub(crate) const BORDER_GRAY: Color = color(BORDER_GRAY_RGB);
pub(crate) const DANGER_RED: Color = color(DANGER_RED_RGB);
pub(crate) const CYAN: Color = color(CYAN_RGB);
pub(crate) const WARNING_YELLOW: Color = color(WARNING_YELLOW_RGB);
/// Elevated preview-card background in the phosphor palette.
pub const PREVIEW_CARD: Color = color(PREVIEW_CARD_RGB);
pub(crate) const DIFF_REMOVED_BG: Color = Color::Rgb(60, 20, 20);
pub(crate) const DIFF_ADDED_BG: Color = Color::Rgb(20, 50, 20);
pub(crate) const DIFF_REMOVED_FG: Color = DANGER_RED;
pub(crate) const DIFF_ADDED_FG: Color = PHOSPHOR_GREEN;

/// Named style constants — the most-repeated `Style::default().fg(…).add_modifier(…)` chains.
pub(crate) const BOLD_WHITE: Style = Style::new().fg(WHITE).add_modifier(Modifier::BOLD);
pub(crate) const DIM: Style = Style::new().fg(PHOSPHOR_DIM);
pub(crate) const GREEN: Style = Style::new().fg(PHOSPHOR_GREEN);
pub(crate) const BORDER: Style = Style::new().fg(BORDER_GRAY);
pub(crate) const DANGER: Style = Style::new().fg(DANGER_RED).add_modifier(Modifier::BOLD);

#[must_use]
/// Blends this color toward the canvas for subdued content.
pub fn faded(color: Color, alpha: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            #[expect(
                clippy::cast_sign_loss,
                reason = "alpha clamped to 0.0..=1.0; product stays in u8 range"
            )]
            let scale = |component: u8| (f32::from(component) * alpha.clamp(0.0, 1.0)) as u8;
            Color::Rgb(scale(r), scale(g), scale(b))
        }
        other => other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic visual roles resolved by a [`RolePalette`].
pub enum Role {
    /// Terminal-wide base background.
    Canvas,
    /// Ordinary component surface above the canvas.
    Surface,
    /// Hover or section surface between ordinary and elevated surfaces.
    Raised,
    /// Raised surface such as a dialog or preview card.
    Elevated,
    /// Recessed well surface used by inputs and inset content.
    Sunken,
    /// Occluding layer behind modal content.
    Backdrop,
    /// Ordinary body text (default weight).
    Text,
    /// Strong or heading text (bold).
    TextStrong,
    /// Secondary explanatory text.
    TextMuted,
    /// Unavailable or non-interactive text.
    TextDisabled,
    /// Inactive component border.
    Border,
    /// Border of the component that owns focus.
    BorderFocused,
    /// Active row or item selection.
    Selection,
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
    /// Informational status or annotation.
    Info,
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
    /// Selected tab while hovered.
    TabActiveHovered,
    /// Unselected tab while hovered.
    TabInactiveHovered,
    /// Active-tab underline while the tab strip owns focus.
    TabUnderlineFocused,
    /// Active-tab underline while content owns focus.
    TabUnderlineUnfocused,
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
    /// Quiet selected-row background wash.
    SelectionTint,
    /// Pointer-hover row background wash.
    HoverTint,
    /// Creation or additive action-row accent.
    ActionConstructive,
    /// Expand/collapse group-header accent.
    DisclosureHeader,
    /// Strong live-status information accent.
    InfoStrong,
    /// Dim live-status information accent.
    InfoDim,
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
pub const ROLE_COUNT: usize = 63;

macro_rules! every_role {
    ($macro:ident) => {
        $macro! {
            Canvas,
            Surface,
            Raised,
            Elevated,
            Sunken,
            Backdrop,
            Text,
            TextStrong,
            TextMuted,
            TextDisabled,
            Border,
            BorderFocused,
            Selection,
            Focus,
            Accent,
            Success,
            Warning,
            Danger,
            Info,
            Link,
            LinkHover,
            Input,
            InputInvalid,
            ScrollTrack,
            ScrollThumb,
            TabActive,
            TabInactive,
            TabActiveHovered,
            TabInactiveHovered,
            TabUnderlineFocused,
            TabUnderlineUnfocused,
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
            SelectionTint,
            HoverTint,
            ActionConstructive,
            DisclosureHeader,
            InfoStrong,
            InfoDim,
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
    #[must_use]
    /// Builds the default phosphor-on-black semantic theme.
    pub fn tailrocks_phosphor() -> Self {
        Self {
            roles: [
                Style::new().bg(color(CANVAS_RGB)),
                Style::new().bg(color(SURFACE_RGB)),
                Style::new().bg(color(RAISED_RGB)),
                Style::new().bg(color(ELEVATED_RGB)),
                Style::new().bg(color(SUNKEN_RGB)),
                Style::new().fg(color(BACKDROP_WASH_RGB)),
                Style::new().fg(WHITE),
                BOLD_WHITE,
                DIM,
                Style::new().fg(BORDER_GRAY),
                BORDER,
                GREEN,
                Style::new().bg(PHOSPHOR_GREEN).fg(INK),
                GREEN,
                GREEN,
                Style::new().fg(color(SUCCESS_GREEN_RGB)),
                Style::new().fg(WARNING_YELLOW),
                DANGER,
                Style::new().fg(CYAN),
                Style::new().fg(LINK_FG),
                Style::new().fg(LINK_FG_HOVER),
                Style::new().bg(INPUT_BG_DIM),
                Style::new().bg(INPUT_BG_DIM).fg(DANGER_RED),
                Style::new().fg(DIALOG_SCROLL_TRACK),
                Style::new().fg(DIALOG_SCROLL_THUMB),
                Style::new().fg(WHITE).bg(TAB_BG_ACTIVE),
                Style::new().fg(WHITE).bg(TAB_BG_INACTIVE),
                Style::new().fg(WHITE).bg(TAB_BG_ACTIVE_HOVER),
                Style::new().fg(WHITE).bg(TAB_BG_INACTIVE_HOVER),
                GREEN,
                Style::new().fg(WHITE),
                Style::new().fg(WHITE).add_modifier(Modifier::BOLD),
                DIM,
                DIM,
                Style::new().fg(BORDER_GRAY),
                // Explicit RGB so lookbook SVG (and monochrome-unaware paths)
                // distinguish focused vs disabled actions without relying on
                // REVERSED/DIM modifiers alone.
                Style::new()
                    .fg(INK)
                    .bg(PHOSPHOR_GREEN)
                    .add_modifier(Modifier::BOLD),
                Style::new().fg(PHOSPHOR_DIM),
                Style::new().fg(WHITE).bg(color(SURFACE_RGB)),
                Style::new().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG),
                Style::new().fg(DIFF_REMOVED_FG).bg(DIFF_REMOVED_BG),
                // Syntax
                Style::new().fg(Color::Rgb(200, 120, 255)), // keyword
                Style::new().fg(Color::Rgb(180, 240, 160)), // string
                Style::new().fg(PHOSPHOR_DIM),              // comment
                Style::new().fg(Color::Rgb(255, 200, 100)), // number
                Style::new().fg(Color::Rgb(120, 220, 255)), // function
                Style::new().bg(color(SELECTION_TINT_RGB)),
                Style::new().bg(color(HOVER_TINT_RGB)),
                Style::new().fg(color(ACTION_CONSTRUCTIVE_RGB)),
                Style::new().fg(color(DISCLOSURE_HEADER_RGB)),
                Style::new().fg(CYAN),
                Style::new().fg(color(INFO_DIM_RGB)),
                Style::new().fg(color(ACTOR_USER_RGB)),
                Style::new().fg(color(ACTOR_ASSISTANT_RGB)),
                Style::new().fg(color(ACTOR_THINKING_RGB)),
                Style::new().fg(color(ACTOR_TOOL_RGB)),
                Style::new().fg(color(ACTOR_PLAN_RGB)),
                Style::new().fg(color(ACTOR_SYSTEM_RGB)),
                // Chart series + axis/grid
                Style::new().fg(color(CHART_GREEN_RGB)),
                Style::new().fg(CYAN),
                Style::new().fg(WARNING_YELLOW),
                Style::new().fg(Color::Rgb(180, 120, 255)),
                Style::new().fg(BORDER_GRAY),
                Style::new().fg(Color::Rgb(50, 50, 50)),
            ],
        }
    }

    /// Builds the phosphor palette while inheriting terminal-default surfaces.
    ///
    /// This preserves the pre-surface-ladder background behavior for hosts
    /// that must follow the operator's terminal theme.
    #[must_use]
    pub fn terminal_native() -> Self {
        let mut palette = Self::tailrocks_phosphor();
        for role in [
            Role::Canvas,
            Role::Surface,
            Role::Raised,
            Role::Elevated,
            Role::Sunken,
        ] {
            palette.roles[role as usize] = Style::new();
        }
        palette.roles[Role::StatusBar as usize] = Style::new().fg(WHITE);
        palette
    }

    /// Cool-gray neutrality proof and rebranding reference.
    ///
    /// Consumers can copy this preset into their application and adjust its
    /// role mappings without depending on TermRock's default design language.
    #[must_use]
    pub fn slate() -> Self {
        let canvas = Color::Rgb(15, 23, 42);
        let surface = Color::Rgb(30, 41, 59);
        let elevated = Color::Rgb(51, 65, 85);
        let text = Color::Rgb(226, 232, 240);
        let muted = Color::Rgb(148, 163, 184);
        let disabled = Color::Rgb(100, 116, 139);
        let border = Color::Rgb(71, 85, 105);
        let accent = Color::Rgb(96, 165, 250);
        let selection = Color::Rgb(30, 64, 175);
        let success = Color::Rgb(74, 222, 128);
        let warning = Color::Rgb(251, 191, 36);
        let danger = Color::Rgb(248, 113, 113);
        let info = Color::Rgb(56, 189, 248);

        Self {
            roles: [
                Style::new().bg(canvas),
                Style::new().bg(surface),
                Style::new().bg(Color::Rgb(40, 52, 72)),
                Style::new().bg(elevated),
                Style::new().bg(Color::Rgb(17, 28, 48)),
                Style::new().bg(Color::Rgb(2, 6, 23)),
                Style::new().fg(text),
                Style::new().fg(text).bold(),
                Style::new().fg(muted),
                Style::new().fg(disabled).dim(),
                Style::new().fg(border),
                Style::new().fg(accent),
                Style::new().fg(text).bg(selection),
                Style::new().fg(accent),
                Style::new().fg(accent),
                Style::new().fg(success),
                Style::new().fg(warning),
                Style::new().fg(danger).bold(),
                Style::new().fg(info),
                Style::new().fg(Color::Rgb(125, 211, 252)),
                Style::new().fg(Color::Rgb(186, 230, 253)).underlined(),
                Style::new().bg(surface),
                Style::new().fg(danger).bg(Color::Rgb(69, 10, 10)),
                Style::new().fg(elevated),
                Style::new().fg(accent),
                Style::new().fg(text).bg(elevated),
                Style::new().fg(muted).bg(surface),
                Style::new().fg(text).bg(Color::Rgb(71, 85, 105)),
                Style::new().fg(text).bg(elevated),
                Style::new().fg(accent),
                Style::new().fg(muted),
                Style::new().fg(text).bold(),
                Style::new().fg(accent),
                Style::new().fg(muted),
                Style::new().fg(border),
                Style::new().fg(canvas).bg(accent).bold(),
                Style::new().fg(disabled),
                Style::new().fg(text).bg(surface),
                Style::new()
                    .fg(Color::Rgb(134, 239, 172))
                    .bg(Color::Rgb(20, 83, 45)),
                Style::new()
                    .fg(Color::Rgb(252, 165, 165))
                    .bg(Color::Rgb(127, 29, 29)),
                // Syntax
                Style::new().fg(Color::Rgb(192, 132, 252)),
                Style::new().fg(Color::Rgb(134, 239, 172)),
                Style::new().fg(muted),
                Style::new().fg(Color::Rgb(253, 186, 116)),
                Style::new().fg(Color::Rgb(125, 211, 252)),
                Style::new().bg(Color::Rgb(20, 55, 80)),
                Style::new().bg(Color::Rgb(36, 52, 68)),
                Style::new().fg(Color::Rgb(167, 243, 208)),
                Style::new().fg(Color::Rgb(252, 211, 77)),
                Style::new().fg(info),
                Style::new().fg(Color::Rgb(14, 116, 144)),
                Style::new().fg(Color::Rgb(203, 213, 225)),
                Style::new().fg(Color::Rgb(216, 180, 254)),
                Style::new().fg(Color::Rgb(167, 139, 250)),
                Style::new().fg(Color::Rgb(148, 163, 184)),
                Style::new().fg(Color::Rgb(253, 230, 138)),
                Style::new().fg(Color::Rgb(147, 197, 253)),
                // Chart
                Style::new().fg(accent),
                Style::new().fg(info),
                Style::new().fg(warning),
                Style::new().fg(Color::Rgb(192, 132, 252)),
                Style::new().fg(muted),
                Style::new().fg(border),
            ],
        }
    }

    /// Light paper / daylight system (dark ink on warm canvas).
    #[must_use]
    pub fn paper() -> Self {
        let canvas = Color::Rgb(250, 248, 245);
        let surface = Color::Rgb(255, 255, 255);
        let elevated = Color::Rgb(244, 241, 236);
        let text = Color::Rgb(28, 25, 23);
        let muted = Color::Rgb(87, 83, 78);
        let disabled = Color::Rgb(168, 162, 158);
        let border = Color::Rgb(214, 211, 209);
        let accent = Color::Rgb(37, 99, 235);
        let selection = Color::Rgb(219, 234, 254);
        let success = Color::Rgb(22, 163, 74);
        let warning = Color::Rgb(202, 138, 4);
        let danger = Color::Rgb(220, 38, 38);
        let info = Color::Rgb(2, 132, 199);
        Self {
            roles: [
                Style::new().bg(canvas),
                Style::new().bg(surface),
                Style::new().bg(Color::Rgb(247, 245, 242)),
                Style::new().bg(elevated),
                Style::new().bg(Color::Rgb(238, 235, 230)),
                Style::new().bg(Color::Rgb(231, 229, 228)),
                Style::new().fg(text),
                Style::new().fg(text).bold(),
                Style::new().fg(muted),
                Style::new().fg(disabled),
                Style::new().fg(border),
                Style::new().fg(accent),
                Style::new().fg(text).bg(selection),
                Style::new().fg(accent),
                Style::new().fg(accent),
                Style::new().fg(success),
                Style::new().fg(warning),
                Style::new().fg(danger).bold(),
                Style::new().fg(info),
                Style::new().fg(accent),
                Style::new().fg(accent).underlined(),
                Style::new().fg(text).bg(surface),
                Style::new().fg(danger).bg(Color::Rgb(254, 226, 226)),
                Style::new().fg(border),
                Style::new().fg(accent),
                Style::new().fg(text).bg(elevated),
                Style::new().fg(muted).bg(surface),
                Style::new().fg(text).bg(Color::Rgb(226, 232, 240)),
                Style::new().fg(text).bg(elevated),
                Style::new().fg(accent),
                Style::new().fg(muted),
                Style::new().fg(text).bold(),
                Style::new().fg(accent),
                Style::new().fg(muted),
                Style::new().fg(border),
                Style::new().fg(Color::Rgb(255, 255, 255)).bg(accent).bold(),
                Style::new().fg(disabled),
                Style::new().fg(text).bg(surface),
                Style::new().fg(success).bg(Color::Rgb(220, 252, 231)),
                Style::new().fg(danger).bg(Color::Rgb(254, 226, 226)),
                Style::new().fg(Color::Rgb(126, 34, 206)),
                Style::new().fg(success),
                Style::new().fg(muted),
                Style::new().fg(warning),
                Style::new().fg(info),
                Style::new().bg(Color::Rgb(220, 252, 231)),
                Style::new().bg(Color::Rgb(240, 253, 244)),
                Style::new().fg(Color::Rgb(5, 150, 105)),
                Style::new().fg(Color::Rgb(180, 83, 9)),
                Style::new().fg(info),
                Style::new().fg(Color::Rgb(14, 116, 144)),
                Style::new().fg(Color::Rgb(68, 64, 60)),
                Style::new().fg(Color::Rgb(126, 34, 206)),
                Style::new().fg(Color::Rgb(107, 33, 168)),
                Style::new().fg(Color::Rgb(87, 83, 78)),
                Style::new().fg(Color::Rgb(161, 98, 7)),
                Style::new().fg(Color::Rgb(29, 78, 216)),
                Style::new().fg(accent),
                Style::new().fg(info),
                Style::new().fg(warning),
                Style::new().fg(Color::Rgb(147, 51, 234)),
                Style::new().fg(muted),
                Style::new().fg(border),
            ],
        }
    }

    /// ANSI 16-color native palette (no RGB truecolor dependency).
    #[must_use]
    pub fn ansi() -> Self {
        Self {
            roles: [
                Style::new().bg(Color::Black),
                Style::new().bg(Color::Black),
                Style::new().bg(Color::DarkGray),
                Style::new().bg(Color::DarkGray),
                Style::new().bg(Color::Black),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::White),
                Style::new().fg(Color::White).bold(),
                Style::new().fg(Color::Gray).dim(),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Black).bg(Color::Green),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Cyan),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Yellow),
                Style::new().fg(Color::Red).bold(),
                Style::new().fg(Color::Cyan),
                Style::new().fg(Color::Blue),
                Style::new().fg(Color::Blue).underlined(),
                Style::new().fg(Color::White),
                Style::new().fg(Color::Red),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::White),
                Style::new().fg(Color::Black).bg(Color::White),
                Style::new().fg(Color::White).bg(Color::DarkGray),
                Style::new().fg(Color::Black).bg(Color::Gray),
                Style::new().fg(Color::White).bg(Color::DarkGray),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::White),
                Style::new().fg(Color::White).bold(),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Gray).dim(),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::Black).bg(Color::Green).bold(),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::White).bg(Color::Black),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Red),
                Style::new().fg(Color::Magenta),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Gray).dim(),
                Style::new().fg(Color::Yellow),
                Style::new().fg(Color::Cyan),
                Style::new().bg(Color::DarkGray),
                Style::new().bg(Color::DarkGray),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Yellow),
                Style::new().fg(Color::Cyan),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::Gray),
                Style::new().fg(Color::Magenta),
                Style::new().fg(Color::Magenta),
                Style::new().fg(Color::DarkGray),
                Style::new().fg(Color::Yellow),
                Style::new().fg(Color::Blue),
                Style::new().fg(Color::Green),
                Style::new().fg(Color::Cyan),
                Style::new().fg(Color::Yellow),
                Style::new().fg(Color::Magenta),
                Style::new().fg(Color::Gray),
                Style::new().fg(Color::DarkGray),
            ],
        }
    }

    /// High-contrast accessibility palette (strong fg/bg pairs, bold cues).
    #[must_use]
    pub fn high_contrast() -> Self {
        let ink = Color::Rgb(255, 255, 255);
        let paper = Color::Rgb(0, 0, 0);
        let accent = Color::Rgb(0, 255, 255);
        let danger = Color::Rgb(255, 64, 64);
        let warn = Color::Rgb(255, 255, 0);
        let ok = Color::Rgb(0, 255, 0);
        Self {
            roles: [
                Style::new().bg(paper),
                Style::new().bg(paper),
                Style::new().bg(Color::Rgb(10, 10, 10)),
                Style::new().bg(Color::Rgb(20, 20, 20)),
                Style::new().bg(paper),
                Style::new().bg(paper),
                Style::new().fg(ink).bold(),
                Style::new().fg(ink).bold(),
                Style::new().fg(ink),
                Style::new().fg(Color::Rgb(180, 180, 180)),
                Style::new().fg(ink),
                Style::new().fg(accent).bold(),
                Style::new().fg(paper).bg(ink).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(ok).bold(),
                Style::new().fg(warn).bold(),
                Style::new().fg(danger).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(accent).underlined(),
                Style::new().fg(accent).underlined().bold(),
                Style::new().fg(ink).bg(paper),
                Style::new().fg(danger).bg(paper).bold(),
                Style::new().fg(ink),
                Style::new().fg(ink).bold(),
                Style::new().fg(paper).bg(ink).bold(),
                Style::new().fg(ink).bg(paper),
                Style::new().fg(paper).bg(accent).bold(),
                Style::new().fg(ink).bg(paper),
                Style::new().fg(accent).bold(),
                Style::new().fg(ink),
                Style::new().fg(ink).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(ink),
                Style::new().fg(ink),
                Style::new().fg(paper).bg(accent).bold(),
                Style::new().fg(Color::Rgb(180, 180, 180)),
                Style::new().fg(ink).bg(paper),
                Style::new().fg(ok).bg(paper).bold(),
                Style::new().fg(danger).bg(paper).bold(),
                Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
                Style::new().fg(ok).bold(),
                Style::new().fg(Color::Rgb(180, 180, 180)),
                Style::new().fg(warn).bold(),
                Style::new().fg(accent).bold(),
                Style::new().bg(Color::Rgb(0, 80, 80)),
                Style::new().bg(Color::Rgb(30, 30, 30)),
                Style::new().fg(ok).bold(),
                Style::new().fg(warn).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(Color::Rgb(0, 180, 180)),
                Style::new().fg(ink),
                Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
                Style::new().fg(Color::Rgb(220, 160, 255)).bold(),
                Style::new().fg(Color::Rgb(180, 180, 180)),
                Style::new().fg(warn).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(ok).bold(),
                Style::new().fg(accent).bold(),
                Style::new().fg(warn).bold(),
                Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
                Style::new().fg(ink),
                Style::new().fg(Color::Rgb(120, 120, 120)),
            ],
        }
    }

    /// Alias for [`Self::tailrocks_phosphor`].
    #[must_use]
    pub fn phosphor() -> Self {
        Self::tailrocks_phosphor()
    }

    /// Alias for phosphor (marketing name).
    #[must_use]
    pub fn obsidian() -> Self {
        Self::tailrocks_phosphor()
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
        Self::tailrocks_phosphor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        assert_eq!(theme.style(Role::TabActive), blue);

        let generated = RolePalette::from_fn(|role| Style::new().fg(Color::Indexed(role as u8)));
        for role in RolePalette::roles() {
            assert_eq!(generated.style(role).fg, Some(Color::Indexed(role as u8)));
        }
    }

    #[test]
    fn default_is_the_phosphor_preset() {
        assert_eq!(RolePalette::default(), RolePalette::tailrocks_phosphor());
    }

    #[test]
    fn default_separates_ordinary_and_strong_text() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        assert_eq!(theme.style(Role::Text).fg, Some(WHITE));
        assert!(
            !theme
                .style(Role::Text)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(theme.style(Role::TextStrong), BOLD_WHITE);
        assert!(
            theme
                .style(Role::TextStrong)
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn default_borders_use_gray_inactive_and_green_focused() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        assert_eq!(theme.style(Role::Border).fg, Some(BORDER_GRAY));
        assert_eq!(theme.style(Role::BorderFocused).fg, Some(PHOSPHOR_GREEN));
    }

    #[test]
    fn ladder_is_monotonic() {
        fn luminance(color: Color) -> f64 {
            let Color::Rgb(r, g, b) = color else {
                panic!("expected RGB ladder color, got {color:?}");
            };
            0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
        }

        let palette = RolePalette::tailrocks_phosphor();
        let bg = |role| palette.style(role).bg.expect("surface role must carry bg");
        let canvas = luminance(bg(Role::Canvas));
        let surface = luminance(bg(Role::Surface));
        let raised = luminance(bg(Role::Raised));
        let elevated = luminance(bg(Role::Elevated));
        let sunken = luminance(bg(Role::Sunken));
        assert!(canvas < surface && surface < raised && raised < elevated);
        assert!(sunken < surface);
    }

    #[test]
    fn accents_are_distinct() {
        let palette = RolePalette::tailrocks_phosphor();
        assert_ne!(palette.style(Role::Success), palette.style(Role::Accent));
        assert_ne!(palette.style(Role::HintText), palette.style(Role::Accent));
        assert_ne!(
            palette.style(Role::ChartSeries1),
            palette.style(Role::Accent)
        );
    }

    #[test]
    fn every_preset_fills_new_roles() {
        let new_roles = [
            Role::Raised,
            Role::Sunken,
            Role::SelectionTint,
            Role::HoverTint,
            Role::ActionConstructive,
            Role::DisclosureHeader,
            Role::InfoStrong,
            Role::InfoDim,
            Role::ActorUser,
            Role::ActorAssistant,
            Role::ActorThinking,
            Role::ActorTool,
            Role::ActorPlan,
            Role::ActorSystem,
        ];
        for palette in [
            RolePalette::phosphor(),
            RolePalette::slate(),
            RolePalette::paper(),
            RolePalette::ansi(),
            RolePalette::high_contrast(),
        ] {
            for role in new_roles {
                let style = palette.style(role);
                assert!(
                    style.fg.is_some() || style.bg.is_some(),
                    "{role:?} must be populated"
                );
            }
        }
    }

    #[test]
    fn tint_roles_carry_bg() {
        for palette in [
            RolePalette::phosphor(),
            RolePalette::slate(),
            RolePalette::paper(),
            RolePalette::high_contrast(),
        ] {
            assert!(palette.style(Role::SelectionTint).bg.is_some());
            assert!(palette.style(Role::HoverTint).bg.is_some());
        }
    }

    #[test]
    fn terminal_native_inherits_terminal_background() {
        let palette = RolePalette::terminal_native();
        for role in [
            Role::Canvas,
            Role::Surface,
            Role::Raised,
            Role::Elevated,
            Role::Sunken,
        ] {
            assert_eq!(palette.style(role), Style::new());
        }
        assert_eq!(palette.style(Role::StatusBar).bg, None);
        assert_eq!(
            crate::style::DesignSystem::terminal_native().palette,
            palette
        );
    }

    #[test]
    fn action_focused_and_disabled_use_distinct_rgb() {
        let theme = RolePalette::tailrocks_phosphor();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let focused = theme.style(Role::ActionFocused);
        let disabled = theme.style(Role::ActionDisabled);
        assert_ne!(focused.fg, disabled.fg);
        assert!(
            focused.bg.is_some(),
            "ActionFocused needs explicit bg for SVG"
        );
        assert!(
            disabled.fg.is_some(),
            "ActionDisabled needs explicit fg for SVG"
        );
        // Not modifier-only styles.
        assert_ne!(focused, Style::new().reversed());
        assert_ne!(disabled, Style::new().dim());
    }

    #[test]
    fn slate_visibly_diverges_from_phosphor() {
        let slate = RolePalette::slate();
        let phosphor = RolePalette::tailrocks_phosphor();
        for role in [
            Role::Accent,
            Role::Selection,
            Role::BorderFocused,
            Role::TabActive,
            Role::HintText,
            Role::DiffAdded,
            Role::DiffRemoved,
        ] {
            assert_ne!(slate.style(role), phosphor.style(role), "{role:?}");
        }
    }

    #[test]
    fn phosphor_preset_pins_load_bearing_role_values() {
        let theme = RolePalette::tailrocks_phosphor();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let expected = [
            (Role::Text, Style::new().fg(Color::Rgb(255, 255, 255))),
            (Role::Border, Style::new().fg(Color::Rgb(80, 80, 80))),
            (Role::BorderFocused, Style::new().fg(Color::Rgb(0, 255, 65))),
            (
                Role::Selection,
                Style::new().bg(Color::Rgb(0, 255, 65)).fg(Color::Black),
            ),
            (Role::Success, Style::new().fg(Color::Rgb(61, 220, 90))),
            (Role::Warning, Style::new().fg(Color::Rgb(255, 216, 94))),
            (
                Role::Danger,
                Style::new()
                    .fg(Color::Rgb(255, 94, 122))
                    .add_modifier(Modifier::BOLD),
            ),
            (Role::Link, Style::new().fg(Color::Rgb(0, 200, 200))),
            (Role::Input, Style::new().bg(Color::Rgb(20, 24, 22))),
            (Role::ScrollThumb, Style::new().fg(Color::Rgb(0, 255, 65))),
            (
                Role::TabActive,
                Style::new()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(Color::Rgb(42, 42, 42)),
            ),
            (
                Role::HintKey,
                Style::new()
                    .fg(Color::Rgb(255, 255, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            (
                Role::DiffAdded,
                Style::new()
                    .fg(Color::Rgb(0, 255, 65))
                    .bg(Color::Rgb(20, 50, 20)),
            ),
            (
                Role::DiffRemoved,
                Style::new()
                    .fg(Color::Rgb(255, 94, 122))
                    .bg(Color::Rgb(60, 20, 20)),
            ),
        ];
        for (role, expected) in expected {
            assert_eq!(theme.style(role), expected, "{role:?}");
        }
    }

    #[test]
    fn slate_preset_pins_load_bearing_role_values() {
        let theme = RolePalette::slate();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let expected = [
            (Role::Text, Style::new().fg(Color::Rgb(226, 232, 240))),
            (Role::Border, Style::new().fg(Color::Rgb(71, 85, 105))),
            (
                Role::BorderFocused,
                Style::new().fg(Color::Rgb(96, 165, 250)),
            ),
            (
                Role::Selection,
                Style::new()
                    .fg(Color::Rgb(226, 232, 240))
                    .bg(Color::Rgb(30, 64, 175)),
            ),
            (Role::Success, Style::new().fg(Color::Rgb(74, 222, 128))),
            (Role::Warning, Style::new().fg(Color::Rgb(251, 191, 36))),
            (
                Role::Danger,
                Style::new().fg(Color::Rgb(248, 113, 113)).bold(),
            ),
            (Role::Link, Style::new().fg(Color::Rgb(125, 211, 252))),
            (Role::Input, Style::new().bg(Color::Rgb(30, 41, 59))),
            (Role::ScrollThumb, Style::new().fg(Color::Rgb(96, 165, 250))),
            (
                Role::TabActive,
                Style::new()
                    .fg(Color::Rgb(226, 232, 240))
                    .bg(Color::Rgb(51, 65, 85)),
            ),
            (
                Role::HintKey,
                Style::new().fg(Color::Rgb(226, 232, 240)).bold(),
            ),
            (
                Role::DiffAdded,
                Style::new()
                    .fg(Color::Rgb(134, 239, 172))
                    .bg(Color::Rgb(20, 83, 45)),
            ),
            (
                Role::DiffRemoved,
                Style::new()
                    .fg(Color::Rgb(252, 165, 165))
                    .bg(Color::Rgb(127, 29, 29)),
            ),
        ];
        for (role, expected) in expected {
            assert_eq!(theme.style(role), expected, "{role:?}");
        }
    }
}
