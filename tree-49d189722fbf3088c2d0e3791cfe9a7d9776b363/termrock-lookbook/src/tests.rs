// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use termrock::keymap::glyph;
use termrock::{Theme, input::KeyCode, keymap::KeyChord};

use crate::{
    PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction, stories::stories,
    svg::render_story_to_buffer,
};

#[test]
fn list_story_visibly_uses_the_selected_theme() {
    let story = stories()
        .into_iter()
        .find(|story| story.id == "list/selection")
        .expect("list story exists");
    let phosphor = render_story_to_buffer(story, &Theme::tailrocks_phosphor());
    let slate = render_story_to_buffer(story, &Theme::slate());

    assert_eq!(phosphor.area, slate.area);
    assert!(
        phosphor
            .content()
            .iter()
            .zip(slate.content())
            .any(|(left, right)| left.symbol() == right.symbol()
                && !left.symbol().trim().is_empty()
                && (left.fg, left.bg, left.modifier) != (right.fg, right.bg, right.modifier)),
        "list cells must visibly differ between themes"
    );
}

// ── SIDEBAR ───────────────────────────────────────────────────────────────────

#[test]
fn sidebar_down_up_dispatch_navigate() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Down)),
        Some(SidebarAction::Navigate)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Up)),
        Some(SidebarAction::Navigate)
    );
}

#[test]
fn sidebar_vim_aliases_dispatch_navigate() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('j'))),
        Some(SidebarAction::Navigate)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('k'))),
        Some(SidebarAction::Navigate)
    );
}

#[test]
fn sidebar_home_end_dispatch_go_to_edge() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Home)),
        Some(SidebarAction::GoToEdge)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::End)),
        Some(SidebarAction::GoToEdge)
    );
}

#[test]
fn sidebar_tab_dispatches_focus_preview() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Tab)),
        Some(SidebarAction::FocusPreview)
    );
}

#[test]
fn sidebar_q_esc_dispatch_quit() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('q'))),
        Some(SidebarAction::Quit)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Esc)),
        Some(SidebarAction::Quit)
    );
}

#[test]
fn sidebar_non_registered_keys_return_none() {
    for chord in [
        KeyChord::plain(KeyCode::Enter),
        KeyChord::plain(KeyCode::Char('a')),
        KeyChord::plain(KeyCode::Char('Q')),
        KeyChord::ctrl(KeyCode::Char('c')),
        KeyChord::plain(KeyCode::PageUp),
        KeyChord::plain(KeyCode::PageDown),
        KeyChord::plain(KeyCode::BackTab),
    ] {
        assert_eq!(
            SIDEBAR_KEYMAP.dispatch(chord),
            None,
            "sidebar must not dispatch {chord:?}"
        );
    }
}

#[test]
fn sidebar_hints_advertise_navigate_and_quit() {
    let spans = SIDEBAR_KEYMAP.hint_spans();
    let text: String = spans
        .iter()
        .filter_map(|s| match s {
            termrock::HintSpan::Key(k) | termrock::HintSpan::Text(k) => Some(*k),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("↑↓"), "must advertise ↑↓ navigate: {text}");
    assert!(text.contains("navigate"), "must label navigate: {text}");
    assert!(text.contains("⇥"), "must advertise Tab: {text}");
    assert!(text.contains("quit"), "must advertise quit: {text}");
    assert!(text.contains("Home"), "must advertise Home: {text}");
    // HiddenAlias — j/k must NOT appear in hint bar
    assert!(
        !text.contains(" j ") && !text.contains("j/k"),
        "j/k alias must not appear in hint: {text}"
    );
}

// ── PREVIEW ───────────────────────────────────────────────────────────────────

#[test]
fn preview_esc_tab_backtab_dispatch_back_to_list() {
    for chord in [
        KeyChord::plain(KeyCode::Esc),
        KeyChord::plain(KeyCode::Tab),
        KeyChord::plain(KeyCode::BackTab),
    ] {
        assert_eq!(
            PREVIEW_KEYMAP.dispatch(chord),
            Some(PreviewAction::BackToList),
            "preview back-to-list must dispatch {chord:?}"
        );
    }
}

#[test]
fn preview_arrows_dispatch_forward() {
    for chord in [
        KeyChord::plain(KeyCode::Up),
        KeyChord::plain(KeyCode::Down),
        KeyChord::plain(KeyCode::Left),
        KeyChord::plain(KeyCode::Right),
    ] {
        assert_eq!(
            PREVIEW_KEYMAP.dispatch(chord),
            Some(PreviewAction::Forward),
            "preview must forward {chord:?}"
        );
    }
}

#[test]
fn preview_page_keys_dispatch() {
    assert_eq!(
        PREVIEW_KEYMAP.dispatch(KeyChord::plain(KeyCode::PageDown)),
        Some(PreviewAction::PageDown)
    );
    assert_eq!(
        PREVIEW_KEYMAP.dispatch(KeyChord::plain(KeyCode::PageUp)),
        Some(PreviewAction::PageUp)
    );
}

#[test]
fn preview_shift_j_k_dispatch_move_preview() {
    assert_eq!(
        PREVIEW_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('J'))),
        Some(PreviewAction::MovePreviewDown)
    );
    assert_eq!(
        PREVIEW_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('K'))),
        Some(PreviewAction::MovePreviewUp)
    );
}

#[test]
fn preview_non_registered_keys_return_none() {
    for chord in [
        KeyChord::plain(KeyCode::Enter),
        KeyChord::plain(KeyCode::Char('q')),
        KeyChord::plain(KeyCode::Char('j')),
        KeyChord::plain(KeyCode::Char('k')),
        KeyChord::ctrl(KeyCode::Char('q')),
    ] {
        assert_eq!(
            PREVIEW_KEYMAP.dispatch(chord),
            None,
            "preview must not dispatch {chord:?}"
        );
    }
}

#[test]
fn preview_hints_advertise_back_and_interact() {
    let spans = PREVIEW_KEYMAP.hint_spans();
    let text: String = spans
        .iter()
        .filter_map(|s| match s {
            termrock::HintSpan::Key(k) | termrock::HintSpan::Text(k) => Some(*k),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("Esc"), "must advertise Esc back: {text}");
    assert!(text.contains("back to list"), "must label back: {text}");
    assert!(text.contains("↑↓"), "must advertise arrow interact: {text}");
    assert!(
        text.contains("J/K"),
        "must advertise J/K move preview: {text}"
    );
    assert!(
        text.contains(glyph::PGUP_PGDN),
        "must advertise page: {text}"
    );
    // BackTab is HiddenAlias — must NOT appear in hint bar separately
    assert!(
        !text.contains("BackTab"),
        "BackTab alias must not appear in hint: {text}"
    );
}
