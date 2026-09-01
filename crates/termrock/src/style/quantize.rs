// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal color capability ladder and theme quantization.

use ratatui_core::style::{Color, Modifier, Style};

use super::{DesignSystem, GlyphSet, Role, RolePalette, SelectionChrome};

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

/// The named ANSI-16 palette, independent of terminal-specific RGB values.
///
/// Recipes targeting [`ColorCapability::Ansi16`] resolve through these names;
/// they never smuggle truecolor values into a nominal 16-color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ansi16Color {
    /// Base black.
    Black,
    /// Base red.
    Red,
    /// Base green.
    Green,
    /// Base yellow.
    Yellow,
    /// Base blue.
    Blue,
    /// Base magenta.
    Magenta,
    /// Base cyan.
    Cyan,
    /// Base light neutral (ANSI white slot).
    Gray,
    /// Bright black / dark neutral.
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
    /// Bright white.
    White,
}

impl Ansi16Color {
    /// Every named ANSI color in terminal index order.
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

    /// Ratatui's native named color for this ANSI slot.
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

impl From<Ansi16Color> for Color {
    fn from(value: Ansi16Color) -> Self {
        value.color()
    }
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
            Color::Rgb(r, g, b) => rgb_to_ansi16(r, g, b).color(),
            Color::Indexed(i) => indexed_to_ansi16(i).color(),
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
    let quantized = RolePalette::from_fn(|role| quantize_style(palette.style(role), capability));
    if matches!(capability, ColorCapability::Indexed256) {
        return separate_elevation(quantized);
    }
    quantized
}

/// Elevation tiers, deepest first: the order the ladder has to preserve.
const ELEVATION_LADDER: [Role; 5] = [
    Role::Canvas,
    Role::Sunken,
    Role::Surface,
    Role::Raised,
    Role::Elevated,
];

/// Pushes colliding elevation tiers apart on the 256-colour gray ramp.
///
/// Elevation is a *hierarchy*, not five unrelated colours, and quantizing each
/// role on its own loses that: the ramp steps by 10 while the phosphor ladder
/// steps by about 5, so `Sunken` (13,16,13) and `Surface` (18,22,18) both round
/// to index 233 and a sunken well becomes invisible — including every text
/// input, which paints its trough with `Role::Sunken`.
///
/// Nearest-colour is the right answer for one colour and the wrong answer for a
/// ladder. Resolving the tiers in order and stepping a tie up by one keeps the
/// rungs the roles exist to express, which is the same reasoning that already
/// cuts the ANSI-16 neutral bands where the hierarchy breaks rather than where
/// the colours are nearest (plans/003, plans/020).
fn separate_elevation(palette: RolePalette) -> RolePalette {
    /// Highest index on the xterm gray ramp.
    const RAMP_TOP: u8 = 255;
    let mut out = palette;
    let mut floor: Option<u8> = None;
    for role in ELEVATION_LADDER {
        let style = out.style(role);
        let Some(Color::Indexed(index)) = style.bg else {
            continue;
        };
        // Only the gray ramp collides this way; a tier that landed in the
        // colour cube is already distinct from its neighbours.
        if index < 232 {
            floor = None;
            continue;
        }
        let lifted = match floor {
            Some(previous) if index <= previous => previous.saturating_add(1).min(RAMP_TOP),
            _ => index,
        };
        floor = Some(lifted);
        if lifted != index {
            out = out.with_role(role, style.bg(Color::Indexed(lifted)));
        }
    }
    out
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

// Four ANSI neutrals have to carry a six-tier design ladder (canvas, border,
// disabled, muted, body, strong), so the bands are cut where the *hierarchy*
// breaks rather than where the colors are nearest. True nearest-neighbor over
// {0, 127, 229, 255} would put the graphite border at Black — invisible chrome
// on a black canvas — and a 128 midpoint would drop muted text into the same
// DarkGray as that border, collapsing metadata into chrome. These three cuts
// keep canvas, chrome, secondary text, and body text on four distinct rungs.
/// Below this, a neutral is canvas or a deep surface.
const NEUTRAL_CHROME: u16 = 48;
/// Below this, a neutral is border or disabled chrome; above it, secondary text.
const NEUTRAL_MUTED: u16 = 120;
/// At or above this, a neutral is body or strong text.
const NEUTRAL_BODY: u16 = 208;
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
fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Ansi16Color {
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    let chroma = hi - lo;
    if chroma <= NEUTRAL_CHROMA || hi < CHROMATIC_FLOOR {
        let level = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
        return match level {
            0..NEUTRAL_CHROME => Ansi16Color::Black,
            NEUTRAL_CHROME..NEUTRAL_MUTED => Ansi16Color::DarkGray,
            NEUTRAL_MUTED..NEUTRAL_BODY => Ansi16Color::Gray,
            _ => Ansi16Color::White,
        };
    }
    // A channel is "on" when it carries at least half of the chroma.
    let on = |channel: u8| u16::from(channel - lo) * 2 >= u16::from(chroma);
    let bright = hi >= BRIGHT_FLOOR;
    match (on(r), on(g), on(b)) {
        (true, false, false) if bright => Ansi16Color::LightRed,
        (true, false, false) => Ansi16Color::Red,
        (true, true, false) if bright => Ansi16Color::LightYellow,
        (true, true, false) => Ansi16Color::Yellow,
        (false, true, false) if bright => Ansi16Color::LightGreen,
        (false, true, false) => Ansi16Color::Green,
        (false, true, true) if bright => Ansi16Color::LightCyan,
        (false, true, true) => Ansi16Color::Cyan,
        (false, false, true) if bright => Ansi16Color::LightBlue,
        (false, false, true) => Ansi16Color::Blue,
        (true, false, true) if bright => Ansi16Color::LightMagenta,
        (true, false, true) => Ansi16Color::Magenta,
        // All-on / all-off cannot happen for `chroma > NEUTRAL_CHROMA`.
        (_, _, _) if bright => Ansi16Color::White,
        (_, _, _) => Ansi16Color::Gray,
    }
}

fn indexed_to_ansi16(index: u8) -> Ansi16Color {
    match index {
        0..=15 => Ansi16Color::ALL[usize::from(index)],
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

    fn rgb_elevation_reference() -> RolePalette {
        RolePalette::from_fn(|_| Style::new())
            .with_role(Role::Canvas, Style::new().bg(Color::Rgb(10, 12, 10)))
            .with_role(Role::Sunken, Style::new().bg(Color::Rgb(13, 16, 13)))
            .with_role(Role::Input, Style::new().bg(Color::Rgb(13, 16, 13)))
            .with_role(Role::Surface, Style::new().bg(Color::Rgb(18, 22, 18)))
            .with_role(Role::Raised, Style::new().bg(Color::Rgb(26, 31, 26)))
            .with_role(Role::Elevated, Style::new().bg(Color::Rgb(30, 38, 32)))
    }

    /// The graphite palette's actual hues, retuned after plan 002 landed.
    ///
    /// These literals are the point: they say what the *shipping* palette does
    /// on a 16-color terminal, so a future revalue that quietly pushes a hue
    /// across a sector boundary fails here rather than in someone's terminal.
    #[test]
    fn ansi16_maps_semantic_hues_to_distinct_colors() {
        let cases = [
            ((0, 255, 65), Color::LightGreen, "phosphor accent"),
            ((255, 94, 122), Color::LightRed, "danger"),
            ((255, 216, 94), Color::LightYellow, "warning"),
            ((0, 180, 180), Color::Cyan, "info"),
            ((93, 255, 160), Color::LightGreen, "success mint"),
            ((48, 58, 50), Color::DarkGray, "border graphite"),
            ((214, 224, 214), Color::White, "body text"),
            ((122, 138, 122), Color::Gray, "muted text"),
            ((10, 12, 10), Color::Black, "canvas"),
        ];
        for ((r, g, b), expected, what) in cases {
            assert_eq!(rgb_to_ansi16(r, g, b).color(), expected, "{what}");
        }
    }

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

    /// The text ladder is a hierarchy channel; ANSI-16 must not flatten it into
    /// the chrome it sits on.
    #[test]
    fn ansi16_keeps_text_above_the_chrome_it_sits_on() {
        let palette = RolePalette::tailrocks_phosphor().quantized(ColorCapability::Ansi16);
        let body = palette.style(Role::Text);
        let muted = palette.style(Role::TextMuted);
        let border = palette.style(Role::Border);

        assert_ne!(body, muted, "body and secondary text collapsed to one tone");
        assert_ne!(
            muted, border,
            "secondary text collapsed into border chrome — metadata becomes chrome"
        );
        assert_ne!(
            border.fg,
            Some(Color::Black),
            "border vanished into a black canvas"
        );
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
    /// nearest-colour rounding used to put `Sunken` and `Surface` on one step
    /// and make every input trough invisible. The ladder is quantized as an
    /// ordered family, so all five tiers keep their own rung (plans/003).
    #[test]
    fn surface_ladder_survives_256_quantization() {
        let palette = rgb_elevation_reference().quantized(ColorCapability::Indexed256);
        let index = |role| match bg_of(&palette, role) {
            Color::Indexed(i) => i,
            other => panic!("{role:?} did not quantize to an index: {other:?}"),
        };
        let canvas = index(Role::Canvas);
        let sunken = index(Role::Sunken);
        let surface = index(Role::Surface);
        let raised = index(Role::Raised);
        let elevated = index(Role::Elevated);
        assert!(
            canvas < sunken && sunken < surface && surface < raised && raised < elevated,
            "elevation flattened: {canvas} {sunken} {surface} {raised} {elevated}"
        );
        for index in [canvas, surface, raised, elevated, sunken] {
            assert!(index >= 232, "surface fell back to the cube: {index}");
        }
    }

    /// A well the operator types into must be visible on a 256-colour terminal.
    #[test]
    fn an_input_trough_is_visible_at_256_colours() {
        let palette = rgb_elevation_reference().quantized(ColorCapability::Indexed256);
        assert_ne!(
            bg_of(&palette, Role::Input),
            bg_of(&palette, Role::Surface),
            "a field well that matches the surface behind it is not a well"
        );
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
