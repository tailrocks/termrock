// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **KeyValueTable** — dense interactive detail table for metadata & properties.
//!
//! **Mission.** HTTP headers, DB columns, process facts, permission claims, and
//! agent/tool detail panels: key · value · type · source · status · copy · edit ·
//! secret redaction · nested groups · validation · compare/diff · row navigation
//! with **one focus target per row** (actions via chords, not tab soup).
//!
//! Contracts columns → stacked rows under width pressure ([`KvLayout`]).
//!
//! **vs siblings.**
//! - [`super::KeyValueList`] — lighter settings/summary list (prefer for simple panes).
//! - [`super::DetailTable`] — dialog-oriented selection/copy/link surface (kept).
//! - [`super::ObjectInspector`] — nested structure paths (uses KV leaves later).
//!
//! Research: inspector panels, HTTP clients, cloud consoles, TermRock DetailTable.
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Glyph, ListRowVisualState, MASK_CELLS, Role},
    text::{contains_lower_all, display_cols, take_display_cols, wrap_display_cols},
    widgets::{
        data_view::LoadState,
        key_value_list::{KvLayout, KvStatus, kv_stack_below},
    },
};

const GUTTER: u16 = 2;
const TYPE_W: u16 = 8;
const SOURCE_W: u16 = 8;

/// Validation state for a field value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvtValidation {
    /// Valid / no rule.
    #[default]
    Ok,
    /// Soft problem (amber tone + message in footer when selected).
    Warning,
    /// Hard problem (danger tone).
    Error,
}

impl KvtValidation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn role(self) -> Option<Role> {
        match self {
            Self::Ok => None,
            Self::Warning => Some(Role::Warning),
            Self::Error => Some(Role::Danger),
        }
    }
}

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvtMode {
    /// Single value column.
    #[default]
    View,
    /// Side-by-side / before-after (uses [`KvtField::compare`]).
    Compare,
}

impl KvtMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Compare => "compare",
        }
    }
}

/// Row kind in the flat projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvtRowKind {
    /// Ordinary field.
    #[default]
    Field,
    /// Nested group header.
    Group,
    /// Visual separator (non-focusable).
    Separator,
}

/// One projected key/value field (or group header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvtField<'a, Id> {
    /// Stable id.
    pub id: Id,
    /// Key / group title.
    pub key: &'a str,
    /// Primary value (empty for pure group headers).
    pub value: &'a str,
    /// Optional type label (`string`, `uuid`, `header`, …).
    pub value_type: Option<&'a str>,
    /// Optional source (`req`, `env`, `claim`, …).
    pub source: Option<&'a str>,
    /// Secondary annotation (unit, path). Dropped first when tight.
    pub annotation: Option<&'a str>,
    /// Optional link on the value.
    pub href: Option<&'a str>,
    /// Compare-side value for [`KvtMode::Compare`] (before / remote / baseline).
    pub compare: Option<&'a str>,
    /// Host may copy primary (or secret plaintext when revealed).
    pub copyable: bool,
    /// Secret — paint redacted unless revealed.
    pub secret: bool,
    /// Inline edit allowed (`e` / EditStarted).
    pub editable: bool,
    /// Status tone for the primary value.
    pub status: Option<KvStatus>,
    /// Validation.
    pub validation: KvtValidation,
    /// Optional validation message (shown in footer when selected).
    pub validation_message: Option<&'a str>,
    /// Nesting depth.
    pub depth: u8,
    /// Row kind.
    pub kind: KvtRowKind,
}

impl<'a, Id> KvtField<'a, Id> {
    /// Simple key/value field.
    #[must_use]
    pub const fn pair(id: Id, key: &'a str, value: &'a str) -> Self {
        Self {
            id,
            key,
            value,
            value_type: None,
            source: None,
            annotation: None,
            href: None,
            compare: None,
            copyable: false,
            secret: false,
            editable: false,
            status: None,
            validation: KvtValidation::Ok,
            validation_message: None,
            depth: 0,
            kind: KvtRowKind::Field,
        }
    }

    /// Group header.
    #[must_use]
    pub const fn group(id: Id, title: &'a str) -> Self {
        Self {
            id,
            key: title,
            value: "",
            value_type: None,
            source: None,
            annotation: None,
            href: None,
            compare: None,
            copyable: false,
            secret: false,
            editable: false,
            status: None,
            validation: KvtValidation::Ok,
            validation_message: None,
            depth: 0,
            kind: KvtRowKind::Group,
        }
    }

    /// Separator.
    #[must_use]
    pub const fn separator(id: Id) -> Self {
        Self {
            id,
            key: "",
            value: "",
            value_type: None,
            source: None,
            annotation: None,
            href: None,
            compare: None,
            copyable: false,
            secret: false,
            editable: false,
            status: None,
            validation: KvtValidation::Ok,
            validation_message: None,
            depth: 0,
            kind: KvtRowKind::Separator,
        }
    }

    /// Sets the type label column.
    #[must_use]
    pub const fn value_type(mut self, t: &'a str) -> Self {
        self.value_type = Some(t);
        self
    }

    /// Sets the source label column.
    #[must_use]
    pub const fn source(mut self, s: &'a str) -> Self {
        self.source = Some(s);
        self
    }

    /// Sets secondary annotation text.
    #[must_use]
    pub const fn annotation(mut self, a: &'a str) -> Self {
        self.annotation = Some(a);
        self
    }

    /// Sets an activatable hyperlink for the value.
    #[must_use]
    pub const fn href(mut self, href: &'a str) -> Self {
        self.href = Some(href);
        self
    }

    /// Sets the compare-side baseline value for diff mode.
    #[must_use]
    pub const fn compare(mut self, other: &'a str) -> Self {
        self.compare = Some(other);
        self
    }

    /// Enables copy chord / click-to-copy.
    #[must_use]
    pub const fn copyable(mut self) -> Self {
        self.copyable = true;
        self
    }

    /// Marks the value as secret (redacted until revealed).
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Allows inline edit (`e` / EditStarted).
    #[must_use]
    pub const fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Sets status tone for the primary value.
    #[must_use]
    pub const fn status(mut self, status: KvStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets validation severity.
    #[must_use]
    pub const fn validation(mut self, v: KvtValidation) -> Self {
        self.validation = v;
        self
    }

    /// Sets validation message shown in the footer when selected.
    #[must_use]
    pub const fn validation_message(mut self, msg: &'a str) -> Self {
        self.validation_message = Some(msg);
        self
    }

    /// Sets nesting depth for indent / groups.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Whether the row is focusable.
    #[must_use]
    pub const fn focusable(&self) -> bool {
        !matches!(self.kind, KvtRowKind::Separator | KvtRowKind::Group)
    }

    /// Clipboard text (true secret when redacted paint hides it).
    #[must_use]
    pub const fn copy_text(&self) -> &'a str {
        self.value
    }
}

/// Hit region for one painted field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvtRegion<Id> {
    /// Field id.
    pub id: Id,
    /// Full row area.
    pub area: Rect,
    /// Value / action hit zone.
    pub value_area: Rect,
}

/// Outcomes — single focus row; actions via chords.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyValueTableOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved.
    Selected(Id),
    /// Copy request.
    Copy {
        /// Field id.
        id: Id,
        /// Clipboard payload.
        text: String,
    },
    /// Open link.
    ActivateLink {
        /// Field id.
        id: Id,
        /// Destination.
        href: String,
    },
    /// Secret reveal on.
    SecretRevealed(Id),
    /// Secret hide.
    SecretHidden(Id),
    /// Inline edit started (host owns buffer).
    EditStarted(Id),
    /// Edit committed.
    EditCommitted {
        /// Field.
        id: Id,
        /// Proposed text.
        text: String,
    },
    /// Edit cancelled.
    EditCancelled,
    /// Scroll changed.
    Scrolled,
    /// Filter query changed (host may reproject).
    FilterChanged(String),
    /// Mode toggled view/compare.
    ModeChanged(KvtMode),
    /// Cancel / clear filter.
    Cancelled,
    /// Retry load.
    RetryLoad,
}

/// Interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueTableState<Id: Clone + PartialEq> {
    /// Cursor field id.
    cursor: Option<Id>,
    /// Scroll in display rows.
    pub scroll_y: u16,
    /// Viewport height (set on paint).
    pub viewport_rows: u16,
    /// Total display rows (set on paint).
    pub total_rows: u16,
    /// Revealed secret ids.
    revealed: BTreeSet<Id>,
    /// Last copied id (affordance).
    pub copied: Option<Id>,
    /// Presentation mode.
    pub mode: KvtMode,
    /// Layout override (Auto still contracts).
    pub layout: KvLayout,
    /// Load chrome.
    pub load: LoadState,
    /// Filter query (`/` filter keys).
    pub filter: Option<String>,
    /// Inline edit session.
    pub editing: bool,
    /// Edit draft.
    pub edit_draft: String,
    /// Host grants input.
    pub accepts_input: bool,
    /// Hit regions.
    pub regions: Vec<KvtRegion<Id>>,
    painted: Rect,
}

impl<Id: Clone + PartialEq + Ord> Default for KeyValueTableState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq + Ord> KeyValueTableState<Id> {
    /// Fresh state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor: None,
            scroll_y: 0,
            viewport_rows: 0,
            total_rows: 0,
            revealed: BTreeSet::new(),
            copied: None,
            mode: KvtMode::View,
            layout: KvLayout::Auto,
            load: LoadState::Ready { count: 0 },
            filter: None,
            editing: false,
            edit_draft: String::new(),
            accepts_input: true,
            regions: Vec::new(),
            painted: Rect::default(),
        }
    }

    /// With initial cursor.
    #[must_use]
    pub fn with_cursor(mut self, id: Id) -> Self {
        self.cursor = Some(id);
        self
    }

    /// Cursor id.
    #[must_use]
    pub const fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Sets cursor.
    pub fn set_cursor(&mut self, id: Option<Id>) {
        self.cursor = id;
    }

    /// Whether secret is revealed.
    #[must_use]
    pub fn is_revealed(&self, id: &Id) -> bool {
        self.revealed.contains(id)
    }

    /// Toggle secret reveal; returns true if now revealed.
    pub fn toggle_reveal(&mut self, id: Id) -> bool {
        if !self.revealed.remove(&id) {
            self.revealed.insert(id);
            true
        } else {
            false
        }
    }

    /// Scroll by delta display rows.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        let max = self.total_rows.saturating_sub(self.viewport_rows);
        let next = if delta >= 0 {
            self.scroll_y.saturating_add(delta as u16).min(max)
        } else {
            self.scroll_y.saturating_sub((-delta) as u16)
        };
        let changed = next != self.scroll_y;
        self.scroll_y = next;
        changed
    }

    fn clamp_scroll(&mut self) {
        let max = self.total_rows.saturating_sub(self.viewport_rows);
        if self.scroll_y > max {
            self.scroll_y = max;
        }
    }
}

/// Dense interactive key/value detail table.
#[derive(Debug, Clone)]
pub struct KeyValueTable<'a, Id> {
    empty_message: &'a str,
    fields: &'a [KvtField<'a, Id>],
    system: &'a DesignSystem,
    key_width: u16,
    show_type: bool,
    show_source: bool,
    separator: &'a str,
}

impl<'a, Id: Clone + PartialEq + Ord> KeyValueTable<'a, Id> {
    /// Table over borrowed fields.
    #[must_use]
    pub const fn new(fields: &'a [KvtField<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            empty_message: "No fields",
            fields,
            system,
            key_width: 0,
            show_type: true,
            show_source: true,
            separator: system.kv_separator().text(),
        }
    }

    /// Line shown when there is nothing to show.
    ///
    /// A collection that paints nothing when empty reads as broken; it has to
    /// say that it is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    /// Fixed key column width (0 = auto).
    #[must_use]
    pub const fn key_width(mut self, w: u16) -> Self {
        self.key_width = w;
        self
    }

    /// Show type column when width allows.
    #[must_use]
    pub const fn show_type(mut self, on: bool) -> Self {
        self.show_type = on;
        self
    }

    /// Show source column when width allows.
    #[must_use]
    pub const fn show_source(mut self, on: bool) -> Self {
        self.show_source = on;
        self
    }

    /// Separator between key and value in columns mode.
    #[must_use]
    pub const fn separator(mut self, sep: &'a str) -> Self {
        self.separator = sep;
        self
    }

    fn resolved_layout(&self, width: u16, state: &KeyValueTableState<Id>) -> KvLayout {
        match state.layout {
            KvLayout::Auto => {
                if width < kv_stack_below() {
                    KvLayout::Stacked
                } else {
                    KvLayout::Columns
                }
            }
            other => other,
        }
    }

    fn show_meta_cols(&self, width: u16, layout: KvLayout) -> (bool, bool) {
        if matches!(layout, KvLayout::Stacked) || width < 48 {
            return (false, false);
        }
        let show_type = self.show_type && width >= 56;
        let show_source = self.show_source && width >= 68;
        (show_type, show_source)
    }

    fn key_col_w(&self, fields: &[KvtField<'a, Id>]) -> usize {
        if self.key_width > 0 {
            return usize::from(self.key_width);
        }
        fields
            .iter()
            .filter(|f| f.focusable() || matches!(f.kind, KvtRowKind::Group))
            .map(|f| display_cols(f.key) + usize::from(f.depth) * 2)
            .max()
            .unwrap_or(8)
            .clamp(4, 28)
    }

    /// Display value with redaction.
    #[must_use]
    pub fn display_value(
        &self,
        field: &KvtField<'a, Id>,
        state: &KeyValueTableState<Id>,
    ) -> String {
        if field.secret && !state.is_revealed(&field.id) {
            return Glyph::Mask.resolve().text.repeat(MASK_CELLS);
        }
        if state.editing && state.cursor.as_ref() == Some(&field.id) {
            return state.edit_draft.clone();
        }
        field.value.to_string()
    }

    fn filtered<'b>(&'b self, state: &KeyValueTableState<Id>) -> Vec<&'b KvtField<'a, Id>> {
        let Some(q) = state.filter.as_ref().map(|s| s.to_ascii_lowercase()) else {
            return self.fields.iter().collect();
        };
        if q.is_empty() {
            return self.fields.iter().collect();
        }
        // Keep matching fields and their ancestor groups by depth walk.
        let mut keep = vec![false; self.fields.len()];
        for (i, f) in self.fields.iter().enumerate() {
            if contains_lower_all(
                &[
                    f.key,
                    f.value,
                    f.value_type.unwrap_or(""),
                    f.source.unwrap_or(""),
                ],
                &q,
            ) && f.focusable()
            {
                keep[i] = true;
                let mut depth = f.depth;
                let mut j = i;
                while depth > 0 && j > 0 {
                    j -= 1;
                    if self.fields[j].depth < depth {
                        keep[j] = true;
                        depth = self.fields[j].depth;
                    }
                }
            }
        }
        self.fields
            .iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, f)| f)
            .collect()
    }

    fn measure_field_h(
        &self,
        field: &KvtField<'a, Id>,
        width: u16,
        layout: KvLayout,
        state: &KeyValueTableState<Id>,
        show_type: bool,
        show_source: bool,
        compare: bool,
    ) -> u16 {
        if width == 0 {
            return 0;
        }
        match field.kind {
            KvtRowKind::Separator => 1,
            KvtRowKind::Group => 1u16,
            KvtRowKind::Field => {
                let value = self.display_value(field, state);
                match layout {
                    KvLayout::Stacked | KvLayout::Auto => {
                        let indent = u16::from(field.depth) * 2;
                        let vw = usize::from(width.saturating_sub(indent).max(1));
                        let mut lines = 1 + wrap_display_cols(&value, vw).len().max(1);
                        if compare {
                            if let Some(c) = field.compare {
                                lines += wrap_display_cols(c, vw).len().max(1);
                            }
                        }
                        if field.annotation.is_some() {
                            lines += 1;
                        }
                        u16::try_from(lines).unwrap_or(u16::MAX)
                    }
                    KvLayout::Columns => {
                        let key_w = self.key_col_w(std::slice::from_ref(field));
                        let mut used = GUTTER
                            + u16::from(field.depth) * 2
                            + u16::try_from(key_w).unwrap_or(8)
                            + u16::try_from(display_cols(self.separator)).unwrap_or(2);
                        if show_type {
                            used = used.saturating_add(TYPE_W + 1);
                        }
                        if show_source {
                            used = used.saturating_add(SOURCE_W + 1);
                        }
                        if compare {
                            used = used.saturating_add(width / 4);
                        }
                        let vw = usize::from(width.saturating_sub(used).max(4));
                        let body = if let Some(ann) = field.annotation {
                            format!("{value}  {ann}")
                        } else {
                            value
                        };
                        u16::try_from(wrap_display_cols(&body, vw).len().max(1)).unwrap_or(1)
                    }
                }
            }
        }
    }

    /// Keys — one focus target; chords for actions.
    pub fn handle_key(
        &self,
        state: &mut KeyValueTableState<Id>,
        key: KeyEvent,
    ) -> KeyValueTableOutcome<Id> {
        if !state.accepts_input || key.is_release() {
            return KeyValueTableOutcome::Ignored;
        }
        let is_press = key.is_press();

        if matches!(
            state.load,
            LoadState::Empty { .. } | LoadState::Error { .. } | LoadState::Loading { .. }
        ) {
            if is_press && matches!(key.code, KeyCode::Char('r' | 'R') | KeyCode::Enter) {
                return KeyValueTableOutcome::RetryLoad;
            }
            return KeyValueTableOutcome::Ignored;
        }

        if state.editing {
            return self.handle_edit_key(state, key);
        }

        // Filter mode
        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if state.filter.is_none() {
                state.filter = Some(String::new());
            }
            return KeyValueTableOutcome::FilterChanged(state.filter.clone().unwrap_or_default());
        }
        if let Some(q) = state.filter.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    state.filter = None;
                    return KeyValueTableOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        state.filter = None;
                    }
                    return KeyValueTableOutcome::FilterChanged(
                        state.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return KeyValueTableOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        if is_press && matches!(key.code, KeyCode::Char('d' | 'D')) && key.modifiers.is_empty() {
            state.mode = match state.mode {
                KvtMode::View => KvtMode::Compare,
                KvtMode::Compare => KvtMode::View,
            };
            return KeyValueTableOutcome::ModeChanged(state.mode);
        }

        if is_press && matches!(key.code, KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            if let Some(id) = state.cursor.clone() {
                if let Some(f) = self.fields.iter().find(|f| f.id == id) {
                    if f.copyable || f.secret {
                        state.copied = Some(id.clone());
                        return KeyValueTableOutcome::Copy {
                            id,
                            text: f.copy_text().to_string(),
                        };
                    }
                }
            }
        }

        if is_press && matches!(key.code, KeyCode::Char('r' | 'R')) && key.modifiers.is_empty() {
            if let Some(id) = state.cursor.clone() {
                if let Some(f) = self.fields.iter().find(|f| f.id == id) {
                    if f.secret {
                        let on = state.toggle_reveal(id.clone());
                        return if on {
                            KeyValueTableOutcome::SecretRevealed(id)
                        } else {
                            KeyValueTableOutcome::SecretHidden(id)
                        };
                    }
                }
            }
        }

        if is_press && matches!(key.code, KeyCode::Char('e' | 'E')) && key.modifiers.is_empty() {
            if let Some(id) = state.cursor.clone() {
                if let Some(f) = self.fields.iter().find(|f| f.id == id) {
                    if f.editable && !f.secret {
                        state.editing = true;
                        state.edit_draft = f.value.to_string();
                        return KeyValueTableOutcome::EditStarted(id);
                    }
                }
            }
        }

        if let Some(intent) = crate::interaction::default_list_intent(key) {
            return self.handle_intent(state, intent);
        }
        KeyValueTableOutcome::Ignored
    }

    fn handle_edit_key(
        &self,
        state: &mut KeyValueTableState<Id>,
        key: KeyEvent,
    ) -> KeyValueTableOutcome<Id> {
        if !key.is_press() {
            return KeyValueTableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                state.editing = false;
                state.edit_draft.clear();
                KeyValueTableOutcome::EditCancelled
            }
            KeyCode::Enter => {
                let Some(id) = state.cursor.clone() else {
                    state.editing = false;
                    return KeyValueTableOutcome::EditCancelled;
                };
                let text = std::mem::take(&mut state.edit_draft);
                state.editing = false;
                KeyValueTableOutcome::EditCommitted { id, text }
            }
            KeyCode::Backspace => {
                state.edit_draft.pop();
                KeyValueTableOutcome::Ignored
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.edit_draft.push(c);
                KeyValueTableOutcome::Ignored
            }
            _ => KeyValueTableOutcome::Ignored,
        }
    }

    /// Semantic intents.
    pub fn handle_intent(
        &self,
        state: &mut KeyValueTableState<Id>,
        intent: UiIntent,
    ) -> KeyValueTableOutcome<Id> {
        if !state.accepts_input {
            return KeyValueTableOutcome::Ignored;
        }
        let view = self.filtered(state);
        let focusable: Vec<&KvtField<'a, Id>> =
            view.iter().copied().filter(|f| f.focusable()).collect();
        if focusable.is_empty() {
            return KeyValueTableOutcome::Ignored;
        }
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                self.move_cursor(state, &focusable, -1)
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                self.move_cursor(state, &focusable, 1)
            }
            UiIntent::Move(NavigationMove::First) => {
                let id = focusable[0].id.clone();
                state.cursor = Some(id.clone());
                state.scroll_y = 0;
                KeyValueTableOutcome::Selected(id)
            }
            UiIntent::Move(NavigationMove::Last) => {
                let id = focusable[focusable.len() - 1].id.clone();
                state.cursor = Some(id.clone());
                KeyValueTableOutcome::Selected(id)
            }
            UiIntent::Page(PageMove::Forward) => {
                if state.scroll_by(i32::from(state.viewport_rows.max(1))) {
                    KeyValueTableOutcome::Scrolled
                } else {
                    KeyValueTableOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                if state.scroll_by(-i32::from(state.viewport_rows.max(1))) {
                    KeyValueTableOutcome::Scrolled
                } else {
                    KeyValueTableOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(id) = state.cursor.clone() {
                    if let Some(f) = self.fields.iter().find(|f| f.id == id) {
                        if let Some(href) = f.href {
                            return KeyValueTableOutcome::ActivateLink {
                                id,
                                href: href.to_string(),
                            };
                        }
                        if f.copyable || f.secret {
                            state.copied = Some(id.clone());
                            return KeyValueTableOutcome::Copy {
                                id,
                                text: f.copy_text().to_string(),
                            };
                        }
                    }
                }
                KeyValueTableOutcome::Ignored
            }
            UiIntent::Cancel => {
                if state.filter.is_some() {
                    state.filter = None;
                    return KeyValueTableOutcome::Cancelled;
                }
                KeyValueTableOutcome::Cancelled
            }
            _ => KeyValueTableOutcome::Ignored,
        }
    }

    fn move_cursor(
        &self,
        state: &mut KeyValueTableState<Id>,
        focusable: &[&KvtField<'a, Id>],
        delta: i32,
    ) -> KeyValueTableOutcome<Id> {
        if focusable.is_empty() {
            return KeyValueTableOutcome::Ignored;
        }
        let cur = state
            .cursor
            .as_ref()
            .and_then(|id| focusable.iter().position(|f| &f.id == id))
            .unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub((-delta) as usize)
        } else {
            (cur + delta as usize).min(focusable.len() - 1)
        };
        let id = focusable[next].id.clone();
        if state.cursor.as_ref() == Some(&id) {
            return KeyValueTableOutcome::Ignored;
        }
        state.cursor = Some(id.clone());
        KeyValueTableOutcome::Selected(id)
    }

    /// Mouse: click select / activate value zone.
    pub fn handle_mouse(
        &self,
        state: &mut KeyValueTableState<Id>,
        event: MouseEvent,
    ) -> KeyValueTableOutcome<Id> {
        if !state.accepts_input {
            return KeyValueTableOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollUp if state.painted.contains(event.position) => {
                if state.scroll_by(-1) {
                    KeyValueTableOutcome::Scrolled
                } else {
                    KeyValueTableOutcome::Ignored
                }
            }
            MouseEventKind::ScrollDown if state.painted.contains(event.position) => {
                if state.scroll_by(1) {
                    KeyValueTableOutcome::Scrolled
                } else {
                    KeyValueTableOutcome::Ignored
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = state
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    let id = r.id.clone();
                    if state.cursor.as_ref() == Some(&id) && r.value_area.contains(event.position) {
                        if let Some(f) = self.fields.iter().find(|f| f.id == id) {
                            if let Some(href) = f.href {
                                return KeyValueTableOutcome::ActivateLink {
                                    id,
                                    href: href.to_string(),
                                };
                            }
                            if f.copyable {
                                state.copied = Some(id.clone());
                                return KeyValueTableOutcome::Copy {
                                    id,
                                    text: f.copy_text().to_string(),
                                };
                            }
                        }
                    }
                    state.cursor = Some(id.clone());
                    return KeyValueTableOutcome::Selected(id);
                }
                KeyValueTableOutcome::Ignored
            }
            _ => KeyValueTableOutcome::Ignored,
        }
    }

    /// Paint O(viewport) display rows.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut KeyValueTableState<Id>) {
        state.regions.clear();
        state.painted = area;
        if area.is_empty() {
            return;
        }

        if self.fields.is_empty() {
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(self.empty_message, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }
        // Footer row for mode / filter / validation
        let footer_h = 1u16;
        let body = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(footer_h),
        };

        match &state.load {
            LoadState::Empty { message } => {
                paint_line(
                    buffer,
                    body.x,
                    body.y,
                    body.width,
                    &format!("{}{}", "∅ ", message.as_deref().unwrap_or("No fields")),
                    self.system.style(Role::TextMuted),
                );
                self.paint_footer(area, buffer, state, None);
                return;
            }
            LoadState::Loading { message } => {
                paint_line(
                    buffer,
                    body.x,
                    body.y,
                    body.width,
                    &format!("{}{}", "… ", message.as_deref().unwrap_or("Loading…")),
                    self.system.style(Role::TextMuted),
                );
                self.paint_footer(area, buffer, state, None);
                return;
            }
            LoadState::Error { message, .. } => {
                paint_line(
                    buffer,
                    body.x,
                    body.y,
                    body.width,
                    &format!("! {message}  (r retry)"),
                    self.system.style(Role::Danger),
                );
                self.paint_footer(area, buffer, state, None);
                return;
            }
            _ => {}
        }

        let layout = self.resolved_layout(body.width, state);
        let paint_layout = if matches!(layout, KvLayout::Stacked) {
            KvLayout::Stacked
        } else {
            KvLayout::Columns
        };
        let (show_type, show_source) = self.show_meta_cols(body.width, paint_layout);
        let compare = matches!(state.mode, KvtMode::Compare);
        let view = self.filtered(state);
        let key_w = self.key_col_w(&view.iter().map(|f| (*f).clone()).collect::<Vec<_>>());

        // Field heights in display order; row positions derive
        // arithmetically instead of materializing a per-row map each frame.
        let heights: Vec<u16> = view
            .iter()
            .map(|field| {
                self.measure_field_h(
                    field,
                    body.width,
                    paint_layout,
                    state,
                    show_type,
                    show_source,
                    compare,
                )
            })
            .collect();
        state.total_rows = heights.iter().fold(0u16, |a, h| a.saturating_add(*h));
        state.viewport_rows = body.height;
        state.clamp_scroll();

        // Ensure cursor exists
        if state.cursor.is_none() {
            if let Some(f) = view.iter().find(|f| f.focusable()) {
                state.cursor = Some(f.id.clone());
            }
        }

        let first = usize::from(state.scroll_y);
        let mut first_y: Vec<Option<u16>> = vec![None; view.len()];
        let mut last_y: Vec<Option<u16>> = vec![None; view.len()];

        for row in 0..body.height {
            let idx = first.saturating_add(usize::from(row));
            let Some((vi, sub)) = walk_field_rows(&heights, idx) else {
                break;
            };
            let y = body.y.saturating_add(row);
            first_y[vi] = first_y[vi].or(Some(y));
            last_y[vi] = Some(y);
            let field = view[vi];
            self.paint_field_row(
                field,
                sub,
                paint_layout,
                Rect {
                    x: body.x,
                    y,
                    width: body.width,
                    height: 1,
                },
                buffer,
                state,
                key_w,
                show_type,
                show_source,
                compare,
            );
        }

        for (vi, field) in view.iter().enumerate() {
            if !field.focusable() {
                continue;
            }
            if let (Some(y0), Some(y1)) = (first_y[vi], last_y[vi]) {
                let root = Rect {
                    x: body.x,
                    y: y0,
                    width: body.width,
                    height: y1.saturating_sub(y0).saturating_add(1),
                };
                // Value zone: right half for activation without extra focus targets
                let value_area = Rect {
                    x: body.x.saturating_add(body.width / 3),
                    y: y0,
                    width: body.width.saturating_sub(body.width / 3),
                    height: root.height,
                };
                state.regions.push(KvtRegion {
                    id: field.id.clone(),
                    area: root,
                    value_area,
                });
            }
        }

        let msg = state.cursor.as_ref().and_then(|id| {
            self.fields
                .iter()
                .find(|f| &f.id == id)
                .and_then(|f| f.validation_message)
        });
        self.paint_footer(area, buffer, state, msg);
    }

    fn paint_footer(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &KeyValueTableState<Id>,
        validation_msg: Option<&str>,
    ) {
        let y = area.bottom().saturating_sub(1);
        if y < area.y {
            return;
        }
        let mut parts = Vec::new();
        parts.push(format!("mode:{}", state.mode.id()));
        if let Some(q) = &state.filter {
            parts.push(format!("/{q}"));
        }
        if state.editing {
            parts.push(format!("edit:{}", state.edit_draft));
        }
        if let Some(m) = validation_msg {
            parts.push(m.to_string());
        }
        let separator = " · ";
        parts.push(["c copy", "e edit", "r reveal", "d compare", "/ filter"].join(separator));
        let line = parts.join(separator);
        paint_line(
            buffer,
            area.x,
            y,
            area.width,
            &line,
            self.system.style(Role::TextMuted),
        );
    }

    fn paint_field_row(
        &self,
        field: &KvtField<'a, Id>,
        sub: u16,
        layout: KvLayout,
        area: Rect,
        buffer: &mut Buffer,
        state: &KeyValueTableState<Id>,
        key_w: usize,
        show_type: bool,
        show_source: bool,
        compare: bool,
    ) {
        let selected = state.cursor.as_ref() == Some(&field.id);
        let chrome = crate::widgets::row_chrome::RowChrome::resolve(
            self.system,
            ListRowVisualState {
                selected,
                focused: true,
                enabled: true,
                ..Default::default()
            },
        );
        let mut style = self.system.style(Role::Text);
        if let Some(role) = field.validation.role() {
            style = self.system.style(role);
        } else if let Some(st) = field.status {
            style = self.system.style(st_role(st));
        }
        // Validation and status keep their tone under the cursor.
        let style = chrome.label_style(style);

        buffer.set_stringn(area.x, area.y, " ", 1, style);
        buffer.set_stringn(area.x.saturating_add(1), area.y, " ", 1, style);

        match field.kind {
            KvtRowKind::Separator => {
                let rule = self.system.glyphs.rule();
                for x in area.x..area.right() {
                    buffer.set_stringn(x, area.y, rule, 1, self.system.style(Role::Border));
                }
                paint_kv_row_chrome(&chrome, sub, buffer, area);
                return;
            }
            KvtRowKind::Group => {
                if sub == 0 {
                    let indent = u16::from(field.depth) * 2;
                    let x = area.x.saturating_add(GUTTER).saturating_add(indent);
                    let mark = "▸ ";
                    let line = format!("{mark}{}", field.key);
                    paint_line(
                        buffer,
                        x,
                        area.y,
                        area.right().saturating_sub(x),
                        &line,
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                paint_kv_row_chrome(&chrome, sub, buffer, area);
                return;
            }
            KvtRowKind::Field => {}
        }

        let indent = u16::from(field.depth) * 2;
        let origin = area.x.saturating_add(GUTTER).saturating_add(indent);
        let value = self.display_value(field, state);
        let value_style = if field.secret && !state.is_revealed(&field.id) {
            self.system.style(Role::TextMuted)
        } else {
            style
        };

        match layout {
            KvLayout::Stacked | KvLayout::Auto => {
                if sub == 0 {
                    let mut key_line = field.key.to_string();
                    if show_type {
                        if let Some(t) = field.value_type {
                            key_line.push_str(&format!("  ({t})"));
                        }
                    }
                    if show_source {
                        if let Some(s) = field.source {
                            key_line.push_str(&format!(" @{s}"));
                        }
                    }
                    paint_line(
                        buffer,
                        origin,
                        area.y,
                        area.right().saturating_sub(origin),
                        &key_line,
                        self.system.style(Role::TextMuted),
                    );
                } else {
                    // value / compare / annotation lines
                    let mut line_idx = 1u16;
                    let vw = usize::from(area.right().saturating_sub(origin).max(1));
                    let vlines = wrap_display_cols(&value, vw);
                    if sub < line_idx + vlines.len() as u16 {
                        let i = usize::from(sub.saturating_sub(line_idx));
                        if let Some(l) = vlines.get(i) {
                            paint_line(
                                buffer,
                                origin,
                                area.y,
                                area.right().saturating_sub(origin),
                                l,
                                value_style,
                            );
                        }
                        paint_kv_row_chrome(&chrome, sub, buffer, area);
                        return;
                    }
                    line_idx += vlines.len() as u16;
                    if compare {
                        if let Some(c) = field.compare {
                            let clines = wrap_display_cols(c, vw);
                            if sub < line_idx + clines.len() as u16 {
                                let i = usize::from(sub.saturating_sub(line_idx));
                                if let Some(l) = clines.get(i) {
                                    let prefix = "↔ ";
                                    paint_line(
                                        buffer,
                                        origin,
                                        area.y,
                                        area.right().saturating_sub(origin),
                                        &format!("{prefix}{l}"),
                                        self.system.style(Role::Warning),
                                    );
                                }
                                paint_kv_row_chrome(&chrome, sub, buffer, area);
                                return;
                            }
                            line_idx += clines.len() as u16;
                        }
                    }
                    if let Some(ann) = field.annotation {
                        if sub == line_idx {
                            paint_line(
                                buffer,
                                origin,
                                area.y,
                                area.right().saturating_sub(origin),
                                ann,
                                self.system.style(Role::TextMuted),
                            );
                        }
                    }
                }
            }
            KvLayout::Columns => {
                if sub > 0 {
                    // continuation of wrapped value
                    let mut x = origin
                        + u16::try_from(key_w).unwrap_or(8)
                        + u16::try_from(display_cols(self.separator)).unwrap_or(2);
                    if show_type {
                        x = x.saturating_add(TYPE_W + 1);
                    }
                    if show_source {
                        x = x.saturating_add(SOURCE_W + 1);
                    }
                    let vw = usize::from(area.right().saturating_sub(x).max(1));
                    let body = if let Some(ann) = field.annotation {
                        format!("{value}  {ann}")
                    } else {
                        value.clone()
                    };
                    let lines = wrap_display_cols(&body, vw);
                    if let Some(l) = lines.get(usize::from(sub)) {
                        paint_line(
                            buffer,
                            x,
                            area.y,
                            area.right().saturating_sub(x),
                            l,
                            value_style,
                        );
                    }
                    paint_kv_row_chrome(&chrome, sub, buffer, area);
                    return;
                }
                let mut x = origin;
                // key
                paint_line(
                    buffer,
                    x,
                    area.y,
                    u16::try_from(key_w).unwrap_or(8),
                    take_display_cols(field.key, key_w).as_ref(),
                    self.system.style(Role::TextMuted),
                );
                x = x.saturating_add(u16::try_from(key_w).unwrap_or(8));
                paint_line(
                    buffer,
                    x,
                    area.y,
                    u16::try_from(display_cols(self.separator)).unwrap_or(2),
                    self.separator,
                    self.system.style(Role::Border),
                );
                x = x.saturating_add(u16::try_from(display_cols(self.separator)).unwrap_or(2));
                if show_type {
                    let t = field.value_type.unwrap_or("");
                    paint_line(
                        buffer,
                        x,
                        area.y,
                        TYPE_W,
                        take_display_cols(t, usize::from(TYPE_W)).as_ref(),
                        self.system.style(Role::TextMuted),
                    );
                    x = x.saturating_add(TYPE_W + 1);
                }
                if show_source {
                    let s = field.source.unwrap_or("");
                    paint_line(
                        buffer,
                        x,
                        area.y,
                        SOURCE_W,
                        take_display_cols(s, usize::from(SOURCE_W)).as_ref(),
                        self.system.style(Role::TextMuted),
                    );
                    x = x.saturating_add(SOURCE_W + 1);
                }
                let remain = area.right().saturating_sub(x);
                if compare {
                    let half = remain / 2;
                    let body = if let Some(ann) = field.annotation {
                        format!("{value}  {ann}")
                    } else {
                        value.clone()
                    };
                    paint_line(
                        buffer,
                        x,
                        area.y,
                        half.saturating_sub(1),
                        take_display_cols(&body, usize::from(half.saturating_sub(1))).as_ref(),
                        value_style,
                    );
                    x = x.saturating_add(half);
                    let other = field.compare.unwrap_or("—");
                    let changed = field.compare.is_some_and(|c| c != field.value);
                    paint_line(
                        buffer,
                        x,
                        area.y,
                        area.right().saturating_sub(x),
                        take_display_cols(other, usize::from(area.right().saturating_sub(x)))
                            .as_ref(),
                        if changed {
                            self.system.style(Role::Warning)
                        } else {
                            self.system.style(Role::TextMuted)
                        },
                    );
                } else {
                    let body = if let Some(ann) = field.annotation {
                        format!("{value}  {ann}")
                    } else {
                        value
                    };
                    // copy affordance when selected + copyable
                    let mut text = take_display_cols(&body, usize::from(remain.saturating_sub(3)))
                        .into_owned();
                    if selected && field.copyable {
                        if state.copied.as_ref() == Some(&field.id) {
                            text.push_str(" ✓");
                        } else {
                            text.push_str(" ⧉");
                        }
                    }
                    paint_line(buffer, x, area.y, remain, &text, value_style);
                }
            }
        }
        paint_kv_row_chrome(&chrome, sub, buffer, area);
    }
}

/// Absolute display row `idx` as `(field, sub_row)`, from field heights.
fn walk_field_rows(heights: &[u16], idx: usize) -> Option<(usize, u16)> {
    let mut seen = 0usize;
    for (vi, h) in heights.iter().enumerate() {
        let h = usize::from(*h);
        if idx < seen.saturating_add(h) {
            return Some((vi, u16::try_from(idx - seen).unwrap_or(u16::MAX)));
        }
        seen = seen.saturating_add(h);
    }
    None
}

fn paint_kv_row_chrome(
    chrome: &crate::widgets::row_chrome::RowChrome,
    continuation: u16,
    buffer: &mut Buffer,
    area: Rect,
) {
    chrome.paint_wash(buffer, area);
    if continuation == 0 {
        chrome.paint_gutter(buffer, area);
    }
}

fn st_role(status: KvStatus) -> Role {
    match status {
        KvStatus::Neutral => Role::Text,
        KvStatus::Success => Role::Success,
        KvStatus::Warning => Role::Warning,
        KvStatus::Danger => Role::Danger,
        KvStatus::Info => Role::TextSecondary,
    }
}

fn paint_line(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    style: ratatui_core::style::Style,
) {
    if width == 0 {
        return;
    }
    let clipped = take_display_cols(text, usize::from(width));
    buffer.set_stringn(x, y, &clipped, usize::from(width), style);
}

impl<'a, Id: Clone + PartialEq + Ord> StatefulWidget for KeyValueTable<'a, Id> {
    type State = KeyValueTableState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        KeyValueTable::paint(&self, area, buffer, state);
    }
}

impl<'a, Id: Clone + PartialEq + Ord> StatefulWidget for &KeyValueTable<'a, Id> {
    type State = KeyValueTableState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        KeyValueTable::paint(self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests::click;

    #[test]
    fn separator_comes_from_the_shared_key_value_token() {
        let system = crate::style::DesignSystem::junie();
        let fields = sample();
        assert_eq!(
            KeyValueTable::new(&fields, &system).separator,
            system.kv_separator().text()
        );
        let colon = system.with_kv_separator(crate::style::KvSeparator::Colon);
        assert_eq!(KeyValueTable::new(&fields, &colon).separator, " : ");
    }

    fn sample() -> Vec<KvtField<'static, &'static str>> {
        vec![
            KvtField::group("g_req", "Request"),
            KvtField::pair("method", "method", "GET")
                .value_type("string")
                .source("line")
                .copyable()
                .depth(1),
            KvtField::pair("auth", "authorization", "Bearer secret-token")
                .value_type("secret")
                .source("header")
                .secret()
                .copyable()
                .depth(1),
            KvtField::pair("ct", "content-type", "application/json")
                .value_type("mime")
                .source("header")
                .editable()
                .depth(1),
            KvtField::pair("len", "content-length", "-1")
                .value_type("int")
                .validation(KvtValidation::Error)
                .validation_message("must be >= 0")
                .depth(1),
        ]
    }

    #[test]
    fn navigation_and_copy() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("method");
        let out = table.handle_key(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(out, KeyValueTableOutcome::Selected("auth")));
        let out = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            KeyValueTableOutcome::Copy {
                id: "auth",
                text
            } if text.contains("Bearer")
        ));
    }

    #[test]
    fn secret_reveal_toggle() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("auth");
        assert!(!state.is_revealed(&"auth"));
        let out = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );
        assert!(matches!(out, KeyValueTableOutcome::SecretRevealed("auth")));
        assert!(state.is_revealed(&"auth"));
        let painted = table.display_value(&fields[2], &state);
        assert!(painted.contains("Bearer"));
        state.toggle_reveal("auth");
        let redacted = table.display_value(&fields[2], &state);
        assert!(!redacted.contains("Bearer"));
    }

    #[test]
    fn edit_commit() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("ct");
        let out = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        );
        assert!(matches!(out, KeyValueTableOutcome::EditStarted("ct")));
        let _ = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        );
        let out = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            KeyValueTableOutcome::EditCommitted { id: "ct", text } if text.ends_with('!')
        ));
    }

    #[test]
    fn compare_mode_toggle() {
        let system = DesignSystem::default();
        let fields = [KvtField::pair("a", "host", "a.example")
            .compare("b.example")
            .copyable()];
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("a");
        let out = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            KeyValueTableOutcome::ModeChanged(KvtMode::Compare)
        ));
    }

    #[test]
    fn stacked_under_narrow() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new();
        state.layout = KvLayout::Auto;
        let layout = table.resolved_layout(30, &state);
        assert_eq!(layout, KvLayout::Stacked);
        let layout_w = table.resolved_layout(80, &state);
        assert_eq!(layout_w, KvLayout::Columns);
    }

    #[test]
    fn paint_http_headers() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("method");
        let area = Rect::new(0, 0, 72, 12);
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        assert!(!state.regions.is_empty());
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("method") || text.contains("Request"),
            "{text}"
        );
        assert!(
            text.contains("••••")
                || text.contains("****")
                || text.contains("auth")
                || text.contains("authorization"),
            "{text}"
        );
    }

    #[test]
    fn filter_keys() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new();
        let _ = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        );
        let _ = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        let _ = table.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
        );
        let view = table.filtered(&state);
        let keys: Vec<_> = view.iter().map(|f| f.key).collect();
        assert!(
            keys.iter()
                .any(|k| k.contains("authorization") || *k == "Request")
        );
    }

    #[test]
    fn mouse_select() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new();
        let area = Rect::new(0, 0, 60, 10);
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        let (rx, ry) = state
            .regions
            .first()
            .map(|r| (r.area.x, r.area.y))
            .expect("region");
        let out = table.handle_mouse(&mut state, click(rx, ry));
        assert!(matches!(out, KeyValueTableOutcome::Selected(_)));
    }

    #[test]
    fn validation_error_paints() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let mut state = KeyValueTableState::new().with_cursor("len");
        let area = Rect::new(0, 0, 64, 10);
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("must be") || text.contains("content-length"),
            "{text}"
        );
    }

    #[test]
    fn fuzz_measure_heights() {
        let system = DesignSystem::default();
        let fields = sample();
        let table = KeyValueTable::new(&fields, &system);
        let state = KeyValueTableState::new();
        for w in [0u16, 20, 36, 48, 72, 100] {
            for f in &fields {
                let _ = table.measure_field_h(
                    f,
                    w,
                    if w < 40 {
                        KvLayout::Stacked
                    } else {
                        KvLayout::Columns
                    },
                    &state,
                    w >= 56,
                    w >= 68,
                    false,
                );
            }
        }
    }
}
