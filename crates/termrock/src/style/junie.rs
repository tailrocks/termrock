// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical junie design tokens — a verbatim port of the reference
//! `junie-tui` `src/theme.rs`.
//!
//! This module is the only source of colour truth in TermRock. Rendering code
//! never spells out an RGB value; every surface reaches a token through
//! [`JunieTheme`] (or the [`crate::style::RolePalette`] built from it).
//!
//! The port keeps the reference's exact structure so a diff against upstream
//! stays reviewable:
//!
//! - [`palette`] — the raw hex swatches.
//! - [`JunieTheme`] — the 24 active semantic tokens plus every style resolver.
//! - [`downgrade`] / [`nearest_256`] / [`nearest_16`] / [`mono`] — the
//!   capability ladder, applied to *every* token rather than per call site.
//!
//! Dormant reference tokens (`accent_bg_subtle`, `error_bg`, `info`) are NOT
//! ported: the reference never reads them from a resolver, and TermRock has no
//! `#8787ff` anywhere in the repo.
//!
//! Colour capability is expressed with TermRock's [`ColorCapability`]
//! (`Truecolor`/`Indexed256`/`Ansi16`/`Monochrome`) instead of the reference's
//! `ColorLevel`; the four rungs map one-to-one.
use ratatui_core::style::{Color, Modifier, Style};

use super::quantize::ColorCapability;

/// Raw palette. Values are the Junie references (jetbrains.com/junie computed
/// styles): canvas `#000000`, chrome `#111111`, cards `#18181b` (zinc-900),
/// input `#1e1e22`, popover `#3f3f46` (zinc-700); accent `#48e054` with hover
/// at 80% over black and 10–20% alpha tints; text is an alpha ladder on white;
/// borders are white at 10% (subtle) and 30% (strong); anchored-menu
/// highlights are blue/deep-red and destructive menu labels use soft rose;
/// destructive `#e44545`, warning `#f59e09`.
pub mod palette {
    use ratatui_core::style::Color;

    /// Builds a truecolor token from a packed hex value.
    pub const fn rgb(hex: u32) -> Color {
        Color::Rgb(
            ((hex >> 16) & 0xff) as u8,
            ((hex >> 8) & 0xff) as u8,
            (hex & 0xff) as u8,
        )
    }

    /// Canvas / terminal ground.
    pub const BLACK: Color = rgb(0x000000);
    /// Chrome plane (ordinary surface).
    pub const CHROME: Color = rgb(0x111111);
    /// Card / elevated surface (zinc-900).
    pub const CARD: Color = rgb(0x18181b);
    /// Input field body.
    pub const INPUT: Color = rgb(0x1e1e22);
    /// Input field body while hovered.
    pub const INPUT_HOVER: Color = rgb(0x232328);
    /// Overlay surface.
    pub const OVERLAY: Color = rgb(0x27272a);
    /// Popover / dialog surface (zinc-700).
    pub const POPOVER: Color = rgb(0x3f3f46);
    /// Cursor-row fill for ordinary anchored-menu commands.
    pub const HIGHLIGHT: Color = rgb(0x2f5aa8);
    /// Cursor-row fill for destructive anchored-menu commands.
    pub const HIGHLIGHT_DANGER: Color = rgb(0x7a2a2a);
    /// Resting destructive anchored-menu label.
    pub const ERROR_SOFT: Color = rgb(0xd98a8a);
    /// Text primary — 100% white.
    pub const WHITE: Color = rgb(0xffffff);
    /// Text secondary — 70% white.
    pub const WHITE_70: Color = rgb(0xb3b3b3);
    /// Text muted — 50% white.
    pub const WHITE_50: Color = rgb(0x808080);
    /// Text faint, border strong, disabled — 30% white.
    pub const WHITE_30: Color = rgb(0x4d4d4d);
    /// Text ghost, border subtle — 15% white.
    pub const WHITE_15: Color = rgb(0x262626);
    /// Accent / focus / success green.
    pub const GREEN: Color = rgb(0x48e054);
    /// Accent hover — accent at 80% over black.
    pub const GREEN_80: Color = rgb(0x3ab343);
    /// Accent pressed — accent at 60% over black.
    pub const GREEN_60: Color = rgb(0x2b8632);
    /// Accent background tint (selection) — accent at 20% alpha.
    pub const GREEN_20: Color = rgb(0x0f2e13);
    /// Text on accent fill.
    pub const ON_GREEN: Color = rgb(0x19191c);
    /// Error / destructive (red-400).
    pub const RED: Color = rgb(0xe44545);
    /// Warning (amber).
    pub const AMBER: Color = rgb(0xf59e09);
}

/// Semantic tokens. Field names are the vocabulary used everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JunieTheme {
    /// Colour capability the tokens were resolved for.
    pub level: ColorCapability,

    /// Terminal ground.
    pub canvas: Color,
    /// Ordinary component surface.
    pub surface: Color,
    /// Card / elevated surface.
    pub surface_elevated: Color,
    /// Overlay surface.
    pub surface_overlay: Color,
    /// Text field body.
    pub field: Color,
    /// Text field body while hovered.
    pub field_hover: Color,
    /// Popover / dialog surface.
    pub popover: Color,
    /// Cursor-row fill for ordinary anchored-menu commands.
    pub highlight: Color,
    /// Cursor-row fill for destructive anchored-menu commands.
    pub highlight_danger: Color,
    /// Resting destructive anchored-menu label.
    pub error_soft: Color,

    /// Resting boundary.
    pub border_subtle: Color,
    /// Boundary of the surface that owns interaction.
    pub border_strong: Color,

    /// Body text.
    pub text_primary: Color,
    /// Secondary explanatory text.
    pub text_secondary: Color,
    /// Metadata text.
    pub text_muted: Color,
    /// Faint meta text.
    pub text_faint: Color,
    /// One step below faint: dimmed backdrops only.
    pub text_ghost: Color,
    /// Text painted on an accent fill.
    pub text_on_accent: Color,

    /// Brand / intent accent.
    pub accent: Color,
    /// Accent while hovered.
    pub accent_hover: Color,
    /// Accent while pressed.
    pub accent_pressed: Color,
    /// Accent background tint (selection).
    pub accent_bg: Color,
    /// Focus colour (same green as [`Self::accent`]).
    pub focus: Color,

    /// Unavailable control text.
    pub disabled: Color,
    /// Error / destructive.
    pub error: Color,
    /// Caution.
    pub warning: Color,
    /// Successful / completed.
    pub success: Color,
}

impl JunieTheme {
    /// The junie theme at truecolor.
    pub const fn junie() -> Self {
        use palette::*;
        Self {
            level: ColorCapability::Truecolor,
            canvas: BLACK,
            surface: CHROME,
            surface_elevated: CARD,
            surface_overlay: OVERLAY,
            field: INPUT,
            field_hover: INPUT_HOVER,
            popover: POPOVER,
            highlight: HIGHLIGHT,
            highlight_danger: HIGHLIGHT_DANGER,
            error_soft: ERROR_SOFT,
            border_subtle: WHITE_15,
            border_strong: WHITE_30,
            text_primary: WHITE,
            text_secondary: WHITE_70,
            text_muted: WHITE_50,
            text_faint: WHITE_30,
            text_ghost: WHITE_15,
            text_on_accent: ON_GREEN,
            accent: GREEN,
            accent_hover: GREEN_80,
            accent_pressed: GREEN_60,
            accent_bg: GREEN_20,
            focus: GREEN,
            disabled: WHITE_30,
            error: RED,
            warning: AMBER,
            success: GREEN,
        }
    }

    /// Junie theme resolved for the given colour capability.
    ///
    /// Every token is run through [`downgrade`]; there is no per-call-site
    /// quantization anywhere else in the crate.
    #[must_use]
    pub fn for_level(level: ColorCapability) -> Self {
        let mut t = Self::junie();
        t.level = level;
        if level == ColorCapability::Truecolor {
            return t;
        }
        macro_rules! map {
            ($($f:ident),*) => { $( t.$f = downgrade(t.$f, level); )* };
        }
        map!(
            canvas,
            surface,
            surface_elevated,
            surface_overlay,
            field,
            field_hover,
            popover,
            highlight,
            highlight_danger,
            error_soft,
            border_subtle,
            border_strong,
            text_primary,
            text_secondary,
            text_muted,
            text_faint,
            text_ghost,
            text_on_accent,
            accent,
            accent_hover,
            accent_pressed,
            accent_bg,
            focus,
            disabled,
            error,
            warning,
            success
        );
        t
    }

    // --- base styles -------------------------------------------------------

    /// Body text on the canvas.
    pub fn base(&self) -> Style {
        Style::new().fg(self.text_primary).bg(self.canvas)
    }

    /// Body text on an explicit ground.
    pub fn on(&self, bg: Color) -> Style {
        Style::new().fg(self.text_primary).bg(bg)
    }

    /// Body text, transparent ground.
    pub fn primary(&self) -> Style {
        Style::new().fg(self.text_primary)
    }

    /// Secondary text.
    pub fn secondary(&self) -> Style {
        Style::new().fg(self.text_secondary)
    }

    /// Muted text.
    pub fn muted(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// Faint text.
    pub fn faint(&self) -> Style {
        Style::new().fg(self.text_faint)
    }

    /// Accent as a foreground (never a fill).
    pub fn accent_fg(&self) -> Style {
        Style::new().fg(self.accent)
    }

    /// Error as a foreground.
    pub fn error_fg(&self) -> Style {
        Style::new().fg(self.error)
    }

    /// Title / heading: body text plus weight.
    pub fn title(&self) -> Style {
        Style::new()
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Field label: weight when the field owns focus, secondary otherwise.
    pub fn label(&self, focused: bool) -> Style {
        if focused {
            self.title()
        } else {
            self.secondary()
        }
    }

    /// Key chord in an interaction hint.
    pub fn key_hint_key(&self) -> Style {
        Style::new()
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Action label paired with a hint key.
    pub fn key_hint_action(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// Boundary style; the interaction owner's boundary is the strong one.
    pub fn border(&self, focused: bool) -> Style {
        Style::new().fg(if focused {
            self.border_strong
        } else {
            self.border_subtle
        })
    }

    /// Style for a backdrop cell under a modal: surfaces stay so the page
    /// keeps its shape, every colour collapses to the faint text tier, and
    /// any coloured fill (accent, error, selection tint, reversed cursor)
    /// drops to a neutral overlay.
    pub fn backdrop(&self, style: Style) -> Style {
        let bg = match style.bg {
            Some(c) if c == self.canvas || c == self.surface || c == self.surface_elevated => c,
            Some(c) if c == self.field || c == self.field_hover => self.surface_elevated,
            Some(_) => self.surface_overlay,
            None => self.canvas,
        };
        // scale the alpha ladder instead of collapsing it: hierarchy survives
        let fg = match style.fg {
            // a glyph painted in its own background is a hidden gutter: keep it hidden
            Some(c) if Some(c) == style.bg => bg,
            Some(c) if c == self.canvas || c == self.surface => bg,
            Some(c)
                if c == self.text_primary
                    || c == self.accent
                    || c == self.error
                    || c == self.warning =>
            {
                self.text_muted
            }
            Some(c) if c == self.text_secondary || c == self.text_on_accent => self.text_faint,
            _ => self.text_ghost,
        };
        Style::new().fg(fg).bg(bg)
    }

    // --- component resolvers ----------------------------------------------
    //
    // All resolvers take the container background so a control looks right
    // on the canvas, on a surface, or inside a dialog.

    /// Row-like control (nav item, list item, table row, tree node).
    pub fn row(&self, s: VisualState, bg: Color) -> Style {
        if s.disabled {
            return Style::new().fg(self.disabled).bg(bg);
        }
        let mut st = Style::new().fg(self.text_primary).bg(bg);
        // selection tint only where the keyboard is (focused row); elsewhere
        // the marker glyph alone carries "selected"
        if s.selected && s.focused {
            st = st.bg(self.accent_bg);
        }
        // hover is always exactly one plane up, never a colour
        if s.hovered {
            st = st.bg(self.lift(bg));
        }
        if s.error {
            st = st.fg(self.error);
        }
        if s.busy {
            st = st.fg(self.text_secondary);
        }
        if s.focused {
            st = st.add_modifier(Modifier::BOLD);
        }
        if s.pressed {
            st = self.reversed();
        }
        st
    }

    /// One step lighter than `bg`, used for hover.
    pub fn lift(&self, bg: Color) -> Color {
        if bg == self.canvas {
            self.surface_elevated
        } else if bg == self.surface || bg == self.surface_elevated {
            self.surface_overlay
        } else if bg == self.field {
            self.field_hover
        } else {
            self.popover
        }
    }

    /// Focus gutter glyph style. `on_accent` is used when the control itself
    /// is filled with the accent (primary button).
    pub fn gutter(&self, s: VisualState, bg: Color, on_accent: bool) -> Style {
        let fg = if !s.focused {
            bg
        } else if on_accent {
            self.text_primary
        } else {
            self.focus
        };
        // Colour-only, matching junie `theme.rs` `gutter()`. Weight arrives
        // from the row/control fill merging `add_modifier` into the cell.
        // Primary fill (`on_accent`) strips BOLD so idle `▎` on accent is
        // colour only.
        let st = Style::new().fg(fg).bg(bg);
        if on_accent {
            st.remove_modifier(Modifier::BOLD)
        } else {
            st
        }
    }

    /// Commit-control paint by kind and interaction state.
    pub fn button(&self, kind: ButtonKind, s: VisualState, bg: Color) -> Style {
        if s.disabled {
            return Style::new()
                .fg(self.disabled)
                .bg(if kind == ButtonKind::Subtle {
                    bg
                } else {
                    self.lift(bg)
                });
        }
        match kind {
            ButtonKind::Primary => {
                let b = if s.pressed {
                    self.accent_pressed
                } else if s.hovered {
                    self.accent_hover
                } else {
                    self.accent
                };
                Style::new()
                    .fg(self.text_on_accent)
                    .bg(b)
                    .add_modifier(Modifier::BOLD)
            }
            ButtonKind::Secondary | ButtonKind::Toggle => {
                let mut st = Style::new().fg(self.text_primary).bg(self.surface_overlay);
                if s.hovered {
                    st = st.bg(self.popover);
                }
                if s.focused {
                    st = st.add_modifier(Modifier::BOLD);
                }
                if s.pressed {
                    st = Style::new().fg(self.canvas).bg(self.text_primary);
                }
                st
            }
            ButtonKind::Subtle => {
                let mut st = Style::new().fg(self.text_secondary).bg(bg);
                if s.hovered {
                    st = st.fg(self.text_primary).bg(self.lift(bg));
                }
                if s.focused {
                    st = st.fg(self.text_primary).add_modifier(Modifier::BOLD);
                }
                if s.pressed {
                    st = Style::new().fg(self.canvas).bg(self.text_primary);
                }
                st
            }
            ButtonKind::Danger => {
                let mut st = Style::new().fg(self.error).bg(self.surface_overlay);
                if s.hovered {
                    st = st.bg(self.popover);
                }
                if s.focused {
                    st = st.add_modifier(Modifier::BOLD);
                }
                if s.pressed {
                    st = Style::new().fg(self.text_primary).bg(self.error);
                }
                st
            }
        }
    }

    /// Text field body (input, textarea, editable cell).
    pub fn field_style(&self, s: VisualState) -> Style {
        if s.disabled {
            return Style::new().fg(self.disabled).bg(self.field);
        }
        let bg = if s.hovered && !s.editing {
            self.field_hover
        } else {
            self.field
        };
        Style::new().fg(self.text_primary).bg(bg)
    }

    /// Placeholder inside a field body.
    pub fn placeholder(&self, s: VisualState) -> Style {
        self.field_style(s).fg(if s.disabled {
            self.disabled
        } else {
            self.text_muted
        })
    }

    /// Selected text / range.
    pub fn selection(&self) -> Style {
        Style::new().fg(self.text_primary).bg(self.popover)
    }

    /// The one inverted cell in the system, written out in full.
    ///
    /// A cell cursor, a pressed row, or the pressed face of a control is
    /// `fg(canvas).bg(text_primary)` plus weight. `Modifier::REVERSED` is
    /// banned: it swaps whatever pair the cell already carried, so a cursor
    /// drawn over an error tone inverts into an invisible cell.
    pub fn reversed(&self) -> Style {
        Style::new()
            .fg(self.canvas)
            .bg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Unoccupied scrollbar track.
    pub fn scrollbar_track(&self) -> Style {
        Style::new().fg(self.border_subtle)
    }

    /// Scrollbar thumb; the interaction owner's thumb is the brightest.
    pub fn scrollbar_thumb(&self, focused: bool, hovered: bool) -> Style {
        Style::new().fg(if focused {
            self.text_primary
        } else if hovered {
            self.text_secondary
        } else {
            self.text_muted
        })
    }

    /// Text tone for values, segments, cells. Maps to the alpha ladder plus
    /// the three semantic colours; never to the accent.
    pub const fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Normal => self.text_primary,
            Tone::Secondary => self.text_secondary,
            Tone::Muted => self.text_muted,
            Tone::Faint => self.text_faint,
            Tone::Error => self.error,
            Tone::Warning => self.warning,
            Tone::Success => self.success,
        }
    }

    /// Restrained syntax palette: structure through weight and the text
    /// ladder, not hue.
    pub fn syntax(&self, tone: SyntaxTone) -> Style {
        match tone {
            SyntaxTone::Keyword => Style::new()
                .fg(self.text_primary)
                .add_modifier(Modifier::BOLD),
            SyntaxTone::Ident | SyntaxTone::Plain => Style::new().fg(self.text_primary),
            SyntaxTone::Str => Style::new().fg(self.text_secondary),
            SyntaxTone::Number => Style::new().fg(self.text_secondary),
            SyntaxTone::Operator | SyntaxTone::Punct => Style::new().fg(self.text_muted),
            SyntaxTone::Comment => Style::new()
                .fg(self.text_faint)
                .add_modifier(Modifier::ITALIC),
        }
    }

    /// Badge paint. [`BadgeKind::Edit`] is the only badge in the system.
    pub fn badge(&self, kind: BadgeKind) -> Style {
        match kind {
            BadgeKind::Edit => Style::new()
                .fg(self.text_on_accent)
                .bg(self.accent)
                .add_modifier(Modifier::BOLD),
        }
    }
}

/// Per-control interaction facts the resolvers read.
///
/// Mirrors the reference `ui::ctx::VisualState` field for field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VisualState {
    /// The control owns the keyboard.
    pub focused: bool,
    /// The pointer is over the control.
    pub hovered: bool,
    /// The pointer is holding the control down.
    pub pressed: bool,
    /// The control is the cursor / is selected.
    pub selected: bool,
    /// The control cannot be interacted with.
    pub disabled: bool,
    /// The control is in an error state.
    pub error: bool,
    /// The control is being edited in place.
    pub editing: bool,
    /// Work is in progress on the control.
    pub busy: bool,
}

/// Text tone for values, segments, and cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tone {
    /// Body text.
    #[default]
    Normal,
    /// Secondary text.
    Secondary,
    /// Metadata text.
    Muted,
    /// Faint text.
    Faint,
    /// Error text.
    Error,
    /// Warning text.
    Warning,
    /// Success text.
    Success,
}

/// Language-agnostic syntax classes for the code editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxTone {
    /// Language keyword.
    Keyword,
    /// Identifier.
    Ident,
    /// Numeric literal.
    Number,
    /// String literal.
    Str,
    /// Operator.
    Operator,
    /// Punctuation.
    Punct,
    /// Comment.
    Comment,
    /// Anything unclassified.
    Plain,
}

/// Commit-control vocabulary for [`JunieTheme::button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonKind {
    /// The one primary commit action.
    Primary,
    /// Ordinary action.
    Secondary,
    /// Quiet text-like action.
    Subtle,
    /// Destructive action.
    Danger,
    /// Stateful switch; shares secondary colours.
    Toggle,
}

/// Badge vocabulary. `EDIT` is the only badge in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BadgeKind {
    /// The ` EDIT ` badge shown while a control is being edited.
    Edit,
}

/// Downgrades one truecolor token to a colour capability.
///
/// Named and indexed colours pass through unchanged — they are already
/// resolved by the operator's terminal.
pub fn downgrade(c: Color, level: ColorCapability) -> Color {
    let Color::Rgb(r, g, b) = c else { return c };
    match level {
        ColorCapability::Truecolor => c,
        ColorCapability::Indexed256 => Color::Indexed(nearest_256(r, g, b)),
        ColorCapability::Ansi16 => nearest_16(r, g, b),
        ColorCapability::Monochrome => match mono_level(r, g, b) {
            0 => Color::Black,
            1 => Color::DarkGray,
            2 => Color::Gray,
            _ => Color::White,
        },
    }
}

/// Four-bucket grey collapse used by the monochrome rung.
fn mono_level(r: u8, g: u8, b: u8) -> u8 {
    match (u32::from(r) + u32::from(g) + u32::from(b)) / 3 {
        0..=40 => 0,
        41..=110 => 1,
        111..=190 => 2,
        _ => 3,
    }
}

/// Nearest xterm-256 index: colour cube or the 232–255 grey ramp, whichever
/// is actually closer in RGB space.
pub fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    let step = |v: u8| -> u8 { ((u32::from(v) * 5 + 127) / 255) as u8 };
    let cube = 16 + 36 * u32::from(step(r)) + 6 * u32::from(step(g)) + u32::from(step(b));
    let cube_val = |i: u8| -> i32 { if i == 0 { 0 } else { 55 + i32::from(i) * 40 } };
    let (cr, cg, cb) = (cube_val(step(r)), cube_val(step(g)), cube_val(step(b)));
    let cube_err =
        (cr - i32::from(r)).pow(2) + (cg - i32::from(g)).pow(2) + (cb - i32::from(b)).pow(2);
    let avg = (i32::from(r) + i32::from(g) + i32::from(b)) / 3;
    let gi = (avg - 8).max(0).min(230) / 10;
    let gv = 8 + gi * 10;
    let gray_err =
        (gv - i32::from(r)).pow(2) + (gv - i32::from(g)).pow(2) + (gv - i32::from(b)).pow(2);
    if gray_err < cube_err {
        u8::try_from(232 + gi).unwrap_or(255)
    } else {
        u8::try_from(cube).unwrap_or(255)
    }
}

/// Nearest named ANSI-16 colour: luminance for neutrals, hue sector then
/// brightness for chromatics.
pub fn nearest_16(r: u8, g: u8, b: u8) -> Color {
    let lum = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
    let max = u32::from(r.max(g).max(b));
    let min = u32::from(r.min(g).min(b));
    if max - min < 40 {
        return match lum {
            0..=30 => Color::Black,
            31..=110 => Color::DarkGray,
            111..=200 => Color::Gray,
            _ => Color::White,
        };
    }
    let bright = max > 180;
    match (r >= g && r >= b, g >= r && g >= b, b >= r && b >= g) {
        (true, _, _) if g > 120 && b < 80 => Color::Yellow,
        (true, _, _) => {
            if bright {
                Color::LightRed
            } else {
                Color::Red
            }
        }
        (_, true, _) => {
            if bright {
                Color::LightGreen
            } else {
                Color::Green
            }
        }
        _ => {
            if bright {
                Color::LightBlue
            } else {
                Color::Blue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downgrade_vectors_match_the_reference() {
        use ColorCapability::{Ansi16, Indexed256, Monochrome};
        // Exact indexed/named results, asserted per the reference algorithm.
        assert_eq!(
            downgrade(palette::WHITE_15, Ansi16),
            Color::DarkGray,
            "#262626 → DarkGray @16"
        );
        assert_eq!(
            downgrade(palette::AMBER, Ansi16),
            Color::Yellow,
            "#f59e09 → Yellow @16"
        );
        assert_eq!(
            downgrade(palette::GREEN, Ansi16),
            Color::LightGreen,
            "#48e054 → LightGreen @16"
        );
        assert_eq!(
            downgrade(palette::RED, Ansi16),
            Color::LightRed,
            "#e44545 → LightRed @16"
        );
        assert_eq!(
            downgrade(palette::BLACK, Ansi16),
            Color::Black,
            "#000000 → Black @16"
        );
        assert_eq!(
            downgrade(palette::WHITE_15, Indexed256),
            Color::Indexed(235),
            "#262626 → 235 @256"
        );
        assert_eq!(
            downgrade(palette::AMBER, Indexed256),
            Color::Indexed(214),
            "#f59e09 → 214 @256"
        );
        assert_eq!(
            downgrade(palette::GREEN, Indexed256),
            Color::Indexed(78),
            "#48e054 → Indexed(78) @256"
        );
        assert_eq!(
            downgrade(palette::CHROME, Indexed256),
            Color::Indexed(232),
            "#111111 → 232 @256"
        );
        assert_eq!(
            downgrade(palette::GREEN, Monochrome),
            Color::Gray,
            "#48e054 → Gray in mono"
        );
        assert_eq!(
            downgrade(palette::CHROME, Monochrome),
            Color::Black,
            "#111111 → Black in mono"
        );
        assert_eq!(
            downgrade(palette::WHITE, Monochrome),
            Color::White,
            "#ffffff → White in mono"
        );
    }

    #[test]
    fn truecolor_downgrade_is_identity() {
        for color in [
            palette::BLACK,
            palette::CHROME,
            palette::GREEN,
            palette::RED,
        ] {
            assert_eq!(downgrade(color, ColorCapability::Truecolor), color);
        }
    }

    #[test]
    fn indexed_and_named_tokens_pass_through_unchanged() {
        for color in [Color::Indexed(78), Color::Green, Color::Reset] {
            assert_eq!(downgrade(color, ColorCapability::Ansi16), color);
            assert_eq!(downgrade(color, ColorCapability::Indexed256), color);
            assert_eq!(downgrade(color, ColorCapability::Monochrome), color);
        }
    }

    #[test]
    fn accent_survives_downgrade() {
        let t = JunieTheme::for_level(ColorCapability::Indexed256);
        assert!(matches!(t.accent, Color::Indexed(_)));
        let t16 = JunieTheme::for_level(ColorCapability::Ansi16);
        assert_eq!(t16.accent, Color::LightGreen);
        assert_eq!(t16.error, Color::LightRed);
        assert_eq!(t16.canvas, Color::Black);
    }

    #[test]
    fn truecolor_theme_carries_the_canonical_hexes() {
        let t = JunieTheme::junie();
        let cases = [
            (t.canvas, palette::BLACK),
            (t.surface, palette::CHROME),
            (t.surface_elevated, palette::CARD),
            (t.surface_overlay, palette::OVERLAY),
            (t.field, palette::INPUT),
            (t.field_hover, palette::INPUT_HOVER),
            (t.popover, palette::POPOVER),
            (t.border_subtle, palette::WHITE_15),
            (t.border_strong, palette::WHITE_30),
            (t.text_primary, palette::WHITE),
            (t.text_secondary, palette::WHITE_70),
            (t.text_muted, palette::WHITE_50),
            (t.text_faint, palette::WHITE_30),
            (t.text_ghost, palette::WHITE_15),
            (t.text_on_accent, palette::ON_GREEN),
            (t.accent, palette::GREEN),
            (t.accent_hover, palette::GREEN_80),
            (t.accent_pressed, palette::GREEN_60),
            (t.accent_bg, palette::GREEN_20),
            (t.focus, palette::GREEN),
            (t.disabled, palette::WHITE_30),
            (t.error, palette::RED),
            (t.warning, palette::AMBER),
            (t.success, palette::GREEN),
        ];
        for (actual, expected) in cases {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn hover_and_focus_are_distinct_styles() {
        let t = JunieTheme::junie();
        let base = t.row(VisualState::default(), t.canvas);
        let hovered = t.row(
            VisualState {
                hovered: true,
                ..Default::default()
            },
            t.canvas,
        );
        let focused = t.row(
            VisualState {
                focused: true,
                ..Default::default()
            },
            t.canvas,
        );
        assert_ne!(base.bg, hovered.bg);
        assert_eq!(base.bg, focused.bg);
        assert!(focused.add_modifier.contains(Modifier::BOLD));
        assert!(!hovered.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn selection_tint_needs_the_keyboard() {
        let t = JunieTheme::junie();
        let parked = t.row(
            VisualState {
                selected: true,
                ..Default::default()
            },
            t.surface,
        );
        let owned = t.row(
            VisualState {
                selected: true,
                focused: true,
                ..Default::default()
            },
            t.surface,
        );
        assert_eq!(parked.bg, Some(t.surface), "a parked row never tints");
        assert_eq!(owned.bg, Some(t.accent_bg));
    }

    #[test]
    fn hover_lifts_exactly_one_plane() {
        let t = JunieTheme::junie();
        assert_eq!(t.lift(t.canvas), t.surface_elevated);
        assert_eq!(t.lift(t.surface), t.surface_overlay);
        assert_eq!(t.lift(t.surface_elevated), t.surface_overlay);
        assert_eq!(t.lift(t.field), t.field_hover);
        assert_eq!(t.lift(t.popover), t.popover, "the top rung cannot lift");
    }

    #[test]
    fn pressed_reverses_explicitly_without_the_reversed_modifier() {
        let t = JunieTheme::junie();
        let pressed = t.row(
            VisualState {
                pressed: true,
                ..Default::default()
            },
            t.surface,
        );
        assert_eq!(pressed.fg, Some(t.canvas));
        assert_eq!(pressed.bg, Some(t.text_primary));
        assert!(!pressed.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn disabled_button_ignores_hover() {
        let t = JunieTheme::junie();
        let d = VisualState {
            disabled: true,
            ..Default::default()
        };
        let dh = VisualState {
            disabled: true,
            hovered: true,
            ..Default::default()
        };
        assert_eq!(
            t.button(ButtonKind::Primary, d, t.surface),
            t.button(ButtonKind::Primary, dh, t.surface)
        );
    }

    #[test]
    fn button_table_matches_the_reference() {
        let t = JunieTheme::junie();
        let idle = VisualState::default();
        let hovered = VisualState {
            hovered: true,
            ..Default::default()
        };
        let focused = VisualState {
            focused: true,
            ..Default::default()
        };
        let pressed = VisualState {
            pressed: true,
            ..Default::default()
        };

        let primary = t.button(ButtonKind::Primary, idle, t.surface);
        assert_eq!(primary.fg, Some(t.text_on_accent));
        assert_eq!(primary.bg, Some(t.accent));
        assert_eq!(
            t.button(ButtonKind::Primary, hovered, t.surface).bg,
            Some(t.accent_hover)
        );
        assert_eq!(
            t.button(ButtonKind::Primary, pressed, t.surface).bg,
            Some(t.accent_pressed)
        );

        for kind in [ButtonKind::Secondary, ButtonKind::Toggle] {
            let s = t.button(kind, idle, t.surface);
            assert_eq!(s.fg, Some(t.text_primary));
            assert_eq!(s.bg, Some(t.surface_overlay));
            assert_eq!(t.button(kind, hovered, t.surface).bg, Some(t.popover));
            assert_eq!(t.button(kind, pressed, t.surface).bg, Some(t.text_primary));
            assert_eq!(
                t.button(kind, pressed, t.surface).fg,
                Some(t.canvas),
                "pressed reversal is explicit"
            );
        }

        let subtle = t.button(ButtonKind::Subtle, idle, t.surface);
        assert_eq!(subtle.fg, Some(t.text_secondary));
        assert_eq!(subtle.bg, Some(t.surface));
        let subtle_hover = t.button(ButtonKind::Subtle, hovered, t.surface);
        assert_eq!(subtle_hover.fg, Some(t.text_primary));
        assert_eq!(subtle_hover.bg, Some(t.lift(t.surface)));

        let danger = t.button(ButtonKind::Danger, idle, t.surface);
        assert_eq!(danger.fg, Some(t.error));
        assert_eq!(danger.bg, Some(t.surface_overlay));
        let danger_pressed = t.button(ButtonKind::Danger, pressed, t.surface);
        assert_eq!(danger_pressed.fg, Some(t.text_primary));
        assert_eq!(danger_pressed.bg, Some(t.error));

        // Focus adds weight to every non-primary kind; primary is already bold.
        for kind in [
            ButtonKind::Primary,
            ButtonKind::Secondary,
            ButtonKind::Subtle,
            ButtonKind::Danger,
        ] {
            assert!(
                t.button(kind, focused, t.surface)
                    .add_modifier
                    .contains(Modifier::BOLD)
                    || t.button(kind, idle, t.surface)
                        .add_modifier
                        .contains(Modifier::BOLD),
                "{kind:?} focus carries weight"
            );
        }
    }

    #[test]
    fn backdrop_collapses_the_ladder_without_losing_shape() {
        let t = JunieTheme::junie();
        // Surfaces keep their fill so the page keeps its shape.
        assert_eq!(t.backdrop(Style::new().bg(t.canvas)).bg, Some(t.canvas));
        assert_eq!(t.backdrop(Style::new().bg(t.surface)).bg, Some(t.surface));
        assert_eq!(
            t.backdrop(Style::new().bg(t.surface_elevated)).bg,
            Some(t.surface_elevated)
        );
        // Field planes recede one rung; unknown fills land on the overlay.
        assert_eq!(
            t.backdrop(Style::new().bg(t.field)).bg,
            Some(t.surface_elevated)
        );
        // A cell with no fill sits on the canvas ground.
        assert_eq!(t.backdrop(Style::new().fg(t.accent)).bg, Some(t.canvas));
        // Any other fill (accent, popover, selection tint) lands on overlay.
        assert_eq!(
            t.backdrop(Style::new().fg(t.accent).bg(t.popover)).bg,
            Some(t.surface_overlay)
        );
        // Text collapses onto the alpha ladder; a hidden gutter stays hidden.
        assert_eq!(
            t.backdrop(Style::new().fg(t.text_primary)).fg,
            Some(t.text_muted)
        );
        assert_eq!(
            t.backdrop(Style::new().fg(t.text_secondary)).fg,
            Some(t.text_faint)
        );
        assert_eq!(
            t.backdrop(Style::new().fg(t.text_muted)).fg,
            Some(t.text_ghost)
        );
        let hidden = Style::new().fg(t.surface).bg(t.surface);
        assert_eq!(t.backdrop(hidden).fg, Some(t.surface));
    }

    #[test]
    fn gutter_is_hidden_until_focused() {
        let t = JunieTheme::junie();
        let idle = t.gutter(VisualState::default(), t.surface, false);
        assert_eq!(idle.fg, Some(t.surface), "unfocused gutter is invisible");
        let owned = t.gutter(
            VisualState {
                focused: true,
                ..Default::default()
            },
            t.surface,
            false,
        );
        assert_eq!(owned.fg, Some(t.focus));
        assert!(
            !owned.add_modifier.contains(Modifier::BOLD),
            "gutter is colour-only; weight comes from the row fill"
        );
        let on_accent = t.gutter(
            VisualState {
                focused: true,
                ..Default::default()
            },
            t.accent,
            true,
        );
        assert_eq!(on_accent.fg, Some(t.text_primary));
    }

    #[test]
    fn tone_never_resolves_the_accent() {
        let t = JunieTheme::junie();
        // Success IS the accent in junie, so it is excluded by design.
        for tone in [
            Tone::Normal,
            Tone::Secondary,
            Tone::Muted,
            Tone::Faint,
            Tone::Error,
            Tone::Warning,
        ] {
            assert_ne!(t.tone(tone), t.accent, "{tone:?} impersonates the accent");
        }
        assert_eq!(t.tone(Tone::Success), t.accent);
    }

    #[test]
    fn syntax_is_the_text_ladder_plus_weight() {
        let t = JunieTheme::junie();
        assert_eq!(t.syntax(SyntaxTone::Keyword).add_modifier, Modifier::BOLD);
        assert_eq!(t.syntax(SyntaxTone::Ident).fg, Some(t.text_primary));
        assert_eq!(t.syntax(SyntaxTone::Str).fg, Some(t.text_secondary));
        assert_eq!(t.syntax(SyntaxTone::Number).fg, Some(t.text_secondary));
        assert_eq!(t.syntax(SyntaxTone::Operator).fg, Some(t.text_muted));
        assert_eq!(t.syntax(SyntaxTone::Punct).fg, Some(t.text_muted));
        let comment = t.syntax(SyntaxTone::Comment);
        assert_eq!(comment.fg, Some(t.text_faint));
        assert_eq!(comment.add_modifier, Modifier::ITALIC);
    }

    #[test]
    fn edit_badge_is_on_accent() {
        let t = JunieTheme::junie();
        let badge = t.badge(BadgeKind::Edit);
        assert_eq!(badge.fg, Some(t.text_on_accent));
        assert_eq!(badge.bg, Some(t.accent));
        assert_eq!(badge.add_modifier, Modifier::BOLD);
    }

    #[test]
    fn mono_is_four_grey_buckets() {
        assert_eq!(mono_level(0x00, 0x00, 0x00), 0);
        assert_eq!(mono_level(0x11, 0x11, 0x11), 0);
        assert_eq!(mono_level(0x26, 0x26, 0x26), 0);
        assert_eq!(mono_level(0x48, 0xe0, 0x54), 2);
        assert_eq!(mono_level(0xff, 0xff, 0xff), 3);
    }

    #[test]
    fn no_ported_theme_emits_dim_or_the_reversed_modifier() {
        // D5: reversal is always expressed as fg(canvas).bg(text_primary).
        let t = JunieTheme::junie();
        let states = [
            VisualState::default(),
            VisualState {
                hovered: true,
                ..Default::default()
            },
            VisualState {
                focused: true,
                ..Default::default()
            },
            VisualState {
                pressed: true,
                ..Default::default()
            },
            VisualState {
                selected: true,
                focused: true,
                ..Default::default()
            },
            VisualState {
                disabled: true,
                ..Default::default()
            },
            VisualState {
                busy: true,
                ..Default::default()
            },
            VisualState {
                error: true,
                ..Default::default()
            },
        ];
        for state in states {
            for bg in [t.canvas, t.surface, t.surface_elevated, t.field] {
                let row = t.row(state, bg);
                assert!(!row.add_modifier.contains(Modifier::REVERSED));
                assert!(!row.add_modifier.contains(Modifier::DIM));
                assert!(!row.sub_modifier.contains(Modifier::REVERSED));
                assert!(!row.sub_modifier.contains(Modifier::DIM));
            }
        }
    }
}
