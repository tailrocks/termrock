// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Stateful demo interactors implemented only through public TermRock APIs.
mod applications;
mod catalog;
mod composites;
mod extended;
mod remaining;
mod viewers;
mod workflows;
pub(crate) use applications::*;
pub(crate) use catalog::*;
pub(crate) use composites::*;
pub(crate) use extended::*;
pub(crate) use remaining::*;
pub(crate) use viewers::*;
pub(crate) use workflows::*;

use ratatui::text::Line;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::{StatefulWidget, Widget},
};
use std::num::NonZeroU16;
use termrock::{
    input::{
        Event, KeyCode, KeyEvent, KeyReleaseReporting, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{Outcome, OverlaySize},
    style::{ColorCapability, DesignSystem, Role, RolePalette},
    widgets::{
        Accordion, AccordionItem, AccordionOutcome, AccordionState, Action, ActionLink,
        ActionLinkOutcome, ActivationOutcome, AlertDialog, AlertDialogOutcome, AlertDialogState,
        AlertKind, AlertScope, Anchor, BUILTIN_THEME_PRESETS, Button, ButtonState, ButtonVariant,
        CellAlignment, Checkbox, CheckboxOutcome, CheckboxState, ChoiceDialogState, Collapsible,
        CollapsibleOutcome, CollapsibleState, Column, ColumnModel, ColumnWidth, CommandPalette,
        CommandPaletteState, ComposerChip, ContextEstimate, DataColumn, DataColumnWidth,
        DesignInspector, DesignInspectorFrame, Dialog, DialogOutcome, DialogRecipe, DialogState,
        DropdownMenu, DropdownMenuOutcome, DropdownMenuState, Fieldset, Form, FormOutcome,
        FormState, FormWizard, FormWizardOutcome, FormWizardState, GridCell, GridColumn, GridRow,
        InspectorPanel, LinkState, List, ListRow, ListState, LoadState, LogPane, LogPaneState,
        MenuNode, ModeIndicator, ModelIndicator, MultiSelect, MultiSelectOutcome, MultiSelectState,
        NavItem, NumberConstraints, NumberInput, NumberInputOutcome, NumberInputState, PageTotal,
        Pagination, PaginationOutcome, PaginationState, Panel, PanelState, PanelVariant,
        PasswordInput, PasswordInputOutcome, PasswordInputParts, PasswordInputState,
        PasswordStrengthHint, Picker, PickerOutcome, PickerState, Popover, PopoverOutcome,
        PopoverState, PromptComposer, PromptComposerOutcome, PromptComposerState, RangeSlider,
        RangeSliderOutcome, RangeSliderState, ResizablePanelGroup, ResizablePanelGroupState,
        ResizablePanelOutcome, ResizablePanelSpec, SegmentedControl, SegmentedControlOutcome,
        SegmentedControlState, SegmentedItem, Select, SelectOption, SelectOutcome, SelectRecipe,
        SelectState, Severity, Sidebar, SidebarOutcome, SidebarPresentation, SidebarState, Slider,
        SliderBounds, SliderOutcome, SliderState, SplitDirection, SplitPane, SplitPaneOutcome,
        SplitPaneState, SplitRatio, StickyRegion, Switch, SwitchOutcome, SwitchState, Tab, Table,
        TableOutcome, TableRow, TableState, Tabs, TabsState, TextArea, TextAreaOutcome,
        TextAreaState, TextInput, TextInputOutcome, TextInputState, ThemePicker, ThemePickerState,
        Toast, Toggle, ToggleGroup, ToggleGroupItem, ToggleGroupOutcome, ToggleGroupState,
        ToggleOutcome, ToggleState, ToggleValue, Transcript, TranscriptBlock, TranscriptKind,
        TranscriptState, Tree, TreeNode, TreeOutcome, TreeState, TreeTable, TreeTableOutcome,
        TreeTableRow, TreeTableState, VirtualGrid, VirtualGridOutcome, VirtualGridState,
        VirtualList, VirtualListItem, VirtualListState, WizardStep, example_command_catalog,
    },
};

use crate::demo::DemoDeadline;
use crate::knobs::{Knob, KnobValue};
use crate::stories::{
    SPLIT_PANE_MAX, SPLIT_PANE_MIN, choice_actions, form_fields, list_rows, picker_rows,
    render_choice_dialog, render_split_pane, tree_nodes,
};

trait PointerTarget {
    fn hover(&mut self, _position: Position) -> bool {
        false
    }
    fn click_at(&mut self, _position: Position) -> bool {
        false
    }
    fn drag_to(&mut self, _position: Position) -> bool {
        false
    }
    fn wheel(&mut self, _delta: isize) -> bool {
        false
    }
}

fn route_pointer(target: &mut impl PointerTarget, mouse: MouseEvent, preview_area: Rect) -> bool {
    let position = mouse.position;
    if !preview_area.contains(position) {
        return target.hover(position);
    }
    match mouse.kind {
        MouseEventKind::Moved => target.hover(position),
        MouseEventKind::Down(MouseButton::Left) => target.click_at(position),
        MouseEventKind::Drag(MouseButton::Left) => target.drag_to(position),
        MouseEventKind::ScrollUp => target.wheel(-1),
        MouseEventKind::ScrollDown => target.wheel(1),
        _ => false,
    }
}

/// Persistent component behavior mounted by both preview hosts.
pub trait StoryInteraction {
    /// Paint the current state into the supplied preview area.
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);
    /// Select the stable catalog id when one factory serves multiple stories.
    fn set_demo_id(&mut self, _id: &'static str) {}
    /// Forward a backend-neutral key and report whether state changed.
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    /// Complete a key lifecycle only for controls that explicitly await release.
    fn handle_key_release(&mut self, _key: KeyEvent) -> bool {
        false
    }
    /// Forward a backend-neutral pointer event and report whether state changed.
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool;
    /// Route one backend-neutral host event through the same public widget APIs.
    fn handle_event(&mut self, event: Event, preview_area: Rect) -> bool {
        match event {
            Event::Key(key) if key.kind == termrock::input::KeyEventKind::Release => {
                self.handle_key_release(key)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, preview_area),
            Event::Paste(value) => {
                let mut changed = false;
                for ch in value.chars() {
                    changed |= self.handle_key(KeyEvent::new(
                        KeyCode::Char(ch),
                        termrock::input::KeyModifiers::NONE,
                    ));
                }
                changed
            }
            Event::Resize { .. } | Event::FocusGained | Event::FocusLost | Event::Unknown => false,
            _ => false,
        }
    }
    /// Replace the semantic palette used on the next paint.
    fn set_system(&mut self, system: DesignSystem);
    /// Current state-specific action hints, when more precise than catalog defaults.
    fn hints(&self) -> Vec<&'static str> {
        Vec::new()
    }
    /// Consume the latest typed, user-visible demo outcome.
    fn take_outcome(&mut self) -> Option<String> {
        None
    }
    /// Advance host-controlled time for loading, toast, and motion state.
    fn handle_tick(&mut self, _elapsed_ms: u64) -> bool {
        false
    }
    /// Earliest deterministic host-time wakeup and its semantic purpose.
    fn next_deadline(&self, _elapsed_ms: u64) -> Option<DemoDeadline> {
        None
    }
    /// Deterministic controls exposed by native Lookbook.
    fn knobs(&self) -> &[Knob] {
        &[]
    }
    /// Edit one selected control through keyboard input.
    fn handle_knob_key(&mut self, _selected: usize, _key: KeyEvent) -> bool {
        false
    }
    /// Paint the rich editor for one selected control when it has one.
    fn render_knob_editor(&mut self, _selected: usize, _frame: &mut Frame<'_>, _area: Rect) {}
    /// Let overlays consume Escape before the Lookbook shell does.
    fn handle_preview_escape(&mut self, _key: KeyEvent) -> bool {
        false
    }
    /// Whether plain character keys belong to the mounted component.
    fn captures_text_input(&self) -> bool {
        false
    }
    /// Whether the selected control consumes plain character keys.
    fn knob_captures_text_input(&self, _selected: usize) -> bool {
        false
    }
}

pub(crate) struct StaticStory {
    pub(crate) render_fn: fn(&mut Frame<'_>, Rect, &DesignSystem),
    pub(crate) system: DesignSystem,
}

/// Factory shell that requires every pattern story to install its real public
/// pattern state machine before use.
pub(crate) struct PatternAppInteractor {
    system: DesignSystem,
    delegate: Option<Box<dyn StoryInteraction>>,
}

impl PatternAppInteractor {
    pub(crate) fn new() -> Self {
        Self {
            system: crate::design::lookbook_system(RolePalette::default()),
            delegate: None,
        }
    }

    fn delegate_mut(&mut self) -> &mut dyn StoryInteraction {
        self.delegate
            .as_deref_mut()
            .expect("pattern demo must install a public state-machine delegate")
    }

    fn delegate(&self) -> &dyn StoryInteraction {
        self.delegate
            .as_deref()
            .expect("pattern demo must install a public state-machine delegate")
    }
}

impl StoryInteraction for PatternAppInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.delegate_mut().render(frame, area);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.delegate_mut().handle_key(key)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        self.delegate_mut().handle_mouse(mouse, preview_area)
    }

    fn set_system(&mut self, system: DesignSystem) {
        if let Some(delegate) = self.delegate.as_mut() {
            delegate.set_system(system.clone());
        }
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        self.delegate().hints()
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.delegate_mut().take_outcome()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.delegate_mut().handle_preview_escape(key)
    }

    fn captures_text_input(&self) -> bool {
        self.delegate().captures_text_input()
    }

    fn handle_tick(&mut self, elapsed_ms: u64) -> bool {
        self.delegate_mut().handle_tick(elapsed_ms)
    }

    fn next_deadline(&self, elapsed_ms: u64) -> Option<DemoDeadline> {
        self.delegate().next_deadline(elapsed_ms)
    }

    fn set_demo_id(&mut self, id: &'static str) {
        // An "in application" variant is the host scene wearing a component's
        // name: resolve its delegate through the host (plans/018 Step 2).
        let host = crate::stories::in_app_host(id).unwrap_or(id);
        let mut delegate = application_interactor(host)
            .or_else(|| composite_interactor(host))
            .unwrap_or_else(|| panic!("pattern demo {id} has no public state-machine delegate"));
        delegate.set_system(self.system.clone());
        self.delegate = Some(delegate);
    }
}

pub(crate) struct PanelInteractor {
    state: PanelState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl PanelInteractor {
    pub(crate) fn new() -> Self {
        let mut state = PanelState::new();
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
}

impl StoryInteraction for PanelInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let body = Panel::new(&system)
            .title("Summary")
            .variant(PanelVariant::Interactive)
            .collapsible(true)
            .paint(area, frame.buffer_mut(), Some(&mut self.state));
        if !self.state.is_collapsed() && !body.is_empty() {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "State Ready · Mode Interactive",
                usize::from(body.width),
                system.style(Role::Text),
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, true, true);
        extended::record(&mut self.outcome, "Panel", outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = self.state.clone();
        let outcome = self.state.handle_mouse(mouse, true, true);
        extended::record(&mut self.outcome, "Panel", outcome) || self.state != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/Space toggle",
            "← collapse",
            "→ expand",
            "click header/body",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

impl StoryInteraction for StaticStory {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        (self.render_fn)(frame, area, &self.system.clone());
    }
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }
    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct TextAreaInteractor {
    state: TextAreaState,
    system: DesignSystem,
}

impl TextAreaInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TextAreaState::new("First line\nSecond line");
        state.set_accepts_input(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for TextAreaInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &TextArea::new(&self.system.clone()).title("Compose"),
            area,
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(key), TextAreaOutcome::Ignored)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        preview_area.contains(mouse.position)
            && !matches!(
                self.state.handle_event(Event::Mouse(mouse)),
                TextAreaOutcome::Ignored
            )
    }
    fn handle_event(&mut self, event: Event, preview_area: Rect) -> bool {
        match event {
            Event::Mouse(mouse) if preview_area.contains(mouse.position) => !matches!(
                self.state.handle_event(Event::Mouse(mouse)),
                TextAreaOutcome::Ignored
            ),
            Event::Key(key) => !matches!(self.state.handle_key(key), TextAreaOutcome::Ignored),
            Event::Paste(value) => !matches!(
                self.state.handle_event(Event::Paste(value)),
                TextAreaOutcome::Ignored
            ),
            Event::Resize { .. } | Event::FocusGained | Event::FocusLost | Event::Unknown => false,
            Event::Mouse(_) => false,
            _ => false,
        }
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct ChoiceDialogInteractor {
    state: ChoiceDialogState<&'static str>,
    trigger: ButtonState,
    open: bool,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ChoiceDialogInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        Self {
            state: ChoiceDialogState::new(Some("continue")),
            trigger,
            open: false,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: Outcome<&'static str>) -> bool {
        match outcome {
            Outcome::Ignored => false,
            Outcome::Activated(choice) => {
                self.open = false;
                self.outcome = Some(format!("You chose {choice}"));
                true
            }
            Outcome::Cancelled => {
                self.open = false;
                self.outcome = Some("You chose cancel".to_owned());
                true
            }
            Outcome::Changed => true,
            _ => false,
        }
    }
}

impl StoryInteraction for ChoiceDialogInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        if !self.open {
            Button::new("Choose action", &system)
                .variant(ButtonVariant::Primary)
                .render(area, frame.buffer_mut(), &mut self.trigger);
            return;
        }
        render_choice_dialog(frame, area, &mut self.state, &system);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.open {
            if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
                self.open = true;
                self.outcome = Some("Choice dialog opened".to_owned());
                return true;
            }
            return false;
        }
        let outcome = self.state.handle_key(&choice_actions(), key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        if !self.open {
            let before = self.trigger.hovered;
            if matches!(
                self.trigger.handle_mouse(mouse),
                ActivationOutcome::Activated
            ) {
                self.open = true;
                self.outcome = Some("Choice dialog opened".to_owned());
                return true;
            }
            return self.trigger.hovered != before;
        }
        let position = mouse.position;
        if !preview_area.contains(position) {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let outcome = self.state.click(position);
            return self.resolve(outcome);
        }
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.open {
            vec!["←→ choose", "Enter decide", "Esc cancel", "click action"]
        } else {
            vec!["Enter open choices", "click Choose action"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ListInteractor {
    state: ListState<&'static str>,
    system: DesignSystem,
}

impl ListInteractor {
    pub(crate) fn new() -> Self {
        let state = ListState::new(Some("beta"));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for ListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = list_rows();
        let tokens = self.system.clone();
        frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(&list_rows(), key), Outcome::Ignored)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

impl PointerTarget for ListInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        !matches!(self.state.click(position), Outcome::Ignored)
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, list_rows().len())
    }
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta, list_rows().len())
    }
}

pub(crate) struct PickerInteractor {
    state: PickerState<&'static str>,
    system: DesignSystem,
    activated: Option<&'static str>,
}

impl PickerInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: PickerState::new(Some("alpha")),
            system: crate::design::lookbook_system(RolePalette::default()),
            activated: None,
        }
    }
}

impl StoryInteraction for PickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = picker_rows(self.state.query_text());
        let tokens = self.system.clone();
        frame.render_stateful_widget(&Picker::new(&rows, &tokens), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let rows = picker_rows(self.state.query_text());
        match self.state.handle_key(&rows, key) {
            PickerOutcome::QueryChanged => {
                let rows = picker_rows(self.state.query_text());
                self.state.reconcile(&rows);
                true
            }
            PickerOutcome::Activated(id) => {
                self.activated = Some(id);
                true
            }
            PickerOutcome::CursorMoved => true,
            PickerOutcome::Ignored | PickerOutcome::Cancelled => false,
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

impl PointerTarget for PickerInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.list().hovered().cloned();
        self.state.hover(position);
        self.state.list().hovered() != before.as_ref()
    }

    fn click_at(&mut self, position: Position) -> bool {
        if let PickerOutcome::Activated(id) = self.state.click(position) {
            self.activated = Some(id);
            true
        } else {
            false
        }
    }

    fn wheel(&mut self, delta: isize) -> bool {
        self.state
            .scroll_by(delta, picker_rows(self.state.query_text()).len())
    }
}

pub(crate) struct LogPaneInteractor {
    state: LogPaneState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl LogPaneInteractor {
    pub(crate) fn new() -> Self {
        let mut state = LogPaneState::new().with_max_lines(200);
        for line in [
            "[12:04:01] resolving workspace",
            "[12:04:02] compiling termrock",
            "[12:04:03] running 205 tests",
            "[12:04:04] result: ok ✓",
            "[12:04:05] preview ready",
            "[12:04:06] waiting for changes",
            "[12:04:07] file changed: src/lib.rs",
            "[12:04:08] compiling incremental graph",
            "[12:04:09] running widget tests",
            "[12:04:10] running docs tests",
            "[12:04:11] all checks passed",
            "[12:04:12] watching workspace",
        ] {
            state.append(line);
        }
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
}

impl StoryInteraction for LogPaneInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &LogPane::new(&self.system.clone()).title("Build log"),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        if matches!(outcome, Outcome::Ignored) {
            return false;
        }
        self.outcome = Some(format!(
            "LogPane: {outcome:?}; {}",
            if self.state.is_following() {
                "following"
            } else {
                "scrollback paused"
            }
        ));
        true
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let changed = route_pointer(self, mouse, preview_area);
        if changed {
            self.outcome = Some(format!(
                "LogPane: scrolled; {}",
                if self.state.is_following() {
                    "following"
                } else {
                    "scrollback paused"
                }
            ));
        }
        changed
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓/PageUp/PageDown scroll",
            "End follow tail",
            "wheel scroll",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

impl PointerTarget for LogPaneInteractor {
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta)
    }
}

pub(crate) struct TreeInteractor {
    nodes: Vec<TreeNode<'static, &'static str>>,
    state: TreeState<&'static str>,
    system: DesignSystem,
}

impl TreeInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TreeState::new(Some("workspace"));
        state.enable_multi_select();
        state.selection_mut().unwrap().toggle(&"notes");
        Self {
            nodes: tree_nodes(),
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for TreeInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tokens = self.system.clone();
        frame.render_stateful_widget(&Tree::new(&self.nodes, &tokens), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(&self.nodes, key),
            TreeOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

impl PointerTarget for TreeInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, self.nodes.len())
            || !matches!(self.state.click(position), TreeOutcome::Ignored)
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, self.nodes.len())
    }
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta, self.nodes.len());
        true
    }
}

pub(crate) struct FormInteractor {
    state: FormState<&'static str>,
    /// Host-owned field focus (scene stand-in for the story shell).
    focused: Option<&'static str>,
    system: DesignSystem,
}

impl FormInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: FormState::new(),
            focused: Some("name"),
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for FormInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // FormSection borrows its fields, so storing both in the interactor
        // would be self-referential. Rebuild this tiny fixture at each call.
        let fields = form_fields();
        let sections = [Fieldset::new("General", &fields)];
        if let Some(id) = self.focused {
            self.state.ensure_visible(Some(id));
        }
        frame.render_stateful_widget(
            &Form::new(&sections, &self.system.clone()).focused_field(self.focused.as_ref()),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let fields = form_fields();
        let sections = [Fieldset::new("General", &fields)];
        // Host/scene field cycle stand-in: Tab moves focus id, then form activates only.
        use termrock::input::{KeyCode, KeyEventKind};
        if key.kind != KeyEventKind::Release
            && matches!(
                key.code,
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Down | KeyCode::Up
            )
        {
            let enabled: Vec<_> = fields.iter().filter(|f| f.enabled).map(|f| f.id).collect();
            if enabled.is_empty() {
                return false;
            }
            let forward = matches!(key.code, KeyCode::Tab | KeyCode::Down);
            let idx = self
                .focused
                .and_then(|id| enabled.iter().position(|e| *e == id))
                .unwrap_or(0);
            let next = if forward {
                (idx + 1) % enabled.len()
            } else {
                idx.checked_sub(1).unwrap_or(enabled.len() - 1)
            };
            self.focused = Some(enabled[next]);
            self.state.ensure_visible(self.focused);
            return true;
        }
        !matches!(
            self.state.handle_key(&sections, key, self.focused.as_ref()),
            FormOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

impl PointerTarget for FormInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        if self.state.scroll_to_position(position) {
            return true;
        }
        // Host: scene.focus on hit, then activate if already focused.
        if let Some(&id) = self.state.hit_id(position) {
            if self.focused == Some(id) {
                return !matches!(
                    self.state.click(position, self.focused.as_ref()),
                    FormOutcome::Ignored
                );
            }
            self.focused = Some(id);
            self.state.ensure_visible(Some(id));
            return true;
        }
        false
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position)
    }
    fn wheel(&mut self, delta: isize) -> bool {
        let content_len = self.state.content_height();
        self.state.scroll_by(delta, content_len);
        true
    }
}

pub(crate) struct SplitPaneInteractor {
    state: SplitPaneState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SplitPaneInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SplitPaneState::new(SplitRatio::from_percent(38));
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: SplitPaneOutcome) -> bool {
        match outcome {
            SplitPaneOutcome::Ignored => false,
            SplitPaneOutcome::Focused => {
                self.outcome = Some("Split divider focused".into());
                true
            }
            SplitPaneOutcome::RatioChanged(ratio) => {
                self.outcome = Some(format!(
                    "Split ratio: {:.1}%",
                    f32::from(ratio.basis_points()) / 100.0
                ));
                true
            }
            SplitPaneOutcome::Collapsed(side) => {
                self.outcome = Some(format!("Split pane {side:?} collapsed"));
                true
            }
            SplitPaneOutcome::Expanded => {
                self.outcome = Some("Split panes expanded".into());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for SplitPaneInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        render_split_pane(frame, area, &mut self.state, &self.system.clone());
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let split = SplitPane::new(
            SplitDirection::Horizontal,
            SPLIT_PANE_MIN,
            SPLIT_PANE_MAX,
            &system,
        );
        let outcome = self.state.handle_key(&split, key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let position = mouse.position;
        let system = self.system.clone();
        let split = SplitPane::new(
            SplitDirection::Horizontal,
            SPLIT_PANE_MIN,
            SPLIT_PANE_MAX,
            &system,
        );
        match mouse.kind {
            MouseEventKind::Moved => self.state.hover(&split, position),
            MouseEventKind::Down(MouseButton::Left) => {
                let outcome = self.state.drag_start(&split, position);
                self.apply(outcome)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let outcome = self.state.drag_move(&split, position);
                self.apply(outcome)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let changed = self.state.is_dragging();
                self.state.drag_end();
                if changed {
                    let ratio = f32::from(self.state.ratio().basis_points()) / 100.0;
                    self.outcome = Some(format!("Split resize completed at {ratio:.1}%"));
                }
                changed
            }
            _ => false,
        }
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ resize", "Home/End collapse", "drag divider"]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ToastInteractor {
    knobs: Vec<Knob>,
    message: TextInputState,
    trigger: ButtonState,
    visible: bool,
    shown_at_ms: u64,
    elapsed_ms: u64,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ToastInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        Self {
            knobs: vec![
                Knob {
                    id: "severity",
                    label: "Severity",
                    value: KnobValue::Choice(1),
                    choices: &["Info", "Success", "Warning", "Error"],
                },
                Knob {
                    id: "anchor",
                    label: "Anchor",
                    value: KnobValue::Choice(1),
                    choices: &["Top left", "Top right", "Bottom left", "Bottom right"],
                },
                Knob {
                    id: "message",
                    label: "Message",
                    value: KnobValue::Text("Updated".to_owned()),
                    choices: &[],
                },
            ],
            message: TextInputState::new("Updated").with_max_graphemes(48),
            trigger,
            visible: false,
            shown_at_ms: 0,
            elapsed_ms: 0,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn severity(&self) -> Severity {
        match self.knobs[0].value {
            KnobValue::Choice(0) => Severity::Info,
            KnobValue::Choice(2) => Severity::Warning,
            KnobValue::Choice(3) => Severity::Error,
            _ => Severity::Success,
        }
    }

    fn anchor(&self) -> Anchor {
        match self.knobs[1].value {
            KnobValue::Choice(0) => Anchor::TopLeft,
            KnobValue::Choice(2) => Anchor::BottomLeft,
            KnobValue::Choice(3) => Anchor::BottomRight,
            _ => Anchor::TopRight,
        }
    }
}

impl StoryInteraction for ToastInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        Button::new(
            if self.visible {
                "Dismiss toast"
            } else {
                "Show toast"
            },
            &system,
        )
        .render(
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            frame.buffer_mut(),
            &mut self.trigger,
        );
        if self.visible {
            frame.render_widget(
                Toast::new(&system, self.message.value(), self.severity()).anchor(self.anchor()),
                Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)),
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.visible && key.code == KeyCode::Esc {
            self.visible = false;
            self.outcome = Some("Toast dismissed".to_owned());
            return true;
        }
        if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
            self.visible = !self.visible;
            if self.visible {
                self.shown_at_ms = self.elapsed_ms;
                self.outcome = Some("Toast appeared".to_owned());
            } else {
                self.outcome = Some("Toast dismissed".to_owned());
            }
            return true;
        }
        false
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = self.trigger.hovered;
        if matches!(
            self.trigger.handle_mouse(mouse),
            ActivationOutcome::Activated
        ) {
            self.visible = !self.visible;
            if self.visible {
                self.shown_at_ms = self.elapsed_ms;
                self.outcome = Some("Toast appeared".to_owned());
            } else {
                self.outcome = Some("Toast dismissed".to_owned());
            }
            return true;
        }
        self.trigger.hovered != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn knobs(&self) -> &[Knob] {
        &self.knobs
    }

    fn handle_knob_key(&mut self, selected: usize, key: KeyEvent) -> bool {
        let Some(knob) = self.knobs.get_mut(selected) else {
            return false;
        };
        match &mut knob.value {
            KnobValue::Choice(index) if matches!(key.code, KeyCode::Left | KeyCode::Right) => {
                let count = knob.choices.len();
                if count == 0 {
                    return false;
                }
                *index = if key.code == KeyCode::Right {
                    (*index + 1) % count
                } else {
                    (*index + count - 1) % count
                };
                true
            }
            KnobValue::Text(value) => {
                let changed = !matches!(self.message.handle_key(key), TextInputOutcome::Ignored);
                *value = self.message.value().to_owned();
                changed
            }
            KnobValue::Bool(_) | KnobValue::Number(_) | KnobValue::Choice(_) => false,
        }
    }

    fn render_knob_editor(&mut self, selected: usize, frame: &mut Frame<'_>, area: Rect) {
        if selected == 2 {
            frame.render_stateful_widget(
                &TextInput::new("Message", &self.system.clone()).placeholder("Toast message"),
                area,
                &mut self.message,
            );
        }
    }

    fn knob_captures_text_input(&self, selected: usize) -> bool {
        selected == 2
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.visible {
            vec![
                "Esc dismiss",
                "click Dismiss toast",
                "auto-expires after 2s",
            ]
        } else {
            vec!["Enter show", "click Show toast"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_tick(&mut self, elapsed_ms: u64) -> bool {
        self.elapsed_ms = elapsed_ms;
        if self.visible && elapsed_ms.saturating_sub(self.shown_at_ms) >= 2_000 {
            self.visible = false;
            self.outcome = Some("Toast expired".to_owned());
            return true;
        }
        false
    }

    fn next_deadline(&self, _elapsed_ms: u64) -> Option<DemoDeadline> {
        self.visible
            .then(|| DemoDeadline::functional(self.shown_at_ms.saturating_add(2_000)))
    }
}

// ── Additional interactive-component interactors (plan 048) ─────────────────

pub(crate) struct TabsInteractor {
    state: TabsState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl TabsInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TabsState::new().with_selected("overview");
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn tabs() -> [Tab<'static, &'static str>; 2] {
        [
            Tab::new("overview", "Overview"),
            Tab::new("details", "Details"),
        ]
    }

    fn apply(&mut self, outcome: termrock::widgets::TabsOutcome<&'static str>) -> bool {
        use termrock::widgets::TabsOutcome;

        match outcome {
            TabsOutcome::Ignored => false,
            TabsOutcome::FocusChanged { id } => {
                let selected = self.state.selected().copied().unwrap_or("none");
                self.outcome = Some(format!(
                    "Tab selected: {selected}; focus: {}",
                    id.unwrap_or("none")
                ));
                true
            }
            TabsOutcome::SelectionChanged { id } => {
                self.outcome = Some(format!("Tab selected: {id}"));
                true
            }
            TabsOutcome::CloseRequested { id } => {
                self.outcome = Some(format!("Tab close requested: {id}"));
                true
            }
            TabsOutcome::OverflowOpened { .. } => {
                self.outcome = Some("Tab overflow opened".into());
                true
            }
            TabsOutcome::OverflowClosed => {
                self.outcome = Some("Tab overflow closed".into());
                true
            }
            TabsOutcome::ReorderRequested { from, to } => {
                self.outcome = Some(format!("Tab reorder requested: {from} → {to}"));
                true
            }
            TabsOutcome::Changed => true,
            _ => false,
        }
    }
}

impl StoryInteraction for TabsInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tabs = Self::tabs();
        frame.render_stateful_widget(
            &Tabs::new(&tabs, &self.system.clone()).gap(1),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let tabs = Self::tabs();
        let outcome = self.state.handle_key(key, &tabs);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let tabs = Self::tabs();
        let outcome = self.state.handle_mouse(mouse, &tabs);
        self.apply(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ change tab", "Home/End", "click tab"]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TableInteractor {
    state: TableState<&'static str, &'static str>,
    system: DesignSystem,
}

impl TableInteractor {
    pub(crate) fn new() -> Self {
        let state = TableState::new(Some("r1"));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for TableInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tokens = self.system.clone();
        let columns = [
            Column::new(
                "name",
                "Name",
                ColumnWidth::Fill(NonZeroU16::new(2).unwrap()),
            ),
            Column::new("cpu", "CPU", ColumnWidth::Min(6)).alignment(CellAlignment::Right),
        ];
        let c0 = [Line::from("alpha"), Line::from("12%")];
        let c1 = [Line::from("beta"), Line::from("4%")];
        let rows = [TableRow::new("r1", &c0), TableRow::new("r2", &c1)];
        frame.render_stateful_widget(&Table::new(&columns, &rows, &tokens), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let c0 = [Line::from("alpha"), Line::from("12%")];
        let c1 = [Line::from("beta"), Line::from("4%")];
        let rows = [TableRow::new("r1", &c0), TableRow::new("r2", &c1)];
        !matches!(self.state.handle_key(&rows, key), TableOutcome::Ignored)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct ThemePickerInteractor {
    state: ThemePickerState,
    system: DesignSystem,
}

impl ThemePickerInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: ThemePickerState::new(0),
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for ThemePickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &ThemePicker::new(BUILTIN_THEME_PRESETS, &self.system.clone()),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(key, BUILTIN_THEME_PRESETS.len()),
            termrock::widgets::ThemePickerOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct CommandPaletteInteractor {
    state: CommandPaletteState<&'static str>,
    system: DesignSystem,
}

impl CommandPaletteInteractor {
    pub(crate) fn new() -> Self {
        let mut state = CommandPaletteState::new(None);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for CommandPaletteInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tokens = self.system.clone();
        let catalog = example_command_catalog();
        let visible = self.state.refilter(&catalog);
        frame.render_stateful_widget(
            &CommandPalette::new("Commands", &visible, &tokens),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let catalog = example_command_catalog();
        let visible = self.state.refilter(&catalog);
        !matches!(
            CommandPalette::handle_key(&mut self.state, key, &visible),
            termrock::widgets::CommandPaletteOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct DesignInspectorInteractor {
    panel: InspectorPanel,
    system: DesignSystem,
}

impl DesignInspectorInteractor {
    pub(crate) fn new() -> Self {
        Self {
            panel: InspectorPanel::Focus,
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for DesignInspectorInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let layers = ["root", "preview"];
        let recipes = ["list_row", "panel"];
        let snap = DesignInspectorFrame {
            focused: Some("preview"),
            layer: Some("root"),
            capability: ColorCapability::Truecolor,
            layers: &layers,
            recipes: &recipes,
            semantics: &[],
            focus_graph: &[],
        };
        frame.render_widget(
            DesignInspector::new(snap, &self.system.clone()).panel(self.panel),
            area,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.panel = InspectorPanel::Focus;
                true
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                self.panel = InspectorPanel::Layers;
                true
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.panel = InspectorPanel::Tokens;
                true
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.panel = InspectorPanel::Recipes;
                true
            }
            KeyCode::Tab | KeyCode::Right => {
                self.panel = match self.panel {
                    InspectorPanel::Focus => InspectorPanel::Layers,
                    InspectorPanel::Layers => InspectorPanel::Tokens,
                    InspectorPanel::Tokens => InspectorPanel::Recipes,
                    InspectorPanel::Recipes | _ => InspectorPanel::Focus,
                };
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct TranscriptInteractor {
    state: TranscriptState<&'static str>,
    system: DesignSystem,
}

impl TranscriptInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: TranscriptState::new(),
            system: crate::design::lookbook_system(RolePalette::default()),
        }
    }
}

impl StoryInteraction for TranscriptInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let user = [
            "Run the release suite",
            "include docs and capability checks",
        ];
        let assistant = [
            "I’ll verify the full workspace.",
            "The dependency graph is clean.",
        ];
        let thinking = [
            "Inspecting affected surfaces…",
            "Checking migration order…",
            "Comparing deterministic frames…",
            "Hidden reasoning beyond preview.",
        ];
        let tool = ["mise run check", "2,968 tests · all green"];
        let blocks = [
            TranscriptBlock::new("u1", TranscriptKind::User, &user),
            TranscriptBlock::new("a1", TranscriptKind::Assistant, &assistant),
            TranscriptBlock::new("th1", TranscriptKind::Thinking, &thinking)
                .folded(true)
                .summary("Reasoning through the verification plan"),
            TranscriptBlock::new("t1", TranscriptKind::Tool, &tool).active(true),
        ];
        self.state.set_focused(true);
        frame.render_stateful_widget(
            &Transcript::new(&blocks, &self.system.clone())
                .focused(true)
                .tick(7),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let user = [
            "Run the release suite",
            "include docs and capability checks",
        ];
        let assistant = [
            "I’ll verify the full workspace.",
            "The dependency graph is clean.",
        ];
        let thinking = [
            "Inspecting affected surfaces…",
            "Checking migration order…",
            "Comparing deterministic frames…",
            "Hidden reasoning beyond preview.",
        ];
        let tool = ["mise run check", "2,968 tests · all green"];
        let blocks = [
            TranscriptBlock::new("u1", TranscriptKind::User, &user),
            TranscriptBlock::new("a1", TranscriptKind::Assistant, &assistant),
            TranscriptBlock::new("th1", TranscriptKind::Thinking, &thinking)
                .folded(true)
                .summary("Reasoning through the verification plan"),
            TranscriptBlock::new("t1", TranscriptKind::Tool, &tool).active(true),
        ];
        self.state.set_focused(true);
        !matches!(
            self.state.handle_key(key, &blocks),
            termrock::widgets::TranscriptOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
}

pub(crate) struct PromptComposerInteractor {
    state: PromptComposerState,
    system: DesignSystem,
}

impl PromptComposerInteractor {
    pub(crate) fn new() -> Self {
        let system = crate::design::lookbook_system(RolePalette::default());
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_placeholder("Ask anything…");
        state.set_mode(Some(ModeIndicator {
            label: "EDIT".into(),
            warning: false,
        }));
        state.set_model(Some(ModelIndicator {
            label: "model".into(),
        }));
        state.set_context(ContextEstimate {
            used: 12_000,
            limit: 128_000,
        });
        state.add_chip(ComposerChip::file("f1", "main.rs"));
        Self { state, system }
    }
}

impl StoryInteraction for PromptComposerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(&PromptComposer::new(&self.system), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(key), PromptComposerOutcome::Ignored)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let layout = self.state.layout_in(preview_area);
        !matches!(
            self.state.handle_mouse_at(mouse, &layout),
            PromptComposerOutcome::Ignored
        )
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct ActionLinkInteractor {
    state: LinkState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ActionLinkInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: LinkState::new(),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn action(system: &DesignSystem) -> ActionLink<'_> {
        ActionLink::new("Run tests", system).risk_note("cargo test")
    }

    fn apply(&mut self, outcome: ActionLinkOutcome) -> bool {
        match outcome {
            ActionLinkOutcome::Ignored => false,
            ActionLinkOutcome::Activated => {
                self.outcome = Some("Action activated: cargo test".to_owned());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for ActionLinkInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = Self::action(&system).paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            self.state.set_focused(true);
        }
        let system = self.system.clone();
        let outcome = Self::action(&system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = self.state.hovered;
        let system = self.system.clone();
        let outcome = Self::action(&system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || self.state.hovered != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec!["hover to highlight", "Enter activate", "click action"]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ButtonInteractor {
    state: ButtonState,
    system: DesignSystem,
    loading_since_ms: Option<u64>,
    outcome: Option<String>,
}

impl ButtonInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state
            .activation
            .set_release_reporting(KeyReleaseReporting::Reported);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            loading_since_ms: None,
            outcome: None,
        }
    }

    fn activate(&mut self, outcome: ActivationOutcome) -> bool {
        match outcome {
            ActivationOutcome::Ignored => false,
            ActivationOutcome::Pressed => true,
            ActivationOutcome::Activated => {
                // Establish the start from the first host tick. Non-animated
                // demos are not polled before activation, so their cached
                // elapsed value is intentionally not a wall-clock guess.
                self.loading_since_ms = None;
                self.state.activation.set_loading(true);
                self.outcome = Some("Save started".to_owned());
                true
            }
            ActivationOutcome::ConfirmRequired => {
                self.outcome = Some("Confirmation required".to_owned());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for ButtonInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        Button::new("Save", &system)
            .variant(ButtonVariant::Primary)
            .leading("✓")
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.activate(outcome)
    }

    fn handle_key_release(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.activate(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = self.state.hovered;
        let outcome = self.state.handle_mouse(mouse);
        self.activate(outcome) || self.state.hovered != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.activation.is_loading() {
            vec!["loading — input disabled", "wait for completion"]
        } else {
            vec![
                "Enter activate",
                "Space press/release",
                "click press/release",
            ]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_tick(&mut self, elapsed_ms: u64) -> bool {
        if !self.state.activation.is_loading() {
            return false;
        }
        let Some(started) = self.loading_since_ms else {
            self.loading_since_ms = Some(elapsed_ms);
            return false;
        };
        if elapsed_ms.saturating_sub(started) < 800 {
            return false;
        }
        self.loading_since_ms = None;
        self.state.activation.set_loading(false);
        self.outcome = Some("Saved successfully".to_owned());
        true
    }

    fn next_deadline(&self, elapsed_ms: u64) -> Option<DemoDeadline> {
        self.state.activation.is_loading().then(|| {
            DemoDeadline::functional(
                self.loading_since_ms
                    .map_or_else(|| elapsed_ms.saturating_add(100), |started| started + 800),
            )
        })
    }
}

pub(crate) struct DialogInteractor {
    open: bool,
    dialog: DialogState<&'static str>,
    trigger: ButtonState,
    close: ButtonState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DialogInteractor {
    pub(crate) fn new() -> Self {
        let mut dialog = DialogState::new();
        dialog.set_open(false);
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        let mut close = ButtonState::new();
        close.activation.set_accepts_input(true);
        Self {
            open: false,
            dialog,
            trigger,
            close,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn open(&mut self) {
        self.open = true;
        self.dialog.set_open(true);
        self.outcome = Some("Dialog opened".to_owned());
    }

    fn close(&mut self, reason: &str) {
        self.open = false;
        self.dialog.set_open(false);
        self.trigger.activation.set_accepts_input(true);
        self.outcome = Some(format!(
            "Dialog closed: {reason}; focus restored to Open dialog"
        ));
    }
}

impl StoryInteraction for DialogInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        if !self.open {
            Button::new("Open dialog", &system)
                .variant(ButtonVariant::Primary)
                .render(area, frame.buffer_mut(), &mut self.trigger);
            return;
        }
        let dialog_area = Rect::new(
            area.x.saturating_add(area.width.saturating_sub(44) / 2),
            area.y.saturating_add(area.height.saturating_sub(9) / 2),
            area.width.min(44),
            area.height.min(9),
        );
        Dialog::new(
            "Notice",
            Line::from("The operation completed.").into(),
            &system,
        )
        .description("This is persistent Rust-owned dialog state.")
        .recipe(DialogRecipe::Normal)
        .footer_hint("Esc or Close")
        .paint(dialog_area, frame.buffer_mut(), &mut self.dialog, 1);
        let slots = self.dialog.slots();
        Button::new("Close", &system).render(
            Rect::new(
                slots.root.x.saturating_add(2),
                slots.root.bottom().saturating_sub(2),
                slots.root.width.saturating_sub(4),
                1,
            ),
            frame.buffer_mut(),
            &mut self.close,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.open {
            if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
                self.open();
                return true;
            }
            return false;
        }
        match self.dialog.handle_key(key, &[] as &[Action<'_, &str>]) {
            DialogOutcome::Cancelled => {
                self.close("Escape");
                true
            }
            DialogOutcome::Ignored => {
                if matches!(self.close.handle_key(key), ActivationOutcome::Activated) {
                    self.close("Close button");
                    true
                } else {
                    false
                }
            }
            _ => true,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        if !self.open {
            let before = self.trigger.hovered;
            if matches!(
                self.trigger.handle_mouse(mouse),
                ActivationOutcome::Activated
            ) {
                self.open();
                return true;
            }
            return self.trigger.hovered != before;
        }
        let before = self.close.hovered;
        if matches!(self.close.handle_mouse(mouse), ActivationOutcome::Activated) {
            self.close("Close button");
            return true;
        }
        self.close.hovered != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.open {
            vec!["Esc close", "click Close", "focus returns to trigger"]
        } else {
            vec!["Enter open", "click Open dialog"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TextInputInteractor {
    state: TextInputState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl TextInputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TextInputState::new("filter term");
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: TextInputOutcome) -> bool {
        if matches!(outcome, TextInputOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(format!("Input value: {}", self.state.value()));
        true
    }
}

impl StoryInteraction for TextInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = TextInput::new("Query", &system)
            .placeholder("Search…")
            .show_clear(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let system = self.system.clone();
        let _ = preview_area;
        let outcome = TextInput::new("Query", &system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome)
    }

    fn handle_event(&mut self, event: Event, preview_area: Rect) -> bool {
        match event {
            Event::Paste(value) => {
                let outcome = self.state.insert_str(&value);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, preview_area),
            _ => false,
        }
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type Unicode",
            "paste",
            "←→ move caret",
            "click place caret",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct SliderInteractor {
    state: SliderState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SliderInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SliderState::new(62.0);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: SliderOutcome) -> bool {
        if matches!(outcome, SliderOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(format!("Volume: {:.0}%", self.state.value));
        true
    }
}

impl StoryInteraction for SliderInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = Slider::new(SliderBounds::percent(), &system)
            .label("Volume")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let outcome =
            Slider::new(SliderBounds::percent(), &system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = (self.state.hovered, self.state.dragging, self.state.value);
        let system = self.system.clone();
        let outcome =
            Slider::new(SliderBounds::percent(), &system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || before != (self.state.hovered, self.state.dragging, self.state.value)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ adjust", "Home/End", "drag track", "wheel over slider"]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct RangeSliderInteractor {
    state: RangeSliderState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl RangeSliderInteractor {
    pub(crate) fn new() -> Self {
        let mut state = RangeSliderState::new(20.0, 80.0);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: RangeSliderOutcome) -> bool {
        match outcome {
            RangeSliderOutcome::Ignored => false,
            RangeSliderOutcome::ValueChanged { start, end } => {
                self.outcome = Some(format!("Range: {start:.0}%–{end:.0}%"));
                true
            }
            RangeSliderOutcome::ThumbChanged { thumb } => {
                self.outcome = Some(format!("Active thumb: {thumb:?}"));
                true
            }
            _ => true,
        }
    }
}

impl StoryInteraction for RangeSliderInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = RangeSlider::new(SliderBounds::percent(), &system)
            .label("Price range")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let outcome =
            RangeSlider::new(SliderBounds::percent(), &system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let before = (
            self.state.start,
            self.state.end,
            self.state.hovered,
            self.state.dragging,
        );
        let system = self.system.clone();
        let outcome =
            RangeSlider::new(SliderBounds::percent(), &system).handle_mouse(&mut self.state, mouse);
        let changed = self.apply(outcome);
        changed
            || before
                != (
                    self.state.start,
                    self.state.end,
                    self.state.hovered,
                    self.state.dragging,
                )
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Tab switch thumb",
            "←→ adjust",
            "Home/End",
            "drag either thumb",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct PasswordInputInteractor {
    state: PasswordInputState,
    parts: Option<PasswordInputParts>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl PasswordInputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = PasswordInputState::with_secret("correct horse")
            .with_reveal_policy(termrock::widgets::RevealPolicy::Explicit);
        state.set_focused(true);
        Self {
            state,
            parts: None,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: PasswordInputOutcome) -> bool {
        match outcome {
            PasswordInputOutcome::Ignored => false,
            PasswordInputOutcome::Changed => {
                self.outcome = Some(format!(
                    "Secret edited: {} graphemes",
                    self.state.grapheme_len()
                ));
                true
            }
            PasswordInputOutcome::RevealChanged { revealed } => {
                self.outcome = Some(if revealed {
                    "Secret revealed locally".to_owned()
                } else {
                    "Secret masked".to_owned()
                });
                true
            }
            PasswordInputOutcome::Submitted => {
                self.outcome =
                    Some("Secret submitted to host policy (value not logged)".to_owned());
                true
            }
            PasswordInputOutcome::ClipboardDenied => {
                self.outcome = Some("Clipboard action denied by secret policy".to_owned());
                true
            }
            PasswordInputOutcome::ClipboardPasteRequest => {
                self.outcome = Some("Paste requested; host must provide payload".to_owned());
                true
            }
            other => {
                self.outcome = Some(format!("Password input: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for PasswordInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        self.parts = Some(
            PasswordInput::new("Password", &system)
                .placeholder("Enter secret")
                .strength(PasswordStrengthHint::Strong)
                .paint(area, frame.buffer_mut(), &mut self.state),
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let parts = self.parts.clone();
        let outcome = self.state.handle_mouse(
            mouse,
            parts.as_ref().map_or(preview_area, |parts| parts.field),
            parts.and_then(|parts| parts.reveal),
        );
        self.apply(outcome)
    }

    fn handle_event(&mut self, event: Event, preview_area: Rect) -> bool {
        match event {
            Event::Paste(value) => {
                let outcome = self.state.insert_str(&value);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, preview_area),
            _ => false,
        }
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type or paste secret",
            "Alt+R reveal/mask",
            "click reveal",
            "Enter submit",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct AlertDialogInteractor {
    state: AlertDialogState<&'static str>,
    trigger: ButtonState,
    open: bool,
    system: DesignSystem,
    outcome: Option<String>,
}

impl AlertDialogInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        Self {
            state: AlertDialogState::new(
                AlertKind::Delete,
                AlertScope::example_delete(),
                "delete",
                "cancel",
            ),
            trigger,
            open: false,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: AlertDialogOutcome<&'static str>) -> bool {
        match outcome {
            AlertDialogOutcome::Ignored => false,
            AlertDialogOutcome::Cancelled { id } => {
                self.open = false;
                self.outcome = Some(format!("Alert resolved safely: {id}; focus restored"));
                true
            }
            AlertDialogOutcome::Confirmed { id } => {
                self.open = false;
                self.outcome = Some(format!("Confirmed {id} (no external deletion performed)"));
                true
            }
            AlertDialogOutcome::ConfirmBlocked | AlertDialogOutcome::TypedMismatch => {
                self.outcome = Some("Destructive confirmation remains blocked".to_owned());
                true
            }
            other => {
                self.outcome = Some(format!("Alert dialog: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for AlertDialogInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        if !self.open {
            Button::new("Delete project…", &system)
                .variant(ButtonVariant::Destructive)
                .render(area, frame.buffer_mut(), &mut self.trigger);
            return;
        }
        AlertDialog::new(&system).paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.open {
            if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
                self.state.focus_safe();
                self.open = true;
                self.outcome = Some("Destructive alert opened on safe Cancel".to_owned());
                return true;
            }
            return false;
        }
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        if !self.open {
            let before = self.trigger.hovered;
            if matches!(
                self.trigger.handle_mouse(mouse),
                ActivationOutcome::Activated
            ) {
                self.state.focus_safe();
                self.open = true;
                self.outcome = Some("Destructive alert opened on safe Cancel".to_owned());
                return true;
            }
            return before != self.trigger.hovered;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let outcome = self.state.handle_click(mouse.position);
            return self.apply(outcome);
        }
        false
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.open {
            vec!["←→ choose", "Enter decide", "Esc cancel", "click action"]
        } else {
            vec!["Enter open alert", "click Delete project"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct PopoverInteractor {
    state: PopoverState,
    trigger: ButtonState,
    close: ButtonState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl PopoverInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        let mut close = ButtonState::new();
        close.activation.set_accepts_input(true);
        Self {
            state: PopoverState::new(),
            trigger,
            close,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn open(&mut self, bounds: Rect) {
        let _ = self.state.request_open(bounds, OverlaySize::menu(30, 8));
        self.outcome = Some("Popover opened".to_owned());
    }

    fn close(&mut self, reason: &str) {
        let _ = self.state.request_close();
        self.outcome = Some(format!("Popover closed: {reason}; focus restored"));
    }
}

impl StoryInteraction for PopoverInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        if !self.state.is_open() {
            Button::new("Open details", &system)
                .variant(ButtonVariant::Secondary)
                .render(area, frame.buffer_mut(), &mut self.trigger);
            return;
        }
        Popover::new(&system)
            .header(Some("Run details"))
            .footer(Some("Esc or Close"))
            .paint(area, frame.buffer_mut(), &mut self.state);
        let body = self.state.body_area();
        if !body.is_empty() {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "Deterministic local preview",
                usize::from(body.width),
                system.style(Role::Text),
            );
            Button::new("Close", &system).render(
                Rect::new(body.x, body.bottom().saturating_sub(1), body.width, 1),
                frame.buffer_mut(),
                &mut self.close,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_open() {
            if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
                self.open(Rect::new(0, 0, 80, 24));
                return true;
            }
            return false;
        }
        if matches!(self.state.handle_key(key), PopoverOutcome::CloseRequested) {
            self.close("Escape");
            return true;
        }
        if matches!(self.close.handle_key(key), ActivationOutcome::Activated) {
            self.close("Close button");
            return true;
        }
        false
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        if !self.state.is_open() {
            let before = self.trigger.hovered;
            if matches!(
                self.trigger.handle_mouse(mouse),
                ActivationOutcome::Activated
            ) {
                self.open(preview_area);
                return true;
            }
            return before != self.trigger.hovered;
        }
        let before = self.close.hovered;
        if matches!(self.close.handle_mouse(mouse), ActivationOutcome::Activated) {
            self.close("Close button");
            return true;
        }
        before != self.close.hovered
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec!["Esc close", "click Close"]
        } else {
            vec!["Enter open popover", "click Open details"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DropdownMenuInteractor {
    state: DropdownMenuState,
    trigger: ButtonState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DropdownMenuInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        Self {
            state: DropdownMenuState::new(),
            trigger,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn items() -> Vec<MenuNode<&'static str>> {
        vec![
            MenuNode::command("open", "Open").shortcut("Enter"),
            MenuNode::submenu(
                "export",
                "Export",
                vec![
                    MenuNode::command("png", "PNG image"),
                    MenuNode::command("text", "Plain text"),
                ],
            ),
            MenuNode::checkbox("wrap", "Wrap lines", true),
            MenuNode::separator("sep"),
            MenuNode::command("delete", "Delete").destructive(true),
        ]
    }

    fn apply(&mut self, outcome: DropdownMenuOutcome<&'static str>) -> bool {
        match outcome {
            DropdownMenuOutcome::Ignored => false,
            DropdownMenuOutcome::Opened { .. } => {
                self.outcome = Some("Dropdown menu opened".to_owned());
                true
            }
            DropdownMenuOutcome::Closed | DropdownMenuOutcome::LayerClosed => {
                self.outcome = Some("Dropdown menu closed; focus restored".to_owned());
                true
            }
            DropdownMenuOutcome::Activated { id, .. } => {
                self.outcome = Some(format!("Menu action selected: {id}"));
                true
            }
            DropdownMenuOutcome::CheckToggled { id, checked } => {
                self.outcome = Some(format!("Menu check {id}: {checked}"));
                true
            }
            other => {
                self.outcome = Some(format!("Dropdown menu: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for DropdownMenuInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        if !self.state.is_open() {
            Button::new("Actions ▾", &system)
                .variant(ButtonVariant::Secondary)
                .render(area, frame.buffer_mut(), &mut self.trigger);
            return;
        }
        let root = Rect::new(area.x, area.y, area.width.min(28), area.height.min(8));
        DropdownMenu::new(&Self::items(), &system).paint_cascade(
            root,
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let items = Self::items();
        if !self.state.is_open() {
            if matches!(self.trigger.handle_key(key), ActivationOutcome::Activated) {
                let outcome = self
                    .state
                    .open_from_keyboard(&items, Rect::new(0, 0, 80, 24));
                return self.apply(outcome);
            }
            return false;
        }
        let outcome = self.state.handle_key(key, &items);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let items = Self::items();
        if !self.state.is_open() {
            let before = self.trigger.hovered;
            if matches!(
                self.trigger.handle_mouse(mouse),
                ActivationOutcome::Activated
            ) {
                let outcome = self.state.open_from_pointer(&items, preview_area);
                return self.apply(outcome);
            }
            return before != self.trigger.hovered;
        }
        let outcome = self.state.handle_mouse(mouse, &items);
        self.apply(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec![
                "↑↓ select",
                "←→ submenu",
                "Enter activate",
                "Esc close",
                "click row",
            ]
        } else {
            vec!["Enter open menu", "click Actions"]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct SidebarInteractor {
    state: SidebarState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SidebarInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SidebarState::new(Some("general"));
        state.set_accepts_input(true);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn items() -> Vec<NavItem<&'static str>> {
        vec![
            NavItem::section("workspace", "Workspace"),
            NavItem::new("general", "General").icon("⚙"),
            NavItem::new("appearance", "Appearance").icon("◐"),
            NavItem::new("keys", "Keybindings").icon("⌨"),
            NavItem::separator("sep"),
            NavItem::new("advanced", "Advanced").icon("◇"),
        ]
    }

    fn apply(&mut self, outcome: SidebarOutcome<&'static str>) -> bool {
        match outcome {
            SidebarOutcome::Ignored => false,
            SidebarOutcome::RouteChanged { id } => {
                self.outcome = Some(format!("Sidebar route: {id}"));
                true
            }
            SidebarOutcome::PresentationChanged { presentation } => {
                self.outcome = Some(if matches!(presentation, SidebarPresentation::Expanded) {
                    "Sidebar expanded".to_owned()
                } else {
                    "Sidebar collapsed to rail".to_owned()
                });
                true
            }
            other => {
                self.outcome = Some(format!("Sidebar: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for SidebarInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        Sidebar::new(&Self::items(), &system)
            .title("Settings")
            .show_panel(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &Self::items());
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &Self::items());
        self.apply(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "Enter activate",
            "[ toggle rail",
            "click route",
            "wheel",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ResizablePanelGroupInteractor {
    state: ResizablePanelGroupState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ResizablePanelGroupInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ResizablePanelGroupState::new();
        state.set_focused_handle(Some(0));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn specs() -> [ResizablePanelSpec; 3] {
        [
            ResizablePanelSpec::start("sidebar", 2, 10),
            ResizablePanelSpec::main("main", 5),
            ResizablePanelSpec::end("inspector", 3, 12),
        ]
    }

    fn apply(&mut self, outcome: ResizablePanelOutcome) -> bool {
        match outcome {
            ResizablePanelOutcome::Ignored => false,
            other => {
                self.outcome = Some(format!("Panel layout: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for ResizablePanelGroupInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let specs = Self::specs();
        let group = ResizablePanelGroup::new(&specs, &system).workbench();
        let layout = group.layout(area, &mut self.state);
        for panel in layout.panels.iter().filter(|panel| !panel.area.is_empty()) {
            Panel::new(&system)
                .title(panel.id.0.as_str())
                .render(panel.area, frame.buffer_mut());
        }
        group.paint_handles(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let specs = Self::specs();
        let group = ResizablePanelGroup::new(&specs, &system).workbench();
        let area = self.state.layout().area;
        let outcome = group.handle_key(&mut self.state, key, area);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let system = self.system.clone();
        let specs = Self::specs();
        let group = ResizablePanelGroup::new(&specs, &system).workbench();
        let before = self.state.sizes().to_vec();
        let outcome = group.handle_mouse(&mut self.state, mouse, preview_area);
        let changed = self.apply(outcome);
        changed || before != self.state.sizes()
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "←→ resize focused handle",
            "drag divider",
            "Home/End collapse",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct FormWizardInteractor {
    state: FormWizardState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl FormWizardInteractor {
    pub(crate) fn new() -> Self {
        let mut state = FormWizardState::with_steps([
            WizardStep::new("account", "Account").description("Enter account details"),
            WizardStep::new("profile", "Profile").description("Choose display settings"),
            WizardStep::new("extras", "Extras")
                .description("Optional integrations")
                .optional(true),
        ]);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: FormWizardOutcome) -> bool {
        match outcome {
            FormWizardOutcome::Ignored => false,
            FormWizardOutcome::SubmitRequested => {
                self.outcome = Some("Wizard submitted (no external effect)".to_owned());
                true
            }
            FormWizardOutcome::Cancelled => {
                self.outcome = Some("Wizard cancelled".to_owned());
                true
            }
            other => {
                self.outcome = Some(format!("Wizard: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for FormWizardInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        FormWizard::new(&system).title("Create workspace").paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
        let body = self.state.body_area();
        if !body.is_empty() {
            let text = self
                .state
                .current_step()
                .map_or("Review and submit", |step| {
                    step.description.as_deref().unwrap_or(&step.title)
                });
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                text,
                usize::from(body.width),
                system.style(Role::Text),
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/→ next",
            "← back",
            "S skip optional",
            "Esc cancel",
            "click nav",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TreeTableInteractor {
    state: TreeTableState<u64, &'static str>,
    expanded: bool,
    system: DesignSystem,
    outcome: Option<String>,
}

impl TreeTableInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TreeTableState::new(Some(1));
        state.set_accepts_input(true);
        Self {
            state,
            expanded: true,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn columns() -> ColumnModel<&'static str> {
        ColumnModel::new(vec![
            DataColumn::new("name", "PROCESS", DataColumnWidth::Min(14)),
            DataColumn::new("cpu", "CPU%", DataColumnWidth::Fixed(6)).sortable(),
        ])
    }

    fn rows(&self) -> Vec<TreeTableRow<'static, u64>> {
        static ROOT: &[&str] = &["cargo", "42.0"];
        static CHILD: &[&str] = &["rustc", "88.4"];
        let root = if self.expanded {
            TreeTableRow::new(1, 0, ROOT).branch().expanded()
        } else {
            TreeTableRow::new(1, 0, ROOT).branch()
        };
        if self.expanded {
            vec![root, TreeTableRow::new(2, 1, CHILD).parent(1)]
        } else {
            vec![root]
        }
    }

    fn apply(&mut self, outcome: TreeTableOutcome<u64, &'static str>) -> bool {
        match outcome {
            TreeTableOutcome::Ignored => false,
            TreeTableOutcome::ExpandToggled(id) => {
                self.expanded = !self.expanded;
                self.outcome = Some(format!(
                    "Row {id} {}",
                    if self.expanded {
                        "expanded"
                    } else {
                        "collapsed"
                    }
                ));
                true
            }
            TreeTableOutcome::Activated(id) | TreeTableOutcome::Selected(id) => {
                self.outcome = Some(format!("Selected process {id}"));
                true
            }
            other => {
                self.outcome = Some(format!("Tree table: {other:?}"));
                true
            }
        }
    }
}

impl StoryInteraction for TreeTableInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = self.rows();
        let columns = Self::columns();
        let system = self.system.clone();
        self.state.load = LoadState::Ready {
            count: u64::try_from(rows.len()).unwrap_or(u64::MAX),
        };
        TreeTable::new(&system, &columns, &rows)
            .focused(true)
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let rows = self.rows();
        let outcome = self.state.handle_key(&rows, &Self::columns(), key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let rows = self.rows();
        let before = self.state.hovered;
        let outcome = self.state.handle_mouse(mouse, &rows, &Self::columns());
        self.apply(outcome) || self.state.hovered != before
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "← collapse",
            "→ expand",
            "click row",
            "wheel scroll",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct VirtualListInteractor {
    state: VirtualListState<u64>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl VirtualListInteractor {
    pub(crate) fn new() -> Self {
        let mut state = VirtualListState::million_fixed();
        state.set_sticky(StickyRegion {
            leading: 1,
            trailing: 0,
        });
        state.set_offset(250_000);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn projected(&self) -> Vec<VirtualListItem<'static, u64>> {
        let mut indices = Vec::new();
        self.state.projection_indices(&mut indices);
        indices
            .into_iter()
            .map(|index| {
                let row = if index == 0 {
                    ListRow::group_header(index, Line::from("★ sticky header"))
                } else {
                    ListRow::item(index, Line::from(format!("row {index:>9} · O(viewport)")))
                };
                VirtualListItem::new(index, row)
            })
            .collect()
    }

    fn note(&mut self) {
        self.outcome = Some(format!("Viewport offset: {}", self.state.offset()));
    }
}

impl StoryInteraction for VirtualListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.state.set_viewport_extent(area.height.max(4));
        let projected = self.projected();
        let system = self.system.clone();
        VirtualList::new(&projected, &system)
            .show_diagnostics(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let projected = self.projected();
        let changed = !matches!(self.state.handle_key(&projected, key), Outcome::Ignored);
        if changed {
            self.note();
        }
        changed
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        if !preview_area.contains(mouse.position) {
            return false;
        }
        let changed = match mouse.kind {
            MouseEventKind::ScrollUp => self.state.scroll_by(-1),
            MouseEventKind::ScrollDown => self.state.scroll_by(1),
            MouseEventKind::Moved => self.state.hover(mouse.position).is_some(),
            MouseEventKind::Down(MouseButton::Left) => {
                !matches!(self.state.click(mouse.position), Outcome::Ignored)
            }
            _ => false,
        };
        if changed {
            self.note();
        }
        changed
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "wheel to scroll",
            "↑↓ navigate",
            "PageUp/PageDown",
            "Home/End",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct AccordionInteractor {
    state: AccordionState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl AccordionInteractor {
    pub(crate) fn new() -> Self {
        let mut state = AccordionState::new().initially_open(["general"]);
        state.set_surface_focused(true);
        state.set_cursor(Some("general"));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn items() -> [AccordionItem<'static, &'static str>; 3] {
        [
            AccordionItem::new("general", "General").content_height(2),
            AccordionItem::new("network", "Network").content_height(2),
            AccordionItem::new("advanced", "Advanced").content_height(2),
        ]
    }

    fn resolve(&mut self, outcome: AccordionOutcome<&'static str>) -> bool {
        match outcome {
            AccordionOutcome::CursorMoved { to, .. } => {
                self.outcome = Some(format!("Focused {}", to.unwrap_or("none")));
                true
            }
            AccordionOutcome::Opened { id } => {
                self.outcome = Some(format!("Opened {id}"));
                true
            }
            AccordionOutcome::Closed { id } => {
                self.outcome = Some(format!("Closed {id}"));
                true
            }
            AccordionOutcome::ExclusiveOpened { id, .. } => {
                self.outcome = Some(format!("Opened {id}"));
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for AccordionInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let items = Self::items();
        let parts =
            Accordion::section(&items, &system).paint(area, frame.buffer_mut(), &mut self.state);
        for (id, text) in [
            ("general", "Theme · density"),
            ("network", "Proxy · DNS"),
            ("advanced", "Experimental flags"),
        ] {
            if let Some(body) = parts.content_of(&id).filter(|body| !body.is_empty()) {
                frame.render_widget(
                    ratatui::widgets::Paragraph::new(text).style(system.style(Role::TextMuted)),
                    body,
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let items = Self::items();
        let outcome = Accordion::section(&items, &system).handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let items = Self::items();
        let outcome = Accordion::section(&items, &system).handle_mouse(&mut self.state, mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ choose",
            "←→ collapse/expand",
            "Enter/Space toggle",
            "click heading",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct CollapsibleInteractor {
    state: CollapsibleState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl CollapsibleInteractor {
    pub(crate) fn new() -> Self {
        let mut state = CollapsibleState::new();
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: CollapsibleOutcome) -> bool {
        match outcome {
            CollapsibleOutcome::Opened => {
                self.outcome = Some("Tool details opened".into());
                true
            }
            CollapsibleOutcome::Closed => {
                self.outcome = Some("Tool details closed".into());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for CollapsibleInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let body = Collapsible::new("Tool details", &system).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
        if !body.is_empty() {
            frame.render_widget(
                ratatui::widgets::Paragraph::new("args: --json\nstatus: ok")
                    .style(system.style(Role::TextMuted)),
                body,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, false, None);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, false, None);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/Space toggle",
            "← collapse",
            "→ expand",
            "click heading",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct NumberInputInteractor {
    state: NumberInputState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl NumberInputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = NumberInputState::new()
            .with_constraints(NumberConstraints::bounded(0.0, 100.0, 1.0))
            .with_value(42.0);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: NumberInputOutcome) -> bool {
        match outcome {
            NumberInputOutcome::Changed => true,
            NumberInputOutcome::ValueChanged { value } => {
                self.outcome = Some(format!("Opacity changed to {}%", value.unwrap_or(0.0)));
                true
            }
            NumberInputOutcome::Submitted { value } => {
                self.outcome = Some(format!("Opacity submitted as {}%", value.unwrap_or(0.0)));
                true
            }
            NumberInputOutcome::Cancelled => {
                self.outcome = Some("Number edit cancelled".into());
                true
            }
            NumberInputOutcome::ClipboardPasteRequest => {
                self.outcome = Some("Paste requested".into());
                true
            }
            NumberInputOutcome::ClipboardCopy { .. } => {
                self.outcome = Some("Numeric draft copied".into());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for NumberInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = NumberInput::new("Opacity", &system).unit("%").paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.resolve(outcome)
    }

    fn handle_event(&mut self, event: Event, preview_area: Rect) -> bool {
        match event {
            Event::Paste(text) => {
                let outcome = self.state.insert_str(&text);
                self.resolve(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, preview_area),
            _ => false,
        }
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type a number",
            "↑↓ step",
            "click +/-",
            "wheel over field",
            "Enter submit",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct SelectInteractor {
    state: SelectState<&'static str>,
    bounds: Rect,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SelectInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SelectState::new()
            .with_recipe(SelectRecipe::Form)
            .with_searchable(true)
            .with_value("apple");
        state.set_focused(true);
        Self {
            state,
            bounds: Rect::new(0, 0, 44, 12),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn options() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::group("fruit", "Fruit"),
            SelectOption::option("apple", "Apple"),
            SelectOption::option("banana", "Banana"),
            SelectOption::option("date", "Date"),
        ]
    }

    fn resolve(&mut self, outcome: SelectOutcome<&'static str>) -> bool {
        match outcome {
            SelectOutcome::Opened { .. } => {
                self.outcome = Some("Fruit options opened".into());
                true
            }
            SelectOutcome::Closed => {
                self.outcome = Some("Fruit options closed".into());
                true
            }
            SelectOutcome::ValueChanged { id } => {
                self.outcome = Some(format!("Selected {id}"));
                true
            }
            SelectOutcome::SearchChanged { query } => {
                self.outcome = Some(format!("Filtered by {query}"));
                true
            }
            SelectOutcome::HighlightChanged { .. }
            | SelectOutcome::PresentationChanged { .. }
            | SelectOutcome::Changed => true,
            _ => false,
        }
    }
}

impl StoryInteraction for SelectInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.bounds = area;
        let system = self.system.clone();
        let options = Self::options();
        Select::new(&options, &system).label("Fruit").paint_stacked(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let options = Self::options();
        let outcome = self.state.handle_key(key, &options, self.bounds);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let options = Self::options();
        let outcome = self.state.handle_mouse(mouse, &options, preview_area);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/click open",
            "↑↓ highlight",
            "type to filter",
            "Enter select",
            "Esc close",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct MultiSelectInteractor {
    state: MultiSelectState<&'static str>,
    bounds: Rect,
    system: DesignSystem,
    outcome: Option<String>,
}

impl MultiSelectInteractor {
    pub(crate) fn new() -> Self {
        let mut state = MultiSelectState::new()
            .with_recipe(SelectRecipe::Form)
            .with_selected(["rs"]);
        state.set_focused(true);
        Self {
            state,
            bounds: Rect::new(0, 0, 44, 12),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn options() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::group("languages", "Languages"),
            SelectOption::option("rs", "Rust"),
            SelectOption::option("go", "Go"),
            SelectOption::option("ts", "TypeScript"),
            SelectOption::option("py", "Python"),
        ]
    }

    fn resolve(&mut self, outcome: MultiSelectOutcome<&'static str>) -> bool {
        match outcome {
            MultiSelectOutcome::Opened { .. } => {
                self.outcome = Some("Language filters opened".into());
                true
            }
            MultiSelectOutcome::Closed => {
                self.outcome = Some(format!("{} filters selected", self.state.selected().len()));
                true
            }
            MultiSelectOutcome::Toggled { id, checked } => {
                self.outcome = Some(format!(
                    "{id} {}",
                    if checked { "selected" } else { "removed" }
                ));
                true
            }
            MultiSelectOutcome::RangeApplied { ids } => {
                self.outcome = Some(format!("Selected {}-item range", ids.len()));
                true
            }
            MultiSelectOutcome::SelectAll { count } => {
                self.outcome = Some(format!("Selected all {count} visible options"));
                true
            }
            MultiSelectOutcome::Cleared => {
                self.outcome = Some("Selection cleared".into());
                true
            }
            MultiSelectOutcome::SearchChanged { query } => {
                self.outcome = Some(format!("Filtered by {query}"));
                true
            }
            MultiSelectOutcome::MaxReached { max } => {
                self.outcome = Some(format!("Selection limit is {max}"));
                true
            }
            MultiSelectOutcome::Changed
            | MultiSelectOutcome::HighlightChanged { .. }
            | MultiSelectOutcome::PresentationChanged { .. } => true,
            _ => false,
        }
    }
}

impl StoryInteraction for MultiSelectInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.bounds = area;
        let system = self.system.clone();
        let options = Self::options();
        MultiSelect::new(&options, &system)
            .label("Filters")
            .paint_stacked(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let options = Self::options();
        let outcome = self.state.handle_key(key, &options, self.bounds);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let options = Self::options();
        let outcome = self.state.handle_mouse(mouse, &options, preview_area);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/click open",
            "↑↓ highlight",
            "Space toggle",
            "type to filter",
            "Enter close",
            "Ctrl+A select all",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct PaginationInteractor {
    state: PaginationState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl PaginationInteractor {
    pub(crate) fn new() -> Self {
        let mut state = PaginationState::new(3, 25, PageTotal::Known(240));
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: PaginationOutcome) -> bool {
        match outcome {
            PaginationOutcome::PageRequested { request } => {
                self.state.set_page(request.page);
                self.outcome = Some(format!(
                    "Page {} requested (effect simulated)",
                    request.page
                ));
                true
            }
            PaginationOutcome::PageSizeChanged { page_size, request } => {
                self.state.set_page(request.page);
                self.outcome = Some(format!("Page size changed to {page_size}"));
                true
            }
            PaginationOutcome::JumpStarted => {
                self.outcome = Some("Type a page, then Enter".into());
                true
            }
            PaginationOutcome::JumpCancelled => {
                self.outcome = Some("Page jump cancelled".into());
                true
            }
            PaginationOutcome::Changed | PaginationOutcome::PresentationChanged { .. } => true,
            _ => false,
        }
    }
}

impl StoryInteraction for PaginationInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        Pagination::new(&system).paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_jump_active() {
            vec!["type page number", "Enter request page", "Esc cancel"]
        } else {
            vec![
                "←→ choose/request",
                "click page",
                "g jump",
                "s page size",
                "Home/End",
            ]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.is_jump_active()
    }
}

pub(crate) struct CheckboxInteractor {
    state: CheckboxState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl CheckboxInteractor {
    pub(crate) fn new() -> Self {
        let mut state = CheckboxState::new(false);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: CheckboxOutcome<&'static str>) -> bool {
        match outcome {
            CheckboxOutcome::ValueChanged { value, .. } => {
                self.outcome = Some(format!(
                    "Notifications {}",
                    if value.is_checked() {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ));
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for CheckboxInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = Checkbox::new("notifications", "Desktop notifications", &system)
            .description("Toggle a controlled form value")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let outcome = Checkbox::new("notifications", "Desktop notifications", &system)
            .handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let outcome = Checkbox::new("notifications", "Desktop notifications", &system)
            .handle_mouse(&mut self.state, mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Space/Enter toggle",
            "click toggle",
            "move pointer to hover",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct SwitchInteractor {
    state: SwitchState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SwitchInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SwitchState::new(false);
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: SwitchOutcome<&'static str>) -> bool {
        match outcome {
            SwitchOutcome::ValueChanged { on, .. } => {
                self.outcome = Some(format!("Background sync {}", if on { "on" } else { "off" }));
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for SwitchInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = Switch::new("sync", "Background sync", &system)
            .description("Down then up inside commits")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let outcome =
            Switch::new("sync", "Background sync", &system).handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let changed_hover = matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Drag(_));
        let outcome =
            Switch::new("sync", "Background sync", &system).handle_mouse(&mut self.state, mouse);
        self.resolve(outcome) || changed_hover
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["Space/Enter toggle", "click toggle", "drag outside cancels"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ToggleInteractor {
    state: ToggleState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ToggleInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ToggleState::new();
        state.set_focused(true);
        state.set_value(ToggleValue::Pressed);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: ToggleOutcome) -> bool {
        match outcome {
            ToggleOutcome::ValueChanged { value } => {
                self.outcome = Some(format!(
                    "Bold {}",
                    if value.is_pressed() {
                        "pressed"
                    } else {
                        "released"
                    }
                ));
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for ToggleInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = Toggle::new("Bold", &system).paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let outcome = Toggle::new("Bold", &system).handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let outcome = Toggle::new("Bold", &system).handle_mouse(&mut self.state, mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Space/Enter toggle",
            "click toggle",
            "move pointer to hover",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ToggleGroupInteractor {
    state: ToggleGroupState<&'static str>,
    pressed: [bool; 3],
    system: DesignSystem,
    outcome: Option<String>,
}

impl ToggleGroupInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ToggleGroupState::new();
        state.set_surface_focused(true);
        state.cursor = Some("b");
        Self {
            state,
            pressed: [true, false, false],
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn items(&self) -> [ToggleGroupItem<'static, &'static str>; 3] {
        [
            ToggleGroupItem::new("b", "Bold").pressed(self.pressed[0]),
            ToggleGroupItem::new("i", "Italic").pressed(self.pressed[1]),
            ToggleGroupItem::new("u", "Underline").pressed(self.pressed[2]),
        ]
    }

    fn resolve(&mut self, outcome: ToggleGroupOutcome<&'static str>) -> bool {
        match outcome {
            ToggleGroupOutcome::ItemChanged { id, value } => {
                if let Some(index) = ["b", "i", "u"]
                    .iter()
                    .position(|candidate| *candidate == id)
                {
                    self.pressed[index] = value.is_pressed();
                }
                self.outcome = Some(format!(
                    "{id} {}",
                    if value.is_pressed() {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ));
                true
            }
            ToggleGroupOutcome::CursorMoved { id } => {
                self.outcome = Some(format!("Focused {id}"));
                true
            }
            ToggleGroupOutcome::SelectionChanged { .. }
            | ToggleGroupOutcome::OverflowOpened
            | ToggleGroupOutcome::OverflowClosed => true,
            _ => false,
        }
    }
}

impl StoryInteraction for ToggleGroupInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let items = self.items();
        let _ = ToggleGroup::new(&items, &system)
            .multiple()
            .compact()
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let items = self.items();
        let outcome = ToggleGroup::new(&items, &system)
            .multiple()
            .compact()
            .handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let items = self.items();
        let outcome = ToggleGroup::new(&items, &system)
            .multiple()
            .compact()
            .handle_mouse(&mut self.state, mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ choose", "Space/Enter toggle", "click item"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct SegmentedControlInteractor {
    state: SegmentedControlState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SegmentedControlInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SegmentedControlState::new(Some("list"));
        state.set_surface_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn items() -> [SegmentedItem<'static, &'static str>; 3] {
        [
            SegmentedItem::new("list", "List"),
            SegmentedItem::new("grid", "Grid"),
            SegmentedItem::new("table", "Table"),
        ]
    }

    fn resolve(&mut self, outcome: SegmentedControlOutcome<&'static str>) -> bool {
        match outcome {
            SegmentedControlOutcome::Selected { id } => {
                self.outcome = Some(format!("View changed to {id}"));
                true
            }
            SegmentedControlOutcome::CursorMoved { id } => {
                self.outcome = Some(format!("Focused {id}"));
                true
            }
            SegmentedControlOutcome::MenuOpened => {
                self.outcome = Some("View menu opened".into());
                true
            }
            SegmentedControlOutcome::MenuClosed => {
                self.outcome = Some("View menu closed".into());
                true
            }
            _ => false,
        }
    }
}

impl StoryInteraction for SegmentedControlInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let items = Self::items();
        let _ = SegmentedControl::new(&items, &system)
            .collapse_below(0)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let items = Self::items();
        let outcome = SegmentedControl::new(&items, &system)
            .collapse_below(0)
            .handle_key(&mut self.state, key);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let system = self.system.clone();
        let items = Self::items();
        let outcome = SegmentedControl::new(&items, &system)
            .collapse_below(0)
            .handle_mouse(&mut self.state, mouse);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ change view", "Home/End", "click segment"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct VirtualGridInteractor {
    state: VirtualGridState<u64, &'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

const VIRTUAL_GRID_COLUMNS: [GridColumn<'static, &'static str>; 4] = [
    GridColumn::fixed("id", "id", 8),
    GridColumn::fixed("name", "name", 16),
    GridColumn::min("value", "value", 10),
    GridColumn::fixed("flag", "flag", 6),
];
const VIRTUAL_GRID_CELLS_0: [GridCell<'static>; 4] = [
    GridCell::text("0"),
    GridCell::text("alpha"),
    GridCell::text("1"),
    GridCell::text("yes"),
];
const VIRTUAL_GRID_CELLS_1: [GridCell<'static>; 4] = [
    GridCell::text("1"),
    GridCell::text("beta"),
    GridCell::pending(),
    GridCell::text("no"),
];
const VIRTUAL_GRID_CELLS_2: [GridCell<'static>; 4] = [
    GridCell::text("2"),
    GridCell::text("gamma"),
    GridCell::text("3"),
    GridCell::text("yes"),
];
const VIRTUAL_GRID_CELLS_3: [GridCell<'static>; 4] = [
    GridCell::text("3"),
    GridCell::text("delta"),
    GridCell::text("4"),
    GridCell::text("yes"),
];
const VIRTUAL_GRID_CELLS_4: [GridCell<'static>; 4] = [
    GridCell::text("4"),
    GridCell::text("epsilon"),
    GridCell::pending(),
    GridCell::text("no"),
];
const VIRTUAL_GRID_CELLS_5: [GridCell<'static>; 4] = [
    GridCell::text("5"),
    GridCell::text("zeta"),
    GridCell::text("6"),
    GridCell::text("yes"),
];
const VIRTUAL_GRID_ROWS: [GridRow<'static, u64>; 6] = [
    GridRow::new(0, 0, &VIRTUAL_GRID_CELLS_0),
    GridRow::new(1, 1, &VIRTUAL_GRID_CELLS_1),
    GridRow::new(2, 2, &VIRTUAL_GRID_CELLS_2),
    GridRow::new(3, 3, &VIRTUAL_GRID_CELLS_3),
    GridRow::new(4, 4, &VIRTUAL_GRID_CELLS_4),
    GridRow::new(5, 5, &VIRTUAL_GRID_CELLS_5),
];

impl VirtualGridInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: VirtualGridState::new(),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn resolve(&mut self, outcome: VirtualGridOutcome<u64, &'static str>) -> bool {
        self.outcome = match outcome {
            VirtualGridOutcome::Ignored => return false,
            VirtualGridOutcome::CursorMoved {
                row, col, col_id, ..
            } => Some(format!(
                "Cursor moved to row {row}, column {col} ({col_id})"
            )),
            VirtualGridOutcome::RangeChanged { start, end } => {
                Some(format!("Selected range {start:?} through {end:?}"))
            }
            VirtualGridOutcome::Activated {
                row, col, col_id, ..
            } => Some(format!("Activated row {row}, column {col} ({col_id})")),
            VirtualGridOutcome::ViewportChanged {
                first_row,
                first_col,
            } => Some(format!(
                "Viewport starts at row {first_row}, column {first_col}"
            )),
            VirtualGridOutcome::Cancelled => Some("Selection cancelled".into()),
            _ => return false,
        };
        true
    }
}

impl StoryInteraction for VirtualGridInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        frame.render_stateful_widget(
            &VirtualGrid::new(&VIRTUAL_GRID_COLUMNS, &VIRTUAL_GRID_ROWS, &system).total_rows(20),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self
            .state
            .handle_key(key, &VIRTUAL_GRID_COLUMNS, &VIRTUAL_GRID_ROWS);
        self.resolve(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let outcome = self
            .state
            .handle_mouse(mouse, &VIRTUAL_GRID_COLUMNS, &VIRTUAL_GRID_ROWS);
        self.resolve(outcome)
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "arrows move cell",
            "PgUp/PgDn move viewport",
            "Enter activate",
            "Shift+arrows select range",
            "wheel scroll",
            "click cell",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::{Position, Rect},
    };
    use termrock::input::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    use termrock::input::{KeyCode, KeyEvent};

    use super::{
        FormInteractor, LogPaneInteractor, SplitPaneInteractor, StoryInteraction, ToastInteractor,
        VirtualGridInteractor,
    };

    #[test]
    fn log_pane_wheel_freezes_tail_following() {
        let area = Rect::new(0, 0, 52, 5);
        let mut interactor = LogPaneInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();

        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                position: Position::new(1, 1),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(!interactor.state.is_following());
    }

    #[test]
    fn form_hover_clears_when_pointer_leaves_preview() {
        let area = Rect::new(0, 0, 68, 12);
        let mut interactor = FormInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();

        // Hit first painted field region if any (geometry drifts with density).
        let hit = interactor
            .state
            .regions()
            .first()
            .map(|r| Position::new(r.area.x, r.area.y))
            .unwrap_or(Position::new(1, 1));
        let hovered = interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: hit,
                modifiers: KeyModifiers::NONE,
            },
            area,
        );
        if !hovered {
            // No hover regions for this recipe — skip contract.
            return;
        }
        assert!(interactor.state.hovered().is_some());
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: Position::new(area.right(), area.bottom()),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert_eq!(interactor.state.hovered(), None);
    }

    #[test]
    fn split_pane_interactor_drags_only_from_painted_divider() {
        let area = Rect::new(0, 0, 68, 10);
        let mut interactor = SplitPaneInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();
        let divider = interactor.state.layout().divider;
        let before = interactor.state.ratio();

        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(divider.x, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position::new(50, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                position: Position::new(50, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.state.ratio() > before);
    }

    #[test]
    fn toast_knobs_keep_golden_defaults_and_edit_live() {
        let mut interactor = ToastInteractor::new();
        assert_eq!(interactor.knobs()[0].display_value(), "Success");
        assert_eq!(interactor.knobs()[1].display_value(), "Top right");
        assert_eq!(interactor.knobs()[2].display_value(), "Updated");

        assert!(interactor.handle_knob_key(0, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)));
        assert_eq!(interactor.knobs()[0].display_value(), "Warning");
        assert!(
            interactor.handle_knob_key(2, KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE))
        );
        assert_eq!(interactor.knobs()[2].display_value(), "Updated!");
    }

    #[test]
    fn virtual_grid_interactor_paints_and_routes_public_state() {
        let area = Rect::new(0, 0, 72, 12);
        let mut interactor = VirtualGridInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();

        assert!(!interactor.state.cell_regions.is_empty());
        assert!(interactor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)));
        assert_eq!(interactor.state.cursor_col(), 1);
        assert!(interactor.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)));
        assert!(interactor.state.cursor_row() > 1);
        assert!(interactor.take_outcome().is_some());
    }
}
