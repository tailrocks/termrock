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

// ── RadioGroup ──────────────────────────────────────────────────────────────

/// When movement commits selection.
///
/// **Default [`FollowFocus`](Self::FollowFocus)** matches native desktop and
/// Radix: arrow/Home/End/typeahead update both active descendant **and**
/// selection. Use [`ActivateToSelect`](Self::ActivateToSelect) when browsing
/// options must not change the value until Space/Enter (settings review,
/// destructive choices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RadioSelectionPolicy {
    /// Movement commits selection (native / Radix default).
    #[default]
    FollowFocus,
    /// Movement only; Space/Enter commits selection.
    ActivateToSelect,
}

impl RadioSelectionPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FollowFocus => "follow-focus",
            Self::ActivateToSelect => "activate-to-select",
        }
    }
}

/// Layout axis for a [`RadioGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RadioGroupOrientation {
    /// One option per row (default; also forced when narrow).
    #[default]
    Vertical,
    /// Options in a single row (settings chips / permission actions).
    Horizontal,
}

impl RadioGroupOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }
}

/// One option in a [`RadioGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioOption<'a, Id> {
    /// Stable id (selection + roving).
    pub id: Id,
    /// Primary label.
    pub label: &'a str,
    /// Optional secondary line (vertical only; dropped when height tight).
    pub description: Option<&'a str>,
    /// Optional trailing badge (e.g. "recommended").
    pub badge: Option<&'a str>,
    /// Whether selectable (skipped by roving when false).
    pub enabled: bool,
}

impl<'a, Id> RadioOption<'a, Id> {
    /// Enabled option with label.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            description: None,
            badge: None,
            enabled: true,
        }
    }

    /// Description line.
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Trailing badge.
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Enabled flag.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Typeahead / a11y label.
    #[must_use]
    pub fn a11y(&self) -> &str {
        if self.label.is_empty() {
            "option"
        } else {
            self.label
        }
    }
}

/// Per-option paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioOptionParts<Id> {
    /// Option id.
    pub id: Id,
    /// Full option hit rect.
    pub area: Rect,
    /// Radio mark box.
    pub mark_area: Rect,
    /// Whether disabled.
    pub disabled: bool,
}

/// Group paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioGroupParts<Id> {
    /// Root area.
    pub root: Rect,
    /// Legend rect when painted.
    pub legend: Option<Rect>,
    /// Option parts (source order).
    pub options: Vec<RadioOptionParts<Id>>,
}

/// Radio group outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadioOutcome<Id> {
    /// No change.
    Ignored,
    /// Active descendant moved without selection change
    /// ([`RadioSelectionPolicy::ActivateToSelect`] only).
    CursorMoved {
        /// New active id.
        id: Id,
    },
    /// Selection committed (also on FollowFocus movement).
    Selected(Id),
}

/// Radio group state: controlled selection + roving active descendant.
///
/// Host owns domain value: apply [`RadioOutcome::Selected`] then
/// [`Self::set_selected`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioState<Id> {
    selected: Option<Id>,
    collection: crate::interaction::CollectionState<Id>,
    /// Group keyboard ownership.
    surface_focused: bool,
    /// Group enabled.
    enabled: bool,
    /// Validation failed (form).
    invalid: bool,
    /// Selection commit policy.
    policy: RadioSelectionPolicy,
    /// Hovered option.
    hovered: Option<Id>,
    /// Last parts.
    parts: Option<RadioGroupParts<Id>>,
    /// Legacy hit regions (same order as painted options).
    regions: Vec<Rect>,
}

impl<Id: Clone + PartialEq> RadioState<Id> {
    /// Optional initial selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        let mut collection = crate::interaction::CollectionState::new()
            .orientation(crate::interaction::RovingOrientation::Vertical)
            .wrap(true);
        collection.set_active(selected.clone());
        Self {
            selected,
            collection,
            surface_focused: false,
            enabled: true,
            invalid: false,
            policy: RadioSelectionPolicy::FollowFocus,
            hovered: None,
            parts: None,
            regions: Vec::new(),
        }
    }

    /// Selected id.
    #[must_use]
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Active descendant (cursor); may differ from selected under ActivateToSelect.
    #[must_use]
    pub const fn active(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Selection policy.
    #[must_use]
    pub const fn policy(&self) -> RadioSelectionPolicy {
        self.policy
    }

    /// Surface focus.
    #[must_use]
    pub const fn is_surface_focused(&self) -> bool {
        self.surface_focused
    }

    /// Parts from last paint.
    #[must_use]
    pub const fn parts(&self) -> Option<&RadioGroupParts<Id>> {
        self.parts.as_ref()
    }

    /// Controlled select (also moves active when `Some`).
    pub fn set_selected(&mut self, selected: Option<Id>) {
        self.selected = selected.clone();
        if selected.is_some() {
            self.collection.set_active(selected);
        }
    }

    /// Surface focus.
    pub fn set_surface_focused(&mut self, on: bool) {
        self.surface_focused = on;
        if !on {
            self.collection.clear_typeahead();
        }
    }

    /// Group enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Invalid chrome.
    pub const fn set_invalid(&mut self, on: bool) {
        self.invalid = on;
    }

    /// Selection policy.
    pub const fn set_policy(&mut self, policy: RadioSelectionPolicy) {
        self.policy = policy;
    }

    /// Builder-style policy.
    #[must_use]
    pub const fn policy_mode(mut self, policy: RadioSelectionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Hit regions (legacy).
    #[must_use]
    pub fn regions(&self) -> &[Rect] {
        &self.regions
    }
}

/// Single-choice group with roving focus.
///
/// **vs Tabs / ModeRibbon.** Those navigate content/modes. RadioGroup is a form
/// value choice (settings, permissions, question flows).
///
/// **vs ToggleGroup single.** ToggleGroup is sticky toolbar chrome; RadioGroup
/// is exclusive form selection with legend, descriptions, and radio marks.
///
/// **Selection policy.** Default [`RadioSelectionPolicy::FollowFocus`]: arrows
/// commit. [`RadioSelectionPolicy::ActivateToSelect`] requires Space/Enter.
#[derive(Debug, Clone, Copy)]
pub struct RadioGroup<'a, Id> {
    options: &'a [RadioOption<'a, Id>],
    system: &'a DesignSystem,
    legend: Option<&'a str>,
    orientation: RadioGroupOrientation,
    /// Auto-vertical when width &lt; this (0 = never). Default 28.
    stack_below: u16,
    colorless: bool,
}

impl<'a, Id> RadioGroup<'a, Id> {
    /// Options + design system.
    #[must_use]
    pub const fn new(options: &'a [RadioOption<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            options,
            system,
            legend: None,
            orientation: RadioGroupOrientation::Vertical,
            stack_below: 28,
            colorless: false,
        }
    }

    /// Group legend / question text.
    #[must_use]
    pub const fn legend(mut self, legend: &'a str) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: RadioGroupOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Horizontal layout.
    #[must_use]
    pub const fn horizontal(mut self) -> Self {
        self.orientation = RadioGroupOrientation::Horizontal;
        self
    }

    /// Vertical layout (default).
    #[must_use]
    pub const fn vertical(mut self) -> Self {
        self.orientation = RadioGroupOrientation::Vertical;
        self
    }

    /// Force vertical when width &lt; `cols` (0 disables).
    #[must_use]
    pub const fn stack_below(mut self, cols: u16) -> Self {
        self.stack_below = cols;
        self
    }

    /// Force ASCII-style marks.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn resolved_orientation(&self, width: u16) -> RadioGroupOrientation {
        if matches!(self.orientation, RadioGroupOrientation::Vertical) {
            return RadioGroupOrientation::Vertical;
        }
        if self.stack_below > 0 && width < self.stack_below {
            RadioGroupOrientation::Vertical
        } else {
            RadioGroupOrientation::Horizontal
        }
    }

    fn mono(&self) -> bool {
        self.colorless
            || self.system.glyphs.is_ascii()
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }

    fn mark(&self, selected: bool) -> &'static str {
        if self.mono() {
            return if selected { "(*)" } else { "( )" };
        }
        // Prefer 3-col bracket forms for alignment stability even in Unicode
        // when glyph is single-cell — still use catalog for enhanced.
        if self.system.glyphs.is_ascii() {
            return if selected { "(*)" } else { "( )" };
        }
        if selected {
            self.system.glyphs.resolve(crate::style::Glyph::RadioOn).text
        } else {
            self.system
                .glyphs
                .resolve(crate::style::Glyph::RadioOff)
                .text
        }
    }

    fn mark_cols(&self, selected: bool) -> u16 {
        display_cols(self.mark(selected)) as u16
    }

    fn option_label_line(&self, opt: &RadioOption<'a, Id>, max_cols: usize) -> String {
        let mut s = opt.label.to_string();
        if let Some(b) = opt.badge {
            if !b.is_empty() {
                s.push(' ');
                s.push('[');
                s.push_str(b);
                s.push(']');
            }
        }
        take_display_cols(&s, max_cols)
    }

    fn collection_items(&self) -> Vec<crate::interaction::CollectionItem<Id>>
    where
        Id: Clone,
    {
        self.options
            .iter()
            .map(|o| crate::interaction::CollectionItem {
                id: o.id.clone(),
                enabled: o.enabled,
                label: o.label.to_string(),
                parent: None,
            })
            .collect()
    }

    fn option_by_id(&self, id: &Id) -> Option<&RadioOption<'a, Id>>
    where
        Id: PartialEq,
    {
        self.options.iter().find(|o| &o.id == id)
    }
}

impl<'a, Id: Clone + PartialEq> RadioGroup<'a, Id> {
    /// Paint group.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut RadioState<Id>,
    ) -> RadioGroupParts<Id> {
        state.regions.clear();
        state.parts = None;
        if area.is_empty() || self.options.is_empty() {
            let parts = RadioGroupParts {
                root: area,
                legend: None,
                options: Vec::new(),
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        // Reconcile roving with enabled flags + typeahead labels
        let items = self.collection_items();
        let _ = state.collection.reconcile(&items);
        if state.collection.active().is_none() {
            if let Some(sel) = state.selected.clone() {
                state.collection.set_active(Some(sel));
            } else if let Some(first) = items.iter().find(|e| e.enabled) {
                state.collection.set_active(Some(first.id.clone()));
            }
        }
        let orient = self.resolved_orientation(area.width);

        let mut y = area.y;
        let mut legend_rect = None;
        if let Some(leg) = self.legend {
            if !leg.is_empty() && y < area.bottom() {
                let mut style = self.system.style(if state.invalid {
                    Role::Danger
                } else {
                    Role::TextStrong
                });
                if state.surface_focused {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                let text = take_display_cols(leg, usize::from(area.width));
                buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
                legend_rect = Some(Rect::new(
                    area.x,
                    y,
                    display_cols(&text).min(usize::from(area.width)) as u16,
                    1,
                ));
                y = y.saturating_add(1);
            }
        }

        let mut option_parts = Vec::new();
        match orient {
            RadioGroupOrientation::Vertical => {
                for opt in self.options {
                    if y >= area.bottom() {
                        break;
                    }
                    let selected = state.selected.as_ref() == Some(&opt.id);
                    let focused = state.surface_focused && state.active() == Some(&opt.id);
                    let hovered = state.hovered.as_ref() == Some(&opt.id);
                    let mark = self.mark(selected);
                    let mark_w = self.mark_cols(selected).min(area.width);
                    let mark_area = Rect::new(area.x, y, mark_w.max(1), 1);
                    let style = self.option_style(state, opt, selected, focused, hovered);
                    buffer.set_stringn(
                        mark_area.x,
                        mark_area.y,
                        mark,
                        usize::from(mark_w.max(1)),
                        style,
                    );
                    let label_x = area.x.saturating_add(mark_w).saturating_add(1);
                    let mut row_h = 1u16;
                    if label_x < area.right() {
                        let lw = area.right().saturating_sub(label_x);
                        let line = self.option_label_line(opt, usize::from(lw));
                        buffer.set_stringn(label_x, y, &line, usize::from(lw), style);
                    }
                    // Description under label when room
                    if area.height.saturating_sub(y.saturating_sub(area.y)) > 1
                        && area.width >= 16
                        && let Some(desc) = opt.description
                        && !desc.is_empty()
                    {
                        let dy = y.saturating_add(1);
                        if dy < area.bottom() {
                            let dx = label_x.min(area.right().saturating_sub(1));
                            let dw = area.right().saturating_sub(dx);
                            let text = take_display_cols(desc, usize::from(dw));
                            let dstyle = if !opt.enabled || !state.enabled {
                                self.system.style(Role::TextDisabled)
                            } else {
                                self.system.style(Role::TextMuted)
                            };
                            buffer.set_stringn(dx, dy, &text, usize::from(dw), dstyle);
                            row_h = 2;
                        }
                    }
                    let hit = Rect::new(area.x, y, area.width, row_h.min(area.bottom() - y));
                    state.regions.push(hit);
                    option_parts.push(RadioOptionParts {
                        id: opt.id.clone(),
                        area: hit,
                        mark_area,
                        disabled: !opt.enabled,
                    });
                    y = y.saturating_add(row_h);
                }
            }
            RadioGroupOrientation::Horizontal => {
                let mut x = area.x;
                let gap = 2u16;
                for opt in self.options {
                    if x >= area.right() {
                        break;
                    }
                    let selected = state.selected.as_ref() == Some(&opt.id);
                    let focused = state.surface_focused && state.active() == Some(&opt.id);
                    let hovered = state.hovered.as_ref() == Some(&opt.id);
                    let mark = self.mark(selected);
                    let mark_w = self.mark_cols(selected);
                    let label = self.option_label_line(opt, 24);
                    let label_w = display_cols(&label) as u16;
                    let w = mark_w
                        .saturating_add(1)
                        .saturating_add(label_w)
                        .min(area.right().saturating_sub(x));
                    if w == 0 {
                        break;
                    }
                    let style = self.option_style(state, opt, selected, focused, hovered);
                    let mark_area = Rect::new(x, area.y, mark_w.min(w).max(1), 1.min(area.height));
                    buffer.set_stringn(
                        mark_area.x,
                        mark_area.y,
                        mark,
                        usize::from(mark_area.width),
                        style,
                    );
                    let lx = x.saturating_add(mark_w).saturating_add(1);
                    if lx < x.saturating_add(w) && area.height > 0 {
                        let lw = x.saturating_add(w).saturating_sub(lx);
                        let text = take_display_cols(&label, usize::from(lw));
                        buffer.set_stringn(lx, area.y, &text, usize::from(lw), style);
                    }
                    let hit = Rect::new(x, area.y, w, 1.min(area.height));
                    state.regions.push(hit);
                    option_parts.push(RadioOptionParts {
                        id: opt.id.clone(),
                        area: hit,
                        mark_area,
                        disabled: !opt.enabled,
                    });
                    x = x.saturating_add(w).saturating_add(gap);
                }
            }
        }

        let root_h = area
            .height
            .min(y.saturating_sub(area.y).max(1));
        let parts = RadioGroupParts {
            root: Rect::new(area.x, area.y, area.width, root_h.min(area.height)),
            legend: legend_rect,
            options: option_parts,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn option_style(
        &self,
        state: &RadioState<Id>,
        opt: &RadioOption<'a, Id>,
        selected: bool,
        focused: bool,
        hovered: bool,
    ) -> ratatui_core::style::Style {
        if !state.enabled || !opt.enabled {
            return self.system.style(Role::TextDisabled);
        }
        if state.invalid && selected {
            return self.system.style(Role::Danger).add_modifier(Modifier::BOLD);
        }
        if focused {
            let mut s = self.system.style(Role::Focus);
            if selected {
                s = s.add_modifier(Modifier::BOLD);
            } else {
                s = s.add_modifier(Modifier::UNDERLINED);
            }
            return s;
        }
        if selected {
            let mut s = self.system.style(Role::TextStrong);
            s = s.add_modifier(Modifier::BOLD);
            if self.mono() {
                s = s.add_modifier(Modifier::REVERSED);
            }
            return s;
        }
        if hovered {
            return self.system.style(Role::Text).add_modifier(Modifier::UNDERLINED);
        }
        self.system.style(Role::Text)
    }

    /// Prefer [`Self::paint`].
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut RadioState<Id>) {
        let _ = self.paint(area, buffer, state);
    }

    fn commit_selected(&self, state: &mut RadioState<Id>, id: Id) -> RadioOutcome<Id> {
        if let Some(opt) = self.option_by_id(&id) {
            if !opt.enabled {
                return RadioOutcome::Ignored;
            }
        } else {
            return RadioOutcome::Ignored;
        }
        state.selected = Some(id.clone());
        state.collection.set_active(Some(id.clone()));
        RadioOutcome::Selected(id)
    }

    /// Keys: roving + typeahead + policy-aware select.
    ///
    /// Up/Down **and** Left/Right move the active option (works for both
    /// orientations). Home/End and printable typeahead via collection roving.
    pub fn handle_key(&self, state: &mut RadioState<Id>, key: KeyEvent) -> RadioOutcome<Id> {
        if !state.enabled || !state.surface_focused || self.options.is_empty() {
            return RadioOutcome::Ignored;
        }
        if key.kind == KeyEventKind::Release {
            return RadioOutcome::Ignored;
        }
        let items = self.collection_items();
        let _ = state.collection.reconcile(&items);
        if items.iter().all(|e| !e.enabled) {
            return RadioOutcome::Ignored;
        }

        let is_press = key.kind == KeyEventKind::Press;

        // Space/Enter always commit active
        if is_press {
            if let Some(intent) = default_button_intent(key) {
                if matches!(
                    intent,
                    UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
                ) {
                    if let Some(id) = state.collection.active().cloned() {
                        return self.commit_selected(state, id);
                    }
                    return RadioOutcome::Ignored;
                }
            }
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                if let Some(id) = state.collection.active().cloned() {
                    return self.commit_selected(state, id);
                }
                return RadioOutcome::Ignored;
            }
        }

        // Explicit cross-axis movement (horizontal groups still use Left/Right)
        if is_press && key.modifiers.is_empty() {
            let before = state.collection.active().cloned();
            match key.code {
                KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                    let _ = state.collection.move_next(&items);
                    if state.collection.active() != before.as_ref() {
                        return self.after_cursor_move(state, before);
                    }
                }
                KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                    let _ = state.collection.move_previous(&items);
                    if state.collection.active() != before.as_ref() {
                        return self.after_cursor_move(state, before);
                    }
                }
                KeyCode::Home => {
                    let _ = state.collection.move_first(&items);
                    if state.collection.active() != before.as_ref() {
                        return self.after_cursor_move(state, before);
                    }
                }
                KeyCode::End => {
                    let _ = state.collection.move_last(&items);
                    if state.collection.active() != before.as_ref() {
                        return self.after_cursor_move(state, before);
                    }
                }
                _ => {}
            }
        }

        // Typeahead / remaining roving keys
        let before = state.collection.active().cloned();
        let _ = state.collection.handle_key(key, &items);
        if state.collection.active() != before.as_ref() {
            return self.after_cursor_move(state, before);
        }
        RadioOutcome::Ignored
    }

    fn after_cursor_move(
        &self,
        state: &mut RadioState<Id>,
        _before: Option<Id>,
    ) -> RadioOutcome<Id> {
        let Some(id) = state.collection.active().cloned() else {
            return RadioOutcome::Ignored;
        };
        match state.policy {
            RadioSelectionPolicy::FollowFocus => self.commit_selected(state, id),
            RadioSelectionPolicy::ActivateToSelect => RadioOutcome::CursorMoved { id },
        }
    }

    /// Intent path.
    pub fn handle_intent(
        &self,
        state: &mut RadioState<Id>,
        intent: UiIntent,
    ) -> RadioOutcome<Id> {
        if !state.enabled || self.options.is_empty() {
            return RadioOutcome::Ignored;
        }
        let items = self.collection_items();
        let _ = state.collection.reconcile(&items);
        match intent {
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                if let Some(id) = state.collection.active().cloned() {
                    self.commit_selected(state, id)
                } else {
                    RadioOutcome::Ignored
                }
            }
            other => {
                let before = state.collection.active().cloned();
                let _ = state.collection.handle_intent(other, &items);
                if state.collection.active() != before.as_ref() {
                    self.after_cursor_move(state, before)
                } else {
                    RadioOutcome::Ignored
                }
            }
        }
    }

    /// Mouse: click option → select + focus.
    pub fn handle_mouse(
        &self,
        state: &mut RadioState<Id>,
        event: MouseEvent,
    ) -> RadioOutcome<Id> {
        if !state.enabled {
            return RadioOutcome::Ignored;
        }
        let Some(parts) = state.parts.clone() else {
            return RadioOutcome::Ignored;
        };
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts
                    .options
                    .iter()
                    .find(|o| o.area.contains(event.position))
                    .map(|o| o.id.clone());
                RadioOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(opt) = parts
                    .options
                    .iter()
                    .find(|o| !o.disabled && o.area.contains(event.position))
                {
                    state.surface_focused = true;
                    state.collection.set_active(Some(opt.id.clone()));
                    return self.commit_selected(state, opt.id.clone());
                }
                RadioOutcome::Ignored
            }
            _ => RadioOutcome::Ignored,
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut RadioState<Id>,
        key: KeyEvent,
    ) -> EventResult<RadioOutcome<Id>> {
        match self.handle_key(state, key) {
            RadioOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic: group list + each option.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        group_id: Id,
        area: Rect,
        state: &RadioState<Id>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(group_id, area)
                .role(SemanticRole::List)
                .label(self.legend.unwrap_or("radio group"))
                .description(state.policy.id())
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.surface_focused,
                    invalid: state.invalid,
                    ..Default::default()
                }),
        );
        if let Some(parts) = &state.parts {
            for op in &parts.options {
                let Some(opt) = self.option_by_id(&op.id) else {
                    continue;
                };
                let selected = state.selected.as_ref() == Some(&op.id);
                let _ = scene.register(
                    SemanticNode::control(op.id.clone(), op.area)
                        .role(SemanticRole::ListItem)
                        .label(opt.label)
                        .description(opt.description.unwrap_or(""))
                        .focusable(opt.enabled && state.enabled)
                        .disabled(!opt.enabled || !state.enabled)
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

impl<Id: Clone + PartialEq> RadioState<Id> {
    /// Headless key path using option ids only (all enabled, no labels).
    ///
    /// Prefer [`RadioGroup::handle_key`] for full option metadata (disabled,
    /// typeahead labels, policy with widget).
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> RadioOutcome<Id> {
        if !self.enabled || options.is_empty() || key.kind == KeyEventKind::Release {
            return RadioOutcome::Ignored;
        }
        // Ensure surface focused for headless tests
        self.surface_focused = true;
        let items: Vec<crate::interaction::CollectionItem<Id>> = options
            .iter()
            .map(|id| crate::interaction::CollectionItem {
                id: id.clone(),
                enabled: true,
                label: String::new(),
                parent: None,
            })
            .collect();
        let _ = self.collection.reconcile(&items);
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
        let before = self.collection.active().cloned();
        let _ = self.collection.handle_key(key, &items);
        if self.collection.active() != before.as_ref() {
            if let Some(id) = self.collection.active().cloned() {
                return match self.policy {
                    RadioSelectionPolicy::FollowFocus => {
                        self.selected = Some(id.clone());
                        RadioOutcome::Selected(id)
                    }
                    RadioSelectionPolicy::ActivateToSelect => RadioOutcome::CursorMoved { id },
                };
            }
        }
        RadioOutcome::Ignored
    }

    /// Intent path (headless ids).
    pub fn handle_intent(&mut self, intent: UiIntent, options: &[Id]) -> RadioOutcome<Id> {
        if !self.enabled || options.is_empty() {
            return RadioOutcome::Ignored;
        }
        self.surface_focused = true;
        let items: Vec<crate::interaction::CollectionItem<Id>> = options
            .iter()
            .map(|id| crate::interaction::CollectionItem {
                id: id.clone(),
                enabled: true,
                label: String::new(),
                parent: None,
            })
            .collect();
        let _ = self.collection.reconcile(&items);
        match intent {
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                if let Some(id) = self.collection.active().cloned() {
                    self.selected = Some(id.clone());
                    RadioOutcome::Selected(id)
                } else {
                    RadioOutcome::Ignored
                }
            }
            other => {
                let before = self.collection.active().cloned();
                let _ = self.collection.handle_intent(other, &items);
                if self.collection.active() != before.as_ref() {
                    if let Some(id) = self.collection.active().cloned() {
                        return match self.policy {
                            RadioSelectionPolicy::FollowFocus => {
                                self.selected = Some(id.clone());
                                RadioOutcome::Selected(id)
                            }
                            RadioSelectionPolicy::ActivateToSelect => {
                                RadioOutcome::CursorMoved { id }
                            }
                        };
                    }
                }
                RadioOutcome::Ignored
            }
        }
    }
}

// ── Switch ──────────────────────────────────────────────────────────────────

/// Density / layout recipe for a [`Switch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SwitchRecipe {
    /// Settings row: label (and optional description) with track on the trailing edge.
    #[default]
    SettingsRow,
    /// Compact: track + label on one tight line (leading track).
    Compact,
}

impl SwitchRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SettingsRow => "settings-row",
            Self::Compact => "compact",
        }
    }
}

/// Switch outcome (controlled: host applies `on`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwitchOutcome<Id> {
    /// No change.
    Ignored,
    /// On/off change request.
    ValueChanged {
        /// Field id.
        id: Id,
        /// Next on state.
        on: bool,
    },
}

/// Paint geometry for a switch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SwitchParts {
    /// Full interactive root (label + track).
    pub root: Rect,
    /// Track / value face only.
    pub track: Rect,
    /// Label area when painted.
    pub label_area: Option<Rect>,
    /// Description area when painted.
    pub description_area: Option<Rect>,
}

/// Switch state (interaction + projected on).
///
/// **Pointer law (scroll-safe):** left **Down** inside the hit arms the control;
/// left **Up** inside the same hit toggles. Dragging out or scrolling away
/// cancels without a change — prevents accidental toggles in scrollable
/// settings lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SwitchState {
    /// Projected on/off.
    pub on: bool,
    /// Keyboard focus.
    pub focused: bool,
    /// Enabled.
    pub enabled: bool,
    /// Read-only: show value, no activate.
    pub read_only: bool,
    /// Loading: busy face, no activate.
    pub loading: bool,
    /// Validation / error chrome.
    pub invalid: bool,
    /// Pointer hover.
    pub hovered: bool,
    /// Armed by mouse Down inside hit (awaiting Up-in-region).
    pointer_armed: bool,
    /// Last paint parts.
    pub parts: Option<SwitchParts>,
    /// Hit root.
    region: Option<Rect>,
}

impl SwitchState {
    /// Initial on/off.
    #[must_use]
    pub const fn new(on: bool) -> Self {
        Self {
            on,
            focused: false,
            enabled: true,
            read_only: false,
            loading: false,
            invalid: false,
            hovered: false,
            pointer_armed: false,
            parts: None,
            region: None,
        }
    }

    /// On.
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.on
    }

    /// Controlled set.
    pub const fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.pointer_armed = false;
        }
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.pointer_armed = false;
        }
    }

    /// Read-only.
    pub const fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        if on {
            self.pointer_armed = false;
        }
    }

    /// Loading / busy.
    pub const fn set_loading(&mut self, on: bool) {
        self.loading = on;
        if on {
            self.pointer_armed = false;
        }
    }

    /// Invalid / error chrome.
    pub const fn set_invalid(&mut self, on: bool) {
        self.invalid = on;
    }

    /// Whether activate is allowed.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.read_only && !self.loading
    }

    /// Hit root.
    #[must_use]
    pub const fn region(&self) -> Option<Rect> {
        self.region
    }

    fn apply_toggle<Id: Clone>(&mut self, id: &Id) -> SwitchOutcome<Id> {
        if !self.can_activate() {
            return SwitchOutcome::Ignored;
        }
        self.on = !self.on;
        SwitchOutcome::ValueChanged {
            id: id.clone(),
            on: self.on,
        }
    }

    /// Space / Enter toggle when focused and activatable.
    pub fn handle_key<Id: Clone>(&mut self, key: KeyEvent, id: &Id) -> SwitchOutcome<Id> {
        if !self.can_activate() || !self.focused || key.kind != KeyEventKind::Press {
            return SwitchOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            if matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
            ) {
                return self.apply_toggle(id);
            }
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.apply_toggle(id),
            _ => SwitchOutcome::Ignored,
        }
    }

    /// Pointer: Down arms, Up-in-region toggles (scroll-safe).
    pub fn handle_mouse<Id: Clone>(&mut self, event: MouseEvent, id: &Id) -> SwitchOutcome<Id> {
        if !self.can_activate() {
            // Still clear arm / hover when disabled
            match event.kind {
                MouseEventKind::Moved => {
                    self.hovered = self.region.is_some_and(|r| r.contains(event.position));
                }
                MouseEventKind::Up(_) | MouseEventKind::Down(_) => {
                    self.pointer_armed = false;
                }
                _ => {}
            }
            return SwitchOutcome::Ignored;
        }
        let inside = self.region.is_some_and(|r| r.contains(event.position));
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                self.hovered = inside;
                if self.pointer_armed && !inside {
                    // Dragged out — cancel arm (scroll / miss)
                    self.pointer_armed = false;
                }
                SwitchOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if inside {
                    self.pointer_armed = true;
                    self.focused = true;
                } else {
                    self.pointer_armed = false;
                }
                SwitchOutcome::Ignored
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.pointer_armed && inside {
                    self.pointer_armed = false;
                    self.focused = true;
                    return self.apply_toggle(id);
                }
                self.pointer_armed = false;
                SwitchOutcome::Ignored
            }
            // Wheel / other: never toggle
            _ => {
                self.pointer_armed = false;
                SwitchOutcome::Ignored
            }
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result<Id: Clone>(
        &mut self,
        key: KeyEvent,
        id: &Id,
    ) -> EventResult<SwitchOutcome<Id>> {
        match self.handle_key(key, id) {
            SwitchOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }
}

/// Immediate on/off settings control.
///
/// **vs [`Checkbox`](Checkbox).** Checkbox is a form field with tri-state and
/// checked semantics (`[x]`). Switch is a settings preference with explicit
/// On/Off text and track chrome.
///
/// **vs [`Toggle`](crate::widgets::Toggle).** Toggle is a sticky toolbar tool.
/// Switch is a binary preference in settings rows.
///
/// **When not to use**
/// - Form multi-select / terms acceptance → Checkbox
/// - Toolbar formatting tools → Toggle / ToggleGroup
/// - Exclusive multi-option → RadioGroup
/// - Momentary action → Button
#[derive(Debug, Clone, Copy)]
pub struct Switch<'a, Id> {
    /// Stable identity.
    pub id: Id,
    label: &'a str,
    description: Option<&'a str>,
    system: &'a DesignSystem,
    recipe: SwitchRecipe,
    /// Paint explicit ON/OFF (or On/Off) text in the track (default true).
    show_value_text: bool,
    colorless: bool,
}

impl<'a, Id> Switch<'a, Id> {
    /// Id + label + design system.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            description: None,
            system,
            recipe: SwitchRecipe::SettingsRow,
            show_value_text: true,
            colorless: false,
        }
    }

    /// Secondary help line (settings-row; dropped when height &lt; 2).
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: SwitchRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Compact leading-track layout.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = SwitchRecipe::Compact;
        self
    }

    /// Settings-row trailing track (default).
    #[must_use]
    pub const fn settings_row(mut self) -> Self {
        self.recipe = SwitchRecipe::SettingsRow;
        self
    }

    /// Show explicit On/Off text in the track (default true — avoids color-only meaning).
    #[must_use]
    pub const fn show_value_text(mut self, on: bool) -> Self {
        self.show_value_text = on;
        self
    }

    /// Force monochrome / ASCII track emphasis.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Preferred height.
    #[must_use]
    pub fn preferred_height(&self) -> u16 {
        if matches!(self.recipe, SwitchRecipe::SettingsRow)
            && self.description.is_some_and(|d| !d.is_empty())
        {
            2
        } else {
            1
        }
    }

    /// Preferred track width (cells).
    #[must_use]
    pub fn track_width(&self, state: &SwitchState) -> u16 {
        if state.loading {
            return 5;
        }
        if self.show_value_text {
            5 // [ON ] / [OFF]
        } else if self.mono() {
            4 // [●=] style compressed
        } else {
            4
        }
    }

    fn mono(&self) -> bool {
        self.colorless
            || self.system.glyphs.is_ascii()
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }

    fn track_face(&self, state: &SwitchState) -> String {
        if state.loading {
            let spin = self.system.glyphs.loading();
            return format!("[{spin:^3}]");
        }
        if self.show_value_text || self.mono() {
            // Explicit text — required for no-color and default ambiguity avoidance
            return if state.on {
                "[ON ]".to_string()
            } else {
                "[OFF]".to_string()
            };
        }
        // Optional compact glyphs when value text disabled and not mono
        if state.on {
            "[●=]".to_string()
        } else {
            "[=●]".to_string()
        }
    }

    fn track_style(&self, state: &SwitchState) -> ratatui_core::style::Style {
        if !state.enabled {
            return self.system.style(Role::TextDisabled);
        }
        if state.read_only {
            return self.system.style(Role::TextMuted);
        }
        if state.loading {
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
            if state.on {
                s = s.add_modifier(Modifier::BOLD);
            }
            return s;
        }
        if state.on {
            let mut s = self.system.style(Role::Success);
            s = s.add_modifier(Modifier::BOLD);
            if self.mono() {
                s = s.add_modifier(Modifier::REVERSED);
            }
            s
        } else {
            self.system.style(Role::TextMuted)
        }
    }

    fn label_style(&self, state: &SwitchState) -> ratatui_core::style::Style {
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
            return self
                .system
                .style(Role::Focus)
                .add_modifier(Modifier::UNDERLINED);
        }
        if state.hovered {
            return self
                .system
                .style(Role::Text)
                .add_modifier(Modifier::UNDERLINED);
        }
        self.system.style(Role::Text)
    }

    /// Paint switch. Prefer this over [`Self::render`].
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SwitchState) -> SwitchParts {
        state.region = None;
        state.parts = None;
        if area.is_empty() {
            return SwitchParts::default();
        }

        let face = self.track_face(state);
        let track_w = (display_cols(&face) as u16)
            .max(self.track_width(state))
            .min(area.width.max(1));
        let track_style = self.track_style(state);
        let label_style = self.label_style(state);

        let (track, label_area, description_area, root) = match self.recipe {
            SwitchRecipe::Compact => {
                // [ON ] Label
                let track = Rect::new(area.x, area.y, track_w, 1.min(area.height));
                let face_t = take_display_cols(&face, usize::from(track_w));
                buffer.set_stringn(
                    track.x,
                    track.y,
                    &face_t,
                    usize::from(track_w),
                    track_style,
                );
                let mut label_area = None;
                let lx = area.x.saturating_add(track_w).saturating_add(1);
                if lx < area.right() && !self.label.is_empty() {
                    let lw = area.right().saturating_sub(lx);
                    let text = take_display_cols(self.label, usize::from(lw));
                    buffer.set_stringn(lx, area.y, &text, usize::from(lw), label_style);
                    let used = display_cols(&text).min(usize::from(lw)) as u16;
                    label_area = Some(Rect::new(lx, area.y, used, 1));
                }
                let content_w = track_w
                    .saturating_add(if label_area.is_some() { 1 } else { 0 })
                    .saturating_add(label_area.map(|r| r.width).unwrap_or(0));
                let root = Rect::new(area.x, area.y, content_w.min(area.width).max(1), 1.min(area.height));
                (track, label_area, None, root)
            }
            SwitchRecipe::SettingsRow => {
                // Label .......... [ON ]
                //   description
                let track_x = area.right().saturating_sub(track_w);
                let track = Rect::new(track_x.max(area.x), area.y, track_w.min(area.width), 1.min(area.height));
                let face_t = take_display_cols(&face, usize::from(track.width));
                buffer.set_stringn(
                    track.x,
                    track.y,
                    &face_t,
                    usize::from(track.width),
                    track_style,
                );
                let mut label_area = None;
                let label_max = track.x.saturating_sub(area.x).saturating_sub(1);
                if label_max > 0 && !self.label.is_empty() {
                    let text = take_display_cols(self.label, usize::from(label_max));
                    buffer.set_stringn(
                        area.x,
                        area.y,
                        &text,
                        usize::from(label_max),
                        label_style,
                    );
                    let used = display_cols(&text).min(usize::from(label_max)) as u16;
                    label_area = Some(Rect::new(area.x, area.y, used, 1));
                }
                let mut description_area = None;
                if area.height >= 2
                    && area.width >= 12
                    && let Some(desc) = self.description
                    && !desc.is_empty()
                {
                    let dy = area.y.saturating_add(1);
                    let dw = area.width.saturating_sub(1);
                    let text = take_display_cols(desc, usize::from(dw));
                    let dstyle = if !state.enabled {
                        self.system.style(Role::TextDisabled)
                    } else if state.invalid {
                        self.system.style(Role::Danger)
                    } else {
                        self.system.style(Role::TextMuted)
                    };
                    buffer.set_stringn(area.x.saturating_add(1), dy, &text, usize::from(dw), dstyle);
                    description_area = Some(Rect::new(
                        area.x.saturating_add(1),
                        dy,
                        display_cols(&text).min(usize::from(dw)) as u16,
                        1,
                    ));
                }
                let root_h = if description_area.is_some() {
                    2.min(area.height)
                } else {
                    1.min(area.height)
                };
                // Full row is hit target in settings lists (label + track)
                let root = Rect::new(area.x, area.y, area.width, root_h);
                (track, label_area, description_area, root)
            }
        };

        state.region = Some(root);
        let parts = SwitchParts {
            root,
            track,
            label_area,
            description_area,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Paint + StatefulWidget path.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut SwitchState) {
        let _ = self.paint(area, buffer, state);
    }

    /// Keys via widget.
    pub fn handle_key(&self, state: &mut SwitchState, key: KeyEvent) -> SwitchOutcome<Id>
    where
        Id: Clone,
    {
        state.handle_key(key, &self.id)
    }

    /// Mouse via widget (Down arm / Up-in-region).
    pub fn handle_mouse(&self, state: &mut SwitchState, event: MouseEvent) -> SwitchOutcome<Id>
    where
        Id: Clone,
    {
        state.handle_mouse(event, &self.id)
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut SwitchState,
        key: KeyEvent,
    ) -> EventResult<SwitchOutcome<Id>>
    where
        Id: Clone,
    {
        state.handle_key_result(key, &self.id)
    }

    /// Semantic registration.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        area: Rect,
        state: &SwitchState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let value = if state.loading {
            "loading"
        } else if state.on {
            "on"
        } else {
            "off"
        };
        let _ = scene.register(
            SemanticNode::control(self.id.clone(), area)
                .role(SemanticRole::Control)
                .label(self.label)
                .description(value)
                .focusable(state.can_activate())
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.on,
                    checked: state.on,
                    busy: state.loading,
                    invalid: state.invalid,
                    pressed: state.pointer_armed,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone> StatefulWidget for Switch<'_, Id> {
    type State = SwitchState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl<Id: Clone> StatefulWidget for &Switch<'_, Id> {
    type State = SwitchState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

// Combobox / Autocomplete live in `combobox.rs` (TextInput + CompletionMenu).

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
    fn radio_follow_focus_selects_on_move() {
        let opts = ["a", "b", "c"];
        let mut state = RadioState::new(Some("a"));
        // Default FollowFocus: Down commits selection
        let out = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &opts);
        assert_eq!(state.active(), Some(&"b"));
        assert_eq!(state.selected(), Some(&"b"));
        assert_eq!(out, RadioOutcome::Selected("b"));
    }

    #[test]
    fn radio_activate_to_select_policy() {
        let opts = ["a", "b", "c"];
        let mut state = RadioState::new(Some("a")).policy_mode(RadioSelectionPolicy::ActivateToSelect);
        let out = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &opts);
        assert_eq!(state.active(), Some(&"b"));
        assert_eq!(state.selected(), Some(&"a"));
        assert!(matches!(out, RadioOutcome::CursorMoved { id: "b" }));
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &opts);
        assert_eq!(out, RadioOutcome::Selected("b"));
        assert_eq!(state.selected(), Some(&"b"));
    }

    #[test]
    fn radio_group_paint_and_mouse() {
        let system = DesignSystem::default();
        let options = [
            RadioOption::new("a", "Alpha").description("First"),
            RadioOption::new("b", "Beta").badge("rec"),
            RadioOption::new("c", "Gamma").enabled(false),
        ];
        let g = RadioGroup::new(&options, &system).legend("Pick one");
        let mut state = RadioState::new(Some("a"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        let parts = g.paint(Rect::new(0, 0, 40, 8), &mut buf, &mut state);
        assert!(parts.legend.is_some());
        assert!(parts.options.len() >= 2);
        let b = parts.options.iter().find(|o| o.id == "b").unwrap();
        let out = g.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: b.area.x,
                    y: b.area.y,
                },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(out, RadioOutcome::Selected("b"));
    }

    #[test]
    fn radio_horizontal_and_narrow_stack() {
        let system = DesignSystem::default();
        let options = [
            RadioOption::new("l", "Low"),
            RadioOption::new("m", "Med"),
            RadioOption::new("h", "High"),
        ];
        let g = RadioGroup::new(&options, &system)
            .horizontal()
            .stack_below(40);
        let mut state = RadioState::new(Some("m"));
        // Wide: horizontal
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 2));
        let parts = g.paint(Rect::new(0, 0, 60, 2), &mut buf, &mut state);
        assert!(parts.options.len() >= 2);
        // Narrow: forced vertical
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 20, 6));
        let parts2 = g.paint(Rect::new(0, 0, 20, 6), &mut buf2, &mut state);
        assert_eq!(parts2.options.len(), 3);
        // stacked: y increases
        assert!(parts2.options[1].area.y > parts2.options[0].area.y);
    }

    #[test]
    fn radio_ascii_marks() {
        let system = DesignSystem::default().glyphs(crate::style::GlyphSet::Ascii);
        let options = [RadioOption::new("a", "A"), RadioOption::new("b", "B")];
        let g = RadioGroup::new(&options, &system).colorless(true);
        let mut state = RadioState::new(Some("a"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        let _ = g.paint(Rect::new(0, 0, 20, 3), &mut buf, &mut state);
        assert_eq!(
            buf.cell((0, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("(")
        );
        assert_eq!(
            buf.cell((1, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("*")
        );
    }

    #[test]
    fn radio_skips_disabled() {
        let system = DesignSystem::default();
        let options = [
            RadioOption::new("a", "A"),
            RadioOption::new("b", "B").enabled(false),
            RadioOption::new("c", "C"),
        ];
        let g = RadioGroup::new(&options, &system);
        let mut state = RadioState::new(Some("a"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
        let _ = g.paint(Rect::new(0, 0, 20, 4), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        );
        // Skips disabled b → c
        assert_eq!(out, RadioOutcome::Selected("c"));
    }

    #[test]
    fn radio_typeahead() {
        let system = DesignSystem::default();
        let options = [
            RadioOption::new("a", "Alpha"),
            RadioOption::new("b", "Beta"),
            RadioOption::new("g", "Gamma"),
        ];
        let g = RadioGroup::new(&options, &system);
        let mut state = RadioState::new(Some("a"));
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 4));
        let _ = g.paint(Rect::new(0, 0, 24, 4), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        );
        assert!(
            matches!(out, RadioOutcome::Selected("b"))
                || state.active() == Some(&"b")
                || state.selected() == Some(&"b")
        );
    }

    #[test]
    fn radio_semantic_and_hot_path() {
        let system = DesignSystem::default();
        let options = [
            RadioOption::new("a", "A"),
            RadioOption::new("b", "B"),
        ];
        let g = RadioGroup::new(&options, &system).legend("Mode");
        let mut state = RadioState::new(Some("a"));
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        for _ in 0..200 {
            let _ = g.paint(area, &mut buf, &mut state);
        }
        let mut scene = SemanticScene::<&str, ()>::default();
        g.register_semantic(&mut scene, "group", area, &state);
        assert!(scene.len() >= 2);
    }

    #[test]
    fn switch_toggles() {
        let mut state = SwitchState::new(false);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &"s"),
            SwitchOutcome::ValueChanged { id: "s", on: true }
        ));
        assert!(state.is_on());
    }

    #[test]
    fn switch_loading_and_read_only_block() {
        let mut state = SwitchState::new(false);
        state.set_focused(true);
        state.set_loading(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &"s"),
            SwitchOutcome::Ignored
        ));
        state.set_loading(false);
        state.set_read_only(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &"s"),
            SwitchOutcome::Ignored
        ));
    }

    #[test]
    fn switch_pointer_up_in_region_toggles() {
        let system = DesignSystem::default();
        let sw = Switch::new("dark", "Dark mode", &system);
        let mut state = SwitchState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = sw.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let pos = Position {
            x: parts.track.x,
            y: parts.track.y,
        };
        // Down only arms
        let out = sw.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: pos,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, SwitchOutcome::Ignored));
        assert!(!state.is_on());
        // Up in region toggles
        let out = sw.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                position: pos,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(
            out,
            SwitchOutcome::ValueChanged { id: "dark", on: true }
        ));
    }

    #[test]
    fn switch_pointer_drag_out_cancels() {
        let system = DesignSystem::default();
        let sw = Switch::new("s", "Sync", &system);
        let mut state = SwitchState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = sw.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let inside = Position {
            x: parts.root.x,
            y: parts.root.y,
        };
        let outside = Position {
            x: parts.root.x.saturating_add(parts.root.width).saturating_add(2),
            y: parts.root.y,
        };
        let _ = sw.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: inside,
                modifiers: KeyModifiers::NONE,
            },
        );
        let _ = sw.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: outside,
                modifiers: KeyModifiers::NONE,
            },
        );
        let out = sw.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                position: outside,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, SwitchOutcome::Ignored));
        assert!(!state.is_on());
    }

    #[test]
    fn switch_settings_row_and_on_off_text() {
        let system = DesignSystem::default().glyphs(crate::style::GlyphSet::Ascii);
        let sw = Switch::new("n", "Notifications", &system)
            .description("Push when idle")
            .colorless(true);
        let mut state = SwitchState::new(true);
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 2));
        let parts = sw.paint(Rect::new(0, 0, 40, 2), &mut buf, &mut state);
        assert!(parts.label_area.is_some());
        assert!(parts.description_area.is_some());
        // Track at trailing edge shows ON
        let tx = parts.track.x;
        assert_eq!(
            buf.cell((tx, 0))
                .map(|c| c.symbol().to_string())
                .as_deref(),
            Some("[")
        );
    }

    #[test]
    fn switch_compact_recipe() {
        let system = DesignSystem::default();
        let sw = Switch::new("c", "Compact", &system).compact();
        let mut state = SwitchState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 1));
        let parts = sw.paint(Rect::new(0, 0, 24, 1), &mut buf, &mut state);
        assert_eq!(parts.track.x, 0);
        assert!(parts.label_area.is_some());
    }

    #[test]
    fn switch_loading_face() {
        let system = DesignSystem::default();
        let sw = Switch::new("l", "Loading", &system).compact();
        let mut state = SwitchState::new(false);
        state.set_loading(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = sw.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        assert_eq!(
            buf.cell((0, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("[")
        );
    }

    #[test]
    fn switch_semantic_and_hot_path() {
        let system = DesignSystem::default();
        let sw = Switch::new("h", "Hot", &system).settings_row();
        let mut state = SwitchState::new(true);
        state.set_focused(true);
        let area = Rect::new(0, 0, 36, 1);
        let mut buf = Buffer::empty(area);
        for _ in 0..400 {
            let _ = sw.paint(area, &mut buf, &mut state);
        }
        let mut scene = SemanticScene::<&str, ()>::default();
        sw.register_semantic(&mut scene, area, &state);
        assert!(scene.len() >= 1);
    }

    // Select / MultiSelect / Combobox: see dedicated widget modules.
}
