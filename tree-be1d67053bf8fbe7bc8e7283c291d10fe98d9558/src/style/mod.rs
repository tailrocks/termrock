// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ratatui adapters for shared terminal design tokens.
//!
//! Also exposes named `Style` constants for the most-repeated combinations
//! (`BOLD_WHITE`, `BOLD_GREEN`, `DIM`, `DANGER`) so callers avoid writing
//! `crate::style::BOLD_WHITE` inline.

use ratatui_core::style::{Color, Modifier, Style};

mod palette;

pub use palette::Rgb;
use palette::{
    BORDER_GRAY as BORDER_GRAY_RGB, CYAN as CYAN_RGB, DANGER_RED as DANGER_RED_RGB,
    DIALOG_SCROLL_THUMB as DIALOG_SCROLL_THUMB_RGB, DIALOG_SCROLL_TRACK as DIALOG_SCROLL_TRACK_RGB,
    INPUT_BG_DIM as INPUT_BG_DIM_RGB, LINK_FG as LINK_FG_RGB, LINK_FG_HOVER as LINK_FG_HOVER_RGB,
    PHOSPHOR_DARK as PHOSPHOR_DARK_RGB, PHOSPHOR_DIM as PHOSPHOR_DIM_RGB,
    PHOSPHOR_GREEN as PHOSPHOR_GREEN_RGB, PREVIEW_CARD as PREVIEW_CARD_RGB,
    TAB_BG_ACTIVE as TAB_BG_ACTIVE_RGB, TAB_BG_ACTIVE_HOVER as TAB_BG_ACTIVE_HOVER_RGB,
    TAB_BG_INACTIVE as TAB_BG_INACTIVE_RGB, TAB_BG_INACTIVE_HOVER as TAB_BG_INACTIVE_HOVER_RGB,
    WARNING_YELLOW as WARNING_YELLOW_RGB, WHITE as WHITE_RGB,
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
/// Semantic visual roles resolved by a [`Theme`].
pub enum Role {
    /// Terminal-wide base background.
    Canvas,
    /// Ordinary component surface above the canvas.
    Surface,
    /// Raised surface such as a dialog or preview card.
    Elevated,
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
}

macro_rules! every_role {
    ($macro:ident) => {
        $macro! {
            Canvas,
            Surface,
            Elevated,
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
            DiffRemoved
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
/// use termrock::{Theme, style::Role};
///
/// let theme = Theme::default().with_role(Role::Accent, Style::new().fg(Color::Cyan));
/// assert_eq!(theme.style(Role::Accent).fg, Some(Color::Cyan));
/// ```
pub struct Theme {
    roles: [Style; 38],
}

impl Theme {
    #[must_use]
    /// Builds the default phosphor-on-black semantic theme.
    pub fn tailrocks_phosphor() -> Self {
        Self {
            roles: [
                Style::new(),
                Style::new(),
                Style::new(),
                Style::new(),
                Style::new().fg(WHITE),
                BOLD_WHITE,
                DIM,
                Style::new().fg(BORDER_GRAY),
                BORDER,
                GREEN,
                Style::new().bg(PHOSPHOR_GREEN).fg(INK),
                GREEN,
                GREEN,
                GREEN,
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
                GREEN,
                DIM,
                Style::new().fg(BORDER_GRAY),
                Style::new().reversed(),
                Style::new().dim(),
                Style::new(),
                Style::new().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG),
                Style::new().fg(DIFF_REMOVED_FG).bg(DIFF_REMOVED_BG),
            ],
        }
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
                Style::new().bg(elevated),
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
                Style::new().fg(disabled).dim(),
                Style::new().fg(text).bg(surface),
                Style::new()
                    .fg(Color::Rgb(134, 239, 172))
                    .bg(Color::Rgb(20, 83, 45)),
                Style::new()
                    .fg(Color::Rgb(252, 165, 165))
                    .bg(Color::Rgb(127, 29, 29)),
            ],
        }
    }

    /// Start from an existing theme and override one semantic role.
    #[must_use]
    pub fn with_role(mut self, role: Role, style: Style) -> Self {
        self.roles[role as usize] = style;
        self
    }

    /// Build a theme by answering every semantic role from a function.
    #[must_use]
    pub fn from_fn(f: impl Fn(Role) -> Style) -> Self {
        let mut roles = [Style::new(); 38];
        for role in Self::roles() {
            roles[role as usize] = f(role);
        }
        Self { roles }
    }

    /// Return every semantic role in stable positional order.
    #[must_use]
    pub const fn roles() -> [Role; 38] {
        every_role!(role_array)
    }

    #[must_use]
    /// Resolves the Ratatui style assigned to a semantic role.
    pub const fn style(&self, role: Role) -> Style {
        self.roles[role as usize]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::tailrocks_phosphor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_cover_the_positional_theme_array() {
        let roles = Theme::roles();
        assert_eq!(roles.len(), 38);
        assert_eq!(Role::DiffRemoved as usize, roles.len() - 1);
        for (index, role) in roles.into_iter().enumerate() {
            role_is_declared(role);
            assert_eq!(role as usize, index);
        }
    }

    #[test]
    fn builders_override_and_populate_every_role() {
        let blue = Style::new().bg(Color::Blue);
        let theme = Theme::default().with_role(Role::TabActive, blue);
        assert_eq!(theme.style(Role::TabActive), blue);

        let generated = Theme::from_fn(|role| Style::new().fg(Color::Indexed(role as u8)));
        for role in Theme::roles() {
            assert_eq!(generated.style(role).fg, Some(Color::Indexed(role as u8)));
        }
    }

    #[test]
    fn default_is_the_phosphor_preset() {
        assert_eq!(Theme::default(), Theme::tailrocks_phosphor());
    }

    #[test]
    fn default_separates_ordinary_and_strong_text() {
        let theme = Theme::default();
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
        let theme = Theme::default();
        assert_eq!(theme.style(Role::Border).fg, Some(BORDER_GRAY));
        assert_eq!(theme.style(Role::BorderFocused).fg, Some(PHOSPHOR_GREEN));
    }

    #[test]
    fn slate_visibly_diverges_from_phosphor() {
        let slate = Theme::slate();
        let phosphor = Theme::tailrocks_phosphor();
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
        let theme = Theme::tailrocks_phosphor();
        let expected = [
            (Role::Text, Style::new().fg(Color::Rgb(255, 255, 255))),
            (Role::Border, Style::new().fg(Color::Rgb(80, 80, 80))),
            (Role::BorderFocused, Style::new().fg(Color::Rgb(0, 255, 65))),
            (
                Role::Selection,
                Style::new().bg(Color::Rgb(0, 255, 65)).fg(Color::Black),
            ),
            (Role::Success, Style::new().fg(Color::Rgb(0, 255, 65))),
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
        let theme = Theme::slate();
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
