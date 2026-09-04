// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ObjectInspector** — expandable typed inspector for structured data.
//!
//! **Mission.** JSON, YAML, TOML, structured logs, Rust debug trees, and
//! arbitrary application objects: scalars/objects/arrays, stable paths, type
//! labels, lazy expansion, search, copy path/value, edit hooks, compare/diff,
//! virtualized visible windows, depth limits, compact inline preview, fullscreen
//! promotion, escaped control characters, and secret redaction.
//!
//! **Ownership.** Host owns the source document and projects a **flattened
//! visible** node list (respecting expansion). State owns cursor, expansion set
//! by path (for preserve-across-update), search, scroll, and outcomes.
//!
//! Research: browser devtools, jq/fx viewers, Textual trees, DB JSON inspectors.
//! Leaves for pure metadata panels: [`super::KeyValueTable`]. Flat hierarchy:
//! [`super::Tree`].
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Glyph, ListRowVisualState, MASK_CELLS, Role},
    text::{contains_lower_all, display_cols, take_display_cols},
    widgets::{data_view::LoadState, scroll_area::ScrollAreaState, tiered_row::TieredRow},
};

const GUTTER: u16 = 2;
const INDENT: u16 = 2;
const DEFAULT_MAX_DEPTH: u8 = 32;

/// Scalar / container type for type-aware formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InspectKind {
    /// Missing / JSON null.
    Null,
    /// Boolean.
    Bool,
    /// Number (int or float display).
    Number,
    /// UTF-8 string.
    String,
    /// Opaque / binary blob.
    Binary,
    /// Map / object / table.
    Object,
    /// Array / list / sequence.
    Array,
    /// Custom domain type label (display via [`InspectorField::type_label`]).
    #[default]
    Unknown,
}

impl InspectKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Number => "number",
            Self::String => "string",
            Self::Binary => "binary",
            Self::Object => "object",
            Self::Array => "array",
            Self::Unknown => "unknown",
        }
    }
}

/// Load / lazy state for a node body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InspectNodeStatus {
    /// Ready for interaction.
    #[default]
    Ready,
    /// Children not yet loaded.
    Lazy,
    /// Fetch in flight.
    Loading,
    /// Load failed.
    Error,
}

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InspectMode {
    /// Single value column.
    #[default]
    View,
    /// Show compare-side when present.
    Compare,
}

impl InspectMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Compare => "compare",
        }
    }
}

/// Compact vs fullscreen chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InspectPresentation {
    /// Inline side panel (default).
    #[default]
    Compact,
    /// Fullscreen / dedicated pane affordances.
    Fullscreen,
}

/// One flattened projected inspector node (host projects visible expanded tree).
///
/// Prefer builders over struct literals — new fields are additive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorField<'a> {
    /// Display key (property name or array index).
    pub key: &'a str,
    /// Formatted value / preview (already type-aware from host).
    pub value: &'a str,
    /// Nesting depth (0 = root).
    pub depth: u8,
    /// Stable path for expansion preserve (`spec.containers[0].image`).
    pub path: &'a str,
    /// Type classification.
    pub kind: InspectKind,
    /// Optional custom type label (overrides kind glyph text when set).
    pub type_label: Option<&'a str>,
    /// Branch / expandable.
    pub branch: bool,
    /// Currently expanded (host projection; should match state expansion set).
    pub expanded: bool,
    /// Child load status.
    pub status: InspectNodeStatus,
    /// Secret — redacted until revealed.
    pub secret: bool,
    /// Optional child count for containers.
    pub child_count: Option<u32>,
    /// Compare-side value for diff mode.
    pub compare: Option<&'a str>,
    /// Editable leaf.
    pub editable: bool,
}

impl<'a> InspectorField<'a> {
    /// Simple leaf with path = key.
    #[must_use]
    pub const fn new(key: &'a str, value: &'a str) -> Self {
        Self {
            key,
            value,
            depth: 0,
            path: key,
            kind: InspectKind::Unknown,
            type_label: None,
            branch: false,
            expanded: false,
            status: InspectNodeStatus::Ready,
            secret: false,
            child_count: None,
            compare: None,
            editable: false,
        }
    }

    /// Object/array branch.
    #[must_use]
    pub const fn container(key: &'a str, path: &'a str, kind: InspectKind) -> Self {
        Self {
            key,
            value: "",
            depth: 0,
            path,
            kind,
            type_label: None,
            branch: true,
            expanded: false,
            status: InspectNodeStatus::Ready,
            secret: false,
            child_count: None,
            compare: None,
            editable: false,
        }
    }

    /// Sets depth.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Sets stable path.
    #[must_use]
    pub const fn path(mut self, path: &'a str) -> Self {
        self.path = path;
        self
    }

    /// Sets kind.
    #[must_use]
    pub const fn kind(mut self, kind: InspectKind) -> Self {
        self.kind = kind;
        self
    }

    /// Custom type label.
    #[must_use]
    pub const fn type_label(mut self, label: &'a str) -> Self {
        self.type_label = Some(label);
        self
    }

    /// Expanded branch.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// Lazy unloaded children.
    #[must_use]
    pub const fn lazy(mut self) -> Self {
        self.status = InspectNodeStatus::Lazy;
        self.branch = true;
        self
    }

    /// Secret redaction.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Child count hint for containers.
    #[must_use]
    pub const fn child_count(mut self, n: u32) -> Self {
        self.child_count = Some(n);
        self
    }

    /// Compare-side value.
    #[must_use]
    pub const fn compare(mut self, other: &'a str) -> Self {
        self.compare = Some(other);
        self
    }

    /// Editable leaf.
    #[must_use]
    pub const fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Preview text for collapsed containers.
    #[must_use]
    pub fn container_preview(&self) -> String {
        match self.kind {
            InspectKind::Object => {
                if let Some(n) = self.child_count {
                    format!("{{…{n}}}")
                } else {
                    "{…}".into()
                }
            }
            InspectKind::Array => {
                if let Some(n) = self.child_count {
                    format!("[{n}]")
                } else {
                    "[…]".into()
                }
            }
            _ => self.value.to_string(),
        }
    }
}

/// Hit region for a painted node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectRegion {
    /// Projected index.
    pub index: usize,
    /// Stable path.
    pub path: String,
    /// Full row.
    pub area: Rect,
    /// Disclosure glyph when branch.
    pub disclosure: Option<Rect>,
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectInspectorOutcome {
    /// No change.
    Ignored,
    /// Cursor moved among projected fields.
    CursorMoved {
        /// Field index in current projection.
        index: usize,
        /// Stable path when known.
        path: String,
    },
    /// Activate a stable projected field.
    Activated {
        /// Stable path.
        path: String,
        /// Projected index.
        index: usize,
    },
    /// Expand / collapse / lazy load requested.
    ExpandToggled {
        /// Stable path.
        path: String,
        /// Index in projection.
        index: usize,
        /// Expanded after toggle (desired).
        expanded: bool,
    },
    /// Copy value text (unredacted when secret revealed).
    CopyValue {
        /// Path.
        path: String,
        /// Text.
        text: String,
    },
    /// Copy stable path.
    CopyPath {
        /// Path.
        path: String,
    },
    /// Inline edit started.
    EditStarted {
        /// Path.
        path: String,
        /// Index.
        index: usize,
    },
    /// Edit committed.
    EditCommitted {
        /// Path.
        path: String,
        /// Proposed text.
        text: String,
    },
    /// Edit cancelled.
    EditCancelled,
    /// Search query changed.
    SearchChanged(String),
    /// Compare mode toggled.
    ModeChanged(InspectMode),
    /// Fullscreen promotion requested.
    FullscreenRequested,
    /// Cancel / clear search.
    Cancelled,
    /// Secret revealed.
    SecretRevealed {
        /// Path.
        path: String,
    },
    /// Secret hidden.
    SecretHidden {
        /// Path.
        path: String,
    },
}

/// Inspector state — expansion by path, cursor, search, scroll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInspectorState {
    cursor: usize,
    /// Preferred cursor path (sticky across reproject).
    cursor_path: Option<String>,
    scroll: ScrollAreaState,
    /// Expanded node paths (host should honor when projecting).
    expanded: BTreeSet<String>,
    /// Revealed secret paths.
    revealed: BTreeSet<String>,
    /// Search query.
    search: Option<String>,
    /// Mode.
    pub mode: InspectMode,
    /// Presentation.
    pub presentation: InspectPresentation,
    /// Max expand depth (hard stop).
    pub max_depth: u8,
    /// Load chrome for root.
    pub load: LoadState,
    /// Inline edit session open.
    pub editing: bool,
    /// Draft text while editing a leaf value.
    pub edit_draft: String,
    /// Host grants input.
    accepts_input: bool,
    /// Colorless.
    pub colorless: bool,
    origin: (u16, u16),
    body_rows: u16,
    body_width: u16,
    /// Hit regions from last paint.
    pub regions: Vec<InspectRegion>,
    /// Virtual window: logical flattened universe size (optional).
    pub logical_len: u64,
    /// Virtual window start when host windows a large flat list.
    pub window_start: u64,
}

impl Default for ObjectInspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectInspectorState {
    /// Fresh inspector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor: 0,
            cursor_path: None,
            scroll: ScrollAreaState::default(),
            expanded: BTreeSet::new(),
            revealed: BTreeSet::new(),
            search: None,
            mode: InspectMode::View,
            presentation: InspectPresentation::Compact,
            max_depth: DEFAULT_MAX_DEPTH,
            load: LoadState::Ready { count: 0 },
            editing: false,
            edit_draft: String::new(),
            accepts_input: true,
            colorless: false,
            origin: (0, 0),
            body_rows: 0,
            body_width: 0,
            regions: Vec::new(),
            logical_len: 0,
            window_start: 0,
        }
    }

    /// Cursor field index in the current projection.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Programmatic cursor by index.
    pub fn set_cursor(&mut self, index: usize) {
        self.cursor = index;
    }

    /// Set cursor by path (resolved on next reconcile).
    pub fn set_cursor_path(&mut self, path: impl Into<String>) {
        self.cursor_path = Some(path.into());
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether path is expanded.
    #[must_use]
    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    /// Set expansion for path (host reprojects after).
    pub fn set_expanded(&mut self, path: impl Into<String>, expanded: bool) {
        let p = path.into();
        if expanded {
            self.expanded.insert(p);
        } else {
            self.expanded.remove(&p);
            // Drop nested expansions under path
            let prefix = format!("{p}.");
            let prefix_b = format!("{p}[");
            self.expanded
                .retain(|x| !x.starts_with(&prefix) && !x.starts_with(&prefix_b) && x != &p);
        }
    }

    /// Whether secret path is revealed.
    #[must_use]
    pub fn is_revealed(&self, path: &str) -> bool {
        self.revealed.contains(path)
    }

    /// Toggle secret reveal.
    pub fn toggle_reveal(&mut self, path: impl Into<String>) -> bool {
        let p = path.into();
        if !self.revealed.remove(&p) {
            self.revealed.insert(p);
            true
        } else {
            false
        }
    }
    /// Reconcile cursor after host reprojects `fields`.
    pub fn reconcile(&mut self, fields: &[InspectorField<'_>]) {
        if fields.is_empty() {
            self.cursor = 0;
            return;
        }
        if let Some(path) = self.cursor_path.as_ref() {
            if let Some(i) = fields.iter().position(|f| f.path == path) {
                self.cursor = i;
                self.ensure_cursor_visible(fields.len());
                return;
            }
        }
        self.cursor = self.cursor.min(fields.len() - 1);
        if let Some(f) = fields.get(self.cursor) {
            self.cursor_path = Some(f.path.to_string());
        }
        self.ensure_cursor_visible(fields.len());
    }

    fn clamp_cursor(&mut self, field_count: usize) {
        if field_count == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(field_count - 1);
        }
    }

    fn ensure_cursor_visible(&mut self, field_count: usize) {
        self.scroll.reveal_row(self.cursor);
        self.scroll
            .set_content_size(1, field_count.min(u16::MAX as usize) as u16);
        self.scroll.set_viewport(1, self.body_rows);
        self.scroll.clamp();
    }

    /// Keys over projected fields (preferred).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        fields: &[InspectorField<'_>],
    ) -> ObjectInspectorOutcome {
        if !self.accepts_input || key.is_release() {
            return ObjectInspectorOutcome::Ignored;
        }
        let is_press = key.is_press();
        let field_count = fields.len();

        if matches!(
            self.load,
            LoadState::Empty { .. } | LoadState::Error { .. } | LoadState::Loading { .. }
        ) {
            return ObjectInspectorOutcome::Ignored;
        }

        if self.editing {
            return self.handle_edit_key(key, fields);
        }

        // Search
        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.search.is_none() {
                self.search = Some(String::new());
            }
            return ObjectInspectorOutcome::SearchChanged(self.search.clone().unwrap_or_default());
        }
        if let Some(q) = self.search.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.search = None;
                    return ObjectInspectorOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.search = None;
                    }
                    return ObjectInspectorOutcome::SearchChanged(
                        self.search.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return ObjectInspectorOutcome::SearchChanged(q.clone());
                }
                _ => {}
            }
        }

        if field_count == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        self.clamp_cursor(field_count);

        // Mode / fullscreen / copy chords
        if is_press && matches!(key.code, KeyCode::Char('d' | 'D')) && key.modifiers.is_empty() {
            self.mode = match self.mode {
                InspectMode::View => InspectMode::Compare,
                InspectMode::Compare => InspectMode::View,
            };
            return ObjectInspectorOutcome::ModeChanged(self.mode);
        }
        if is_press
            && matches!(key.code, KeyCode::Char('f' | 'F'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.presentation = InspectPresentation::Fullscreen;
            return ObjectInspectorOutcome::FullscreenRequested;
        }
        if is_press && matches!(key.code, KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            return self.copy_value(fields);
        }
        if is_press && matches!(key.code, KeyCode::Char('y' | 'Y')) && key.modifiers.is_empty() {
            return self.copy_path(fields);
        }
        if is_press && matches!(key.code, KeyCode::Char('e' | 'E')) && key.modifiers.is_empty() {
            return self.start_edit(fields);
        }
        if is_press && matches!(key.code, KeyCode::Char('r' | 'R')) && key.modifiers.is_empty() {
            return self.toggle_secret(fields);
        }

        // Hierarchy: Left/Right expand
        if is_press
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l' | 'H' | 'L')
            )
            && key.modifiers.is_empty()
        {
            let expand = matches!(key.code, KeyCode::Right | KeyCode::Char('l' | 'L'));
            return self.hierarchy_step(fields, expand);
        }

        if let Some(intent) = crate::interaction::default_inspector_intent(key) {
            let out = self.handle_intent(intent, fields);
            if !matches!(out, ObjectInspectorOutcome::Ignored) {
                return out;
            }
        }
        ObjectInspectorOutcome::Ignored
    }

    /// Intent routing (fields for path-aware outcomes).
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        fields: &[InspectorField<'_>],
    ) -> ObjectInspectorOutcome {
        if !self.accepts_input {
            return ObjectInspectorOutcome::Ignored;
        }
        let field_count = fields.len();
        if field_count == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        self.clamp_cursor(field_count);
        let out = match intent {
            UiIntent::Move(NavigationMove::Next) => self.move_cursor(fields, 1),
            UiIntent::Move(NavigationMove::Previous) => self.move_cursor(fields, -1),
            UiIntent::Move(NavigationMove::First) => {
                if self.cursor == 0 {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = 0;
                self.sync_path(fields);
                ObjectInspectorOutcome::CursorMoved {
                    index: 0,
                    path: path_at(fields, 0),
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let last = field_count - 1;
                if self.cursor == last {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = last;
                self.sync_path(fields);
                ObjectInspectorOutcome::CursorMoved {
                    index: last,
                    path: path_at(fields, last),
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = self.body_rows.max(1) as isize;
                self.move_cursor(fields, step)
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = self.body_rows.max(1) as isize;
                self.move_cursor(fields, -step)
            }
            UiIntent::Expand => self.hierarchy_step(fields, true),
            UiIntent::Collapse => self.hierarchy_step(fields, false),
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                let f = &fields[self.cursor];
                if f.branch {
                    return self.hierarchy_step(fields, !f.expanded);
                }
                ObjectInspectorOutcome::Activated {
                    path: f.path.to_string(),
                    index: self.cursor,
                }
            }
            UiIntent::Cancel => {
                if self.search.is_some() {
                    self.search = None;
                    return ObjectInspectorOutcome::Cancelled;
                }
                ObjectInspectorOutcome::Cancelled
            }
            _ => ObjectInspectorOutcome::Ignored,
        };
        if matches!(out, ObjectInspectorOutcome::CursorMoved { .. }) {
            self.ensure_cursor_visible(field_count);
        }
        out
    }

    fn move_cursor(
        &mut self,
        fields: &[InspectorField<'_>],
        delta: isize,
    ) -> ObjectInspectorOutcome {
        let n = fields.len();
        if n == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        let next = if delta >= 0 {
            (self.cursor + delta as usize).min(n - 1)
        } else {
            self.cursor.saturating_sub((-delta) as usize)
        };
        if next == self.cursor {
            return ObjectInspectorOutcome::Ignored;
        }
        self.cursor = next;
        self.sync_path(fields);
        ObjectInspectorOutcome::CursorMoved {
            index: self.cursor,
            path: path_at(fields, self.cursor),
        }
    }

    fn hierarchy_step(
        &mut self,
        fields: &[InspectorField<'_>],
        expand: bool,
    ) -> ObjectInspectorOutcome {
        let f = &fields[self.cursor];
        if f.depth >= self.max_depth && expand {
            return ObjectInspectorOutcome::Ignored;
        }
        if !f.branch && !matches!(f.status, InspectNodeStatus::Lazy) {
            if !expand {
                // Collapse to parent by depth
                return self.select_parent(fields);
            }
            return ObjectInspectorOutcome::Ignored;
        }
        if expand {
            if matches!(f.status, InspectNodeStatus::Lazy) || !f.expanded {
                let path = f.path.to_string();
                let index = self.cursor;
                self.set_expanded(&path, true);
                return ObjectInspectorOutcome::ExpandToggled {
                    path,
                    index,
                    expanded: true,
                };
            }
            // Enter first child
            let depth = f.depth;
            if let Some((i, child)) = fields
                .iter()
                .enumerate()
                .skip(self.cursor + 1)
                .find(|(_, n)| n.depth > depth)
            {
                self.cursor = i;
                self.sync_path(fields);
                self.ensure_cursor_visible(fields.len());
                return ObjectInspectorOutcome::CursorMoved {
                    index: i,
                    path: child.path.to_string(),
                };
            }
            ObjectInspectorOutcome::Ignored
        } else {
            if f.expanded {
                let path = f.path.to_string();
                let index = self.cursor;
                self.set_expanded(&path, false);
                return ObjectInspectorOutcome::ExpandToggled {
                    path,
                    index,
                    expanded: false,
                };
            }
            self.select_parent(fields)
        }
    }

    fn select_parent(&mut self, fields: &[InspectorField<'_>]) -> ObjectInspectorOutcome {
        let depth = fields[self.cursor].depth;
        if depth == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        if let Some((i, p)) = fields[..self.cursor]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, n)| n.depth < depth)
        {
            self.cursor = i;
            self.sync_path(fields);
            self.ensure_cursor_visible(fields.len());
            return ObjectInspectorOutcome::CursorMoved {
                index: i,
                path: p.path.to_string(),
            };
        }
        ObjectInspectorOutcome::Ignored
    }

    fn sync_path(&mut self, fields: &[InspectorField<'_>]) {
        if let Some(f) = fields.get(self.cursor) {
            self.cursor_path = Some(f.path.to_string());
        }
    }

    fn copy_value(&self, fields: &[InspectorField<'_>]) -> ObjectInspectorOutcome {
        let Some(f) = fields.get(self.cursor) else {
            return ObjectInspectorOutcome::Ignored;
        };
        let text = if f.secret && !self.is_revealed(f.path) {
            // Still copy true value — host passes real secret in value
            f.value.to_string()
        } else {
            f.value.to_string()
        };
        ObjectInspectorOutcome::CopyValue {
            path: f.path.to_string(),
            text,
        }
    }

    fn copy_path(&self, fields: &[InspectorField<'_>]) -> ObjectInspectorOutcome {
        let Some(f) = fields.get(self.cursor) else {
            return ObjectInspectorOutcome::Ignored;
        };
        ObjectInspectorOutcome::CopyPath {
            path: f.path.to_string(),
        }
    }

    fn start_edit(&mut self, fields: &[InspectorField<'_>]) -> ObjectInspectorOutcome {
        let Some(f) = fields.get(self.cursor) else {
            return ObjectInspectorOutcome::Ignored;
        };
        if !f.editable || f.branch || f.secret {
            return ObjectInspectorOutcome::Ignored;
        }
        self.editing = true;
        self.edit_draft = f.value.to_string();
        ObjectInspectorOutcome::EditStarted {
            path: f.path.to_string(),
            index: self.cursor,
        }
    }

    fn toggle_secret(&mut self, fields: &[InspectorField<'_>]) -> ObjectInspectorOutcome {
        let Some(f) = fields.get(self.cursor) else {
            return ObjectInspectorOutcome::Ignored;
        };
        if !f.secret {
            return ObjectInspectorOutcome::Ignored;
        }
        let path = f.path.to_string();
        let on = self.toggle_reveal(&path);
        if on {
            ObjectInspectorOutcome::SecretRevealed { path }
        } else {
            ObjectInspectorOutcome::SecretHidden { path }
        }
    }

    fn handle_edit_key(
        &mut self,
        key: KeyEvent,
        fields: &[InspectorField<'_>],
    ) -> ObjectInspectorOutcome {
        if !key.is_press() {
            return ObjectInspectorOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                self.editing = false;
                self.edit_draft.clear();
                ObjectInspectorOutcome::EditCancelled
            }
            KeyCode::Enter => {
                let path = fields
                    .get(self.cursor)
                    .map(|f| f.path.to_string())
                    .unwrap_or_default();
                let text = std::mem::take(&mut self.edit_draft);
                self.editing = false;
                ObjectInspectorOutcome::EditCommitted { path, text }
            }
            KeyCode::Backspace => {
                self.edit_draft.pop();
                ObjectInspectorOutcome::Ignored
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_draft.push(c);
                ObjectInspectorOutcome::Ignored
            }
            _ => ObjectInspectorOutcome::Ignored,
        }
    }
}

fn path_at(fields: &[InspectorField<'_>], index: usize) -> String {
    fields
        .get(index)
        .map(|f| f.path.to_string())
        .unwrap_or_default()
}

/// Escape control characters for safe display.
#[must_use]
pub fn escape_inspect_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Filter projected nodes by query (key/value/path) keeping ancestor depths.
#[must_use]
pub fn filter_inspect_fields<'a>(
    fields: &'a [InspectorField<'a>],
    query: &str,
) -> Vec<&'a InspectorField<'a>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return fields.iter().collect();
    }
    let mut keep = vec![false; fields.len()];
    for (i, f) in fields.iter().enumerate() {
        if contains_lower_all(&[f.key, f.value, f.path], &q) {
            keep[i] = true;
            let mut depth = f.depth;
            let mut j = i;
            while depth > 0 && j > 0 {
                j -= 1;
                if fields[j].depth < depth {
                    keep[j] = true;
                    depth = fields[j].depth;
                }
            }
        }
    }
    fields
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, f)| f)
        .collect()
}

/// Object inspector widget.
#[derive(Debug, Clone)]
pub struct ObjectInspector<'a> {
    fields: &'a [InspectorField<'a>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
    presentation: InspectPresentation,
    show_types: bool,
}

impl<'a> ObjectInspector<'a> {
    /// Fields + design system.
    #[must_use]
    pub const fn new(fields: &'a [InspectorField<'a>], system: &'a DesignSystem) -> Self {
        Self {
            fields,
            system,
            focused: true,
            colorless: false,
            presentation: InspectPresentation::Compact,
            show_types: true,
        }
    }

    /// Scene surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// Reduced-color paint.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Display value with redaction + escape.
    #[must_use]
    pub fn display_value(
        &self,
        field: &InspectorField<'a>,
        state: &ObjectInspectorState,
    ) -> String {
        if field.secret && !state.is_revealed(field.path) {
            return Glyph::Mask.resolve().text.repeat(MASK_CELLS);
        }
        if state.editing && state.cursor_path.as_deref() == Some(field.path) {
            return escape_inspect_value(&state.edit_draft);
        }
        if field.branch && !field.expanded {
            return field.container_preview();
        }
        if matches!(field.status, InspectNodeStatus::Loading) {
            return "…".into();
        }
        if matches!(field.status, InspectNodeStatus::Error) {
            return if field.value.is_empty() {
                "error".into()
            } else {
                escape_inspect_value(field.value)
            };
        }
        if matches!(field.status, InspectNodeStatus::Lazy) && field.value.is_empty() {
            return "(lazy)".into();
        }
        escape_inspect_value(field.value)
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ObjectInspectorState) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            return;
        }
        let colorless = self.colorless || state.colorless || self.system.mono();
        let footer = 1u16;
        let header = u16::from(
            matches!(self.presentation, InspectPresentation::Fullscreen)
                || matches!(state.presentation, InspectPresentation::Fullscreen)
                || state.search.is_some(),
        );
        let body_h = area.height.saturating_sub(footer + header).max(1);
        state.origin = (area.x, area.y.saturating_add(header));
        state.body_rows = body_h;
        state.body_width = area.width;

        let mut y = area.y;
        if header > 0 && y < area.bottom() {
            let title = if let Some(q) = &state.search {
                format!("/ {q}")
            } else if matches!(self.presentation, InspectPresentation::Fullscreen)
                || matches!(state.presentation, InspectPresentation::Fullscreen)
            {
                "Object inspector".into()
            } else {
                String::new()
            };
            if !title.is_empty() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&title, usize::from(area.width)).as_ref(),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
            }
            y = y.saturating_add(1);
        }

        if let Some(chrome) =
            super::data_view::data_load_chrome(&state.load, self.system, colorless, "Empty object")
        {
            let line = format!("{}{}", chrome.prefix, chrome.message);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(chrome.role),
            );
            state.origin = (area.x, y);
            state.body_rows = 0;
            state.body_width = area.width;
            return;
        }

        // Apply search filter view (indices into self.fields)
        let view: Vec<&InspectorField<'a>> = if let Some(q) = state.search.as_ref() {
            filter_inspect_fields(self.fields, q)
        } else {
            self.fields.iter().collect()
        };
        // For paint we need owned projection slice — use view
        let field_count = view.len();
        state.clamp_cursor(field_count);
        // Reconcile path against view
        if let Some(path) = state.cursor_path.as_ref() {
            if let Some(i) = view.iter().position(|f| f.path == path) {
                state.cursor = i;
            }
        }
        state
            .scroll
            .set_content_size(1, field_count.min(u16::MAX as usize) as u16);
        state.scroll.set_viewport(1, body_h);
        state.ensure_cursor_visible(field_count.max(1));

        let surface = self.focused && state.accepts_input;
        let narrow = area.width < 28;
        let tiny = area.width < 16;
        let compare = matches!(state.mode, InspectMode::Compare);

        if view.is_empty() {
            let glyph = "∅ ";
            let line = if tiny {
                format!("{glyph}empty")
            } else if state.search.is_some() {
                format!("{glyph}(no matches)")
            } else {
                format!("{glyph}(empty object)")
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            self.paint_footer(area, buffer, state);
            return;
        }

        let start = state.scroll.offset_y() as usize;
        let body_bottom = area.bottom().saturating_sub(footer);
        for (i, field) in view.iter().enumerate().skip(start) {
            if y >= body_bottom {
                break;
            }
            let cursor = i == state.cursor;
            let gutter = if cursor && surface {
                self.system.glyphs.selection_gutter()
            } else if cursor {
                "·"
            } else {
                " "
            };
            let gstyle = if cursor {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(area.x, y, gutter, 1, gstyle);
            buffer.set_stringn(area.x.saturating_add(1), y, " ", 1, gstyle);

            let mut x = area.x.saturating_add(GUTTER);
            let max_indent = area.width.saturating_sub(GUTTER + 8);
            let indent = u16::from(field.depth)
                .saturating_mul(INDENT)
                .min(max_indent);
            x = x.saturating_add(indent);

            let mut disclosure = None;
            if field.branch {
                let glyph = if field.expanded {
                    self.system.glyphs.disclosure_open()
                } else {
                    self.system.glyphs.disclosure_closed()
                };
                if x < area.right() {
                    buffer.set_stringn(x, y, glyph, 1, self.system.style(Role::TextMuted));
                    disclosure = Some(Rect::new(x, y, 1, 1));
                    x = x.saturating_add(2);
                }
            } else {
                x = x.saturating_add(2); // align with branch rows
            }

            let value = self.display_value(field, state);
            let style = if colorless {
                if cursor && surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if field.depth > 0 {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::Text)
            };
            let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected: cursor,
                    focused: surface,
                    enabled: true,
                    ..Default::default()
                },
            );
            let style = chrome.label_style(style);

            // The key names the fact; the value is the fact. They are not the
            // same tier, and the type annotation is quieter than both
            // (plans/012 Step 3).
            let tone = |role: Role| (!colorless).then(|| self.system.style(role));
            let key_tone = tone(Role::TextMuted);
            let meta_tone = tone(Role::TextFaint);
            let mut tiers = TieredRow::with_separator("");
            if tiny {
                if cursor {
                    tiers.push_joined(&value, None);
                } else {
                    tiers.push_joined(field.key, key_tone);
                }
            } else if narrow {
                tiers.push_joined(field.key, key_tone);
                tiers.push_joined("=", meta_tone);
                tiers.push_joined(&value, None);
            } else {
                tiers.push_joined(field.key, key_tone);
                tiers.push_joined(":", meta_tone);
                tiers.push_joined(" ", None);
                tiers.push_joined(&value, None);
                if self.show_types && area.width >= 48 {
                    let tl = field.type_label.unwrap_or_else(|| field.kind.id());
                    let type_part = format!("  <{tl}>");
                    if display_cols(tiers.text()) + display_cols(&type_part)
                        < usize::from(area.right().saturating_sub(x))
                    {
                        tiers.push_joined(&type_part, meta_tone);
                    }
                }
                if compare && let Some(c) = field.compare {
                    let esc = escape_inspect_value(c);
                    let mark = " ↔ ";
                    tiers.push_joined(mark, meta_tone);
                    tiers.push_joined(&esc, key_tone);
                }
            }
            let line = tiers.text().to_string();
            let remain = area.right().saturating_sub(x);
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&line, usize::from(remain)).as_ref(),
                usize::from(remain),
                style,
            );
            tiers.paint_tiers(buffer, Rect::new(x, y, remain, 1), 0);
            chrome.paint(buffer, Rect::new(area.x, y, area.width, 1));

            state.regions.push(InspectRegion {
                index: i,
                path: field.path.to_string(),
                area: Rect::new(area.x, y, area.width, 1),
                disclosure,
            });
            y = y.saturating_add(1);
        }

        self.paint_footer(area, buffer, state);
    }

    fn paint_footer(&self, area: Rect, buffer: &mut Buffer, state: &ObjectInspectorState) {
        let y = area.bottom().saturating_sub(1);
        if y < area.y {
            return;
        }
        let mut parts = Vec::new();
        parts.push(format!("mode:{}", state.mode.id()));
        if let Some(q) = &state.search {
            parts.push(format!("/{q}"));
        }
        if state.editing {
            parts.push(format!("edit:{}", state.edit_draft));
        }
        if matches!(state.presentation, InspectPresentation::Fullscreen) {
            parts.push("full".into());
        }
        parts.push("c value · y path · e edit · r secret · / find · C-f full".into());
        let line = parts.join(" · ");
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&line, usize::from(area.width)).as_ref(),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
    }
}

impl StatefulWidget for ObjectInspector<'_> {
    type State = ObjectInspectorState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        ObjectInspector::paint(&self, area, buffer, state);
    }
}

impl StatefulWidget for &ObjectInspector<'_> {
    type State = ObjectInspectorState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        ObjectInspector::paint(self, area, buffer, state);
    }
}

// ── Compatibility: handle_key(field_count) for old call sites ───────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> Vec<InspectorField<'static>> {
        vec![
            InspectorField::container("spec", "spec", InspectKind::Object)
                .child_count(2)
                .expanded(),
            InspectorField::container("containers", "spec.containers", InspectKind::Array)
                .depth(1)
                .child_count(1)
                .expanded(),
            InspectorField::new("image", "ghcr.io/app:1.2")
                .path("spec.containers[0].image")
                .depth(2)
                .kind(InspectKind::String),
            InspectorField::new("token", "secret-value")
                .path("spec.token")
                .depth(1)
                .kind(InspectKind::String)
                .secret(),
            InspectorField::new("port", "8080")
                .path("spec.port")
                .depth(1)
                .kind(InspectKind::Number)
                .editable()
                .compare("9090"),
        ]
    }

    #[test]
    fn expand_toggle_by_path() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        state.set_cursor(0);
        let out = state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &fields);
        assert!(matches!(
            out,
            ObjectInspectorOutcome::ExpandToggled {
                path,
                expanded: false,
                ..
            } if path == "spec"
        ));
        assert!(!state.is_expanded("spec"));
    }

    #[test]
    fn right_on_expanded_enters_child() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        state.set_expanded("spec", true);
        state.set_cursor(0);
        let out = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &fields);
        assert!(matches!(
            out,
            ObjectInspectorOutcome::CursorMoved { index: 1, .. }
        ));
    }

    #[test]
    fn expansion_preserved_across_reconcile() {
        let mut state = ObjectInspectorState::new();
        state.set_expanded("spec", true);
        state.set_expanded("spec.containers", true);
        state.set_cursor_path("spec.containers[0].image");
        let fields = sample_tree();
        state.reconcile(&fields);
        assert_eq!(state.cursor(), 2);
        assert!(state.is_expanded("spec"));
    }

    #[test]
    fn secret_redaction_and_reveal() {
        let system = DesignSystem::default();
        let fields = sample_tree();
        let table = ObjectInspector::new(&fields, &system);
        let mut state = ObjectInspectorState::new();
        state.set_cursor(3);
        state.cursor_path = Some("spec.token".into());
        let redacted = table.display_value(&fields[3], &state);
        assert!(!redacted.contains("secret"));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &fields,
        );
        assert!(matches!(out, ObjectInspectorOutcome::SecretRevealed { .. }));
        let shown = table.display_value(&fields[3], &state);
        assert!(shown.contains("secret"));
    }

    #[test]
    fn copy_path_and_value() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        state.set_cursor(2);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
            &fields,
        );
        assert!(matches!(
            out,
            ObjectInspectorOutcome::CopyPath { path } if path == "spec.containers[0].image"
        ));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &fields,
        );
        assert!(matches!(
            out,
            ObjectInspectorOutcome::CopyValue { text, .. } if text.contains("ghcr.io")
        ));
    }

    #[test]
    fn escape_control_chars() {
        assert_eq!(escape_inspect_value("a\nb\t"), "a\\nb\\t");
        assert!(escape_inspect_value("\u{1}").contains("\\u{"));
    }

    #[test]
    fn filter_keeps_ancestors() {
        let fields = sample_tree();
        let kept = filter_inspect_fields(&fields, "image");
        let paths: Vec<_> = kept.iter().map(|f| f.path).collect();
        assert!(paths.contains(&"spec"));
        assert!(paths.contains(&"spec.containers"));
        assert!(paths.contains(&"spec.containers[0].image"));
    }

    #[test]
    fn edit_commit() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        state.set_cursor(4);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &fields,
        );
        assert!(matches!(out, ObjectInspectorOutcome::EditStarted { .. }));
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE),
            &fields,
        );
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &fields);
        assert!(matches!(
            out,
            ObjectInspectorOutcome::EditCommitted { text, .. } if text.ends_with('9')
        ));
    }

    #[test]
    fn paint_nested_json_like() {
        let system = DesignSystem::default();
        let fields = sample_tree();
        let table = ObjectInspector::new(&fields, &system).focused(true);
        let mut state = ObjectInspectorState::new();
        state.set_cursor(2);
        let area = Rect::new(0, 0, 64, 12);
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        assert!(!state.regions.is_empty());
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("image") || text.contains("spec"), "{text}");
    }

    #[test]
    fn root_load_states_paint_before_the_empty_object_fallback() {
        let system = DesignSystem::junie().no_color();
        let fields: [InspectorField<'_>; 0] = [];
        let render = |load| {
            let mut state = ObjectInspectorState::new();
            state.load = load;
            let area = Rect::new(0, 0, 32, 4);
            let mut buffer = Buffer::empty(area);
            ObjectInspector::new(&fields, &system).render(area, &mut buffer, &mut state);
            buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
        };

        assert!(render(LoadState::Loading { message: None }).contains("… Loading…"));
        assert!(render(LoadState::Empty { message: None }).contains("∅ Empty object"));
        assert!(
            render(LoadState::Error {
                message: "failed".into(),
                retryable: false,
            })
            .contains("✗ failed")
        );
    }

    #[test]
    fn max_depth_blocks_expand() {
        let fields = [
            InspectorField::container("deep", "a.b.c", InspectKind::Object)
                .depth(10)
                .lazy(),
        ];
        let mut state = ObjectInspectorState::new();
        state.max_depth = 5;
        state.set_cursor(0);
        let out = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &fields);
        assert!(matches!(out, ObjectInspectorOutcome::Ignored));
    }

    #[test]
    fn compare_mode_toggle() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            &fields,
        );
        assert!(matches!(
            out,
            ObjectInspectorOutcome::ModeChanged(InspectMode::Compare)
        ));
    }

    #[test]
    fn fullscreen_chord() {
        let fields = sample_tree();
        let mut state = ObjectInspectorState::new();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            &fields,
        );
        assert!(matches!(out, ObjectInspectorOutcome::FullscreenRequested));
    }

    #[test]
    fn fuzz_escape_and_depth() {
        for s in ["", "ok", "a\0b", "日本語", "line\nbreak"] {
            let e = escape_inspect_value(s);
            assert!(!e.contains('\n') || s.is_empty());
        }
        for d in 0u8..40 {
            let f = InspectorField::new("k", "v").depth(d);
            assert_eq!(f.depth, d);
        }
    }
}
