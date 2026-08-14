// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ratatui adapters for shared terminal design tokens.
//!
//! Also exposes named `Style` constants for the most-repeated combinations
//! (`STRONG`, `MUTED`, `BORDER`, `GREEN`, `DANGER`) so callers avoid writing
//! `crate::style::STRONG` inline.

use ratatui_core::style::{Color, Modifier, Style};

mod appearance;
#[cfg(test)]
mod contrast_floor;
mod density;
mod glyph;
mod motion;
mod palette;
mod preview_host;
mod quantize;
mod tokens;

pub use appearance::{Appearance, AppearanceThemeMap, palette_for_appearance};
pub use density::{Density, Motion};
pub use glyph::{
    BLOCK_RAMP, BRAILLE_RAMP, GLYPH_CONTEXTS, Glyph, GlyphGroup, GlyphResolved, LEFT_BLOCK_RAMP,
    MASK_CELLS, SHADE_RAMP, SPINNER_BRAILLE_FRAMES, SPINNER_DOT_PULSE_FRAMES, glyph_by_id,
};
pub use motion::{
    blend_toward, coalesce_cells, edge_fade, effective_alpha, fade_style, pulse_brightness,
    smoothstep, wave_brightness,
};
use palette::{
    ACTION_CONSTRUCTIVE as ACTION_CONSTRUCTIVE_RGB, ACTOR_ASSISTANT as ACTOR_ASSISTANT_RGB,
    ACTOR_PLAN as ACTOR_PLAN_RGB, ACTOR_SYSTEM as ACTOR_SYSTEM_RGB,
    ACTOR_THINKING as ACTOR_THINKING_RGB, ACTOR_TOOL as ACTOR_TOOL_RGB,
    ACTOR_USER as ACTOR_USER_RGB, BACKDROP_WASH as BACKDROP_WASH_RGB,
    BORDER_GRAY as BORDER_GRAY_RGB, CANVAS as CANVAS_RGB, CHART_GREEN as CHART_GREEN_RGB,
    CYAN as CYAN_RGB, DANGER_RED as DANGER_RED_RGB, DISCLOSURE_HEADER as DISCLOSURE_HEADER_RGB,
    ELEVATED as ELEVATED_RGB, FOCUS_GREEN as FOCUS_GREEN_RGB, HOVER_TINT as HOVER_TINT_RGB,
    INFO_DIM as INFO_DIM_RGB, LINK_FG as LINK_FG_RGB, LINK_FG_HOVER as LINK_FG_HOVER_RGB,
    PHOSPHOR_DARK as PHOSPHOR_DARK_RGB, PHOSPHOR_GREEN as PHOSPHOR_GREEN_RGB,
    PREVIEW_CARD as PREVIEW_CARD_RGB, RAISED as RAISED_RGB, SCROLL_TRACK as SCROLL_TRACK_RGB,
    SELECTION_TINT as SELECTION_TINT_RGB, SUCCESS_GREEN as SUCCESS_GREEN_RGB, SUNKEN as SUNKEN_RGB,
    SURFACE as SURFACE_RGB, TEXT_BODY as TEXT_BODY_RGB, TEXT_DISABLED as TEXT_DISABLED_RGB,
    TEXT_FAINT as TEXT_FAINT_RGB, TEXT_MUTED as TEXT_MUTED_RGB, TEXT_STRONG as TEXT_STRONG_RGB,
    WARNING_YELLOW as WARNING_YELLOW_RGB, WHITE as WHITE_RGB,
};
pub use palette::{Rgb, contrast_ratio, relative_luminance};
pub use preview_host::{
    CapabilityPreviewHost, MediaSessionCommand, PreviewPresentation, PreviewSurface,
    PreviewSurfaceKind,
};
pub(crate) use quantize::degrade_chrome as degrade_projection_chrome;
pub use quantize::{ColorCapability, quantize_color, quantize_palette, rgb_to_xterm256};
pub use tokens::{
    BorderShape, BreakpointScale, ButtonRecipe, ButtonRecipeVariant, ContentInset, ControlState,
    DesignSystem, Elevation, GlyphSet, InputRecipe, KvSeparator, ListRowRecipe, ListRowVisualState,
    PanelChrome, PanelRecipe, SelectionChrome, SpacerBand, SpacingScale, ThemePackage,
};

#[must_use]
/// Converts this palette color into Ratatui color space.
pub const fn color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

/// Primary phosphor accent used by the default design language.
pub const PHOSPHOR_GREEN: Color = color(PHOSPHOR_GREEN_RGB);
/// Dark phosphor surface used behind emphasized content.
pub const PHOSPHOR_DARK: Color = color(PHOSPHOR_DARK_RGB);
// Dialog backdrops paint the terminal's DEFAULT background, not a
// fixed colour: `Color::Reset` emits `\x1b[49m`, so modal overlays match the
// operator's terminal theme instead of forcing pure black that stands out
// against a themed (non-black) default. Occlusion still holds — Reset cells
// overwrite the chrome behind them with a space on the default background.
pub(crate) const DIALOG_BACKDROP: Color = Color::Reset;
pub(crate) const SCROLL_TRACK: Color = color(SCROLL_TRACK_RGB);
/// Pure white — reserved for consumer overrides and the ANSI/high-contrast
/// presets; the phosphor foreground ladder uses [`TEXT_BODY`]/[`TEXT_STRONG`].
pub(crate) const WHITE: Color = color(WHITE_RGB);
/// Phosphor foreground ladder.
pub(crate) const TEXT_BODY: Color = color(TEXT_BODY_RGB);
pub(crate) const TEXT_STRONG: Color = color(TEXT_STRONG_RGB);
pub(crate) const TEXT_MUTED: Color = color(TEXT_MUTED_RGB);
pub(crate) const TEXT_DISABLED: Color = color(TEXT_DISABLED_RGB);
pub(crate) const TEXT_FAINT: Color = color(TEXT_FAINT_RGB);
/// Non-border focus cue, distinct from `BorderFocused`.
pub(crate) const FOCUS_GREEN: Color = color(FOCUS_GREEN_RGB);
/// Foreground for text on bright chips/buttons.
///
/// ANSI black by design so terminals map it consistently with their palette.
pub(crate) const INK: Color = Color::Black;
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
pub(crate) const DIFF_ADDED_FG: Color = color(SUCCESS_GREEN_RGB);

/// Named style constants — the most-repeated `Style::default().fg(…).add_modifier(…)` chains.
pub(crate) const STRONG: Style = Style::new().fg(TEXT_STRONG).add_modifier(Modifier::BOLD);
pub(crate) const MUTED: Style = Style::new().fg(TEXT_MUTED);
pub(crate) const GREEN: Style = Style::new().fg(PHOSPHOR_GREEN);
pub(crate) const BORDER: Style = Style::new().fg(BORDER_GRAY);
pub(crate) const DANGER: Style = Style::new().fg(DANGER_RED).add_modifier(Modifier::BOLD);

#[must_use]
/// Blends this color toward the canvas for subdued content.
pub fn faded(color: Color, alpha: f32) -> Color {
    blend_toward(color, Color::Rgb(0, 0, 0), 1.0 - alpha.clamp(0.0, 1.0))
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
    /// Faint meta text (timestamps, counts) — below TextMuted.
    TextFaint,
    /// Bg-carrying overlay dim: the canvas blended ~60%, painted under every
    /// modal layer so the content behind a dialog recedes without going black.
    BackdropWash,
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
            ChartGrid,
            TextFaint,
            BackdropWash
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
        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(color(CANVAS_RGB)),
            Role::Surface => Style::new().bg(color(SURFACE_RGB)),
            Role::Raised => Style::new().bg(color(RAISED_RGB)),
            Role::Elevated => Style::new().bg(color(ELEVATED_RGB)),
            Role::Sunken => Style::new().bg(color(SUNKEN_RGB)),
            Role::Backdrop => Style::new().fg(color(BACKDROP_WASH_RGB)),
            Role::Text => Style::new().fg(TEXT_BODY),
            Role::TextStrong => STRONG,
            Role::TextMuted => MUTED,
            Role::TextDisabled => Style::new().fg(TEXT_DISABLED),
            Role::Border => BORDER,
            Role::BorderFocused => GREEN,
            Role::Selection => Style::new().bg(PHOSPHOR_GREEN).fg(INK),
            Role::Focus => Style::new().fg(FOCUS_GREEN),
            Role::Accent => GREEN,
            Role::Success => Style::new().fg(color(SUCCESS_GREEN_RGB)),
            Role::Warning => Style::new().fg(WARNING_YELLOW),
            Role::Danger => DANGER,
            Role::Info => Style::new().fg(CYAN),
            Role::Link => Style::new().fg(LINK_FG),
            Role::LinkHover => Style::new().fg(LINK_FG_HOVER),
            Role::Input => Style::new().bg(color(SUNKEN_RGB)),
            Role::InputInvalid => Style::new().bg(color(SUNKEN_RGB)).fg(DANGER_RED),
            Role::ScrollTrack => Style::new().fg(SCROLL_TRACK),
            Role::ScrollThumb => Style::new().fg(BORDER_GRAY),
            Role::TabActive => STRONG,
            Role::TabInactive => MUTED,
            Role::TabActiveHovered => STRONG.bg(color(HOVER_TINT_RGB)),
            Role::TabInactiveHovered => MUTED.bg(color(HOVER_TINT_RGB)),
            Role::HintKey => STRONG,
            Role::HintText => MUTED,
            Role::HintDim => Style::new().fg(TEXT_DISABLED),
            Role::HintSeparator => Style::new().fg(BORDER_GRAY),
            // Explicit RGB so lookbook SVG (and monochrome-unaware paths)
            // distinguish focused vs disabled actions without relying on
            // REVERSED/DIM modifiers alone.
            Role::ActionFocused => Style::new()
                .fg(INK)
                .bg(PHOSPHOR_GREEN)
                .add_modifier(Modifier::BOLD),
            Role::ActionDisabled => Style::new().fg(TEXT_DISABLED),
            Role::StatusBar => Style::new().fg(TEXT_BODY).bg(color(SURFACE_RGB)),
            Role::DiffAdded => Style::new().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG),
            Role::DiffRemoved => Style::new().fg(DIFF_REMOVED_FG).bg(DIFF_REMOVED_BG),
            Role::SyntaxKeyword => Style::new().fg(Color::Rgb(200, 120, 255)),
            Role::SyntaxString => Style::new().fg(Color::Rgb(180, 240, 160)),
            Role::SyntaxComment => Style::new().fg(TEXT_MUTED),
            Role::SyntaxNumber => Style::new().fg(Color::Rgb(255, 200, 100)),
            Role::SyntaxFunction => Style::new().fg(Color::Rgb(120, 220, 255)),
            Role::SelectionTint => Style::new().bg(color(SELECTION_TINT_RGB)),
            Role::HoverTint => Style::new().bg(color(HOVER_TINT_RGB)),
            Role::ActionConstructive => Style::new().fg(color(ACTION_CONSTRUCTIVE_RGB)),
            Role::DisclosureHeader => Style::new().fg(color(DISCLOSURE_HEADER_RGB)),
            Role::InfoStrong => Style::new().fg(CYAN),
            Role::InfoDim => Style::new().fg(color(INFO_DIM_RGB)),
            Role::ActorUser => Style::new().fg(color(ACTOR_USER_RGB)),
            Role::ActorAssistant => Style::new().fg(color(ACTOR_ASSISTANT_RGB)),
            Role::ActorThinking => Style::new().fg(color(ACTOR_THINKING_RGB)),
            Role::ActorTool => Style::new().fg(color(ACTOR_TOOL_RGB)),
            Role::ActorPlan => Style::new().fg(color(ACTOR_PLAN_RGB)),
            Role::ActorSystem => Style::new().fg(color(ACTOR_SYSTEM_RGB)),
            Role::ChartSeries1 => Style::new().fg(color(CHART_GREEN_RGB)),
            Role::ChartSeries2 => Style::new().fg(CYAN),
            Role::ChartSeries3 => Style::new().fg(WARNING_YELLOW),
            Role::ChartSeries4 => Style::new().fg(Color::Rgb(180, 120, 255)),
            Role::ChartAxis => Style::new().fg(BORDER_GRAY),
            Role::ChartGrid => Style::new().fg(Color::Rgb(50, 50, 50)),
            Role::TextFaint => Style::new().fg(TEXT_FAINT).add_modifier(Modifier::DIM),
            Role::BackdropWash => {
                Style::new().bg(blend_toward(color(CANVAS_RGB), Color::Rgb(0, 0, 0), 0.4))
            }
        })
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

        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(canvas),
            Role::Surface => Style::new().bg(surface),
            Role::Raised => Style::new().bg(Color::Rgb(40, 52, 72)),
            Role::Elevated => Style::new().bg(elevated),
            Role::Sunken => Style::new().bg(Color::Rgb(17, 28, 48)),
            Role::Backdrop => Style::new().bg(Color::Rgb(2, 6, 23)),
            Role::Text => Style::new().fg(text),
            Role::TextStrong => Style::new().fg(text).bold(),
            Role::TextMuted => Style::new().fg(muted),
            Role::TextDisabled => Style::new().fg(disabled).dim(),
            Role::Border => Style::new().fg(border),
            Role::BorderFocused => Style::new().fg(accent),
            Role::Selection => Style::new().fg(text).bg(selection),
            Role::Focus => Style::new().fg(accent),
            Role::Accent => Style::new().fg(accent),
            Role::Success => Style::new().fg(success),
            Role::Warning => Style::new().fg(warning),
            Role::Danger => Style::new().fg(danger).bold(),
            Role::Info => Style::new().fg(info),
            Role::Link => Style::new().fg(Color::Rgb(125, 211, 252)),
            Role::LinkHover => Style::new().fg(Color::Rgb(186, 230, 253)).underlined(),
            Role::Input => Style::new().bg(surface),
            Role::InputInvalid => Style::new().fg(danger).bg(Color::Rgb(69, 10, 10)),
            Role::ScrollTrack => Style::new().fg(elevated),
            Role::ScrollThumb => Style::new().fg(accent),
            Role::TabActive => Style::new().fg(text).bold(),
            Role::TabInactive => Style::new().fg(muted),
            Role::TabActiveHovered => Style::new().fg(text).bold().bg(Color::Rgb(36, 52, 68)),
            Role::TabInactiveHovered => Style::new().fg(muted).bg(Color::Rgb(36, 52, 68)),
            Role::HintKey => Style::new().fg(text).bold(),
            Role::HintText => Style::new().fg(accent),
            Role::HintDim => Style::new().fg(muted),
            Role::HintSeparator => Style::new().fg(border),
            Role::ActionFocused => Style::new().fg(canvas).bg(accent).bold(),
            Role::ActionDisabled => Style::new().fg(disabled),
            Role::StatusBar => Style::new().fg(text).bg(surface),
            Role::DiffAdded => Style::new()
                .fg(Color::Rgb(134, 239, 172))
                .bg(Color::Rgb(20, 83, 45)),
            Role::DiffRemoved => Style::new()
                .fg(Color::Rgb(252, 165, 165))
                .bg(Color::Rgb(127, 29, 29)),
            Role::SyntaxKeyword => Style::new().fg(Color::Rgb(192, 132, 252)),
            Role::SyntaxString => Style::new().fg(Color::Rgb(134, 239, 172)),
            Role::SyntaxComment => Style::new().fg(muted),
            Role::SyntaxNumber => Style::new().fg(Color::Rgb(253, 186, 116)),
            Role::SyntaxFunction => Style::new().fg(Color::Rgb(125, 211, 252)),
            Role::SelectionTint => Style::new().bg(Color::Rgb(20, 55, 80)),
            Role::HoverTint => Style::new().bg(Color::Rgb(36, 52, 68)),
            Role::ActionConstructive => Style::new().fg(Color::Rgb(167, 243, 208)),
            Role::DisclosureHeader => Style::new().fg(Color::Rgb(252, 211, 77)),
            Role::InfoStrong => Style::new().fg(info),
            Role::InfoDim => Style::new().fg(Color::Rgb(14, 116, 144)),
            Role::ActorUser => Style::new().fg(Color::Rgb(203, 213, 225)),
            Role::ActorAssistant => Style::new().fg(Color::Rgb(216, 180, 254)),
            Role::ActorThinking => Style::new().fg(Color::Rgb(167, 139, 250)),
            Role::ActorTool => Style::new().fg(Color::Rgb(148, 163, 184)),
            Role::ActorPlan => Style::new().fg(Color::Rgb(253, 230, 138)),
            Role::ActorSystem => Style::new().fg(Color::Rgb(147, 197, 253)),
            Role::ChartSeries1 => Style::new().fg(accent),
            Role::ChartSeries2 => Style::new().fg(info),
            Role::ChartSeries3 => Style::new().fg(warning),
            Role::ChartSeries4 => Style::new().fg(Color::Rgb(192, 132, 252)),
            Role::ChartAxis => Style::new().fg(muted),
            Role::ChartGrid => Style::new().fg(border),
            Role::TextFaint => Style::new().fg(Color::Rgb(100, 116, 139)).dim(),
            Role::BackdropWash => Style::new().bg(blend_toward(canvas, Color::Rgb(0, 0, 0), 0.4)),
        })
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
        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(canvas),
            Role::Surface => Style::new().bg(surface),
            Role::Raised => Style::new().bg(Color::Rgb(247, 245, 242)),
            Role::Elevated => Style::new().bg(elevated),
            Role::Sunken => Style::new().bg(Color::Rgb(238, 235, 230)),
            Role::Backdrop => Style::new().bg(Color::Rgb(231, 229, 228)),
            Role::Text => Style::new().fg(text),
            Role::TextStrong => Style::new().fg(text).bold(),
            Role::TextMuted => Style::new().fg(muted),
            Role::TextDisabled => Style::new().fg(disabled),
            Role::Border => Style::new().fg(border),
            Role::BorderFocused => Style::new().fg(accent),
            Role::Selection => Style::new().fg(text).bg(selection),
            Role::Focus => Style::new().fg(accent),
            Role::Accent => Style::new().fg(accent),
            Role::Success => Style::new().fg(success),
            Role::Warning => Style::new().fg(warning),
            Role::Danger => Style::new().fg(danger).bold(),
            Role::Info => Style::new().fg(info),
            Role::Link => Style::new().fg(accent),
            Role::LinkHover => Style::new().fg(accent).underlined(),
            Role::Input => Style::new().fg(text).bg(Color::Rgb(238, 235, 230)),
            Role::InputInvalid => Style::new().fg(danger).bg(Color::Rgb(254, 226, 226)),
            Role::ScrollTrack => Style::new().fg(border),
            Role::ScrollThumb => Style::new().fg(accent),
            Role::TabActive => Style::new().fg(text).bg(elevated),
            Role::TabInactive => Style::new().fg(muted).bg(surface),
            Role::TabActiveHovered => Style::new().fg(text).bg(Color::Rgb(226, 232, 240)),
            Role::TabInactiveHovered => Style::new().fg(text).bg(elevated),
            Role::HintKey => Style::new().fg(text).bold(),
            Role::HintText => Style::new().fg(accent),
            Role::HintDim => Style::new().fg(muted),
            Role::HintSeparator => Style::new().fg(border),
            Role::ActionFocused => Style::new().fg(Color::Rgb(255, 255, 255)).bg(accent).bold(),
            Role::ActionDisabled => Style::new().fg(disabled),
            Role::StatusBar => Style::new().fg(text).bg(surface),
            Role::DiffAdded => Style::new().fg(success).bg(Color::Rgb(220, 252, 231)),
            Role::DiffRemoved => Style::new().fg(danger).bg(Color::Rgb(254, 226, 226)),
            Role::SyntaxKeyword => Style::new().fg(Color::Rgb(126, 34, 206)),
            Role::SyntaxString => Style::new().fg(success),
            Role::SyntaxComment => Style::new().fg(muted),
            Role::SyntaxNumber => Style::new().fg(warning),
            Role::SyntaxFunction => Style::new().fg(info),
            Role::SelectionTint => Style::new().bg(Color::Rgb(219, 234, 254)),
            Role::HoverTint => Style::new().bg(Color::Rgb(241, 239, 236)),
            Role::ActionConstructive => Style::new().fg(Color::Rgb(5, 150, 105)),
            Role::DisclosureHeader => Style::new().fg(Color::Rgb(180, 83, 9)),
            Role::InfoStrong => Style::new().fg(info),
            Role::InfoDim => Style::new().fg(Color::Rgb(14, 116, 144)),
            Role::ActorUser => Style::new().fg(Color::Rgb(68, 64, 60)),
            Role::ActorAssistant => Style::new().fg(Color::Rgb(126, 34, 206)),
            Role::ActorThinking => Style::new().fg(Color::Rgb(107, 33, 168)),
            Role::ActorTool => Style::new().fg(Color::Rgb(87, 83, 78)),
            Role::ActorPlan => Style::new().fg(Color::Rgb(161, 98, 7)),
            Role::ActorSystem => Style::new().fg(Color::Rgb(29, 78, 216)),
            Role::ChartSeries1 => Style::new().fg(accent),
            Role::ChartSeries2 => Style::new().fg(info),
            Role::ChartSeries3 => Style::new().fg(warning),
            Role::ChartSeries4 => Style::new().fg(Color::Rgb(147, 51, 234)),
            Role::ChartAxis => Style::new().fg(muted),
            Role::ChartGrid => Style::new().fg(border),
            Role::TextFaint => Style::new().fg(disabled).dim(),
            Role::BackdropWash => Style::new().bg(blend_toward(canvas, Color::Rgb(0, 0, 0), 0.4)),
        })
    }

    /// ANSI 16-color native palette (no RGB truecolor dependency).
    #[must_use]
    pub fn ansi() -> Self {
        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(Color::Black),
            Role::Surface => Style::new().bg(Color::Black),
            Role::Raised => Style::new().bg(Color::DarkGray),
            Role::Elevated => Style::new().bg(Color::Gray),
            Role::Sunken => Style::new().bg(Color::Black),
            Role::Backdrop => Style::new().fg(Color::DarkGray),
            Role::Text => Style::new().fg(Color::White),
            Role::TextStrong => Style::new().fg(Color::White).bold(),
            Role::TextMuted => Style::new().fg(Color::Gray).dim(),
            Role::TextDisabled => Style::new().fg(Color::DarkGray),
            Role::Border => Style::new().fg(Color::DarkGray),
            Role::BorderFocused => Style::new().fg(Color::Green),
            Role::Selection => Style::new().fg(Color::Black).bg(Color::Green),
            Role::Focus => Style::new().fg(Color::Green),
            Role::Accent => Style::new().fg(Color::Green),
            Role::Success => Style::new().fg(Color::Green),
            Role::Warning => Style::new().fg(Color::Yellow),
            Role::Danger => Style::new().fg(Color::Red).bold(),
            Role::Info => Style::new().fg(Color::Cyan),
            Role::Link => Style::new().fg(Color::Blue),
            Role::LinkHover => Style::new().fg(Color::Blue).underlined(),
            Role::Input => Style::new().fg(Color::White).bg(Color::Black),
            Role::InputInvalid => Style::new().fg(Color::Red),
            Role::ScrollTrack => Style::new().fg(Color::DarkGray),
            Role::ScrollThumb => Style::new().fg(Color::White),
            Role::TabActive => Style::new().fg(Color::Black).bg(Color::White),
            Role::TabInactive => Style::new().fg(Color::White).bg(Color::DarkGray),
            Role::TabActiveHovered => Style::new().fg(Color::Black).bg(Color::Gray),
            Role::TabInactiveHovered => Style::new().fg(Color::White).bg(Color::DarkGray),
            Role::HintKey => Style::new().fg(Color::White).bold(),
            Role::HintText => Style::new().fg(Color::Green),
            Role::HintDim => Style::new().fg(Color::Gray).dim(),
            Role::HintSeparator => Style::new().fg(Color::DarkGray),
            Role::ActionFocused => Style::new().fg(Color::Black).bg(Color::Green).bold(),
            Role::ActionDisabled => Style::new().fg(Color::DarkGray),
            Role::StatusBar => Style::new().fg(Color::White).bg(Color::Black),
            Role::DiffAdded => Style::new().fg(Color::Green),
            Role::DiffRemoved => Style::new().fg(Color::Red),
            Role::SyntaxKeyword => Style::new().fg(Color::Magenta),
            Role::SyntaxString => Style::new().fg(Color::Green),
            Role::SyntaxComment => Style::new().fg(Color::Gray).dim(),
            Role::SyntaxNumber => Style::new().fg(Color::Yellow),
            Role::SyntaxFunction => Style::new().fg(Color::Cyan),
            Role::SelectionTint => Style::new().bg(Color::DarkGray),
            Role::HoverTint => Style::new().bg(Color::Black),
            Role::ActionConstructive => Style::new().fg(Color::Green),
            Role::DisclosureHeader => Style::new().fg(Color::Yellow),
            Role::InfoStrong => Style::new().fg(Color::Cyan),
            Role::InfoDim => Style::new().fg(Color::DarkGray),
            Role::ActorUser => Style::new().fg(Color::Gray),
            Role::ActorAssistant => Style::new().fg(Color::Magenta),
            Role::ActorThinking => Style::new().fg(Color::LightMagenta),
            Role::ActorTool => Style::new().fg(Color::DarkGray),
            Role::ActorPlan => Style::new().fg(Color::Yellow),
            Role::ActorSystem => Style::new().fg(Color::Blue),
            Role::ChartSeries1 => Style::new().fg(Color::Green),
            Role::ChartSeries2 => Style::new().fg(Color::Cyan),
            Role::ChartSeries3 => Style::new().fg(Color::Yellow),
            Role::ChartSeries4 => Style::new().fg(Color::Magenta),
            Role::ChartAxis => Style::new().fg(Color::Gray),
            Role::ChartGrid => Style::new().fg(Color::DarkGray),
            Role::TextFaint => Style::new().fg(Color::DarkGray).dim(),
            Role::BackdropWash => Style::new().bg(Color::Black),
        })
    }

    /// High-contrast accessibility palette (strong fg/bg pairs, bold cues).
    #[must_use]
    pub fn high_contrast() -> Self {
        let ink = Color::Rgb(255, 255, 255);
        let body = Color::Rgb(230, 230, 230);
        let muted = Color::Rgb(192, 192, 192);
        let faint = Color::Rgb(166, 166, 166);
        let disabled = Color::Rgb(150, 150, 150);
        let paper = Color::Rgb(0, 0, 0);
        let accent = Color::Rgb(0, 255, 255);
        let danger = Color::Rgb(255, 64, 64);
        let warn = Color::Rgb(255, 255, 0);
        let ok = Color::Rgb(0, 255, 0);
        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(paper),
            Role::Surface => Style::new().bg(paper),
            Role::Raised => Style::new().bg(Color::Rgb(10, 10, 10)),
            Role::Elevated => Style::new().bg(Color::Rgb(20, 20, 20)),
            Role::Sunken => Style::new().bg(Color::Rgb(10, 10, 10)),
            Role::Backdrop => Style::new().bg(Color::Rgb(30, 30, 30)),
            Role::Text => Style::new().fg(body),
            Role::TextStrong => Style::new().fg(ink).bold(),
            Role::TextMuted => Style::new().fg(muted),
            Role::TextDisabled => Style::new().fg(disabled),
            Role::Border => Style::new().fg(ink),
            Role::BorderFocused => Style::new().fg(accent).bold(),
            Role::Selection => Style::new().fg(ink).bg(Color::Rgb(0, 80, 80)).bold(),
            Role::Focus => Style::new().fg(accent).bold(),
            Role::Accent => Style::new().fg(accent).bold(),
            Role::Success => Style::new().fg(ok).bold(),
            Role::Warning => Style::new().fg(warn).bold(),
            Role::Danger => Style::new().fg(danger).bold(),
            Role::Info => Style::new().fg(accent).bold(),
            Role::Link => Style::new().fg(accent).underlined(),
            Role::LinkHover => Style::new().fg(accent).underlined().bold(),
            Role::Input => Style::new().fg(ink).bg(paper),
            Role::InputInvalid => Style::new().fg(danger).bg(paper).bold(),
            Role::ScrollTrack => Style::new().fg(ink),
            Role::ScrollThumb => Style::new().fg(ink).bold(),
            Role::TabActive => Style::new().fg(paper).bg(ink).bold(),
            Role::TabInactive => Style::new().fg(ink).bg(paper),
            Role::TabActiveHovered => Style::new().fg(paper).bg(accent).bold(),
            Role::TabInactiveHovered => Style::new().fg(ink).bg(paper),
            Role::HintKey => Style::new().fg(ink).bold(),
            Role::HintText => Style::new().fg(accent).bold(),
            Role::HintDim => Style::new().fg(ink),
            Role::HintSeparator => Style::new().fg(ink),
            Role::ActionFocused => Style::new().fg(paper).bg(accent).bold(),
            Role::ActionDisabled => Style::new().fg(disabled),
            Role::StatusBar => Style::new().fg(ink).bg(paper),
            Role::DiffAdded => Style::new().fg(ok).bg(paper).bold(),
            Role::DiffRemoved => Style::new().fg(danger).bg(paper).bold(),
            Role::SyntaxKeyword => Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
            Role::SyntaxString => Style::new().fg(ok).bold(),
            Role::SyntaxComment => Style::new().fg(muted),
            Role::SyntaxNumber => Style::new().fg(warn).bold(),
            Role::SyntaxFunction => Style::new().fg(accent).bold(),
            Role::SelectionTint => Style::new().bg(Color::Rgb(0, 80, 80)),
            Role::HoverTint => Style::new().bg(Color::Rgb(30, 30, 30)),
            Role::ActionConstructive => Style::new().fg(ok).bold(),
            Role::DisclosureHeader => Style::new().fg(warn).bold(),
            Role::InfoStrong => Style::new().fg(accent).bold(),
            Role::InfoDim => Style::new().fg(Color::Rgb(0, 180, 180)),
            Role::ActorUser => Style::new().fg(ink),
            Role::ActorAssistant => Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
            Role::ActorThinking => Style::new().fg(Color::Rgb(220, 160, 255)).bold(),
            Role::ActorTool => Style::new().fg(muted),
            Role::ActorPlan => Style::new().fg(warn).bold(),
            Role::ActorSystem => Style::new().fg(accent).bold(),
            Role::ChartSeries1 => Style::new().fg(ok).bold(),
            Role::ChartSeries2 => Style::new().fg(accent).bold(),
            Role::ChartSeries3 => Style::new().fg(warn).bold(),
            Role::ChartSeries4 => Style::new().fg(Color::Rgb(255, 128, 255)).bold(),
            Role::ChartAxis => Style::new().fg(ink),
            Role::ChartGrid => Style::new().fg(Color::Rgb(120, 120, 120)),
            Role::TextFaint => Style::new().fg(faint).dim(),
            Role::BackdropWash => Style::new().bg(paper),
        })
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
        assert_eq!(Role::BackdropWash as usize, roles.len() - 1);
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
    fn default_is_the_phosphor_preset() {
        assert_eq!(RolePalette::default(), RolePalette::tailrocks_phosphor());
    }

    #[test]
    fn default_separates_ordinary_and_strong_text() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
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
        // The ladder separates by value, not by weight alone: bold on its own
        // is invisible on terminals that render it as brightness only.
        let tones = [
            Role::TextStrong,
            Role::Text,
            Role::TextMuted,
            Role::TextFaint,
        ];
        for (index, role) in tones.into_iter().enumerate() {
            for other in tones.into_iter().skip(index + 1) {
                assert_ne!(
                    theme.style(role).fg,
                    theme.style(other).fg,
                    "{role:?} and {other:?} share a foreground"
                );
            }
        }
        // Disabled and faint share the dimmest value; the modifier is what
        // tells "unavailable" apart from "meta".
        assert_ne!(
            theme.style(Role::TextDisabled),
            theme.style(Role::TextFaint),
            "disabled and faint text must not resolve to the same style"
        );
    }

    #[test]
    fn default_borders_use_gray_inactive_and_green_focused() {
        let theme = RolePalette::default();
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

        // Each step must be visible, not merely ordered.
        fn channel_sum(color: Color) -> i32 {
            let Color::Rgb(r, g, b) = color else {
                panic!("expected RGB ladder color, got {color:?}");
            };
            i32::from(r) + i32::from(g) + i32::from(b)
        }
        let steps = [Role::Canvas, Role::Surface, Role::Raised, Role::Elevated];
        for pair in steps.windows(2) {
            let step = channel_sum(bg(pair[1])) - channel_sum(bg(pair[0]));
            assert!(
                step >= 8,
                "{:?} -> {:?} is a {step}-point step; the ladder needs >= 8",
                pair[0],
                pair[1]
            );
        }

        // A field is a well in the same ladder, not a hand-picked gray.
        assert_eq!(
            palette.style(Role::Input).bg,
            palette.style(Role::Sunken).bg
        );
    }

    #[test]
    fn accents_are_distinct() {
        let palette = RolePalette::tailrocks_phosphor();
        let accent = palette.style(Role::Accent).fg;

        // Brand green is spent on exactly two roles: the accent itself and the
        // border of the container that owns focus. Anything ambient that used
        // to collapse into it now resolves somewhere else.
        assert_eq!(palette.style(Role::BorderFocused).fg, accent);
        for role in [
            Role::Focus,
            Role::Success,
            Role::ScrollThumb,
            Role::ScrollTrack,
            Role::ChartSeries1,
            Role::DiffAdded,
            Role::HintText,
            Role::TabActive,
            Role::Border,
        ] {
            assert_ne!(
                palette.style(role).fg,
                accent,
                "{role:?} still paints the brand accent"
            );
        }

        let distinct = [
            Role::Accent,
            Role::Focus,
            Role::Success,
            Role::ScrollThumb,
            Role::ChartSeries1,
        ];
        for (index, role) in distinct.into_iter().enumerate() {
            for other in distinct.into_iter().skip(index + 1) {
                assert_ne!(
                    palette.style(role).fg,
                    palette.style(other).fg,
                    "{role:?} and {other:?} share a foreground"
                );
            }
        }
    }

    #[test]
    fn hc_and_paper_have_text_ladders() {
        for palette in [RolePalette::paper(), RolePalette::high_contrast()] {
            assert_ne!(
                palette.style(Role::Text),
                palette.style(Role::TextStrong),
                "body and strong text must differ"
            );
            assert_ne!(
                palette.style(Role::TextMuted).fg,
                palette.style(Role::Text).fg,
                "muted text must differ from body"
            );
            assert_ne!(
                palette.style(Role::TextFaint).fg,
                palette.style(Role::TextMuted).fg,
                "faint text must differ from muted"
            );
        }
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
        let expected = [
            (Role::Text, Style::new().fg(Color::Rgb(214, 224, 214))),
            (
                Role::TextStrong,
                Style::new()
                    .fg(Color::Rgb(240, 245, 240))
                    .add_modifier(Modifier::BOLD),
            ),
            (Role::TextMuted, Style::new().fg(Color::Rgb(122, 138, 122))),
            (Role::TextDisabled, Style::new().fg(Color::Rgb(82, 96, 82))),
            (
                Role::TextFaint,
                Style::new()
                    .fg(Color::Rgb(94, 109, 94))
                    .add_modifier(Modifier::DIM),
            ),
            (Role::Border, Style::new().fg(Color::Rgb(48, 58, 50))),
            (Role::BorderFocused, Style::new().fg(Color::Rgb(0, 255, 65))),
            (Role::Focus, Style::new().fg(Color::Rgb(51, 255, 106))),
            (
                Role::Selection,
                Style::new().bg(Color::Rgb(0, 255, 65)).fg(Color::Black),
            ),
            (Role::Success, Style::new().fg(Color::Rgb(93, 255, 160))),
            (Role::Warning, Style::new().fg(Color::Rgb(255, 216, 94))),
            (
                Role::Danger,
                Style::new()
                    .fg(Color::Rgb(255, 94, 122))
                    .add_modifier(Modifier::BOLD),
            ),
            (Role::Link, Style::new().fg(Color::Rgb(94, 200, 255))),
            (Role::Input, Style::new().bg(Color::Rgb(13, 16, 13))),
            (Role::ScrollThumb, Style::new().fg(Color::Rgb(48, 58, 50))),
            (Role::ScrollTrack, Style::new().fg(Color::Rgb(22, 27, 22))),
            (
                Role::TabActive,
                Style::new()
                    .fg(Color::Rgb(240, 245, 240))
                    .add_modifier(Modifier::BOLD),
            ),
            (
                Role::HintKey,
                Style::new()
                    .fg(Color::Rgb(240, 245, 240))
                    .add_modifier(Modifier::BOLD),
            ),
            (
                Role::DiffAdded,
                Style::new()
                    .fg(Color::Rgb(93, 255, 160))
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
                    .add_modifier(Modifier::BOLD),
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
