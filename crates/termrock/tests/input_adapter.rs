#![cfg(feature = "crossterm")]

use crossterm::event::{KeyCode as CrosstermKeyCode, MediaKeyCode};
use ratatui_core::text::Line;
use termrock::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::Outcome,
    widgets::{
        Action, ChoiceDialogState, ListOutcome, ListRow, ListState, RowRole, TextInputOutcome,
        TextInputState,
    },
};

#[test]
fn every_mapped_crossterm_key_roundtrips() {
    let pairs = [
        (CrosstermKeyCode::Backspace, KeyCode::Backspace),
        (CrosstermKeyCode::Enter, KeyCode::Enter),
        (CrosstermKeyCode::Left, KeyCode::Left),
        (CrosstermKeyCode::Right, KeyCode::Right),
        (CrosstermKeyCode::Up, KeyCode::Up),
        (CrosstermKeyCode::Down, KeyCode::Down),
        (CrosstermKeyCode::Home, KeyCode::Home),
        (CrosstermKeyCode::End, KeyCode::End),
        (CrosstermKeyCode::PageUp, KeyCode::PageUp),
        (CrosstermKeyCode::PageDown, KeyCode::PageDown),
        (CrosstermKeyCode::Tab, KeyCode::Tab),
        (CrosstermKeyCode::BackTab, KeyCode::BackTab),
        (CrosstermKeyCode::Delete, KeyCode::Delete),
        (CrosstermKeyCode::Esc, KeyCode::Esc),
        (CrosstermKeyCode::Char('x'), KeyCode::Char('x')),
    ];

    for (backend, neutral) in pairs {
        assert_eq!(KeyCode::from(backend), neutral);
    }
}

#[test]
fn unmapped_keys_become_unknown() {
    let keys = [
        CrosstermKeyCode::F(5),
        CrosstermKeyCode::Insert,
        CrosstermKeyCode::CapsLock,
        CrosstermKeyCode::Media(MediaKeyCode::Play),
    ];

    for key in keys {
        let neutral = KeyCode::from(key);
        assert_eq!(neutral, KeyCode::Unknown);
        assert_ne!(neutral, KeyCode::Esc);
    }
}

#[test]
fn unknown_is_inert_in_widgets() {
    let key = KeyEvent::new(KeyCode::Unknown, KeyModifiers::NONE);
    let mut input = TextInputState::new("value");
    let rows = [ListRow {
        id: 1,
        label: Line::from("one"),
        role: RowRole::Item,
        enabled: true,
    }];
    let mut list = ListState::new(Some(1));
    let actions = [Action {
        id: 1,
        label: "Accept",
        enabled: true,
        style: None,
    }];
    let mut dialog = ChoiceDialogState::new(Some(1));

    assert_eq!(input.handle_key(key), TextInputOutcome::Ignored);
    assert_eq!(list.handle_key(&rows, key), ListOutcome::Ignored);
    assert_eq!(dialog.handle_key(key, &actions), Outcome::Ignored);
}

#[test]
fn release_events_are_ignored() {
    let key = KeyEvent {
        kind: KeyEventKind::Release,
        ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    };
    let mut input = TextInputState::new("value");
    let rows = [ListRow {
        id: 1,
        label: Line::from("one"),
        role: RowRole::Item,
        enabled: true,
    }];
    let mut list = ListState::new(Some(1));
    let actions = [Action {
        id: 1,
        label: "Accept",
        enabled: true,
        style: None,
    }];
    let mut dialog = ChoiceDialogState::new(Some(1));

    assert_eq!(input.handle_key(key), TextInputOutcome::Ignored);
    assert_eq!(list.handle_key(&rows, key), ListOutcome::Ignored);
    assert_eq!(dialog.handle_key(key, &actions), Outcome::Ignored);
}
