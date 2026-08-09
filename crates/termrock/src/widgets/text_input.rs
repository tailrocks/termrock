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
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{EventResult, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
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
    /// Clear value.
    Clear,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
}

impl EditAction {
    /// Move left without selection (legacy).
    #[must_use]
    pub const fn move_left() -> Self {
        Self::MoveLeft { select: false }
    }

    /// Move right without selection (legacy).
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
    #[cfg_attr(feature = "serde", serde(skip))]
    selecting_with_mouse: bool,
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
fn default_true() -> bool {
    true
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextInputState {
    /// Creates text-input state with the cursor at the end of the value.
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
        if key.kind == KeyEventKind::Release {
            return TextInputOutcome::Ignored;
        }
        if !self.enabled {
            return TextInputOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

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
                KeyCode::Char('a' | 'A') => return self.edit(EditAction::SelectAll),
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
                    // kill word backward
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
            KeyCode::Enter => self.submit(),
            KeyCode::Char('m' | 'M') if ctrl => self.submit(),
            KeyCode::Esc => TextInputOutcome::Cancelled,
            KeyCode::Backspace if ctrl => {
                // word backspace
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
            KeyCode::Left if ctrl || alt => {
                self.edit(EditAction::WordLeft { select: shift })
            }
            KeyCode::Right if ctrl || alt => {
                self.edit(EditAction::WordRight { select: shift })
            }
            KeyCode::Left => self.edit(EditAction::MoveLeft { select: shift }),
            KeyCode::Right => self.edit(EditAction::MoveRight { select: shift }),
            KeyCode::Home => self.edit(EditAction::Home { select: shift }),
            KeyCode::End => self.edit(EditAction::End { select: shift }),
            KeyCode::Char(character)
                if !ctrl
                    && !alt
                    && !character.is_control() =>
            {
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
            UiIntent::Cancel | UiIntent::Close => TextInputOutcome::Cancelled,
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
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        field_area: Rect,
    ) -> TextInputOutcome {
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

    /// EventResult wrapper.
    pub fn handle_key_result(&mut self, key: KeyEvent) -> EventResult<TextInputOutcome> {
        match self.handle_key(key) {
            TextInputOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
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
    placeholder: &'a str,
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
            placeholder: "",
            validation: Validation::Valid,
            system,
            prefix: None,
            suffix: None,
            secret: false,
            show_clear: false,
            secret_mask: '*',
        }
    }

    /// Placeholder while empty.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
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

    /// Secret / password recipe (mask graphemes).
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

    fn masked_display(&self, value: &str) -> String {
        if !self.secret {
            return value.to_owned();
        }
        value
            .graphemes(true)
            .map(|_| self.secret_mask)
            .collect()
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

        let invalid = !state.is_valid() || matches!(self.validation, Validation::Invalid(_));
        let mut y = area.y;
        // Optional label row when height >= 2
        if area.height >= 2 && !self.label.is_empty() {
            let mut style = self.system.style(if invalid {
                Role::Danger
            } else if state.focused {
                Role::Focus
            } else {
                Role::Text
            });
            if state.focused {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            let text = take_display_cols(self.label, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
        }

        let field_row = Rect::new(area.x, y.min(area.bottom().saturating_sub(1)), area.width, 1);
        let mut x = field_row.x;
        let mut prefix_rect = None;
        let mut suffix_rect = None;
        let mut clear_rect = None;

        if let Some(p) = self.prefix {
            if !p.is_empty() {
                let pw = display_cols(p).min(usize::from(field_row.width)) as u16;
                buffer.set_stringn(
                    x,
                    field_row.y,
                    take_display_cols(p, usize::from(pw)),
                    usize::from(pw),
                    self.system.style(Role::TextMuted),
                );
                prefix_rect = Some(Rect::new(x, field_row.y, pw, 1));
                x = x.saturating_add(pw).saturating_add(1);
            }
        }

        let mut right = field_row.right();
        let show_clear = self.show_clear
            && state.focused
            && state.can_edit()
            && !state.value.is_empty()
            && right > x.saturating_add(2);
        if show_clear {
            let cw = 1u16;
            right = right.saturating_sub(cw.saturating_add(1));
            clear_rect = Some(Rect::new(right.saturating_add(1), field_row.y, cw, 1));
        }
        if let Some(s) = self.suffix {
            if !s.is_empty() && right > x.saturating_add(2) {
                let sw = display_cols(s).min(4) as u16;
                right = right.saturating_sub(sw.saturating_add(1));
                suffix_rect = Some(Rect::new(right.saturating_add(1), field_row.y, sw, 1));
            }
        }

        let field = Rect::new(x, field_row.y, right.saturating_sub(x).max(1), 1);
        let input_style = self.system.style(if !state.enabled {
            Role::TextDisabled
        } else if invalid {
            Role::InputInvalid
        } else if state.loading {
            Role::TextMuted
        } else {
            Role::Input
        });
        buffer.set_style(field, input_style);

        let field_w = usize::from(field.width);
        state.reveal_cursor(field_w);

        let display_src = if state.value.is_empty() {
            self.placeholder
        } else {
            &state.value[state.viewport..]
        };
        let painted = if state.value.is_empty() {
            take_display_cols(display_src, field_w)
        } else {
            take_display_cols(&self.masked_display(display_src), field_w)
        };
        let text_style = if state.value.is_empty() {
            self.system.style(Role::TextMuted)
        } else {
            input_style
        };
        buffer.set_stringn(
            field.x,
            field.y,
            &painted,
            field_w,
            text_style,
        );

        // Selection reverse
        if let Some((a, b)) = state.selection_range() {
            let a = a.max(state.viewport);
            if b > a {
                let start_col = UnicodeWidthStr::width(&state.value[state.viewport..a]);
                let end_col = UnicodeWidthStr::width(&state.value[state.viewport..b.min(state.value.len())]);
                let sx = field.x.saturating_add(u16::try_from(start_col).unwrap_or(0));
                let ex = field
                    .x
                    .saturating_add(u16::try_from(end_col).unwrap_or(0))
                    .min(field.right());
                if ex > sx {
                    buffer.set_style(
                        Rect::new(sx, field.y, ex.saturating_sub(sx), 1),
                        self.system
                            .style(Role::Focus)
                            .add_modifier(Modifier::REVERSED),
                    );
                }
            }
        }

        let cursor_column =
            UnicodeWidthStr::width(&state.value[state.viewport..state.cursor.min(state.value.len())]);
        let cursor_x = field
            .x
            .saturating_add(u16::try_from(cursor_column).unwrap_or(u16::MAX))
            .min(field.right().saturating_sub(1));
        let cursor_rect = if state.focused && state.enabled {
            buffer.set_style(
                Rect::new(cursor_x, field.y, 1, 1),
                self.system.style(Role::Focus).add_modifier(Modifier::REVERSED),
            );
            Some(Rect::new(cursor_x, field.y, 1, 1))
        } else {
            None
        };

        if let Some(sr) = suffix_rect {
            if let Some(s) = self.suffix {
                buffer.set_stringn(
                    sr.x,
                    sr.y,
                    take_display_cols(s, usize::from(sr.width)),
                    usize::from(sr.width),
                    self.system.style(Role::TextMuted),
                );
            }
        }
        if let Some(cr) = clear_rect {
            buffer.set_stringn(
                cr.x,
                cr.y,
                "×",
                1,
                self.system.style(Role::TextMuted),
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
                    self.system.style(Role::TextMuted),
                );
            }
        }

        // Validation message on extra row
        if area.height >= 3 {
            if let Validation::Invalid(msg) = self.validation {
                let text = take_display_cols(msg, usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    area.y.saturating_add(2),
                    &text,
                    usize::from(area.width),
                    self.system.style(Role::Danger),
                );
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
    pub fn handle_mouse(
        &self,
        state: &mut TextInputState,
        event: MouseEvent,
    ) -> TextInputOutcome {
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
        state.handle_mouse(event, parts.field)
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &TextInputState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = if self.secret {
            "secret"
        } else if state.value.is_empty() {
            self.placeholder
        } else {
            "text"
        };
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "text input"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: !state.is_valid()
                        || matches!(self.validation, Validation::Invalid(_)),
                    busy: state.loading,
                    ..Default::default()
                }),
        );
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
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn keyboard_owns_edit_submit_cancel_and_validation() {
        let mut state = TextInputState::new("")
            .with_forbidden(["taken".to_owned()])
            .with_max_graphemes(5);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Ignored
        );
        for character in "taken!".chars() {
            state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
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
            TextInputOutcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_wide_cursor_in_narrow_viewport() {
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());
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
        state.set_cursor_byte(0);
        let _ = state.apply(EditAction::End { select: true });
        let out = state.handle_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert!(matches!(
            out,
            TextInputOutcome::ClipboardCopy { ref text } if text == "hello"
        ));
        let out = state.handle_key(KeyEvent::new(
            KeyCode::Char('v'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(out, TextInputOutcome::ClipboardPasteRequest);
    }

    #[test]
    fn paste_strips_newlines_and_controls() {
        let mut state = TextInputState::new("").with_allow_empty(true);
        assert_eq!(
            state.insert_str("ab\ncd\x01ef"),
            TextInputOutcome::Changed
        );
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
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: parts.field.x.saturating_add(2),
                    y: parts.field.y,
                },
                modifiers: KeyModifiers::NONE,
            },
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
            state.handle_intent(UiIntent::Move(
                crate::interaction::NavigationMove::Left
            )),
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
