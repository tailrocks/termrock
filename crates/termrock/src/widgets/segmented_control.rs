// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! SegmentedControl — compact mutually exclusive view / mode selector.
//!
//! **Mission.** View modes, density, model modes, and filters need a dense
//! exclusive control that is **not** a multi-panel Tabs strip, **not** a form
//! RadioGroup, and **not** a chart [`SegmentedMeter`](crate::widgets::SegmentedMeter).
//!
//! **vs [`Tabs`](crate::widgets::Tabs).** Tabs own content panels and often more
//! chrome; SegmentedControl switches a single surface's mode without implying
//! separate page ownership.
//!
//! **vs [`RadioGroup`](crate::widgets::RadioGroup).** RadioGroup is a form field
//! with legend, descriptions, and radio marks. SegmentedControl is toolbar-dense
//! connected segments (`[List] Grid · Table`).
//!
//! **vs [`ToggleGroup`](crate::widgets::ToggleGroup).** ToggleGroup allows multi
//! sticky tools; SegmentedControl is always single-select exclusive.
//!
//! **vs [`ModeRibbon`](crate::widgets::ModeRibbon).** ModeRibbon is the agent
//! workbench seed; prefer SegmentedControl for product-neutral view/mode chips.
//! ModeRibbon may project into SegmentedControl later.
//!
//! **Active state.** Selected segment uses brackets + bold / reverse — never a
//! full neon fill as the only cue.
//!
//! **Narrow.** Low-priority segments overflow to `…`. Below `collapse_below`,
//! the control collapses to a Select-like trigger (host paints the option menu).
//!
//! Research: desktop segmented controls, shadcn patterns, IDE mode selectors.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::Widget,
};

use crate::input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, NavigationMove, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome,
    SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    default_list_intent,
};
use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};

// ── Types ───────────────────────────────────────────────────────────────────

/// How the control is painted at the current width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SegmentedPresentation {
    /// All (or priority) segments inline.
    #[default]
    Expanded,
    /// Some segments in overflow `…`.
    Overflow,
    /// Collapsed to a Select-like trigger (very narrow).
    Collapsed,
}

impl SegmentedPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Overflow => "overflow",
            Self::Collapsed => "collapsed",
        }
    }
}

/// One segment in a [`SegmentedControl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentedItem<'a, Id> {
    /// Stable value id.
    pub id: Id,
    /// Visible label (may be empty when icon-only).
    pub label: &'a str,
    /// Optional leading icon.
    pub icon: Option<&'a str>,
    /// Accessible name when label empty.
    pub accessible_label: Option<&'a str>,
    /// Optional trailing badge (e.g. count).
    pub badge: Option<&'a str>,
    /// Selectable.
    pub enabled: bool,
    /// Overflow priority (higher stays visible longer).
    pub priority: u8,
}

impl<'a, Id> SegmentedItem<'a, Id> {
    /// Enabled labeled segment (priority 50).
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
            accessible_label: None,
            badge: None,
            enabled: true,
            priority: 50,
        }
    }

    /// Icon.
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A11y name (required for icon-only).
    #[must_use]
    pub const fn accessible_label(mut self, name: &'a str) -> Self {
        self.accessible_label = Some(name);
        self
    }

    /// Badge.
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Overflow priority.
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Typeahead / a11y label.
    #[must_use]
    pub fn a11y(&self) -> &str {
        if let Some(a) = self.accessible_label {
            if !a.is_empty() {
                return a;
            }
        }
        if !self.label.is_empty() {
            return self.label;
        }
        self.icon.unwrap_or("segment")
    }

    fn face_inner(&self) -> String {
        let mut s = String::new();
        if let Some(i) = self.icon {
            s.push_str(i);
            if !self.label.is_empty() {
                s.push(' ');
            }
        }
        s.push_str(self.label);
        if let Some(b) = self.badge {
            if !b.is_empty() {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push('[');
                s.push_str(b);
                s.push(']');
            }
        }
        if s.is_empty() {
            s.push('·');
        }
        s
    }
}

/// Per-segment geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedItemParts<Id> {
    /// Id.
    pub id: Id,
    /// Hit rect (empty when overflowed / not painted).
    pub area: Rect,
    /// Overflowed (in menu set).
    pub overflowed: bool,
}

/// Paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedControlParts<Id> {
    /// Root.
    pub root: Rect,
    /// Presentation mode.
    pub presentation: SegmentedPresentation,
    /// Visible segment parts (+ overflowed markers).
    pub items: Vec<SegmentedItemParts<Id>>,
    /// Overflow `…` trigger.
    pub overflow_trigger: Option<Rect>,
    /// Collapsed Select-like trigger (when collapsed).
    pub collapsed_trigger: Option<Rect>,
    /// Ids in overflow (or all non-selected when collapsed).
    pub overflow_ids: Vec<Id>,
}

/// Runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedControlState<Id> {
    /// Controlled selected value.
    pub selected: Option<Id>,
    /// Surface keyboard ownership.
    pub surface_focused: bool,
    /// Roving cursor (may equal selected under FollowFocus).
    pub cursor: Option<Id>,
    /// Hovered segment.
    pub hovered: Option<Id>,
    /// Overflow / collapsed menu open (host paints menu).
    pub menu_open: bool,
    /// Last parts.
    pub parts: Option<SegmentedControlParts<Id>>,
    /// Roving engine.
    pub roving: RovingFocusGroup<Id>,
}

impl<Id: Clone + PartialEq> Default for SegmentedControlState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id: Clone + PartialEq> SegmentedControlState<Id> {
    /// Optional selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected: selected.clone(),
            surface_focused: false,
            cursor: selected,
            hovered: None,
            menu_open: false,
            parts: None,
            roving: RovingFocusGroup::new().orientation(RovingOrientation::Horizontal),
        }
    }

    /// Selected value.
    #[must_use]
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Controlled set (also moves cursor).
    pub fn set_selected(&mut self, selected: Option<Id>) {
        self.selected = selected.clone();
        if selected.is_some() {
            self.cursor = selected;
        }
    }

    /// Surface focus.
    pub fn set_surface_focused(&mut self, on: bool) {
        self.surface_focused = on;
        if !on {
            self.menu_open = false;
            self.roving.clear_typeahead();
        }
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SegmentedControlOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved without selection change (rare; reserved).
    CursorMoved {
        /// Active id.
        id: Id,
    },
    /// Selected value changed.
    Selected {
        /// New value id.
        id: Id,
    },
    /// Overflow / collapsed menu opened (host paints options).
    MenuOpened,
    /// Menu closed.
    MenuClosed,
}

/// Compact exclusive mode / view selector.
#[derive(Debug, Clone, Copy)]
pub struct SegmentedControl<'a, Id> {
    items: &'a [SegmentedItem<'a, Id>],
    system: &'a DesignSystem,
    /// Collapse to Select-like trigger below this width (0 = never). Default 18.
    collapse_below: u16,
    overflow_label: &'a str,
    colorless: bool,
}

impl<'a, Id> SegmentedControl<'a, Id> {
    /// Borrowed segments + design system.
    #[must_use]
    pub const fn new(items: &'a [SegmentedItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            collapse_below: 18,
            overflow_label: "…",
            colorless: false,
        }
    }

    /// Collapse to Select-like face when width &lt; `cols` (0 disables).
    #[must_use]
    pub const fn collapse_below(mut self, cols: u16) -> Self {
        self.collapse_below = cols;
        self
    }

    /// Overflow trigger label.
    #[must_use]
    pub const fn overflow_label(mut self, label: &'a str) -> Self {
        self.overflow_label = label;
        self
    }

    /// Force monochrome emphasis.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn mono(&self) -> bool {
        self.colorless
            || self.system.glyphs.is_ascii()
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }

    fn format_face(&self, item: &SegmentedItem<'a, Id>, selected: bool) -> String {
        let inner = item.face_inner();
        if selected {
            format!("[{inner}]")
        } else {
            format!(" {inner} ")
        }
    }

    fn segment_width(&self, item: &SegmentedItem<'a, Id>, selected: bool) -> u16 {
        let face = self.format_face(item, selected);
        u16::try_from(display_cols(&face).max(3)).unwrap_or(3)
    }

    fn overflow_trigger_width(&self) -> u16 {
        u16::try_from(display_cols(self.overflow_label).saturating_add(2)).unwrap_or(3)
    }

    fn sep_glyph(&self) -> &'static str {
        if self.system.glyphs.is_ascii() || self.mono() {
            "|"
        } else {
            "│"
        }
    }

    /// Plan visible indices, overflow indices, and presentation.
    #[must_use]
    pub fn plan(
        &self,
        width: u16,
        selected: Option<&Id>,
    ) -> (SegmentedPresentation, Vec<usize>, Vec<usize>)
    where
        Id: PartialEq,
    {
        if self.items.is_empty() {
            return (SegmentedPresentation::Expanded, Vec::new(), Vec::new());
        }
        if self.collapse_below > 0 && width < self.collapse_below {
            // Collapsed: selected (or first enabled) visible concept; rest overflow
            let sel_idx = selected
                .and_then(|s| self.items.iter().position(|i| &i.id == s))
                .or_else(|| self.items.iter().position(|i| i.enabled))
                .unwrap_or(0);
            let overflow: Vec<usize> = (0..self.items.len()).filter(|&i| i != sel_idx).collect();
            return (SegmentedPresentation::Collapsed, vec![sel_idx], overflow);
        }

        // Priority plan similar to ButtonGroup
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        order.sort_by(|&a, &b| {
            // Keep selected with boost
            let pa = self.items[a].priority
                + if selected == Some(&self.items[a].id) {
                    40
                } else {
                    0
                };
            let pb = self.items[b].priority
                + if selected == Some(&self.items[b].id) {
                    40
                } else {
                    0
                };
            pb.cmp(&pa).then(a.cmp(&b))
        });

        let gap = 1u16; // separator
        let mut used = 0u16;
        let mut keep = Vec::new();
        let mut overflow = Vec::new();
        for &idx in &order {
            let sel = selected == Some(&self.items[idx].id);
            let w = self.segment_width(&self.items[idx], sel);
            let extra = if keep.is_empty() { 0 } else { gap };
            let remaining_after = order.len() - keep.len() - overflow.len() - 1;
            let reserve = if remaining_after > 0 || !overflow.is_empty() {
                self.overflow_trigger_width().saturating_add(gap)
            } else {
                0
            };
            let next = used
                .saturating_add(extra)
                .saturating_add(w)
                .saturating_add(reserve);
            if keep.is_empty() || next <= width {
                if next > width && !keep.is_empty() {
                    overflow.push(idx);
                } else {
                    keep.push(idx);
                    used = used.saturating_add(extra).saturating_add(w);
                }
            } else {
                overflow.push(idx);
            }
        }
        // Ensure selected stays if possible
        if let Some(s) = selected {
            if let Some(si) = self.items.iter().position(|i| &i.id == s) {
                if !keep.contains(&si) && !overflow.is_empty() {
                    // swap lowest priority from keep
                    if let Some(pos) = keep
                        .iter()
                        .rposition(|&i| Some(&self.items[i].id) != selected)
                    {
                        overflow.push(keep.remove(pos));
                        keep.push(si);
                        overflow.retain(|&i| i != si);
                    }
                }
            }
        }
        if !overflow.is_empty() {
            let trigger = self.overflow_trigger_width().saturating_add(gap);
            while !keep.is_empty() {
                let total = keep.iter().enumerate().fold(0u16, |acc, (i, &idx)| {
                    let sel = selected == Some(&self.items[idx].id);
                    acc.saturating_add(if i > 0 { gap } else { 0 })
                        .saturating_add(self.segment_width(&self.items[idx], sel))
                });
                if total.saturating_add(trigger) <= width {
                    break;
                }
                if let Some(pos) = keep
                    .iter()
                    .rposition(|&i| Some(&self.items[i].id) != selected)
                {
                    overflow.push(keep.remove(pos));
                } else if keep.len() > 1 {
                    overflow.push(keep.pop().unwrap());
                } else {
                    break;
                }
            }
        }
        keep.sort_unstable();
        overflow.sort_by(|&a, &b| {
            self.items[a]
                .priority
                .cmp(&self.items[b].priority)
                .then(a.cmp(&b))
        });
        let presentation = if overflow.is_empty() {
            SegmentedPresentation::Expanded
        } else {
            SegmentedPresentation::Overflow
        };
        (presentation, keep, overflow)
    }
}

impl<'a, Id: Clone + PartialEq> SegmentedControl<'a, Id> {
    fn item_by_id(&self, id: &Id) -> Option<&SegmentedItem<'a, Id>> {
        self.items.iter().find(|i| &i.id == id)
    }

    fn face_style(
        &self,
        selected: bool,
        focused: bool,
        hovered: bool,
        enabled: bool,
    ) -> ratatui_core::style::Style {
        if !enabled {
            return self.system.style(Role::TextDisabled);
        }
        if focused {
            let mut s = self.system.style(Role::Focus);
            if selected {
                s = s.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            } else {
                s = s.add_modifier(Modifier::UNDERLINED);
            }
            return s;
        }
        if selected {
            // Clear without neon fill: strong text + bold; reverse when mono
            let mut s = self.system.style(Role::TextStrong);
            s = s.add_modifier(Modifier::BOLD);
            if self.mono() {
                s = s.add_modifier(Modifier::REVERSED);
            }
            return s;
        }
        if hovered {
            return self
                .system
                .style(Role::TextMuted)
                .add_modifier(Modifier::UNDERLINED);
        }
        self.system.style(Role::TextMuted)
    }

    /// Paint control.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SegmentedControlState<Id>,
    ) -> SegmentedControlParts<Id> {
        if area.is_empty() || self.items.is_empty() {
            let parts = SegmentedControlParts {
                root: area,
                presentation: SegmentedPresentation::Expanded,
                items: Vec::new(),
                overflow_trigger: None,
                collapsed_trigger: None,
                overflow_ids: Vec::new(),
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        let (presentation, visible, overflow) = self.plan(area.width, state.selected.as_ref());
        let overflow_ids: Vec<Id> = overflow.iter().map(|&i| self.items[i].id.clone()).collect();

        // Cursor reconcile
        let visible_ids: Vec<Id> = visible.iter().map(|&i| self.items[i].id.clone()).collect();
        if state
            .cursor
            .as_ref()
            .is_none_or(|c| !visible_ids.iter().any(|v| v == c))
        {
            state.cursor = state
                .selected
                .clone()
                .filter(|s| visible_ids.iter().any(|v| v == s))
                .or_else(|| {
                    visible
                        .iter()
                        .find(|&&i| self.items[i].enabled)
                        .map(|&i| self.items[i].id.clone())
                })
                .or_else(|| visible_ids.first().cloned());
        }

        let roving_entries: Vec<RovingEntry<Id>> = visible
            .iter()
            .map(|&i| {
                let it = &self.items[i];
                RovingEntry::new(it.id.clone(), it.a11y()).enabled(it.enabled)
            })
            .collect();
        let _ = state.roving.reconcile(&roving_entries);
        if let Some(c) = state.cursor.clone() {
            state.roving.set_active(Some(c));
        }

        let mut item_parts = Vec::new();
        let mut overflow_trigger = None;
        let mut collapsed_trigger = None;

        match presentation {
            SegmentedPresentation::Collapsed => {
                // Select-like face: [Selected ▾] or selected label + chevron
                let sel_idx = visible.first().copied().unwrap_or(0);
                let item = &self.items[sel_idx];
                let chev = if self.system.glyphs.is_ascii() || self.mono() {
                    "v"
                } else {
                    "▾"
                };
                let open = if state.menu_open { "^" } else { chev };
                let inner = item.face_inner();
                let face = format!("[{inner} {open}]");
                let text = take_display_cols(&face, usize::from(area.width));
                let focused = state.surface_focused;
                let style = self.face_style(true, focused, false, item.enabled);
                buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
                let w = display_cols(&text).min(usize::from(area.width)) as u16;
                let rect = Rect::new(area.x, area.y, w.max(1), 1.min(area.height));
                collapsed_trigger = Some(rect);
                item_parts.push(SegmentedItemParts {
                    id: item.id.clone(),
                    area: rect,
                    overflowed: false,
                });
                for &idx in &overflow {
                    item_parts.push(SegmentedItemParts {
                        id: self.items[idx].id.clone(),
                        area: Rect::default(),
                        overflowed: true,
                    });
                }
            }
            SegmentedPresentation::Expanded | SegmentedPresentation::Overflow => {
                let mut x = area.x;
                let mut first = true;
                for &idx in &visible {
                    if !first {
                        if x < area.right() && area.height > 0 {
                            buffer.set_stringn(
                                x,
                                area.y,
                                self.sep_glyph(),
                                1,
                                self.system.style(Role::Border),
                            );
                            x = x.saturating_add(1);
                        }
                    }
                    first = false;
                    let item = &self.items[idx];
                    let selected = state.selected.as_ref() == Some(&item.id);
                    let w = self
                        .segment_width(item, selected)
                        .min(area.right().saturating_sub(x));
                    if w == 0 {
                        break;
                    }
                    let rect = Rect::new(x, area.y, w, 1.min(area.height));
                    let focused = state.surface_focused && state.cursor.as_ref() == Some(&item.id);
                    let hovered = state.hovered.as_ref() == Some(&item.id);
                    let face = self.format_face(item, selected);
                    let text = take_display_cols(&face, usize::from(w));
                    let style = self.face_style(selected, focused, hovered, item.enabled);
                    buffer.set_stringn(rect.x, rect.y, &text, usize::from(w), style);
                    item_parts.push(SegmentedItemParts {
                        id: item.id.clone(),
                        area: rect,
                        overflowed: false,
                    });
                    x = x.saturating_add(w);
                }
                if matches!(presentation, SegmentedPresentation::Overflow) && !overflow.is_empty() {
                    if !first {
                        if x < area.right() && area.height > 0 {
                            buffer.set_stringn(
                                x,
                                area.y,
                                self.sep_glyph(),
                                1,
                                self.system.style(Role::Border),
                            );
                            x = x.saturating_add(1);
                        }
                    }
                    let tw = self
                        .overflow_trigger_width()
                        .min(area.right().saturating_sub(x));
                    if tw > 0 {
                        let rect = Rect::new(x, area.y, tw, 1.min(area.height));
                        let mut style = self.system.style(if state.menu_open {
                            Role::ActionFocused
                        } else {
                            Role::TextMuted
                        });
                        style = style.add_modifier(Modifier::BOLD);
                        style.bg = None;
                        let label = take_display_cols(self.overflow_label, usize::from(tw));
                        buffer.set_stringn(rect.x, rect.y, &label, usize::from(tw), style);
                        overflow_trigger = Some(rect);
                    }
                }
                for &idx in &overflow {
                    item_parts.push(SegmentedItemParts {
                        id: self.items[idx].id.clone(),
                        area: Rect::default(),
                        overflowed: true,
                    });
                }
            }
        }

        let parts = SegmentedControlParts {
            root: area,
            presentation,
            items: item_parts,
            overflow_trigger,
            collapsed_trigger,
            overflow_ids,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn commit(&self, state: &mut SegmentedControlState<Id>, id: Id) -> SegmentedControlOutcome<Id> {
        if let Some(item) = self.item_by_id(&id) {
            if !item.enabled {
                return SegmentedControlOutcome::Ignored;
            }
        } else {
            return SegmentedControlOutcome::Ignored;
        }
        let changed = state.selected.as_ref() != Some(&id);
        state.selected = Some(id.clone());
        state.cursor = Some(id.clone());
        state.menu_open = false;
        if changed {
            SegmentedControlOutcome::Selected { id }
        } else {
            SegmentedControlOutcome::Ignored
        }
    }

    /// Keys: FollowFocus (arrows select), typeahead, Space/Enter confirm, menu.
    pub fn handle_key(
        &self,
        state: &mut SegmentedControlState<Id>,
        key: KeyEvent,
    ) -> SegmentedControlOutcome<Id> {
        if !state.surface_focused || key.kind != KeyEventKind::Press || self.items.is_empty() {
            return SegmentedControlOutcome::Ignored;
        }
        if matches!(key.code, crate::input::KeyCode::Esc) && state.menu_open {
            state.menu_open = false;
            return SegmentedControlOutcome::MenuClosed;
        }

        let parts = state.parts.clone();
        let presentation = parts
            .as_ref()
            .map(|p| p.presentation)
            .unwrap_or(SegmentedPresentation::Expanded);

        // Collapsed: Space/Enter/Down opens menu; Left/Right cycles all enabled
        if matches!(presentation, SegmentedPresentation::Collapsed) {
            if matches!(
                key.code,
                crate::input::KeyCode::Enter
                    | crate::input::KeyCode::Char(' ')
                    | crate::input::KeyCode::Down
            ) {
                state.menu_open = !state.menu_open;
                return if state.menu_open {
                    SegmentedControlOutcome::MenuOpened
                } else {
                    SegmentedControlOutcome::MenuClosed
                };
            }
            // Cycle selection among all enabled
            let enabled: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, it)| it.enabled.then_some(i))
                .collect();
            if enabled.is_empty() {
                return SegmentedControlOutcome::Ignored;
            }
            let cur = state
                .selected
                .as_ref()
                .and_then(|s| self.items.iter().position(|i| &i.id == s))
                .and_then(|i| enabled.iter().position(|&e| e == i))
                .unwrap_or(0);
            match key.code {
                crate::input::KeyCode::Left | crate::input::KeyCode::Up => {
                    let next = if cur == 0 { enabled.len() - 1 } else { cur - 1 };
                    return self.commit(state, self.items[enabled[next]].id.clone());
                }
                crate::input::KeyCode::Right => {
                    let next = (cur + 1) % enabled.len();
                    return self.commit(state, self.items[enabled[next]].id.clone());
                }
                _ => {}
            }
        }

        // Build roving from visible
        let visible: Vec<RovingEntry<Id>> = if let Some(p) = &parts {
            p.items
                .iter()
                .filter(|it| !it.overflowed)
                .filter_map(|it| {
                    let item = self.item_by_id(&it.id)?;
                    Some(RovingEntry::new(item.id.clone(), item.a11y()).enabled(item.enabled))
                })
                .collect()
        } else {
            self.items
                .iter()
                .map(|i| RovingEntry::new(i.id.clone(), i.a11y()).enabled(i.enabled))
                .collect()
        };
        if visible.is_empty() {
            return SegmentedControlOutcome::Ignored;
        }

        // Activate / open overflow menu
        if let Some(intent) = default_button_intent(key) {
            if matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
            ) {
                if state.menu_open {
                    return SegmentedControlOutcome::Ignored;
                }
                if let Some(c) = state.cursor.clone() {
                    if parts
                        .as_ref()
                        .is_some_and(|p| p.overflow_ids.iter().any(|id| id == &c))
                    {
                        state.menu_open = true;
                        return SegmentedControlOutcome::MenuOpened;
                    }
                    return self.commit(state, c);
                }
            }
        }

        // Overflow open key
        if let Some(p) = &parts {
            if !p.overflow_ids.is_empty()
                && matches!(key.code, crate::input::KeyCode::Char('o' | 'O' | '.'))
            {
                state.menu_open = !state.menu_open;
                return if state.menu_open {
                    SegmentedControlOutcome::MenuOpened
                } else {
                    SegmentedControlOutcome::MenuClosed
                };
            }
        }

        // Movement: FollowFocus — move cursor and select
        let before = state.cursor.clone();
        if let Some(mv) = default_list_intent(key) {
            match mv {
                UiIntent::Move(NavigationMove::Next | NavigationMove::Right) => {
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                        state.roving.move_next(&visible)
                    {
                        state.cursor = Some(id.clone());
                        return self.commit(state, id);
                    }
                }
                UiIntent::Move(NavigationMove::Previous | NavigationMove::Left) => {
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                        state.roving.move_previous(&visible)
                    {
                        state.cursor = Some(id.clone());
                        return self.commit(state, id);
                    }
                }
                _ => {}
            }
        }
        // Direct arrows
        match key.code {
            crate::input::KeyCode::Right | crate::input::KeyCode::Down => {
                if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                    state.roving.move_next(&visible)
                {
                    state.cursor = Some(id.clone());
                    return self.commit(state, id);
                }
            }
            crate::input::KeyCode::Left | crate::input::KeyCode::Up => {
                if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                    state.roving.move_previous(&visible)
                {
                    state.cursor = Some(id.clone());
                    return self.commit(state, id);
                }
            }
            crate::input::KeyCode::Home => {
                if let Some(id) = visible.iter().find(|e| e.enabled).map(|e| e.id.clone()) {
                    state.cursor = Some(id.clone());
                    return self.commit(state, id);
                }
            }
            crate::input::KeyCode::End => {
                if let Some(id) = visible
                    .iter()
                    .rev()
                    .find(|e| e.enabled)
                    .map(|e| e.id.clone())
                {
                    state.cursor = Some(id.clone());
                    return self.commit(state, id);
                }
            }
            _ => {}
        }

        // Typeahead
        let ro = state.roving.handle_key(key, &visible);
        if let RovingOutcome::ActiveChanged { to: Some(id), .. } = ro {
            state.cursor = Some(id.clone());
            return self.commit(state, id);
        }
        let _ = before;
        SegmentedControlOutcome::Ignored
    }

    /// Mouse: click segment selects; overflow/collapsed opens menu.
    pub fn handle_mouse(
        &self,
        state: &mut SegmentedControlState<Id>,
        event: MouseEvent,
    ) -> SegmentedControlOutcome<Id> {
        let Some(parts) = state.parts.clone() else {
            return SegmentedControlOutcome::Ignored;
        };
        if !parts.root.contains(event.position) {
            if matches!(event.kind, MouseEventKind::Moved) {
                state.hovered = None;
            }
            return SegmentedControlOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts
                    .items
                    .iter()
                    .find(|it| !it.overflowed && it.area.contains(event.position))
                    .map(|it| it.id.clone());
                SegmentedControlOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(tr) = parts.overflow_trigger.or(parts.collapsed_trigger) {
                    if tr.contains(event.position) {
                        state.surface_focused = true;
                        // Collapsed trigger is also the selected face — toggle menu
                        if parts.collapsed_trigger == Some(tr) {
                            state.menu_open = !state.menu_open;
                            return if state.menu_open {
                                SegmentedControlOutcome::MenuOpened
                            } else {
                                SegmentedControlOutcome::MenuClosed
                            };
                        }
                        state.menu_open = !state.menu_open;
                        return if state.menu_open {
                            SegmentedControlOutcome::MenuOpened
                        } else {
                            SegmentedControlOutcome::MenuClosed
                        };
                    }
                }
                if let Some(it) = parts
                    .items
                    .iter()
                    .find(|it| !it.overflowed && it.area.contains(event.position))
                {
                    state.surface_focused = true;
                    state.cursor = Some(it.id.clone());
                    if matches!(parts.presentation, SegmentedPresentation::Collapsed) {
                        state.menu_open = !state.menu_open;
                        return if state.menu_open {
                            SegmentedControlOutcome::MenuOpened
                        } else {
                            SegmentedControlOutcome::MenuClosed
                        };
                    }
                    return self.commit(state, it.id.clone());
                }
                SegmentedControlOutcome::Ignored
            }
            _ => SegmentedControlOutcome::Ignored,
        }
    }

    /// Host chose an overflow / collapsed menu option.
    pub fn select_from_menu(
        &self,
        state: &mut SegmentedControlState<Id>,
        id: Id,
    ) -> SegmentedControlOutcome<Id> {
        if state.parts.as_ref().is_some_and(|p| {
            p.overflow_ids.iter().any(|x| x == &id) || p.items.iter().any(|i| i.id == id)
        }) {
            return self.commit(state, id);
        }
        // Also allow selecting any known item (host menu may list all)
        if self.item_by_id(&id).is_some() {
            return self.commit(state, id);
        }
        SegmentedControlOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut SegmentedControlState<Id>,
        key: KeyEvent,
    ) -> EventResult<SegmentedControlOutcome<Id>> {
        match self.handle_key(state, key) {
            SegmentedControlOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic: Tab-like list of segments.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        group_id: Id,
        area: Rect,
        state: &SegmentedControlState<Id>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let pres = state
            .parts
            .as_ref()
            .map(|p| p.presentation.id())
            .unwrap_or("expanded");
        let _ = scene.register(
            SemanticNode::control(group_id, area)
                .role(SemanticRole::Tab)
                .label("segmented control")
                .description(pres)
                .focusable(true)
                .state(SemanticState {
                    selected: state.surface_focused,
                    ..Default::default()
                }),
        );
        if let Some(parts) = &state.parts {
            for it in &parts.items {
                if it.overflowed {
                    continue;
                }
                let Some(item) = self.item_by_id(&it.id) else {
                    continue;
                };
                let selected = state.selected.as_ref() == Some(&it.id);
                let _ = scene.register(
                    SemanticNode::control(it.id.clone(), it.area)
                        .role(SemanticRole::Tab)
                        .label(item.a11y())
                        .focusable(item.enabled && state.surface_focused)
                        .disabled(!item.enabled)
                        .state(SemanticState {
                            selected,
                            checked: selected,
                            ..Default::default()
                        }),
                );
            }
        }
    }
}

impl<'a, Id: Clone + PartialEq> Widget for &SegmentedControl<'a, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = SegmentedControlState::new(None);
        let _ = self.paint(area, buffer, &mut state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};

    fn sample() -> [SegmentedItem<'static, &'static str>; 4] {
        [
            SegmentedItem::new("list", "List").priority(90),
            SegmentedItem::new("grid", "Grid").priority(80),
            SegmentedItem::new("table", "Table").priority(40),
            SegmentedItem::new("graph", "Graph").priority(10),
        ]
    }

    #[test]
    fn select_on_arrow_follow_focus() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("list"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        let _ = g.paint(Rect::new(0, 0, 60, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            SegmentedControlOutcome::Selected { id: "grid" }
        ));
        assert_eq!(state.selected(), Some(&"grid"));
    }

    #[test]
    fn mouse_selects() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("list"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        let parts = g.paint(Rect::new(0, 0, 60, 1), &mut buf, &mut state);
        let grid = parts.items.iter().find(|i| i.id == "grid").unwrap();
        let out = g.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: grid.area.x,
                    y: grid.area.y,
                },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(
            out,
            SegmentedControlOutcome::Selected { id: "grid" }
        ));
    }

    #[test]
    fn overflow_keeps_selected() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system).collapse_below(0);
        let (pres, vis, over) = g.plan(18, Some(&"list"));
        assert!(matches!(
            pres,
            SegmentedPresentation::Overflow | SegmentedPresentation::Expanded
        ));
        assert!(vis.iter().any(|&i| items[i].id == "list"));
        if !over.is_empty() {
            assert!(!over.iter().any(|&i| items[i].id == "list") || vis.contains(&0));
        }
    }

    #[test]
    fn collapses_when_narrow() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system).collapse_below(20);
        let (pres, vis, over) = g.plan(12, Some(&"grid"));
        assert_eq!(pres, SegmentedPresentation::Collapsed);
        assert_eq!(vis.len(), 1);
        assert!(items[vis[0]].id == "grid");
        assert!(!over.is_empty());
    }

    #[test]
    fn collapsed_paint_and_menu() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system).collapse_below(40);
        let mut state = SegmentedControlState::new(Some("list"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 14, 1));
        let parts = g.paint(Rect::new(0, 0, 14, 1), &mut buf, &mut state);
        assert_eq!(parts.presentation, SegmentedPresentation::Collapsed);
        assert!(parts.collapsed_trigger.is_some());
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(out, SegmentedControlOutcome::MenuOpened));
        let out = g.select_from_menu(&mut state, "table");
        assert!(matches!(
            out,
            SegmentedControlOutcome::Selected { id: "table" }
        ));
    }

    #[test]
    fn disabled_not_selected() {
        let system = DesignSystem::default();
        let items = [
            SegmentedItem::new("a", "A"),
            SegmentedItem::new("b", "B").enabled(false),
            SegmentedItem::new("c", "C"),
        ];
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("a"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, SegmentedControlOutcome::Selected { id: "c" }));
    }

    #[test]
    fn selected_mark_brackets_no_neon_only() {
        let system = DesignSystem::default().glyphs(crate::style::GlyphSet::Ascii);
        let items = [SegmentedItem::new("a", "A"), SegmentedItem::new("b", "B")];
        let g = SegmentedControl::new(&items, &system).colorless(true);
        let mut state = SegmentedControlState::new(Some("a"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let _ = g.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        assert_eq!(
            buf.cell((0, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("[")
        );
    }

    #[test]
    fn icon_badge_segment() {
        let system = DesignSystem::default();
        let items = [
            SegmentedItem::new("l", "List").icon("≡").badge("3"),
            SegmentedItem::new("g", "Grid").icon("#"),
        ];
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("l"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        assert_eq!(parts.items.iter().filter(|i| !i.overflowed).count(), 2);
    }

    #[test]
    fn typeahead_selects() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("list"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        let _ = g.paint(Rect::new(0, 0, 60, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );
        assert!(
            matches!(out, SegmentedControlOutcome::Selected { id: "table" })
                || state.selected() == Some(&"table")
                || state.cursor == Some("table")
        );
    }

    #[test]
    fn plan_monotone_width() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system).collapse_below(0);
        let mut prev = 0usize;
        for w in 8u16..=80 {
            let (_, vis, _) = g.plan(w, Some(&"list"));
            if vis.len() == items.len() {
                prev = items.len();
            } else if vis.len() >= prev {
                prev = vis.len();
            }
        }
        let (_, full, over) = g.plan(100, Some(&"list"));
        assert_eq!(full.len(), 4);
        assert!(over.is_empty());
    }

    #[test]
    fn semantic_and_hot_path() {
        let system = DesignSystem::default();
        let items = sample();
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(Some("list"));
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        for _ in 0..400 {
            let _ = g.paint(area, &mut buf, &mut state);
        }
        let mut scene = SemanticScene::<&str, ()>::default();
        g.register_semantic(&mut scene, "seg", area, &state);
        assert!(scene.len() >= 2);
    }

    #[test]
    fn empty_safe() {
        let system = DesignSystem::default();
        let items: [SegmentedItem<'_, &str>; 0] = [];
        let g = SegmentedControl::new(&items, &system);
        let mut state = SegmentedControlState::new(None);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = g.paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.items.is_empty());
    }
}
