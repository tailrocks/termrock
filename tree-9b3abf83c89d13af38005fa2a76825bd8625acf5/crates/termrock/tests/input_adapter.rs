//! Integration coverage for Crossterm event conversion.
#![cfg(feature = "crossterm")]

use crossterm::event::{
    KeyCode as CrosstermKeyCode, MediaKeyCode, MouseButton as CrosstermMouseButton,
    MouseEventKind as CrosstermMouseEventKind,
};
use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use termrock::{
    Theme,
    input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    interaction::Outcome,
    widgets::{
        Action, ChoiceDialogState, List, ListRow, ListState, RowRole, TextInputOutcome,
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
fn every_mouse_button_action_maps_to_the_neutral_vocabulary() {
    let buttons = [
        (CrosstermMouseButton::Left, MouseButton::Left),
        (CrosstermMouseButton::Right, MouseButton::Right),
        (CrosstermMouseButton::Middle, MouseButton::Middle),
    ];
    for (backend, neutral) in buttons {
        assert_eq!(
            MouseEventKind::from(CrosstermMouseEventKind::Down(backend)),
            MouseEventKind::Down(neutral)
        );
        assert_eq!(
            MouseEventKind::from(CrosstermMouseEventKind::Up(backend)),
            MouseEventKind::Up(neutral)
        );
        assert_eq!(
            MouseEventKind::from(CrosstermMouseEventKind::Drag(backend)),
            MouseEventKind::Drag(neutral)
        );
    }
}

#[test]
fn neutral_mouse_event_drives_list_activation() {
    let backend = crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
        kind: CrosstermMouseEventKind::Down(CrosstermMouseButton::Left),
        column: 2,
        row: 0,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    let Event::Mouse(mouse) = Event::from(backend) else {
        panic!("mouse event remains mouse input");
    };
    assert_eq!(mouse.kind, MouseEventKind::Down(MouseButton::Left));

    let rows = [ListRow {
        id: "entry",
        label: Line::from("Entry"),
        trailing: None,
        role: RowRole::Item,
        enabled: true,
    }];
    let theme = Theme::default();
    let list = List::new(&rows, &theme);
    let area = Rect::new(0, 0, 12, 1);
    let mut buffer = Buffer::empty(area);
    let mut state = ListState::new(Some("entry"));
    (&list).render(area, &mut buffer, &mut state);

    assert_eq!(state.click(mouse.position), Outcome::Activated("entry"));
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
        trailing: None,
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
    assert_eq!(list.handle_key(&rows, key), Outcome::Ignored);
    assert_eq!(dialog.handle_key(&actions, key), Outcome::Ignored);
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
        trailing: None,
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
    assert_eq!(list.handle_key(&rows, key), Outcome::Ignored);
    assert_eq!(dialog.handle_key(&actions, key), Outcome::Ignored);
}

#[test]
fn paste_carries_unicode_text() {
    let Event::Paste(text) = Event::from(crossterm::event::Event::Paste("héllo🧪".to_owned()))
    else {
        panic!("paste remains paste input");
    };
    assert_eq!(text, "héllo🧪");
}

#[test]
fn text_input_inserts_paste_at_a_grapheme_boundary() {
    let mut input = TextInputState::new("a🧪z");
    assert!(input.set_cursor_byte(1));

    assert_eq!(input.insert_str("界e\u{301}"), TextInputOutcome::Changed);
    assert_eq!(input.value(), "a界e\u{301}🧪z");
    assert_eq!(input.cursor_byte(), "a界e\u{301}".len());
    assert!(input.set_cursor_byte(input.cursor_byte()));
}

#[test]
fn text_input_truncates_multiline_paste_at_the_first_line_break() {
    let mut input = TextInputState::new("start:");
    assert_eq!(
        input.insert_str("first\r\nsecond"),
        TextInputOutcome::Changed
    );
    assert_eq!(input.value(), "start:first");
}

#[test]
fn text_input_paste_honors_max_graphemes() {
    let mut input = TextInputState::new("a").with_max_graphemes(3);
    assert_eq!(input.insert_str("界🧪overflow"), TextInputOutcome::Changed);
    assert_eq!(input.value(), "a界🧪");
}

#[test]
fn text_input_paste_repairs_global_grapheme_boundaries() {
    let mut combining = TextInputState::new("\u{301}x");
    assert!(combining.set_cursor_byte(0));
    assert_eq!(combining.insert_str("eb"), TextInputOutcome::Changed);
    assert_eq!(combining.value(), "eb\u{301}x");
    assert_eq!(combining.cursor_byte(), "eb\u{301}".len());
    assert!(combining.set_cursor_byte(combining.cursor_byte()));

    let mut zwj = TextInputState::new("👩👩");
    assert!(zwj.set_cursor_byte("👩".len()));
    assert_eq!(zwj.insert_str("\u{200d}👧"), TextInputOutcome::Changed);
    assert_eq!(zwj.value(), "👩\u{200d}👧👩");
    assert_eq!(zwj.cursor_byte(), "👩\u{200d}👧".len());
    assert!(zwj.set_cursor_byte(zwj.cursor_byte()));
}
