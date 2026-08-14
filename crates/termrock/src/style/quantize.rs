// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal color capability ladder and theme quantization.

use ratatui_core::style::{Color, Modifier, Style};

use super::{DesignSystem, GlyphSet, RolePalette, SelectionChrome};

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
        // Chromatic information cannot survive; `quantize_style` compensates by
        // adding `REVERSED` so filled styles keep their shape.
        ColorCapability::Monochrome => Color::Reset,
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

/// Quantizes one style, preserving structure the color ladder can no longer carry.
///
/// Monochrome erases every fill, so a style that *had* a background is given
/// [`Modifier::REVERSED`]: selection, focused actions, and filled badges stay
/// legible as inverted cells instead of vanishing into the canvas.
fn quantize_style(style: Style, capability: ColorCapability) -> Style {
    let mut out = style;
    if let Some(fg) = style.fg {
        out = out.fg(quantize_color(fg, capability));
    }
    if let Some(bg) = style.bg {
        out = out.bg(quantize_color(bg, capability));
    }
    if matches!(capability, ColorCapability::Monochrome)
        && matches!(style.bg, Some(bg) if bg != Color::Reset)
    {
        out = out.add_modifier(Modifier::REVERSED);
    }
    out
}

/// Downgrades interaction chrome a degraded ladder can no longer paint.
///
/// Quantizing only the palette lies about what a projection looks like:
/// monochrome erases every selection fill, and an ASCII terminal has no block
/// glyph to fill with, so selection falls back to the leading gutter mark. This
/// mirrors [`DesignSystem::no_color`] so every projection path agrees.
///
/// The caller sets `capability` (and projects the palette) first.
pub(crate) fn degrade_chrome(system: &mut DesignSystem) {
    if matches!(system.capability, ColorCapability::Monochrome) {
        system.glyphs = GlyphSet::Ascii;
    }
    if matches!(system.capability, ColorCapability::Monochrome)
        || matches!(system.glyphs, GlyphSet::Ascii)
    {
        system.selection = SelectionChrome::Gutter;
    }
}

/// Largest channel spread still treated as "gray enough" for the 232-255 ramp.
///
/// The phosphor surface ladder is a near-neutral green wash (`(18,22,18)` …
/// `(30,38,32)`); without this tolerance every surface floors to cube index 16
/// (pure black) and the whole elevation ladder disappears on 256-color
/// terminals.
const NEAR_GRAY_SPREAD: u8 = 12;

/// xterm 256-color cube + grayscale mapping.
///
/// Exact grays and near-grays (channel spread ≤ 12) use the 232-255 ramp with
/// nearest-step rounding; everything else uses the 6×6×6 cube.
#[must_use]
pub fn rgb_to_xterm256(r: u8, g: u8, b: u8) -> u8 {
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    if hi - lo <= NEAR_GRAY_SPREAD {
        let level = ((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8;
        return gray_ramp_index(level);
    }
    let ri = (u16::from(r) * 5 / 255) as u8;
    let gi = (u16::from(g) * 5 / 255) as u8;
    let bi = (u16::from(b) * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

/// Nearest xterm index for a neutral level (`16`/`231` at the ends).
///
/// Ramp entries are `8 + 10 * n` for `n` in `0..24` (indices 232-255).
const fn gray_ramp_index(level: u8) -> u8 {
    if level < 4 {
        // Closer to cube black than to the ramp's first step.
        return 16;
    }
    if level >= 247 {
        // Closer to cube white (255) than to the ramp's last step (238).
        return 231;
    }
    let step = (level as u16 + 5 - 8) / 10;
    let step = if step > 23 { 23 } else { step };
    232 + step as u8
}

/// xterm 256-color index back to RGB (cube levels + gray ramp).
const fn xterm256_to_rgb(index: u8) -> (u8, u8, u8) {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if index < 16 {
        // Handled by the exact ANSI-16 table; return black as a neutral stand-in.
        return (0, 0, 0);
    }
    if index >= 232 {
        let level = 8 + (index - 232) * 10;
        return (level, level, level);
    }
    let i = index - 16;
    (
        CUBE[(i / 36) as usize],
        CUBE[((i % 36) / 6) as usize],
        CUBE[(i % 6) as usize],
    )
}

/// Channel spread below which a color has no hue worth keeping.
const NEUTRAL_CHROMA: u8 = 24;
/// Peak channel below which no chromatic ANSI slot is dark enough to be honest.
///
/// ANSI `Green` is `(0,205,0)`; painting a `(20,51,26)` selection tint with it
/// would turn a whisper into a shout, so very dark colors take the gray ladder.
const CHROMATIC_FLOOR: u8 = 64;
/// Peak channel at or above which the bright half of the palette is used.
const BRIGHT_FLOOR: u8 = 200;

/// Nearest ANSI-16 color for an RGB value — hue first, brightness second.
///
/// A plain nearest-RGB search is *worse* than useless here: pastel semantic
/// hues (danger `(255,94,122)`) sit closer to mid-gray than to their own primary
/// in RGB space, so "nearest" answers `DarkGray` and the warning stops looking
/// like a warning. Matching the hue sector and then choosing the bright or dim
/// half keeps meaning intact, which is the whole job of the ladder.
fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Color {
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    let chroma = hi - lo;
    if chroma <= NEUTRAL_CHROMA || hi < CHROMATIC_FLOOR {
        let level = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
        return match level {
            0..48 => Color::Black,
            48..128 => Color::DarkGray,
            128..208 => Color::Gray,
            _ => Color::White,
        };
    }
    // A channel is "on" when it carries at least half of the chroma.
    let on = |channel: u8| u16::from(channel - lo) * 2 >= u16::from(chroma);
    let bright = hi >= BRIGHT_FLOOR;
    match (on(r), on(g), on(b)) {
        (true, false, false) if bright => Color::LightRed,
        (true, false, false) => Color::Red,
        (true, true, false) if bright => Color::LightYellow,
        (true, true, false) => Color::Yellow,
        (false, true, false) if bright => Color::LightGreen,
        (false, true, false) => Color::Green,
        (false, true, true) if bright => Color::LightCyan,
        (false, true, true) => Color::Cyan,
        (false, false, true) if bright => Color::LightBlue,
        (false, false, true) => Color::Blue,
        (true, false, true) if bright => Color::LightMagenta,
        (true, false, true) => Color::Magenta,
        // All-on / all-off cannot happen for `chroma > NEUTRAL_CHROMA`.
        (_, _, _) if bright => Color::White,
        (_, _, _) => Color::Gray,
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
        // Cube / gray-ramp entries round-trip through RGB so they land on the
        // same hue the truecolor source would have picked.
        other => {
            let (r, g, b) = xterm256_to_rgb(other);
            rgb_to_ansi16(r, g, b)
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
    use crate::style::Role;

    fn bg_of(palette: &RolePalette, role: Role) -> Color {
        palette.style(role).bg.expect("surface role paints a bg")
    }

    /// Palette hues pinned at plan-003 execution time (plan 002 had not landed;
    /// its STOP rule says pin the values that exist at HEAD).
    #[test]
    fn ansi16_maps_semantic_hues_to_distinct_colors() {
        let cases = [
            ((0, 255, 65), Color::LightGreen, "phosphor accent"),
            ((255, 94, 122), Color::LightRed, "danger"),
            ((255, 216, 94), Color::LightYellow, "warning"),
            ((0, 180, 180), Color::Cyan, "info"),
            ((61, 220, 90), Color::LightGreen, "success mint"),
            ((80, 80, 80), Color::DarkGray, "border graphite"),
            ((255, 255, 255), Color::White, "text"),
            ((10, 12, 10), Color::Black, "canvas"),
        ];
        for ((r, g, b), expected, what) in cases {
            assert_eq!(rgb_to_ansi16(r, g, b), expected, "{what}");
        }
    }

    #[test]
    fn ansi16_keeps_phosphor_status_hues_separable() {
        let palette = RolePalette::tailrocks_phosphor().quantized(ColorCapability::Ansi16);
        let hues = [Role::Accent, Role::Danger, Role::Warning, Role::Info]
            .map(|role| palette.style(role).fg.expect("status role paints a fg"));
        for (i, hue) in hues.iter().enumerate() {
            assert!(
                !matches!(hue, Color::White | Color::Gray),
                "status hue {i} collapsed to neutral: {hue:?}"
            );
            for other in &hues[i + 1..] {
                assert_ne!(hue, other, "status hues collided");
            }
        }
    }

    /// The 232-255 ramp steps by 10 while the phosphor ladder steps by ~5, so
    /// `Sunken`/`Surface` share a step; the *elevation* direction still survives.
    #[test]
    fn surface_ladder_survives_256_quantization() {
        let palette = RolePalette::tailrocks_phosphor().quantized(ColorCapability::Indexed256);
        let index = |role| match bg_of(&palette, role) {
            Color::Indexed(i) => i,
            other => panic!("{role:?} did not quantize to an index: {other:?}"),
        };
        let canvas = index(Role::Canvas);
        let surface = index(Role::Surface);
        let raised = index(Role::Raised);
        let elevated = index(Role::Elevated);
        let sunken = index(Role::Sunken);
        assert!(
            canvas < surface && surface < raised && raised < elevated,
            "elevation flattened: {canvas} {surface} {raised} {elevated}"
        );
        assert!(sunken <= surface, "sunken rose above surface");
        for index in [canvas, surface, raised, elevated, sunken] {
            assert!(index >= 232, "surface fell back to the cube: {index}");
        }
    }

    #[test]
    fn near_gray_uses_the_ramp_not_the_cube() {
        assert_eq!(rgb_to_xterm256(0, 0, 0), 16);
        assert_eq!(rgb_to_xterm256(255, 255, 255), 231);
        assert_eq!(rgb_to_xterm256(18, 22, 18), 233);
        // Saturated colors still take the cube.
        assert!((16..=231).contains(&rgb_to_xterm256(255, 0, 0)));
    }

    #[test]
    fn monochrome_reverses_filled_styles() {
        let palette = RolePalette::tailrocks_phosphor().quantized(ColorCapability::Monochrome);
        for role in [Role::Selection, Role::ActionFocused] {
            assert!(
                palette
                    .style(role)
                    .add_modifier
                    .contains(Modifier::REVERSED),
                "{role:?} lost its fill without a REVERSED substitute"
            );
        }
        assert!(
            !palette
                .style(Role::Text)
                .add_modifier
                .contains(Modifier::REVERSED),
            "unfilled text should not invert"
        );
    }

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
        assert!(theme.style(Role::Accent).fg.is_some());
        assert_eq!(RolePalette::roles().len(), crate::style::ROLE_COUNT);
    }

    #[test]
    fn monochrome_clears_chromatic_fg() {
        let q = quantize_color(Color::Rgb(1, 2, 3), ColorCapability::Monochrome);
        assert_eq!(q, Color::Reset);
    }
}
