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
    /// Expand a tree node / disclosure.
    Expand,
    /// Collapse a tree node / disclosure.
    Collapse,
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

/// Default intent map for tree collections (list + expand/collapse).
#[must_use]
pub fn default_tree_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    match key.code {
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Expand),
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Collapse),
        _ => default_list_intent(key),
    }
}

/// Default intent map for tabular collections.
#[must_use]
pub fn default_table_intent(key: KeyEvent) -> Option<UiIntent> {
    default_list_intent(key).and_then(|intent| match intent {
        // Tables do not toggle multi-select with Space by default.
        UiIntent::Toggle => None,
        other => Some(other),
    })
}

/// Default intent map for [`crate::widgets::Transcript`] navigation.
///
/// Scroll / selection / activate / fold-toggle / cancel. Ctrl+F fold stays on
/// [`crate::widgets::TranscriptState::handle_key`] as a product chord.
#[must_use]
pub fn default_transcript_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Expand),
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Collapse),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::Menu`] / context menus.
#[must_use]
pub fn default_menu_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::DataTable`] navigation.
///
/// Product chords (sort `s`, filter `/`, expand Shift+arrow, copy, edit) stay on
/// [`DataTableState::handle_key`].
#[must_use]
pub fn default_data_table_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    // Ctrl+Home / Ctrl+End handled as page extremes via intent + host, or product path.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Home | KeyCode::End)
    {
        return match key.code {
            KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
            KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
            _ => None,
        };
    }
    if !key.modifiers.is_empty()
        && !matches!(key.code, KeyCode::Char(_))
        && key.modifiers != KeyModifiers::SHIFT
    {
        return None;
    }
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::ObjectInspector`] field navigation.
///
/// Same shape as list nav (j/k, arrows, Home/End, page, Enter/Space activate).
/// Expand/collapse of nested paths stays consumer-owned projection.
#[must_use]
pub fn default_inspector_intent(key: KeyEvent) -> Option<UiIntent> {
    default_list_intent(key).and_then(|intent| match intent {
        // Inspector does not cancel itself — host owns Esc / overlay dismiss.
        UiIntent::Cancel => None,
        other => Some(other),
    })
}

/// Default intent map for [`crate::widgets::DiffReview`] line scroll + activate.
///
/// Product chords **n/p** (hunk step) and **s** (toggle split) stay on
/// [`crate::widgets::DiffReviewState::handle_key`]. j/k and arrows scroll lines;
/// Home/End jump first/last hunk when hunks exist.
#[must_use]
pub fn default_diff_review_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::LogStream`] scroll + follow.
///
/// - Arrows / j/k / page / Home → scroll (host maps to Detach/Scrolled)
/// - End → [`NavigationMove::Last`] (re-follow tail)
/// - Space → [`UiIntent::Toggle`] follow
/// - `f` is a product chord on [`LogStreamState::handle_key`] (also Toggle)
///
/// Esc / Enter are not mapped — host owns dismiss and line activate if any.
#[must_use]
pub fn default_log_stream_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::ChoiceDialog`] action bar.
///
/// - Left/Right (and j/k) move **local action cursor**
/// - Enter activates; Esc cancels
/// - **Tab / BackTab are not mapped** — host InteractionScene owns trap Tab
///   when action ids are registered as focus targets
#[must_use]
pub fn default_choice_dialog_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'H' | 'k' | 'K') => {
            Some(UiIntent::Move(NavigationMove::Previous))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('l' | 'L' | 'j' | 'J') => {
            Some(UiIntent::Move(NavigationMove::Next))
        }
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::Form`] (activate + page scroll only).
///
/// **Field cycle (Tab / Up / Down) is host / scene owned** — not mapped here.
#[must_use]
pub fn default_form_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::PermissionPrompt`] navigation.
///
/// Covers Activate / Cancel / Move / Expand-Collapse details. Product chords
/// (`e` edit, `p` pattern, `n` deny, scope brackets) remain on
/// [`crate::widgets::PermissionPromptState::handle_key`] until a dedicated
/// keymap pack is adopted.
#[must_use]
pub fn default_permission_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    // Press-only for confirm/cancel to avoid held-key multi-fire.
    let is_press = key.kind == KeyEventKind::Press;
    match key.code {
        KeyCode::Left | KeyCode::Up => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Right | KeyCode::Down | KeyCode::Tab
            if !key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(UiIntent::Move(NavigationMove::Next))
        }
        KeyCode::BackTab => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(UiIntent::Move(NavigationMove::Previous))
        }
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        KeyCode::Char('d' | 'D') if is_press => Some(UiIntent::Toggle), // details — host maps Toggle→Expand/Collapse
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

    #[test]
    fn default_transcript_intent_maps_scroll_and_activate() {
        assert_eq!(
            default_transcript_intent(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Previous))
        );
        assert_eq!(
            default_transcript_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_transcript_intent(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(UiIntent::Toggle)
        );
    }

    #[test]
    fn default_inspector_intent_maps_nav_not_cancel() {
        assert_eq!(
            default_inspector_intent(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Next))
        );
        assert_eq!(
            default_inspector_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_inspector_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn default_log_stream_intent_maps_scroll_and_toggle() {
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(UiIntent::Page(PageMove::Backward))
        );
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Last))
        );
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(UiIntent::Toggle)
        );
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn default_diff_review_intent_maps_scroll_and_activate() {
        assert_eq!(
            default_diff_review_intent(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Next))
        );
        assert_eq!(
            default_diff_review_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_diff_review_intent(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn default_choice_dialog_intent_no_tab() {
        assert_eq!(
            default_choice_dialog_intent(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Next))
        );
        assert_eq!(
            default_choice_dialog_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            default_choice_dialog_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(UiIntent::Cancel)
        );
    }

    #[test]
    fn default_permission_intent_maps_nav_and_fail_safe_keys() {
        assert_eq!(
            default_permission_intent(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Previous))
        );
        assert_eq!(
            default_permission_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_permission_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(UiIntent::Cancel)
        );
        // No grant-on-y in the intent map.
        assert_eq!(
            default_permission_intent(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            None
        );
    }
}
