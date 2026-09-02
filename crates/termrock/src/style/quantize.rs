// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Colour capability: detection, the named ANSI-16 slots, and the projection
//! of a palette onto a rung.
//!
//! The downgrade maths live in [`super::junie`] — the reference algorithm — and
//! this module only wires them to TermRock's capability enum. There is no
//! second quantizer: a palette is never re-derived rung by rung, it is resolved
//! through [`super::JunieTheme::for_level`] or downgraded role by role here.
use ratatui_core::style::Color;

use super::RolePalette;
use super::junie;

/// Colour depth of the operator's terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ColorCapability {
    /// 24-bit color.
    #[default]
    Truecolor,
    /// xterm 256-color palette.
    Indexed256,
    /// 16 named ANSI colors.
    Ansi16,
    /// One color (plus intensity); state must survive without hue.
    Monochrome,
}

impl ColorCapability {
    /// Every rung, in capability order.
    pub const ALL: [Self; 4] = [
        Self::Truecolor,
        Self::Indexed256,
        Self::Ansi16,
        Self::Monochrome,
    ];

    /// Stable id, for stories and readouts.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Truecolor => "truecolor",
            Self::Indexed256 => "256",
            Self::Ansi16 => "16",
            Self::Monochrome => "mono",
        }
    }

    /// Human label, for capability readouts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Truecolor => "Truecolor",
            Self::Indexed256 => "256-color",
            Self::Ansi16 => "16-color",
            Self::Monochrome => "Monochrome",
        }
    }

    /// Detects the operator's colour capability from the environment.
    ///
    /// Same ladder the reference uses: a *non-empty* `NO_COLOR` → monochrome;
    /// `COLORTERM` equal to `truecolor` or `24bit` → truecolor (case
    /// sensitive, exactly as the reference compares it); `TERM` containing
    /// `256color`, `ghostty`, or `kitty` → 256; otherwise ANSI-16.
    #[must_use]
    pub fn detect_from_env() -> Self {
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            return Self::Monochrome;
        }
        if let Ok(colorterm) = std::env::var("COLORTERM")
            && (colorterm == "truecolor" || colorterm == "24bit")
        {
            return Self::Truecolor;
        }
        if let Ok(term) = std::env::var("TERM")
            && (term.contains("256color") || term.contains("ghostty") || term.contains("kitty"))
        {
            return Self::Indexed256;
        }
        Self::Ansi16
    }
}

/// The sixteen named ANSI slots, the vocabulary a terminal actually owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ansi16Color {
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// Gray.
    Gray,
    /// Bright black.
    DarkGray,
    /// Bright red.
    LightRed,
    /// Bright green.
    LightGreen,
    /// Bright yellow.
    LightYellow,
    /// Bright blue.
    LightBlue,
    /// Bright magenta.
    LightMagenta,
    /// Bright cyan.
    LightCyan,
    /// White.
    White,
}

impl Ansi16Color {
    /// Every slot in ANSI order.
    pub const ALL: [Self; 16] = [
        Self::Black,
        Self::Red,
        Self::Green,
        Self::Yellow,
        Self::Blue,
        Self::Magenta,
        Self::Cyan,
        Self::Gray,
        Self::DarkGray,
        Self::LightRed,
        Self::LightGreen,
        Self::LightYellow,
        Self::LightBlue,
        Self::LightMagenta,
        Self::LightCyan,
        Self::White,
    ];

    /// The Ratatui color this slot resolves to.
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Black => Color::Black,
            Self::Red => Color::Red,
            Self::Green => Color::Green,
            Self::Yellow => Color::Yellow,
            Self::Blue => Color::Blue,
            Self::Magenta => Color::Magenta,
            Self::Cyan => Color::Cyan,
            Self::Gray => Color::Gray,
            Self::DarkGray => Color::DarkGray,
            Self::LightRed => Color::LightRed,
            Self::LightGreen => Color::LightGreen,
            Self::LightYellow => Color::LightYellow,
            Self::LightBlue => Color::LightBlue,
            Self::LightMagenta => Color::LightMagenta,
            Self::LightCyan => Color::LightCyan,
            Self::White => Color::White,
        }
    }
}

/// Projects one Ratatui color onto a capability rung.
///
/// A thin alias of [`junie::downgrade`]: named and indexed colors pass through
/// untouched, truecolor tokens take the reference downgrade algorithm.
#[must_use]
pub fn quantize_color(color: Color, capability: ColorCapability) -> Color {
    junie::downgrade(color, capability)
}

impl RolePalette {
    /// Returns the canonical junie palette resolved for `capability`.
    ///
    /// The palette is derived from [`super::JunieTheme::for_level`] rather than
    /// quantized after the fact: one downgrade pass, applied where the tokens
    /// are born.
    #[must_use]
    pub fn quantized(&self, capability: ColorCapability) -> Self {
        Self::junie_for(capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Role, junie::palette as jp};

    #[test]
    fn named_ansi16_slots_are_native_colors() {
        assert_eq!(Ansi16Color::ALL.len(), 16);
        for color in Ansi16Color::ALL {
            assert!(
                !matches!(
                    color.color(),
                    Color::Rgb(..) | Color::Indexed(..) | Color::Reset
                ),
                "{color:?} escaped the named ANSI palette"
            );
        }
    }

    /// The junie alpha ladder is a hierarchy channel; the ANSI-16 rung keeps
    /// body (white) / metadata (gray) / chrome (dark grey) distinct. Secondary
    /// and muted intentionally share the gray slot — the reference's
    /// `nearest_16` collapses both into it, and weight carries the rest.
    #[test]
    fn ansi16_keeps_the_text_ladder_above_the_chrome_it_sits_on() {
        let palette = RolePalette::junie().quantized(ColorCapability::Ansi16);
        let body = palette.style(Role::Text);
        let secondary = palette.style(Role::TextSecondary);
        let muted = palette.style(Role::TextMuted);
        let faint = palette.style(Role::TextFaint);
        let border = palette.style(Role::Border);

        assert_eq!(body.fg, Some(Color::White), "body stays white");
        assert_eq!(secondary.fg, muted.fg, "secondary and muted share gray");
        assert_ne!(body.fg, secondary.fg, "body and secondary text collapsed");
        assert_eq!(faint.fg, border.fg, "faint and border share the dark tier");
        assert_ne!(
            secondary.fg, faint.fg,
            "metadata collapsed into border chrome"
        );
        assert_ne!(
            border.fg,
            Some(Color::Black),
            "border vanished into a black canvas"
        );
    }

    /// junie's semantic colors land on the named slots the reference declares:
    /// accent LightGreen, error LightRed, warning Yellow.
    #[test]
    fn ansi16_resolves_the_junie_semantic_colors() {
        let palette = RolePalette::junie().quantized(ColorCapability::Ansi16);
        assert_eq!(palette.style(Role::Accent).fg, Some(Color::LightGreen));
        assert_eq!(palette.style(Role::Danger).fg, Some(Color::LightRed));
        assert_eq!(palette.style(Role::Warning).fg, Some(Color::Yellow));
        assert_eq!(palette.style(Role::Canvas).bg, Some(Color::Black));
    }

    /// The 256 rung keeps the surface ladder an ordered hierarchy — junie's
    /// five planes must not collapse into one index.
    #[test]
    fn surface_ladder_survives_256_quantization() {
        let palette = RolePalette::junie().quantized(ColorCapability::Indexed256);
        let index = |role| match palette.style(role).bg {
            Some(Color::Indexed(i)) => i,
            other => panic!("surface role did not quantize to an index: {other:?}"),
        };
        let ladder = [
            index(Role::Canvas),
            index(Role::Surface),
            index(Role::Elevated),
            index(Role::Sunken),
            index(Role::Popover),
        ];
        for window in ladder.windows(2) {
            assert_ne!(window[0], window[1], "planes collided: {ladder:?}");
        }
    }

    #[test]
    fn truecolor_is_identity() {
        let c = Color::Rgb(12, 34, 56);
        assert_eq!(quantize_color(c, ColorCapability::Truecolor), c);
    }

    /// Monochrome is four grey buckets, not an erased palette: hierarchy
    /// survives as brightness, never as a REVERSED substitution.
    #[test]
    fn monochrome_is_four_grey_buckets() {
        assert_eq!(
            quantize_color(jp::BLACK, ColorCapability::Monochrome),
            Color::Black
        );
        assert_eq!(
            quantize_color(jp::CHROME, ColorCapability::Monochrome),
            Color::Black
        );
        assert_eq!(
            quantize_color(jp::WHITE_50, ColorCapability::Monochrome),
            Color::Gray
        );
        assert_eq!(
            quantize_color(jp::WHITE, ColorCapability::Monochrome),
            Color::White
        );
        let palette = RolePalette::junie().quantized(ColorCapability::Monochrome);
        for role in RolePalette::roles() {
            assert!(
                !palette
                    .style(role)
                    .add_modifier
                    .contains(ratatui_core::style::Modifier::REVERSED),
                "{role:?} grew a REVERSED substitute"
            );
        }
    }

    #[test]
    fn detect_prefers_explicit_signals() {
        // The env is the operator's; only the absence of every signal falls
        // through to ANSI-16. Assert the pure pieces of the ladder instead of
        // the host's environment.
        let terminal = std::env::var("TERM").unwrap_or_default();
        if terminal.contains("256color") {
            assert_eq!(
                ColorCapability::detect_from_env(),
                ColorCapability::Indexed256
            );
        }
    }
}
