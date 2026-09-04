// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::scroll::ScrollAxes;
use crate::widgets::HintSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum TestAction {
    Confirm,
    Cancel,
    Navigate,
    HiddenVim,
    Missing,
}

const CONST_BORROWED_MAP: Keymap<TestAction> = Keymap::from_static(TEST_BINDINGS);

#[test]
fn static_construction_is_const_and_stays_borrowed_until_a_successful_edit() {
    let mut keymap = CONST_BORROWED_MAP.clone();
    assert!(matches!(keymap.bindings, std::borrow::Cow::Borrowed(_)));

    assert!(!keymap.remap(TestAction::Missing, Vec::new()));
    assert!(!keymap.disable(TestAction::Missing));
    assert!(!keymap.replace(
        TestAction::Missing,
        KeyBinding::owned(
            Vec::new(),
            TestAction::Missing,
            None,
            Visibility::Internal,
            None,
        ),
    ));
    assert!(matches!(keymap.bindings, std::borrow::Cow::Borrowed(_)));

    assert!(keymap.disable(TestAction::Cancel));
    assert!(matches!(keymap.bindings, std::borrow::Cow::Owned(_)));
}

#[test]
fn runtime_remap_updates_dispatch_hints_and_glyphs_from_one_binding() {
    let mut keymap = TEST_MAP.clone();
    assert!(keymap.remap(
        TestAction::Confirm,
        vec![KeyChord::ctrl(KeyCode::Char('c'))]
    ));

    assert_eq!(keymap.dispatch(KeyChord::plain(KeyCode::Enter)), None);
    assert_eq!(
        keymap.dispatch(KeyChord::ctrl(KeyCode::Char('c'))),
        Some(TestAction::Confirm)
    );
    assert_eq!(keymap.glyph_for(TestAction::Confirm), "Ctrl-C");
    assert!(matches!(
        keymap.hint_spans().first(),
        Some(HintSpan::Key("Ctrl-C"))
    ));
}

#[test]
fn disable_and_replace_mutate_the_same_resolved_table() {
    let mut keymap = TEST_MAP.clone();
    assert!(keymap.disable(TestAction::Cancel));
    assert_eq!(keymap.binding_for(TestAction::Cancel), None);
    assert_eq!(keymap.dispatch(KeyChord::plain(KeyCode::Esc)), None);

    assert!(keymap.replace(
        TestAction::Confirm,
        KeyBinding::owned(
            vec![KeyChord::plain(KeyCode::Char('y'))],
            TestAction::Confirm,
            Some("yes".to_owned()),
            Visibility::Shown,
            Some("Y".to_owned()),
        )
    ));
    assert_eq!(keymap.glyph_for(TestAction::Confirm), "Y");
}

#[test]
fn conflicts_report_order_while_dispatch_remains_first_binding_wins() {
    let chord = KeyChord::plain(KeyCode::Enter);
    let keymap = Keymap::from_owned(vec![
        KeyBinding::owned(
            vec![chord],
            TestAction::Confirm,
            None,
            Visibility::Internal,
            None,
        ),
        KeyBinding::owned(
            vec![chord],
            TestAction::Cancel,
            None,
            Visibility::Internal,
            None,
        ),
    ]);

    assert_eq!(keymap.dispatch(chord), Some(TestAction::Confirm));
    assert_eq!(
        keymap.conflicts(),
        [Conflict {
            first: &TestAction::Confirm,
            second: &TestAction::Cancel,
            chord,
        }]
    );
}

#[test]
fn repeated_chords_emit_one_conflict_per_binding_pair_and_chord() {
    let chord = KeyChord::plain(KeyCode::Enter);
    let keymap = Keymap::from_owned(vec![
        KeyBinding::owned(
            vec![chord, chord],
            TestAction::Confirm,
            None,
            Visibility::Internal,
            None,
        ),
        KeyBinding::owned(
            vec![chord],
            TestAction::Cancel,
            None,
            Visibility::Internal,
            None,
        ),
    ]);

    assert_eq!(keymap.conflicts().len(), 1);
}

fn owned_test_map() -> Keymap<TestAction> {
    Keymap::from_owned(
        TEST_BINDINGS
            .iter()
            .map(|binding| {
                KeyBinding::owned(
                    binding.chords().to_vec(),
                    *binding.action(),
                    binding.hint().map(str::to_owned),
                    binding.visibility(),
                    binding.glyph().map(str::to_owned),
                )
            })
            .collect(),
    )
}

fn assert_complete_runtime_contract(mut keymap: Keymap<TestAction>) {
    assert!(keymap.hint_spans_for_axes(ScrollAxes::none()).len() < keymap.hint_spans().len());

    let remapped = KeyChord::ctrl(KeyCode::Char('c'));
    assert!(keymap.remap(TestAction::Confirm, vec![remapped]));
    assert_eq!(keymap.dispatch(KeyChord::plain(KeyCode::Enter)), None);
    assert_eq!(keymap.dispatch(remapped), Some(TestAction::Confirm));
    assert_eq!(keymap.glyph_for(TestAction::Confirm), "Ctrl-C");

    assert!(keymap.replace(
        TestAction::Cancel,
        KeyBinding::owned(
            vec![remapped],
            TestAction::Cancel,
            Some("cancel".to_owned()),
            Visibility::Shown,
            None,
        ),
    ));
    assert_eq!(keymap.dispatch(remapped), Some(TestAction::Confirm));
    assert_eq!(keymap.conflicts().len(), 1);

    assert!(keymap.disable(TestAction::Cancel));
    assert!(keymap.conflicts().is_empty());
}

#[test]
fn borrowed_and_owned_maps_share_the_complete_runtime_contract() {
    assert_complete_runtime_contract(TEST_MAP.clone());
    assert_complete_runtime_contract(owned_test_map());
}

#[test]
fn owned_axis_bindings_use_the_same_filtering_contract() {
    let keymap = Keymap::from_owned(vec![KeyBinding::owned(
        vec![KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
        TestAction::Navigate,
        Some("navigate".to_owned()),
        Visibility::Shown,
        Some("↑↓".to_owned()),
    )]);

    assert!(keymap.hint_spans_for_axes(ScrollAxes::none()).is_empty());
    assert!(
        !keymap
            .hint_spans_for_axes(ScrollAxes {
                vertical: true,
                horizontal: false,
            })
            .is_empty()
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_deserialization_produces_an_owned_runtime_map() {
    let json = serde_json::to_string(&TEST_MAP).expect("serialize borrowed keymap");
    let mut decoded: Keymap<TestAction> =
        serde_json::from_str(&json).expect("deserialize owned keymap");

    assert!(matches!(decoded.bindings, std::borrow::Cow::Owned(_)));
    assert!(decoded.remap(
        TestAction::Confirm,
        vec![KeyChord::ctrl(KeyCode::Char('c'))]
    ));
    assert_eq!(
        decoded.dispatch(KeyChord::ctrl(KeyCode::Char('c'))),
        Some(TestAction::Confirm)
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_unknown_modifier_bits() {
    let error = serde_json::from_str::<KeyModifiers>("8").expect_err("bit 3 is not a modifier");
    assert!(error.to_string().contains("unknown key modifier bits"));
}

const TEST_BINDINGS: &[KeyBinding<TestAction>] = &[
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Enter)],
        TestAction::Confirm,
        Some("confirm"),
        Visibility::Shown,
        None,
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Esc),
            KeyChord::plain(KeyCode::Char('n')),
            KeyChord::plain(KeyCode::Char('N')),
        ],
        TestAction::Cancel,
        Some("cancel"),
        Visibility::Shown,
        Some("N/Esc"),
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
        TestAction::Navigate,
        Some("navigate"),
        Visibility::Shown,
        Some("\u{2191}\u{2193}"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TestAction::HiddenVim,
        None,
        Visibility::HiddenAlias,
        None,
    ),
];

static TEST_MAP: Keymap<TestAction> = Keymap::from_static(TEST_BINDINGS);

const CTRL_BINDINGS: &[KeyBinding<TestAction>] = &[KeyBinding::borrowed(
    &[KeyChord::ctrl(KeyCode::Char('x'))],
    TestAction::Confirm,
    None,
    Visibility::Internal,
    None,
)];

static CTRL_MAP: Keymap<TestAction> = Keymap::from_static(CTRL_BINDINGS);

#[test]
fn binding_tables_do_not_use_unknown_keys() {
    assert!(
        TEST_BINDINGS
            .iter()
            .flat_map(KeyBinding::chords)
            .all(|chord| chord.key != KeyCode::Unknown)
    );
}

#[test]
fn key_event_conversion_preserves_unified_modifiers() {
    let chord = KeyChord::from(crate::input::KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::CONTROL,
    ));

    assert_eq!(chord, KeyChord::ctrl(KeyCode::Char('x')));
    assert_eq!(CTRL_MAP.dispatch(chord), Some(TestAction::Confirm));
}

#[test]
fn dispatch_finds_primary_chord() {
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(KeyCode::Enter)),
        Some(TestAction::Confirm)
    );
}

#[test]
fn dispatch_finds_alias_chord() {
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(KeyCode::Esc)),
        Some(TestAction::Cancel)
    );
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(KeyCode::Char('n'))),
        Some(TestAction::Cancel)
    );
    assert_eq!(
        TEST_MAP.dispatch(KeyChord::plain(KeyCode::Char('k'))),
        Some(TestAction::HiddenVim)
    );
}

#[test]
fn dispatch_returns_none_for_unbound_chord() {
    assert_eq!(TEST_MAP.dispatch(KeyChord::plain(KeyCode::Tab)), None);
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
        chord_glyph(Some(KeyChord::ctrl(KeyCode::Char('q')))),
        "Ctrl-Q"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::ctrl(KeyCode::Char('c')))),
        "Ctrl-C"
    );
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(KeyCode::Enter))),
        glyph::ENTER
    );
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Esc))), glyph::ESC);
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Tab))), glyph::TAB);
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Up))), "\u{2191}");
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(KeyCode::Down))),
        "\u{2193}"
    );
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Char('y')))), "Y");
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Char('Y')))), "Y");
    assert_eq!(chord_glyph(None), "");
}

#[test]
fn canonical_glyph_constants_reject_known_drift_spellings() {
    assert_ne!(glyph::TAB, concat!("T", "ab"));
    assert_ne!(glyph::UP_DOWN, "\u{2191}/\u{2193}");
    assert_ne!(glyph::LEFT_RIGHT, "\u{2190}/\u{2192}");
    assert_ne!(glyph::PGUP_PGDN, concat!("PgUp", " PgDn"));
    assert!(!glyph::ALL_ARROWS.contains('+'));
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Tab))), glyph::TAB);
    assert_eq!(chord_glyph(Some(KeyChord::plain(KeyCode::Esc))), glyph::ESC);
    assert_eq!(
        chord_glyph(Some(KeyChord::plain(KeyCode::Enter))),
        glyph::ENTER
    );
}

#[test]
fn mods_bit_flags_combine_correctly() {
    let ctrl_shift = KeyModifiers::NONE.with_ctrl().with_shift();
    assert!(ctrl_shift.contains(KeyModifiers::CONTROL));
    assert!(ctrl_shift.contains(KeyModifiers::SHIFT));
    assert!(!ctrl_shift.contains(KeyModifiers::ALT));
    assert!(!ctrl_shift.is_empty());
    assert!(KeyModifiers::NONE.is_empty());
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
    assert_eq!(chord.key, KeyCode::Char('q'));
    assert!(chord.mods.contains(KeyModifiers::CONTROL));

    let ev2 = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(KeyChord::from(ev2), KeyChord::plain(KeyCode::Enter));
}

// ── raw_bytes_to_chord ────────────────────────────────────────────────────────

#[test]
fn raw_bytes_enter_and_escape() {
    assert_eq!(
        raw_bytes_to_chord(b"\r"),
        Some(KeyChord::plain(KeyCode::Enter))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\n"),
        Some(KeyChord::plain(KeyCode::Enter))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b"),
        Some(KeyChord::plain(KeyCode::Esc))
    );
}

#[test]
fn raw_bytes_tab_and_backspace() {
    assert_eq!(
        raw_bytes_to_chord(b"\t"),
        Some(KeyChord::plain(KeyCode::Tab))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x08"),
        Some(KeyChord::plain(KeyCode::Backspace))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x7f"),
        Some(KeyChord::plain(KeyCode::Backspace))
    );
}

#[test]
fn raw_bytes_printable_ascii() {
    assert_eq!(
        raw_bytes_to_chord(b"y"),
        Some(KeyChord::plain(KeyCode::Char('y')))
    );
    assert_eq!(
        raw_bytes_to_chord(b"N"),
        Some(KeyChord::plain(KeyCode::Char('N')))
    );
}

#[test]
fn raw_bytes_ctrl_c() {
    assert_eq!(
        raw_bytes_to_chord(b"\x03"),
        Some(KeyChord::ctrl(KeyCode::Char('c')))
    );
}

#[test]
fn raw_bytes_csi_and_ss3_arrows() {
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[A"),
        Some(KeyChord::plain(KeyCode::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[B"),
        Some(KeyChord::plain(KeyCode::Down))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[C"),
        Some(KeyChord::plain(KeyCode::Right))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[D"),
        Some(KeyChord::plain(KeyCode::Left))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1bOA"),
        Some(KeyChord::plain(KeyCode::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1bOD"),
        Some(KeyChord::plain(KeyCode::Left))
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
        Some(KeyChord::alt_shift(KeyCode::Up))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4B"),
        Some(KeyChord::alt_shift(KeyCode::Down))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4C"),
        Some(KeyChord::alt_shift(KeyCode::Right))
    );
    assert_eq!(
        raw_bytes_to_chord(b"\x1b[1;4D"),
        Some(KeyChord::alt_shift(KeyCode::Left))
    );
}
