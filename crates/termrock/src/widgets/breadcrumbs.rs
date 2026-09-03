// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Location context and ancestor navigation (files, routes, schemas, paths).
//!
//! **Mission.** Surfaces need a compact trail of ancestors with collapse,
//! optional path edit, overflow menu, and current-item semantics — without
//! exploding global Tab stops (one focusable control; Left/Right inside).
//!
//! **vs [`FilePicker`](super::FilePicker) crumbs.** FilePicker embeds path
//! crumbs; prefer this widget for shared chrome (schemas, master-detail, docs).
//! FilePicker may project into [`BreadcrumbItem`] via host adapters.
//! **vs [`NavigationList`](super::NavigationList).** Side rail hierarchy; crumbs
//! are a single-line ancestor trail.
//!
//! **Contraction.** Always keep **root/first** and **current/last**; middle
//! collapses to `…` / overflow.
//!
//! Research: desktop breadcrumbs, terminal file managers, shadcn Breadcrumb.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Width under which middle segments collapse (when len > 3).
pub const BREADCRUMBS_COLLAPSE_MAX_WIDTH: u16 = 40;

// ── Model ───────────────────────────────────────────────────────────────────

/// Status on a crumb (non-color mark when present).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BreadcrumbStatus {
    /// Ordinary.
    #[default]
    None,
    /// Loading / pending.
    Loading,
    /// Warning.
    Warning,
    /// Error / broken path.
    Error,
}

impl BreadcrumbStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Loading => "loading",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Mark.
    #[must_use]
    pub const fn mark(self, ascii: bool) -> Option<&'static str> {
        match (self, ascii) {
            (Self::None, _) => None,
            (Self::Loading, true) => Some("..."),
            (Self::Loading, false) => Some("…"),
            (Self::Warning, true) => Some("!"),
            (Self::Warning, false) => Some("⚠"),
            (Self::Error, true) => Some("x"),
            (Self::Error, false) => Some("✗"),
        }
    }
}

/// One crumb (root → … → current).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem<Id> {
    /// Stable id (path segment, route, schema node).
    pub id: Id,
    /// Display label.
    pub label: String,
    /// Optional status.
    pub status: BreadcrumbStatus,
    /// Enabled for navigation (current may be disabled).
    pub enabled: bool,
    /// Explicit current flag; if none set, last item is current.
    pub current: bool,
}

impl<Id> BreadcrumbItem<Id> {
    /// Crumb.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            status: BreadcrumbStatus::None,
            enabled: true,
            current: false,
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: BreadcrumbStatus) -> Self {
        self.status = s;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Mark as current (leaf).
    #[must_use]
    pub const fn current(mut self, on: bool) -> Self {
        self.current = on;
        self
    }
}

/// Interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BreadcrumbsMode {
    /// Navigate among ancestors (default).
    #[default]
    Trail,
    /// Editable path string (host interprets on commit).
    Editable,
}

impl BreadcrumbsMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Trail => "trail",
            Self::Editable => "editable",
        }
    }
}

/// Paint presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BreadcrumbsPresentation {
    /// All segments visible.
    #[default]
    Full,
    /// First + ellipsis + current (middle hidden).
    Collapsed,
}

impl BreadcrumbsPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Collapsed => "collapsed",
        }
    }
}

/// Separator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BreadcrumbSeparator {
    /// ` / ` (default, path-like).
    #[default]
    Slash,
    /// ` › ` / ` > `.
    Chevron,
    /// ` → ` / ` -> `.
    Arrow,
}

impl BreadcrumbSeparator {
    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        match (self, ascii) {
            (Self::Slash, _) => " / ",
            (Self::Chevron, true) => " > ",
            (Self::Chevron, false) => " › ",
            (Self::Arrow, true) => " -> ",
            (Self::Arrow, false) => " → ",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Breadcrumbs outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BreadcrumbsOutcome<Id> {
    /// No change.
    Ignored,
    /// Focus index / chrome.
    Changed,
    /// Navigate to ancestor (or current if host allows).
    Navigate(Id),
    /// Overflow menu for collapsed middle segments.
    OpenOverflow {
        /// Hidden segment ids (middle).
        ids: Vec<Id>,
    },
    /// Overflow closed.
    OverflowClosed,
    /// Entered editable path mode.
    EditStarted {
        /// Initial draft (joined labels or host path).
        draft: String,
    },
    /// Path edit committed.
    EditCommitted {
        /// Draft text.
        path: String,
    },
    /// Path edit cancelled.
    EditCancelled,
    /// Esc while not editing / blur.
    Blurred,
}

// ── Painted slot (internal) ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum PaintSlot<Id> {
    /// Real item at source index.
    Item { index: usize, id: Id },
    /// Ellipsis standing for middle range.
    Ellipsis { hidden: Vec<Id> },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state — **one** host Tab stop; Left/Right inside the trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbsState {
    /// Focus index into **source** items (or special ellipsis handling).
    focus_index: usize,
    /// When collapsed, focus may sit on ellipsis.
    focus_on_ellipsis: bool,
    mode: BreadcrumbsMode,
    draft: String,
    overflow_open: bool,
    focused: bool,
    enabled: bool,
    /// Prefer editable mode available.
    editable: bool,
    separator: BreadcrumbSeparator,
    presentation: BreadcrumbsPresentation,
    root: Rect,
}

impl Default for BreadcrumbsState {
    fn default() -> Self {
        Self::new()
    }
}

impl BreadcrumbsState {
    /// Default trail mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focus_index: 0,
            focus_on_ellipsis: false,
            mode: BreadcrumbsMode::Trail,
            draft: String::new(),
            overflow_open: false,
            focused: false,
            enabled: true,
            editable: false,
            separator: BreadcrumbSeparator::Slash,
            presentation: BreadcrumbsPresentation::Full,
            root: Rect::default(),
        }
    }

    /// Allow path edit mode.
    #[must_use]
    pub const fn with_editable(mut self, on: bool) -> Self {
        self.editable = on;
        self
    }

    /// Separator.
    #[must_use]
    pub const fn with_separator(mut self, sep: BreadcrumbSeparator) -> Self {
        self.separator = sep;
        self
    }

    /// Focus index into source items.
    #[must_use]
    pub const fn focus_index(&self) -> usize {
        self.focus_index
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> BreadcrumbsMode {
        self.mode
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> BreadcrumbsPresentation {
        self.presentation
    }

    /// Draft path when editing.
    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    /// Focus whole control (single Tab stop).
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on && matches!(self.mode, BreadcrumbsMode::Editable) {
            self.mode = BreadcrumbsMode::Trail;
            self.draft.clear();
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Clamp focus after items change; prefer current (last).
    pub fn reconcile_len(&mut self, len: usize) {
        if len == 0 {
            self.focus_index = 0;
            self.focus_on_ellipsis = false;
            return;
        }
        self.focus_index = self.focus_index.min(len - 1);
    }

    fn join_path<Id>(items: &[BreadcrumbItem<Id>], sep: &str) -> String {
        items
            .iter()
            .map(|i| i.label.as_str())
            .collect::<Vec<_>>()
            .join(sep.trim())
    }

    fn current_index<Id>(items: &[BreadcrumbItem<Id>]) -> usize {
        items
            .iter()
            .rposition(|i| i.current)
            .unwrap_or_else(|| items.len().saturating_sub(1))
    }

    /// Start editable mode with draft from labels.
    pub fn start_edit<Id>(&mut self, items: &[BreadcrumbItem<Id>]) -> BreadcrumbsOutcome<Id>
    where
        Id: Clone,
    {
        if !self.editable || !self.enabled {
            return BreadcrumbsOutcome::Ignored;
        }
        self.mode = BreadcrumbsMode::Editable;
        self.draft = Self::join_path(items, "/");
        self.focus_on_ellipsis = false;
        BreadcrumbsOutcome::EditStarted {
            draft: self.draft.clone(),
        }
    }

    /// Commit edit.
    pub fn commit_edit<Id: Clone>(&mut self) -> BreadcrumbsOutcome<Id> {
        if !matches!(self.mode, BreadcrumbsMode::Editable) {
            return BreadcrumbsOutcome::Ignored;
        }
        let path = self.draft.clone();
        self.mode = BreadcrumbsMode::Trail;
        self.draft.clear();
        BreadcrumbsOutcome::EditCommitted { path }
    }

    /// Cancel edit.
    pub fn cancel_edit<Id: Clone>(&mut self) -> BreadcrumbsOutcome<Id> {
        if !matches!(self.mode, BreadcrumbsMode::Editable) {
            return BreadcrumbsOutcome::Ignored;
        }
        self.mode = BreadcrumbsMode::Trail;
        self.draft.clear();
        BreadcrumbsOutcome::EditCancelled
    }

    /// Key adapter — single control focus.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        items: &[BreadcrumbItem<Id>],
    ) -> BreadcrumbsOutcome<Id> {
        if !self.enabled || key.is_release() {
            return BreadcrumbsOutcome::Ignored;
        }
        if !self.focused {
            return BreadcrumbsOutcome::Ignored;
        }
        if items.is_empty() {
            return BreadcrumbsOutcome::Ignored;
        }
        self.reconcile_len(items.len());
        let is_press = key.is_press();

        // Editable mode
        if matches!(self.mode, BreadcrumbsMode::Editable) {
            match key.code {
                KeyCode::Esc if is_press => return self.cancel_edit(),
                KeyCode::Enter if is_press => return self.commit_edit(),
                KeyCode::Backspace => {
                    self.draft.pop();
                    return BreadcrumbsOutcome::Changed;
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                        && !c.is_control() =>
                {
                    self.draft.push(c);
                    return BreadcrumbsOutcome::Changed;
                }
                _ => return BreadcrumbsOutcome::Ignored,
            }
        }

        // Overflow open: Esc closes
        if self.overflow_open {
            if key.code == KeyCode::Esc && is_press {
                self.overflow_open = false;
                return BreadcrumbsOutcome::OverflowClosed;
            }
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Start edit: F2-like via Ctrl+E or when editable + Enter on current
        if self.editable
            && is_press
            && ctrl
            && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))
        {
            return self.start_edit(items);
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                self.move_focus(-1, items)
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                self.move_focus(1, items)
            }
            KeyCode::Home => {
                self.focus_on_ellipsis = false;
                self.focus_index = 0;
                BreadcrumbsOutcome::Changed
            }
            KeyCode::End => {
                self.focus_on_ellipsis = false;
                self.focus_index = items.len().saturating_sub(1);
                BreadcrumbsOutcome::Changed
            }
            KeyCode::Enter | KeyCode::Char(' ') if is_press && key.modifiers.is_empty() => {
                self.activate(items)
            }
            KeyCode::Esc if is_press => {
                if self.overflow_open {
                    self.overflow_open = false;
                    BreadcrumbsOutcome::OverflowClosed
                } else {
                    BreadcrumbsOutcome::Blurred
                }
            }
            // `/` start path edit when editable
            KeyCode::Char('/') if is_press && self.editable && key.modifiers.is_empty() => {
                self.start_edit(items)
            }
            _ => BreadcrumbsOutcome::Ignored,
        }
    }

    fn move_focus<Id: Clone>(
        &mut self,
        delta: i32,
        items: &[BreadcrumbItem<Id>],
    ) -> BreadcrumbsOutcome<Id> {
        let collapsed =
            matches!(self.presentation, BreadcrumbsPresentation::Collapsed) && items.len() > 3;
        if collapsed {
            // slots: first | ellipsis | last  — 3 focus positions
            let slot = if self.focus_on_ellipsis {
                1i32
            } else if self.focus_index == 0 {
                0
            } else {
                2
            };
            let next = (slot + delta).clamp(0, 2);
            match next {
                0 => {
                    self.focus_on_ellipsis = false;
                    self.focus_index = 0;
                }
                1 => {
                    self.focus_on_ellipsis = true;
                }
                _ => {
                    self.focus_on_ellipsis = false;
                    self.focus_index = items.len() - 1;
                }
            }
            return BreadcrumbsOutcome::Changed;
        }
        self.focus_on_ellipsis = false;
        if delta < 0 {
            self.focus_index = self.focus_index.saturating_sub(1);
        } else {
            self.focus_index = (self.focus_index + 1).min(items.len().saturating_sub(1));
        }
        BreadcrumbsOutcome::Changed
    }

    fn activate<Id: Clone>(&mut self, items: &[BreadcrumbItem<Id>]) -> BreadcrumbsOutcome<Id> {
        if self.focus_on_ellipsis
            && matches!(self.presentation, BreadcrumbsPresentation::Collapsed)
            && items.len() > 3
        {
            let ids: Vec<Id> = items[1..items.len() - 1]
                .iter()
                .map(|i| i.id.clone())
                .collect();
            self.overflow_open = true;
            return BreadcrumbsOutcome::OpenOverflow { ids };
        }
        let idx = self.focus_index.min(items.len() - 1);
        let item = &items[idx];
        // Current item: optional edit when editable
        let is_current = item.current || idx == items.len() - 1;
        if is_current && self.editable {
            return self.start_edit(items);
        }
        if !item.enabled {
            return BreadcrumbsOutcome::Ignored;
        }
        BreadcrumbsOutcome::Navigate(item.id.clone())
    }

    /// Mouse — click segment or ellipsis.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        items: &[BreadcrumbItem<Id>],
        hits: &[(BreadcrumbHit<Id>, Rect)],
    ) -> BreadcrumbsOutcome<Id> {
        if !self.enabled {
            return BreadcrumbsOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return BreadcrumbsOutcome::Ignored;
        }
        self.focused = true;
        for (hit, rect) in hits {
            if !rect.contains(event.position) {
                continue;
            }
            match hit {
                BreadcrumbHit::Item { index, id } => {
                    self.focus_on_ellipsis = false;
                    self.focus_index = *index;
                    let is_current = items
                        .get(*index)
                        .is_some_and(|i| i.current || *index + 1 == items.len());
                    if is_current && self.editable {
                        return self.start_edit(items);
                    }
                    if items.get(*index).is_some_and(|i| !i.enabled) {
                        return BreadcrumbsOutcome::Changed;
                    }
                    return BreadcrumbsOutcome::Navigate(id.clone());
                }
                BreadcrumbHit::Ellipsis { hidden } => {
                    self.focus_on_ellipsis = true;
                    self.overflow_open = true;
                    return BreadcrumbsOutcome::OpenOverflow {
                        ids: hidden.clone(),
                    };
                }
            }
        }
        BreadcrumbsOutcome::Ignored
    }
}

/// Hit target for mouse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreadcrumbHit<Id> {
    /// Segment.
    Item {
        /// Source index.
        index: usize,
        /// Id.
        id: Id,
    },
    /// Collapsed middle.
    Ellipsis {
        /// Hidden ids.
        hidden: Vec<Id>,
    },
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Breadcrumb trail chrome.
#[derive(Debug, Clone, Copy)]
pub struct Breadcrumbs<'a, Id> {
    items: &'a [BreadcrumbItem<Id>],
    system: &'a DesignSystem,
    separator: BreadcrumbSeparator,
}

impl<'a, Id: Clone + PartialEq> Breadcrumbs<'a, Id> {
    /// Items root → leaf.
    #[must_use]
    pub const fn new(items: &'a [BreadcrumbItem<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system: system,
            separator: BreadcrumbSeparator::Slash,
        }
    }

    /// ASCII separators / ellipsis.
    #[must_use]
    /// Separator style.
    pub const fn separator(mut self, sep: BreadcrumbSeparator) -> Self {
        self.separator = sep;
        self
    }

    /// Paint; returns hit list for mouse routing.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut BreadcrumbsState,
    ) -> Vec<(BreadcrumbHit<Id>, Rect)> {
        state.root = area;
        let mut hits = Vec::new();
        if area.is_empty() || self.items.is_empty() {
            return hits;
        }

        // Editable path field
        if matches!(state.mode, BreadcrumbsMode::Editable) {
            let style = self
                .system
                .style(if state.focused {
                    Role::Focus
                } else {
                    Role::Text
                })
                .add_modifier(Modifier::BOLD);
            let line = format!(" {}", state.draft);
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            return hits;
        }

        let collapse = area.width < BREADCRUMBS_COLLAPSE_MAX_WIDTH && self.items.len() > 3;
        state.presentation = if collapse {
            BreadcrumbsPresentation::Collapsed
        } else {
            BreadcrumbsPresentation::Full
        };

        let sep = self.separator.glyph(false);
        // One ellipsis in the library, resolved from the glyph catalog under
        // the profile this trail is painting in (plans/020).
        let glyphs = { self.system.glyphs };
        let ellipsis = glyphs.ellipsis();

        // Build slots
        let slots: Vec<PaintSlot<Id>> = if collapse {
            let hidden: Vec<Id> = self.items[1..self.items.len() - 1]
                .iter()
                .map(|i| i.id.clone())
                .collect();
            vec![
                PaintSlot::Item {
                    index: 0,
                    id: self.items[0].id.clone(),
                },
                PaintSlot::Ellipsis { hidden },
                PaintSlot::Item {
                    index: self.items.len() - 1,
                    id: self.items[self.items.len() - 1].id.clone(),
                },
            ]
        } else {
            self.items
                .iter()
                .enumerate()
                .map(|(index, i)| PaintSlot::Item {
                    index,
                    id: i.id.clone(),
                })
                .collect()
        };

        let mut x = area.x;
        let y = area.y;
        let current_idx = BreadcrumbsState::current_index(self.items);

        for (si, slot) in slots.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            if si > 0 {
                let sw = display_cols(sep) as u16;
                if x.saturating_add(sw) > area.right() {
                    break;
                }
                buffer.set_stringn(
                    x,
                    y,
                    sep,
                    usize::from(sw),
                    self.system.style(Role::TextMuted),
                );
                x = x.saturating_add(sw);
            }

            match slot {
                PaintSlot::Item { index, id } => {
                    let item = &self.items[*index];
                    let is_current = *index == current_idx || item.current;
                    let is_focus =
                        state.focused && !state.focus_on_ellipsis && state.focus_index == *index;
                    let mut label = item.label.clone();
                    if let Some(m) = item.status.mark(false) {
                        label = format!("{m}{label}");
                    }
                    let max_w = if collapse { 12usize } else { 24 };
                    let shown = take_display_cols(&label, max_w);
                    let w = display_cols(&shown) as u16;
                    let avail = area.right().saturating_sub(x);
                    let w = w.min(avail);
                    if w == 0 {
                        break;
                    }
                    let rect = Rect::new(x, y, w, 1);
                    let style = if !item.enabled && !is_current {
                        self.system.style(Role::TextDisabled)
                    } else if is_focus {
                        // The keyboard says itself with the focus tone and
                        // weight, not a reversed slab.
                        self.system.style(Role::Focus).add_modifier(Modifier::BOLD)
                    } else if is_current {
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD)
                    } else if matches!(item.status, BreadcrumbStatus::Error) {
                        self.system.style(Role::Danger)
                    } else if matches!(item.status, BreadcrumbStatus::Warning) {
                        self.system.style(Role::Warning)
                    } else {
                        self.system.style(Role::TextMuted)
                    };
                    // Bold carries "you are here" without color.
                    let style = if is_current && !is_focus {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    };
                    buffer.set_stringn(rect.x, rect.y, &shown, usize::from(rect.width), style);
                    hits.push((
                        BreadcrumbHit::Item {
                            index: *index,
                            id: id.clone(),
                        },
                        rect,
                    ));
                    x = x.saturating_add(w);
                }
                PaintSlot::Ellipsis { hidden } => {
                    let w = display_cols(ellipsis) as u16;
                    let avail = area.right().saturating_sub(x);
                    let w = w.min(avail);
                    if w == 0 {
                        break;
                    }
                    let rect = Rect::new(x, y, w, 1);
                    let is_focus = state.focused && state.focus_on_ellipsis;
                    let style = if is_focus {
                        self.system.style(Role::Focus).add_modifier(Modifier::BOLD)
                    } else {
                        self.system.style(Role::TextMuted)
                    };
                    buffer.set_stringn(rect.x, rect.y, ellipsis, usize::from(rect.width), style);
                    hits.push((
                        BreadcrumbHit::Ellipsis {
                            hidden: hidden.clone(),
                        },
                        rect,
                    ));
                    x = x.saturating_add(w);
                }
            }
        }
        hits
    }

    /// Semantic: one control, not N tab stops.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &BreadcrumbsState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "breadcrumbs mode={} presentation={} n={}",
            state.mode.id(),
            state.presentation.id(),
            self.items.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("breadcrumbs")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: false,
                    invalid: false,
                    expanded: state.overflow_open
                        || matches!(state.mode, BreadcrumbsMode::Editable),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Breadcrumbs<'_, Id> {
    type State = BreadcrumbsState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Breadcrumbs<'_, Id> {
    type State = BreadcrumbsState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// Fallback if SemanticRole::Navigation missing
// checked at compile time

/// Adapter: path-like labels to crumbs with synthetic string ids.
#[must_use]
pub fn crumbs_from_labels(labels: &[&str]) -> Vec<BreadcrumbItem<String>> {
    let n = labels.len();
    labels
        .iter()
        .enumerate()
        .map(|(i, l)| BreadcrumbItem::new((*l).to_owned(), *l).current(i + 1 == n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;
    use crate::widgets::tests::click;

    fn sample() -> Vec<BreadcrumbItem<&'static str>> {
        vec![
            BreadcrumbItem::new("root", "home"),
            BreadcrumbItem::new("a", "projects"),
            BreadcrumbItem::new("b", "termrock"),
            BreadcrumbItem::new("c", "src").current(true),
        ]
    }

    #[test]
    fn breadcrumbs_navigate() {
        let items = [
            BreadcrumbItem::new("r", "root"),
            BreadcrumbItem::new("a", "a"),
        ];
        let mut state = BreadcrumbsState::default();
        state.set_focused(true);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::Navigate("a")
        ));
    }

    #[test]
    fn collapse_preserves_first_and_last() {
        let system = DesignSystem::new(RolePalette::default());
        let items = sample();
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 28, 1); // < 40, len 4
        let mut buf = Buffer::empty(area);
        let hits = Breadcrumbs::new(&items, &system).paint(area, &mut buf, &mut state);
        assert_eq!(state.presentation(), BreadcrumbsPresentation::Collapsed);
        // first and last present
        assert!(
            hits.iter()
                .any(|(h, _)| matches!(h, BreadcrumbHit::Item { index: 0, .. }))
        );
        assert!(
            hits.iter()
                .any(|(h, _)| matches!(h, BreadcrumbHit::Item { index: 3, .. }))
        );
        assert!(
            hits.iter()
                .any(|(h, _)| matches!(h, BreadcrumbHit::Ellipsis { .. }))
        );
    }

    #[test]
    fn ellipsis_opens_overflow() {
        let items = sample();
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        state.presentation = BreadcrumbsPresentation::Collapsed;
        state.focus_on_ellipsis = true;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::OpenOverflow { ids } if ids == ["a", "b"]
        ));
    }

    #[test]
    fn editable_path_commit() {
        let items = sample();
        let mut state = BreadcrumbsState::new().with_editable(true);
        state.set_focused(true);
        assert!(matches!(
            state.start_edit(&items),
            BreadcrumbsOutcome::EditStarted { .. }
        ));
        assert!(matches!(state.mode(), BreadcrumbsMode::Editable));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::EditCommitted { path } if path.contains("termrock")
        ));
    }

    #[test]
    fn editable_esc_cancels() {
        let items = sample();
        let mut state = BreadcrumbsState::new().with_editable(true);
        state.set_focused(true);
        let _ = state.start_edit(&items);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::EditCancelled
        ));
    }

    #[test]
    fn single_tab_stop_internal_move() {
        let items = sample();
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        assert_eq!(state.focus_index(), 0);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        assert_eq!(state.focus_index(), 2);
        // not editing → one control
        assert!(matches!(state.mode(), BreadcrumbsMode::Trail));
    }

    #[test]
    fn current_not_navigated_starts_edit_when_editable() {
        let items = sample();
        let mut state = BreadcrumbsState::new().with_editable(true);
        state.set_focused(true);
        state.focus_index = 3; // current
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::EditStarted { .. }
        ));
    }

    #[test]
    fn repeated_one_shot_actions_are_ignored_but_draft_text_repeats() {
        let items = sample();
        let mut state = BreadcrumbsState::new().with_editable(true);
        state.set_focused(true);
        state.focus_index = 0;

        for (code, modifiers) in [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char(' '), KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('/'), KeyModifiers::NONE),
            (KeyCode::Char('e'), KeyModifiers::CONTROL),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            let before = state.clone();
            assert_eq!(
                state.handle_key(repeat, &items),
                BreadcrumbsOutcome::Ignored
            );
            assert_eq!(state, before, "{code:?} repeat mutated breadcrumbs");
        }

        state.focus_index = items.len() - 1;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::EditStarted { .. }
        ));
        for code in [KeyCode::Enter, KeyCode::Esc] {
            let mut repeat = KeyEvent::new(code, KeyModifiers::NONE);
            repeat.kind = KeyEventKind::Repeat;
            let before = state.clone();
            assert_eq!(
                state.handle_key(repeat, &items),
                BreadcrumbsOutcome::Ignored
            );
            assert_eq!(
                state, before,
                "{code:?} repeat mutated editable breadcrumbs"
            );
        }

        let mut repeat_text = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        repeat_text.kind = KeyEventKind::Repeat;
        assert_eq!(
            state.handle_key(repeat_text, &items),
            BreadcrumbsOutcome::Changed
        );
        assert!(state.draft.ends_with('x'));
    }

    #[test]
    fn disabled_segment_ignored() {
        let items = [
            BreadcrumbItem::new("a", "A"),
            BreadcrumbItem::new("b", "B").enabled(false),
            BreadcrumbItem::new("c", "C").current(true),
        ];
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        state.focus_index = 1;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::Ignored
        ));
    }

    #[test]
    fn status_paint_and_ascii() {
        let system = DesignSystem::default();
        let items = [
            BreadcrumbItem::new("r", "root"),
            BreadcrumbItem::new("x", "broken")
                .status(BreadcrumbStatus::Error)
                .current(true),
        ];
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let _ = Breadcrumbs::new(&items, &system)
            .separator(BreadcrumbSeparator::Chevron)
            .paint(area, &mut buf, &mut state);
    }

    #[test]
    fn mouse_navigate() {
        let system = DesignSystem::default();
        let items = sample();
        let mut state = BreadcrumbsState::new();
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let hits = Breadcrumbs::new(&items, &system).paint(area, &mut buf, &mut state);
        let (hit, rect) = hits
            .iter()
            .find(|(h, _)| matches!(h, BreadcrumbHit::Item { index: 1, .. }))
            .expect("projects");
        assert!(matches!(
            state.handle_mouse(click(rect.x, rect.y), &items, &[(hit.clone(), *rect)],),
            BreadcrumbsOutcome::Navigate("a")
        ));
    }

    #[test]
    fn fuzz_keys() {
        let items = sample();
        let mut state = BreadcrumbsState::new().with_editable(true);
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &items);
            state.set_focused(true);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let items = sample();
        let mut state = BreadcrumbsState::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        let w = Breadcrumbs::new(&items, &system);
        for _ in 0..50 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic_single_control() {
        let system = DesignSystem::default();
        let items = sample();
        let state = BreadcrumbsState::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        Breadcrumbs::new(&items, &system).register_semantic(
            &mut scene,
            "bc",
            Rect::new(0, 0, 40, 1),
            &state,
        );
        assert!(scene.get(&"bc").is_some());
    }

    #[test]
    fn crumbs_from_labels_marks_current() {
        let c = crumbs_from_labels(&["a", "b", "c"]);
        assert!(c[2].current);
        assert!(!c[0].current);
    }
}
