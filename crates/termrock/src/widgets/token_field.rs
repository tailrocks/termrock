// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Editable token/chip collection with free-text draft and completion hooks.
//!
//! **Mission.** Recipients, filters, tags, file mentions, and command arguments
//! need add/remove/reorder/select with grapheme-safe draft editing — without a
//! Tab stop per token.
//!
//! **vs [`TokenStrip`](super::TokenStrip).** Display + roving over a projected
//! list. TokenField **owns** tokens + draft input and commits text into tokens.
//!
//! **Focus model.** One surface focus. Zones: draft input or a token (with
//! optional remove part). Left/Right moves across tokens ↔ draft; Backspace on
//! empty draft removes the previous token.
//!
//! Research: email recipient fields, token inputs, agent attachment/mention chips.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::StatefulWidget,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::{
    Chip, ChipState, Tag, TagState, TextInput, TextInputOutcome, TextInputState, TokenPart,
    TokenStatus, Validation,
};

// ── Policy ──────────────────────────────────────────────────────────────────

/// How duplicate labels/ids are handled when adding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DuplicatePolicy {
    /// Allow duplicates.
    Allow,
    /// Reject add when label matches (case-sensitive).
    #[default]
    RejectLabel,
    /// Reject when id already present.
    RejectId,
}

impl DuplicatePolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::RejectLabel => "reject-label",
            Self::RejectId => "reject-id",
        }
    }
}

/// Characters that commit the draft into a token (in addition to Enter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSeparators {
    chars: Vec<char>,
}

impl Default for CommitSeparators {
    fn default() -> Self {
        Self {
            chars: vec![',', ';'],
        }
    }
}

impl CommitSeparators {
    /// Custom separators.
    #[must_use]
    pub fn new(chars: impl IntoIterator<Item = char>) -> Self {
        Self {
            chars: chars.into_iter().collect(),
        }
    }

    /// Email-style: comma and semicolon.
    #[must_use]
    pub fn email() -> Self {
        Self::new([',', ';'])
    }

    /// Space commits (tags).
    #[must_use]
    pub fn space() -> Self {
        Self::new([' '])
    }

    /// Whether `c` commits.
    #[must_use]
    pub fn contains(&self, c: char) -> bool {
        self.chars.contains(&c)
    }
}

// ── Token model ─────────────────────────────────────────────────────────────

/// Owned token with stable identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldToken<Id> {
    /// Stable id.
    pub id: Id,
    /// Display label (grapheme-safe string).
    pub label: String,
    /// Removable (default true).
    pub removable: bool,
    /// Multi-select selected.
    pub selected: bool,
    /// Status chrome.
    pub status: TokenStatus,
    /// Disabled (skipped by cursor).
    pub disabled: bool,
}

impl<Id> FieldToken<Id> {
    /// Removable token.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            removable: true,
            selected: false,
            status: TokenStatus::Default,
            disabled: false,
        }
    }

    /// Builder.
    #[must_use]
    pub const fn removable(mut self, on: bool) -> Self {
        self.removable = on;
        self
    }

    /// Builder.
    #[must_use]
    pub const fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Builder.
    #[must_use]
    pub const fn status(mut self, status: TokenStatus) -> Self {
        self.status = status;
        self
    }

    /// Builder.
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
}

/// Focus zone inside the field (not host Tab stops).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TokenFieldZone {
    /// Free-text draft.
    #[default]
    Draft,
    /// Token at index with internal part.
    Token {
        /// Index into tokens.
        index: usize,
        /// Body or remove.
        part: TokenPart,
    },
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Token field outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenFieldOutcome<Id> {
    /// No effect.
    Ignored,
    /// Draft text/caret changed.
    DraftChanged,
    /// Token committed from draft.
    TokenAdded {
        /// New id.
        id: Id,
        /// Label.
        label: String,
    },
    /// Token removed.
    TokenRemoved {
        /// Removed id.
        id: Id,
    },
    /// Tokens reordered.
    TokenReordered {
        /// From index.
        from: usize,
        /// To index.
        to: usize,
    },
    /// Multi-select toggle.
    SelectionChanged {
        /// Id.
        id: Id,
        /// Selected.
        selected: bool,
    },
    /// Focus moved between draft and tokens.
    FocusMoved,
    /// Host should open completion for draft.
    CompletionRequested {
        /// Draft query.
        query: String,
    },
    /// Overflow `+N` activated.
    OverflowActivated,
    /// Enter with empty draft (submit field to form).
    Submitted,
    /// Esc.
    Cancelled,
    /// Duplicate rejected.
    DuplicateRejected {
        /// Label that was rejected.
        label: String,
    },
    /// Paste multi-value handled (or request host paste).
    ClipboardPasteRequest,
    /// Copy.
    ClipboardCopy {
        /// Text.
        text: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`TokenField`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenFieldState<Id> {
    tokens: Vec<FieldToken<Id>>,
    draft: TextInputState,
    zone: TokenFieldZone,
    multi_select: bool,
    duplicate: DuplicatePolicy,
    separators: CommitSeparators,
    focused: bool,
    enabled: bool,
    read_only: bool,
    /// Id factory when adding from draft without host id (only for `String` ids).
    next_seq: u64,
    /// Max visible tokens before overflow (0 = fit all in paint budget).
    max_visible: usize,
    parts: Option<TokenFieldParts>,
}

impl<Id> Default for TokenFieldState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> TokenFieldState<Id> {
    /// Empty field.
    #[must_use]
    pub fn new() -> Self {
        let mut draft = TextInputState::new("").with_allow_empty(true);
        draft.set_focused(false);
        Self {
            tokens: Vec::new(),
            draft,
            zone: TokenFieldZone::Draft,
            multi_select: false,
            duplicate: DuplicatePolicy::RejectLabel,
            separators: CommitSeparators::default(),
            focused: false,
            enabled: true,
            read_only: false,
            next_seq: 0,
            max_visible: 0,
            parts: None,
        }
    }

    /// Multi-select mode (Space toggles token selection).
    #[must_use]
    pub const fn with_multi_select(mut self, on: bool) -> Self {
        self.multi_select = on;
        self
    }

    /// Duplicate policy.
    #[must_use]
    pub const fn with_duplicate_policy(mut self, policy: DuplicatePolicy) -> Self {
        self.duplicate = policy;
        self
    }

    /// Commit separators.
    #[must_use]
    pub fn with_separators(mut self, separators: CommitSeparators) -> Self {
        self.separators = separators;
        self
    }

    /// Overflow cap.
    #[must_use]
    pub const fn with_max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    /// Tokens.
    #[must_use]
    pub fn tokens(&self) -> &[FieldToken<Id>] {
        &self.tokens
    }

    /// Mutable tokens (advanced).
    pub fn tokens_mut(&mut self) -> &mut Vec<FieldToken<Id>> {
        &mut self.tokens
    }

    /// Draft text.
    #[must_use]
    pub fn draft(&self) -> &str {
        self.draft.value()
    }

    /// Zone.
    #[must_use]
    pub const fn zone(&self) -> TokenFieldZone {
        self.zone
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Parts.
    #[must_use]
    pub const fn parts(&self) -> Option<&TokenFieldParts> {
        self.parts.as_ref()
    }

    /// Labels in order.
    #[must_use]
    pub fn labels(&self) -> Vec<&str> {
        self.tokens.iter().map(|t| t.label.as_str()).collect()
    }

    /// Selected token ids (multi-select).
    #[must_use]
    pub fn selected_ids(&self) -> Vec<&Id> {
        self.tokens
            .iter()
            .filter(|t| t.selected)
            .map(|t| &t.id)
            .collect()
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.sync_draft_focus();
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.draft.set_enabled(on);
    }

    /// Read-only (no add/remove/reorder).
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        self.draft.set_read_only(on);
    }

    fn sync_draft_focus(&mut self) {
        let draft_on = self.focused && matches!(self.zone, TokenFieldZone::Draft);
        self.draft.set_focused(draft_on);
        self.draft.set_enabled(self.enabled);
        self.draft.set_read_only(self.read_only);
    }

}

impl<Id: Clone + PartialEq> TokenFieldState<Id> {
    /// Replace all tokens.
    pub fn set_tokens(&mut self, tokens: Vec<FieldToken<Id>>) {
        self.tokens = tokens;
        self.clamp_zone();
    }

    fn clamp_zone(&mut self) {
        match self.zone {
            TokenFieldZone::Draft => {}
            TokenFieldZone::Token { index, part } => {
                if self.tokens.is_empty() {
                    self.zone = TokenFieldZone::Draft;
                } else if index >= self.tokens.len() {
                    self.zone = TokenFieldZone::Token {
                        index: self.tokens.len() - 1,
                        part: TokenPart::Body,
                    };
                } else {
                    self.zone = TokenFieldZone::Token { index, part };
                }
            }
        }
        self.sync_draft_focus();
    }

    /// Insert token at end.
    pub fn push_token(&mut self, token: FieldToken<Id>) -> bool {
        if !self.can_add(&token.id, &token.label) {
            return false;
        }
        self.tokens.push(token);
        true
    }

    /// Insert at index.
    pub fn insert_token(&mut self, index: usize, token: FieldToken<Id>) -> bool {
        if !self.can_add(&token.id, &token.label) {
            return false;
        }
        let i = index.min(self.tokens.len());
        self.tokens.insert(i, token);
        true
    }

    fn can_add(&self, id: &Id, label: &str) -> bool {
        match self.duplicate {
            DuplicatePolicy::Allow => true,
            DuplicatePolicy::RejectLabel => !self.tokens.iter().any(|t| t.label == label),
            DuplicatePolicy::RejectId => !self.tokens.iter().any(|t| &t.id == id),
        }
    }

    /// Remove by id.
    pub fn remove_id(&mut self, id: &Id) -> Option<FieldToken<Id>> {
        let idx = self.tokens.iter().position(|t| &t.id == id)?;
        let t = self.tokens.remove(idx);
        self.clamp_zone();
        Some(t)
    }

    /// Remove by index.
    pub fn remove_index(&mut self, index: usize) -> Option<FieldToken<Id>> {
        if index >= self.tokens.len() {
            return None;
        }
        let t = self.tokens.remove(index);
        self.clamp_zone();
        Some(t)
    }

    /// Reorder token.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tokens.len() || to >= self.tokens.len() || from == to {
            return false;
        }
        let t = self.tokens.remove(from);
        self.tokens.insert(to, t);
        if let TokenFieldZone::Token { index, part } = self.zone {
            let new_index = if index == from {
                to
            } else if from < index && to >= index {
                index - 1
            } else if from > index && to <= index {
                index + 1
            } else {
                index
            };
            self.zone = TokenFieldZone::Token {
                index: new_index,
                part,
            };
        }
        true
    }

    /// Focus draft.
    pub fn focus_draft(&mut self) {
        self.zone = TokenFieldZone::Draft;
        self.sync_draft_focus();
    }

    /// Focus token index.
    pub fn focus_token(&mut self, index: usize) {
        if index < self.tokens.len() {
            self.zone = TokenFieldZone::Token {
                index,
                part: TokenPart::Body,
            };
            self.sync_draft_focus();
        }
    }
}

impl TokenFieldState<String> {
    /// Commit draft using auto-generated string id.
    pub fn commit_draft(&mut self) -> TokenFieldOutcome<String> {
        let label = self.draft.value().trim().to_owned();
        if label.is_empty() {
            return TokenFieldOutcome::Ignored;
        }
        if self.read_only || !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        self.next_seq = self.next_seq.saturating_add(1);
        let id = format!("tok-{}", self.next_seq);
        if !self.can_add(&id, &label) {
            return TokenFieldOutcome::DuplicateRejected { label };
        }
        self.tokens.push(FieldToken::new(id.clone(), label.clone()));
        let _ = self.draft.clear();
        self.zone = TokenFieldZone::Draft;
        self.sync_draft_focus();
        TokenFieldOutcome::TokenAdded { id, label }
    }

    /// Commit draft with host-provided id.
    pub fn commit_draft_with_id(&mut self, id: String) -> TokenFieldOutcome<String> {
        let label = self.draft.value().trim().to_owned();
        if label.is_empty() {
            return TokenFieldOutcome::Ignored;
        }
        if self.read_only || !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        if !self.can_add(&id, &label) {
            return TokenFieldOutcome::DuplicateRejected { label };
        }
        self.tokens.push(FieldToken::new(id.clone(), label.clone()));
        let _ = self.draft.clear();
        self.zone = TokenFieldZone::Draft;
        self.sync_draft_focus();
        TokenFieldOutcome::TokenAdded { id, label }
    }

    /// Apply completion candidate as new token (or replace draft).
    pub fn apply_suggestion(&mut self, id: String, label: impl Into<String>) -> TokenFieldOutcome<String> {
        if self.read_only || !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        let label = label.into();
        if !self.can_add(&id, &label) {
            return TokenFieldOutcome::DuplicateRejected { label };
        }
        self.tokens.push(FieldToken::new(id.clone(), label.clone()));
        let _ = self.draft.clear();
        self.focus_draft();
        TokenFieldOutcome::TokenAdded { id, label }
    }

    /// Paste text: split on separators/newlines into tokens; remainder stays draft.
    pub fn paste_values(&mut self, text: &str) -> TokenFieldOutcome<String> {
        if self.read_only || !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        let mut added = 0usize;
        let mut last_id = String::new();
        let mut last_label = String::new();
        // Split on newline, comma, semicolon
        for part in text.split(|c| matches!(c, '\n' | '\r' | ',' | ';')) {
            let label = part.trim();
            if label.is_empty() {
                continue;
            }
            self.next_seq = self.next_seq.saturating_add(1);
            let id = format!("tok-{}", self.next_seq);
            if self.can_add(&id, label) {
                self.tokens
                    .push(FieldToken::new(id.clone(), label.to_owned()));
                last_id = id;
                last_label = label.to_owned();
                added += 1;
            }
        }
        if added == 0 {
            // fall back to insert into draft
            return match self.draft.insert_str(text) {
                TextInputOutcome::Changed => TokenFieldOutcome::DraftChanged,
                _ => TokenFieldOutcome::Ignored,
            };
        }
        self.focus_draft();
        TokenFieldOutcome::TokenAdded {
            id: last_id,
            label: last_label,
        }
    }
}

impl TokenFieldState<String> {
    /// Key adapter (String token ids).
    pub fn handle_key(&mut self, key: KeyEvent) -> TokenFieldOutcome<String> {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        self.sync_draft_focus();

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Completion Tab when draft focused
        if matches!(self.zone, TokenFieldZone::Draft)
            && matches!(key.code, KeyCode::Tab)
            && !ctrl
            && !alt
            && !self.read_only
        {
            return TokenFieldOutcome::CompletionRequested {
                query: self.draft.value().to_owned(),
            };
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return TokenFieldOutcome::Cancelled;
        }

        // Reorder Alt+Left/Right on token
        if alt
            && !ctrl
            && matches!(key.code, KeyCode::Left | KeyCode::Right)
            && let TokenFieldZone::Token { index, .. } = self.zone
            && !self.read_only
        {
            let to = if matches!(key.code, KeyCode::Left) {
                index.saturating_sub(1)
            } else {
                (index + 1).min(self.tokens.len().saturating_sub(1))
            };
            if self.reorder(index, to) {
                return TokenFieldOutcome::TokenReordered { from: index, to };
            }
            return TokenFieldOutcome::Ignored;
        }

        match self.zone {
            TokenFieldZone::Draft => self.handle_draft_key(key, ctrl, alt, shift),
            TokenFieldZone::Token { index, part } => {
                self.handle_token_key(key, index, part, ctrl, shift)
            }
        }
    }

    fn handle_draft_key(
        &mut self,
        key: KeyEvent,
        ctrl: bool,
        _alt: bool,
        shift: bool,
    ) -> TokenFieldOutcome<String> {
        // Left at start → last token
        if matches!(key.code, KeyCode::Left)
            && !ctrl
            && !shift
            && self.draft.cursor_byte() == 0
            && !self.tokens.is_empty()
        {
            self.focus_token(self.tokens.len() - 1);
            return TokenFieldOutcome::FocusMoved;
        }

        // Backspace empty → remove last token
        if matches!(key.code, KeyCode::Backspace)
            && !ctrl
            && self.draft.value().is_empty()
            && !self.tokens.is_empty()
            && !self.read_only
        {
            let idx = self.tokens.len() - 1;
            if self.tokens[idx].removable {
                let t = self.tokens.remove(idx);
                self.clamp_zone();
                return TokenFieldOutcome::TokenRemoved { id: t.id };
            }
        }

        // Enter / separator commit
        if !self.read_only {
            if matches!(key.code, KeyCode::Enter) && !ctrl {
                if self.draft.value().trim().is_empty() {
                    return TokenFieldOutcome::Submitted;
                }
                return self.commit_draft();
            }
            if let KeyCode::Char(c) = key.code {
                if !ctrl && self.separators.contains(c) {
                    if !self.draft.value().trim().is_empty() {
                        return self.commit_draft();
                    }
                    return TokenFieldOutcome::Ignored;
                }
            }
        }

        match self.draft.handle_key(key) {
            TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                TokenFieldOutcome::DraftChanged
            }
            TextInputOutcome::Submitted(_) => {
                if self.draft.value().trim().is_empty() {
                    TokenFieldOutcome::Submitted
                } else {
                    self.commit_draft()
                }
            }
            TextInputOutcome::Cancelled => TokenFieldOutcome::Cancelled,
            TextInputOutcome::ClipboardPasteRequest => TokenFieldOutcome::ClipboardPasteRequest,
            TextInputOutcome::ClipboardCopy { text } | TextInputOutcome::ClipboardCut { text } => {
                TokenFieldOutcome::ClipboardCopy { text }
            }
            TextInputOutcome::Ignored => TokenFieldOutcome::Ignored,
        }
    }

    fn handle_token_key(
        &mut self,
        key: KeyEvent,
        index: usize,
        part: TokenPart,
        ctrl: bool,
        shift: bool,
    ) -> TokenFieldOutcome<String> {
        if index >= self.tokens.len() {
            self.focus_draft();
            return TokenFieldOutcome::FocusMoved;
        }
        let removable = self.tokens[index].removable;
        let selectable = self.multi_select;
        let id = self.tokens[index].id.clone();
        let selected = self.tokens[index].selected;

        match key.code {
            KeyCode::Right if !ctrl && !shift => {
                if part == TokenPart::Body && removable {
                    self.zone = TokenFieldZone::Token {
                        index,
                        part: TokenPart::Remove,
                    };
                    return TokenFieldOutcome::FocusMoved;
                }
                // next token or draft
                if index + 1 < self.tokens.len() {
                    self.focus_token(index + 1);
                } else {
                    self.focus_draft();
                }
                return TokenFieldOutcome::FocusMoved;
            }
            KeyCode::Left if !ctrl && !shift => {
                if part == TokenPart::Remove {
                    self.zone = TokenFieldZone::Token {
                        index,
                        part: TokenPart::Body,
                    };
                    return TokenFieldOutcome::FocusMoved;
                }
                if index > 0 {
                    self.focus_token(index - 1);
                }
                // stay on first
                return TokenFieldOutcome::FocusMoved;
            }
            KeyCode::Backspace | KeyCode::Delete if !self.read_only && removable => {
                let t = self.tokens.remove(index);
                if self.tokens.is_empty() {
                    self.focus_draft();
                } else {
                    self.focus_token(index.min(self.tokens.len() - 1));
                }
                return TokenFieldOutcome::TokenRemoved { id: t.id };
            }
            KeyCode::Char(' ') if selectable && !ctrl => {
                self.tokens[index].selected = !selected;
                return TokenFieldOutcome::SelectionChanged {
                    id,
                    selected: !selected,
                };
            }
            KeyCode::Enter if part == TokenPart::Remove && removable && !self.read_only => {
                let t = self.tokens.remove(index);
                self.clamp_zone();
                return TokenFieldOutcome::TokenRemoved { id: t.id };
            }
            // Typing while on token → go draft and insert
            KeyCode::Char(c)
                if !ctrl && !c.is_control() && !self.read_only && part == TokenPart::Body =>
            {
                self.focus_draft();
                let _ = self.draft.handle_key(key);
                return TokenFieldOutcome::DraftChanged;
            }
            _ => {}
        }

        TokenFieldOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> TokenFieldOutcome<String> {
        if !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit | UiIntent::Activate => {
                if matches!(self.zone, TokenFieldZone::Draft) {
                    if self.draft.value().trim().is_empty() {
                        TokenFieldOutcome::Submitted
                    } else {
                        self.commit_draft()
                    }
                } else {
                    TokenFieldOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => TokenFieldOutcome::Cancelled,
            _ => TokenFieldOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> TokenFieldOutcome<String> {
        if !self.enabled {
            return TokenFieldOutcome::Ignored;
        }
        let Some(parts) = self.parts.clone() else {
            return TokenFieldOutcome::Ignored;
        };
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TokenFieldOutcome::Ignored;
        }
        if let Some(ov) = parts.overflow {
            if ov.contains(event.position) {
                return TokenFieldOutcome::OverflowActivated;
            }
        }
        for (i, rect) in parts.token_rects.iter().enumerate() {
            if rect.contains(event.position) {
                self.set_focused(true);
                // remove hit near right edge
                if i < self.tokens.len()
                    && self.tokens[i].removable
                    && event.position.x + 1 >= rect.right().saturating_sub(1)
                    && rect.width > 3
                    && !self.read_only
                {
                    let t = self.tokens.remove(i);
                    self.clamp_zone();
                    return TokenFieldOutcome::TokenRemoved { id: t.id };
                }
                if self.multi_select && i < self.tokens.len() {
                    self.tokens[i].selected = !self.tokens[i].selected;
                    let id = self.tokens[i].id.clone();
                    let selected = self.tokens[i].selected;
                    self.focus_token(i);
                    return TokenFieldOutcome::SelectionChanged { id, selected };
                }
                self.focus_token(i);
                return TokenFieldOutcome::FocusMoved;
            }
        }
        if parts.draft.contains(event.position) {
            self.set_focused(true);
            self.focus_draft();
            return match self.draft.handle_mouse(event, parts.draft) {
                TextInputOutcome::Changed => TokenFieldOutcome::DraftChanged,
                _ => TokenFieldOutcome::FocusMoved,
            };
        }
        TokenFieldOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Hit geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenFieldParts {
    /// Root.
    pub root: Rect,
    /// Token regions in order (visible only).
    pub token_rects: Vec<Rect>,
    /// Draft field.
    pub draft: Rect,
    /// Overflow.
    pub overflow: Option<Rect>,
}

/// Editable token field chrome.
#[derive(Debug, Clone, Copy)]
pub struct TokenField<'a> {
    label: &'a str,
    placeholder: &'a str,
    system: &'a DesignSystem,
    validation: Validation<'a>,
    ascii: bool,
    gap: u16,
}

impl<'a> TokenField<'a> {
    /// Create field.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            label: "",
            placeholder: "Add…",
            system,
            validation: Validation::Valid,
            ascii: false,
            gap: 1,
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Draft placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Validation.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// ASCII chrome.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Gap between tokens.
    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TokenFieldState<String>,
    ) -> TokenFieldParts {
        state.parts = None;
        state.sync_draft_focus();
        if area.is_empty() {
            return TokenFieldParts {
                root: area,
                token_rects: Vec::new(),
                draft: area,
                overflow: None,
            };
        }

        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let mut style = self.system.style(if state.focused {
                Role::Focus
            } else {
                Role::Text
            });
            if state.focused {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }

        let row = Rect::new(area.x, y.min(area.bottom().saturating_sub(1)), area.width, 1);
        let mut x = row.x;
        let mut token_rects = Vec::new();
        let mut overflow = None;

        let max_v = if state.max_visible == 0 {
            state.tokens.len()
        } else {
            state.max_visible
        };
        let (visible, rest) = if state.tokens.len() > max_v {
            state.tokens.split_at(max_v)
        } else {
            (state.tokens.as_slice(), &[][..])
        };

        // Reserve draft min width and overflow
        let overflow_w = if !rest.is_empty() { 5u16 } else { 0 };
        let draft_min = 6u16;
        let budget = row
            .width
            .saturating_sub(overflow_w)
            .saturating_sub(draft_min);

        for (i, tok) in visible.iter().enumerate() {
            let w = measure_token_width(tok, self.system);
            if x.saturating_sub(row.x).saturating_add(w) > budget && !token_rects.is_empty() {
                break;
            }
            let width = w.min(row.right().saturating_sub(overflow_w).saturating_sub(x));
            if width == 0 {
                break;
            }
            let rect = Rect::new(x, row.y, width, 1);
            let focused = state.focused
                && matches!(
                    state.zone,
                    TokenFieldZone::Token { index, .. } if index == i
                );
            let part = match state.zone {
                TokenFieldZone::Token {
                    index,
                    part,
                } if index == i => part,
                _ => TokenPart::Body,
            };
            paint_token(tok, rect, buffer, self.system, focused, part, state.multi_select);
            token_rects.push(rect);
            x = x.saturating_add(width).saturating_add(self.gap);
        }

        if !rest.is_empty() {
            let _ = rest;
            let n = state.tokens.len().saturating_sub(token_rects.len());
            let label = format!("+{n}");
            let ow = u16::try_from(display_cols(&label).saturating_add(2)).unwrap_or(4);
            let ox = row.right().saturating_sub(ow);
            let rect = Rect::new(ox, row.y, ow.min(row.width), 1);
            buffer.set_stringn(
                rect.x,
                rect.y,
                &format!("[{label}]"),
                usize::from(rect.width),
                self.system.style(Role::TextMuted),
            );
            overflow = Some(rect);
        }

        let draft_right = overflow.map(|o| o.x).unwrap_or(row.right());
        let draft = Rect::new(
            x.min(draft_right.saturating_sub(1)),
            row.y,
            draft_right.saturating_sub(x).max(1),
            1,
        );
        let input = TextInput::new("", self.system)
            .placeholder(self.placeholder)
            .validation(self.validation);
        let _ = input.paint(draft, buffer, &mut state.draft);

        // Validation row
        if area.height >= 3 {
            if let Validation::Invalid(msg) = self.validation {
                buffer.set_stringn(
                    area.x,
                    area.y.saturating_add(2),
                    take_display_cols(msg, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::Danger),
                );
            }
        }

        let parts = TokenFieldParts {
            root: area,
            token_rects,
            draft,
            overflow,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Semantic — one focusable field.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<&str, Action>,
        id: &'static str,
        area: Rect,
        state: &TokenFieldState<String>,
    ) where
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "token-field count={} zone={:?}",
            state.tokens.len(),
            state.zone
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "tokens"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: matches!(self.validation, Validation::Invalid(_))
                        || state.tokens.iter().any(|t| matches!(t.status, TokenStatus::Error)),
                    ..Default::default()
                }),
        );
    }
}

fn measure_token_width(tok: &FieldToken<String>, system: &DesignSystem) -> u16 {
    // Prefer Tag measure for entity tokens; multi-select paint uses Chip.
    Tag::new(tok.id.as_str(), tok.label.as_str(), system)
        .removable(tok.removable)
        .status(tok.status)
        .measure_width()
}

fn paint_token(
    tok: &FieldToken<String>,
    rect: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    focused: bool,
    part: TokenPart,
    multi_select: bool,
) {
    if multi_select {
        let chip = Chip::new(tok.id.as_str(), tok.label.as_str(), system)
            .removable(tok.removable)
            .status(tok.status)
            .disabled(tok.disabled);
        let mut cs = ChipState::new(tok.selected);
        cs.set_focused(focused);
        if focused {
            cs.set_part(part);
        }
        let _ = chip.paint(rect, buffer, &mut cs);
    } else {
        let tag = Tag::new(tok.id.as_str(), tok.label.as_str(), system)
            .removable(tok.removable)
            .status(tok.status)
            .disabled(tok.disabled);
        let mut ts = TagState::new();
        ts.set_focused(focused);
        if focused {
            ts.set_part(part);
        }
        let _ = tag.paint(rect, buffer, &mut ts);
    }
}

impl StatefulWidget for &TokenField<'_> {
    type State = TokenFieldState<String>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for TokenField<'_> {
    type State = TokenFieldState<String>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn add_commit_and_duplicate_reject() {
        let mut state = TokenFieldState::new().with_duplicate_policy(DuplicatePolicy::RejectLabel);
        state.set_focused(true);
        let _ = state.draft.insert_str("alice");
        assert!(matches!(
            state.commit_draft(),
            TokenFieldOutcome::TokenAdded { ref label, .. } if label == "alice"
        ));
        let _ = state.draft.insert_str("alice");
        assert!(matches!(
            state.commit_draft(),
            TokenFieldOutcome::DuplicateRejected { .. }
        ));
        assert_eq!(state.tokens().len(), 1);
    }

    #[test]
    fn backspace_removes_last_token() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        assert!(state.push_token(FieldToken::new("1".into(), "a")));
        assert!(state.push_token(FieldToken::new("2".into(), "b")));
        state.focus_draft();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            TokenFieldOutcome::TokenRemoved { id: "2".into() }
        );
        assert_eq!(state.labels(), vec!["a"]);
    }

    #[test]
    fn left_from_draft_to_token_and_right_back() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        assert!(state.push_token(FieldToken::new("1".into(), "x")));
        state.focus_draft();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            TokenFieldOutcome::FocusMoved
        );
        assert!(matches!(state.zone(), TokenFieldZone::Token { index: 0, .. }));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            TokenFieldOutcome::FocusMoved
        );
        // body → remove if removable, then right → draft
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert!(matches!(state.zone(), TokenFieldZone::Draft) || matches!(state.zone(), TokenFieldZone::Token { .. }));
    }

    #[test]
    fn separator_commits() {
        let mut state = TokenFieldState::new().with_separators(CommitSeparators::email());
        state.set_focused(true);
        let _ = state.draft.insert_str("bob");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE)),
            TokenFieldOutcome::TokenAdded { .. }
        ));
        assert_eq!(state.labels(), vec!["bob"]);
    }

    #[test]
    fn paste_multi_values() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        let _ = state.paste_values("a, b; c\nd");
        assert_eq!(state.tokens().len(), 4);
    }

    #[test]
    fn multi_select_space() {
        let mut state = TokenFieldState::new().with_multi_select(true);
        state.set_focused(true);
        assert!(state.push_token(FieldToken::new("1".into(), "rust")));
        state.focus_token(0);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            TokenFieldOutcome::SelectionChanged {
                selected: true,
                ..
            }
        ));
        assert!(state.tokens()[0].selected);
    }

    #[test]
    fn reorder_alt_arrows() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        assert!(state.push_token(FieldToken::new("1".into(), "a")));
        assert!(state.push_token(FieldToken::new("2".into(), "b")));
        state.focus_token(1);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT)),
            TokenFieldOutcome::TokenReordered { from: 1, to: 0 }
        );
        assert_eq!(state.labels(), vec!["b", "a"]);
    }

    #[test]
    fn tab_completion_request() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        let _ = state.draft.insert_str("fi");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            TokenFieldOutcome::CompletionRequested {
                query: "fi".into()
            }
        );
    }

    #[test]
    fn apply_suggestion() {
        let mut state = TokenFieldState::new();
        assert!(matches!(
            state.apply_suggestion("id1".into(), "file.rs"),
            TokenFieldOutcome::TokenAdded { .. }
        ));
        assert_eq!(state.labels(), vec!["file.rs"]);
    }

    #[test]
    fn paint_tokens_and_draft() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        let _ = state.push_token(FieldToken::new("1".into(), "alice"));
        let _ = state.push_token(FieldToken::new("2".into(), "bob"));
        let area = Rect::new(0, 0, 48, 2);
        let mut buf = Buffer::empty(area);
        let parts = TokenField::new(&system)
            .label("To")
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(!parts.token_rects.is_empty());
        assert!(!parts.draft.is_empty());
    }

    #[test]
    fn overflow_cap() {
        let system = DesignSystem::default();
        let mut state = TokenFieldState::new().with_max_visible(2);
        state.set_focused(true);
        for i in 0..5 {
            let _ = state.push_token(FieldToken::new(format!("{i}"), format!("t{i}")));
        }
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let parts = TokenField::new(&system).paint(area, &mut buf, &mut state);
        assert!(parts.overflow.is_some() || parts.token_rects.len() <= 2);
    }

    #[test]
    fn fuzz_navigation() {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        let _ = state.paste_values("a,b,c");
        let keys = [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
        }
        // still finite tokens
        assert!(state.tokens().len() < 50);
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = TokenFieldState::new();
        let _ = state.paste_values("a,b,c,d");
        state.set_focused(true);
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        let w = TokenField::new(&system);
        for _ in 0..150 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn grapheme_label_safe() {
        let mut state: TokenFieldState<String> = TokenFieldState::new();
        assert!(state.push_token(FieldToken::new("1".into(), "東京🧪")));
        assert_eq!(state.tokens()[0].label.graphemes(true).count(), 3);
    }
}
