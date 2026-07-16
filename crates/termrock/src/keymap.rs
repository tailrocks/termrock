// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Keybinding registry — single source of truth coupling key dispatch and hint advertisement.
//!
//! A [`Keymap<A>`] binds each action to one or more key chords and a hint label. The
//! dispatcher matches incoming keys against the table; the hint renderer produces
//! [`HintSpan`] sequences from the same table. Divergence between handled keys and
//! advertised keys is therefore structurally impossible for [`Visibility::Shown`] and
//! [`Visibility::HiddenAlias`] bindings.

use crate::input::{KeyCode, KeyModifiers};
use crate::scroll::ScrollAxes;
use crate::widgets::HintSpan;

/// A key chord: a logical key plus zero or more modifier bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    /// Chord with no modifiers.
    #[must_use]
    pub const fn plain(key: KeyCode) -> Self {
        Self {
            key,
            mods: KeyModifiers::NONE,
        }
    }

    /// Chord with Ctrl held.
    #[must_use]
    pub const fn ctrl(key: KeyCode) -> Self {
        Self {
            key,
            mods: KeyModifiers::CONTROL,
        }
    }

    /// Chord with Alt held.
    #[must_use]
    pub const fn alt(key: KeyCode) -> Self {
        Self {
            key,
            mods: KeyModifiers::ALT,
        }
    }

    /// Chord with Shift held (typically only meaningful for non-Char keys).
    #[must_use]
    pub const fn shift(key: KeyCode) -> Self {
        Self {
            key,
            mods: KeyModifiers::SHIFT,
        }
    }

    /// Chord with Alt-Shift held (used for pane-resize CSI sequences).
    #[must_use]
    pub const fn alt_shift(key: KeyCode) -> Self {
        Self {
            key,
            mods: KeyModifiers::ALT.with_shift(),
        }
    }
}

/// Convert a crossterm `KeyEvent` into a platform-neutral [`KeyChord`].
///
/// Shift is only tracked for non-`Char` keys because for `Char` keys the
/// shifted character is already encoded in the `char` value (`'Q'` vs `'q'`).
/// Unknown backend keys remain `KeyCode::Unknown`, which never appears in a
/// binding table and therefore stays inert.
impl From<crate::input::KeyEvent> for KeyChord {
    fn from(ev: crate::input::KeyEvent) -> Self {
        let is_char = matches!(ev.code, KeyCode::Char(_));
        let mut mods = KeyModifiers::NONE;
        if ev.modifiers.contains(KeyModifiers::CONTROL) {
            mods = mods.with_ctrl();
        }
        if ev.modifiers.contains(KeyModifiers::ALT) {
            mods = mods.with_alt();
        }
        // Shift is intrinsic to Char casing; only track it for non-Char keys.
        if !is_char && ev.modifiers.contains(KeyModifiers::SHIFT) {
            mods = mods.with_shift();
        }
        Self { key: ev.code, mods }
    }
}

/// Convert a bare crossterm `KeyCode` into a [`KeyChord`] with no modifiers.
///
/// Surfaces whose dispatch resolvers operate on `KeyCode` (no modifier
/// context, because the modified chords like `Ctrl+Q` are intercepted
/// upstream) build chords through this conversion, keeping their keymap
/// dispatch identical in spirit to the `KeyEvent`-based surfaces.
impl From<crate::input::KeyCode> for KeyChord {
    fn from(code: crate::input::KeyCode) -> Self {
        Self::plain(code)
    }
}

// ── Binding model ────────────────────────────────────────────────────────────

/// Whether a binding is visible in the hint bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Key is advertised in the hint bar.
    Shown,
    /// Key works but is not advertised (convenience alias, e.g. vim `h`/`j`/`k`/`l`).
    HiddenAlias,
    /// Key is consumed internally by the widget (e.g. editing keys in a text input).
    Internal,
}

/// One entry in a [`Keymap`]: a set of chords all mapping to the same action.
///
/// The first chord in `chords` drives the glyph in the hint bar; remaining
/// chords are aliases. Set `glyph` to a `Some` string for grouped glyphs like
/// `"↑↓"` that span multiple bindings.
#[derive(Debug)]
pub struct KeyBinding<A> {
    /// All chords that fire this action. First chord drives the hint glyph.
    pub chords: &'static [KeyChord],
    /// The action value returned by [`Keymap::dispatch`].
    pub action: A,
    /// Label displayed after the key glyph in the hint bar (e.g. `"confirm"`).
    /// `None` silences the label; set to `None` for `Internal` bindings.
    pub hint: Option<&'static str>,
    /// Whether this binding appears in the hint bar.
    pub visibility: Visibility,
    /// Override the auto-derived glyph from [`chord_glyph`]. Use for grouped
    /// glyphs like `"↑↓"` or combined glyphs like `"N/Esc"`.
    pub glyph: Option<&'static str>,
}

/// A static keymap binding all chords to actions for a single widget context.
#[derive(Debug)]
pub struct Keymap<A: 'static> {
    bindings: &'static [KeyBinding<A>],
}

impl<A: Copy + 'static> Keymap<A> {
    /// Construct a keymap from a static binding slice.
    #[must_use]
    pub const fn new(bindings: &'static [KeyBinding<A>]) -> Self {
        Self { bindings }
    }

    /// Return all bindings in declaration order (for testing and introspection).
    #[must_use]
    pub fn bindings(&self) -> &[KeyBinding<A>] {
        self.bindings
    }

    /// Return the action for the first binding whose chord set contains `chord`,
    /// or `None` if no binding matches.
    #[must_use]
    pub fn dispatch(&self, chord: KeyChord) -> Option<A> {
        self.bindings
            .iter()
            .find(|b| b.chords.contains(&chord))
            .map(|b| b.action)
    }

    /// Produce [`HintSpan`] sequences for all [`Visibility::Shown`] bindings.
    /// Adjacent `Shown` bindings are separated by [`HintSpan::GroupSep`], matching
    /// the codebase convention that distinct key actions sit in separate visual groups.
    #[must_use]
    pub fn hint_spans(&self) -> Vec<HintSpan<'static>> {
        self.hint_spans_filtered(|_| true)
    }

    /// Like [`Self::hint_spans`] but omits scroll-axis arrow bindings when the
    /// corresponding scroll axis is unavailable (matching the behaviour of
    /// a caller-defined scroll hint renderer.
    #[must_use]
    pub fn hint_spans_for_axes(&self, axes: ScrollAxes) -> Vec<HintSpan<'static>> {
        self.hint_spans_filtered(|b| Self::axis_gate_passes(b, axes))
    }

    fn hint_spans_filtered(
        &self,
        filter: impl Fn(&KeyBinding<A>) -> bool,
    ) -> Vec<HintSpan<'static>> {
        let mut spans: Vec<HintSpan<'static>> = Vec::new();
        for binding in self
            .bindings
            .iter()
            .filter(|b| b.visibility == Visibility::Shown)
        {
            if !filter(binding) {
                continue;
            }
            if !spans.is_empty() {
                spans.push(HintSpan::GroupSep);
            }
            let glyph: &'static str = binding
                .glyph
                .unwrap_or_else(|| chord_glyph(binding.chords.first().copied()));
            spans.push(HintSpan::Key(glyph));
            if let Some(label) = binding.hint {
                spans.push(HintSpan::Text(label));
            }
        }
        spans
    }

    fn axis_gate_passes(binding: &KeyBinding<A>, axes: ScrollAxes) -> bool {
        let all_vertical = !binding.chords.is_empty()
            && binding
                .chords
                .iter()
                .all(|c| matches!(c.key, KeyCode::Up | KeyCode::Down) && c.mods.is_empty());
        let all_horizontal = !binding.chords.is_empty()
            && binding
                .chords
                .iter()
                .all(|c| matches!(c.key, KeyCode::Left | KeyCode::Right) && c.mods.is_empty());
        if all_vertical && !axes.vertical {
            return false;
        }
        if all_horizontal && !axes.horizontal {
            return false;
        }
        true
    }
}

impl<A: Copy + PartialEq + 'static> Keymap<A> {
    /// Return the first binding whose action equals `action`, or `None`.
    #[must_use]
    pub fn binding_for(&self, action: A) -> Option<&KeyBinding<A>> {
        self.bindings.iter().find(|b| b.action == action)
    }

    /// The hint glyph for `action`, derived from its binding the same way
    /// [`hint_spans`](Keymap::hint_spans) derives it. Returns `""` if the
    /// action is unbound. Lets context-composed footers pull a key's glyph
    /// from the same table that drives dispatch, so the two cannot drift.
    #[must_use]
    pub fn glyph_for(&self, action: A) -> &'static str {
        self.binding_for(action).map_or("", |b| {
            b.glyph
                .unwrap_or_else(|| chord_glyph(b.chords.first().copied()))
        })
    }

    /// Push the `Key` (and optional `Text`) spans for `action` onto `out`,
    /// derived from the binding. No separators are added — the caller owns
    /// layout. Does nothing if the action is unbound. The single primitive
    /// for building context-dependent footers from a keymap.
    pub fn push_spans_for(&self, action: A, out: &mut Vec<HintSpan<'static>>) {
        if let Some(binding) = self.binding_for(action) {
            let glyph = binding
                .glyph
                .unwrap_or_else(|| chord_glyph(binding.chords.first().copied()));
            out.push(HintSpan::Key(glyph));
            if let Some(label) = binding.hint {
                out.push(HintSpan::Text(label));
            }
        }
    }
}

// ── Raw bytes → chord ────────────────────────────────────────────────────────

/// Convert a raw PTY byte sequence into a [`KeyChord`] for dispatch through [`Keymap`]
/// as the crossterm surfaces.
///
/// Covers the supported subset of VT100 / xterm sequences:
/// printable ASCII, the common control bytes (`Ctrl+C`, `Ctrl+Q`, `Ctrl+\`,
/// `Ctrl+L`, `Ctrl+H`), Enter, Esc, Tab, and CSI / SS3 cursor-key sequences in
/// both legacy form (`\x1b[A-D`, `\x1bOA-D`) and kitty-extended form for
/// arrow modifiers (Alt-Shift-Arrow for resize). Returns `None` for sequences
/// outside the covered set so callers can fall back to legacy `match` arms
/// during migration.
#[must_use]
pub fn raw_bytes_to_chord(bytes: &[u8]) -> Option<KeyChord> {
    match bytes {
        // Specific control bytes handled before the generic range below.
        // Enter (\r or \n — 0x0d / 0x0a both in the Ctrl range)
        [b'\r' | b'\n'] => Some(KeyChord::plain(KeyCode::Enter)),
        // Esc (0x1b — just above the Ctrl range)
        [0x1b] => Some(KeyChord::plain(KeyCode::Esc)),
        // Tab (0x09 — inside Ctrl range; map to Tab, not Ctrl+I)
        [0x09] => Some(KeyChord::plain(KeyCode::Tab)),
        // Backspace: both the Ctrl+H byte (0x08) and the DEL byte (0x7f)
        [0x08 | 0x7f] => Some(KeyChord::plain(KeyCode::Backspace)),
        // Printable ASCII (0x20 .. 0x7e, single byte, no modifier)
        [b] if (0x20..=0x7e).contains(b) => Some(KeyChord::plain(KeyCode::Char(*b as char))),
        // Remaining single-byte control codes: Ctrl+A (0x01) through Ctrl+Z (0x1A),
        // minus the already-matched 0x08 (Backspace), 0x09 (Tab), 0x0a (LF), 0x0d (CR).
        // Formula: letter = 'a' + (byte - 1).
        [b @ 0x01..=0x1a] => {
            let letter = (b'a' + (b - 1)) as char;
            Some(KeyChord::ctrl(KeyCode::Char(letter)))
        }
        // Delete (CSI 3~)
        b"\x1b[3~" => Some(KeyChord::plain(KeyCode::Delete)),
        // CSI (legacy xterm/VT100) and SS3 (application cursor mode) arrows
        b"\x1b[A" | b"\x1bOA" => Some(KeyChord::plain(KeyCode::Up)),
        b"\x1b[B" | b"\x1bOB" => Some(KeyChord::plain(KeyCode::Down)),
        b"\x1b[C" | b"\x1bOC" => Some(KeyChord::plain(KeyCode::Right)),
        b"\x1b[D" | b"\x1bOD" => Some(KeyChord::plain(KeyCode::Left)),
        // Home / End variants
        b"\x1b[H" | b"\x1b[1~" => Some(KeyChord::plain(KeyCode::Home)),
        b"\x1b[F" | b"\x1b[4~" => Some(KeyChord::plain(KeyCode::End)),
        // Page Up / Down
        b"\x1b[5~" => Some(KeyChord::plain(KeyCode::PageUp)),
        b"\x1b[6~" => Some(KeyChord::plain(KeyCode::PageDown)),
        // CSI with modifier 4 = Alt-Shift — resize pane arrows
        b"\x1b[1;4A" => Some(KeyChord::alt_shift(KeyCode::Up)),
        b"\x1b[1;4B" => Some(KeyChord::alt_shift(KeyCode::Down)),
        b"\x1b[1;4C" => Some(KeyChord::alt_shift(KeyCode::Right)),
        b"\x1b[1;4D" => Some(KeyChord::alt_shift(KeyCode::Left)),
        _ => None,
    }
}

// ── Glyph derivation ─────────────────────────────────────────────────────────

/// Canonical display spellings for keys that appear in hints.
///
/// Every `KeyBinding.glyph` override and `HintSpan::Key` literal for these keys
/// must use these constants: one spelling per key, everywhere.
pub mod glyph {
    pub const TAB: &str = "\u{21e5}";
    pub const UP_DOWN: &str = "\u{2191}\u{2193}";
    pub const LEFT_RIGHT: &str = "\u{2190}\u{2192}";
    pub const ALL_ARROWS: &str = "\u{2191}\u{2193}\u{2190}\u{2192}";
    pub const ALT_SHIFT_ALL_ARROWS: &str = "Alt-Shift-\u{2191}\u{2193}\u{2190}\u{2192}";
    pub const PGUP_PGDN: &str = "PgUp/PgDn";
    pub const ESC: &str = "Esc";
    pub const ENTER: &str = "\u{21b5}";
}

/// Derive the hint-bar key glyph from a chord.
///
/// Reproduces the exact glyphs already in use across the codebase so output is
/// byte-identical to hand-written hints. Callers that need a *grouped* glyph
/// (e.g. `"↑↓"` for a pair of bindings) should set [`KeyBinding::glyph`]
/// instead of relying on this function.
///
/// Returns `""` when `chord` is `None`. Returns `"?"` for Char values not in
/// the common-shortcut set — callers must supply an explicit `glyph` for those.
#[must_use]
pub fn chord_glyph(chord: Option<KeyChord>) -> &'static str {
    let Some(chord) = chord else { return "" };
    match chord.key {
        KeyCode::Char(c) if chord.mods.contains(KeyModifiers::CONTROL) => {
            match c.to_ascii_lowercase() {
                'q' => "Ctrl-Q",
                'c' => "Ctrl-C",
                'l' => "Ctrl-L",
                'h' => "Ctrl-H",
                _ => "Ctrl-?",
            }
        }
        KeyCode::Char(c) if chord.mods.is_empty() || chord.mods == KeyModifiers::SHIFT => {
            match c.to_ascii_uppercase() {
                'A' => "A",
                'B' => "B",
                'C' => "C",
                'D' => "D",
                'E' => "E",
                'F' => "F",
                'G' => "G",
                'H' => "H",
                'I' => "I",
                'J' => "J",
                'K' => "K",
                'L' => "L",
                'M' => "M",
                'N' => "N",
                'O' => "O",
                'P' => "P",
                'Q' => "Q",
                'R' => "R",
                'S' => "S",
                'T' => "T",
                'U' => "U",
                'V' => "V",
                'W' => "W",
                'X' => "X",
                'Y' => "Y",
                'Z' => "Z",
                '*' => "*",
                '1' => "1",
                '2' => "2",
                '3' => "3",
                '4' => "4",
                _ => "?",
            }
        }
        KeyCode::Enter => glyph::ENTER,
        KeyCode::Esc => glyph::ESC,
        KeyCode::Tab => glyph::TAB,
        KeyCode::BackTab => "\u{21e4}", // ⇤
        KeyCode::Up => "\u{2191}",      // ↑
        KeyCode::Down => "\u{2193}",    // ↓
        KeyCode::Left => "\u{2190}",    // ←
        KeyCode::Right => "\u{2192}",   // →
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PgUp",
        KeyCode::PageDown => "PgDn",
        KeyCode::Backspace => "⌫",
        KeyCode::Delete => "Del",
        // Other modifier combos on Char (e.g. Alt-Shift-Arrow converted as Char)
        // are not in the common-shortcut set — callers must supply an explicit glyph.
        KeyCode::Char(_) => "?",
        KeyCode::Unknown => "",
    }
}

// ── Scroll hint keymap ───────────────────────────────────────────────────────

/// Axis discriminant for [`SCROLL_HINT_KEYMAP`].
///
/// The action type is never used for dispatch; `SCROLL_HINT_KEYMAP` exists
/// solely to produce axis-gated [`HintSpan`] sequences via
/// [`Keymap::hint_spans_for_axes`], eliminating the duplicate gating logic
/// that previously lived in `scroll_hint_spans`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollHintAxis {
    Vertical,
    Horizontal,
}

/// Hint-only keymap for the two scroll axes.
///
/// Each binding carries a pre-composed combined glyph (`"↑↓/j/k"`,
/// `"←→/h/l"`). The chords drive axis-gating in
/// The same axis gate used by [`Keymap::hint_spans_for_axes`] applies: a binding whose chords are all `Up`/`Down`
/// is suppressed when `axes.vertical` is false; one whose chords are all
/// `Left`/`Right` is suppressed when `axes.horizontal` is false.
///
/// Use via [`Keymap::hint_spans_for_axes`]. Never call
/// [`Keymap::dispatch`] on this keymap — both Up and Down map to
/// `ScrollHintAxis::Vertical`, so the return value has no directional meaning.
pub static SCROLL_HINT_KEYMAP: Keymap<ScrollHintAxis> = Keymap::new(&[
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
        action: ScrollHintAxis::Vertical,
        hint: Some("scroll"),
        visibility: Visibility::Shown,
        glyph: Some("↑↓/j/k"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
        ],
        action: ScrollHintAxis::Horizontal,
        hint: Some("scroll"),
        visibility: Visibility::Shown,
        glyph: Some("←→/h/l"),
    },
]);

#[cfg(test)]
mod tests;
