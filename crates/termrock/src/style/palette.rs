// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

/// Three-byte RGB value used to construct terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red channel intensity.
    pub r: u8,
    /// Green channel intensity.
    pub g: u8,
    /// Blue channel intensity.
    pub b: u8,
}

impl Rgb {
    #[must_use]
    /// Creates a color from red, green, and blue channel intensities.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    /// Recovers the channel triple from a truecolor terminal color.
    ///
    /// Indexed and named ANSI colors resolve against the operator's terminal
    /// palette, so they carry no value TermRock can measure and yield `None`.
    pub const fn from_color(color: ratatui_core::style::Color) -> Option<Self> {
        match color {
            ratatui_core::style::Color::Rgb(r, g, b) => Some(Self { r, g, b }),
            _ => None,
        }
    }
}

/// WCAG relative luminance of a terminal color (0.0 black … 1.0 white).
#[must_use]
pub fn relative_luminance(color: Rgb) -> f32 {
    fn channel(value: u8) -> f32 {
        let normalized = f32::from(value) / 255.0;
        if normalized <= 0.039_28 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// WCAG contrast ratio between two terminal colors, from `1.0` (identical) to
/// `21.0` (black on white).
///
/// Theme authors use this to prove a custom [`crate::style::RolePalette`]
/// clears TermRock's contrast floor before shipping it: body text ≥ 7.0,
/// secondary text ≥ 4.5, borders ≥ 1.3 against the surface they sit on.
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
    let (high, low) = {
        let (first, second) = (relative_luminance(a), relative_luminance(b));
        if first >= second {
            (first, second)
        } else {
            (second, first)
        }
    };
    (high + 0.05) / (low + 0.05)
}

pub(crate) const PHOSPHOR_GREEN: Rgb = Rgb::new(0, 255, 65);
pub(crate) const PHOSPHOR_DIM: Rgb = Rgb::new(0, 140, 30);
pub(crate) const PHOSPHOR_DARK: Rgb = Rgb::new(0, 80, 18);
pub(crate) const DIALOG_SCROLL_THUMB: Rgb = PHOSPHOR_GREEN;
pub(crate) const DIALOG_SCROLL_TRACK: Rgb = PHOSPHOR_DARK;
pub(crate) const WHITE: Rgb = Rgb::new(255, 255, 255);
pub(crate) const INPUT_BG_DIM: Rgb = Rgb::new(20, 24, 22);
pub(crate) const TAB_BG_INACTIVE: Rgb = Rgb::new(30, 30, 30);
pub(crate) const TAB_BG_INACTIVE_HOVER: Rgb = Rgb::new(48, 48, 48);
pub(crate) const TAB_BG_ACTIVE: Rgb = Rgb::new(42, 42, 42);
pub(crate) const TAB_BG_ACTIVE_HOVER: Rgb = Rgb::new(58, 58, 58);
pub(crate) const LINK_FG: Rgb = Rgb::new(0, 200, 200);
pub(crate) const LINK_FG_HOVER: Rgb = Rgb::new(130, 240, 240);
pub(crate) const BORDER_GRAY: Rgb = Rgb::new(80, 80, 80);
pub(crate) const DANGER_RED: Rgb = Rgb::new(255, 94, 122);
pub(crate) const CYAN: Rgb = Rgb::new(0, 180, 180);
pub(crate) const WARNING_YELLOW: Rgb = Rgb::new(255, 216, 94);
pub(crate) const PREVIEW_CARD: Rgb = Rgb::new(28, 28, 28);
pub(crate) const CANVAS: Rgb = Rgb::new(10, 12, 10);
pub(crate) const SURFACE: Rgb = Rgb::new(18, 22, 18);
pub(crate) const RAISED: Rgb = Rgb::new(26, 31, 26);
pub(crate) const ELEVATED: Rgb = Rgb::new(30, 38, 32);
pub(crate) const SUNKEN: Rgb = Rgb::new(13, 16, 13);
pub(crate) const BACKDROP_WASH: Rgb = Rgb::new(58, 68, 58);
pub(crate) const SELECTION_TINT: Rgb = Rgb::new(20, 51, 26);
pub(crate) const HOVER_TINT: Rgb = Rgb::new(26, 34, 28);
pub(crate) const SUCCESS_GREEN: Rgb = Rgb::new(61, 220, 90);
pub(crate) const CHART_GREEN: Rgb = Rgb::new(43, 217, 104);
pub(crate) const ACTION_CONSTRUCTIVE: Rgb = Rgb::new(180, 255, 180);
pub(crate) const DISCLOSURE_HEADER: Rgb = Rgb::new(255, 208, 102);
pub(crate) const INFO_DIM: Rgb = Rgb::new(0, 120, 120);
pub(crate) const ACTOR_USER: Rgb = Rgb::new(200, 200, 200);
pub(crate) const ACTOR_ASSISTANT: Rgb = Rgb::new(187, 154, 247);
pub(crate) const ACTOR_THINKING: Rgb = Rgb::new(154, 127, 209);
pub(crate) const ACTOR_TOOL: Rgb = Rgb::new(120, 120, 120);
pub(crate) const ACTOR_PLAN: Rgb = Rgb::new(255, 219, 141);
pub(crate) const ACTOR_SYSTEM: Rgb = Rgb::new(122, 162, 247);
