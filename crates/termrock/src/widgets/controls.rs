// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Controlled form controls: checkbox, radio, switch, select, multiselect, combobox (Plan 051).
//!
//! [`Checkbox`] is the form-field boolean/tri-state control (label + description).
//! Prefer [`crate::widgets::Toggle`] for sticky toolbar tools and
//! [`Switch`] for settings On/Off.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        EventResult, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
        default_button_intent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

// ── Checkbox ────────────────────────────────────────────────────────────────

/// Checked value for a [`Checkbox`] (binary or tri-state).
///
/// **Indeterminate** is for mixed groups (select-all when children disagree) and
/// partial selection in lists/tables. Activation cycles
/// Indeterminate/Unchecked → Checked → Unchecked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CheckboxValue {
    /// Off.
    #[default]
    Unchecked,
    /// On.
    Checked,
    /// Mixed / partial (group or multi-select summary).
    Indeterminate,
}

impl CheckboxValue {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::Checked => "checked",
            Self::Indeterminate => "indeterminate",
        }
    }

    /// Fully checked.
    #[must_use]
    pub const fn is_checked(self) -> bool {
        matches!(self, Self::Checked)
    }

    /// Mixed.
    #[must_use]
    pub const fn is_indeterminate(self) -> bool {
        matches!(self, Self::Indeterminate)
    }

    /// From bool (no indeterminate).
    #[must_use]
    pub const fn from_bool(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }

    /// `Some(true/false)` when determinate; `None` when indeterminate.
    #[must_use]
    pub const fn as_bool(self) -> Option<bool> {
        match self {
            Self::Checked => Some(true),
            Self::Unchecked => Some(false),
            Self::Indeterminate => None,
        }
    }

    /// Next value after Space/Activate (Radix-like: mixed/off → on, on → off).
    #[must_use]
    pub const fn activate(self) -> Self {
        match self {
            Self::Unchecked | Self::Indeterminate => Self::Checked,
            Self::Checked => Self::Unchecked,
        }
    }

    /// Aggregate child bools for a mixed-group parent.
    #[must_use]
    pub fn from_children(children: impl IntoIterator<Item = bool>) -> Self {
        let mut any = false;
        let mut all = true;
        let mut saw = false;
        for c in children {
            saw = true;
            any |= c;
            all &= c;
        }
        if !saw {
            return Self::Unchecked;
        }
        if all {
            Self::Checked
        } else if any {
            Self::Indeterminate
        } else {
            Self::Unchecked
        }
    }
}

/// Checkbox outcome (controlled: consumer applies value).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckboxOutcome<Id> {
    /// No change.
    Ignored,
    /// Host should set new value for `id`.
    ValueChanged {
        /// Field id.
        id: Id,
        /// Next value.
        value: CheckboxValue,
    },
}

impl<Id> CheckboxOutcome<Id> {
    /// Convenience: checked flag when determinate change.
    #[must_use]
    pub const fn checked(&self) -> Option<bool> {
        match self {
            Self::ValueChanged { value, .. } => value.as_bool(),
            Self::Ignored => None,
        }
    }
}

/// Paint geometry for a checkbox.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckboxParts {
    /// Full interactive root (box + label + optional description).
    pub root: Rect,
    /// Box / mark only.
    pub box_area: Rect,
    /// Label row.
    pub label_area: Rect,
    /// Description row when painted.
    pub description_area: Option<Rect>,
}

/// Checkbox state (interaction + projected value).
///
/// Domain persistence is host-owned: apply [`CheckboxOutcome::ValueChanged`] to
/// your model, then [`Self::set_value`]. Paint may optimistically mirror for UX.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckboxState {
    /// Projected value.
    pub value: CheckboxValue,
    /// Keyboard focus.
    pub focused: bool,
    /// Enabled (toggleable when not read-only).
    pub enabled: bool,
    /// Read-only: show value, ignore activate.
    pub read_only: bool,
    /// Validation failed (form integration).
    pub invalid: bool,
    /// Pointer hover.
    pub hovered: bool,
    /// Last paint geometry.
    pub parts: Option<CheckboxParts>,
    /// Hit root (alias of parts.root for mouse).
    region: Option<Rect>,
}

impl CheckboxState {
    /// Binary initial value.
    #[must_use]
    pub const fn new(checked: bool) -> Self {
        Self {
            value: CheckboxValue::from_bool(checked),
            focused: false,
            enabled: true,
            read_only: false,
            invalid: false,
            hovered: false,
            parts: None,
            region: None,
        }
    }

    /// Explicit value (incl. indeterminate).
    #[must_use]
    pub const fn with_value(value: CheckboxValue) -> Self {
        Self {
            value,
            focused: false,
            enabled: true,
            read_only: false,
            invalid: false,
            hovered: false,
            parts: None,
            region: None,
        }
    }

    /// Fully checked.
    #[must_use]
    pub const fn is_checked(&self) -> bool {
        self.value.is_checked()
    }

    /// Current value.
    #[must_use]
    pub const fn value(&self) -> CheckboxValue {
        self.value
    }

    /// Controlled set (bool).
    pub const fn set_checked(&mut self, checked: bool) {
        self.value = CheckboxValue::from_bool(checked);
    }

    /// Controlled set (tri-state).
    pub const fn set_value(&mut self, value: CheckboxValue) {
        self.value = value;
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Read-only.
    pub const fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
    }

    /// Invalid / error chrome.
    pub const fn set_invalid(&mut self, on: bool) {
        self.invalid = on;
    }

    /// Whether activate is allowed.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.read_only
    }

    /// Hit root.
    #[must_use]
    pub const fn region(&self) -> Option<Rect> {
        self.region
    }

    fn apply_activate<Id: Clone>(&mut self, id: &Id) -> CheckboxOutcome<Id> {
        if !self.can_activate() {
            return CheckboxOutcome::Ignored;
        }
        let next = self.value.activate();
        self.value = next;
        CheckboxOutcome::ValueChanged {
            id: id.clone(),
            value: next,
        }
    }

    /// Toggle when focused and activatable.
    pub fn handle_key<Id: Clone>(&mut self, key: KeyEvent, id: &Id) -> CheckboxOutcome<Id> {
        if !self.can_activate() || !self.focused || key.kind != KeyEventKind::Press {
            return CheckboxOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            if matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
            ) {
                return self.apply_activate(id);
            }
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.apply_activate(id),
            _ => CheckboxOutcome::Ignored,
        }
    }

    /// Click on root toggles.
    pub fn handle_mouse<Id: Clone>(&mut self, event: MouseEvent, id: &Id) -> CheckboxOutcome<Id> {
        if !self.can_activate() {
            return CheckboxOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                self.hovered = self.region.is_some_and(|r| r.contains(event.position));
                CheckboxOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.region.is_some_and(|r| r.contains(event.position)) {
                    self.focused = true;
                    self.apply_activate(id)
                } else {
                    CheckboxOutcome::Ignored
                }
            }
            _ => CheckboxOutcome::Ignored,
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result<Id: Clone>(
        &mut self,
        key: KeyEvent,
        id: &Id,
    ) -> EventResult<CheckboxOutcome<Id>> {
        match self.handle_key(key, id) {
            CheckboxOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }
}

/// Form-field checkbox: box + label + optional description.
///
/// **vs Toggle.** Toggle is a sticky toolbar tool (`[B]`). Checkbox is a form
/// field with label association and tri-state mixed groups.
///
/// **vs Switch.** Switch is settings On/Off (`[ON ]`/`[OFF]`).
#[derive(Debug, Clone, Copy)]
pub struct Checkbox<'a, Id> {
    /// Stable identity (outcomes + semantics).
    pub id: Id,
    label: &'a str,
    description: Option<&'a str>,
    system: &'a DesignSystem,
    /// Force monochrome / no-color emphasis.
    colorless: bool,
}

impl<'a, Id> Checkbox<'a, Id> {
    /// Id + label + design system.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            description: None,
            system,
            colorless: false,
        }
    }

    /// Secondary description line (dropped when height < 2 or narrow).
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Colorless emphasis (brackets always from glyph ASCII path when mono).
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Preferred height: 2 when description present, else 1.
    #[must_use]
    pub fn preferred_height(&self) -> u16 {
        if self.description.is_some_and(|d| !d.is_empty()) {
            2
        } else {
            1
        }
    }

    /// Preferred width for box + gap + label (description not included).
    #[must_use]
    pub fn preferred_width(&self, state: &CheckboxState) -> u16 {
        let mark = self.box_mark(state.value);
        let gap = 1u16;
        let label_w = display_cols(self.label) as u16;
        let mark_w = display_cols(mark) as u16;
        mark_w.saturating_add(gap).saturating_add(label_w).max(3)
    }

    fn mono(&self) -> bool {
        self.colorless
            || self.system.glyphs.is_ascii()
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }

    fn box_mark(&self, value: CheckboxValue) -> &'static str {
        // Prefer catalog; force ASCII bracket forms when mono for no-color clarity.
        if self.mono() {
            return match value {
                CheckboxValue::Checked => "[x]",
                CheckboxValue::Unchecked => "[ ]",
                CheckboxValue::Indeterminate => "[-]",
            };
        }
        match value {
            CheckboxValue::Checked => self.system.glyphs.check_on(),
            CheckboxValue::Unchecked => self.system.glyphs.check_off(),
            CheckboxValue::Indeterminate => self.system.glyphs.check_mixed(),
        }
    }

    fn mark_style(&self, state: &CheckboxState) -> ratatui_core::style::Style {
        if !state.enabled {
            return self.system.style(Role::TextDisabled);
        }
        if state.read_only {
            return self.system.style(Role::TextMuted);
        }
        if state.invalid {
            let mut s = self.system.style(Role::Danger);
            if state.focused {
                s = s.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
            }
            return s;
        }
        if state.focused {
            let mut s = self.system.style(Role::Focus);
            if state.value.is_checked() {
                s = s.add_modifier(Modifier::BOLD);
            }
            return s;
        }
        match state.value {
            CheckboxValue::Checked => {
                let mut s = self.system.style(Role::TextStrong);
                s = s.add_modifier(Modifier::BOLD);
                s
            }
            CheckboxValue::Indeterminate => self.system.style(Role::TextMuted),
            CheckboxValue::Unchecked => self.system.style(Role::Text),
        }
    }

    fn label_style(&self, state: &CheckboxState) -> ratatui_core::style::Style {
        if !state.enabled {
            return self.system.style(Role::TextDisabled);
        }
        if state.read_only {
            return self.system.style(Role::TextMuted);
        }
        if state.invalid {
            return self.system.style(Role::Danger);
        }
        if state.focused {
            return self.system.style(Role::Focus).add_modifier(Modifier::UNDERLINED);
        }
        if state.hovered {
            return self.system.style(Role::Text).add_modifier(Modifier::UNDERLINED);
        }
        self.system.style(Role::Text)
    }

    /// Paint checkbox. Prefer this over [`StatefulWidget::render`].
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut CheckboxState) -> CheckboxParts {
        state.region = None;
        state.parts = None;
        if area.is_empty() {
            return CheckboxParts::default();
        }

        let mark = self.box_mark(state.value);
        let mark_w = display_cols(mark).min(usize::from(area.width)) as u16;
        let box_area = Rect::new(area.x, area.y, mark_w.max(1), 1.min(area.height));
        buffer.set_stringn(
            box_area.x,
            box_area.y,
            mark,
            usize::from(box_area.width),
            self.mark_style(state),
        );

        // Tiny: box only when width cannot hold gap+1 label col
        let after_box = area.x.saturating_add(mark_w);
        let label_x = after_box.saturating_add(1);
        let mut label_area = Rect::new(area.x, area.y, 0, 0);
        if label_x < area.right() && area.height > 0 && !self.label.is_empty() {
            let lw = area.right().saturating_sub(label_x);
            label_area = Rect::new(label_x, area.y, lw, 1);
            let text = take_display_cols(self.label, usize::from(lw));
            buffer.set_stringn(
                label_area.x,
                label_area.y,
                &text,
                usize::from(lw),
                self.label_style(state),
            );
            let used = display_cols(&text).min(usize::from(lw)) as u16;
            label_area.width = used;
        } else if self.label.is_empty() {
            label_area = box_area;
        }

        // Description: second row when available; drop when height < 2 or width < 12
        let mut description_area = None;
        if area.height >= 2
            && area.width >= 12
            && let Some(desc) = self.description
            && !desc.is_empty()
        {
            let dx = label_x.min(area.right().saturating_sub(1)).max(area.x);
            let dw = area.right().saturating_sub(dx);
            if dw > 0 {
                let drect = Rect::new(dx, area.y.saturating_add(1), dw, 1);
                let text = take_display_cols(desc, usize::from(dw));
                let mut style = self.system.style(if state.enabled {
                    Role::TextMuted
                } else {
                    Role::TextDisabled
                });
                if state.invalid {
                    style = self.system.style(Role::Danger);
                }
                buffer.set_stringn(drect.x, drect.y, &text, usize::from(dw), style);
                description_area = Some(drect);
            }
        }

        let root_w = area.width;
        let root_h = if description_area.is_some() {
            2.min(area.height)
        } else {
            1.min(area.height)
        };
        let root = Rect::new(area.x, area.y, root_w, root_h);
        // Hit prefers painted content width when single-line without full stretch
        let hit_w = if description_area.is_some() {
            root_w
        } else {
            let content = mark_w
                .saturating_add(if label_area.width > 0 { 1 } else { 0 })
                .saturating_add(label_area.width);
            content.max(mark_w).min(root_w)
        };
        let hit = Rect::new(area.x, area.y, hit_w.max(1), root_h);
        state.region = Some(hit);
        let parts = CheckboxParts {
            root: hit,
            box_area,
            label_area,
            description_area,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Paint + StatefulWidget path.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut CheckboxState) {
        let _ = self.paint(area, buffer, state);
    }

    /// Keys via widget (delegates to state with owned id).
    pub fn handle_key(&self, state: &mut CheckboxState, key: KeyEvent) -> CheckboxOutcome<Id>
    where
        Id: Clone,
    {
        state.handle_key(key, &self.id)
    }

    /// Mouse via widget.
    pub fn handle_mouse(&self, state: &mut CheckboxState, event: MouseEvent) -> CheckboxOutcome<Id>
    where
        Id: Clone,
    {
        state.handle_mouse(event, &self.id)
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut CheckboxState,
        key: KeyEvent,
    ) -> EventResult<CheckboxOutcome<Id>>
    where
        Id: Clone,
    {
        state.handle_key_result(key, &self.id)
    }

    /// Semantic: checkbox control with checked/mixed state.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        area: Rect,
        state: &CheckboxState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = self.description.unwrap_or("");
        let _ = scene.register(
            SemanticNode::control(self.id.clone(), area)
                .role(SemanticRole::Control)
                .label(self.label)
                .description(if desc.is_empty() {
                    state.value.id()
                } else {
                    desc
                })
                .focusable(state.can_activate())
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.value.is_checked(),
                    checked: state.value.is_checked(),
                    invalid: state.invalid,
                    pressed: state.focused && state.value.is_checked(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone> StatefulWidget for Checkbox<'_, Id> {
    type State = CheckboxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl<Id: Clone> StatefulWidget for &Checkbox<'_, Id> {
    type State = CheckboxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

// ── Radio ───────────────────────────────────────────────────────────────────

/// Radio change outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadioOutcome<Id> {
    /// No change.
    Ignored,
    /// Selected option id within group.
    Selected(Id),
}

/// Radio group state: selected value + collection active descendant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioState<Id> {
    selected: Option<Id>,
    collection: crate::interaction::CollectionState<Id>,
    enabled: bool,
    regions: Vec<Rect>,
}

impl<Id: Clone + PartialEq> RadioState<Id> {
    /// Empty selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        let mut collection = crate::interaction::CollectionState::new()
            .orientation(crate::interaction::RovingOrientation::Vertical);
        collection.set_active(selected.clone());
        Self {
            selected,
            collection,
            enabled: true,
            regions: Vec::new(),
        }
    }

    #[must_use]
    /// Selected id.
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Active descendant (cursor); may differ from selected until Activate.
    #[must_use]
    pub const fn active(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Controlled select (also moves collection active when `Some`).
    pub fn set_selected(&mut self, selected: Option<Id>) {
        self.selected = selected.clone();
        if selected.is_some() {
            self.collection.set_active(selected);
        }
    }

    fn entries_plain(options: &[Id]) -> Vec<crate::interaction::CollectionItem<Id>> {
        options
            .iter()
            .map(|id| crate::interaction::CollectionItem {
                id: id.clone(),
                enabled: true,
                label: String::new(),
                parent: None,
            })
            .collect()
    }

    /// Arrow/Home/End move + Space/Enter select.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> RadioOutcome<Id>
    where
        Id: Clone,
    {
        if !self.enabled || options.is_empty() || key.kind == KeyEventKind::Release {
            return RadioOutcome::Ignored;
        }
        let entries = Self::entries_plain(options);
        let _ = self.collection.reconcile(&entries);
        let is_press = key.kind == KeyEventKind::Press;
        if is_press
            && key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
        {
            if let Some(id) = self.collection.active().cloned() {
                self.selected = Some(id.clone());
                return RadioOutcome::Selected(id);
            }
            return RadioOutcome::Ignored;
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
            || (matches!(key.code, KeyCode::Tab)
                && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            let reverse = matches!(key.code, KeyCode::BackTab)
                || key.modifiers.contains(KeyModifiers::SHIFT);
            if reverse {
                let _ = self.collection.move_previous(&entries);
            } else {
                let _ = self.collection.move_next(&entries);
            }
            return RadioOutcome::Ignored;
        }
        let _ = self.collection.handle_key(key, &entries);
        RadioOutcome::Ignored
    }

    /// Intent path for collection move / activate.
    pub fn handle_intent(
        &mut self,
        intent: crate::interaction::UiIntent,
        options: &[Id],
    ) -> RadioOutcome<Id> {
        if !self.enabled || options.is_empty() {
            return RadioOutcome::Ignored;
        }
        let entries = Self::entries_plain(options);
        let _ = self.collection.reconcile(&entries);
        match intent {
            crate::interaction::UiIntent::Activate
            | crate::interaction::UiIntent::Submit
            | crate::interaction::UiIntent::Toggle => {
                if let Some(id) = self.collection.active().cloned() {
                    self.selected = Some(id.clone());
                    RadioOutcome::Selected(id)
                } else {
                    RadioOutcome::Ignored
                }
            }
            other => {
                let _ = self.collection.handle_intent(other, &entries);
                RadioOutcome::Ignored
            }
        }
    }
}

/// Radio group paint.
#[derive(Debug, Clone, Copy)]
pub struct RadioGroup<'a, Id> {
    options: &'a [(Id, &'a str)],
    tokens: &'a DesignSystem,
}

impl<'a, Id> RadioGroup<'a, Id> {
    /// Options as (id, label).
    #[must_use]
    pub const fn new(options: &'a [(Id, &'a str)], tokens: &'a DesignSystem) -> Self {
        Self { options, tokens }
    }

    /// Render vertical radio list.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut RadioState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.regions.clear();
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        for (id, label) in self.options.iter() {
            if y >= area.bottom() {
                break;
            }
            let selected = state.selected.as_ref() == Some(id);
            let focused = state.active() == Some(id);
            let mark = if selected { "(•)" } else { "( )" };
            let style = if focused {
                self.tokens.style(Role::Focus)
            } else {
                self.tokens.style(Role::Text)
            };
            let line = format!("{mark} {label}");
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            state
                .regions
                .push(Rect::new(area.x, y, area.width.min(20), 1));
            y = y.saturating_add(1);
        }
    }
}

// ── Switch ──────────────────────────────────────────────────────────────────

/// Switch outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwitchOutcome<Id> {
    /// No change.
    Ignored,
    /// On/off change.
    ValueChanged {
        /// Id.
        id: Id,
        /// On.
        on: bool,
    },
}

/// Switch state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SwitchState {
    on: bool,
    focused: bool,
    enabled: bool,
    region: Option<Rect>,
}

impl SwitchState {
    /// Initial.
    #[must_use]
    pub const fn new(on: bool) -> Self {
        Self {
            on,
            focused: false,
            enabled: true,
            region: None,
        }
    }

    #[must_use]
    /// On.
    pub const fn is_on(&self) -> bool {
        self.on
    }

    /// Controlled.
    pub const fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Toggle.
    pub fn handle_key<Id: Clone>(&mut self, key: KeyEvent, id: &Id) -> SwitchOutcome<Id> {
        if !self.enabled || !self.focused || key.kind != KeyEventKind::Press {
            return SwitchOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.on = !self.on;
                SwitchOutcome::ValueChanged {
                    id: id.clone(),
                    on: self.on,
                }
            }
            _ => SwitchOutcome::Ignored,
        }
    }

    /// Click.
    pub fn handle_mouse<Id: Clone>(&mut self, event: MouseEvent, id: &Id) -> SwitchOutcome<Id> {
        if !self.enabled || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return SwitchOutcome::Ignored;
        }
        if self.region.is_some_and(|r| r.contains(event.position)) {
            self.on = !self.on;
            SwitchOutcome::ValueChanged {
                id: id.clone(),
                on: self.on,
            }
        } else {
            SwitchOutcome::Ignored
        }
    }
}

/// Switch widget.
#[derive(Debug, Clone, Copy)]
pub struct Switch<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Label.
    label: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a, Id> Switch<'a, Id> {
    /// Id + label.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, tokens: &'a DesignSystem) -> Self {
        Self { id, label, tokens }
    }
}

impl<Id> Switch<'_, Id> {
    /// Paint switch.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut SwitchState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let knob = if state.on { "[ON ]" } else { "[OFF]" };
        let style = if !state.enabled {
            self.tokens.style(Role::TextDisabled)
        } else if state.focused {
            self.tokens.style(Role::Focus)
        } else if state.on {
            self.tokens.style(Role::Success)
        } else {
            self.tokens.style(Role::TextMuted)
        };
        let line = format!("{knob} {}", self.label);
        let text = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.region = Some(Rect::new(area.x, area.y, area.width.min(16), 1));
    }
}

// ── Select / MultiSelect / Combobox ─────────────────────────────────────────

/// Select outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectOutcome<Id> {
    /// No change.
    Ignored,
    /// Open menu requested.
    OpenMenu,
    /// Close menu.
    CloseMenu,
    /// Option chosen.
    Selected(Id),
}

/// Select state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectState<Id> {
    selected: Option<Id>,
    open: bool,
    focus_index: usize,
    focused: bool,
    trigger_region: Option<Rect>,
}

impl<Id: Clone> SelectState<Id> {
    /// Closed select.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            open: false,
            focus_index: 0,
            focused: false,
            trigger_region: None,
        }
    }

    #[must_use]
    /// Selected.
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    /// Menu open.
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Keys: open/close/navigate/select.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> SelectOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if key.kind == KeyEventKind::Release {
            return SelectOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        if !self.open {
            if !self.focused {
                return SelectOutcome::Ignored;
            }
            return match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Down if is_press => {
                    self.open = true;
                    SelectOutcome::OpenMenu
                }
                _ => SelectOutcome::Ignored,
            };
        }
        match key.code {
            KeyCode::Esc if is_press => {
                self.open = false;
                SelectOutcome::CloseMenu
            }
            KeyCode::Down => {
                if !options.is_empty() {
                    self.focus_index = (self.focus_index + 1) % options.len();
                }
                SelectOutcome::Ignored
            }
            KeyCode::Up => {
                if !options.is_empty() {
                    self.focus_index = self.focus_index.checked_sub(1).unwrap_or(options.len() - 1);
                }
                SelectOutcome::Ignored
            }
            KeyCode::Enter if is_press => {
                if let Some(id) = options.get(self.focus_index) {
                    self.selected = Some(id.clone());
                    self.open = false;
                    SelectOutcome::Selected(id.clone())
                } else {
                    SelectOutcome::Ignored
                }
            }
            _ => SelectOutcome::Ignored,
        }
    }
}

/// Select trigger chrome (menu list is separate / OverlayStack).
#[derive(Debug, Clone, Copy)]
pub struct Select<'a, Id> {
    placeholder: &'a str,
    tokens: &'a DesignSystem,
    _id: core::marker::PhantomData<Id>,
}

impl<'a, Id> Select<'a, Id> {
    /// Placeholder when empty.
    #[must_use]
    pub const fn new(placeholder: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            placeholder,
            tokens,
            _id: core::marker::PhantomData,
        }
    }

    /// Paint trigger; labels from selected display string.
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SelectState<Id>,
        selected_label: Option<&str>,
    ) {
        state.trigger_region = None;
        if area.is_empty() {
            return;
        }
        let label = selected_label.unwrap_or(self.placeholder);
        let open = if state.open { "▴" } else { "▾" };
        let style = if state.focused {
            self.tokens.style(Role::Input)
        } else {
            self.tokens.style(Role::Text)
        };
        let line = format!(" {label} {open} ");
        let text = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.trigger_region = Some(area);
    }
}

/// Multi-select membership outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MultiSelectOutcome<Id> {
    /// No change.
    Ignored,
    /// Added id.
    Added(Id),
    /// Removed id.
    Removed(Id),
}

/// Multi-select state using membership set order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultiSelectState<Id: Clone + PartialEq> {
    selected: Vec<Id>,
    focus_index: usize,
    focused: bool,
}

impl<Id: Clone + PartialEq> MultiSelectState<Id> {
    /// From selected ids.
    #[must_use]
    pub fn new(selected: Vec<Id>) -> Self {
        Self {
            selected,
            focus_index: 0,
            focused: false,
        }
    }

    #[must_use]
    /// Membership.
    pub fn selected(&self) -> &[Id] {
        &self.selected
    }

    /// Space toggles focused option.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> MultiSelectOutcome<Id> {
        if !self.focused || options.is_empty() || key.kind != KeyEventKind::Press {
            return MultiSelectOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down => {
                self.focus_index = (self.focus_index + 1) % options.len();
                MultiSelectOutcome::Ignored
            }
            KeyCode::Up => {
                self.focus_index = self.focus_index.checked_sub(1).unwrap_or(options.len() - 1);
                MultiSelectOutcome::Ignored
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let id = options[self.focus_index].clone();
                if let Some(pos) = self.selected.iter().position(|s| s == &id) {
                    self.selected.remove(pos);
                    MultiSelectOutcome::Removed(id)
                } else {
                    self.selected.push(id.clone());
                    MultiSelectOutcome::Added(id)
                }
            }
            _ => MultiSelectOutcome::Ignored,
        }
    }
}

/// MultiSelect paint helper.
#[derive(Debug, Clone, Copy)]
pub struct MultiSelect<'a, Id> {
    options: &'a [(Id, &'a str)],
    tokens: &'a DesignSystem,
}

impl<'a, Id: Clone + PartialEq> MultiSelect<'a, Id> {
    /// Options.
    #[must_use]
    pub const fn new(options: &'a [(Id, &'a str)], tokens: &'a DesignSystem) -> Self {
        Self { options, tokens }
    }

    /// Render checklist.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &MultiSelectState<Id>) {
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        for (i, (id, label)) in self.options.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let on = state.selected.iter().any(|s| s == id);
            let mark = if on { "[x]" } else { "[ ]" };
            let style = if state.focus_index == i && state.focused {
                self.tokens.style(Role::Focus)
            } else {
                self.tokens.style(Role::Text)
            };
            let line = format!("{mark} {label}");
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
        }
    }
}

/// Combobox: query + results (Picker evolution). Free-text optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxState {
    query: String,
    focus_index: usize,
    open: bool,
    focused: bool,
    free_text: bool,
}

/// Combobox outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComboboxOutcome {
    /// No change.
    Ignored,
    /// Query text changed.
    QueryChanged,
    /// Selected result index.
    Selected {
        /// Index into projected results.
        index: usize,
    },
    /// Submit free text (when free_text).
    SubmitQuery,
    /// Close.
    Closed,
}

impl ComboboxState {
    /// Empty query.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            focus_index: 0,
            open: false,
            focused: false,
            free_text: false,
        }
    }

    /// Allow free-text submit.
    #[must_use]
    pub fn free_text(mut self, free_text: bool) -> Self {
        self.free_text = free_text;
        self
    }

    #[must_use]
    /// Query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Controlled query.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, result_len: usize) -> ComboboxOutcome {
        if !self.focused || key.kind == KeyEventKind::Release {
            return ComboboxOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        match key.code {
            KeyCode::Esc if is_press && self.open => {
                self.open = false;
                ComboboxOutcome::Closed
            }
            KeyCode::Down if result_len > 0 => {
                self.open = true;
                self.focus_index = (self.focus_index + 1) % result_len;
                ComboboxOutcome::Ignored
            }
            KeyCode::Up if result_len > 0 => {
                self.open = true;
                self.focus_index = self.focus_index.checked_sub(1).unwrap_or(result_len - 1);
                ComboboxOutcome::Ignored
            }
            KeyCode::Enter if is_press => {
                if self.open && result_len > 0 {
                    ComboboxOutcome::Selected {
                        index: self.focus_index.min(result_len - 1),
                    }
                } else if self.free_text {
                    ComboboxOutcome::SubmitQuery
                } else {
                    ComboboxOutcome::Ignored
                }
            }
            KeyCode::Backspace if is_press || key.kind == KeyEventKind::Repeat => {
                self.query.pop();
                self.open = true;
                ComboboxOutcome::QueryChanged
            }
            KeyCode::Char(c)
                if !c.is_control() && (is_press || key.kind == KeyEventKind::Repeat) =>
            {
                self.query.push(c);
                self.open = true;
                ComboboxOutcome::QueryChanged
            }
            _ => ComboboxOutcome::Ignored,
        }
    }
}

impl Default for ComboboxState {
    fn default() -> Self {
        Self::new()
    }
}

/// Combobox chrome (results list separate).
#[derive(Debug, Clone, Copy)]
pub struct Combobox<'a> {
    placeholder: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a> Combobox<'a> {
    /// Placeholder.
    #[must_use]
    pub const fn new(placeholder: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            placeholder,
            tokens,
        }
    }

    /// Paint query field.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &ComboboxState) {
        if area.is_empty() {
            return;
        }
        let show = if state.query.is_empty() {
            self.placeholder
        } else {
            state.query.as_str()
        };
        let style = if state.focused {
            self.tokens.style(Role::Input)
        } else {
            self.tokens.style(Role::TextMuted)
        };
        let text = take_display_cols(show, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_space_toggles_outcome() {
        let mut state = CheckboxState::new(false);
        state.set_focused(true);
        let out = state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &"a");
        assert!(matches!(
            out,
            CheckboxOutcome::ValueChanged {
                id: "a",
                value: CheckboxValue::Checked
            }
        ));
        assert!(state.is_checked());
    }

    #[test]
    fn checkbox_disabled_ignores() {
        let mut state = CheckboxState::new(false);
        state.set_focused(true);
        state.set_enabled(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &1),
            CheckboxOutcome::Ignored
        ));
    }

    #[test]
    fn checkbox_indeterminate_activates_to_checked() {
        let mut state = CheckboxState::with_value(CheckboxValue::Indeterminate);
        state.set_focused(true);
        let out = state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &"g");
        assert!(matches!(
            out,
            CheckboxOutcome::ValueChanged {
                value: CheckboxValue::Checked,
                ..
            }
        ));
    }

    #[test]
    fn checkbox_read_only_ignores() {
        let mut state = CheckboxState::new(true);
        state.set_focused(true);
        state.set_read_only(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &"x"),
            CheckboxOutcome::Ignored
        ));
    }

    #[test]
    fn checkbox_value_from_children_mixed() {
        assert_eq!(
            CheckboxValue::from_children([true, false, true]),
            CheckboxValue::Indeterminate
        );
        assert_eq!(
            CheckboxValue::from_children([true, true]),
            CheckboxValue::Checked
        );
        assert_eq!(
            CheckboxValue::from_children([false, false]),
            CheckboxValue::Unchecked
        );
    }

    #[test]
    fn checkbox_ascii_marks_no_color() {
        let system = DesignSystem::default().glyphs(crate::style::GlyphSet::Ascii);
        let cb = Checkbox::new("e", "Enable", &system).colorless(true);
        let mut state = CheckboxState::new(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let parts = cb.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        assert!(!parts.box_area.is_empty());
        assert_eq!(
            buf.cell((0, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("[")
        );
        state.set_value(CheckboxValue::Indeterminate);
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = cb.paint(Rect::new(0, 0, 20, 1), &mut buf2, &mut state);
        assert_eq!(
            buf2.cell((1, 0))
                .map(|c| c.symbol().to_string())
                .as_deref(),
            Some("-")
        );
    }

    #[test]
    fn checkbox_description_and_narrow_drop() {
        let system = DesignSystem::default();
        let cb = Checkbox::new("n", "Notify", &system).description("Email on complete");
        let mut state = CheckboxState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 2));
        let parts = cb.paint(Rect::new(0, 0, 40, 2), &mut buf, &mut state);
        assert!(parts.description_area.is_some());
        // Narrow / short: drop description
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 10, 1));
        let parts2 = cb.paint(Rect::new(0, 0, 10, 1), &mut buf2, &mut state);
        assert!(parts2.description_area.is_none());
    }

    #[test]
    fn checkbox_mouse_toggles() {
        let system = DesignSystem::default();
        let cb = Checkbox::new("m", "Mouse", &system);
        let mut state = CheckboxState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 1));
        let parts = cb.paint(Rect::new(0, 0, 24, 1), &mut buf, &mut state);
        let out = cb.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: parts.root.x,
                    y: parts.root.y,
                },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(
            out,
            CheckboxOutcome::ValueChanged {
                value: CheckboxValue::Checked,
                ..
            }
        ));
    }

    #[test]
    fn checkbox_semantic_registers() {
        let system = DesignSystem::default();
        let cb = Checkbox::new("s", "Save", &system);
        let mut state = CheckboxState::new(true);
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = cb.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let mut scene = SemanticScene::<&str, ()>::default();
        cb.register_semantic(&mut scene, Rect::new(0, 0, 20, 1), &state);
        assert!(scene.len() >= 1);
    }

    #[test]
    fn checkbox_list_composition() {
        // Dynamic controlled list of checkboxes (table/list leading pattern).
        let system = DesignSystem::default();
        let ids = ["a", "b", "c"];
        let mut values = [
            CheckboxValue::Checked,
            CheckboxValue::Unchecked,
            CheckboxValue::Checked,
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
        for (i, id) in ids.iter().enumerate() {
            let mut st = CheckboxState::with_value(values[i]);
            let cb = Checkbox::new(*id, *id, &system);
            let y = u16::try_from(i).unwrap_or(0);
            let _ = cb.paint(Rect::new(0, y, 30, 1), &mut buf, &mut st);
        }
        // Parent mixed aggregate
        assert_eq!(
            CheckboxValue::from_children(values.iter().map(|v| v.is_checked())),
            CheckboxValue::Indeterminate
        );
        // Activate first → unchecked; recompute
        let mut st0 = CheckboxState::with_value(values[0]);
        st0.set_focused(true);
        let out = st0.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &ids[0],
        );
        if let CheckboxOutcome::ValueChanged { value, .. } = out {
            values[0] = value;
        }
        assert_eq!(
            CheckboxValue::from_children(values.iter().map(|v| v.is_checked())),
            CheckboxValue::Indeterminate
        );
    }

    #[test]
    fn checkbox_paint_hot_path() {
        let system = DesignSystem::default();
        let cb = Checkbox::new("h", "Hot", &system).description("fast path");
        let mut state = CheckboxState::new(false);
        let area = Rect::new(0, 0, 32, 2);
        let mut buf = Buffer::empty(area);
        for _ in 0..500 {
            let _ = cb.paint(area, &mut buf, &mut state);
        }
        assert!(state.parts.is_some());
    }

    #[test]
    fn checkbox_invalid_paint() {
        let system = DesignSystem::default();
        let cb = Checkbox::new("i", "Accept", &system);
        let mut state = CheckboxState::new(false);
        state.set_invalid(true);
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 1));
        let parts = cb.paint(Rect::new(0, 0, 24, 1), &mut buf, &mut state);
        assert!(!parts.root.is_empty());
    }

    #[test]
    fn radio_roving_and_select() {
        let opts = ["a", "b", "c"];
        let mut state = RadioState::new(Some("a"));
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &opts);
        assert_eq!(state.active(), Some(&"b"));
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &opts);
        assert_eq!(out, RadioOutcome::Selected("b"));
    }

    #[test]
    fn switch_toggles() {
        let mut state = SwitchState::new(false);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &"s"),
            SwitchOutcome::ValueChanged { id: "s", on: true }
        ));
    }

    #[test]
    fn select_open_esc_close() {
        let opts = ["x", "y"];
        let mut state = SelectState::new(None::<&str>);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &opts),
            SelectOutcome::OpenMenu
        ));
        assert!(state.is_open());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &opts),
            SelectOutcome::CloseMenu
        ));
    }

    #[test]
    fn multiselect_toggle_membership() {
        let opts = ["a", "b"];
        let mut state = MultiSelectState::new(vec![]);
        state.focused = true;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &opts),
            MultiSelectOutcome::Added("a")
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &opts),
            MultiSelectOutcome::Removed("a")
        ));
    }

    #[test]
    fn combobox_query_and_select() {
        let mut state = ComboboxState::new().free_text(true);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), 0),
            ComboboxOutcome::QueryChanged
        ));
        assert_eq!(state.query(), "h");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 0),
            ComboboxOutcome::SubmitQuery
        ));
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), 2);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 2);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 2),
            ComboboxOutcome::Selected { index: 1 }
        ));
    }
}
