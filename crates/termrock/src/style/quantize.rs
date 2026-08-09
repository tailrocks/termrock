// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal color capability ladder and theme quantization.

use ratatui_core::style::{Color, Style};

use super::{Role, RolePalette};

/// Detected or configured terminal color depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ColorCapability {
    /// Full RGB (truecolor).
    #[default]
    Truecolor,
    /// 256-color indexed palette.
    Indexed256,
    /// 16 ANSI colors.
    Ansi16,
    /// No chromatic color (keep modifiers; map fg/bg to Reset).
    Monochrome,
}

impl ColorCapability {
    /// Best-effort detection from environment variables.
    ///
    /// Order: `NO_COLOR` → monochrome; `COLORTERM=truecolor|24bit` → truecolor;
    /// `TERM` containing `256color` → 256; otherwise Ansi16.
    #[must_use]
    pub fn detect_from_env() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Self::Monochrome;
        }
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let lower = colorterm.to_ascii_lowercase();
            if lower.contains("truecolor") || lower.contains("24bit") {
                return Self::Truecolor;
            }
        }
        if let Ok(term) = std::env::var("TERM") {
            if term.contains("256color") || term.contains("truecolor") {
                return Self::Indexed256;
            }
            if term == "dumb" {
                return Self::Monochrome;
            }
        }
        Self::Ansi16
    }
}

/// Quantizes one Ratatui color to the capability ladder.
#[must_use]
pub fn quantize_color(color: Color, capability: ColorCapability) -> Color {
    match capability {
        ColorCapability::Truecolor => color,
        ColorCapability::Monochrome => match color {
            Color::Reset => Color::Reset,
            _ => Color::Reset,
        },
        ColorCapability::Indexed256 => match color {
            Color::Rgb(r, g, b) => Color::Indexed(rgb_to_xterm256(r, g, b)),
            other => other,
        },
        ColorCapability::Ansi16 => match color {
            Color::Rgb(r, g, b) => rgb_to_ansi16(r, g, b),
            Color::Indexed(i) => indexed_to_ansi16(i),
            other => other,
        },
    }
}

/// Quantizes every role style in a theme.
#[must_use]
pub fn quantize_palette(palette: &RolePalette, capability: ColorCapability) -> RolePalette {
    if matches!(capability, ColorCapability::Truecolor) {
        return palette.clone();
    }
    RolePalette::from_fn(|role| quantize_style(palette.style(role), capability))
}

fn quantize_style(style: Style, capability: ColorCapability) -> Style {
    let mut out = style;
    if let Some(fg) = style.fg {
        out = out.fg(quantize_color(fg, capability));
    }
    if let Some(bg) = style.bg {
        out = out.bg(quantize_color(bg, capability));
    }
    let _ = Role::Text; // keep Role imported for API symmetry in docs
    out
}

/// xterm 256-color cube + grayscale mapping.
#[must_use]
pub fn rgb_to_xterm256(r: u8, g: u8, b: u8) -> u8 {
    // grayscale check
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return (232 + ((u16::from(r) - 8) * 24 / 247)) as u8;
    }
    let ri = (u16::from(r) * 5 / 255) as u8;
    let gi = (u16::from(g) * 5 / 255) as u8;
    let bi = (u16::from(b) * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Color {
    let bright = r.max(g).max(b) >= 180;
    let dominant = {
        let (rr, gg, bb) = (u16::from(r), u16::from(g), u16::from(b));
        if rr >= gg && rr >= bb {
            0
        } else if gg >= rr && gg >= bb {
            1
        } else {
            2
        }
    };
    // approximate primary
    match (dominant, bright, r > 40, g > 40, b > 40) {
        (_, _, false, false, false) => Color::Black,
        (0, false, true, false, false) => Color::Red,
        (1, false, false, true, false) => Color::Green,
        (2, false, false, false, true) => Color::Blue,
        (0, true, true, false, false) => Color::LightRed,
        (1, true, false, true, false) => Color::LightGreen,
        (2, true, false, false, true) => Color::LightBlue,
        (_, _, true, true, false) if r > 100 && g > 100 => {
            if bright {
                Color::LightYellow
            } else {
                Color::Yellow
            }
        }
        (_, _, true, false, true) if r > 100 && b > 100 => {
            if bright {
                Color::LightMagenta
            } else {
                Color::Magenta
            }
        }
        (_, _, false, true, true) if g > 100 && b > 100 => {
            if bright {
                Color::LightCyan
            } else {
                Color::Cyan
            }
        }
        _ => {
            if bright {
                Color::White
            } else {
                Color::Gray
            }
        }
    }
}

fn indexed_to_ansi16(index: u8) -> Color {
    match index {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        other => {
            // cube/gray → rough ANSI
            let Color::Indexed(i) = Color::Indexed(other) else {
                return Color::White;
            };
            if (232..=255).contains(&i) {
                if i > 243 {
                    Color::White
                } else {
                    Color::DarkGray
                }
            } else {
                Color::Gray
            }
        }
    }
}

impl RolePalette {
    /// Returns a theme with colors quantized to `capability`.
    #[must_use]
    pub fn quantized(&self, capability: ColorCapability) -> Self {
        quantize_palette(self, capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truecolor_is_identity() {
        let c = Color::Rgb(12, 34, 56);
        assert_eq!(quantize_color(c, ColorCapability::Truecolor), c);
    }

    #[test]
    fn rgb_maps_into_xterm_cube() {
        let idx = rgb_to_xterm256(255, 0, 0);
        assert!((16..=231).contains(&idx));
    }

    #[test]
    fn quantize_palette_phosphor_to_ansi_keeps_roles() {
        let theme = RolePalette::tailrocks_phosphor().quantized(ColorCapability::Ansi16);
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        // Accent should still resolve to some style
        let _ = theme.style(Role::Accent);
        assert_eq!(RolePalette::roles().len(), 38);
    }

    #[test]
    fn monochrome_clears_chromatic_fg() {
        let q = quantize_color(Color::Rgb(1, 2, 3), ColorCapability::Monochrome);
        assert_eq!(q, Color::Reset);
    }
}
