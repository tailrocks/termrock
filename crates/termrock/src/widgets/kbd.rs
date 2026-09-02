// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Kbd and ShortcutHint — canonical keyboard chord display.
//!
//! **Kbd** paints a single chord, sequence, or alternative set.  
//! **ShortcutHint** pairs a chord display with a semantic command label
//! (footer, inline docs, or keycap form). Prefer deriving from
//! [`crate::keymap::Keymap`] / [`crate::keymap::KeyBinding`] so advertised
//! keys cannot drift from dispatch.
//!
//! Platform-aware modifier names (Emacs `C-` / spelled `Ctrl+` / Mac symbols)
//! with ASCII and narrow contraction.
//!
//! References: shadcn Kbd, editor shortcut UIs, Textual bindings, Zellij help.
#![allow(unused_variables, unused_mut)] // unit-test fixtures
use std::borrow::Cow;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyCode, KeyModifiers};
use crate::keymap::{KeyBinding, KeyChord, Keymap, chord_glyph};
use crate::style::{DesignSystem, GlyphSet, Role};
use crate::text::{display_cols, take_display_cols};

// ── Platform / format style ─────────────────────────────────────────────────

/// Host platform for modifier naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Platform {
    /// Detect at compile time (`cfg(target_os = "macos")` → Mac).
    #[default]
    Auto,
    /// macOS-style symbols when not forced ASCII.
    Mac,
    /// Linux / Windows / other — Ctrl/Alt/Shift wording.
    Other,
}

impl Platform {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mac => "mac",
            Self::Other => "other",
        }
    }

    /// Resolved platform (Auto → Mac/Other).
    #[must_use]
    pub const fn resolve(self) -> Self {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    Self::Mac
                } else {
                    Self::Other
                }
            }
            other => other,
        }
    }
}

/// How modifiers are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ModifierStyle {
    /// Emacs-style compact: `C-`, `A-`, `S-` (default for footers).
    #[default]
    Emacs,
    /// Spelled: `Ctrl+`, `Alt+`, `Shift+`.
    Spelled,
    /// Symbolic (Mac-first): `⌃` `⌥` `⇧` `⌘` when not ASCII.
    Symbols,
}

impl ModifierStyle {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Emacs => "emacs",
            Self::Spelled => "spelled",
            Self::Symbols => "symbols",
        }
    }
}

/// Visual form of a keycap / chord paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KbdVariant {
    /// Compact footer form (`C-s`).
    #[default]
    Compact,
    /// Keycap-like brackets (`[ C-s ]`).
    Keycap,
    /// Inline documentation form (often spelled modifiers).
    Inline,
}

impl KbdVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Keycap => "keycap",
            Self::Inline => "inline",
        }
    }
}

/// How a shortcut pair is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ShortcutForm {
    /// Footer strip: `C-s Save` (default).
    #[default]
    Footer,
    /// Inline docs: `Save  Ctrl+S` (command first, spaced).
    InlineDoc,
    /// Keycap-heavy: `[C-s] Save`.
    Keycap,
}

impl ShortcutForm {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Footer => "footer",
            Self::InlineDoc => "inline-doc",
            Self::Keycap => "keycap",
        }
    }
}

// ── Chord formatting ────────────────────────────────────────────────────────

/// Format options for chords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChordFormat {
    /// Platform.
    pub platform: Platform,
    /// Modifier naming.
    pub modifiers: ModifierStyle,
}

impl Default for ChordFormat {
    fn default() -> Self {
        Self {
            platform: Platform::Auto,
            modifiers: ModifierStyle::Emacs,
        }
    }
}

impl ChordFormat {
    /// Footer-friendly defaults.
    #[must_use]
    pub const fn footer() -> Self {
        Self {
            platform: Platform::Auto,
            modifiers: ModifierStyle::Emacs,
        }
    }

    /// Documentation defaults (spelled modifiers).
    #[must_use]
    pub const fn docs() -> Self {
        Self {
            platform: Platform::Auto,
            modifiers: ModifierStyle::Spelled,
        }
    }

    /// From glyph set (ASCII profile → ascii format).
    #[must_use]
    pub const fn from_glyphs(glyphs: GlyphSet) -> Self {
        Self {
            platform: Platform::Auto,
            modifiers: ModifierStyle::Emacs,
        }
    }

    /// ASCII forced.
    #[must_use]
    /// Modifier style.
    pub const fn modifiers(mut self, style: ModifierStyle) -> Self {
        self.modifiers = style;
        self
    }

    /// Platform.
    #[must_use]
    pub const fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
}

/// Format one [`KeyChord`] for display.
#[must_use]
pub fn format_chord(chord: KeyChord, fmt: ChordFormat) -> String {
    let platform = fmt.platform.resolve();
    let mut out = String::new();
    let mods = chord.mods;
    let style = if matches!(fmt.modifiers, ModifierStyle::Symbols) {
        ModifierStyle::Emacs
    } else {
        fmt.modifiers
    };

    match style {
        ModifierStyle::Emacs => {
            if mods.contains(KeyModifiers::CONTROL) {
                out.push_str("C-");
            }
            if mods.contains(KeyModifiers::ALT) {
                out.push_str("A-");
            }
            if mods.contains(KeyModifiers::SHIFT) {
                // Char keys already encode shift in case.
                if !matches!(chord.key, KeyCode::Char(_)) {
                    out.push_str("S-");
                }
            }
        }
        ModifierStyle::Spelled => {
            if mods.contains(KeyModifiers::CONTROL) {
                out.push_str(if matches!(platform, Platform::Mac) {
                    "Control+"
                } else {
                    "Ctrl+"
                });
            }
            if mods.contains(KeyModifiers::ALT) {
                out.push_str(if matches!(platform, Platform::Mac) {
                    "Option+"
                } else {
                    "Alt+"
                });
            }
            if mods.contains(KeyModifiers::SHIFT) && !matches!(chord.key, KeyCode::Char(_)) {
                out.push_str("Shift+");
            }
        }
        ModifierStyle::Symbols => {
            if matches!(platform, Platform::Mac) {
                if mods.contains(KeyModifiers::CONTROL) {
                    out.push('⌃');
                }
                if mods.contains(KeyModifiers::ALT) {
                    out.push('⌥');
                }
                if mods.contains(KeyModifiers::SHIFT) && !matches!(chord.key, KeyCode::Char(_)) {
                    out.push('⇧');
                }
            } else {
                // Non-Mac symbols fall back to Emacs compact.
                return format_chord(
                    chord,
                    ChordFormat {
                        modifiers: ModifierStyle::Emacs,
                        ..fmt
                    },
                );
            }
        }
    }

    out.push_str(&format_key(chord.key));
    out
}

fn format_key(key: KeyCode) -> String {
    match key {
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
        KeyCode::Enter => "↵".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "⇥".into(),
        KeyCode::BackTab => "⇤".into(),
        KeyCode::Backspace => "⌫".into(),
        KeyCode::Delete => "Del".into(),
        KeyCode::Up => "↑".into(),
        KeyCode::Down => "↓".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PgUp".into(),
        KeyCode::PageDown => "PgDn".into(),
        KeyCode::Unknown => "?".into(),
    }
}

/// Format a chord sequence (e.g. `g` then `g` → `g g`).
#[must_use]
pub fn format_sequence(chords: &[KeyChord], fmt: ChordFormat, sep: &str) -> String {
    chords
        .iter()
        .map(|c| format_chord(*c, fmt))
        .collect::<Vec<_>>()
        .join(sep)
}

/// Format alternatives for the same action (`C-s / :w`).
#[must_use]
pub fn format_alternatives(chords: &[KeyChord], fmt: ChordFormat) -> String {
    format_sequence(chords, fmt, " / ")
}

/// Prefer binding glyph when set; otherwise format all chords as alternatives.
#[must_use]
pub fn format_binding<A: Clone + 'static>(binding: &KeyBinding<A>, fmt: ChordFormat) -> String {
    if let Some(g) = binding.glyph() {
        if !g.is_empty() {
            return g.to_string();
        }
    }
    let chords = binding.chords();
    if chords.is_empty() {
        return String::new();
    }
    if chords.len() == 1 {
        let formatted = format_chord(chords[0], fmt);
        if formatted.is_empty() {
            chord_glyph(Some(chords[0])).to_string()
        } else {
            formatted
        }
    } else {
        format_alternatives(chords, fmt)
    }
}

// ── Kbd widget ──────────────────────────────────────────────────────────────

/// Canonical key chord display.
#[derive(Debug, Clone)]
pub struct Kbd<'a> {
    label: Cow<'a, str>,
    system: &'a DesignSystem,
    variant: KbdVariant,
}

/// The keycap form of a chord: one place composes the brackets.
///
/// Measurement and paint used to compose `[..]` separately in `Kbd` and in
/// `ShortcutHint`, which is how the two renderings drifted apart (plans/015).
#[must_use]
pub fn keycap_text(chord: &str) -> String {
    format!("[{}]", chord.trim())
}

impl<'a> Kbd<'a> {
    /// Explicit chord label (e.g. already resolved glyph).
    #[must_use]
    pub fn new(label: impl Into<Cow<'a, str>>, system: &'a DesignSystem) -> Self {
        Self {
            label: label.into(),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// From one chord.
    #[must_use]
    pub fn from_chord(chord: KeyChord, system: &'a DesignSystem) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        Self {
            label: Cow::Owned(format_chord(chord, fmt)),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// From chord with format options.
    #[must_use]
    pub fn from_chord_fmt(chord: KeyChord, fmt: ChordFormat, system: &'a DesignSystem) -> Self {
        Self {
            label: Cow::Owned(format_chord(chord, fmt)),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// Sequence of chords.
    #[must_use]
    pub fn sequence(chords: &[KeyChord], system: &'a DesignSystem) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        Self {
            label: Cow::Owned(format_sequence(chords, fmt, " ")),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// Alternative chords for one command.
    #[must_use]
    pub fn alternatives(chords: &[KeyChord], system: &'a DesignSystem) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        Self {
            label: Cow::Owned(format_alternatives(chords, fmt)),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// From a keymap binding (glyph override wins).
    #[must_use]
    pub fn from_binding<A: Clone + 'static>(
        binding: &KeyBinding<A>,
        system: &'a DesignSystem,
    ) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        Self {
            label: Cow::Owned(format_binding(binding, fmt)),
            system,
            variant: KbdVariant::Compact,
        }
    }

    /// From keymap action.
    #[must_use]
    pub fn for_action<A: Clone + Copy + PartialEq + 'static>(
        map: &Keymap<A>,
        action: A,
        system: &'a DesignSystem,
    ) -> Option<Self> {
        map.binding_for(action)
            .map(|b| Self::from_binding(b, system))
    }

    /// Visual variant.
    #[must_use]
    pub const fn variant(mut self, variant: KbdVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Keycap form.
    #[must_use]
    pub const fn keycap(mut self) -> Self {
        self.variant = KbdVariant::Keycap;
        self
    }

    /// Inline docs form (still paints HintKey role).
    #[must_use]
    pub const fn inline(mut self) -> Self {
        self.variant = KbdVariant::Inline;
        self
    }

    /// Compact footer form.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.variant = KbdVariant::Compact;
        self
    }

    /// Display text (without keycap padding).
    #[must_use]
    pub fn text(&self) -> &str {
        self.label.as_ref()
    }

    /// Painted string including variant chrome.
    #[must_use]
    pub fn decorated(&self) -> String {
        match self.variant {
            KbdVariant::Compact | KbdVariant::Inline => self.label.to_string(),
            KbdVariant::Keycap => format!(" {} ", self.label.trim()),
        }
    }

    /// Measure display columns.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        let d = match self.variant {
            KbdVariant::Keycap => keycap_text(&self.decorated()),
            _ => self.decorated(),
        };
        u16::try_from(display_cols(&d)).unwrap_or(1).max(1)
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let text = match self.variant {
            KbdVariant::Keycap => keycap_text(&self.decorated()),
            KbdVariant::Compact | KbdVariant::Inline => self.decorated(),
        };
        let clipped = take_display_cols(&text, usize::from(area.width));
        let mut style = self.system.key_hint_key();
        if matches!(self.variant, KbdVariant::Keycap) {
            style = style.add_modifier(Modifier::BOLD);
        }
        if matches!(self.variant, KbdVariant::Keycap) {
            // junie: a keycap is one surface plane above the chrome.
            if let Some(bg) = self.system.style(Role::Surface).bg {
                style = style.bg(self.system.lift(bg));
            }
        }
        buffer.set_stringn(area.x, area.y, &clipped, usize::from(area.width), style);
    }
}

impl Widget for &Kbd<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for Kbd<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

// ── ShortcutHint ────────────────────────────────────────────────────────────

/// Chord + semantic command label.
#[derive(Debug, Clone)]
pub struct ShortcutHint<'a> {
    chord: Cow<'a, str>,
    command: Cow<'a, str>,
    system: &'a DesignSystem,
    form: ShortcutForm,
    /// Drop command before chord when width is tight.
    contract_command_first: bool,
}

impl<'a> ShortcutHint<'a> {
    /// Explicit chord string + command.
    #[must_use]
    pub fn new(
        chord: impl Into<Cow<'a, str>>,
        command: impl Into<Cow<'a, str>>,
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            chord: chord.into(),
            command: command.into(),
            system,
            form: ShortcutForm::Footer,
            contract_command_first: true,
        }
    }

    /// From chords + command.
    #[must_use]
    pub fn from_chords(
        chords: &[KeyChord],
        command: impl Into<Cow<'a, str>>,
        system: &'a DesignSystem,
    ) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        let chord = if chords.len() <= 1 {
            chords
                .first()
                .map(|c| format_chord(*c, fmt))
                .unwrap_or_default()
        } else {
            format_alternatives(chords, fmt)
        };
        Self {
            chord: Cow::Owned(chord),
            command: command.into(),
            system,
            form: ShortcutForm::Footer,
            contract_command_first: true,
        }
    }

    /// From keymap binding (display + hint label).
    #[must_use]
    pub fn from_binding<A: Clone + 'static>(
        binding: &KeyBinding<A>,
        system: &'a DesignSystem,
    ) -> Self {
        let fmt = ChordFormat::from_glyphs(system.glyphs);
        let chord = format_binding(binding, fmt);
        let command = binding.hint().unwrap_or("").to_string();
        Self {
            chord: Cow::Owned(chord),
            command: Cow::Owned(command),
            system,
            form: ShortcutForm::Footer,
            contract_command_first: true,
        }
    }

    /// From keymap action (requires Shown/Hidden binding with optional hint).
    #[must_use]
    pub fn for_action<A: Clone + Copy + PartialEq + 'static>(
        map: &Keymap<A>,
        action: A,
        system: &'a DesignSystem,
    ) -> Option<Self> {
        map.binding_for(action)
            .map(|b| Self::from_binding(b, system))
    }

    /// Form.
    #[must_use]
    pub const fn form(mut self, form: ShortcutForm) -> Self {
        self.form = form;
        self
    }

    /// Footer form.
    #[must_use]
    pub const fn footer(mut self) -> Self {
        self.form = ShortcutForm::Footer;
        self
    }

    /// Inline documentation form.
    #[must_use]
    pub const fn inline_doc(mut self) -> Self {
        self.form = ShortcutForm::InlineDoc;
        self
    }

    /// Keycap form.
    #[must_use]
    pub const fn keycap(mut self) -> Self {
        self.form = ShortcutForm::Keycap;
        self
    }

    /// Chord display.
    #[must_use]
    pub fn chord(&self) -> &str {
        self.chord.as_ref()
    }

    /// Command label.
    #[must_use]
    pub fn command(&self) -> &str {
        self.command.as_ref()
    }

    /// Full plain text for a11y / copy.
    #[must_use]
    pub fn plain(&self) -> String {
        if self.command.is_empty() {
            self.chord.to_string()
        } else {
            format!("{} {}", self.chord, self.command)
        }
    }

    /// Measure natural width.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        let k = match self.form {
            ShortcutForm::Keycap => display_cols(&keycap_text(self.chord.as_ref())),
            _ => display_cols(self.chord.as_ref()),
        };
        let c = if self.command.is_empty() {
            0
        } else {
            1 + display_cols(self.command.as_ref())
        };
        u16::try_from(k + c).unwrap_or(1).max(1)
    }

    /// Whether command is shown at this width.
    #[must_use]
    pub fn shows_command(&self, width: u16) -> bool {
        if self.command.is_empty() {
            return false;
        }
        if !self.contract_command_first {
            return true;
        }
        // Keep at least the chord; drop command under ~ half natural or < 16 cols.
        let natural = self.measure_width();
        width >= natural.saturating_sub(2).max(16)
    }

    /// Paint into area (contracts command before chord).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let show_cmd = self.shows_command(area.width);
        let key_style = self.system.key_hint_key();
        let text_style = self.system.key_hint_action();
        let sep_style = self.system.style(Role::HintSeparator);

        match self.form {
            ShortcutForm::Footer | ShortcutForm::Keycap => {
                // One keycap renderer for the library: `Kbd` owns the bracket,
                // the weight and the raised ground, so a hint's keycap and a
                // standalone keycap cannot drift (plans/015 Step 2).
                let keycap = matches!(self.form, ShortcutForm::Keycap);
                let key = if keycap {
                    keycap_text(self.chord.as_ref())
                } else {
                    self.chord.to_string()
                };
                let key = take_display_cols(&key, usize::from(area.width));
                let kw = display_cols(&key) as u16;
                if keycap {
                    Kbd::new(self.chord.as_ref(), self.system)
                        .keycap()
                        .paint(Rect::new(area.x, area.y, kw.min(area.width), 1), buffer);
                } else {
                    buffer.set_stringn(area.x, area.y, &key, usize::from(area.width), key_style);
                }
                if show_cmd {
                    let x = area.x.saturating_add(kw).saturating_add(1);
                    if x < area.right() {
                        let rest = area.right().saturating_sub(x);
                        let cmd = take_display_cols(self.command.as_ref(), usize::from(rest));
                        buffer.set_stringn(x, area.y, &cmd, usize::from(rest), text_style);
                    }
                }
            }
            ShortcutForm::InlineDoc => {
                // Command first, then chord.
                if show_cmd {
                    let cmd = take_display_cols(self.command.as_ref(), usize::from(area.width));
                    let cw = display_cols(&cmd) as u16;
                    buffer.set_stringn(area.x, area.y, &cmd, usize::from(area.width), text_style);
                    let x = area.x.saturating_add(cw).saturating_add(2);
                    if x < area.right() {
                        buffer.set_stringn(x.saturating_sub(1), area.y, " ", 1, sep_style);
                        let rest = area.right().saturating_sub(x);
                        let key = take_display_cols(self.chord.as_ref(), usize::from(rest));
                        buffer.set_stringn(x, area.y, &key, usize::from(rest), key_style);
                    }
                } else {
                    let key = take_display_cols(self.chord.as_ref(), usize::from(area.width));
                    buffer.set_stringn(area.x, area.y, &key, usize::from(area.width), key_style);
                }
            }
        }
    }
}

impl Widget for &ShortcutHint<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for ShortcutHint<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

// ── Legacy Kbd::from_chord buffer API ───────────────────────────────────────

impl<'a> Kbd<'a> {
    /// Format a [`KeyChord`] into a short display label (writes into `buf`).
    #[must_use]
    pub fn from_chord_buf(chord: KeyChord, buf: &'a mut String, tokens: &'a DesignSystem) -> Self {
        let fmt = ChordFormat::from_glyphs(tokens.glyphs);
        *buf = format_chord(chord, fmt);
        Self {
            label: Cow::Borrowed(buf.as_str()),
            system: tokens,
            variant: KbdVariant::Keycap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{KeyBinding, Visibility};

    #[test]
    fn emacs_ctrl_s() {
        let s = format_chord(KeyChord::ctrl(KeyCode::Char('s')), ChordFormat::footer());
        assert_eq!(s, "C-S");
    }

    #[test]
    fn spelled_ctrl() {
        let fmt = ChordFormat::docs().platform(Platform::Other);
        let s = format_chord(KeyChord::ctrl(KeyCode::Char('s')), fmt);
        assert!(s.starts_with("Ctrl+"));
        assert!(s.ends_with('S'));
    }

    #[test]
    fn mac_symbols() {
        let fmt = ChordFormat {
            platform: Platform::Mac,
            modifiers: ModifierStyle::Symbols,
        };
        let s = format_chord(KeyChord::ctrl(KeyCode::Char('c')), fmt);
        assert!(s.contains('⌃') || s.contains('C'));
    }

    #[test]
    fn alternatives_join() {
        let chords = [
            KeyChord::ctrl(KeyCode::Char('s')),
            KeyChord::plain(KeyCode::Char('s')),
        ];
        let s = format_alternatives(&chords, ChordFormat::footer());
        assert!(s.contains(" / "));
    }

    #[test]
    fn sequence_join() {
        let chords = [KeyChord::plain(KeyCode::Char('g')); 2];
        let s = format_sequence(&chords, ChordFormat::footer(), " ");
        assert_eq!(s, "G G");
    }

    #[test]
    fn kbd_keycap_measure() {
        let system = DesignSystem::default();
        let k = Kbd::new("C-S", &system).keycap();
        assert!(k.measure_width() >= 3);
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        k.paint(Rect::new(0, 0, 12, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "[");
    }

    #[test]
    fn shortcut_from_binding() {
        let system = DesignSystem::default();
        let b = KeyBinding::owned(
            vec![KeyChord::ctrl(KeyCode::Char('s'))],
            (),
            Some("Save".into()),
            Visibility::Shown,
            None,
        );
        let h = ShortcutHint::from_binding(&b, &system);
        assert_eq!(h.command(), "Save");
        assert!(h.chord().contains('S') || h.chord().contains('s') || h.chord().contains('C'));
    }

    #[test]
    fn shortcut_contracts_command() {
        let system = DesignSystem::default();
        let h = ShortcutHint::new("C-S", "Save document to disk", &system);
        assert!(!h.shows_command(10));
        assert!(h.shows_command(40));
    }

    #[test]
    fn for_action_from_keymap() {
        #[derive(Clone, Copy, PartialEq)]
        enum A {
            Save,
        }
        let map = Keymap::from_owned(vec![KeyBinding::owned(
            vec![KeyChord::ctrl(KeyCode::Char('s'))],
            A::Save,
            Some("Save".into()),
            Visibility::Shown,
            None,
        )]);
        let system = DesignSystem::default();
        let h = ShortcutHint::for_action(&map, A::Save, &system).expect("bound");
        assert_eq!(h.command(), "Save");
        let k = Kbd::for_action(&map, A::Save, &system).expect("bound");
        assert!(!k.text().is_empty());
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let chord = KeyChord::ctrl(KeyCode::Char('q'));
        for _ in 0..20_000 {
            let _ = format_chord(chord, ChordFormat::footer());
        }
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        Kbd::new("X", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        ShortcutHint::new("C-X", "Cut", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
    }

    #[test]
    fn footer_hint_is_bold_key_and_muted_action() {
        let system = DesignSystem::default();
        let hint = ShortcutHint::new("Esc", "Cancel", &system).footer();
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);
        hint.paint(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "E");
        assert_eq!(buffer[(0, 0)].fg, system.key_hint_key().fg.unwrap());
        assert!(
            buffer[(0, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::BOLD)
        );
        assert_eq!(buffer[(4, 0)].symbol(), "C");
        assert_eq!(buffer[(4, 0)].fg, system.key_hint_action().fg.unwrap());
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.starts_with("Esc Cancel"), "{row}");
    }
}
