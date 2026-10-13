// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Keybinding registry — single source of truth coupling key dispatch and hint advertisement.
//!
//! A [`Keymap<A>`] binds each action to one or more key chords and a hint label. The
//! dispatcher matches incoming keys against the table; the hint renderer produces
//! [`HintSpan`] sequences from the same table. Divergence between handled keys and
//! advertised keys is therefore structurally impossible for [`Visibility::Shown`] and
//! [`Visibility::HiddenAlias`] bindings.

use std::borrow::Cow;

use crate::input::{KeyCode, KeyModifiers};
use crate::scroll::ScrollAxes;
use crate::widgets::HintSpan;

/// A key chord: a logical key plus zero or more modifier bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyChord {
    /// Key code in this chord.
    pub key: KeyCode,
    /// Modifier flags required by this chord.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct KeyBinding<A: Clone + 'static> {
    chords: Cow<'static, [KeyChord]>,
    action: A,
    hint: Option<Cow<'static, str>>,
    visibility: Visibility,
    glyph: Option<Cow<'static, str>>,
}

impl<A: Clone + 'static> KeyBinding<A> {
    /// Creates a const binding that borrows all static text and chords.
    pub const fn borrowed(
        chords: &'static [KeyChord],
        action: A,
        hint: Option<&'static str>,
        visibility: Visibility,
        glyph: Option<&'static str>,
    ) -> Self {
        Self {
            chords: Cow::Borrowed(chords),
            action,
            hint: match hint {
                Some(hint) => Some(Cow::Borrowed(hint)),
                None => None,
            },
            visibility,
            glyph: match glyph {
                Some(glyph) => Some(Cow::Borrowed(glyph)),
                None => None,
            },
        }
    }

    /// Creates a binding that owns runtime-loaded chords and text.
    pub fn owned(
        chords: Vec<KeyChord>,
        action: A,
        hint: Option<String>,
        visibility: Visibility,
        glyph: Option<String>,
    ) -> Self {
        Self {
            chords: Cow::Owned(chords),
            action,
            hint: hint.map(Cow::Owned),
            visibility,
            glyph: glyph.map(Cow::Owned),
        }
    }

    /// Returns every chord that fires this action.
    pub fn chords(&self) -> &[KeyChord] {
        &self.chords
    }

    /// Returns the action produced by this binding.
    pub const fn action(&self) -> &A {
        &self.action
    }

    /// Returns the optional hint label.
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// Returns this binding's hint visibility.
    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Returns the optional explicit glyph.
    pub fn glyph(&self) -> Option<&str> {
        self.glyph.as_deref()
    }
}

/// A keymap binding all chords to actions for a single widget context.
///
/// # Examples
///
/// ```
/// use termrock::{
///     input::KeyCode,
///     keymap::{KeyBinding, KeyChord, Keymap, Visibility},
/// };
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// enum Action { Quit }
/// static BINDINGS: &[KeyBinding<Action>] = &[KeyBinding::borrowed(
///     &[KeyChord::plain(KeyCode::Char('q'))],
///     Action::Quit,
///     Some("quit"),
///     Visibility::Shown,
///     None,
/// )];
///
/// let keymap = Keymap::from_static(BINDINGS);
/// assert_eq!(keymap.dispatch(KeyChord::plain(KeyCode::Char('q'))), Some(Action::Quit));
///
/// let mut runtime_keymap = keymap.clone();
/// runtime_keymap.remap(Action::Quit, vec![KeyChord::ctrl(KeyCode::Char('c'))]);
/// assert_eq!(runtime_keymap.glyph_for(Action::Quit), "Ctrl-C");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap<A: Clone + 'static> {
    bindings: Cow<'static, [KeyBinding<A>]>,
}

impl<A: Clone + Copy + 'static> Keymap<A> {
    /// Construct a keymap from a static binding slice.
    #[must_use]
    pub const fn from_static(bindings: &'static [KeyBinding<A>]) -> Self {
        Self {
            bindings: Cow::Borrowed(bindings),
        }
    }

    /// Constructs a keymap that owns runtime-loaded bindings.
    pub fn from_owned(bindings: Vec<KeyBinding<A>>) -> Self {
        Self {
            bindings: Cow::Owned(bindings),
        }
    }

    /// Return all bindings in declaration order (for testing and introspection).
    #[must_use]
    pub fn bindings(&self) -> &[KeyBinding<A>] {
        &self.bindings
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
            match &binding.glyph {
                Some(Cow::Borrowed(glyph)) => spans.push(HintSpan::Key(glyph)),
                Some(Cow::Owned(glyph)) => spans.push(HintSpan::DynKey(glyph.clone())),
                None => spans.push(HintSpan::Key(chord_glyph(binding.chords.first().copied()))),
            }
            match &binding.hint {
                Some(Cow::Borrowed(label)) => spans.push(HintSpan::Text(label)),
                Some(Cow::Owned(label)) => spans.push(HintSpan::Dyn(label.clone())),
                None => {}
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

impl<A: Clone + Copy + PartialEq + 'static> Keymap<A> {
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
    pub fn glyph_for(&self, action: A) -> Cow<'_, str> {
        self.binding_for(action).map_or(Cow::Borrowed(""), |b| {
            b.glyph
                .as_deref()
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Borrowed(chord_glyph(b.chords.first().copied())))
        })
    }

    /// Push the `Key` (and optional `Text`) spans for `action` onto `out`,
    /// derived from the binding. No separators are added — the caller owns
    /// layout. Does nothing if the action is unbound. The single primitive
    /// for building context-dependent footers from a keymap.
    pub fn push_spans_for(&self, action: A, out: &mut Vec<HintSpan<'static>>) {
        if let Some(binding) = self.binding_for(action) {
            match &binding.glyph {
                Some(Cow::Borrowed(glyph)) => out.push(HintSpan::Key(glyph)),
                Some(Cow::Owned(glyph)) => out.push(HintSpan::DynKey(glyph.clone())),
                None => out.push(HintSpan::Key(chord_glyph(binding.chords.first().copied()))),
            }
            match &binding.hint {
                Some(Cow::Borrowed(label)) => out.push(HintSpan::Text(label)),
                Some(Cow::Owned(label)) => out.push(HintSpan::Dyn(label.clone())),
                None => {}
            }
        }
    }

    /// Replaces an action's chord set and clears any now-stale explicit glyph.
    ///
    /// A static map clones only on its first successful edit. Hints immediately
    /// derive a canonical glyph from the replacement's first chord.
    pub fn remap(&mut self, action: A, chords: Vec<KeyChord>) -> bool {
        let Some(index) = self
            .bindings
            .iter()
            .position(|binding| binding.action == action)
        else {
            return false;
        };
        let binding = &mut self.bindings.to_mut()[index];
        binding.chords = Cow::Owned(chords);
        binding.glyph = None;
        true
    }

    /// Replaces the first binding for `action` with a complete binding.
    pub fn replace(&mut self, action: A, binding: KeyBinding<A>) -> bool {
        let Some(index) = self
            .bindings
            .iter()
            .position(|binding| binding.action == action)
        else {
            return false;
        };
        self.bindings.to_mut()[index] = binding;
        true
    }

    /// Removes every binding for an action.
    pub fn disable(&mut self, action: A) -> bool {
        if !self.bindings.iter().any(|binding| binding.action == action) {
            return false;
        }
        let bindings = self.bindings.to_mut();
        bindings.retain(|binding| binding.action != action);
        true
    }

    /// Finds shared chords between distinct bindings in declaration order.
    pub fn conflicts(&self) -> Vec<Conflict<'_, A>> {
        let mut conflicts = Vec::new();
        for (left_index, left) in self.bindings.iter().enumerate() {
            for right in &self.bindings[left_index + 1..] {
                let mut seen = Vec::new();
                for chord in left.chords.iter().copied() {
                    if right.chords.contains(&chord) && !seen.contains(&chord) {
                        seen.push(chord);
                        conflicts.push(Conflict {
                            first: &left.action,
                            second: &right.action,
                            chord,
                        });
                    }
                }
            }
        }
        conflicts
    }
}

/// One diagnostic collision between two declaration-ordered bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conflict<'a, A> {
    /// Action from the earlier binding.
    pub first: &'a A,
    /// Action from the later binding.
    pub second: &'a A,
    /// Chord shared by both bindings.
    pub chord: KeyChord,
}

#[cfg(feature = "serde")]
impl<'de, A> serde::Deserialize<'de> for KeyBinding<A>
where
    A: Clone + serde::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct OwnedBinding<A> {
            chords: Vec<KeyChord>,
            action: A,
            hint: Option<String>,
            visibility: Visibility,
            glyph: Option<String>,
        }

        let binding = OwnedBinding::deserialize(deserializer)?;
        Ok(Self::owned(
            binding.chords,
            binding.action,
            binding.hint,
            binding.visibility,
            binding.glyph,
        ))
    }
}

#[cfg(feature = "serde")]
impl<A> serde::Serialize for Keymap<A>
where
    A: Clone + serde::Serialize + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.bindings.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, A> serde::Deserialize<'de> for Keymap<A>
where
    A: Clone + Copy + serde::Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<KeyBinding<A>>::deserialize(deserializer).map(Self::from_owned)
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
    /// Canonical Tab-key hint glyph.
    pub const TAB: &str = "\u{21e5}";
    /// Canonical grouped vertical-arrow hint glyph.
    pub const UP_DOWN: &str = "\u{2191}\u{2193}";
    /// Canonical grouped horizontal-arrow hint glyph.
    pub const LEFT_RIGHT: &str = "\u{2190}\u{2192}";
    /// Canonical grouped four-direction hint glyph.
    pub const ALL_ARROWS: &str = "\u{2191}\u{2193}\u{2190}\u{2192}";
    /// Canonical Alt-Shift four-direction resize hint.
    pub const ALT_SHIFT_ALL_ARROWS: &str = "Alt-Shift-\u{2191}\u{2193}\u{2190}\u{2192}";
    /// Canonical paired page-navigation hint.
    pub const PGUP_PGDN: &str = "PgUp/PgDn";
    /// Canonical Escape-key hint label.
    pub const ESC: &str = "Esc";
    /// Canonical Enter-key hint glyph.
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
    /// The vertical terminal axis.
    Vertical,
    /// The horizontal terminal axis.
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
static SCROLL_HINT_BINDINGS: &[KeyBinding<ScrollHintAxis>] = &[
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
        ScrollHintAxis::Vertical,
        Some("scroll"),
        Visibility::Shown,
        Some("↑↓/j/k"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
        ],
        ScrollHintAxis::Horizontal,
        Some("scroll"),
        Visibility::Shown,
        Some("←→/h/l"),
    ),
];

/// Hint-only keymap for both scroll axes, filtered at render time by available axes.
pub static SCROLL_HINT_KEYMAP: Keymap<ScrollHintAxis> = Keymap::from_static(SCROLL_HINT_BINDINGS);

#[cfg(test)]
mod tests;
