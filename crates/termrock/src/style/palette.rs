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
pub(crate) const PHOSPHOR_DARK: Rgb = Rgb::new(0, 80, 18);
pub(crate) const PREVIEW_CARD: Rgb = Rgb::new(28, 28, 28);

/// Lifts a colour one step, for the "the pointer is here" answer on a surface
/// that already owns its ground.
///
/// A hover wash needs an empty background to land on. A filled control has
/// none, so its hover has to come from the fill itself: RGB colours lighten
/// (or darken, when they are already near white), and the eight base ANSI
/// colours swap to their bright twins. Anything else is returned unchanged —
/// the caller then falls back to a non-colour cue.
#[must_use]
pub fn lift(color: ratatui_core::style::Color) -> ratatui_core::style::Color {
    use ratatui_core::style::Color;

    match color {
        Color::Rgb(r, g, b) => {
            let rgb = Rgb { r, g, b };
            let lighten = relative_luminance(rgb) < 0.6;
            let step = |channel: u8| -> u8 {
                if lighten {
                    channel.saturating_add(((255 - channel) as f32 * 0.22) as u8)
                } else {
                    channel.saturating_sub((channel as f32 * 0.18) as u8)
                }
            };
            Color::Rgb(step(r), step(g), step(b))
        }
        Color::Black => Color::DarkGray,
        Color::Red => Color::LightRed,
        Color::Green => Color::LightGreen,
        Color::Yellow => Color::LightYellow,
        Color::Blue => Color::LightBlue,
        Color::Magenta => Color::LightMagenta,
        Color::Cyan => Color::LightCyan,
        Color::Gray => Color::White,
        other => other,
    }
}
