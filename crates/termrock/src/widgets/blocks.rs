// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Source-ownable application block state machines (Plan 053).
//!
//! Blocks compose public TermRock APIs only. Domain data, I/O, and effects stay
//! consumer-owned and surface as typed outcomes.

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::DesignTokens,
    widgets::{
        data_table::{DataTableOutcome, DataTableState},
        menu_nav::{SidebarOutcome, SidebarState},
        review::{LogStreamOutcome, LogStreamState, ObjectInspectorState},
        scroll_area::ScrollAreaState,
    },
};

// ── OpsDashboard ────────────────────────────────────────────────────────────

/// Ops dashboard outcomes (never execute domain effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpsDashboardOutcome<RowId, ColId> {
    /// No change.
    Ignored,
    /// Focus region changed.
    FocusRegion(OpsRegion),
    /// Table interaction bubbled.
    Table(DataTableOutcome<RowId, ColId>),
    /// Log interaction.
    Log(LogStreamOutcome),
    /// Request time-range change (consumer applies).
    TimeRangeRequested,
    /// Retry failed load (consumer).
    RetryRequested,
}

/// Focusable regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OpsRegion {
    /// Metrics strip.
    Metrics,
    /// Main table.
    #[default]
    Main,
    /// Log stream.
    Log,
    /// Status.
    Status,
}

/// Controlled ops dashboard chrome state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsDashboardState<RowId: Clone + Ord, ColId: Clone + PartialEq> {
    /// Focused region.
    pub region: OpsRegion,
    /// Table state.
    pub table: DataTableState<RowId, ColId>,
    /// Log state.
    pub log: LogStreamState,
    /// Inspector optional.
    pub inspector: ObjectInspectorState,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> Default for OpsDashboardState<RowId, ColId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> OpsDashboardState<RowId, ColId> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            region: OpsRegion::Main,
            table: DataTableState::new(),
            log: LogStreamState::new(),
            inspector: ObjectInspectorState::new(),
        }
    }

    /// Tab cycles regions; region keys route to child.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        visible_rows: &[RowId],
        columns: &crate::widgets::data_view::ColumnModel<ColId>,
    ) -> OpsDashboardOutcome<RowId, ColId> {
        if key.kind != KeyEventKind::Press {
            return OpsDashboardOutcome::Ignored;
        }
        if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
            self.region = match self.region {
                OpsRegion::Metrics => OpsRegion::Main,
                OpsRegion::Main => OpsRegion::Log,
                OpsRegion::Log => OpsRegion::Status,
                OpsRegion::Status => OpsRegion::Metrics,
            };
            return OpsDashboardOutcome::FocusRegion(self.region);
        }
        if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return OpsDashboardOutcome::RetryRequested;
        }
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return OpsDashboardOutcome::TimeRangeRequested;
        }
        match self.region {
            OpsRegion::Main => {
                OpsDashboardOutcome::Table(self.table.handle_key(key, visible_rows, columns))
            }
            OpsRegion::Log => OpsDashboardOutcome::Log(self.log.handle_key(key)),
            _ => OpsDashboardOutcome::Ignored,
        }
    }
}

// ── ResourceBrowser ─────────────────────────────────────────────────────────

/// Resource browser outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceBrowserOutcome<Id> {
    /// No change.
    Ignored,
    /// Sidebar selection.
    Sidebar(SidebarOutcome<Id>),
    /// Request load of selection (consumer).
    LoadRequested(Id),
    /// Open preview.
    PreviewRequested(Id),
}

/// Resource browser state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBrowserState<Id: Clone + PartialEq> {
    /// Sidebar.
    pub sidebar: SidebarState<Id>,
    /// List scroll.
    pub list_scroll: ScrollAreaState,
    /// Generation for stale preview guard.
    pub selection_generation: u64,
}

impl<Id: Clone + PartialEq> ResourceBrowserState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar: SidebarState::new(None),
            list_scroll: ScrollAreaState::new(),
            selection_generation: 0,
        }
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        items: &[crate::widgets::menu_nav::SidebarItem<Id>],
    ) -> ResourceBrowserOutcome<Id> {
        let out = self.sidebar.handle_key(key, items);
        match out {
            SidebarOutcome::Selected(id) => {
                self.selection_generation = self.selection_generation.saturating_add(1);
                ResourceBrowserOutcome::LoadRequested(id)
            }
            other => ResourceBrowserOutcome::Sidebar(other),
        }
    }
}

impl<Id: Clone + PartialEq> Default for ResourceBrowserState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

// ── SettingsShell ───────────────────────────────────────────────────────────

/// Settings shell outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SettingsShellOutcome<SectionId> {
    /// No change.
    Ignored,
    /// Section selected.
    SectionSelected(SectionId),
    /// Save requested.
    SaveRequested,
    /// Reset section.
    ResetRequested,
    /// Discard dirty.
    DiscardRequested,
    /// Search query changed (consumer filters).
    SearchChanged,
}

/// Settings shell state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsShellState<SectionId: Clone + PartialEq> {
    /// Selected section.
    pub section: Option<SectionId>,
    /// Dirty flag projection.
    pub dirty: bool,
    /// Search text.
    pub search: String,
    /// Focus in search field.
    pub search_focused: bool,
}

impl<SectionId: Clone + PartialEq> SettingsShellState<SectionId> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            section: None,
            dirty: false,
            search: String::new(),
            search_focused: false,
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsShellOutcome<SectionId> {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return SettingsShellOutcome::Ignored;
        }
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return SettingsShellOutcome::SaveRequested;
        }
        if self.search_focused {
            match key.code {
                KeyCode::Esc => {
                    self.search_focused = false;
                    SettingsShellOutcome::Ignored
                }
                KeyCode::Backspace => {
                    self.search.pop();
                    SettingsShellOutcome::SearchChanged
                }
                KeyCode::Char(c) if !c.is_control() && key.modifiers.is_empty() => {
                    self.search.push(c);
                    SettingsShellOutcome::SearchChanged
                }
                _ => SettingsShellOutcome::Ignored,
            }
        } else {
            SettingsShellOutcome::Ignored
        }
    }

    /// Select section (controlled).
    pub fn select_section(&mut self, id: SectionId) -> SettingsShellOutcome<SectionId> {
        self.section = Some(id.clone());
        SettingsShellOutcome::SectionSelected(id)
    }
}

impl<SectionId: Clone + PartialEq> Default for SettingsShellState<SectionId> {
    fn default() -> Self {
        Self::new()
    }
}

// ── FormWizard ──────────────────────────────────────────────────────────────

/// Form wizard outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormWizardOutcome {
    /// No change.
    Ignored,
    /// Step index changed.
    StepChanged {
        /// New step.
        index: usize,
    },
    /// Next blocked by invalid projection.
    BlockedInvalid,
    /// Submit requested (consumer validates/submits).
    SubmitRequested,
    /// Cancel.
    Cancelled,
}

/// Form wizard navigation state only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormWizardState {
    step: usize,
    step_count: usize,
    /// Caller says current step valid.
    step_valid: bool,
}

impl FormWizardState {
    /// Wizard with N steps.
    #[must_use]
    pub const fn new(step_count: usize) -> Self {
        Self {
            step: 0,
            step_count: if step_count == 0 { 1 } else { step_count },
            step_valid: true,
        }
    }

    #[must_use]
    /// Current step.
    pub const fn step(&self) -> usize {
        self.step
    }

    /// Project validity (consumer).
    pub const fn set_step_valid(&mut self, valid: bool) {
        self.step_valid = valid;
    }

    /// Keys: left/right or n/p.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormWizardOutcome {
        if key.kind != KeyEventKind::Press {
            return FormWizardOutcome::Ignored;
        }
        match key.code {
            KeyCode::Right | KeyCode::Char('n') => self.try_next(),
            KeyCode::Left | KeyCode::Char('p') => {
                if self.step > 0 {
                    self.step -= 1;
                    FormWizardOutcome::StepChanged { index: self.step }
                } else {
                    FormWizardOutcome::Ignored
                }
            }
            KeyCode::Enter if self.step + 1 >= self.step_count => {
                if self.step_valid {
                    FormWizardOutcome::SubmitRequested
                } else {
                    FormWizardOutcome::BlockedInvalid
                }
            }
            KeyCode::Esc => FormWizardOutcome::Cancelled,
            _ => FormWizardOutcome::Ignored,
        }
    }

    fn try_next(&mut self) -> FormWizardOutcome {
        if !self.step_valid {
            return FormWizardOutcome::BlockedInvalid;
        }
        if self.step + 1 >= self.step_count {
            return FormWizardOutcome::SubmitRequested;
        }
        self.step += 1;
        FormWizardOutcome::StepChanged { index: self.step }
    }
}

/// Paint chrome for form wizard step progress (domain fields stay consumer-owned).
#[derive(Debug, Clone, Copy)]
pub struct FormWizard<'a> {
    tokens: &'a DesignTokens,
    label: &'a str,
}

impl<'a> FormWizard<'a> {
    /// Create chrome with a progress label.
    #[must_use]
    pub const fn new(tokens: &'a DesignTokens, label: &'a str) -> Self {
        Self { tokens, label }
    }

    /// Paint progress line.
    pub fn render(
        &self,
        area: ratatui_core::layout::Rect,
        buffer: &mut ratatui_core::buffer::Buffer,
        state: &FormWizardState,
    ) {
        if area.is_empty() {
            return;
        }
        let line = format!("{}  step {}", self.label, state.step() + 1);
        let text = crate::text::take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.theme.style(crate::style::Role::TextStrong),
        );
    }
}

impl ratatui_core::widgets::StatefulWidget for FormWizard<'_> {
    type State = FormWizardState;

    fn render(
        self,
        area: ratatui_core::layout::Rect,
        buffer: &mut ratatui_core::buffer::Buffer,
        state: &mut Self::State,
    ) {
        FormWizard::render(&self, area, buffer, state);
    }
}

impl ratatui_core::widgets::StatefulWidget for &FormWizard<'_> {
    type State = FormWizardState;

    fn render(
        self,
        area: ratatui_core::layout::Rect,
        buffer: &mut ratatui_core::buffer::Buffer,
        state: &mut Self::State,
    ) {
        FormWizard::render(self, area, buffer, state);
    }
}

/// Marker type for block chrome that needs tokens (paint lives in consumer/story).
#[derive(Debug, Clone, Copy)]
pub struct BlockChrome<'a> {
    /// Design tokens.
    pub tokens: &'a DesignTokens,
}

impl<'a> BlockChrome<'a> {
    /// Tokens.
    #[must_use]
    pub const fn new(tokens: &'a DesignTokens) -> Self {
        Self { tokens }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_view::{ColumnModel, DataColumn, DataColumnWidth};
    use crate::widgets::menu_nav::SidebarItem;

    #[test]
    fn ops_tab_cycles_region() {
        let mut state = OpsDashboardState::<u64, &str>::new();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(4))]);
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            OpsDashboardOutcome::FocusRegion(OpsRegion::Log)
        ));
    }

    #[test]
    fn wizard_blocks_invalid_next() {
        let mut w = FormWizardState::new(3);
        w.set_step_valid(false);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            FormWizardOutcome::BlockedInvalid
        ));
        w.set_step_valid(true);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            FormWizardOutcome::StepChanged { index: 1 }
        ));
    }

    #[test]
    fn resource_load_on_select() {
        let mut state = ResourceBrowserState::new();
        let items = [SidebarItem::new("a", "A")];
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        assert!(matches!(out, ResourceBrowserOutcome::LoadRequested("a")));
        assert_eq!(state.selection_generation, 1);
    }

    #[test]
    fn settings_save_shortcut() {
        let mut s = SettingsShellState::<&str>::new();
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            SettingsShellOutcome::SaveRequested
        ));
    }
}
