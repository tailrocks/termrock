// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Controlled form controls: checkbox, radio, switch, select, multiselect, combobox (Plan 051).

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

// ── Checkbox ────────────────────────────────────────────────────────────────

/// Checkbox outcome (controlled: consumer applies value).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckboxOutcome<Id> {
    /// No change.
    Ignored,
    /// Requested checked value for id.
    ValueChanged {
        /// Field id.
        id: Id,
        /// New checked state.
        checked: bool,
    },
}

/// Checkbox state (holds projected checked + focus; value changes via outcomes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CheckboxState {
    checked: bool,
    focused: bool,
    enabled: bool,
    region: Option<Rect>,
}

impl CheckboxState {
    /// Initial checked.
    #[must_use]
    pub const fn new(checked: bool) -> Self {
        Self {
            checked,
            focused: false,
            enabled: true,
            region: None,
        }
    }

    #[must_use]
    /// Checked projection.
    pub const fn is_checked(&self) -> bool {
        self.checked
    }

    /// Controlled set (consumer after outcome).
    pub const fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Toggle when focused and enabled.
    pub fn handle_key<Id: Clone>(&mut self, key: KeyEvent, id: &Id) -> CheckboxOutcome<Id> {
        if !self.enabled || !self.focused || key.kind != KeyEventKind::Press {
            return CheckboxOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                let next = !self.checked;
                // Controlled: do not mutate until consumer applies — but for UX we
                // optimistically mirror; consumer still owns persistence.
                self.checked = next;
                CheckboxOutcome::ValueChanged {
                    id: id.clone(),
                    checked: next,
                }
            }
            _ => CheckboxOutcome::Ignored,
        }
    }

    /// Click toggles.
    pub fn handle_mouse<Id: Clone>(&mut self, event: MouseEvent, id: &Id) -> CheckboxOutcome<Id> {
        if !self.enabled || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return CheckboxOutcome::Ignored;
        }
        if self.region.is_some_and(|r| r.contains(event.position)) {
            let next = !self.checked;
            self.checked = next;
            CheckboxOutcome::ValueChanged {
                id: id.clone(),
                checked: next,
            }
        } else {
            CheckboxOutcome::Ignored
        }
    }
}

/// Checkbox widget.
#[derive(Debug, Clone, Copy)]
pub struct Checkbox<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Label.
    label: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a, Id> Checkbox<'a, Id> {
    /// Id + label.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, tokens: &'a DesignSystem) -> Self {
        Self { id, label, tokens }
    }
}

impl<Id> Checkbox<'_, Id> {
    /// Paint checkbox.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut CheckboxState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let mark = if state.checked { "[x]" } else { "[ ]" };
        let style = if !state.enabled {
            self.tokens.style(Role::TextDisabled)
        } else if state.focused {
            self.tokens.style(Role::Focus)
        } else {
            self.tokens.style(Role::Text)
        };
        let line = format!("{mark} {}", self.label);
        let text = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.region = Some(Rect::new(
            area.x,
            area.y,
            display_cols(&text).min(usize::from(area.width)) as u16,
            1,
        ));
    }
}

impl<Id> StatefulWidget for Checkbox<'_, Id> {
    type State = CheckboxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Checkbox::render(&self, area, buffer, state);
    }
}

impl<Id> StatefulWidget for &Checkbox<'_, Id> {
    type State = CheckboxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Checkbox::render(self, area, buffer, state);
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

/// Radio group state: selected option + focus index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioState<Id> {
    selected: Option<Id>,
    focus_index: usize,
    enabled: bool,
    regions: Vec<Rect>,
}

impl<Id: Clone + PartialEq> RadioState<Id> {
    /// Empty selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            focus_index: 0,
            enabled: true,
            regions: Vec::new(),
        }
    }

    #[must_use]
    /// Selected id.
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Controlled select.
    pub fn set_selected(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    /// Arrow/tab roving + space/enter select.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[Id]) -> RadioOutcome<Id>
    where
        Id: Clone,
    {
        if !self.enabled || options.is_empty() || key.kind == KeyEventKind::Release {
            return RadioOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        match key.code {
            KeyCode::Down | KeyCode::Right | KeyCode::Tab
                if !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.focus_index = (self.focus_index + 1) % options.len();
                RadioOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                self.focus_index = self.focus_index.checked_sub(1).unwrap_or(options.len() - 1);
                RadioOutcome::Ignored
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus_index = self.focus_index.checked_sub(1).unwrap_or(options.len() - 1);
                RadioOutcome::Ignored
            }
            KeyCode::Enter | KeyCode::Char(' ') if is_press => {
                let id = options[self.focus_index].clone();
                self.selected = Some(id.clone());
                RadioOutcome::Selected(id)
            }
            _ => RadioOutcome::Ignored,
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
        for (i, (id, label)) in self.options.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let selected = state.selected.as_ref() == Some(id);
            let focused = state.focus_index == i;
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
                checked: true
            }
        ));
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
    fn radio_roving_and_select() {
        let opts = ["a", "b", "c"];
        let mut state = RadioState::new(None);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &opts);
        assert_eq!(state.focus_index, 1);
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
