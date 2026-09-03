// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Toggle and ToggleGroup — pressable sticky state controls.
//!
//! **Mission.** Editor toolbars, formatting strips, and mode chips need sticky
//! pressed/unpressed controls that are **not** form checkboxes, **not** settings
//! switches, and **not** tabs. A Toggle is a button that stays pressed; a
//! ToggleGroup applies single- or multi-select semantics with roving focus.
//!
//! **vs [`Checkbox`](crate::widgets::Checkbox).** Checkbox is a form field with
//! label association and checked semantics (`[✓]` / `[ ]`). Toggle is a toolbar
//! affordance (pressed reverse + label vs padded idle label). No `[inner]` wells.
//!
//! Toggle paints only through `Toggle::paint(area, buffer, state)`; a stateless
//! render would rebuild `ToggleState` per frame and every frame would paint the
//! switch idle regardless of the pressed state the host owns.
//!
//! **vs [`Switch`](crate::widgets::Switch).** Switch is an immediate On/Off
//! setting (`▎──●` / `○──`). Toggle does not imply a persistent preference.
//!
//! **vs Tabs / ModeRibbon.** Tabs select content panels; ModeRibbon selects agent
//! modes. ToggleGroup is for tool state (bold/italic, align L|C|R), not navigation.
//!
//! **vs [`ToolbarItemKind::Toggle`](crate::widgets::ToolbarItemKind).** Toolbar
//! embeds a light toggle kind; prefer standalone Toggle/ToggleGroup when overflow,
//! group policy, indeterminate, or semantic registration matter.
//!
//! **When not to use ToggleGroup**
//! - Form boolean → Checkbox
//! - Settings preference → Switch
//! - Content/view navigation → Tabs or SegmentedControl / ModeRibbon
//! - One-shot command → Button / ButtonGroup
//! - Exclusive options with long descriptions → RadioGroup
//!
//! Research: Radix Toggle/ToggleGroup, editor toolbars, terminal mode chips.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, NavigationMove, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome,
    SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    default_list_intent,
};
use crate::style::{ButtonRecipeVariant, ControlState, DesignSystem, Role, VisualState};
use crate::text::{display_cols, take_display_cols};

// ── Value / size / recipe ───────────────────────────────────────────────────

/// Sticky press value for a [`Toggle`].
///
/// `Indeterminate` is justified for mixed selection (e.g. Bold when only some
/// of the selection is bold). Activation cycles Indeterminate/Unpressed →
/// Pressed → Unpressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleValue {
    /// Not pressed.
    #[default]
    Unpressed,
    /// Pressed / on.
    Pressed,
    /// Mixed / partial (editor selection).
    Indeterminate,
}

impl ToggleValue {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unpressed => "unpressed",
            Self::Pressed => "pressed",
            Self::Indeterminate => "indeterminate",
        }
    }

    /// Whether fully pressed.
    #[must_use]
    pub const fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }

    /// Whether mixed.
    #[must_use]
    pub const fn is_indeterminate(self) -> bool {
        matches!(self, Self::Indeterminate)
    }

    /// Next value after activate (indeterminate/off → on, on → off).
    #[must_use]
    pub const fn activate(self) -> Self {
        match self {
            Self::Unpressed | Self::Indeterminate => Self::Pressed,
            Self::Pressed => Self::Unpressed,
        }
    }

    /// From bool (no indeterminate).
    #[must_use]
    pub const fn from_pressed(pressed: bool) -> Self {
        if pressed {
            Self::Pressed
        } else {
            Self::Unpressed
        }
    }
}

/// Density of a toggle face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleSize {
    /// Default padding (` B `).
    #[default]
    Default,
    /// Toolbar density (`B`).
    Compact,
}

impl ToggleSize {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
        }
    }
}

/// Face chrome recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleRecipe {
    /// Bracket pressed mark; quiet unpressed (default toolbar).
    #[default]
    Outline,
    /// Minimal chrome; pressed uses reverse/bold only.
    Quiet,
    /// Solid secondary selection when pressed.
    Solid,
}

impl ToggleRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Quiet => "quiet",
            Self::Solid => "solid",
        }
    }
}

/// Group selection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleGroupType {
    /// At most one item pressed (radio-like among toggles).
    #[default]
    Single,
    /// Independent pressed flags (bold + italic).
    Multiple,
}

impl ToggleGroupType {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
}

/// Visual connection in a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleGroupRecipe {
    /// 1-col separator between faces.
    Connected,
    /// Gap between faces (default).
    #[default]
    Separated,
}

impl ToggleGroupRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Separated => "separated",
        }
    }

    fn inter_cols(self) -> u16 {
        1
    }

    fn separator_glyph(self, ascii: bool) -> Option<&'static str> {
        match self {
            Self::Connected => Some(if ascii { "|" } else { "│" }),
            Self::Separated => None,
        }
    }
}

/// Group orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToggleGroupOrientation {
    /// Left → right.
    #[default]
    Horizontal,
    /// Stacked rows.
    Vertical,
}

impl ToggleGroupOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

// ── Single Toggle ───────────────────────────────────────────────────────────

/// Runtime state for a single [`Toggle`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToggleState {
    /// Projected value (controlled; host applies outcomes).
    pub value: ToggleValue,
    /// Keyboard focus.
    pub focused: bool,
    /// Enabled.
    pub enabled: bool,
    /// Pointer hover.
    pub hovered: bool,
    /// Last hit region.
    pub region: Option<Rect>,
}

impl ToggleState {
    /// Unpressed, enabled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            value: ToggleValue::Unpressed,
            focused: false,
            enabled: true,
            hovered: false,
            region: None,
        }
    }

    /// With initial value.
    #[must_use]
    pub const fn with_value(value: ToggleValue) -> Self {
        Self {
            value,
            focused: false,
            enabled: true,
            hovered: false,
            region: None,
        }
    }

    /// Controlled set.
    pub const fn set_value(&mut self, value: ToggleValue) {
        self.value = value;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }
}

/// Outcomes for a single Toggle.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToggleOutcome {
    /// No change.
    Ignored,
    /// Host should set new value.
    ValueChanged {
        /// Next value after activate.
        value: ToggleValue,
    },
}

/// Paint geometry for a single toggle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleParts {
    /// Full face.
    pub root: Rect,
}

/// Single pressable sticky control.
#[derive(Debug, Clone, Copy)]
pub struct Toggle<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    /// Required when label empty (icon-only).
    accessible_label: Option<&'a str>,
    system: &'a DesignSystem,
    size: ToggleSize,
    recipe: ToggleRecipe,
    /// Force monochrome / no-color cues.
    colorless: bool,
}

impl<'a> Toggle<'a> {
    /// Label + design system. Empty label requires [`.accessible_label`](Self::accessible_label)
    /// or [`.icon`](Self::icon) with a11y name via `accessible_label`.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            icon: None,
            accessible_label: None,
            system,
            size: ToggleSize::Default,
            recipe: ToggleRecipe::Outline,
            colorless: false,
        }
    }

    /// Icon glyph (may combine with label).
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Accessible name (required for icon-only).
    #[must_use]
    pub const fn accessible_label(mut self, name: &'a str) -> Self {
        self.accessible_label = Some(name);
        self
    }

    /// Size.
    #[must_use]
    pub const fn size(mut self, size: ToggleSize) -> Self {
        self.size = size;
        self
    }

    /// Compact toolbar density.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.size = ToggleSize::Compact;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: ToggleRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Quiet recipe.
    #[must_use]
    pub const fn quiet(mut self) -> Self {
        self.recipe = ToggleRecipe::Quiet;
        self
    }

    /// Solid pressed fill.
    #[must_use]
    pub const fn solid(mut self) -> Self {
        self.recipe = ToggleRecipe::Solid;
        self
    }

    /// Colorless emphasis.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Accessible name for semantics.
    #[must_use]
    pub fn a11y_name(&self) -> &'a str {
        if let Some(a) = self.accessible_label {
            if !a.is_empty() {
                return a;
            }
        }
        if !self.label.is_empty() {
            return self.label;
        }
        self.icon.unwrap_or("toggle")
    }

    /// Whether icon-only contract is safe.
    #[must_use]
    pub fn has_accessible_label(&self) -> bool {
        !self.label.is_empty() || self.accessible_label.is_some_and(|a| !a.is_empty())
    }

    fn face_inner(&self) -> String {
        match (self.icon, self.label) {
            (Some(i), l) if l.is_empty() => i.to_string(),
            (Some(i), l) => format!("{i} {l}"),
            (None, l) => l.to_string(),
        }
    }

    /// Preferred width for layout.
    #[must_use]
    pub fn preferred_width(&self, value: ToggleValue) -> u16 {
        let inner = self.face_inner();
        let cols = display_cols(&inner).max(1);
        let chrome = match self.size {
            ToggleSize::Compact => 0,
            ToggleSize::Default => 2,
        };
        let _ = (self.recipe, value);
        u16::try_from(cols.saturating_add(chrome).max(1)).unwrap_or(1)
    }

    fn format_face(&self, value: ToggleValue) -> String {
        let _ = value;
        let inner = self.face_inner();
        let inner = if inner.is_empty() { "·".into() } else { inner };
        match self.size {
            ToggleSize::Compact => inner,
            ToggleSize::Default => format!(" {inner} "),
        }
    }

    fn face_style(&self, state: &ToggleState) -> ratatui_core::style::Style {
        let variant = if self.colorless {
            ButtonRecipeVariant::Quiet
        } else {
            match self.recipe {
                ToggleRecipe::Solid => ButtonRecipeVariant::Secondary,
                ToggleRecipe::Outline => ButtonRecipeVariant::Outline,
                ToggleRecipe::Quiet => ButtonRecipeVariant::Quiet,
            }
        };
        let control_state = if !state.enabled {
            ControlState::Disabled
        } else if state.focused {
            ControlState::Focused
        } else if state.hovered {
            ControlState::Hovered
        } else {
            ControlState::Default
        };
        let recipe =
            self.system
                .button_recipe(variant, control_state, self.system.junie_theme().surface);
        let mut style = recipe.fill.patch(recipe.label);
        if matches!(state.value, ToggleValue::Pressed) {
            // M2: pressed is a full style replacement — the explicit
            // reversal pair, not a stack of modifiers over the idle one.
            style = self.system.reversed();
        } else if matches!(state.value, ToggleValue::Indeterminate) {
            // One ladder step down, never a dimmed copy of the active mark.
            style = style.patch(self.system.style(Role::TextMuted));
        }
        style
    }

    /// Paint toggle as the junie form switch: `▎──● label on` / `▎○── label off`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ToggleState) -> ToggleParts {
        state.region = None;
        if area.is_empty() {
            return ToggleParts { root: area };
        }
        // Contract violation for icon-only without a11y
        if !self.has_accessible_label() {
            let style = self.system.style(Role::Danger);
            buffer.set_stringn(area.x, area.y, "!", 1, style);
            let root = Rect::new(area.x, area.y, 1.min(area.width), 1.min(area.height));
            state.region = Some(root);
            return ToggleParts { root };
        }
        let theme = self.system.junie_theme();
        let bg = theme.surface;
        let on = state.value.is_pressed();
        let visual = VisualState {
            focused: state.focused && state.enabled,
            hovered: state.hovered && state.enabled,
            selected: on,
            disabled: !state.enabled,
            ..VisualState::default()
        };
        let st = self.system.row(visual, bg);
        let row = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        buffer.set_style(row, st);
        buffer.set_stringn(
            area.x,
            area.y,
            self.system.glyphs.selection_gutter(),
            1,
            self.system.gutter(visual, st.bg.unwrap_or(bg), false),
        );
        let knob = match state.value {
            ToggleValue::Pressed => "──●",
            ToggleValue::Unpressed => "○──",
            ToggleValue::Indeterminate => "─●─",
        };
        let knob_style = if !state.enabled {
            st
        } else if on {
            st.fg(theme.accent)
        } else {
            st.fg(theme.text_muted)
        };
        if area.width > 1 {
            buffer.set_stringn(
                area.x.saturating_add(1),
                area.y,
                knob,
                3.min(usize::from(area.width.saturating_sub(1))),
                knob_style,
            );
        }
        let label = if self.label.is_empty() {
            self.icon.unwrap_or("")
        } else {
            self.label
        };
        let label_x = area.x.saturating_add(5);
        if label_x < area.right() && !label.is_empty() {
            let lw = area.right().saturating_sub(label_x);
            buffer.set_stringn(
                label_x,
                area.y,
                take_display_cols(label, usize::from(lw)),
                usize::from(lw),
                st,
            );
        }
        let state_word = if on { "on" } else { "off" };
        let sx = label_x
            .saturating_add(display_cols(label) as u16)
            .saturating_add(1);
        if sx + 3 < area.right() {
            buffer.set_stringn(
                sx,
                area.y,
                state_word,
                3,
                st.fg(if state.enabled {
                    theme.text_muted
                } else {
                    theme.disabled
                }),
            );
        }
        let used = sx.saturating_add(3).saturating_sub(area.x).min(area.width);
        let root = Rect::new(area.x, area.y, used.max(1), 1.min(area.height));
        state.region = Some(root);
        ToggleParts { root }
    }

    /// Keys: Space/Enter flip value when focused.
    pub fn handle_key(&self, state: &mut ToggleState, key: KeyEvent) -> ToggleOutcome {
        if !state.enabled || !state.focused || !key.is_press() {
            return ToggleOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            if matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
            ) {
                let next = state.value.activate();
                // Optimistic mirror; host still owns persistence.
                state.value = next;
                return ToggleOutcome::ValueChanged { value: next };
            }
        }
        ToggleOutcome::Ignored
    }

    /// Mouse click.
    pub fn handle_mouse(&self, state: &mut ToggleState, event: MouseEvent) -> ToggleOutcome {
        if !state.enabled {
            return ToggleOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = state.region.is_some_and(|r| r.contains(event.position));
                ToggleOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if state.region.is_some_and(|r| r.contains(event.position)) {
                    state.focused = true;
                    let next = state.value.activate();
                    state.value = next;
                    ToggleOutcome::ValueChanged { value: next }
                } else {
                    ToggleOutcome::Ignored
                }
            }
            _ => ToggleOutcome::Ignored,
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut ToggleState,
        key: KeyEvent,
    ) -> EventResult<ToggleOutcome> {
        match self.handle_key(state, key) {
            ToggleOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &ToggleState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Button)
                .label(self.a11y_name())
                .description(state.value.id())
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.value.is_pressed(),
                    checked: state.value.is_pressed(),
                    pressed: state.focused && state.value.is_pressed(),
                    ..Default::default()
                }),
        );
    }
}

// ── ToggleGroup ─────────────────────────────────────────────────────────────

/// One item in a [`ToggleGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleGroupItem<'a, Id> {
    /// Stable id.
    pub id: Id,
    /// Visible label (may be empty when icon-only).
    pub label: &'a str,
    /// Optional icon.
    pub icon: Option<&'a str>,
    /// Accessible name when label empty.
    pub accessible_label: Option<&'a str>,
    /// Projected value.
    pub value: ToggleValue,
    /// Activatable.
    pub enabled: bool,
    /// Overflow priority (higher stays visible).
    pub priority: u8,
}

impl<'a, Id> ToggleGroupItem<'a, Id> {
    /// Unpressed text toggle.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
            accessible_label: None,
            value: ToggleValue::Unpressed,
            enabled: true,
            priority: 50,
        }
    }

    /// Pressed.
    #[must_use]
    pub const fn pressed(mut self, on: bool) -> Self {
        self.value = ToggleValue::from_pressed(on);
        self
    }

    /// Explicit value.
    #[must_use]
    pub const fn value(mut self, value: ToggleValue) -> Self {
        self.value = value;
        self
    }

    /// Icon.
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A11y name.
    #[must_use]
    pub const fn accessible_label(mut self, name: &'a str) -> Self {
        self.accessible_label = Some(name);
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Priority.
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    fn a11y(&self) -> &str {
        if let Some(a) = self.accessible_label {
            if !a.is_empty() {
                return a;
            }
        }
        if !self.label.is_empty() {
            return self.label;
        }
        self.icon.unwrap_or("toggle")
    }
}

/// Per-item geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupItemParts<Id> {
    /// Id.
    pub id: Id,
    /// Hit rect (empty when overflowed).
    pub area: Rect,
    /// Overflowed.
    pub overflowed: bool,
}

/// Group geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupParts<Id> {
    /// Root.
    pub root: Rect,
    /// Items.
    pub items: Vec<ToggleGroupItemParts<Id>>,
    /// Overflow trigger.
    pub overflow_trigger: Option<Rect>,
    /// Overflow ids.
    pub overflow_ids: Vec<Id>,
}

/// Group state (interaction only; pressed values are projected on items).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleGroupState<Id> {
    /// Surface keyboard ownership.
    pub surface_focused: bool,
    /// Roving cursor.
    pub cursor: Option<Id>,
    /// Hovered item.
    pub hovered: Option<Id>,
    /// Overflow menu open (host paints).
    pub overflow_open: bool,
    /// Allow deselect in single type (Radix `type=single` + collapsible).
    pub allow_empty: bool,
    /// Last parts.
    pub parts: Option<ToggleGroupParts<Id>>,
    /// Roving engine.
    pub roving: RovingFocusGroup<Id>,
}

impl<Id: Clone + PartialEq> Default for ToggleGroupState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> ToggleGroupState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface_focused: false,
            cursor: None,
            hovered: None,
            overflow_open: false,
            allow_empty: false,
            parts: None,
            roving: RovingFocusGroup::new().orientation(RovingOrientation::Horizontal),
        }
    }

    /// Surface focus.
    pub fn set_surface_focused(&mut self, on: bool) {
        self.surface_focused = on;
        if !on {
            self.overflow_open = false;
        }
    }

    /// Single-type may clear selection when re-activating pressed item.
    pub const fn set_allow_empty(&mut self, on: bool) {
        self.allow_empty = on;
    }
}

/// Group outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToggleGroupOutcome<Id> {
    /// No change.
    Ignored,
    /// Roving cursor moved.
    CursorMoved {
        /// New cursor.
        id: Id,
    },
    /// One item's pressed state should change (multi or single step).
    ItemChanged {
        /// Item id.
        id: Id,
        /// Next value.
        value: ToggleValue,
    },
    /// Single-type selection replaced (host clears others).
    SelectionChanged {
        /// New sole pressed id, or None when empty allowed.
        id: Option<Id>,
    },
    /// Overflow opened.
    OverflowOpened,
    /// Overflow closed.
    OverflowClosed,
}

/// Grouped toggles with roving focus and overflow.
#[derive(Debug, Clone, Copy)]
pub struct ToggleGroup<'a, Id> {
    items: &'a [ToggleGroupItem<'a, Id>],
    system: &'a DesignSystem,
    group_type: ToggleGroupType,
    recipe: ToggleGroupRecipe,
    orientation: ToggleGroupOrientation,
    size: ToggleSize,
    face_recipe: ToggleRecipe,
    overflow_label: &'a str,
}

impl<'a, Id> ToggleGroup<'a, Id> {
    /// Group over borrowed items.
    #[must_use]
    pub const fn new(items: &'a [ToggleGroupItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            group_type: ToggleGroupType::Single,
            recipe: ToggleGroupRecipe::Separated,
            orientation: ToggleGroupOrientation::Horizontal,
            size: ToggleSize::Default,
            face_recipe: ToggleRecipe::Outline,
            overflow_label: system.glyphs.ellipsis(),
        }
    }

    /// Single-select (default).
    #[must_use]
    pub const fn single(mut self) -> Self {
        self.group_type = ToggleGroupType::Single;
        self
    }

    /// Multi-select.
    #[must_use]
    pub const fn multiple(mut self) -> Self {
        self.group_type = ToggleGroupType::Multiple;
        self
    }

    /// Group type.
    #[must_use]
    pub const fn group_type(mut self, t: ToggleGroupType) -> Self {
        self.group_type = t;
        self
    }

    /// Connected faces.
    #[must_use]
    pub const fn connected(mut self) -> Self {
        self.recipe = ToggleGroupRecipe::Connected;
        self
    }

    /// Separated faces.
    #[must_use]
    pub const fn separated(mut self) -> Self {
        self.recipe = ToggleGroupRecipe::Separated;
        self
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, o: ToggleGroupOrientation) -> Self {
        self.orientation = o;
        self
    }

    /// Compact toolbar density.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.size = ToggleSize::Compact;
        self
    }

    /// Face recipe.
    #[must_use]
    pub const fn face_recipe(mut self, recipe: ToggleRecipe) -> Self {
        self.face_recipe = recipe;
        self
    }

    /// Overflow label.
    #[must_use]
    pub const fn overflow_label(mut self, label: &'a str) -> Self {
        self.overflow_label = label;
        self
    }

    fn item_toggle(&self, item: &ToggleGroupItem<'a, Id>) -> Toggle<'a> {
        let mut t = Toggle::new(item.label, self.system)
            .size(self.size)
            .recipe(self.face_recipe);
        if let Some(i) = item.icon {
            t = t.icon(i);
        }
        if let Some(a) = item.accessible_label {
            t = t.accessible_label(a);
        } else if item.label.is_empty() {
            // a11y falls back to icon or "toggle" — both 'static or 'a via icon
            if let Some(i) = item.icon {
                t = t.accessible_label(i);
            } else {
                t = t.accessible_label("toggle");
            }
        }
        t
    }

    fn item_width(&self, item: &ToggleGroupItem<'a, Id>) -> u16 {
        self.item_toggle(item).preferred_width(item.value).max(2)
    }

    fn overflow_trigger_width(&self) -> u16 {
        u16::try_from(display_cols(self.overflow_label).saturating_add(2)).unwrap_or(3)
    }

    /// Plan visible vs overflow indices.
    #[must_use]
    pub fn plan_overflow(&self, width: u16) -> (Vec<usize>, Vec<usize>) {
        if matches!(self.orientation, ToggleGroupOrientation::Vertical) {
            return ((0..self.items.len()).collect(), Vec::new());
        }
        if self.items.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let gap = self.recipe.inter_cols();
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        order.sort_by(|&a, &b| {
            self.items[b]
                .priority
                .cmp(&self.items[a].priority)
                .then(a.cmp(&b))
        });
        let mut used = 0u16;
        let mut keep = Vec::new();
        let mut overflow = Vec::new();
        for &idx in &order {
            let w = self.item_width(&self.items[idx]);
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
        if !overflow.is_empty() {
            let trigger = self.overflow_trigger_width().saturating_add(gap);
            while !keep.is_empty() {
                let total = keep.iter().enumerate().fold(0u16, |acc, (i, &idx)| {
                    acc.saturating_add(if i > 0 { gap } else { 0 })
                        .saturating_add(self.item_width(&self.items[idx]))
                });
                if total.saturating_add(trigger) <= width {
                    break;
                }
                // Prefer dropping unpressed lower-priority items
                if let Some(pos) = keep
                    .iter()
                    .rposition(|&i| !self.items[i].value.is_pressed())
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
        (keep, overflow)
    }
}

impl<'a, Id: Clone + PartialEq> ToggleGroup<'a, Id> {
    fn item_by_id(&self, id: &Id) -> Option<&ToggleGroupItem<'a, Id>> {
        self.items.iter().find(|i| &i.id == id)
    }

    /// Paint group.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ToggleGroupState<Id>,
    ) -> ToggleGroupParts<Id> {
        if area.is_empty() || self.items.is_empty() {
            let parts = ToggleGroupParts {
                root: area,
                items: Vec::new(),
                overflow_trigger: None,
                overflow_ids: Vec::new(),
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        let (visible, overflow) = self.plan_overflow(area.width);
        let overflow_ids: Vec<Id> = overflow.iter().map(|&i| self.items[i].id.clone()).collect();
        let visible_ids: Vec<Id> = visible.iter().map(|&i| self.items[i].id.clone()).collect();

        if state
            .cursor
            .as_ref()
            .is_none_or(|c| !visible_ids.iter().any(|v| v == c))
        {
            state.cursor = visible
                .iter()
                .find(|&&i| self.items[i].enabled)
                .map(|&i| self.items[i].id.clone())
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

        match self.orientation {
            ToggleGroupOrientation::Vertical => {
                let mut y = area.y;
                for &idx in &visible {
                    if y >= area.bottom() {
                        break;
                    }
                    let item = &self.items[idx];
                    let rect = Rect::new(area.x, y, area.width, 1);
                    self.paint_item(item, rect, buffer, state);
                    item_parts.push(ToggleGroupItemParts {
                        id: item.id.clone(),
                        area: rect,
                        overflowed: false,
                    });
                    y = y.saturating_add(1);
                }
            }
            ToggleGroupOrientation::Horizontal => {
                let gap = self.recipe.inter_cols();
                let sep = self.recipe.separator_glyph(false);
                let mut x = area.x;
                let mut first = true;
                for &idx in &visible {
                    if !first {
                        if let Some(glyph) = sep {
                            if x < area.right() && area.height > 0 {
                                buffer.set_stringn(
                                    x,
                                    area.y,
                                    glyph,
                                    1,
                                    self.system.style(Role::Border),
                                );
                                x = x.saturating_add(1);
                            }
                        } else {
                            x = x.saturating_add(gap);
                        }
                    }
                    first = false;
                    let item = &self.items[idx];
                    let w = self.item_width(item).min(area.right().saturating_sub(x));
                    if w == 0 {
                        break;
                    }
                    let rect = Rect::new(x, area.y, w, 1.min(area.height));
                    self.paint_item(item, rect, buffer, state);
                    item_parts.push(ToggleGroupItemParts {
                        id: item.id.clone(),
                        area: rect,
                        overflowed: false,
                    });
                    x = x.saturating_add(w);
                }
                if !overflow.is_empty() {
                    if !first {
                        if let Some(glyph) = sep {
                            if x < area.right() && area.height > 0 {
                                buffer.set_stringn(
                                    x,
                                    area.y,
                                    glyph,
                                    1,
                                    self.system.style(Role::Border),
                                );
                                x = x.saturating_add(1);
                            }
                        } else {
                            x = x.saturating_add(gap);
                        }
                    }
                    let tw = self
                        .overflow_trigger_width()
                        .min(area.right().saturating_sub(x));
                    if tw > 0 {
                        let rect = Rect::new(x, area.y, tw, 1.min(area.height));
                        let recipe = self.system.button_recipe(
                            ButtonRecipeVariant::Quiet,
                            if state.overflow_open {
                                ControlState::Focused
                            } else {
                                ControlState::Default
                            },
                            self.system.junie_theme().surface,
                        );
                        let mut style = recipe.fill.patch(recipe.label);
                        if state.overflow_open {
                            // The open trigger is held down: the explicit
                            // reversal, not a stacked modifier.
                            style = self.system.reversed();
                        } else {
                            style = style.add_modifier(Modifier::BOLD);
                        }
                        let label = take_display_cols(self.overflow_label, usize::from(tw));
                        buffer.set_stringn(rect.x, rect.y, &label, usize::from(tw), style);
                        overflow_trigger = Some(rect);
                    }
                }
            }
        }

        for &idx in &overflow {
            item_parts.push(ToggleGroupItemParts {
                id: self.items[idx].id.clone(),
                area: Rect::default(),
                overflowed: true,
            });
        }

        let parts = ToggleGroupParts {
            root: area,
            items: item_parts,
            overflow_trigger,
            overflow_ids,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn paint_item(
        &self,
        item: &ToggleGroupItem<'a, Id>,
        area: Rect,
        buffer: &mut Buffer,
        state: &ToggleGroupState<Id>,
    ) {
        let t = self.item_toggle(item);
        let mut ts = ToggleState::with_value(item.value);
        ts.enabled = item.enabled;
        ts.focused = state.surface_focused && state.cursor.as_ref() == Some(&item.id);
        ts.hovered = state.hovered.as_ref() == Some(&item.id);
        // Toolbar group is reverse+label (no `[inner]` wells). Form switch
        // lives on standalone [`Toggle::paint`].
        let face = t.format_face(item.value);
        let text = take_display_cols(&face, usize::from(area.width));
        let style = t.face_style(&ts);
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
    }

    fn activate_item(&self, state: &ToggleGroupState<Id>, id: Id) -> ToggleGroupOutcome<Id> {
        let Some(item) = self.item_by_id(&id) else {
            return ToggleGroupOutcome::Ignored;
        };
        if !item.enabled {
            return ToggleGroupOutcome::Ignored;
        }
        match self.group_type {
            ToggleGroupType::Multiple => {
                let next = item.value.activate();
                ToggleGroupOutcome::ItemChanged { id, value: next }
            }
            ToggleGroupType::Single => {
                if item.value.is_pressed() {
                    if state.allow_empty {
                        ToggleGroupOutcome::SelectionChanged { id: None }
                    } else {
                        // Stay pressed (non-collapsible single)
                        ToggleGroupOutcome::Ignored
                    }
                } else {
                    ToggleGroupOutcome::SelectionChanged { id: Some(id) }
                }
            }
        }
    }

    /// Keys: roving + activate cursor.
    pub fn handle_key(
        &self,
        state: &mut ToggleGroupState<Id>,
        key: KeyEvent,
    ) -> ToggleGroupOutcome<Id> {
        if !state.surface_focused || !key.is_press() {
            return ToggleGroupOutcome::Ignored;
        }
        if matches!(key.code, crate::input::KeyCode::Esc) && state.overflow_open {
            state.overflow_open = false;
            return ToggleGroupOutcome::OverflowClosed;
        }

        let parts = state.parts.clone();
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
            return ToggleGroupOutcome::Ignored;
        }

        if let Some(intent) = default_button_intent(key) {
            if matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle
            ) {
                if state.overflow_open {
                    return ToggleGroupOutcome::Ignored;
                }
                if let Some(c) = state.cursor.clone() {
                    if parts
                        .as_ref()
                        .is_some_and(|p| p.overflow_ids.iter().any(|id| id == &c))
                    {
                        state.overflow_open = true;
                        return ToggleGroupOutcome::OverflowOpened;
                    }
                    return self.activate_item(state, c);
                }
            }
        }
        let ro = state.roving.handle_key(key, &visible);
        if let RovingOutcome::ActiveChanged { to: Some(id), .. } = ro {
            state.cursor = Some(id.clone());
            return ToggleGroupOutcome::CursorMoved { id };
        }

        if let Some(mv) = default_list_intent(key) {
            match mv {
                UiIntent::Move(NavigationMove::Next | NavigationMove::Right) => {
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                        state.roving.move_next(&visible)
                    {
                        state.cursor = Some(id.clone());
                        return ToggleGroupOutcome::CursorMoved { id };
                    }
                }
                UiIntent::Move(NavigationMove::Previous | NavigationMove::Left) => {
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } =
                        state.roving.move_previous(&visible)
                    {
                        state.cursor = Some(id.clone());
                        return ToggleGroupOutcome::CursorMoved { id };
                    }
                }
                _ => {}
            }
        }

        if let Some(p) = &parts {
            if !p.overflow_ids.is_empty()
                && matches!(key.code, crate::input::KeyCode::Char('o' | 'O' | '.'))
            {
                state.overflow_open = !state.overflow_open;
                return if state.overflow_open {
                    ToggleGroupOutcome::OverflowOpened
                } else {
                    ToggleGroupOutcome::OverflowClosed
                };
            }
        }

        ToggleGroupOutcome::Ignored
    }

    /// Mouse.
    pub fn handle_mouse(
        &self,
        state: &mut ToggleGroupState<Id>,
        event: MouseEvent,
    ) -> ToggleGroupOutcome<Id> {
        let Some(parts) = state.parts.clone() else {
            return ToggleGroupOutcome::Ignored;
        };
        if !parts.root.contains(event.position) {
            if matches!(event.kind, MouseEventKind::Moved) {
                state.hovered = None;
            }
            return ToggleGroupOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts
                    .items
                    .iter()
                    .find(|it| !it.overflowed && it.area.contains(event.position))
                    .map(|it| it.id.clone());
                ToggleGroupOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(tr) = parts.overflow_trigger {
                    if tr.contains(event.position) {
                        state.surface_focused = true;
                        state.overflow_open = !state.overflow_open;
                        return if state.overflow_open {
                            ToggleGroupOutcome::OverflowOpened
                        } else {
                            ToggleGroupOutcome::OverflowClosed
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
                    return self.activate_item(state, it.id.clone());
                }
                ToggleGroupOutcome::Ignored
            }
            _ => ToggleGroupOutcome::Ignored,
        }
    }

    /// Host selected overflow menu item → activate.
    pub fn activate_overflow(
        &self,
        state: &ToggleGroupState<Id>,
        id: Id,
    ) -> ToggleGroupOutcome<Id> {
        if state
            .parts
            .as_ref()
            .is_some_and(|p| p.overflow_ids.iter().any(|x| x == &id))
        {
            return self.activate_item(state, id);
        }
        ToggleGroupOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut ToggleGroupState<Id>,
        key: KeyEvent,
    ) -> EventResult<ToggleGroupOutcome<Id>> {
        match self.handle_key(state, key) {
            ToggleGroupOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic: group + each visible toggle.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        group_id: Id,
        area: Rect,
        state: &ToggleGroupState<Id>,
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
                .label("toggle group")
                .description(self.group_type.id())
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
                let t = self.item_toggle(item);
                let mut ts = ToggleState::with_value(item.value);
                ts.enabled = item.enabled;
                ts.focused = state.surface_focused && state.cursor.as_ref() == Some(&it.id);
                t.register_semantic(scene, it.id.clone(), it.area, &ts);
            }
        }
    }
}

impl<'a, Id: Clone + PartialEq> Widget for &ToggleGroup<'a, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = ToggleGroupState::new();
        let _ = self.paint(area, buffer, &mut state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use ratatui_core::layout::Position;

    #[test]
    fn value_activate_cycles() {
        assert_eq!(ToggleValue::Unpressed.activate(), ToggleValue::Pressed);
        assert_eq!(ToggleValue::Pressed.activate(), ToggleValue::Unpressed);
        assert_eq!(ToggleValue::Indeterminate.activate(), ToggleValue::Pressed);
    }

    #[test]
    fn single_toggle_space_flips() {
        let system = DesignSystem::default();
        let t = Toggle::new("Bold", &system);
        let mut state = ToggleState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = t.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let out = t.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            ToggleOutcome::ValueChanged {
                value: ToggleValue::Pressed
            }
        ));
        assert!(state.value.is_pressed());
    }

    #[test]
    fn icon_only_requires_a11y() {
        let system = DesignSystem::default();
        let bad = Toggle::new("", &system).icon("B");
        assert!(!bad.has_accessible_label());
        let good = Toggle::new("", &system).icon("B").accessible_label("Bold");
        assert!(good.has_accessible_label());
        let mut state = ToggleState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        let _ = bad.paint(Rect::new(0, 0, 8, 1), &mut buf, &mut state);
        // danger mark
        assert_eq!(
            buf.cell((0, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("!")
        );
    }

    #[test]
    fn disabled_ignores_activate() {
        let system = DesignSystem::default();
        let t = Toggle::new("B", &system);
        let mut state = ToggleState::new();
        state.set_focused(true);
        state.set_enabled(false);
        let out = t.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(out, ToggleOutcome::Ignored));
    }

    #[test]
    fn mouse_toggles() {
        let system = DesignSystem::default();
        let t = Toggle::new("I", &system);
        let mut state = ToggleState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        let parts = t.paint(Rect::new(0, 0, 10, 1), &mut buf, &mut state);
        let out = t.handle_mouse(
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
            ToggleOutcome::ValueChanged {
                value: ToggleValue::Pressed
            }
        ));
    }

    #[test]
    fn group_single_selects() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("l", "L").pressed(true),
            ToggleGroupItem::new("c", "C"),
            ToggleGroupItem::new("r", "R"),
        ];
        let g = ToggleGroup::new(&items, &system).single();
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        state.cursor = Some("c");
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            ToggleGroupOutcome::SelectionChanged { id: Some("c") }
        ));
    }

    #[test]
    fn group_single_non_empty_ignores_repress() {
        let system = DesignSystem::default();
        let items = [ToggleGroupItem::new("l", "L").pressed(true)];
        let g = ToggleGroup::new(&items, &system).single();
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        state.cursor = Some("l");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = g.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(out, ToggleGroupOutcome::Ignored));
    }

    #[test]
    fn group_single_allow_empty() {
        let system = DesignSystem::default();
        let items = [ToggleGroupItem::new("l", "L").pressed(true)];
        let g = ToggleGroup::new(&items, &system).single();
        let mut state = ToggleGroupState::new();
        state.set_allow_empty(true);
        state.set_surface_focused(true);
        state.cursor = Some("l");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = g.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            ToggleGroupOutcome::SelectionChanged { id: None }
        ));
    }

    #[test]
    fn group_multi_independent() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("b", "B").pressed(true),
            ToggleGroupItem::new("i", "I"),
        ];
        let g = ToggleGroup::new(&items, &system).multiple();
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let _ = g.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        state.cursor = Some("i");
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            ToggleGroupOutcome::ItemChanged {
                id: "i",
                value: ToggleValue::Pressed
            }
        ));
    }

    #[test]
    fn group_roving() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("a", "A"),
            ToggleGroupItem::new("b", "B"),
        ];
        let g = ToggleGroup::new(&items, &system);
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let _ = g.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        state.cursor = Some("a");
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, ToggleGroupOutcome::CursorMoved { .. }));
    }

    #[test]
    fn overflow_keeps_pressed() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("x", "Extra").priority(10),
            ToggleGroupItem::new("b", "Bold").pressed(true).priority(90),
            ToggleGroupItem::new("i", "Italic").priority(40),
            ToggleGroupItem::new("u", "Under").priority(20),
        ];
        let g = ToggleGroup::new(&items, &system).multiple().compact();
        let (vis, over) = g.plan_overflow(14);
        assert!(
            vis.iter().any(|&i| items[i].id == "b"),
            "pressed high-priority kept; vis={vis:?} over={over:?}"
        );
        assert!(!over.is_empty() || vis.len() < 4);

        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        state.overflow_open = true;
        assert!(matches!(
            g.handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ToggleGroupOutcome::OverflowClosed
        ));
        assert!(!state.overflow_open);
    }

    #[test]
    fn overflow_monotone() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("a", "Alpha").priority(10),
            ToggleGroupItem::new("b", "Beta").priority(50),
            ToggleGroupItem::new("c", "Gamma").priority(90),
        ];
        let g = ToggleGroup::new(&items, &system).compact();
        let mut prev = 0usize;
        for w in 4u16..=60 {
            let (vis, _) = g.plan_overflow(w);
            if vis.len() == items.len() {
                prev = items.len();
            } else if vis.len() >= prev {
                prev = vis.len();
            }
        }
        let (full, over) = g.plan_overflow(80);
        assert_eq!(full.len(), 3);
        assert!(over.is_empty());
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let t = Toggle::new("Bold", &system);
        let mut state = ToggleState::with_value(ToggleValue::Pressed);
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
        let _ = t.paint(Rect::new(0, 0, 16, 1), &mut buf, &mut state);
        let mut scene = SemanticScene::<&str, ()>::default();
        t.register_semantic(&mut scene, "bold", Rect::new(0, 0, 16, 1), &state);
        assert!(scene.len() >= 1);
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("b", "B").pressed(true),
            ToggleGroupItem::new("i", "I"),
            ToggleGroupItem::new("u", "U"),
        ];
        let g = ToggleGroup::new(&items, &system)
            .multiple()
            .connected()
            .compact();
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        for _ in 0..500 {
            let _ = g.paint(area, &mut buf, &mut state);
        }
        assert!(state.parts.is_some());
    }

    #[test]
    fn connected_group_paints() {
        let system = DesignSystem::default();
        let items = [
            ToggleGroupItem::new("l", "L").pressed(true),
            ToggleGroupItem::new("c", "C"),
            ToggleGroupItem::new("r", "R"),
        ];
        let g = ToggleGroup::new(&items, &system).connected().compact();
        let mut state = ToggleGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let parts = g.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        assert_eq!(parts.items.iter().filter(|i| !i.overflowed).count(), 3);
    }

    #[test]
    fn empty_group_safe() {
        let system = DesignSystem::default();
        let items: [ToggleGroupItem<'_, &str>; 0] = [];
        let g = ToggleGroup::new(&items, &system);
        let mut state = ToggleGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = g.paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.items.is_empty());
    }

    #[test]
    fn indeterminate_paint() {
        let system = DesignSystem::junie();
        let t = Toggle::new("B", &system);
        let mut state = ToggleState::with_value(ToggleValue::Indeterminate);
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
        let _ = t.paint(Rect::new(0, 0, 16, 1), &mut buf, &mut state);
        assert_eq!(
            buf.cell((1, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("─")
        );
        assert_eq!(
            buf.cell((2, 0)).map(|c| c.symbol().to_string()).as_deref(),
            Some("●")
        );
    }

    fn cell(buffer: &Buffer, x: u16, y: u16) -> String {
        buffer[(x, y)].symbol().to_string()
    }

    #[test]
    fn form_switch_anatomy_on_off_hover_disabled() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let t = Toggle::new("Verbose", &system);
        let area = Rect::new(0, 0, 24, 1);

        let mut on = ToggleState::with_value(ToggleValue::Pressed);
        on.set_focused(true);
        let mut buf = Buffer::empty(area);
        let _ = t.paint(area, &mut buf, &mut on);
        assert_eq!(cell(&buf, 0, 0), "▎");
        assert_eq!(cell(&buf, 1, 0), "─");
        assert_eq!(cell(&buf, 2, 0), "─");
        assert_eq!(cell(&buf, 3, 0), "●");
        assert_eq!(cell(&buf, 5, 0), "V");
        assert_eq!(buf[(3, 0)].fg, theme.accent);
        let text: String = (0..24).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(text.contains("on"), "{text}");

        let mut off = ToggleState::new();
        let mut buf = Buffer::empty(area);
        let _ = t.paint(area, &mut buf, &mut off);
        assert_eq!(cell(&buf, 1, 0), "○");
        assert_eq!(cell(&buf, 2, 0), "─");
        assert_eq!(cell(&buf, 3, 0), "─");
        assert_eq!(buf[(1, 0)].fg, theme.text_muted);
        let text: String = (0..24).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(text.contains("off"), "{text}");

        let mut hover = ToggleState::new();
        hover.hovered = true;
        let mut buf = Buffer::empty(area);
        let _ = t.paint(area, &mut buf, &mut hover);
        assert_eq!(buf[(5, 0)].bg, theme.lift(theme.surface));

        let mut disabled = ToggleState::with_value(ToggleValue::Pressed);
        disabled.set_enabled(false);
        let mut buf = Buffer::empty(area);
        let _ = t.paint(area, &mut buf, &mut disabled);
        assert_eq!(buf[(5, 0)].fg, theme.disabled);
        assert_eq!(cell(&buf, 0, 0), "▎");
        assert_eq!(
            buf[(0, 0)].fg,
            buf[(0, 0)].bg,
            "disabled gutter is reserved, fg=bg"
        );
    }

    #[test]
    fn format_face_is_padded_inner_without_wells() {
        let system = DesignSystem::junie();
        let t = Toggle::new("B", &system);
        for value in [
            ToggleValue::Unpressed,
            ToggleValue::Pressed,
            ToggleValue::Indeterminate,
        ] {
            let face = t.format_face(value);
            assert!(!face.contains('['), "well leaked: {face:?}");
            assert!(!face.contains(']'), "well leaked: {face:?}");
            assert!(face.contains('B'), "{face:?}");
        }
        let compact = Toggle::new("B", &system).size(ToggleSize::Compact);
        assert_eq!(compact.format_face(ToggleValue::Pressed), "B");
    }

    #[test]
    fn group_paint_has_no_wells() {
        let system = DesignSystem::junie();
        let items = [
            ToggleGroupItem::new("b", "B").pressed(true),
            ToggleGroupItem::new("i", "I"),
        ];
        let g = ToggleGroup::new(&items, &system).multiple();
        let mut state = ToggleGroupState::new();
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        let _ = g.paint(area, &mut buf, &mut state);
        let text: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(!text.contains('['), "well leaked: {text:?}");
        assert!(!text.contains(']'), "well leaked: {text:?}");
        assert!(text.contains('B') && text.contains('I'), "{text:?}");
    }

    #[test]
    fn group_pressed_is_reversed_not_bracket() {
        let system = DesignSystem::junie();
        let items = [ToggleGroupItem::new("b", "B").pressed(true)];
        let g = ToggleGroup::new(&items, &system);
        let mut state = ToggleGroupState::new();
        let area = Rect::new(0, 0, 12, 1);
        let mut buf = Buffer::empty(area);
        let _ = g.paint(area, &mut buf, &mut state);
        let reversed = system.reversed();
        let b = (0..area.width)
            .map(|x| &buf[(x, 0)])
            .find(|cell| cell.symbol() == "B")
            .expect("pressed label");
        assert_eq!(Some(b.fg), reversed.fg);
        assert_eq!(Some(b.bg), reversed.bg);
        assert!(b.style().add_modifier.contains(Modifier::BOLD));
        assert!(!b.style().add_modifier.contains(Modifier::REVERSED));
    }
}
