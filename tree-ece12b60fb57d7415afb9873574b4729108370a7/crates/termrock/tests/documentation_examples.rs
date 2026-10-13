// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Executable mirrors of the crate's public documentation examples.

use ratatui_core::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
};
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::Outcome,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility},
    osc::{PointerShape, Request, encode},
    style::Role,
    widgets::{Anchor, List, ListRow, ListState, RowRole, Severity, Toast},
};

#[test]
fn toast_documentation_example() {
    let theme = Theme::default();
    let toast = Toast::new(&theme, "Saved", Severity::Success)
        .anchor(Anchor::BottomRight)
        .margins(1, 1);
    assert!(toast.rect(Rect::new(0, 0, 40, 8)).is_some());
}

#[test]
fn list_documentation_example() {
    let rows = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let theme = Theme::default();
    let _widget = List::new(&rows, &theme);
    let mut state = ListState::new(Some("a"));
    let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert!(matches!(outcome, Outcome::Changed));
    assert_eq!(state.selected(), Some(&"b"));
}

#[test]
fn theme_documentation_example() {
    let theme = Theme::default().with_role(Role::Accent, Style::new().fg(Color::Cyan));
    assert_eq!(theme.style(Role::Accent).fg, Some(Color::Cyan));
}

#[test]
fn osc_documentation_example() {
    assert_eq!(
        encode(Request::Pointer(PointerShape::Pointer)),
        b"\x1b]22;pointer\x1b\\",
    );
}

#[test]
fn keymap_documentation_example() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Quit,
    }
    static BINDINGS: &[KeyBinding<Action>] = &[KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('q'))],
        Action::Quit,
        Some("quit"),
        Visibility::Shown,
        None,
    )];

    let keymap = Keymap::from_static(BINDINGS);
    assert_eq!(
        keymap.dispatch(KeyChord::plain(KeyCode::Char('q'))),
        Some(Action::Quit)
    );

    let mut runtime_keymap = keymap.clone();
    runtime_keymap.remap(Action::Quit, vec![KeyChord::ctrl(KeyCode::Char('c'))]);
    assert_eq!(runtime_keymap.glyph_for(Action::Quit), "Ctrl-C");
}
