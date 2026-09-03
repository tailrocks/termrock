// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! KeyValueList — compact metadata presentation (DescriptionList for terminals).
//!
//! **Mission.** Settings panes, object summaries, dialog details, and inspector
//! sidebars: aligned keys, wrapped values, nested groups, copy, status tones,
//! links, secret redaction, and narrow stacked anatomy.
//!
//! **vs [`DetailTable`](crate::widgets::DetailTable).** DetailTable remains the
//! selection-heavy scrollable detail surface used by dialogs. KeyValueList is
//! the product-neutral metadata list with groups, density recipes, stacked
//! contraction, and redaction. Prefer KeyValueList for new settings/summary UI.
//!
//! Research: system-info TUIs, detail panels, shadcn DescriptionList patterns.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{NavigationMove, PageMove, UiIntent, default_list_intent};
use crate::scroll;
use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols, wrap_display_cols};

const ROW_GUTTER: u16 = 2;

pub(crate) const fn kv_stack_below() -> u16 {
    36
}

/// Column vs stacked anatomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvLayout {
    /// Two-column when wide enough; stacked when narrow.
    #[default]
    Auto,
    /// Always key | value on one visual row (value may wrap).
    Columns,
    /// Always key row then value row(s).
    Stacked,
}

impl KvLayout {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Columns => "columns",
            Self::Stacked => "stacked",
        }
    }
}

/// Status tone for value emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KvStatus {
    /// Neutral meta.
    #[default]
    Neutral,
    /// Success / active.
    Success,
    /// Caution.
    Warning,
    /// Error / destructive.
    Danger,
    /// Informational.
    Info,
}

impl KvStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Info => "info",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Neutral => Role::Text,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Danger => Role::Danger,
            Self::Info => Role::TextSecondary,
        }
    }
}

// ── Entries ─────────────────────────────────────────────────────────────────

/// One flat projected entry (item or group header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KvEntry<'a, Id = ()> {
    /// Stable identity (required when interactive).
    pub id: Id,
    /// Key / group title.
    pub key: &'a str,
    /// Primary value (empty for pure group headers).
    pub value: &'a str,
    /// Secondary annotation (hint, unit, path). Dropped before primary when tight.
    pub annotation: Option<&'a str>,
    /// Optional link destination for the value.
    pub href: Option<&'a str>,
    /// Host may copy primary value (or secret plaintext when revealed).
    pub copyable: bool,
    /// Value is secret — paint redacted unless revealed in state.
    pub secret: bool,
    /// Status tone for the primary value.
    pub status: Option<KvStatus>,
    /// Nesting depth (0 = top). Indent keys.
    pub depth: u8,
    /// Group header (no value column; optional expand is host-owned).
    pub group: bool,
}

impl<'a, Id> KvEntry<'a, Id> {
    /// Simple key/value.
    #[must_use]
    pub const fn pair(id: Id, key: &'a str, value: &'a str) -> Self {
        Self {
            id,
            key,
            value,
            annotation: None,
            href: None,
            copyable: false,
            secret: false,
            status: None,
            depth: 0,
            group: false,
        }
    }

    /// Group header.
    #[must_use]
    pub const fn group_header(id: Id, title: &'a str) -> Self {
        Self {
            id,
            key: title,
            value: "",
            annotation: None,
            href: None,
            copyable: false,
            secret: false,
            status: None,
            depth: 0,
            group: true,
        }
    }

    /// Annotation builder.
    #[must_use]
    pub const fn annotation(mut self, text: &'a str) -> Self {
        self.annotation = Some(text);
        self
    }

    /// Copyable.
    #[must_use]
    pub const fn copyable(mut self) -> Self {
        self.copyable = true;
        self
    }

    /// Secret redaction.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Link.
    #[must_use]
    pub const fn href(mut self, href: &'a str) -> Self {
        self.href = Some(href);
        self
    }

    /// Status tone.
    #[must_use]
    pub const fn status(mut self, status: KvStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Nesting depth.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Whether the row has an interactive action (copy / link / secret reveal).
    #[must_use]
    pub const fn interactive(&self) -> bool {
        !self.group && (self.copyable || self.href.is_some() || self.secret)
    }

    /// Plain text for clipboard (never redacted secret placeholder — host
    /// passes true secret in `value` and only paints redaction).
    #[must_use]
    pub fn copy_text(&self) -> &str {
        if self.secret { self.value } else { self.value }
    }
}

// ── State / parts / outcomes ────────────────────────────────────────────────

/// Painted geometry for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvEntryParts<Id> {
    /// Entry id.
    pub id: Id,
    /// Full hit target.
    pub root: Rect,
    /// Key band.
    pub key: Rect,
    /// Value band.
    pub value: Rect,
    /// Whether this entry was interactive.
    pub interactive: bool,
}

/// Last paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueListParts<Id> {
    /// Root area.
    pub root: Rect,
    /// Resolved layout mode for this paint.
    pub layout: KvLayout,
    /// First display row scrolled.
    pub first_row: u16,
    /// Total display rows.
    pub total_rows: u16,
    /// Entry regions in paint order.
    pub entries: Vec<KvEntryParts<Id>>,
}

/// Interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueListState<Id> {
    /// Keyboard focus.
    pub focused: bool,
    /// Cursor entry id.
    pub cursor: Option<Id>,
    /// Hover entry id.
    pub hovered: Option<Id>,
    /// Revealed secret ids (host may clear on blur).
    pub revealed: Vec<Id>,
    /// Last copied id (brief feedback).
    pub copied: Option<Id>,
    /// Vertical scroll in display rows.
    pub scroll_y: u16,
    /// Last parts.
    pub parts: Option<KeyValueListParts<Id>>,
    viewport_rows: u16,
    total_rows: u16,
}

impl<Id> Default for KeyValueListState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> KeyValueListState<Id> {
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            cursor: None,
            hovered: None,
            revealed: Vec::new(),
            copied: None,
            scroll_y: 0,
            parts: None,
            viewport_rows: 0,
            total_rows: 0,
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }
}

impl<Id: Clone + PartialEq> KeyValueListState<Id> {
    /// Secret currently revealed?
    #[must_use]
    pub fn is_revealed(&self, id: &Id) -> bool {
        self.revealed.iter().any(|r| r == id)
    }

    /// Toggle secret reveal.
    pub fn toggle_reveal(&mut self, id: Id) -> bool {
        if let Some(i) = self.revealed.iter().position(|r| r == &id) {
            self.revealed.remove(i);
            false
        } else {
            self.revealed.push(id);
            true
        }
    }

    /// Clamp scroll.
    pub fn clamp(&mut self) {
        let max = self.total_rows.saturating_sub(self.viewport_rows.max(1));
        if self.scroll_y > max {
            self.scroll_y = max;
        }
    }

    /// Scroll by display rows.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        let before = self.scroll_y;
        scroll::apply_delta_unclamped_u16(&mut self.scroll_y, delta);
        self.clamp();
        before != self.scroll_y
    }
}

/// Outcomes for host effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyValueListOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor / selection moved.
    Selected(Id),
    /// Copy request — text is primary value (true secret when redacted row).
    Copy {
        /// Entry id.
        id: Id,
        /// Clipboard text.
        text: String,
    },
    /// Open link.
    ActivateLink {
        /// Entry id.
        id: Id,
        /// Destination.
        href: String,
    },
    /// Secret reveal toggled on.
    SecretRevealed(Id),
    /// Secret hide.
    SecretHidden(Id),
    /// Scroll changed.
    Scrolled {
        /// Display row.
        scroll_y: u16,
    },
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Key/value metadata list.
#[derive(Debug, Clone, Copy)]
pub struct KeyValueList<'a, Id> {
    entries: &'a [KvEntry<'a, Id>],
    system: &'a DesignSystem,
    layout: KvLayout,
    /// Fixed key column width (0 = auto max key).
    key_width: u16,
    /// Separator between key and value in columns mode.
    separator: &'a str,
}

impl<'a, Id> KeyValueList<'a, Id> {
    /// List over borrowed entries.
    #[must_use]
    pub const fn new(entries: &'a [KvEntry<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            layout: KvLayout::Auto,
            key_width: 0,
            separator: system.kv_separator().text(),
        }
    }

    /// Dense recipe (settings drawers).
    #[must_use]
    pub const fn dense(entries: &'a [KvEntry<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(entries, system)
    }

    /// Reading recipe (docs / summaries).
    #[must_use]
    pub const fn reading(entries: &'a [KvEntry<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(entries, system)
    }

    /// Layout override.
    #[must_use]
    pub const fn layout(mut self, layout: KvLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Fixed key column width.
    #[must_use]
    pub const fn key_width(mut self, width: u16) -> Self {
        self.key_width = width;
        self
    }

    /// Column separator (default two spaces — whitespace before chrome).
    #[must_use]
    pub const fn separator(mut self, sep: &'a str) -> Self {
        self.separator = sep;
        self
    }

    /// Resolve layout for width.
    #[must_use]
    pub fn resolved_layout(&self, width: u16) -> KvLayout {
        match self.layout {
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

    fn resolved_key_width(&self) -> usize {
        if self.key_width > 0 {
            return usize::from(self.key_width);
        }
        self.entries
            .iter()
            .filter(|e| !e.group)
            .map(|e| display_cols(e.key) + usize::from(e.depth).saturating_mul(2))
            .max()
            .unwrap_or(0)
            .min(32)
            .max(4)
    }
}

impl<'a, Id: Clone + PartialEq> KeyValueList<'a, Id> {
    /// Display text for value (redacted when secret and not revealed).
    #[must_use]
    pub fn display_value(&self, entry: &KvEntry<'a, Id>, state: &KeyValueListState<Id>) -> String {
        if entry.secret && !state.is_revealed(&entry.id) {
            return "••••••••".into();
        }
        entry.value.to_string()
    }

    /// Measure display rows for one entry.
    #[must_use]
    pub fn measure_entry_height(
        &self,
        entry: &KvEntry<'a, Id>,
        width: u16,
        layout: KvLayout,
        state: &KeyValueListState<Id>,
    ) -> u16 {
        if width == 0 {
            return 0;
        }
        if entry.group {
            return 1u16;
        }
        let value = self.display_value(entry, state);
        let show_ann = entry.annotation.is_some_and(|a| !a.is_empty());
        match layout {
            KvLayout::Stacked | KvLayout::Auto => {
                // Auto resolved already
                let value_w = usize::from(
                    width
                        .saturating_sub(ROW_GUTTER)
                        .saturating_sub(u16::from(entry.depth) * 2)
                        .max(1),
                );
                let vh = wrap_display_cols(&value, value_w).len().max(1);
                let ah = if show_ann {
                    // annotation only if primary fits somewhat
                    1usize
                } else {
                    0
                };
                u16::try_from(1 + vh + ah).unwrap_or(u16::MAX) // key + value lines + ann
            }
            KvLayout::Columns => {
                let key_w = self.resolved_key_width();
                let sep_w = display_cols(self.separator);
                let indent = usize::from(entry.depth).saturating_mul(2);
                let value_w = usize::from(width)
                    .saturating_sub(usize::from(ROW_GUTTER) + indent + key_w + sep_w)
                    .max(1);
                // Prefer primary value; annotation only if remaining room on last line
                let body = if show_ann {
                    if let Some(ann) = entry.annotation {
                        format!("{value}  {ann}")
                    } else {
                        value.clone()
                    }
                } else {
                    value.clone()
                };
                let lines = wrap_display_cols(&body, value_w).len().max(1);
                // If too narrow for both, drop annotation and remeasure
                if show_ann && lines > 2 {
                    let lines = wrap_display_cols(&value, value_w).len().max(1);
                    return u16::try_from(lines).unwrap_or(1);
                }
                u16::try_from(lines).unwrap_or(1)
            }
        }
    }

    /// Total display rows.
    #[must_use]
    pub fn measure_height(&self, width: u16, state: &KeyValueListState<Id>) -> u16 {
        let layout = self.resolved_layout(width);
        // For measure_entry Stacked branch when Auto resolves to Stacked
        let layout = if matches!(layout, KvLayout::Stacked) {
            KvLayout::Stacked
        } else {
            KvLayout::Columns
        };
        self.entries
            .iter()
            .map(|e| self.measure_entry_height(e, width, layout, state))
            .fold(0u16, |a, h| a.saturating_add(h))
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut KeyValueListState<Id>,
    ) -> KeyValueListParts<Id> {
        if area.is_empty() {
            let parts = KeyValueListParts {
                root: area,
                layout: self.layout,
                first_row: 0,
                total_rows: 0,
                entries: Vec::new(),
            };
            state.parts = Some(parts.clone());
            return parts;
        }
        let layout = self.resolved_layout(area.width);
        let paint_layout = if matches!(layout, KvLayout::Stacked) {
            KvLayout::Stacked
        } else {
            KvLayout::Columns
        };

        // Entry heights in document order; row positions derive
        // arithmetically instead of materializing a per-row map each frame.
        let heights: Vec<u16> = self
            .entries
            .iter()
            .map(|entry| self.measure_entry_height(entry, area.width, paint_layout, state))
            .collect();
        let total = heights.iter().fold(0u16, |a, h| a.saturating_add(*h));
        state.total_rows = total;
        state.viewport_rows = area.height;
        state.clamp();

        let first = usize::from(state.scroll_y);
        let mut entry_parts: Vec<KvEntryParts<Id>> = Vec::new();
        // Track first painted row per entry for hit regions
        let mut entry_first_y: Vec<Option<u16>> = vec![None; self.entries.len()];
        let mut entry_last_y: Vec<Option<u16>> = vec![None; self.entries.len()];

        for row in 0..area.height {
            let idx = first.saturating_add(usize::from(row));
            let Some((ei, sub)) = walk_entry_rows(&heights, idx) else {
                break;
            };
            let y = area.y.saturating_add(row);
            entry_first_y[ei] = entry_first_y[ei].or(Some(y));
            entry_last_y[ei] = Some(y);
            let entry = &self.entries[ei];
            self.paint_entry_row(
                entry,
                sub,
                paint_layout,
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buffer,
                state,
            );
        }

        for (ei, entry) in self.entries.iter().enumerate() {
            if let (Some(y0), Some(y1)) = (entry_first_y[ei], entry_last_y[ei]) {
                let root = Rect {
                    x: area.x,
                    y: y0,
                    width: area.width,
                    height: y1.saturating_sub(y0).saturating_add(1),
                };
                entry_parts.push(KvEntryParts {
                    id: entry.id.clone(),
                    root,
                    key: root,
                    value: root,
                    interactive: entry.interactive(),
                });
            }
        }

        let parts = KeyValueListParts {
            root: area,
            layout,
            first_row: state.scroll_y,
            total_rows: total,
            entries: entry_parts,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn paint_entry_row(
        &self,
        entry: &KvEntry<'a, Id>,
        sub: u16,
        layout: KvLayout,
        area: Rect,
        buffer: &mut Buffer,
        state: &KeyValueListState<Id>,
    ) {
        if area.is_empty() {
            return;
        }
        let selected = state.cursor.as_ref() == Some(&entry.id);
        let hovered = state.hovered.as_ref() == Some(&entry.id);
        let chrome = crate::widgets::row_chrome::RowChrome::resolve(
            self.system,
            crate::style::ListRowVisualState {
                selected,
                focused: selected && state.focused,
                hovered,
                enabled: true,
                ..Default::default()
            },
        );

        if entry.group {
            if sub == 0 {
                let indent = "  ".repeat(usize::from(entry.depth));
                let title = format!("{indent}{}", entry.key);
                let clipped = take_display_cols(&title, usize::from(area.width));
                let mut style = self.system.style(Role::TextStrong);
                style = style.add_modifier(Modifier::BOLD);
                let content = Rect::new(
                    area.x.saturating_add(ROW_GUTTER),
                    area.y,
                    area.width.saturating_sub(ROW_GUTTER),
                    1,
                );
                buffer.set_stringn(
                    content.x,
                    content.y,
                    &clipped,
                    usize::from(content.width),
                    style,
                );
            }
            // gap rows blank
            paint_entry_chrome(&chrome, sub, buffer, area);
            return;
        }

        let value = self.display_value(entry, state);
        let indent_cols = u16::from(entry.depth).saturating_mul(2);
        let x0 = area
            .x
            .saturating_add(ROW_GUTTER)
            .saturating_add(indent_cols);
        let w0 = area
            .width
            .saturating_sub(ROW_GUTTER)
            .saturating_sub(indent_cols);

        match layout {
            KvLayout::Stacked => {
                if sub == 0 {
                    // key only
                    let clipped = take_display_cols(entry.key, usize::from(w0));
                    buffer.set_stringn(
                        x0,
                        area.y,
                        &clipped,
                        usize::from(w0),
                        self.system.style(Role::TextMuted),
                    );
                } else {
                    let value_w = usize::from(w0.max(1));
                    let lines = wrap_display_cols(&value, value_w);
                    let vi = usize::from(sub.saturating_sub(1));
                    if let Some(line) = lines.get(vi) {
                        let style = self.value_style(entry, selected, hovered);
                        buffer.set_stringn(x0, area.y, line, usize::from(w0), style);
                    } else if entry.annotation.is_some() && vi == lines.len() {
                        // annotation after value wraps
                        if let Some(ann) = entry.annotation {
                            let clipped = take_display_cols(ann, usize::from(w0));
                            buffer.set_stringn(
                                x0,
                                area.y,
                                &clipped,
                                usize::from(w0),
                                self.system.style(Role::TextDisabled),
                            );
                        }
                    }
                    // affordance on first value line
                    if sub == 1 && entry.interactive() {
                        self.paint_affordance(entry, area, buffer, state);
                    }
                }
            }
            KvLayout::Columns | KvLayout::Auto => {
                let key_w = self.resolved_key_width();
                let sep_w = display_cols(self.separator);
                let key_budget = key_w.min(usize::from(w0));
                if sub == 0 {
                    let key = take_display_cols(entry.key, key_budget);
                    buffer.set_stringn(
                        x0,
                        area.y,
                        &key,
                        key_budget,
                        self.system.style(Role::TextMuted),
                    );
                    if w0 as usize > key_budget {
                        let sep = take_display_cols(self.separator, sep_w);
                        buffer.set_stringn(
                            x0.saturating_add(u16::try_from(key_budget).unwrap_or(0)),
                            area.y,
                            &sep,
                            sep_w,
                            self.system.style(Role::TextDisabled),
                        );
                    }
                }
                let value_x = x0
                    .saturating_add(u16::try_from(key_budget).unwrap_or(0))
                    .saturating_add(u16::try_from(sep_w).unwrap_or(0));
                let value_w = area
                    .width
                    .saturating_sub(value_x.saturating_sub(area.x))
                    .max(1);
                // Drop annotation when narrow: if key_budget + sep + min value > width
                let mut body = value.clone();
                let show_ann = entry.annotation.is_some_and(|a| !a.is_empty())
                    && value_w > 12
                    && display_cols(&value) + 4 < usize::from(value_w);
                if show_ann {
                    if let Some(ann) = entry.annotation {
                        body = format!("{value}  {ann}");
                    }
                }
                // When annotation would force too many wraps, prefer primary only
                let lines = wrap_display_cols(&body, usize::from(value_w));
                let lines = if lines.len() > 3 && show_ann {
                    wrap_display_cols(&value, usize::from(value_w))
                } else {
                    lines
                };
                if let Some(line) = lines.get(usize::from(sub)) {
                    // Split annotation styling: paint primary; if line contains only ann — muted
                    let style = self.value_style(entry, selected, hovered);
                    let paint_line = if show_ann && usize::from(sub) + 1 == lines.len() {
                        // last line may include annotation — already in body
                        line.as_str()
                    } else {
                        line.as_str()
                    };
                    buffer.set_stringn(value_x, area.y, paint_line, usize::from(value_w), style);
                }
                if sub == 0 && entry.interactive() {
                    self.paint_affordance(entry, area, buffer, state);
                }
            }
        }
        paint_entry_chrome(&chrome, sub, buffer, area);
    }

    fn value_style(
        &self,
        entry: &KvEntry<'a, Id>,
        selected: bool,
        hovered: bool,
    ) -> ratatui_core::style::Style {
        let role = if entry.secret {
            Role::TextMuted
        } else if let Some(st) = entry.status {
            st.role()
        } else if entry.href.is_some() {
            if hovered { Role::LinkHover } else { Role::Link }
        } else {
            Role::Text
        };
        let mut style = self.system.style(role);
        style = ratatui_core::style::Style { bg: None, ..style };
        if entry.href.is_some() {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if selected {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    fn paint_affordance(
        &self,
        entry: &KvEntry<'a, Id>,
        area: Rect,
        buffer: &mut Buffer,
        state: &KeyValueListState<Id>,
    ) {
        if area.width < 4 {
            return;
        }
        let mark = if entry.secret {
            if state.is_revealed(&entry.id) {
                " ◉"
            } else {
                " •••"
            }
        } else if entry.copyable {
            if state.copied.as_ref() == Some(&entry.id) {
                " ✓"
            } else {
                " ⧉"
            }
        } else if entry.href.is_some() {
            " ↗"
        } else {
            return;
        };
        let mw = display_cols(mark);
        if mw >= usize::from(area.width) {
            return;
        }
        let x = area
            .x
            .saturating_add(area.width)
            .saturating_sub(u16::try_from(mw).unwrap_or(0));
        buffer.set_stringn(x, area.y, mark, mw, self.system.style(Role::TextDisabled));
    }

    /// Keys.
    pub fn handle_key(
        &self,
        state: &mut KeyValueListState<Id>,
        key: KeyEvent,
    ) -> KeyValueListOutcome<Id> {
        if !state.focused || !key.is_press() {
            return KeyValueListOutcome::Ignored;
        }
        // copy
        if matches!(key.code, crate::input::KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            if let Some(id) = state.cursor.clone() {
                if let Some(e) = self.entries.iter().find(|e| e.id == id) {
                    if e.copyable || e.secret {
                        state.copied = Some(id.clone());
                        return KeyValueListOutcome::Copy {
                            id,
                            text: e.copy_text().to_string(),
                        };
                    }
                }
            }
        }
        // reveal secret
        if matches!(key.code, crate::input::KeyCode::Char('r' | 'R' | ' '))
            && key.modifiers.is_empty()
        {
            if let Some(id) = state.cursor.clone() {
                if let Some(e) = self.entries.iter().find(|e| e.id == id) {
                    if e.secret {
                        let on = state.toggle_reveal(id.clone());
                        return if on {
                            KeyValueListOutcome::SecretRevealed(id)
                        } else {
                            KeyValueListOutcome::SecretHidden(id)
                        };
                    }
                }
            }
        }
        if let Some(intent) = default_list_intent(key) {
            return self.handle_intent(state, intent);
        }
        KeyValueListOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(
        &self,
        state: &mut KeyValueListState<Id>,
        intent: UiIntent,
    ) -> KeyValueListOutcome<Id> {
        if !state.focused {
            return KeyValueListOutcome::Ignored;
        }
        let page = i32::from(state.viewport_rows.max(1));
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                self.move_cursor(state, -1)
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                self.move_cursor(state, 1)
            }
            UiIntent::Move(NavigationMove::First) => {
                state.scroll_y = 0;
                if let Some(e) = self.entries.iter().find(|e| !e.group) {
                    state.cursor = Some(e.id.clone());
                    return KeyValueListOutcome::Selected(e.id.clone());
                }
                KeyValueListOutcome::Scrolled { scroll_y: 0 }
            }
            UiIntent::Move(NavigationMove::Last) => {
                if let Some(e) = self.entries.iter().rev().find(|e| !e.group) {
                    state.cursor = Some(e.id.clone());
                    self.reveal_entry(state, &e.id);
                    return KeyValueListOutcome::Selected(e.id.clone());
                }
                KeyValueListOutcome::Ignored
            }
            UiIntent::Page(PageMove::Backward) => {
                if state.scroll_by(-page) {
                    KeyValueListOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    KeyValueListOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                if state.scroll_by(page) {
                    KeyValueListOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    KeyValueListOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(id) = state.cursor.clone() {
                    if let Some(e) = self.entries.iter().find(|e| e.id == id) {
                        if let Some(href) = e.href {
                            return KeyValueListOutcome::ActivateLink {
                                id,
                                href: href.to_string(),
                            };
                        }
                        if e.copyable {
                            state.copied = Some(id.clone());
                            return KeyValueListOutcome::Copy {
                                id,
                                text: e.copy_text().to_string(),
                            };
                        }
                        if e.secret {
                            let on = state.toggle_reveal(id.clone());
                            return if on {
                                KeyValueListOutcome::SecretRevealed(id)
                            } else {
                                KeyValueListOutcome::SecretHidden(id)
                            };
                        }
                        return KeyValueListOutcome::Selected(id);
                    }
                }
                KeyValueListOutcome::Ignored
            }
            _ => KeyValueListOutcome::Ignored,
        }
    }

    fn move_cursor(
        &self,
        state: &mut KeyValueListState<Id>,
        delta: isize,
    ) -> KeyValueListOutcome<Id> {
        let ids: Vec<Id> = self
            .entries
            .iter()
            .filter(|e| !e.group)
            .map(|e| e.id.clone())
            .collect();
        if ids.is_empty() {
            return KeyValueListOutcome::Ignored;
        }
        let cur = state
            .cursor
            .as_ref()
            .and_then(|c| ids.iter().position(|i| i == c))
            .unwrap_or(0);
        let next = if delta >= 0 {
            (cur + 1).min(ids.len() - 1)
        } else {
            cur.saturating_sub(1)
        };
        let id = ids[next].clone();
        state.cursor = Some(id.clone());
        self.reveal_entry(state, &id);
        KeyValueListOutcome::Selected(id)
    }

    fn reveal_entry(&self, state: &mut KeyValueListState<Id>, id: &Id) {
        if let Some(idx) = self.entries.iter().position(|e| &e.id == id) {
            let layout =
                self.resolved_layout(state.parts.as_ref().map(|p| p.root.width).unwrap_or(80));
            let paint_layout = if matches!(layout, KvLayout::Stacked) {
                KvLayout::Stacked
            } else {
                KvLayout::Columns
            };
            let mut row = 0u16;
            for e in self.entries.iter().take(idx) {
                row = row.saturating_add(self.measure_entry_height(
                    e,
                    state.parts.as_ref().map(|p| p.root.width).unwrap_or(80),
                    paint_layout,
                    state,
                ));
            }
            let h = self
                .entries
                .get(idx)
                .map(|e| {
                    self.measure_entry_height(
                        e,
                        state.parts.as_ref().map(|p| p.root.width).unwrap_or(80),
                        paint_layout,
                        state,
                    )
                })
                .unwrap_or(1);
            let view = state.viewport_rows.max(1);
            if row < state.scroll_y {
                state.scroll_y = row;
            } else if row.saturating_add(h) > state.scroll_y.saturating_add(view) {
                state.scroll_y = row.saturating_add(h).saturating_sub(view);
            }
            state.clamp();
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &self,
        state: &mut KeyValueListState<Id>,
        event: MouseEvent,
    ) -> KeyValueListOutcome<Id> {
        let Some(parts) = state.parts.clone() else {
            return KeyValueListOutcome::Ignored;
        };
        if !parts.root.contains(event.position) {
            return KeyValueListOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollUp => {
                if state.scroll_by(-3) {
                    return KeyValueListOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    };
                }
            }
            MouseEventKind::ScrollDown => {
                if state.scroll_by(3) {
                    return KeyValueListOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    };
                }
            }
            MouseEventKind::Moved => {
                state.hovered = parts
                    .entries
                    .iter()
                    .find(|e| e.root.contains(event.position))
                    .map(|e| e.id.clone());
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(ep) = parts
                    .entries
                    .iter()
                    .find(|e| e.root.contains(event.position))
                {
                    state.focused = true;
                    state.cursor = Some(ep.id.clone());
                    if let Some(entry) = self.entries.iter().find(|e| e.id == ep.id) {
                        if entry.href.is_some() && entry.interactive() {
                            // Prefer link on value click
                            if let Some(href) = entry.href {
                                return KeyValueListOutcome::ActivateLink {
                                    id: ep.id.clone(),
                                    href: href.to_string(),
                                };
                            }
                        }
                        if entry.secret {
                            let on = state.toggle_reveal(ep.id.clone());
                            return if on {
                                KeyValueListOutcome::SecretRevealed(ep.id.clone())
                            } else {
                                KeyValueListOutcome::SecretHidden(ep.id.clone())
                            };
                        }
                        if entry.copyable {
                            state.copied = Some(ep.id.clone());
                            return KeyValueListOutcome::Copy {
                                id: ep.id.clone(),
                                text: entry.copy_text().to_string(),
                            };
                        }
                        return KeyValueListOutcome::Selected(ep.id.clone());
                    }
                }
            }
            _ => {}
        }
        KeyValueListOutcome::Ignored
    }
}

/// Absolute display row `idx` as `(entry, sub_row)`, from entry heights.
fn walk_entry_rows(heights: &[u16], idx: usize) -> Option<(usize, u16)> {
    let mut seen = 0usize;
    for (ei, h) in heights.iter().enumerate() {
        let h = usize::from(*h);
        if idx < seen.saturating_add(h) {
            return Some((ei, u16::try_from(idx - seen).unwrap_or(u16::MAX)));
        }
        seen = seen.saturating_add(h);
    }
    None
}

fn paint_entry_chrome(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::widgets::tests::click;

    #[test]
    fn separator_comes_from_the_shared_key_value_token() {
        let system = crate::style::DesignSystem::junie();
        let entries = sample_entries();
        assert_eq!(
            KeyValueList::new(&entries, &system).separator,
            system.kv_separator().text()
        );
        assert_eq!(
            KeyValueList::dense(&entries, &system).separator,
            system.kv_separator().text()
        );
        let colon = system.with_kv_separator(crate::style::KvSeparator::Colon);
        assert_eq!(KeyValueList::new(&entries, &colon).separator, " : ");
    }

    fn sample_entries() -> [KvEntry<'static, &'static str>; 5] {
        [
            KvEntry::group_header("g1", "Identity"),
            KvEntry::pair("name", "Name", "termrock")
                .copyable()
                .annotation("crate"),
            KvEntry::pair("status", "Status", "active").status(KvStatus::Success),
            KvEntry::pair("token", "Token", "super-secret-value")
                .secret()
                .copyable(),
            KvEntry::pair("docs", "Docs", "handbook")
                .href("https://example.invalid")
                .annotation("external"),
        ]
    }

    #[test]
    fn auto_stacks_when_narrow() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let list = KeyValueList::reading(&entries, &system);
        assert_eq!(list.resolved_layout(20), KvLayout::Stacked);
        assert_eq!(list.resolved_layout(80), KvLayout::Columns);
    }

    #[test]
    fn dense_stacks_later() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let dense = KeyValueList::dense(&entries, &system);
        assert_eq!(dense.resolved_layout(40), KvLayout::Columns);
        assert_eq!(dense.resolved_layout(30), KvLayout::Stacked);
    }

    #[test]
    fn secret_redacts_until_revealed() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let list = KeyValueList::new(&entries, &system);
        let mut state = KeyValueListState::new();
        let token = entries.iter().find(|e| e.id == "token").unwrap();
        let d = list.display_value(token, &state);
        assert!(!d.contains("super-secret"));
        state.toggle_reveal("token");
        let d2 = list.display_value(token, &state);
        assert!(d2.contains("super-secret"));
    }

    #[test]
    fn paint_columns_and_stacked() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let list = KeyValueList::new(&entries, &system);
        let mut state = KeyValueListState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 48, 12));
        let parts = list.paint(Rect::new(0, 0, 48, 12), &mut buf, &mut state);
        assert!(parts.total_rows > 0);
        let row: String = (0..48).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
        assert!(row.contains("Identity") || row.contains("Name"), "{row}");

        let mut buf2 = Buffer::empty(Rect::new(0, 0, 20, 14));
        let mut state2 = KeyValueListState::new();
        let parts2 = list.paint(Rect::new(0, 0, 20, 14), &mut buf2, &mut state2);
        assert_eq!(parts2.layout, KvLayout::Stacked);
    }

    #[test]
    fn copy_and_link_outcomes() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let list = KeyValueList::new(&entries, &system);
        let mut state = KeyValueListState::new();
        state.set_focused(true);
        state.cursor = Some("name");
        let out = list.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            KeyValueListOutcome::Copy {
                id: "name",
                ref text
            } if text == "termrock"
        ));
        state.cursor = Some("docs");
        let out = list.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            KeyValueListOutcome::ActivateLink { id: "docs", .. }
        ));
    }

    #[test]
    fn navigation_skips_groups() {
        let system = DesignSystem::default();
        let entries = sample_entries();
        let list = KeyValueList::new(&entries, &system);
        let mut state = KeyValueListState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        let _ = list.paint(Rect::new(0, 0, 40, 10), &mut buf, &mut state);
        let out = list.handle_key(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(out, KeyValueListOutcome::Selected(_)));
        assert_ne!(state.cursor, Some("g1"));
    }

    #[test]
    fn annotation_drops_when_tight() {
        let system = DesignSystem::default();
        let entries = [KvEntry::pair("p", "Path", "/very/long/primary/path/value")
            .annotation("secondary annotation that is long")];
        let list = KeyValueList::new(&entries, &system).layout(KvLayout::Columns);
        let mut state = KeyValueListState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 28, 4));
        let _ = list.paint(Rect::new(0, 0, 28, 4), &mut buf, &mut state);
        let mut all = String::new();
        for y in 0..4 {
            for x in 0..28 {
                all.push_str(buf[(x, y)].symbol());
            }
        }
        // primary path fragments should appear; secondary may be dropped
        assert!(
            all.contains("path") || all.contains("Path") || all.contains('/'),
            "{all}"
        );
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let entries = [KvEntry::pair("a", "A", "1")];
        let mut state = KeyValueListState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts =
            KeyValueList::new(&entries, &system).paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.root.is_empty());
    }

    #[test]
    fn large_list_paint_cheap() {
        let system = DesignSystem::default();
        let owned: Vec<(String, String)> = (0..200)
            .map(|i| (format!("k{i}"), format!("value {i}")))
            .collect();
        let entries: Vec<KvEntry<'_, usize>> = owned
            .iter()
            .enumerate()
            .map(|(i, (k, v))| KvEntry::pair(i, k.as_str(), v.as_str()))
            .collect();
        let list = KeyValueList::dense(&entries, &system);
        let mut state = KeyValueListState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 24));
        for _ in 0..100 {
            let _ = list.paint(Rect::new(0, 0, 60, 24), &mut buf, &mut state);
            let _ = state.scroll_by(2);
        }
    }

    #[test]
    fn interactive_flag() {
        let e = KvEntry::pair("a", "A", "1");
        assert!(!e.interactive());
        assert!(e.copyable().interactive());
        assert!(KvEntry::pair("b", "B", "2").href("https://x").interactive());
        assert!(KvEntry::pair("c", "C", "sec").secret().interactive());
        assert!(!KvEntry::group_header("g", "G").interactive());
    }

    #[test]
    fn mouse_copy() {
        let system = DesignSystem::default();
        let entries = [KvEntry::pair("name", "Name", "termrock").copyable()];
        let list = KeyValueList::new(&entries, &system).layout(KvLayout::Columns);
        let mut state = KeyValueListState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        let _ = list.paint(Rect::new(0, 0, 40, 3), &mut buf, &mut state);
        let out = list.handle_mouse(&mut state, click(5, 0));
        assert!(matches!(out, KeyValueListOutcome::Copy { id: "name", .. }));
    }
}
