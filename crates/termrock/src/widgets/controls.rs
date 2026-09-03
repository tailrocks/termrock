// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Controlled form controls: checkbox, radio, switch, select, multiselect, combobox (Plan 051).
//!
//! [`Checkbox`] is the form-field boolean/tri-state control (label + description).
//! Prefer [`crate::widgets::Toggle`] for sticky toolbar tools and
//! [`Switch`] for settings On/Off.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
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
    style::{
        ButtonRecipeVariant, ControlState, DesignSystem, ListRowVisualState, Role, VisualState,
    },
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
        if !self.can_activate() || !self.focused || !key.is_press() {
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
        CheckboxOutcome::Ignored
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
        let _ = state;
        // gutter + `[✓]` + space + label
        let label_w = display_cols(self.label) as u16;
        5u16.saturating_add(label_w).max(5)
    }

    fn box_mark(&self, value: CheckboxValue) -> &'static str {
        match value {
            CheckboxValue::Checked => self.system.glyphs.check_on(),
            CheckboxValue::Unchecked => self.system.glyphs.check_off(),
            // Junie has no mixed checkbox. Three-cell slot, catalog minus, no well.
            CheckboxValue::Indeterminate => " \u{2212} ",
        }
    }

    /// Paint checkbox. Prefer this over [`StatefulWidget::render`].
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut CheckboxState,
    ) -> CheckboxParts {
        state.region = None;
        state.parts = None;
        if area.is_empty() {
            return CheckboxParts::default();
        }

        let theme = self.system.junie_theme();
        let bg = theme.surface;
        let visual = VisualState {
            focused: state.focused && state.enabled,
            hovered: state.hovered && state.enabled && !state.read_only,
            disabled: !state.enabled,
            error: state.invalid,
            ..VisualState::default()
        };
        let row_style = self.system.row(visual, bg);
        let row = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        buffer.set_style(row, row_style);
        buffer.set_stringn(
            area.x,
            area.y,
            self.system.glyphs.selection_gutter(),
            1,
            self.system
                .gutter(visual, row_style.bg.unwrap_or(bg), false),
        );

        let mark = self.box_mark(state.value);
        let mark_w = 3u16.min(area.width.saturating_sub(1)).max(1);
        let box_area = Rect::new(area.x.saturating_add(1), area.y, mark_w, 1.min(area.height));
        let mark_style = if !state.enabled {
            row_style
        } else if matches!(state.value, CheckboxValue::Checked) {
            row_style.fg(theme.accent)
        } else {
            row_style.fg(theme.text_muted)
        };
        if area.width > 1 {
            buffer.set_stringn(
                box_area.x,
                box_area.y,
                mark,
                usize::from(box_area.width),
                mark_style,
            );
        }

        // junie: `[✓]` is 3 cells then space then label (label starts col 5).
        let label_x = area.x.saturating_add(5);
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
                row_style,
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
        let _root = Rect::new(area.x, area.y, root_w, root_h);
        // Hit prefers painted content width when single-line without full stretch
        let hit_w = if description_area.is_some() {
            root_w
        } else {
            let content = 5u16.saturating_add(label_area.width);
            content.max(5).min(root_w)
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

    /// The pip an option wears: shape before colour.
    ///
    /// `○` empty → `◎` the cursor is on it but nothing is committed → `●`
    /// chosen. The middle rung existed in the model and was never painted, so
    /// a roving cursor looked identical to a made choice (plans/015 Step 5).
    fn mark(&self, selected: bool, previewed: bool) -> &'static str {
        // junie choice.rs: `(●)` / `( )`. Preview keeps the empty form so
        // FollowFocus (move = select) is the painted truth. No ASCII profile.
        let _ = previewed;
        if selected { "(●)" } else { "( )" }
    }

    fn mark_cols(&self, selected: bool, previewed: bool) -> u16 {
        display_cols(self.mark(selected, previewed)) as u16
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
        take_display_cols(&s, max_cols).into_owned()
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
                let theme = self.system.junie_theme();
                let mut style = theme.label(state.surface_focused).bg(theme.surface);
                if !state.enabled {
                    style = theme.faint().bg(theme.surface);
                }
                if state.invalid {
                    style = style.fg(theme.error);
                }
                let lx = area.x.saturating_add(2);
                let lw = area.right().saturating_sub(lx);
                let text = take_display_cols(leg, usize::from(lw));
                if lw > 0 {
                    buffer.set_stringn(lx, y, &text, usize::from(lw), style);
                }
                legend_rect = Some(Rect::new(
                    lx,
                    y,
                    display_cols(&text).min(usize::from(lw.max(1))) as u16,
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
                    let hovered =
                        state.hovered.as_ref() == Some(&opt.id) && state.enabled && opt.enabled;
                    let theme = self.system.junie_theme();
                    let bg = theme.surface;
                    let visual = VisualState {
                        focused,
                        hovered,
                        selected,
                        disabled: !state.enabled || !opt.enabled,
                        error: state.invalid && selected,
                        ..VisualState::default()
                    };
                    let style = self.system.row(visual, bg);
                    let row = Rect::new(area.x, y, area.width, 1);
                    buffer.set_style(row, style);
                    buffer.set_stringn(
                        area.x,
                        y,
                        self.system.glyphs.selection_gutter(),
                        1,
                        self.system.gutter(visual, style.bg.unwrap_or(bg), false),
                    );
                    let mark = self.mark(selected, focused && !selected);
                    let mark_w = 3u16.min(area.width.saturating_sub(1)).max(1);
                    let mark_area = Rect::new(area.x.saturating_add(1), y, mark_w, 1);
                    let mark_style = if !state.enabled || !opt.enabled {
                        style
                    } else if selected {
                        style.fg(theme.accent)
                    } else {
                        style.fg(theme.text_muted)
                    };
                    if area.width > 1 {
                        buffer.set_stringn(
                            mark_area.x,
                            mark_area.y,
                            mark,
                            usize::from(mark_w),
                            mark_style,
                        );
                    }
                    let label_x = area.x.saturating_add(5);
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
                    let mark = self.mark(selected, focused && !selected);
                    let mark_w = self.mark_cols(selected, focused && !selected);
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

        let root_h = area.height.min(y.saturating_sub(area.y).max(1));
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
        let recipe = self.system.resolve_list_row(ListRowVisualState {
            selected,
            focused,
            hovered,
            enabled: state.enabled && opt.enabled,
            loading: false,
            checked: selected,
            ..ListRowVisualState::default()
        });
        let mut style = recipe.label;
        if selected {
            style = style.add_modifier(Modifier::BOLD);
        }
        if state.invalid && selected {
            style = style.patch(self.system.style(Role::Danger));
        }
        style
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
        if key.is_release() {
            return RadioOutcome::Ignored;
        }
        let items = self.collection_items();
        let _ = state.collection.reconcile(&items);
        if items.iter().all(|e| !e.enabled) {
            return RadioOutcome::Ignored;
        }

        let is_press = key.is_press();

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
        }

        // Explicit cross-axis movement (horizontal groups still use Left/Right)
        if is_press && key.modifiers.is_empty() {
            let before = state.collection.active().cloned();
            match key.code {
                KeyCode::Down | KeyCode::Right | KeyCode::Char('j' | 'J') => {
                    let _ = state.collection.move_next(&items);
                    if state.collection.active() != before.as_ref() {
                        return self.after_cursor_move(state, before);
                    }
                }
                KeyCode::Up | KeyCode::Left | KeyCode::Char('k' | 'K') => {
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
    pub fn handle_intent(&self, state: &mut RadioState<Id>, intent: UiIntent) -> RadioOutcome<Id> {
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
    pub fn handle_mouse(&self, state: &mut RadioState<Id>, event: MouseEvent) -> RadioOutcome<Id> {
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
    fn collection_items(options: &[Id]) -> Vec<crate::interaction::CollectionItem<Id>> {
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

    /// Headless key path using option ids only (all enabled, no labels).
    ///
    /// Prefer [`RadioGroup::handle_key`] for full option metadata (disabled,
    /// typeahead labels, policy with widget).
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> RadioOutcome<Id> {
        if !self.enabled || options.is_empty() || key.is_release() {
            return RadioOutcome::Ignored;
        }
        // Ensure surface focused for headless tests
        self.surface_focused = true;
        let items = Self::collection_items(options);
        let _ = self.collection.reconcile(&items);
        let is_press = key.is_press();
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
        let items = Self::collection_items(options);
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
        if !self.can_activate() || !self.focused || !key.is_press() {
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
        SwitchOutcome::Ignored
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
            return 1;
        }
        3
    }

    fn track_face(&self, state: &SwitchState) -> String {
        if state.loading {
            return self.system.glyphs.loading().to_string();
        }
        if state.on {
            "──●".to_string()
        } else {
            "○──".to_string()
        }
    }

    fn track_style(&self, state: &SwitchState) -> ratatui_core::style::Style {
        let control_state = if !state.enabled {
            ControlState::Disabled
        } else if state.loading {
            ControlState::Loading
        } else if state.focused {
            ControlState::Focused
        } else if state.hovered {
            ControlState::Hovered
        } else {
            ControlState::Default
        };
        let recipe = self.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            control_state,
            self.system.junie_theme().surface,
        );
        let mut style = recipe.fill.patch(recipe.label);
        if state.read_only {
            // Read-only is the disabled fact: say it with the disabled tone,
            // never with a dimmed copy of the active one.
            style = style.patch(self.system.style(Role::TextDisabled));
        }
        if state.invalid {
            style = style.patch(self.system.style(Role::Danger));
        }
        if state.on {
            style = style.patch(self.system.style(Role::Success));
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    fn label_style(&self, state: &SwitchState) -> ratatui_core::style::Style {
        let control_state = if !state.enabled {
            ControlState::Disabled
        } else if state.loading {
            ControlState::Loading
        } else if state.focused {
            ControlState::Focused
        } else if state.hovered {
            ControlState::Hovered
        } else {
            ControlState::Default
        };
        let recipe = self.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            control_state,
            self.system.junie_theme().surface,
        );
        let mut style = recipe.fill.patch(recipe.label);
        if state.read_only {
            // Read-only is the disabled fact: say it with the disabled tone,
            // never with a dimmed copy of the active one.
            style = style.patch(self.system.style(Role::TextDisabled));
        }
        if state.invalid {
            style = style.patch(self.system.style(Role::Danger));
        }
        style
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
                // junie: `▎──● label on`
                let theme = self.system.junie_theme();
                let visual = VisualState {
                    focused: state.focused && state.enabled,
                    hovered: state.hovered && state.enabled && !state.read_only,
                    selected: state.on,
                    disabled: !state.enabled,
                    error: state.invalid,
                    ..VisualState::default()
                };
                let row_style = self.system.row(visual, theme.surface);
                let row = Rect::new(area.x, area.y, area.width, 1.min(area.height));
                buffer.set_style(row, row_style);
                buffer.set_stringn(
                    area.x,
                    area.y,
                    self.system.glyphs.selection_gutter(),
                    1,
                    self.system
                        .gutter(visual, row_style.bg.unwrap_or(theme.surface), false),
                );
                let track = Rect::new(
                    area.x.saturating_add(1),
                    area.y,
                    track_w.min(area.width.saturating_sub(1)),
                    1.min(area.height),
                );
                let face_t = take_display_cols(&face, usize::from(track.width));
                let knob_style = if !state.enabled {
                    row_style
                } else if state.on {
                    row_style.fg(theme.accent)
                } else {
                    row_style.fg(theme.text_muted)
                };
                let _ = track_style;
                buffer.set_stringn(
                    track.x,
                    track.y,
                    &face_t,
                    usize::from(track.width),
                    knob_style,
                );
                let mut label_area = None;
                let lx = area.x.saturating_add(5);
                if lx < area.right() && !self.label.is_empty() {
                    let lw = area.right().saturating_sub(lx);
                    let text = take_display_cols(self.label, usize::from(lw));
                    let compact_label = if state.enabled {
                        row_style.fg(theme.text_primary)
                    } else {
                        label_style
                    };
                    buffer.set_stringn(lx, area.y, &text, usize::from(lw), compact_label);
                    let used = display_cols(&text).min(usize::from(lw)) as u16;
                    label_area = Some(Rect::new(lx, area.y, used, 1));
                    let word = if state.on { "on" } else { "off" };
                    let sx = lx.saturating_add(used).saturating_add(1);
                    if sx + 3 < area.right() {
                        buffer.set_stringn(
                            sx,
                            area.y,
                            word,
                            3,
                            row_style.fg(if state.enabled {
                                theme.text_muted
                            } else {
                                theme.disabled
                            }),
                        );
                    }
                }
                let content_w = 5u16.saturating_add(label_area.map(|r| r.width).unwrap_or(0));
                let root = Rect::new(
                    area.x,
                    area.y,
                    content_w.min(area.width).max(1),
                    1.min(area.height),
                );
                (track, label_area, None, root)
            }
            SwitchRecipe::SettingsRow => {
                // Label .......... [ON ]
                //   description
                let track_x = area.right().saturating_sub(track_w);
                let track = Rect::new(
                    track_x.max(area.x),
                    area.y,
                    track_w.min(area.width),
                    1.min(area.height),
                );
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
                    buffer.set_stringn(area.x, area.y, &text, usize::from(label_max), label_style);
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
                    buffer.set_stringn(
                        area.x.saturating_add(1),
                        dy,
                        &text,
                        usize::from(dw),
                        dstyle,
                    );
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
    fn checkbox_indeterminate_paint_is_catalog_minus_not_a_well() {
        let system = DesignSystem::junie();
        let cb = Checkbox::new("mix", "Mixed", &system);
        let mut state = CheckboxState::with_value(CheckboxValue::Indeterminate);
        let area = Rect::new(0, 0, 16, 1);
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut state);
        let row: String = (0..16).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            !row.contains('['),
            "indeterminate is catalog minus, not [–]: {row:?}"
        );
        assert!(
            row.contains('\u{2212}'),
            "indeterminate uses Glyph::Remove: {row:?}"
        );
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
        let mut state =
            RadioState::new(Some("a")).policy_mode(RadioSelectionPolicy::ActivateToSelect);
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
        let out = g.handle_key(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
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
        let options = [RadioOption::new("a", "A"), RadioOption::new("b", "B")];
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
    fn empty_radio_group_is_safe_and_has_no_pointer_targets() {
        let system = DesignSystem::default();
        let options: [RadioOption<'_, &str>; 0] = [];
        let group = RadioGroup::new(&options, &system);
        let mut state = RadioState::<&str>::new(None);
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        let parts = group.paint(area, &mut buffer, &mut state);

        assert!(parts.options.is_empty());
    }

    #[test]
    fn radio_and_switch_invalid_states_reach_paint() {
        let system = DesignSystem::default();
        let options = [RadioOption::new("a", "Alpha")];
        let mut radio_state = RadioState::new(Some("a"));
        radio_state.set_invalid(true);
        let radio_area = Rect::new(0, 0, 20, 1);
        let mut radio_buffer = Buffer::empty(radio_area);
        let radio_parts = RadioGroup::new(&options, &system).paint(
            radio_area,
            &mut radio_buffer,
            &mut radio_state,
        );

        let mut switch_state = SwitchState::new(false);
        switch_state.set_invalid(true);
        let switch_area = Rect::new(0, 0, 20, 1);
        let mut switch_buffer = Buffer::empty(switch_area);
        let switch_parts = Switch::new("s", "Sync", &system).paint(
            switch_area,
            &mut switch_buffer,
            &mut switch_state,
        );

        assert_eq!(radio_parts.options.len(), 1);
        assert!(!switch_parts.root.is_empty());
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
            SwitchOutcome::ValueChanged {
                id: "dark",
                on: true
            }
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
            x: parts
                .root
                .x
                .saturating_add(parts.root.width)
                .saturating_add(2),
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
    fn switch_compact_recipe() {
        let system = DesignSystem::default();
        let sw = Switch::new("c", "Compact", &system).compact();
        let mut state = SwitchState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 1));
        let parts = sw.paint(Rect::new(0, 0, 24, 1), &mut buf, &mut state);
        assert_eq!(parts.track.x, 1, "knob sits after the focus bar");
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
            Some("▎")
        );
        assert_eq!(
            buf.cell((1, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some(system.glyphs.loading())
        );
    }

    #[test]
    fn radio_marks_are_the_junie_paren_forms() {
        let system = DesignSystem::junie();
        let options = [
            RadioOption::new("a", "Compact"),
            RadioOption::new("b", "Comfortable"),
        ];
        let mut state = RadioState::new(Some("a"));
        state.surface_focused = true;
        let area = Rect::new(0, 0, 30, 3);
        let mut buffer = Buffer::empty(area);
        RadioGroup::new(&options, &system)
            .legend("Density")
            .render(area, &mut buffer, &mut state);

        let rows: Vec<String> = (0..area.height)
            .map(|y| (0..area.width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect();
        let all = rows.join("\n");
        assert!(all.contains("(●)"), "selected: {all:?}");
        assert!(all.contains("( )"), "idle: {all:?}");
        assert!(!all.contains('◎'), "junie has no preview pip: {all:?}");
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

    fn cell(buffer: &Buffer, x: u16, y: u16) -> String {
        buffer[(x, y)].symbol().to_string()
    }

    #[test]
    fn checkbox_anatomy_gutter_mark_label() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let cb = Checkbox::new("n", "Notify", &system);
        let area = Rect::new(0, 0, 24, 1);

        let mut on = CheckboxState::new(true);
        on.set_focused(true);
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut on);
        assert_eq!(cell(&buf, 0, 0), "▎");
        assert_eq!(cell(&buf, 1, 0), "[");
        assert_eq!(cell(&buf, 2, 0), "✓");
        assert_eq!(cell(&buf, 3, 0), "]");
        assert_eq!(cell(&buf, 4, 0), " ");
        assert_eq!(cell(&buf, 5, 0), "N");
        assert_eq!(buf[(2, 0)].fg, theme.accent);

        let mut off = CheckboxState::new(false);
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut off);
        assert_eq!(cell(&buf, 2, 0), " ");
        assert_eq!(buf[(1, 0)].fg, theme.text_muted);

        let mut hover = CheckboxState::new(false);
        hover.hovered = true;
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut hover);
        assert_eq!(buf[(5, 0)].bg, theme.lift(theme.surface));

        let mut disabled = CheckboxState::new(true);
        disabled.set_enabled(false);
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut disabled);
        assert_eq!(buf[(5, 0)].fg, theme.disabled);
        assert_eq!(cell(&buf, 0, 0), "▎");
        assert_eq!(
            buf[(0, 0)].fg,
            buf[(0, 0)].bg,
            "disabled gutter is reserved, fg=bg"
        );

        let mut err = CheckboxState::new(false);
        err.set_invalid(true);
        err.set_focused(true);
        let mut buf = Buffer::empty(area);
        let _ = cb.paint(area, &mut buf, &mut err);
        assert_eq!(buf[(5, 0)].fg, theme.error);
    }

    #[test]
    fn radio_anatomy_and_jk_select() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let options = [
            RadioOption::new("a", "Fast"),
            RadioOption::new("b", "Balanced"),
        ];
        let g = RadioGroup::new(&options, &system).legend("Mode");
        let mut state = RadioState::new(Some("a"));
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 24, 4);
        let mut buf = Buffer::empty(area);
        let _ = g.paint(area, &mut buf, &mut state);
        assert_eq!(cell(&buf, 2, 0), "M");
        assert_eq!(cell(&buf, 0, 1), "▎");
        assert_eq!(cell(&buf, 1, 1), "(");
        assert_eq!(cell(&buf, 2, 1), "●");
        assert_eq!(cell(&buf, 3, 1), ")");
        assert_eq!(cell(&buf, 5, 1), "F");
        assert_eq!(buf[(2, 1)].fg, theme.accent);
        assert_eq!(cell(&buf, 2, 2), " ");
        assert_eq!(cell(&buf, 5, 2), "B");

        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        assert_eq!(out, RadioOutcome::Selected("b"));
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        );
        assert_eq!(out, RadioOutcome::Selected("a"));

        let out = g.handle_key(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(out, RadioOutcome::Ignored);
        assert_eq!(state.selected(), Some(&"a"));

        state.set_enabled(false);
        let mut buf = Buffer::empty(area);
        let _ = g.paint(area, &mut buf, &mut state);
        assert_eq!(buf[(5, 1)].fg, theme.disabled);
    }

    // Select / MultiSelect / Combobox: see dedicated widget modules.
}
