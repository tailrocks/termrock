// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::geometry::HintSpan;
use crate::scroll::ScrollAxes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestAction {
    Confirm,
    Cancel,
    Navigate,
    HiddenVim,
}

const TEST_BINDINGS: &[KeyBinding<TestAction>] = &[
    KeyBinding {
        chords: &[KeyChord::plain(LogicalKey::Enter)],
        action: TestAction::Confirm,
        hint: Some("confirm"),
        visibility: Visibility::Shown,
        glyph: None,
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(LogicalKey::Esc),
            KeyChord::plain(LogicalKey::Char('n')),
            KeyChord::plain(LogicalKey::Char('N')),
        ],
        action: TestAction::Cancel,
        hint: Some("cancel"),
        visibility: Visibility::Shown,
        glyph: Some("N/Esc"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(LogicalKey::Up),
            KeyChord::plain(LogicalKey::Down),
        ],
        action: TestAction::Navigate,
        hint: Some("navigate"),
        visibility: Visibility::Shown,
        glyph: Some("\u{2191}\u{2193}"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(LogicalKey::Char('k')),
            KeyChord::plain(LogicalKey::Char('j')),
        ],
        action: TestAction::HiddenVim,
        hint: None,
        visibility: Visibility::HiddenAlias,
        glyph: None,
    },
];

static TEST_MAP: Keymap<TestAction> = Keymap::new(TEST_BINDINGS);

#[test]
fn dispatch_finds_primary_chord() {
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(LogicalKey::Enter)),
        Some(TestAction::Confirm)
    );
}

#[test]
fn dispatch_finds_alias_chord() {
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(LogicalKey::Esc)),
        Some(TestAction::Cancel)
    );
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(LogicalKey::Char('n'))),
        Some(TestAction::Cancel)
    );
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(LogicalKey::Char('k'))),
        Some(TestAction::HiddenVim)
    );
}

#[test]
fn dispatch_returns_none_for_unbound_chord() {
    assert_eq!(TEST_MAP.dispatch(KeyChord::plain(LogicalKey::Tab)), None);
}

#[test]
fn hint_spans_only_includes_shown_bindings() {
    let spans = TEST_MAP.hint_spans();
    let keys: Vec<&str> = spans
        .iter()
        .filter_map(|s| {
            if let HintSpan::Key(k) = s {
                Some(*k)
            } else {
                None
            }
        })
        .collect();
    assert!(keys.contains(&"\u{21b5}"), "should have Enter glyph (↵)");
    assert!(keys.contains(&"N/Esc"), "should have glyph override");
    assert!(
        keys.contains(&"\u{2191}\u{2193}"),
        "should have grouped arrow glyph (↑↓)"
    );
    // HiddenAlias should be absent
    assert!(!keys.contains(&"K"), "vim alias should not appear");
}

#[test]
fn hint_spans_for_axes_omits_arrows_when_no_scroll() {
    let axes = ScrollAxes {
        vertical: false,
        horizontal: false,
    };
    let spans = TEST_MAP.hint_spans_for_axes(axes);
    let keys: Vec<&str> = spans
        .iter()
        .filter_map(|s| {
            if let HintSpan::Key(k) = s {
                Some(*k)
            } else {
                None
            }
        })
        .collect();
    assert!(
        !keys.contains(&"\u{2191}\u{2193}"),
        "arrow group must be omitted when no scroll"
    );
    assert!(keys.contains(&"\u{21b5}"), "Enter must still be shown");
}

#[test]
fn hint_spans_for_axes_includes_arrows_when_vertical_available() {
    let axes = ScrollAxes {
        vertical: true,
        horizontal: false,
    };
    let spans = TEST_MAP.hint_spans_for_axes(axes);
    let keys: Vec<&str> = spans
        .iter()
        .filter_map(|s| {
            if let HintSpan::Key(k) = s {
                Some(*k)
            } else {
                None
            }
        })
        .collect();
    assert!(
        keys.contains(&"\u{2191}\u{2193}"),
        "arrow group must show when vertical available"
    );
}

#[test]
fn chord_glyph_reproduces_existing_glyphs() {
    assert_eq!(
        chord_glyph(Some(KeyChord::ctrl(LogicalKey::Char('q')))),
        "Ctrl-Q"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::ctrl(LogicalKey::Char('c')))),
        "Ctrl-C"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Enter))),
        glyph::ENTER
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Esc))),
        glyph::ESC
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Tab))),
        glyph::TAB
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Up))),
        "\u{2191}"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Down))),
        "\u{2193}"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Char('y')))),
        "Y"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Char('Y')))),
        "Y"
    );
    assert_eq!(chord_glyph(None), "");
}

#[test]
fn canonical_glyph_constants_reject_known_drift_spellings() {
    assert_ne!(glyph::TAB, concat!("T", "ab"));
    assert_ne!(glyph::UP_DOWN, "\u{2191}/\u{2193}");
    assert_ne!(glyph::LEFT_RIGHT, "\u{2190}/\u{2192}");
    assert_ne!(glyph::PGUP_PGDN, concat!("PgUp", " PgDn"));
    assert!(!glyph::ALL_ARROWS.contains('+'));
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Tab))),
        glyph::TAB
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Esc))),
        glyph::ESC
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(LogicalKey::Enter))),
        glyph::ENTER
    );
}

#[test]
fn mods_bit_flags_combine_correctly() {
    let ctrl_shift = Mods::NONE.with_ctrl().with_shift();
    assert!(ctrl_shift.contains(Mods::CTRL));
    assert!(ctrl_shift.contains(Mods::SHIFT));
    assert!(!ctrl_shift.contains(Mods::ALT));
    assert!(!ctrl_shift.is_empty());
    assert!(Mods::NONE.is_empty());
}

#[test]
fn from_crossterm_key_event_converts_basic_keys() {
    use crate::input::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let ev = KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let chord = KeyChord::from(ev);
    assert_eq!(chord.key, LogicalKey::Char('q'));
    assert!(chord.mods.contains(Mods::CTRL));

    let ev2 = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(KeyChord::from(ev2), KeyChord::plain(LogicalKey::Enter));
}

// ── raw_bytes_to_chord ────────────────────────────────────────────────────────

#[test]
fn raw_bytes_enter_and_escape() {
    assert_eq!(
        raw_bytes_to_chord(b"\r"),
        Some(KeyChord::plain(LogicalKey::Enter))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\n"),
        Some(KeyChord::plain(LogicalKey::Enter))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b"),
        Some(KeyChord::plain(LogicalKey::Esc))
    );
}

#[test]
fn raw_bytes_tab_and_backspace() {
    assert_eq!(
        raw_bytes_to_chord(b"\t"),
        Some(KeyChord::plain(LogicalKey::Tab))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x08"),
        Some(KeyChord::plain(LogicalKey::Backspace))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x7f"),
        Some(KeyChord::plain(LogicalKey::Backspace))
    );
}

#[test]
fn raw_bytes_printable_ascii() {
    assert_eq!(
        raw_bytes_to_chord(b"y"),
        Some(KeyChord::plain(LogicalKey::Char('y')))
    );
    assert_eq!(
        raw_bytes_to_chord(b"N"),
        Some(KeyChord::plain(LogicalKey::Char('N')))
    );
}

#[test]
fn raw_bytes_ctrl_c() {
    assert_eq!(
        raw_bytes_to_chord(b"\x03"),
        Some(KeyChord::ctrl(LogicalKey::Char('c')))
    );
}

#[test]
fn raw_bytes_csi_and_ss3_arrows() {
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[A"),
        Some(KeyChord::plain(LogicalKey::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[B"),
        Some(KeyChord::plain(LogicalKey::Down))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[C"),
        Some(KeyChord::plain(LogicalKey::Right))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[D"),
        Some(KeyChord::plain(LogicalKey::Left))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1bOA"),
        Some(KeyChord::plain(LogicalKey::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1bOD"),
        Some(KeyChord::plain(LogicalKey::Left))
    );
}

#[test]
fn raw_bytes_unknown_returns_none() {
    assert_eq!(raw_bytes_to_chord(b"\x1b[200~"), None);
    assert_eq!(raw_bytes_to_chord(b"\x00"), None);
}

#[test]
fn raw_bytes_csi_alt_shift_arrows() {
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4A"),
        Some(KeyChord::alt_shift(LogicalKey::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4B"),
        Some(KeyChord::alt_shift(LogicalKey::Down))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4C"),
        Some(KeyChord::alt_shift(LogicalKey::Right))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4D"),
        Some(KeyChord::alt_shift(LogicalKey::Left))
    );
}
