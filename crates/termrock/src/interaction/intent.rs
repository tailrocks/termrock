// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Semantic UI intents — widgets consume these instead of raw key matching.
use crate::input::{KeyCode, KeyEvent, KeyModifiers};

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
    /// Explicit upward spatial step (FocusGraph / 2D).
    Up,
    /// Explicit downward spatial step.
    Down,
    /// Explicit leftward spatial step.
    Left,
    /// Explicit rightward spatial step.
    Right,
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

/// Stable application command identity for palette / global maps.
///
/// Static only so [`UiIntent`] stays [`Copy`] and can sit in [`crate::keymap::Keymap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppCommandId(pub &'static str);

impl AppCommandId {
    /// Constructs a command id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Underlying static id.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl core::fmt::Display for AppCommandId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

/// Semantic intent for collection and chrome surfaces.
///
/// Widgets consume intents; physical keys live only in [`crate::keymap::Keymap`]
/// tables and `default_*_intent` bridges.
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
    /// Linear focus next (Tab) — scene/FocusGraph owned when registered.
    FocusNext,
    /// Linear focus previous (BackTab / Shift+Tab).
    FocusPrevious,
    /// Enter jump-to-region mode.
    JumpStart,
    /// Activate a jump badge letter while jump mode is open.
    JumpLabel(char),
    /// Enter edit mode / focus the field editor.
    Edit,
    /// Forward delete.
    Delete,
    /// Backward delete.
    Backspace,
    /// Open find / filter / search surface.
    Search,
    /// Show keyboard help / bindings panel.
    Help,
    /// Promote current surface to fullscreen.
    Fullscreen,
    /// Open the command palette.
    OpenCommandPalette,
    /// Application-level command (palette / global map).
    AppCommand(AppCommandId),
}

impl UiIntent {
    /// Short stable token for help / palette rows.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Move(_) => "move",
            Self::Page(_) => "page",
            Self::Activate => "activate",
            Self::Toggle => "toggle",
            Self::Open => "open",
            Self::Close => "close",
            Self::Cancel => "cancel",
            Self::Submit => "submit",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::FocusNext => "focus_next",
            Self::FocusPrevious => "focus_previous",
            Self::JumpStart => "jump_start",
            Self::JumpLabel(_) => "jump_label",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Backspace => "backspace",
            Self::Search => "search",
            Self::Help => "help",
            Self::Fullscreen => "fullscreen",
            Self::OpenCommandPalette => "command_palette",
            Self::AppCommand(_) => "app_command",
        }
    }

    /// Whether this intent is primarily chrome/global rather than leaf-widget.
    #[must_use]
    pub const fn is_global_chrome(self) -> bool {
        matches!(
            self,
            Self::Help
                | Self::Search
                | Self::Fullscreen
                | Self::OpenCommandPalette
                | Self::JumpStart
                | Self::AppCommand(_)
                | Self::FocusNext
                | Self::FocusPrevious
        )
    }
}

/// Maps a key event to a list-oriented intent using TermRock defaults.
///
/// Applications may replace this with a keymap-driven adapter. Returns `None`
/// for releases and unmapped keys.
#[must_use]
pub fn default_list_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
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
    if !key.is_insert() {
        return None;
    }
    match key.code {
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Expand),
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Collapse),
        _ => default_list_intent(key),
    }
}

/// Default intent map for tabular collections.
///
/// List vertical nav + activate/cancel; Left/Right drive cell focus or horizontal
/// scroll on [`crate::widgets::Table`]. Space does not toggle multi-select.
#[must_use]
pub fn default_table_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    match key.code {
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Move(NavigationMove::Left)),
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Move(NavigationMove::Right)),
        _ => default_list_intent(key).and_then(|intent| match intent {
            UiIntent::Toggle => None,
            other => Some(other),
        }),
    }
}

/// Default intent map for [`crate::widgets::Transcript`] navigation.
///
/// Scroll / selection / activate / fold-toggle / cancel. Ctrl+F fold stays on
/// [`crate::widgets::TranscriptState::handle_key`] as a product chord.
#[must_use]
pub fn default_transcript_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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

/// Default intent map for [`crate::widgets::DropdownMenu`] / context menus.
#[must_use]
pub fn default_menu_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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

/// Default intent map for readline-style result overlays
/// ([`crate::widgets::CommandPalette`], [`crate::widgets::HistoryPicker`],
/// [`crate::widgets::QuickOpen`]).
///
/// Plain ↑/↓ and Ctrl+J/K move the result cursor, PageUp/PageDown page,
/// Ctrl+Home/End jump; Enter (Activate) and Esc (Cancel) fire on press only so
/// a held key cannot close and reopen the overlay.
#[must_use]
pub fn default_palette_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Down => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Char('j' | 'J') if ctrl => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Char('k' | 'K') if ctrl => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::Home if ctrl => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End if ctrl => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
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
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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
/// - `f` is a product chord on [`LogStreamState::handle_key`] (Toggle follow)
/// - Space is a product chord for multi-select (not mapped here)
///
/// Esc / Enter stay on the state path (search cancel / copy).
#[must_use]
pub fn default_log_stream_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::TextArea`] navigation / cancel.
///
/// Character insert, Backspace, Delete, Enter-newline stay on
/// [`crate::widgets::TextAreaState::handle_key`]. Home/End/Page map here;
/// Up/Down line motion stays key-path (not list Previous/Next).
#[must_use]
pub fn default_text_area_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    if !key.modifiers.is_empty() && key.modifiers != KeyModifiers::SHIFT {
        return None;
    }
    match key.code {
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::Esc => Some(UiIntent::Cancel),
        KeyCode::Left if key.modifiers.is_empty() => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Right if key.modifiers.is_empty() => Some(UiIntent::Move(NavigationMove::Next)),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::Button`] / activation controls.
///
/// Enter and Space → [`UiIntent::Activate`]. Repeat/Release are ignored by the
/// map (callers still see kind on the key event). Product chords stay out.
#[must_use]
pub fn default_button_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_press() {
        return None;
    }
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Enter => Some(UiIntent::Activate),
        // Space maps to Activate; ActivationState may arm-on-press separately.
        KeyCode::Char(' ') => Some(UiIntent::Activate),
        _ => None,
    }
}

/// Default intent map for [`crate::widgets::PromptComposer`] surface chords.
///
/// Enter → [`UiIntent::Submit`] (composer applies submit-vs-newline policy).
/// Esc → [`UiIntent::Cancel`] (completion / fullscreen / dismiss peel).
/// Editor caret, history, and product Ctrl chords stay on
/// [`crate::widgets::PromptComposerState::handle_key`].
#[must_use]
pub fn default_prompt_composer_intent(key: KeyEvent) -> Option<UiIntent> {
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Enter if is_press => Some(UiIntent::Submit),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
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
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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
    if !key.is_insert() {
        return None;
    }
    let is_press = key.is_press();
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
    if !key.is_insert() {
        return None;
    }
    // Press-only for confirm/cancel to avoid held-key multi-fire.
    let is_press = key.is_press();
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
    use crate::input::KeyEventKind;

    use super::*;

    #[test]
    fn default_text_area_intent_nav_cancel() {
        assert_eq!(
            default_text_area_intent(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::First))
        );
        let mut repeat = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        repeat.kind = KeyEventKind::Repeat;
        assert_eq!(
            default_text_area_intent(repeat),
            Some(UiIntent::Move(NavigationMove::First))
        );
        assert_eq!(
            default_text_area_intent(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            Some(UiIntent::Page(PageMove::Forward))
        );
        assert_eq!(
            default_text_area_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(UiIntent::Cancel)
        );
    }

    #[test]
    fn default_button_intent_maps_activate() {
        assert_eq!(
            default_button_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_button_intent(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        let mut rep = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        rep.kind = KeyEventKind::Repeat;
        assert_eq!(default_button_intent(rep), None);
    }

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
    fn default_palette_intent_moves_pages_and_press_gates() {
        assert_eq!(
            default_palette_intent(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Next))
        );
        assert_eq!(
            default_palette_intent(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            Some(UiIntent::Move(NavigationMove::Previous))
        );
        assert_eq!(
            default_palette_intent(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(UiIntent::Page(PageMove::Backward))
        );
        assert_eq!(
            default_palette_intent(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL)),
            Some(UiIntent::Move(NavigationMove::First))
        );
        // Home without Ctrl is not a palette chord.
        assert_eq!(
            default_palette_intent(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
            None
        );
        // Enter/Esc fire on press only; a held key cannot re-cancel the overlay.
        let mut repeat = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        repeat.kind = KeyEventKind::Repeat;
        assert_eq!(default_palette_intent(repeat), None);
        let mut release = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(default_palette_intent(release), None);
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
    fn default_log_stream_intent_maps_scroll() {
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(UiIntent::Page(PageMove::Backward))
        );
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Last))
        );
        // Space is multi-select on LogStreamState::handle_key (not intent-mapped).
        assert_eq!(
            default_log_stream_intent(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            None
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
    fn default_prompt_composer_intent_submit_cancel() {
        assert_eq!(
            default_prompt_composer_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Submit)
        );
        assert_eq!(
            default_prompt_composer_intent(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(UiIntent::Cancel)
        );
        assert_eq!(
            default_prompt_composer_intent(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            None
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
