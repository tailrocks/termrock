// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Production-grade single-line text editor.
//!
//! **Mission.** Grapheme-safe editing with selection, undo/redo, word movement,
//! horizontal scroll, placeholder, prefix/suffix, clear action, validation,
//! secret masking, and host-owned clipboard via typed outcomes.
//!
//! **Keys.** Prefer [`TextInputState::handle_intent`] / keymap packs; raw
//! [`TextInputState::handle_key`] maps common Emacs-style chords as a default
//! adapter (not the only binding surface).
//!
//! **Clipboard.** Copy/cut emit text for the host; paste is `insert_str` after
//! host resolves bracketed paste / OSC 52 / system clipboard.
//!
//! Research: prompt-toolkit, Reedline, Textual Input, terminal line editors.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::UiIntent,
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, MASK_CELLS, VisualState},
    text::{display_cols, take_display_cols, truncate_cols},
};

use super::edit_core;

/// Undo stack depth (snapshots).
const UNDO_LIMIT: usize = 64;

// ── Edit actions ────────────────────────────────────────────────────────────

/// Grapheme-safe edit operations accepted by text-input state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditAction {
    /// Insert one character at the cursor (replaces selection).
    Insert(char),
    /// Delete grapheme before cursor (or selection).
    Backspace,
    /// Delete grapheme at cursor (or selection).
    Delete,
    /// Move left one grapheme (`select` extends selection).
    MoveLeft {
        /// Extend selection.
        select: bool,
    },
    /// Move right one grapheme.
    MoveRight {
        /// Extend selection.
        select: bool,
    },
    /// Word left.
    WordLeft {
        /// Extend selection.
        select: bool,
    },
    /// Word right.
    WordRight {
        /// Extend selection.
        select: bool,
    },
    /// Line start.
    Home {
        /// Extend selection.
        select: bool,
    },
    /// Line end.
    End {
        /// Extend selection.
        select: bool,
    },
    /// Select entire value.
    SelectAll,
    /// Delete from start of line to cursor (Ctrl+U).
    KillToStart,
    /// Delete from cursor to end of line (Ctrl+K).
    KillToEnd,
    /// Clear value.
    Clear,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
}

impl EditAction {
    /// Move left without selection.
    #[must_use]
    pub const fn move_left() -> Self {
        Self::MoveLeft { select: false }
    }

    /// Move right without selection.
    #[must_use]
    pub const fn move_right() -> Self {
        Self::MoveRight { select: false }
    }

    /// Home without selection.
    #[must_use]
    pub const fn home() -> Self {
        Self::Home { select: false }
    }

    /// End without selection.
    #[must_use]
    pub const fn end() -> Self {
        Self::End { select: false }
    }
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Validation state and optional feedback for a form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validation<'a> {
    /// Field value is accepted and has no feedback message.
    Valid,
    /// Field value is rejected with caller-provided feedback.
    Invalid(&'a str),
}

/// Validation states rendered by a text input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputValidity {
    /// Trimmed value satisfies every configured rule.
    Valid,
    /// Trimmed value is empty while empty input is disallowed.
    Empty,
    /// Trimmed value exactly matches a configured forbidden value.
    Forbidden,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Semantic results produced by text-input interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextInputOutcome {
    /// The key produced no editing or submission action.
    Ignored,
    /// The input value or cursor/selection changed.
    Changed,
    /// The current value passed trimmed validation and was submitted unchanged.
    Submitted(String),
    /// Editing was cancelled by the user.
    Cancelled,
    /// Host should write this text to the clipboard.
    ClipboardCopy {
        /// Selected or full text.
        text: String,
    },
    /// Host should write to clipboard; text already removed from the field.
    ClipboardCut {
        /// Cut text.
        text: String,
    },
    /// Host should resolve paste and call [`TextInputState::insert_str`].
    ClipboardPasteRequest,
    /// Value cleared via clear action.
    Cleared,
}

// ── Snapshot / state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditSnapshot {
    value: String,
    cursor: usize,
    anchor: Option<usize>,
}

/// Runtime state for [`TextInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextInputState {
    value: String,
    cursor: usize,
    /// Selection anchor; `None` means caret only.
    #[cfg_attr(feature = "serde", serde(default))]
    anchor: Option<usize>,
    viewport: usize,
    max_graphemes: Option<usize>,
    forbidden: Vec<String>,
    allow_empty: bool,
    #[cfg_attr(feature = "serde", serde(default = "default_true"))]
    enabled: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    read_only: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    loading: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    undo: Vec<EditSnapshot>,
    #[cfg_attr(feature = "serde", serde(skip))]
    redo: Vec<EditSnapshot>,
    #[cfg_attr(feature = "serde", serde(skip))]
    parts: Option<TextInputParts>,
    #[cfg_attr(feature = "serde", serde(default))]
    focused: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    editing: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    hovered: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    snapshot: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    selecting_with_mouse: bool,
}

#[cfg_attr(not(feature = "serde"), expect(dead_code))]
fn default_true() -> bool {
    true
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextInputState {
    /// Creates idle (not editing) text-input state with the cursor at the end
    /// of the value. Junie `TextInput::new` starts `editing: false`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            value,
            cursor,
            anchor: None,
            viewport: 0,
            max_graphemes: None,
            forbidden: Vec::new(),
            allow_empty: false,
            enabled: true,
            read_only: false,
            loading: false,
            undo: Vec::new(),
            redo: Vec::new(),
            parts: None,
            focused: false,
            editing: false,
            hovered: false,
            snapshot: None,
            selecting_with_mouse: false,
        }
    }

    /// Limits accepted input to this many extended grapheme clusters.
    #[must_use]
    pub fn with_max_graphemes(mut self, max_graphemes: usize) -> Self {
        self.max_graphemes = Some(max_graphemes);
        self
    }

    /// Replaces the exact trimmed values rejected by validation.
    #[must_use]
    pub fn with_forbidden(mut self, forbidden: impl IntoIterator<Item = String>) -> Self {
        self.forbidden = forbidden.into_iter().collect();
        self
    }

    /// Configures whether a trimmed empty value is valid.
    #[must_use]
    pub const fn with_allow_empty(mut self, allow_empty: bool) -> Self {
        self.allow_empty = allow_empty;
        self
    }

    /// Live query, search, or draft surfaces that type immediately.
    ///
    /// Idle fields stay on [`Self::new`] (`editing: false`).
    #[must_use]
    pub fn with_editing(mut self) -> Self {
        self.set_editing(true);
        self
    }

    /// New value, keep editing/focus/enabled/read_only/allow_empty.
    #[must_use]
    pub(crate) fn reseed(&self, text: impl Into<String>) -> Self {
        let mut next = Self::new(text).with_allow_empty(self.allow_empty);
        if let Some(max) = self.max_graphemes {
            next = next.with_max_graphemes(max);
        }
        if !self.forbidden.is_empty() {
            next = next.with_forbidden(self.forbidden.clone());
        }
        if self.editing {
            next.set_editing(true);
        }
        next.set_focused(self.focused);
        next.set_enabled(self.enabled);
        next.set_read_only(self.read_only);
        next.set_loading(self.loading);
        next
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Read-only (navigation ok; no mutation).
    pub const fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
    }

    /// Loading (blocks edits).
    pub const fn set_loading(&mut self, on: bool) {
        self.loading = on;
    }

    /// Focus flag for paint.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Pointer hover (lifts the well only while not editing).
    pub const fn set_hovered(&mut self, on: bool) {
        self.hovered = on;
    }

    /// Whether the field owns the keyboard.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether the pointer is over the field.
    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether the field is in the editing session.
    #[must_use]
    pub const fn is_editing(&self) -> bool {
        self.editing
    }

    /// Enter or leave the editing session without committing.
    pub fn set_editing(&mut self, on: bool) {
        if on {
            self.begin_edit();
        } else {
            self.editing = false;
            self.snapshot = None;
        }
    }

    /// Start an editing session (snapshot for Esc).
    pub fn begin_edit(&mut self) {
        if self.editing {
            return;
        }
        if !self.can_edit() {
            return;
        }
        self.editing = true;
        self.snapshot = Some(self.value.clone());
        self.anchor = None;
    }

    /// End the editing session, keeping the current value.
    pub fn commit(&mut self) {
        self.editing = false;
        self.snapshot = None;
        self.anchor = None;
    }

    /// End the editing session and restore the snapshot.
    pub fn cancel_edit(&mut self) {
        if let Some(snap) = self.snapshot.take() {
            self.value = snap;
            self.cursor = self.value.len();
            self.anchor = None;
            self.viewport = 0;
        }
        self.editing = false;
    }

    /// Whether edits are allowed.
    #[must_use]
    pub const fn can_edit(&self) -> bool {
        self.enabled && !self.read_only && !self.loading
    }

    /// Current input value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Trimmed value.
    #[must_use]
    pub fn trimmed_value(&self) -> &str {
        self.value.trim()
    }

    /// Cursor UTF-8 byte offset.
    #[must_use]
    pub const fn cursor_byte(&self) -> usize {
        self.cursor
    }

    /// Selection anchor byte offset when selecting.
    #[must_use]
    pub const fn selection_anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// Ordered selection range if non-empty.
    #[must_use]
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        if a == self.cursor {
            return None;
        }
        Some(if a < self.cursor {
            (a, self.cursor)
        } else {
            (self.cursor, a)
        })
    }

    /// Selected text if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        let (a, b) = self.selection_range()?;
        self.value.get(a..b)
    }

    /// Last paint geometry.
    #[must_use]
    pub const fn parts(&self) -> Option<&TextInputParts> {
        self.parts.as_ref()
    }

    /// Clears value and selection; preserves configuration.
    pub fn clear(&mut self) -> bool {
        if self.value.is_empty() && self.cursor == 0 && self.viewport == 0 && self.anchor.is_none()
        {
            return false;
        }
        if self.can_edit() {
            self.push_undo();
        }
        self.value.clear();
        self.cursor = 0;
        self.anchor = None;
        self.viewport = 0;
        true
    }

    /// Overwrite secret bytes then clear (best-effort; not a full memory wipe).
    ///
    /// Used by [`super::PasswordInputState`] and hosts that must not leave
    /// plaintext lingering in the `String` buffer after dismiss.
    pub fn secure_clear(&mut self) {
        let taken = std::mem::take(&mut self.value);
        let mut bytes = taken.into_bytes();
        for byte in &mut bytes {
            *byte = 0;
        }
        drop(bytes);
        self.cursor = 0;
        self.anchor = None;
        self.viewport = 0;
        // Drop undo/redo snapshots that may hold prior secrets.
        for snap in self.undo.drain(..) {
            let mut bytes = snap.value.into_bytes();
            for byte in &mut bytes {
                *byte = 0;
            }
            drop(bytes);
        }
        for snap in self.redo.drain(..) {
            let mut bytes = snap.value.into_bytes();
            for byte in &mut bytes {
                *byte = 0;
            }
            drop(bytes);
        }
    }

    /// Set cursor to grapheme boundary.
    pub fn set_cursor_byte(&mut self, cursor: usize) -> bool {
        let valid = edit_core::is_boundary(&self.value, cursor);
        if valid {
            self.cursor = cursor;
            self.anchor = None;
        }
        valid
    }

    /// Validity rules.
    #[must_use]
    pub fn validity(&self) -> TextInputValidity {
        let value = self.trimmed_value();
        if value.is_empty() && !self.allow_empty {
            TextInputValidity::Empty
        } else if !value.is_empty() && self.forbidden.iter().any(|item| item == value) {
            TextInputValidity::Forbidden
        } else {
            TextInputValidity::Valid
        }
    }

    /// Submit-ready.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validity() == TextInputValidity::Valid
    }

    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            value: self.value.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn restore(&mut self, snap: EditSnapshot) {
        self.value = snap.value;
        self.cursor = snap.cursor;
        self.anchor = snap.anchor;
        self.viewport = self.viewport.min(self.cursor);
    }

    fn delete_selection(&mut self) -> bool {
        let Some((a, b)) = self.selection_range() else {
            return false;
        };
        self.value.drain(a..b);
        self.cursor = a;
        self.anchor = None;
        true
    }

    fn begin_move(&mut self, select: bool) {
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
    }

    /// Apply grapheme-safe edit.
    pub fn apply(&mut self, action: EditAction) -> bool {
        // Navigation allowed when read-only; mutations need can_edit
        let mutating = matches!(
            action,
            EditAction::Insert(_)
                | EditAction::Backspace
                | EditAction::Delete
                | EditAction::Clear
                | EditAction::KillToStart
                | EditAction::KillToEnd
                | EditAction::Undo
                | EditAction::Redo
        );
        if mutating && !self.can_edit() {
            return false;
        }
        if matches!(action, EditAction::Undo | EditAction::Redo) {
            // handled below without double push
        } else if mutating {
            self.push_undo();
        }

        let before = (self.value.clone(), self.cursor, self.anchor);
        match action {
            EditAction::Insert(character) => {
                if character.is_control() {
                    let _ = self.undo.pop();
                    return false;
                }
                self.delete_selection();
                let mut candidate = self.value.clone();
                if !edit_core::is_boundary(&candidate, self.cursor) {
                    self.cursor = edit_core::boundary_at_or_before(&candidate, self.cursor);
                }
                candidate.insert(self.cursor, character);
                if self
                    .max_graphemes
                    .is_some_and(|max| candidate.graphemes(true).count() > max)
                {
                    let _ = self.undo.pop();
                    return false;
                }
                self.value = candidate;
                self.cursor = edit_core::boundary_at_or_after(
                    &self.value,
                    self.cursor + character.len_utf8(),
                );
                self.anchor = None;
            }
            EditAction::Backspace => {
                if self.delete_selection() {
                    // ok
                } else {
                    edit_core::backspace(&mut self.value, &mut self.cursor);
                }
            }
            EditAction::Delete => {
                if self.delete_selection() {
                    // ok
                } else {
                    edit_core::delete(&mut self.value, self.cursor);
                }
            }
            EditAction::MoveLeft { select } => {
                self.begin_move(select);
                if let Some(index) = edit_core::previous_boundary(&self.value, self.cursor) {
                    self.cursor = index;
                }
            }
            EditAction::MoveRight { select } => {
                self.begin_move(select);
                if let Some(index) = edit_core::next_boundary(&self.value, self.cursor) {
                    self.cursor = index;
                }
            }
            EditAction::WordLeft { select } => {
                self.begin_move(select);
                self.cursor = edit_core::previous_word_boundary(&self.value, self.cursor);
            }
            EditAction::WordRight { select } => {
                self.begin_move(select);
                self.cursor = edit_core::next_word_boundary(&self.value, self.cursor);
            }
            EditAction::Home { select } => {
                self.begin_move(select);
                self.cursor = 0;
            }
            EditAction::End { select } => {
                self.begin_move(select);
                self.cursor = self.value.len();
            }
            EditAction::SelectAll => {
                if self.value.is_empty() {
                    self.anchor = None;
                    self.cursor = 0;
                } else {
                    self.anchor = Some(0);
                    self.cursor = self.value.len();
                }
            }
            EditAction::KillToStart => {
                if self.delete_selection() {
                    // ok
                } else if self.cursor > 0 {
                    self.value.drain(..self.cursor);
                    self.cursor = 0;
                    self.anchor = None;
                }
            }
            EditAction::KillToEnd => {
                if self.delete_selection() {
                    // ok
                } else if self.cursor < self.value.len() {
                    self.value.drain(self.cursor..);
                    self.anchor = None;
                }
            }
            EditAction::Clear => {
                if self.value.is_empty() {
                    let _ = self.undo.pop();
                    return false;
                }
                self.value.clear();
                self.cursor = 0;
                self.anchor = None;
                self.viewport = 0;
            }
            EditAction::Undo => {
                let Some(prev) = self.undo.pop() else {
                    return false;
                };
                self.redo.push(self.snapshot());
                self.restore(prev);
                return true;
            }
            EditAction::Redo => {
                let Some(next) = self.redo.pop() else {
                    return false;
                };
                self.undo.push(self.snapshot());
                self.restore(next);
                return true;
            }
        }
        let changed = before != (self.value.clone(), self.cursor, self.anchor);
        if !changed && mutating {
            let _ = self.undo.pop();
        }
        changed
    }

    fn edit(&mut self, action: EditAction) -> TextInputOutcome {
        if self.apply(action) {
            TextInputOutcome::Changed
        } else {
            TextInputOutcome::Ignored
        }
    }

    fn submit(&self) -> TextInputOutcome {
        if self.is_valid() {
            TextInputOutcome::Submitted(self.value.clone())
        } else {
            TextInputOutcome::Ignored
        }
    }

    /// Default key adapter (Emacs-style chords). Prefer keymaps → intents in hosts.
    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputOutcome {
        if key.is_release() {
            return TextInputOutcome::Ignored;
        }
        if !self.enabled {
            return TextInputOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // These physical actions have lifecycle or host-facing side effects.
        // Reject repeats before any editor mutation; ordinary text and cursor
        // repeats remain supported for held-key behavior.
        let one_shot = matches!(
            key.code,
            KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Esc
                | KeyCode::Char(
                    'c' | 'C'
                        | 'k'
                        | 'K'
                        | 'm'
                        | 'M'
                        | 'u'
                        | 'U'
                        | 'v'
                        | 'V'
                        | 'w'
                        | 'W'
                        | 'x'
                        | 'X',
                )
        );
        if !key.is_press()
            && (matches!(
                key.code,
                KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab | KeyCode::Esc
            ) || (ctrl && one_shot))
        {
            return TextInputOutcome::Ignored;
        }

        if !self.editing {
            if matches!(key.code, KeyCode::Enter) && !ctrl && !alt && self.can_edit() {
                self.begin_edit();
                return TextInputOutcome::Changed;
            }
            return TextInputOutcome::Ignored;
        }

        // Clipboard chords → host
        if ctrl && !alt {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    let text = self
                        .selected_text()
                        .unwrap_or(self.value.as_str())
                        .to_owned();
                    if text.is_empty() {
                        return TextInputOutcome::Ignored;
                    }
                    return TextInputOutcome::ClipboardCopy { text };
                }
                KeyCode::Char('x' | 'X') if self.can_edit() => {
                    let Some(text) = self.selected_text().map(str::to_owned) else {
                        return TextInputOutcome::Ignored;
                    };
                    self.push_undo();
                    let _ = self.delete_selection();
                    return TextInputOutcome::ClipboardCut { text };
                }
                KeyCode::Char('v' | 'V') if self.can_edit() => {
                    return TextInputOutcome::ClipboardPasteRequest;
                }
                KeyCode::Char('a' | 'A') => {
                    return self.edit(EditAction::Home { select: false });
                }
                KeyCode::Char('e' | 'E') => {
                    return self.edit(EditAction::End { select: false });
                }
                KeyCode::Char('u' | 'U') if self.can_edit() => {
                    return self.edit(EditAction::KillToStart);
                }
                KeyCode::Char('k' | 'K') if self.can_edit() => {
                    return self.edit(EditAction::KillToEnd);
                }
                KeyCode::Char('l' | 'L') => return self.edit(EditAction::SelectAll),
                KeyCode::Char('z' | 'Z') if !shift && self.can_edit() => {
                    return self.edit(EditAction::Undo);
                }
                KeyCode::Char('z' | 'Z') if shift && self.can_edit() => {
                    return self.edit(EditAction::Redo);
                }
                KeyCode::Char('y' | 'Y') if self.can_edit() => {
                    return self.edit(EditAction::Redo);
                }
                KeyCode::Char('w' | 'W') if self.can_edit() => {
                    self.push_undo();
                    if self.delete_selection() {
                        return TextInputOutcome::Changed;
                    }
                    let start = edit_core::previous_word_boundary(&self.value, self.cursor);
                    if start < self.cursor {
                        self.value.drain(start..self.cursor);
                        self.cursor = start;
                        self.anchor = None;
                        return TextInputOutcome::Changed;
                    }
                    let _ = self.undo.pop();
                    return TextInputOutcome::Ignored;
                }
                KeyCode::Home => return self.edit(EditAction::Home { select: false }),
                KeyCode::End => return self.edit(EditAction::End { select: false }),
                _ => {}
            }
        }

        if alt && !ctrl {
            match key.code {
                KeyCode::Char('b' | 'B') => {
                    return self.edit(EditAction::WordLeft { select: shift });
                }
                KeyCode::Char('f' | 'F') => {
                    return self.edit(EditAction::WordRight { select: shift });
                }
                KeyCode::Left => return self.edit(EditAction::WordLeft { select: shift }),
                KeyCode::Right => return self.edit(EditAction::WordRight { select: shift }),
                KeyCode::Backspace if self.can_edit() => {
                    self.push_undo();
                    let start = edit_core::previous_word_boundary(&self.value, self.cursor);
                    if start < self.cursor {
                        self.value.drain(start..self.cursor);
                        self.cursor = start;
                        self.anchor = None;
                        return TextInputOutcome::Changed;
                    }
                    let _ = self.undo.pop();
                    return TextInputOutcome::Ignored;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter => {
                self.commit();
                self.submit()
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.commit();
                self.submit()
            }
            KeyCode::Char('m' | 'M') if ctrl => {
                self.commit();
                self.submit()
            }
            KeyCode::Esc => {
                self.cancel_edit();
                TextInputOutcome::Cancelled
            }
            KeyCode::Backspace if ctrl || alt => {
                if !self.can_edit() {
                    return TextInputOutcome::Ignored;
                }
                self.push_undo();
                if self.delete_selection() {
                    return TextInputOutcome::Changed;
                }
                let start = edit_core::previous_word_boundary(&self.value, self.cursor);
                if start < self.cursor {
                    self.value.drain(start..self.cursor);
                    self.cursor = start;
                    self.anchor = None;
                    TextInputOutcome::Changed
                } else {
                    let _ = self.undo.pop();
                    TextInputOutcome::Ignored
                }
            }
            KeyCode::Backspace => self.edit(EditAction::Backspace),
            KeyCode::Delete => self.edit(EditAction::Delete),
            KeyCode::Left if ctrl || alt => self.edit(EditAction::WordLeft { select: shift }),
            KeyCode::Right if ctrl || alt => self.edit(EditAction::WordRight { select: shift }),
            KeyCode::Left => self.edit(EditAction::MoveLeft { select: shift }),
            KeyCode::Right => self.edit(EditAction::MoveRight { select: shift }),
            KeyCode::Home => self.edit(EditAction::Home { select: shift }),
            KeyCode::End => self.edit(EditAction::End { select: shift }),
            KeyCode::Char(character) if !ctrl && !alt && !character.is_control() => {
                self.edit(EditAction::Insert(character))
            }
            _ => TextInputOutcome::Ignored,
        }
    }

    /// Intent path (keymap packs).
    pub fn handle_intent(&mut self, intent: UiIntent) -> TextInputOutcome {
        if !self.enabled {
            return TextInputOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit | UiIntent::Activate => self.submit(),
            UiIntent::Cancel | UiIntent::Close => {
                self.cancel_edit();
                TextInputOutcome::Cancelled
            }
            UiIntent::Move(crate::interaction::NavigationMove::Previous)
            | UiIntent::Move(crate::interaction::NavigationMove::Left) => {
                self.edit(EditAction::MoveLeft { select: false })
            }
            UiIntent::Move(crate::interaction::NavigationMove::Next)
            | UiIntent::Move(crate::interaction::NavigationMove::Right) => {
                self.edit(EditAction::MoveRight { select: false })
            }
            UiIntent::Move(crate::interaction::NavigationMove::First) => {
                self.edit(EditAction::Home { select: false })
            }
            UiIntent::Move(crate::interaction::NavigationMove::Last) => {
                self.edit(EditAction::End { select: false })
            }
            _ => TextInputOutcome::Ignored,
        }
    }

    /// Bracketed / multi-char paste (strips newlines and controls; grapheme-limited).
    pub fn insert_str(&mut self, text: &str) -> TextInputOutcome {
        if !self.can_edit() {
            return TextInputOutcome::Ignored;
        }
        self.push_undo();
        self.delete_selection();
        let mut insertion = self.cursor;
        if !edit_core::is_boundary(&self.value, insertion) {
            insertion = edit_core::boundary_at_or_before(&self.value, insertion);
            self.cursor = insertion;
        }
        let mut changed = false;
        for character in text
            .chars()
            .take_while(|character| !matches!(character, '\n' | '\r'))
        {
            if character.is_control() {
                continue;
            }
            let mut candidate = self.value.clone();
            candidate.insert(insertion, character);
            if self
                .max_graphemes
                .is_some_and(|max| candidate.graphemes(true).count() > max)
            {
                continue;
            }
            self.value = candidate;
            insertion += character.len_utf8();
            changed = true;
        }
        if changed {
            self.cursor = edit_core::boundary_at_or_after(&self.value, insertion);
            self.anchor = None;
            TextInputOutcome::Changed
        } else {
            let _ = self.undo.pop();
            TextInputOutcome::Ignored
        }
    }

    /// Place cursor from field-local display column.
    pub fn set_cursor_from_display_col(&mut self, field_col: usize) -> bool {
        let absolute = self.viewport_display_col() + field_col;
        let byte = edit_core::byte_at_display_column(&self.value, absolute);
        if edit_core::is_boundary(&self.value, byte) {
            self.cursor = byte;
            if !self.selecting_with_mouse {
                self.anchor = None;
            }
            true
        } else {
            false
        }
    }

    fn viewport_display_col(&self) -> usize {
        UnicodeWidthStr::width(&self.value[..self.viewport.min(self.value.len())])
    }

    /// Mouse: click places cursor; drag selects.
    pub fn handle_mouse(&mut self, event: MouseEvent, field_area: Rect) -> TextInputOutcome {
        if !self.enabled || field_area.is_empty() {
            return TextInputOutcome::Ignored;
        }
        if !field_area.contains(event.position) {
            if matches!(event.kind, MouseEventKind::Up(_)) {
                self.selecting_with_mouse = false;
            }
            return TextInputOutcome::Ignored;
        }
        let col = usize::from(event.position.x.saturating_sub(field_area.x));
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focused = true;
                self.selecting_with_mouse = true;
                let _ = self.set_cursor_from_display_col(col);
                self.anchor = Some(self.cursor);
                TextInputOutcome::Changed
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved
                if self.selecting_with_mouse =>
            {
                let before = self.cursor;
                let _ = self.set_cursor_from_display_col(col);
                if self.anchor.is_none() {
                    self.anchor = Some(before);
                }
                if before != self.cursor {
                    TextInputOutcome::Changed
                } else {
                    TextInputOutcome::Ignored
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.selecting_with_mouse = false;
                if self.anchor == Some(self.cursor) {
                    self.anchor = None;
                }
                TextInputOutcome::Ignored
            }
            _ => TextInputOutcome::Ignored,
        }
    }

    fn reveal_cursor(&mut self, width: usize) {
        let width = width.max(1);
        self.viewport = self.viewport.min(self.cursor);
        while UnicodeWidthStr::width(&self.value[self.viewport..self.cursor]) >= width {
            let Some(grapheme) = self.value[self.viewport..].graphemes(true).next() else {
                break;
            };
            self.viewport += grapheme.len();
        }
        // Keep some left context when possible
        if self.viewport > 0
            && UnicodeWidthStr::width(&self.value[self.viewport..self.cursor]) + 2 < width
        {
            // ok
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputParts {
    /// Full root.
    pub root: Rect,
    /// Editable field region (excluding label).
    pub field: Rect,
    /// Prefix slot.
    pub prefix: Option<Rect>,
    /// Suffix slot.
    pub suffix: Option<Rect>,
    /// Clear action hit.
    pub clear: Option<Rect>,
    /// Cursor cell.
    pub cursor: Option<Rect>,
}

/// Single-line grapheme-safe text input.
#[derive(Debug, Clone, Copy)]
pub struct TextInput<'a> {
    label: &'a str,
    required: bool,
    show_optional: bool,
    placeholder: &'a str,
    help: &'a str,
    validation: Validation<'a>,
    system: &'a DesignSystem,
    prefix: Option<&'a str>,
    suffix: Option<&'a str>,
    secret: bool,
    show_clear: bool,
    secret_mask: char,
}

impl<'a> TextInput<'a> {
    /// Creates a text input with no placeholder.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            required: false,
            show_optional: false,
            placeholder: "",
            help: "",
            validation: Validation::Valid,
            system,
            prefix: None,
            suffix: None,
            secret: false,
            show_clear: false,
            secret_mask: '●',
        }
    }

    /// Marks the field as required; paints `*` after the label.
    ///
    /// Non-color cue by design — the mark is the fact and `Role::Danger` only
    /// reinforces it, so the requirement survives a colorless terminal.
    #[must_use]
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Placeholder while empty.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Helper copy on the third row (replaced by the error message when invalid).
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.help = help;
        self
    }

    /// Paint the `optional` suffix when the field is not required and the word fits.
    #[must_use]
    pub const fn optional(mut self, on: bool) -> Self {
        self.show_optional = on;
        self
    }

    /// Form-field validation feedback.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// Leading adornment (icon / unit).
    #[must_use]
    pub const fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Trailing adornment.
    #[must_use]
    pub const fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Secret / password paint mask only.
    ///
    /// Prefer [`super::PasswordInput`] for real credentials: it redacts
    /// `Debug`, blocks secret clipboard outcomes, and never embeds the value
    /// in submit outcomes. This flag only masks paint for non-secret demos.
    #[must_use]
    pub const fn secret(mut self, on: bool) -> Self {
        self.secret = on;
        self
    }

    /// Mask character for secret mode.
    #[must_use]
    pub const fn secret_mask(mut self, mask: char) -> Self {
        self.secret_mask = mask;
        self
    }

    /// Show clear `×` when non-empty and focused.
    #[must_use]
    pub const fn show_clear(mut self, on: bool) -> Self {
        self.show_clear = on;
        self
    }

    /// Label.
    #[must_use]
    pub const fn label(&self) -> &'a str {
        self.label
    }

    fn masked_display(&self) -> String {
        self.secret_mask.to_string().repeat(MASK_CELLS)
    }

    /// Paint (preferred over StatefulWidget when parts needed).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TextInputState,
    ) -> TextInputParts {
        state.parts = None;
        if area.is_empty() {
            return TextInputParts {
                root: area,
                field: area,
                prefix: None,
                suffix: None,
                clear: None,
                cursor: None,
            };
        }

        let theme = self.system.junie_theme();
        let invalid = matches!(self.validation, Validation::Invalid(_));
        let visual = VisualState {
            focused: state.focused && state.enabled,
            hovered: state.hovered && state.enabled && !state.editing,
            disabled: !state.enabled,
            error: invalid,
            editing: state.editing && state.focused && state.enabled,
            busy: state.loading,
            ..VisualState::default()
        };
        let field_style = theme.field_style(visual);
        let field_bg = field_style.bg.unwrap_or(theme.field);

        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let mut label = crate::widgets::label::Label::<()>::new(self.label, self.system);
            if self.required {
                label = label.required();
            } else if self.show_optional {
                label = label.optional();
            }
            if !state.enabled {
                label = label.disabled();
            } else if state.focused {
                label = label.focused();
            }
            let indent = 2u16.min(area.width);
            let label_area = Rect::new(
                area.x.saturating_add(indent),
                y,
                area.width.saturating_sub(indent),
                1,
            );
            let _ = label.paint(label_area, buffer);
            y = y.saturating_add(1);
        }

        let field_row = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1,
        );
        buffer.set_style(field_row, field_style);
        let gutter = theme.gutter(visual, field_bg, false);
        buffer.set_stringn(
            field_row.x,
            field_row.y,
            self.system.glyphs.selection_gutter(),
            1,
            gutter,
        );

        let mut x = field_row.x.saturating_add(2);
        let mut prefix_rect = None;
        let mut suffix_rect = None;
        let mut clear_rect = None;

        if let Some(p) = self.prefix {
            if !p.is_empty() {
                let pw =
                    display_cols(p).min(usize::from(field_row.right().saturating_sub(x))) as u16;
                buffer.set_stringn(
                    x,
                    field_row.y,
                    take_display_cols(p, usize::from(pw)),
                    usize::from(pw),
                    theme.placeholder(visual),
                );
                prefix_rect = Some(Rect::new(x, field_row.y, pw, 1));
                x = x.saturating_add(pw).saturating_add(1);
            }
        }

        let mut right = field_row.right();
        if invalid && right > x.saturating_add(2) {
            right = right.saturating_sub(2);
        }
        let show_clear = self.show_clear
            && state.focused
            && state.can_edit()
            && !state.value.is_empty()
            && right > x.saturating_add(2);
        if self.show_clear && right > x.saturating_add(2) {
            let cw = 1u16;
            right = right.saturating_sub(cw.saturating_add(1));
            if show_clear {
                clear_rect = Some(Rect::new(right.saturating_add(1), field_row.y, cw, 1));
            }
        }
        if let Some(s) = self.suffix {
            if !s.is_empty() && right > x.saturating_add(2) {
                let sw = display_cols(s).min(4) as u16;
                right = right.saturating_sub(sw.saturating_add(1));
                suffix_rect = Some(Rect::new(right.saturating_add(1), field_row.y, sw, 1));
            }
        }

        let field = Rect::new(x, field_row.y, right.saturating_sub(x).max(1), 1);
        let field_w = usize::from(field.width);
        state.reveal_cursor(field_w);

        let empty = state.value.is_empty();
        let painted = if empty {
            truncate_cols(self.placeholder, field_w, self.system.glyphs.ellipsis()).into_owned()
        } else if self.secret {
            take_display_cols(&self.masked_display(), field_w).into_owned()
        } else {
            take_display_cols(&state.value[state.viewport..], field_w).into_owned()
        };
        let mut text_style = if empty {
            theme.placeholder(visual)
        } else if !state.enabled {
            theme.faint().bg(field_bg)
        } else {
            Style::new().fg(theme.text_primary).bg(field_bg)
        };
        // junie `input.rs`: underline ONLY while editing, always accent.
        // Idle invalid is the trailing bold `!` plus helper, not a red line.
        if visual.editing {
            text_style = text_style
                .add_modifier(Modifier::UNDERLINED)
                .underline_color(theme.accent);
        }
        buffer.set_stringn(field.x, field.y, &painted, field_w, text_style);

        if let Some((a, b)) = state.selection_range() {
            let a = a.max(state.viewport);
            if b > a {
                let start_col = UnicodeWidthStr::width(&state.value[state.viewport..a]);
                let end_col =
                    UnicodeWidthStr::width(&state.value[state.viewport..b.min(state.value.len())]);
                let sx = field
                    .x
                    .saturating_add(u16::try_from(start_col).unwrap_or(0));
                let ex = field
                    .x
                    .saturating_add(u16::try_from(end_col).unwrap_or(0))
                    .min(field.right());
                if ex > sx {
                    buffer.set_style(
                        Rect::new(sx, field.y, ex.saturating_sub(sx), 1),
                        self.system.selected_text(),
                    );
                }
            }
        }

        let cursor_column = if self.secret && !empty {
            0
        } else {
            UnicodeWidthStr::width(
                &state.value[state.viewport..state.cursor.min(state.value.len())],
            )
        };
        let cursor_x = field
            .x
            .saturating_add(u16::try_from(cursor_column).unwrap_or(u16::MAX))
            .min(field.right().saturating_sub(1));
        let cursor_rect = if visual.editing {
            Some(Rect::new(cursor_x, field.y, 1, 1))
        } else {
            None
        };

        if invalid {
            let bang_x = field_row.right().saturating_sub(2);
            if bang_x >= field_row.x {
                buffer.set_stringn(
                    bang_x,
                    field_row.y,
                    Glyph::Error.resolve().text,
                    1,
                    Style::new()
                        .fg(theme.error)
                        .bg(field_bg)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        if let Some(sr) = suffix_rect {
            if let Some(s) = self.suffix {
                buffer.set_stringn(
                    sr.x,
                    sr.y,
                    take_display_cols(s, usize::from(sr.width)),
                    usize::from(sr.width),
                    theme.placeholder(visual),
                );
            }
        }
        if let Some(cr) = clear_rect {
            let clear_recipe = self.system.button_recipe(
                ButtonRecipeVariant::Quiet,
                if state.enabled {
                    ControlState::Focused
                } else {
                    ControlState::Disabled
                },
                theme.surface,
            );
            buffer.set_style(cr, clear_recipe.fill);
            buffer.set_stringn(
                cr.x,
                cr.y,
                self.system.glyphs.resolve(Glyph::Close).text,
                1,
                clear_recipe.label,
            );
        }
        if state.loading {
            let g = self.system.glyphs.loading();
            if field.width > 0 {
                buffer.set_stringn(
                    field.right().saturating_sub(1),
                    field.y,
                    g,
                    1,
                    theme.placeholder(visual),
                );
            }
        }

        if field_row.y.saturating_add(1) < area.bottom() {
            let msg_row = Rect::new(
                area.x.saturating_add(2.min(area.width)),
                field_row.y.saturating_add(1),
                area.width.saturating_sub(2.min(area.width)),
                1,
            );
            match self.validation {
                Validation::Invalid(msg) => {
                    crate::widgets::field_message::paint_field_message(
                        buffer,
                        msg_row,
                        self.system,
                        crate::widgets::label::DescriptionKind::Error,
                        msg,
                    );
                }
                Validation::Valid if !self.help.is_empty() => {
                    crate::widgets::field_message::paint_field_message(
                        buffer,
                        msg_row,
                        self.system,
                        crate::widgets::label::DescriptionKind::Help,
                        self.help,
                    );
                }
                Validation::Valid => {}
            }
        }

        let parts = TextInputParts {
            root: area,
            field,
            prefix: prefix_rect,
            suffix: suffix_rect,
            clear: clear_rect,
            cursor: cursor_rect,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Clear hit test helper.
    ///
    /// First click focuses; a second click on an already-focused field starts
    /// editing and places the caret.
    pub fn handle_mouse(&self, state: &mut TextInputState, event: MouseEvent) -> TextInputOutcome {
        let Some(parts) = state.parts.clone() else {
            return TextInputOutcome::Ignored;
        };
        if let Some(clear) = parts.clear {
            if clear.contains(event.position)
                && matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
                && state.can_edit()
            {
                if state.clear() {
                    return TextInputOutcome::Cleared;
                }
                return TextInputOutcome::Ignored;
            }
        }
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
            && (parts.field.contains(event.position) || parts.root.contains(event.position))
        {
            if !state.focused {
                state.set_focused(true);
                return TextInputOutcome::Changed;
            }
            if !state.editing {
                state.begin_edit();
            }
        }
        state.handle_mouse(event, parts.field)
    }
}

impl StatefulWidget for &TextInput<'_> {
    type State = TextInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for TextInput<'_> {
    type State = TextInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::input::KeyEventKind;

    #[test]
    fn editing_uses_hardware_cursor_not_a_reversed_cell() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 16, 1);
        let mut state = TextInputState::new("abc");
        state.set_focused(true);
        state.set_editing(true);
        assert!(state.set_cursor_byte(0), "the caret sits on a grapheme");
        let mut buffer = Buffer::empty(area);
        let parts = TextInput::new("", &system).paint(area, &mut buffer, &mut state);

        let cursor = parts.cursor.expect("editing publishes a hardware caret");
        let cell = &buffer[(cursor.x, cursor.y)];
        assert_eq!(cell.symbol(), "a");
        assert_eq!(cell.fg, theme.text_primary);
        assert_eq!(cell.bg, theme.field);
        assert!(!cell.style().add_modifier.contains(Modifier::REVERSED));
        assert_eq!(cell.style().underline_color, Some(theme.accent));
    }

    #[test]
    fn selected_text_uses_the_selection_pair() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 16, 1);
        let mut state = TextInputState::new("abcdef");
        state.set_focused(true);
        assert!(state.apply(EditAction::Home { select: false }));
        assert!(state.apply(EditAction::MoveRight { select: true }));
        assert!(state.apply(EditAction::MoveRight { select: true }));
        let mut buffer = Buffer::empty(area);
        let parts = TextInput::new("", &system).paint(area, &mut buffer, &mut state);

        let cell = &buffer[(parts.field.x, area.y)];
        assert_eq!(cell.fg, theme.text_primary);
        assert_eq!(cell.bg, theme.popover, "selection is text on popover");
        assert!(!cell.style().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn focus_gutter_keeps_value_column_stable() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 16, 1);
        let mut idle = TextInputState::new("abc");
        idle.set_editing(false);
        let mut idle_buffer = Buffer::empty(area);
        let idle_parts = TextInput::new("", &system).paint(area, &mut idle_buffer, &mut idle);

        let mut focused = TextInputState::new("abc");
        focused.set_focused(true);
        focused.set_editing(false);
        let mut focused_buffer = Buffer::empty(area);
        let focused_parts =
            TextInput::new("", &system).paint(area, &mut focused_buffer, &mut focused);

        assert_eq!(idle_parts.field.x, focused_parts.field.x);
        assert_eq!(idle_parts.field.x, area.x + 2);
        assert_eq!(idle_buffer[(area.x + 2, area.y)].symbol(), "a");
        assert_eq!(focused_buffer[(area.x + 2, area.y)].symbol(), "a");
        assert_eq!(focused_buffer[(area.x, area.y)].symbol(), "▎");
    }

    #[test]
    fn conditional_clear_action_does_not_resize_the_field() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 16, 1);
        let input = TextInput::new("", &system).show_clear(true);

        let mut idle = TextInputState::new("abc");
        let mut idle_buffer = Buffer::empty(area);
        let idle_parts = input.paint(area, &mut idle_buffer, &mut idle);

        let mut focused = TextInputState::new("abc");
        focused.set_focused(true);
        let mut focused_buffer = Buffer::empty(area);
        let focused_parts = input.paint(area, &mut focused_buffer, &mut focused);

        assert_eq!(idle_parts.field, focused_parts.field);
        assert!(idle_parts.clear.is_none());
        assert!(focused_parts.clear.is_some());
    }

    #[test]
    fn a_required_field_says_so_before_you_submit() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 20, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = TextInputState::new("");
        TextInput::new("Email", &system)
            .required(true)
            .paint(area, &mut buffer, &mut state);

        let label: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            label.trim_end().ends_with('*'),
            "the requirement is marked beside the name, got {label:?}"
        );

        let mut plain = Buffer::empty(area);
        let mut plain_state = TextInputState::new("");
        TextInput::new("Email", &system).paint(area, &mut plain, &mut plain_state);
        let plain_label: String = (0..area.width)
            .map(|x| plain[(x, 0)].symbol().to_string())
            .collect();
        assert!(!plain_label.contains('*'));
    }

    #[test]
    fn an_invalid_field_trails_a_bang_and_keeps_the_value_tone() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 28, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = TextInputState::new("x");
        TextInput::new("Email", &system)
            .validation(Validation::Invalid("not an address"))
            .paint(area, &mut buffer, &mut state);

        let field_row: String = (0..area.width)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        assert!(
            field_row.contains('!'),
            "invalid fields trail a bold `!`, got {field_row:?}"
        );
        let bang = &buffer[(area.width - 2, 1)];
        assert_eq!(bang.symbol(), "!");
        assert_eq!(bang.fg, theme.error);
        assert!(bang.style().add_modifier.contains(Modifier::BOLD));
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "x")
            .expect("value");
        assert_eq!(value.fg, theme.text_primary);
        assert!(
            !value.style().add_modifier.contains(Modifier::UNDERLINED),
            "idle invalid value is not underlined"
        );
        assert_ne!(value.style().underline_color, Some(theme.error));
        let msg: String = (0..area.width)
            .map(|x| buffer[(x, 2)].symbol().to_string())
            .collect();
        assert!(msg.contains("not an address"), "{msg:?}");
        assert!(
            !msg.contains('•'),
            "error copy must not use the pending bullet"
        );
    }

    #[test]
    fn an_invalid_editing_field_underlines_in_accent_and_trails_a_bang() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 28, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = TextInputState::new("x");
        state.set_focused(true);
        state.set_editing(true);
        TextInput::new("Email", &system)
            .validation(Validation::Invalid("not an address"))
            .paint(area, &mut buffer, &mut state);

        let field_row: String = (0..area.width)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        assert!(
            field_row.contains('!'),
            "invalid fields trail a bold `!`, got {field_row:?}"
        );
        let bang = &buffer[(area.width - 2, 1)];
        assert_eq!(bang.symbol(), "!");
        assert_eq!(bang.fg, theme.error);
        assert!(bang.style().add_modifier.contains(Modifier::BOLD));
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "x")
            .expect("value");
        assert_eq!(value.fg, theme.text_primary);
        assert!(value.style().add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(value.style().underline_color, Some(theme.accent));
    }

    fn row_text(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn idle_field_is_field_plane_with_hidden_gutter() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TextInputState::new("Ada");
        state.set_editing(false);
        let mut buffer = Buffer::empty(area);
        TextInput::new("Name", &system).paint(area, &mut buffer, &mut state);
        let gutter = &buffer[(0, 1)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, gutter.bg);
        assert_eq!(gutter.bg, theme.field);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.bg, theme.field);
        assert_eq!(value.fg, theme.text_primary);
    }

    #[test]
    fn focused_editing_keeps_field_plane_accent_bar_and_underline() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TextInputState::new("Ada");
        state.set_focused(true);
        state.set_editing(true);
        state.set_hovered(true);
        let mut buffer = Buffer::empty(area);
        let parts = TextInput::new("Name", &system).paint(area, &mut buffer, &mut state);
        let gutter = &buffer[(0, 1)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, theme.accent);
        assert_eq!(gutter.bg, theme.field);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.bg, theme.field, "editing does not lift the well");
        assert_eq!(value.style().underline_color, Some(theme.accent));
        assert!(parts.cursor.is_some(), "hardware cursor while editing");
        assert!(
            buffer[(2, 0)].style().add_modifier.contains(Modifier::BOLD),
            "focused label is bold"
        );
        assert_eq!(buffer[(2, 0)].fg, theme.text_primary);
    }

    #[test]
    fn hover_not_editing_lifts_to_field_hover() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TextInputState::new("Ada");
        state.set_editing(false);
        state.set_hovered(true);
        let mut buffer = Buffer::empty(area);
        TextInput::new("Name", &system).paint(area, &mut buffer, &mut state);
        assert_eq!(buffer[(2, 1)].bg, theme.field_hover);
        assert_eq!(buffer[(0, 1)].fg, buffer[(0, 1)].bg);
    }

    #[test]
    fn disabled_is_faint_and_does_not_hover() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TextInputState::new("Ada");
        state.set_enabled(false);
        state.set_hovered(true);
        state.set_editing(false);
        let mut buffer = Buffer::empty(area);
        TextInput::new("Name", &system).paint(area, &mut buffer, &mut state);
        let gutter = &buffer[(0, 1)];
        assert_eq!(gutter.fg, gutter.bg, "disabled has no focus bar");
        assert_eq!(gutter.bg, theme.field);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.fg, theme.disabled);
        assert_eq!(value.bg, theme.field);
    }

    #[test]
    fn password_mask_is_eight_cells() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 28, 3);
        let mut state = TextInputState::new("hunter2-secret");
        state.set_focused(true);
        let mut buffer = Buffer::empty(area);
        TextInput::new("Password", &system)
            .secret(true)
            .paint(area, &mut buffer, &mut state);
        let row = row_text(&buffer, 1, area.width);
        assert!(
            row.contains(&"●".repeat(MASK_CELLS)),
            "masked secret is {MASK_CELLS} cells, got {row:?}"
        );
        assert!(!row.contains("hunter"), "{row:?}");
        assert!(
            !row.contains(&"●".repeat(MASK_CELLS + 1)),
            "mask must not track length: {row:?}"
        );
    }

    #[test]
    fn emacs_chords_home_end_kill_and_words() {
        let mut state = TextInputState::new("hello world");
        state.set_focused(true);
        state.set_editing(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            TextInputOutcome::Changed
        );
        assert_eq!(state.cursor_byte(), 0);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            TextInputOutcome::Changed
        );
        assert_eq!(state.cursor_byte(), state.value().len());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT)),
            TextInputOutcome::Changed
        );
        assert_eq!(&state.value()[state.cursor_byte()..], "world");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            TextInputOutcome::Changed
        );
        assert_eq!(state.value(), "world");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            TextInputOutcome::Changed
        );
        assert_eq!(state.value(), "");
    }

    #[test]
    fn repeated_one_shot_actions_are_ignored_but_text_repeats() {
        let mut state = TextInputState::new("hello world");
        state.set_focused(true);
        state.set_editing(true);
        let actions = [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::BackTab, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('m'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('m'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
            (KeyCode::Char('u'), KeyModifiers::CONTROL),
            (KeyCode::Char('k'), KeyModifiers::CONTROL),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
        ];
        for (code, modifiers) in actions {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(state.handle_key(key), TextInputOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated text-input state");
        }

        let mut repeat_text = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE);
        repeat_text.kind = KeyEventKind::Repeat;
        assert_eq!(
            state.handle_key(repeat_text),
            TextInputOutcome::Changed,
            "ordinary text repeats remain supported"
        );
        assert_eq!(state.value(), "hello world!");
    }

    #[test]
    fn enter_begins_edit_when_navigating_esc_reverts() {
        let mut state = TextInputState::new("keep");
        state.set_focused(true);
        state.set_editing(false);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Changed
        );
        assert!(state.is_editing());
        assert!(state.apply(EditAction::Insert('!')));
        assert_eq!(state.value(), "keep!");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TextInputOutcome::Cancelled
        );
        assert_eq!(state.value(), "keep");
        assert!(!state.is_editing());
    }

    #[test]
    fn new_focused_field_is_nav_not_editing() {
        let mut state = TextInputState::new("abc");
        state.set_focused(true);
        assert!(!state.is_editing());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TextInputOutcome::Ignored
        );
        assert_eq!(state.value(), "abc");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Changed
        );
        assert!(state.is_editing());
    }

    #[test]
    fn first_click_focuses_second_click_edits() {
        let system = DesignSystem::junie();
        let mut state = TextInputState::new("abc");
        state.set_editing(false);
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let input = TextInput::new("", &system);
        let _ = input.paint(area, &mut buf, &mut state);
        let down = click(4, 0);
        assert_eq!(
            input.handle_mouse(&mut state, down),
            TextInputOutcome::Changed
        );
        assert!(state.is_focused());
        assert!(!state.is_editing());
        assert_eq!(
            input.handle_mouse(&mut state, down),
            TextInputOutcome::Changed
        );
        assert!(state.is_editing());
    }

    #[test]
    fn helper_text_is_muted_below_the_field() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 32, 3);
        let mut state = TextInputState::new("Ada");
        state.set_editing(false);
        let mut buffer = Buffer::empty(area);
        TextInput::new("Name", &system)
            .help("Display name")
            .paint(area, &mut buffer, &mut state);
        let msg = row_text(&buffer, 2, area.width);
        assert!(msg.contains("Display name"), "{msg:?}");
        assert_eq!(buffer[(2, 2)].fg, theme.text_muted);
    }

    #[test]
    fn overflow_placeholder_uses_ellipsis_not_hard_clip() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 16, 1);
        let mut state = TextInputState::new("");
        let mut buffer = Buffer::empty(area);
        TextInput::new("", &system)
            .placeholder("What should Junie do, and what does done look like?")
            .paint(area, &mut buffer, &mut state);
        let line = row_text(&buffer, 0, area.width);
        assert!(
            line.contains(system.glyphs.ellipsis()),
            "overflow placeholder must mark the cut, got {line:?}"
        );
        assert!(
            !line.contains("look"),
            "overflow placeholder must not hard-clip the tail, got {line:?}"
        );
    }
    use super::*;
    use crate::style::{MASK_CELLS, RolePalette};
    use crate::widgets::tests::click;

    #[test]
    fn keyboard_owns_edit_submit_cancel_and_validation() {
        let mut state = TextInputState::new("")
            .with_forbidden(["taken".to_owned()])
            .with_max_graphemes(5);
        state.set_editing(true);
        for character in "taken!".chars() {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        assert_eq!(state.value(), "taken");
        assert_eq!(state.validity(), TextInputValidity::Forbidden);
        state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Submitted("take".to_owned())
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TextInputOutcome::Ignored,
            "Esc after commit is idle; revert is only while editing"
        );
    }

    #[test]
    fn render_reveals_wide_cursor_in_narrow_viewport() {
        let theme = RolePalette::default();
        let system = DesignSystem::new(theme.clone());
        let mut state = TextInputState::new("alpha🧪");
        state.set_focused(true);
        let area = Rect::new(3, 2, 4, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        (&TextInput::new("Name", &system)).render(area, &mut buffer, &mut state);
        assert!(state.viewport > 0);
        assert!(state.cursor_byte() >= state.viewport);
    }

    #[test]
    fn external_cursor_accepts_grapheme_boundaries_and_rejects_splits() {
        let mut state = TextInputState::new("a👩‍💻🧪");

        assert!(state.set_cursor_byte(1));
        assert_eq!(state.cursor_byte(), 1);
        assert!(!state.set_cursor_byte(2));
        assert_eq!(state.cursor_byte(), 1);
        assert!(state.set_cursor_byte("a👩‍💻".len()));
        assert_eq!(state.cursor_byte(), "a👩‍💻".len());
        assert!(state.set_cursor_byte(state.value().len()));
    }

    #[test]
    fn insertion_repairs_cursor_after_merging_with_leading_combining_mark() {
        let mut state = TextInputState::new("\u{301}x");
        assert!(state.set_cursor_byte(0));
        assert!(state.apply(EditAction::Insert('e')));
        assert_eq!(state.value(), "e\u{301}x");
        assert_eq!(state.cursor_byte(), "e\u{301}".len());
        assert!(state.set_cursor_byte(state.cursor_byte()));
    }

    #[test]
    fn clear_preserves_configuration_and_resets_editing_state() {
        let mut state = TextInputState::new("taken")
            .with_forbidden(["taken".to_owned()])
            .with_max_graphemes(3)
            .with_allow_empty(true);
        assert!(state.clear());
        assert_eq!(state.value(), "");
        assert_eq!(state.cursor_byte(), 0);
        assert!(state.is_valid());
        assert!(!state.is_editing());
        state.set_editing(true);
        for character in "abcd".chars() {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        assert_eq!(state.value(), "abc");
        assert!(state.clear());
        assert!(!state.clear());
    }

    #[test]
    fn selection_shift_and_delete() {
        let mut state = TextInputState::new("hello");
        state.set_cursor_byte(0);
        assert!(state.apply(EditAction::MoveRight { select: true }));
        assert!(state.apply(EditAction::MoveRight { select: true }));
        assert!(state.selection_range().is_some());
        assert_eq!(state.selected_text(), Some("he"));
        assert!(state.apply(EditAction::Delete));
        assert_eq!(state.value(), "llo");
    }

    #[test]
    fn word_movement() {
        let mut state = TextInputState::new("foo bar baz");
        state.set_cursor_byte(state.value().len());
        assert!(state.apply(EditAction::WordLeft { select: false }));
        assert_eq!(&state.value()[state.cursor_byte()..], "baz");
        assert!(state.apply(EditAction::WordLeft { select: false }));
        assert_eq!(&state.value()[state.cursor_byte()..], "bar baz");
    }

    #[test]
    fn undo_redo() {
        let mut state = TextInputState::new("a");
        assert!(state.apply(EditAction::Insert('b')));
        assert_eq!(state.value(), "ab");
        assert!(state.apply(EditAction::Undo));
        assert_eq!(state.value(), "a");
        assert!(state.apply(EditAction::Redo));
        assert_eq!(state.value(), "ab");
    }

    #[test]
    fn clipboard_outcomes() {
        let mut state = TextInputState::new("hello");
        state.set_editing(true);
        state.set_cursor_byte(0);
        let _ = state.apply(EditAction::End { select: true });
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(
            out,
            TextInputOutcome::ClipboardCopy { ref text } if text == "hello"
        ));
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL));
        assert_eq!(out, TextInputOutcome::ClipboardPasteRequest);
    }

    #[test]
    fn paste_strips_newlines_and_controls() {
        let mut state = TextInputState::new("").with_allow_empty(true);
        assert_eq!(state.insert_str("ab\ncd\x01ef"), TextInputOutcome::Changed);
        assert_eq!(state.value(), "ab");
    }

    #[test]
    fn secret_and_prefix_paint() {
        let system = DesignSystem::default();
        let mut state = TextInputState::new("secret");
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 2));
        let parts = TextInput::new("Password", &system)
            .secret(true)
            .prefix("🔒")
            .show_clear(true)
            .paint(Rect::new(0, 0, 30, 2), &mut buf, &mut state);
        assert!(parts.field.width > 0);
        assert!(parts.prefix.is_some() || parts.root.width > 0);
    }

    #[test]
    fn read_only_blocks_insert() {
        let mut state = TextInputState::new("x");
        state.set_read_only(true);
        assert!(!state.apply(EditAction::Insert('y')));
        assert_eq!(state.value(), "x");
        assert!(state.apply(EditAction::MoveLeft { select: false }));
    }

    #[test]
    fn mouse_places_cursor() {
        let system = DesignSystem::default();
        let mut state = TextInputState::new("abcdef");
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let parts = TextInput::new("", &system).paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let out = state.handle_mouse(
            click(parts.field.x.saturating_add(2), parts.field.y),
            parts.field,
        );
        assert!(matches!(out, TextInputOutcome::Changed));
        assert!(state.cursor_byte() > 0);
    }

    #[test]
    fn unicode_fuzz_random_ops_keep_boundary() {
        let samples = [
            "a",
            "東京",
            "e\u{301}",
            "👩‍🔬",
            "a\u{200d}b",
            "café",
            "\u{301}x",
        ];
        let actions = [
            EditAction::Insert('z'),
            EditAction::Insert('あ'),
            EditAction::Backspace,
            EditAction::Delete,
            EditAction::MoveLeft { select: false },
            EditAction::MoveRight { select: true },
            EditAction::WordLeft { select: false },
            EditAction::WordRight { select: false },
            EditAction::Home { select: false },
            EditAction::End { select: true },
            EditAction::SelectAll,
        ];
        for seed in samples {
            let mut state = TextInputState::new(seed).with_allow_empty(true);
            for (i, action) in actions.iter().cycle().take(40).enumerate() {
                let _ = state.apply(action.clone());
                assert!(
                    edit_core::is_boundary(state.value(), state.cursor_byte()),
                    "seed={seed:?} step={i} cursor={}",
                    state.cursor_byte()
                );
            }
        }
    }

    #[test]
    fn long_input_hot_path() {
        let system = DesignSystem::default();
        let mut state = TextInputState::new("").with_allow_empty(true);
        let chunk = "αβγδεζηθικλμνξοπρστυφχψω";
        for _ in 0..200 {
            let _ = state.insert_str(chunk);
        }
        assert!(state.value().len() > 1000);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let input = TextInput::new("q", &system);
        for _ in 0..100 {
            let _ = input.paint(area, &mut buf, &mut state);
        }
        assert!(state.parts().is_some());
    }

    #[test]
    fn intent_submit_and_move() {
        let mut state = TextInputState::new("ok").with_allow_empty(true);
        assert_eq!(
            state.handle_intent(UiIntent::Submit),
            TextInputOutcome::Submitted("ok".into())
        );
        let _ = state.apply(EditAction::End { select: false });
        assert!(matches!(
            state.handle_intent(UiIntent::Move(crate::interaction::NavigationMove::Left)),
            TextInputOutcome::Changed | TextInputOutcome::Ignored
        ));
    }

    #[test]
    fn select_all_legacy_helpers() {
        let mut state = TextInputState::new("ab");
        assert!(state.apply(EditAction::move_left()));
        assert!(state.apply(EditAction::home()));
        assert!(state.apply(EditAction::end()));
        assert_eq!(state.cursor_byte(), 2);
    }
}
