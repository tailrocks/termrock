// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Semantic UI intents — widgets consume these instead of raw key matching.

use crate::input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Relative navigation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NavigationMove {
    /// Previous item / up / left depending on context.
    Previous,
    /// Next item / down / right depending on context.
    Next,
    /// First item / home.
    First,
    /// Last item / end.
    Last,
}

/// Page-scale movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PageMove {
    /// Page up / previous page.
    Backward,
    /// Page down / next page.
    Forward,
}

/// Semantic intent for collection and chrome surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UiIntent {
    /// Move selection or caret.
    Move(NavigationMove),
    /// Page through a viewport.
    Page(PageMove),
    /// Activate the focused item (Enter).
    Activate,
    /// Toggle check / multi-select on the focused item.
    Toggle,
    /// Open a nested surface.
    Open,
    /// Close the current surface.
    Close,
    /// Cancel / Escape peel.
    Cancel,
    /// Submit a form or prompt.
    Submit,
}

/// Maps a key event to a list-oriented intent using TermRock defaults.
///
/// Applications may replace this with a keymap-driven adapter. Returns `None`
/// for releases and unmapped keys.
#[must_use]
pub fn default_list_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    // Ignore pure-modifier noise; list defaults ignore most modifiers.
    if !key.modifiers.is_empty()
        && !matches!(key.code, KeyCode::Char(_))
        && key.modifiers != KeyModifiers::SHIFT
    {
        // Allow nothing with Ctrl/Alt for default list map.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return None;
        }
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::Enter => Some(UiIntent::Activate),
        KeyCode::Char(' ') => Some(UiIntent::Toggle),
        KeyCode::Esc => Some(UiIntent::Cancel),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_list_intent_maps_core_keys() {
        assert_eq!(
            default_list_intent(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Next))
        );
        assert_eq!(
            default_list_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_list_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(UiIntent::Cancel)
        );
        assert_eq!(
            default_list_intent(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            None
        );
    }
}
