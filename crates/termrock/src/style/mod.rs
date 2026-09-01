// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ratatui adapters for shared terminal design tokens.
//!
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
pub use density::Density;
pub use glyph::{
    BLOCK_RAMP, BRAILLE_RAMP, GLYPH_CONTEXTS, Glyph, GlyphGroup, GlyphResolved, LEFT_BLOCK_RAMP,
    MASK_CELLS, SHADE_RAMP, SPINNER_BRAILLE_FRAMES, SPINNER_DOT_PULSE_FRAMES, glyph_by_id,
};
pub use motion::{
    ACTION_FLASH_MS, AMBIENT_PEAK, ActionFlash, BASIC_TRANSITION_CAP, Easing, HEARTBEAT_PERIOD_MS,
    MotionChannel, MotionPolicy, blend_toward, breathe_over, channel_brightness, coalesce_cells,
    edge_fade, effective_alpha, fade_style, pulse_brightness, shimmer_at, shimmer_cells,
    smoothstep, wave_brightness,
};
use palette::{
    PHOSPHOR_DARK as PHOSPHOR_DARK_RGB, PHOSPHOR_GREEN as PHOSPHOR_GREEN_RGB,
    PREVIEW_CARD as PREVIEW_CARD_RGB,
};
pub use palette::{Rgb, contrast_ratio, relative_luminance};
pub use preview_host::{
    CapabilityPreviewHost, MediaSessionCommand, PreviewPresentation, PreviewSurface,
    PreviewSurfaceKind,
};
pub(crate) use quantize::degrade_chrome as degrade_projection_chrome;
pub use quantize::{
    Ansi16Color, ColorCapability, quantize_color, quantize_palette, rgb_to_xterm256,
};
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
pub const PHOSPHOR_GREEN: Color = color(PHOSPHOR_GREEN_RGB);
/// Truecolor dark phosphor swatch for web/SVG export; never runtime TUI authority.
pub const PHOSPHOR_DARK: Color = color(PHOSPHOR_DARK_RGB);
/// Truecolor preview-card swatch for web/SVG export; never runtime TUI authority.
pub const PREVIEW_CARD: Color = color(PREVIEW_CARD_RGB);

#[must_use]
/// Blends this color toward the canvas for subdued content.
pub fn faded(color: Color, alpha: f32) -> Color {
    let black = if matches!(
        color,
        Color::Reset
            | Color::Black
            | Color::Red
            | Color::Green
            | Color::Yellow
            | Color::Blue
            | Color::Magenta
            | Color::Cyan
            | Color::Gray
            | Color::DarkGray
            | Color::LightRed
            | Color::LightGreen
            | Color::LightYellow
            | Color::LightBlue
            | Color::LightMagenta
            | Color::LightCyan
            | Color::White
    ) {
        Color::Black
    } else {
        Color::Rgb(0, 0, 0)
    };
    blend_toward(color, black, 1.0 - alpha.clamp(0.0, 1.0))
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
    /// Builds the default restrained phosphor-on-black semantic theme.
    ///
    /// The runtime TUI baseline uses named ANSI-16 colors only. Their actual
    /// luminance belongs to the operator's terminal; TermRock controls
    /// hierarchy through roles, weight, reverse, borders, and glyphs.
    pub fn tailrocks_phosphor() -> Self {
        use Ansi16Color as A;

        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(Color::Reset),
            Role::Surface | Role::Raised | Role::Sunken => Style::new().bg(A::Black.color()),
            // Overlay elevation is structural (frame + title marker); Reset
            // gives it a terminal-native ground distinct from the black deck.
            Role::Elevated => Style::new().bg(Color::Reset),
            Role::Backdrop => Style::new().fg(A::DarkGray.color()),
            Role::Text => Style::new().fg(A::Gray.color()),
            Role::TextStrong => Style::new().fg(A::White.color()).bold(),
            Role::TextMuted => Style::new().fg(A::DarkGray.color()).dim(),
            Role::TextDisabled => Style::new()
                .fg(A::White.color())
                .add_modifier(Modifier::DIM | Modifier::CROSSED_OUT),
            Role::Border => Style::new().fg(A::DarkGray.color()),
            Role::BorderFocused => Style::new().fg(A::LightGreen.color()),
            Role::Selection => Style::new().bg(A::Green.color()).fg(A::Black.color()),
            Role::Focus => Style::new().fg(A::LightGreen.color()),
            Role::Accent => Style::new().fg(A::Green.color()),
            Role::Success => Style::new().fg(A::LightGreen.color()),
            Role::Warning => Style::new().fg(A::Yellow.color()),
            Role::Danger => Style::new().fg(A::Red.color()).bold(),
            Role::Info => Style::new().fg(A::Cyan.color()),
            Role::Link => Style::new().fg(A::Cyan.color()),
            Role::LinkHover => Style::new().fg(A::LightCyan.color()),
            Role::Input => Style::new().fg(A::Gray.color()).bg(A::Black.color()),
            Role::InputInvalid => Style::new().fg(A::Red.color()).bg(A::Black.color()),
            Role::ScrollTrack => Style::new().fg(A::Black.color()),
            Role::ScrollThumb => Style::new().fg(A::DarkGray.color()),
            Role::TabActive => Style::new().fg(A::White.color()).bold(),
            Role::TabInactive => Style::new().fg(A::Gray.color()),
            Role::TabActiveHovered => Style::new().fg(A::White.color()).bold(),
            Role::TabInactiveHovered => Style::new().fg(A::Gray.color()),
            Role::HintKey => Style::new().fg(A::White.color()).bold(),
            Role::HintText => Style::new().fg(A::Gray.color()),
            Role::HintDim => Style::new().fg(A::DarkGray.color()).dim(),
            Role::HintSeparator => Style::new().fg(A::DarkGray.color()),
            Role::ActionFocused => Style::new()
                .fg(A::Black.color())
                .bg(A::Green.color())
                .add_modifier(Modifier::BOLD),
            Role::ActionDisabled => Style::new().fg(A::DarkGray.color()).dim(),
            Role::StatusBar => Style::new().fg(A::Gray.color()).bg(A::Black.color()),
            Role::DiffAdded => Style::new().fg(A::LightGreen.color()),
            Role::DiffRemoved => Style::new().fg(A::Red.color()),
            Role::SyntaxKeyword => Style::new().fg(A::Magenta.color()),
            Role::SyntaxString => Style::new().fg(A::LightGreen.color()),
            Role::SyntaxComment => Style::new().fg(A::DarkGray.color()),
            Role::SyntaxNumber => Style::new().fg(A::Yellow.color()),
            Role::SyntaxFunction => Style::new().fg(A::Cyan.color()),
            Role::SelectionTint => Style::new().bg(A::DarkGray.color()),
            Role::HoverTint => Style::new().bg(A::Black.color()),
            Role::ActionConstructive => Style::new().fg(A::LightGreen.color()).bold(),
            Role::DisclosureHeader => Style::new().fg(A::Yellow.color()).bold(),
            Role::InfoStrong => Style::new().fg(A::Cyan.color()).bold(),
            Role::InfoDim => Style::new().fg(A::DarkGray.color()),
            Role::ActorUser => Style::new().fg(A::Gray.color()),
            Role::ActorAssistant => Style::new().fg(A::Magenta.color()),
            Role::ActorThinking => Style::new().fg(A::LightMagenta.color()),
            Role::ActorTool => Style::new().fg(A::DarkGray.color()),
            Role::ActorPlan => Style::new().fg(A::Yellow.color()),
            Role::ActorSystem => Style::new().fg(A::Cyan.color()),
            Role::ChartSeries1 => Style::new().fg(A::LightGreen.color()),
            Role::ChartSeries2 => Style::new().fg(A::Cyan.color()),
            Role::ChartSeries3 => Style::new().fg(A::Yellow.color()),
            Role::ChartSeries4 => Style::new().fg(A::Magenta.color()),
            Role::ChartAxis => Style::new().fg(A::Gray.color()),
            Role::ChartGrid => Style::new().fg(A::DarkGray.color()),
            Role::TextFaint => Style::new().fg(A::DarkGray.color()).italic(),
            Role::BackdropWash => Style::new().bg(A::Black.color()),
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
        palette.roles[Role::StatusBar as usize] = Style::new().fg(Ansi16Color::Gray.color());
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
            Role::TextFaint => Style::new().fg(Color::Rgb(118, 133, 156)).dim(),
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
        let disabled = Color::Rgb(161, 154, 150);
        let faint = Color::Rgb(133, 126, 122);
        let border = Color::Rgb(214, 211, 209);
        let accent = Color::Rgb(37, 99, 235);
        let selection = Color::Rgb(219, 234, 254);
        let success = Color::Rgb(17, 129, 59);
        let warning = Color::Rgb(152, 104, 3);
        let danger = Color::Rgb(202, 33, 33);
        let info = Color::Rgb(2, 120, 181);
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
            Role::TextFaint => Style::new().fg(faint).dim(),
            Role::BackdropWash => Style::new().bg(blend_toward(canvas, Color::Rgb(0, 0, 0), 0.4)),
        })
    }

    /// ANSI 16-color native palette (no RGB truecolor dependency).
    #[must_use]
    pub fn ansi() -> Self {
        use Ansi16Color as A;

        Self::from_fn(|role| match role {
            Role::Canvas => Style::new().bg(A::Black.color()),
            Role::Surface => Style::new().bg(A::Black.color()),
            Role::Raised => Style::new().bg(A::DarkGray.color()),
            Role::Elevated => Style::new().bg(A::Gray.color()),
            Role::Sunken => Style::new().bg(A::Black.color()),
            Role::Backdrop => Style::new().fg(A::DarkGray.color()),
            Role::Text => Style::new().fg(A::White.color()),
            Role::TextStrong => Style::new().fg(A::White.color()).bold(),
            Role::TextMuted => Style::new().fg(A::Gray.color()).dim(),
            Role::TextDisabled => Style::new().fg(A::DarkGray.color()),
            Role::Border => Style::new().fg(A::DarkGray.color()),
            Role::BorderFocused => Style::new().fg(A::Green.color()),
            Role::Selection => Style::new().fg(A::Black.color()).bg(A::Green.color()),
            Role::Focus => Style::new().fg(A::Green.color()),
            Role::Accent => Style::new().fg(A::Green.color()),
            Role::Success => Style::new().fg(A::Green.color()),
            Role::Warning => Style::new().fg(A::Yellow.color()),
            Role::Danger => Style::new().fg(A::Red.color()).bold(),
            Role::Info => Style::new().fg(A::Cyan.color()),
            Role::Link => Style::new().fg(A::Blue.color()),
            Role::LinkHover => Style::new().fg(A::Blue.color()).underlined(),
            Role::Input => Style::new().fg(A::White.color()).bg(A::Black.color()),
            Role::InputInvalid => Style::new().fg(A::Red.color()),
            Role::ScrollTrack => Style::new().fg(A::DarkGray.color()),
            Role::ScrollThumb => Style::new().fg(A::White.color()),
            Role::TabActive => Style::new().fg(A::Black.color()).bg(A::White.color()),
            Role::TabInactive => Style::new().fg(A::White.color()).bg(A::DarkGray.color()),
            Role::TabActiveHovered => Style::new().fg(A::Black.color()).bg(A::Gray.color()),
            Role::TabInactiveHovered => Style::new().fg(A::White.color()).bg(A::DarkGray.color()),
            Role::HintKey => Style::new().fg(A::White.color()).bold(),
            Role::HintText => Style::new().fg(A::Green.color()),
            Role::HintDim => Style::new().fg(A::Gray.color()).dim(),
            Role::HintSeparator => Style::new().fg(A::DarkGray.color()),
            Role::ActionFocused => Style::new()
                .fg(A::Black.color())
                .bg(A::Green.color())
                .bold(),
            Role::ActionDisabled => Style::new().fg(A::DarkGray.color()),
            Role::StatusBar => Style::new().fg(A::White.color()).bg(A::Black.color()),
            Role::DiffAdded => Style::new().fg(A::Green.color()),
            Role::DiffRemoved => Style::new().fg(A::Red.color()),
            Role::SyntaxKeyword => Style::new().fg(A::Magenta.color()),
            Role::SyntaxString => Style::new().fg(A::Green.color()),
            Role::SyntaxComment => Style::new().fg(A::Gray.color()).dim(),
            Role::SyntaxNumber => Style::new().fg(A::Yellow.color()),
            Role::SyntaxFunction => Style::new().fg(A::Cyan.color()),
            Role::SelectionTint => Style::new().bg(A::DarkGray.color()),
            Role::HoverTint => Style::new().bg(A::Black.color()),
            Role::ActionConstructive => Style::new().fg(A::Green.color()),
            Role::DisclosureHeader => Style::new().fg(A::Yellow.color()),
            Role::InfoStrong => Style::new().fg(A::Cyan.color()),
            Role::InfoDim => Style::new().fg(A::DarkGray.color()),
            Role::ActorUser => Style::new().fg(A::Gray.color()),
            Role::ActorAssistant => Style::new().fg(A::Magenta.color()),
            Role::ActorThinking => Style::new().fg(A::LightMagenta.color()),
            Role::ActorTool => Style::new().fg(A::DarkGray.color()),
            Role::ActorPlan => Style::new().fg(A::Yellow.color()),
            Role::ActorSystem => Style::new().fg(A::Blue.color()),
            Role::ChartSeries1 => Style::new().fg(A::Green.color()),
            Role::ChartSeries2 => Style::new().fg(A::Cyan.color()),
            Role::ChartSeries3 => Style::new().fg(A::Yellow.color()),
            Role::ChartSeries4 => Style::new().fg(A::Magenta.color()),
            Role::ChartAxis => Style::new().fg(A::Gray.color()),
            Role::ChartGrid => Style::new().fg(A::DarkGray.color()),
            Role::TextFaint => Style::new().fg(A::DarkGray.color()).dim(),
            Role::BackdropWash => Style::new().bg(A::Black.color()),
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
        // ANSI-16 has only four neutrals. Tone establishes the main ladder;
        // modifiers keep the lowest tiers distinct without inventing RGB.
        let tones = [
            Role::TextStrong,
            Role::Text,
            Role::TextMuted,
            Role::TextFaint,
        ];
        for (index, role) in tones.into_iter().enumerate() {
            for other in tones.into_iter().skip(index + 1) {
                assert_ne!(
                    theme.style(role),
                    theme.style(other),
                    "{role:?} and {other:?} share a complete style"
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
        assert_eq!(
            theme.style(Role::Border).fg,
            Some(Ansi16Color::DarkGray.color())
        );
        assert_eq!(
            theme.style(Role::BorderFocused).fg,
            Some(Ansi16Color::LightGreen.color())
        );
    }

    #[test]
    fn phosphor_baseline_uses_named_ansi_only() {
        let palette = RolePalette::tailrocks_phosphor();
        for role in RolePalette::roles() {
            let style = palette.style(role);
            for color in [style.fg, style.bg].into_iter().flatten() {
                assert!(
                    !matches!(color, Color::Rgb(..) | Color::Indexed(..)),
                    "{role:?} escaped the named ANSI baseline: {color:?}"
                );
            }
        }
    }

    #[test]
    fn faded_named_ansi_stays_in_named_terminal_space() {
        for color in [Color::Green, Color::LightGreen, Color::Gray, Color::Reset] {
            for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
                assert!(
                    !matches!(faded(color, alpha), Color::Rgb(..) | Color::Indexed(..)),
                    "fading {color:?} at {alpha} emitted a non-ANSI color"
                );
            }
        }
    }

    #[test]
    fn accents_are_distinct() {
        let palette = RolePalette::tailrocks_phosphor();
        let accent = palette.style(Role::Accent).fg;

        // Base green is the restrained brand mark. Focus and semantic success
        // use the bright ANSI slot so they do not impersonate ambient brand.
        assert_eq!(accent, Some(Ansi16Color::Green.color()));
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
            RolePalette::tailrocks_phosphor(),
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
            RolePalette::tailrocks_phosphor(),
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
        use Ansi16Color as A;
        let expected = [
            (Role::Text, Style::new().fg(A::Gray.color())),
            (
                Role::TextStrong,
                Style::new()
                    .fg(A::White.color())
                    .add_modifier(Modifier::BOLD),
            ),
            (Role::TextMuted, Style::new().fg(A::DarkGray.color()).dim()),
            (
                Role::TextDisabled,
                Style::new()
                    .fg(A::White.color())
                    .add_modifier(Modifier::DIM | Modifier::CROSSED_OUT),
            ),
            (
                Role::TextFaint,
                Style::new().fg(A::DarkGray.color()).italic(),
            ),
            (Role::Border, Style::new().fg(A::DarkGray.color())),
            (Role::BorderFocused, Style::new().fg(A::LightGreen.color())),
            (Role::Focus, Style::new().fg(A::LightGreen.color())),
            (
                Role::Selection,
                Style::new().bg(A::Green.color()).fg(A::Black.color()),
            ),
            (Role::Success, Style::new().fg(A::LightGreen.color())),
            (Role::Warning, Style::new().fg(A::Yellow.color())),
            (
                Role::Danger,
                Style::new().fg(A::Red.color()).add_modifier(Modifier::BOLD),
            ),
            (Role::Link, Style::new().fg(A::Cyan.color())),
            (
                Role::Input,
                Style::new().fg(A::Gray.color()).bg(A::Black.color()),
            ),
            (Role::ScrollThumb, Style::new().fg(A::DarkGray.color())),
            (Role::ScrollTrack, Style::new().fg(A::Black.color())),
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
