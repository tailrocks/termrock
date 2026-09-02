// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent interactions for scrollable, selectable, and inspectable views.
use ratatui::{Frame, layout::Rect, widgets::StatefulWidget};
use termrock::{
    input::{KeyCode, KeyEvent, MouseEvent},
    style::{DesignSystem, RolePalette},
    widgets::{
        CodeBlock, CodeBlockOutcome, CodeBlockState, DetailCapability, DetailRow, DetailTable,
        DetailTableOutcome, DetailTableState, Diagnostic, DiagnosticOutcome, DiagnosticRecipe,
        DiagnosticSeverity, DiagnosticState, DiagnosticView, DiffMode, DiffView, DiffViewOutcome,
        DiffViewState, EventStream, EventStreamOutcome, EventStreamState, HelpEntry, HexViewer,
        HexViewerOutcome, HexViewerState, HexWindow, HistoryEntry, HistoryPicker,
        HistoryPickerOutcome, HistoryPickerState, InspectKind, InspectorField, KeyboardHelp,
        KeyboardHelpOutcome, KeyboardHelpState, LogStream, LogStreamOutcome, LogStreamState,
        MarkdownOutcome, MarkdownView, MarkdownViewState, ObjectInspector, ObjectInspectorOutcome,
        ObjectInspectorState, TerminalCommandMeta, TerminalOutput, TerminalOutputOutcome,
        TerminalOutputState, TerminalRunStatus, Timeline, TimelineEvent, TimelineOutcome,
        TimelineState, TimelineStatus, example_help_entries, example_history_entries,
        filter_help_entries, filter_history_entries, project_markdown,
    },
};

use super::{StoryInteraction, extended::record};
use crate::stories::{
    diff_sample_lines, event_stream_sample, hex_viewer_sample, log_stream_sample_lines,
    terminal_output_sample_lines,
};

pub(crate) struct CodeBlockInteractor {
    state: CodeBlockState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl CodeBlockInteractor {
    pub(crate) fn new() -> Self {
        let mut state = CodeBlockState::new();
        state.set_focused(true);
        state.set_cursor_line(Some(1));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn lines() -> [&'static str; 12] {
        [
            "fn main() {",
            "    let project = \"TermRock\";",
            "    println!(\"{project}\");",
            "}",
            "",
            "fn render() {",
            "    // scroll, select, and copy",
            "    draw_frame();",
            "}",
            "",
            "// Unicode stays aligned",
            "let status = \"ready 🧪\";",
        ]
    }
    fn apply(&mut self, outcome: CodeBlockOutcome) -> bool {
        record(&mut self.outcome, "CodeBlock", outcome)
    }
}

impl StoryInteraction for CodeBlockInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let lines = Self::lines();
        let _ = CodeBlock::new(&lines, &system)
            .language("rust")
            .path("src/main.rs")
            .line_numbers(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let lines = Self::lines();
        let outcome = CodeBlock::new(&lines, &system)
            .language("rust")
            .line_numbers(true)
            .handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = self.system.clone();
        let lines = Self::lines();
        let outcome = CodeBlock::new(&lines, &system)
            .language("rust")
            .line_numbers(true)
            .handle_mouse(&mut self.state, mouse);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move line",
            "wheel scroll",
            "click line",
            "Enter activate",
            "C copy",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DetailTableInteractor {
    state: DetailTableState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DetailTableInteractor {
    pub(crate) fn new() -> Self {
        let mut state = DetailTableState::default();
        state.selected = Some("reference");
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn rows() -> [DetailRow<'static, &'static str>; 4] {
        [
            DetailRow {
                id: "state",
                label: "State",
                value: "Ready",
                href: None,
                capability: DetailCapability::Copy,
                emphasis: true,
                style: None,
            },
            DetailRow {
                id: "reference",
                label: "Reference",
                value: "https://termrock.dev",
                href: Some("https://termrock.dev"),
                capability: DetailCapability::CopyAndLink,
                emphasis: false,
                style: None,
            },
            DetailRow {
                id: "region",
                label: "Region",
                value: "ap-southeast-1",
                href: None,
                capability: DetailCapability::None,
                emphasis: false,
                style: None,
            },
            DetailRow {
                id: "owner",
                label: "Owner",
                value: "platform / runtime",
                href: None,
                capability: DetailCapability::Copy,
                emphasis: false,
                style: None,
            },
        ]
    }
    fn apply(&mut self, outcome: DetailTableOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "DetailTable", outcome)
    }
}

impl StoryInteraction for DetailTableInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let rows = Self::rows();
        frame.render_stateful_widget(
            &DetailTable::new(&rows, &system).label_width(14).wrap(true),
            area,
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(&Self::rows(), key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) -> bool {
        false
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["↑↓ select row", "Enter copy/open"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DiagnosticViewInteractor {
    state: DiagnosticState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DiagnosticViewInteractor {
    pub(crate) fn new() -> Self {
        let mut state = DiagnosticState::new();
        state.set_accepts_input(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn items() -> [Diagnostic<'static>; 3] {
        [
            Diagnostic::new("d1", DiagnosticSeverity::Error, "mismatched types")
                .code("E0308")
                .source("rustc")
                .file("src/main.rs"),
            Diagnostic::new("d2", DiagnosticSeverity::Warning, "unused variable: `y`")
                .source("rustc")
                .file("src/main.rs"),
            Diagnostic::new(
                "d3",
                DiagnosticSeverity::Info,
                "build finished with warnings",
            )
            .source("cargo"),
        ]
    }
    fn apply(&mut self, outcome: DiagnosticOutcome) -> bool {
        record(&mut self.outcome, "DiagnosticView", outcome)
    }
}

impl StoryInteraction for DiagnosticViewInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let items = Self::items();
        DiagnosticView::new(&items, &system)
            .recipe(DiagnosticRecipe::List)
            .title("Problems")
            .render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &Self::items());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &Self::items());
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "Enter expand",
            "wheel scroll",
            "click diagnostic",
            "C copy",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DiffViewInteractor {
    state: DiffViewState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl DiffViewInteractor {
    pub(crate) fn new() -> Self {
        let mut state = DiffViewState::new();
        state.mode = DiffMode::Unified;
        state.set_accepts_input(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: DiffViewOutcome) -> bool {
        record(&mut self.outcome, "DiffView", outcome)
    }
}
impl StoryInteraction for DiffViewInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let (lines, hunks) = diff_sample_lines();
        DiffView::new(&lines, &system)
            .hunks(&hunks)
            .title("main.rs")
            .render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let (lines, hunks) = diff_sample_lines();
        let outcome = self.state.handle_key(key, &lines, &hunks);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let (lines, hunks) = diff_sample_lines();
        let outcome = self.state.handle_mouse(mouse, &lines, &hunks);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "N/P next/previous hunk",
            "/ search",
            "wheel scroll",
            "click line",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.search.is_some()
    }
}

pub(crate) struct EventStreamInteractor {
    state: EventStreamState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}
impl EventStreamInteractor {
    pub(crate) fn new() -> Self {
        let mut state = EventStreamState::new();
        state.set_accepts_input(true);
        state.set_following(false);
        state.cursor = 1;
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: EventStreamOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "EventStream", outcome)
    }
}
impl StoryInteraction for EventStreamInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let events = event_stream_sample();
        self.state
            .on_append(events.len() as u16, area.height.saturating_sub(1));
        EventStream::with_events(&events, &system)
            .focused(true)
            .render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &event_stream_sample());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &event_stream_sample());
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "Enter details",
            "/ filter",
            "F follow",
            "wheel scroll",
            "click event",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.filter.is_some()
    }
}

pub(crate) struct HexViewerInteractor {
    state: HexViewerState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl HexViewerInteractor {
    pub(crate) fn new() -> Self {
        let mut state = HexViewerState::new();
        state.set_accepts_input(true);
        state.cursor = 0x10;
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: HexViewerOutcome) -> bool {
        record(&mut self.outcome, "HexViewer", outcome)
    }
}
impl StoryInteraction for HexViewerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let data = hex_viewer_sample();
        let window = HexWindow::new(0, &data, data.len() as u64);
        HexViewer::new(window, &system).title("blob.bin").render(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let data = hex_viewer_sample();
        let window = HexWindow::new(0, &data, data.len() as u64);
        let outcome = self.state.handle_key(key, &window);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let data = hex_viewer_sample();
        let window = HexWindow::new(0, &data, data.len() as u64);
        let outcome = self.state.handle_mouse(mouse, &window);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "arrows move byte",
            "Shift+arrows select",
            "wheel scroll",
            "click byte",
            "I inspector",
            "B bookmark",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct HistoryPickerInteractor {
    state: HistoryPickerState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}
impl HistoryPickerInteractor {
    pub(crate) fn new() -> Self {
        let mut state = HistoryPickerState::new();
        let _ = state.open(None);
        state.set_focused(true);
        state.set_accepts_input(true);
        let entries = example_history_entries();
        state.reconcile(&entries);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn entries(&self) -> Vec<HistoryEntry<&'static str>> {
        let all = example_history_entries();
        filter_history_entries(&all, self.state.query_text())
    }
    fn apply(&mut self, outcome: HistoryPickerOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "HistoryPicker", outcome)
    }
}
impl StoryInteraction for HistoryPickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let entries = self.entries();
        self.state.reconcile(&entries);
        HistoryPicker::new(&entries, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_open() && matches!(key.code, KeyCode::Char('o' | 'O') | KeyCode::Enter) {
            let outcome = self.state.open(None);
            self.state.set_focused(true);
            return self.apply(outcome);
        }
        let entries = self.entries();
        let outcome = self.state.handle_key(key, &entries);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let entries = self.entries();
        let outcome = self.state.handle_mouse(mouse, &entries);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec![
                "type to filter",
                "↑↓ choose",
                "Enter restore",
                "Ctrl+P pin",
                "Ctrl+D delete",
                "click entry",
            ]
        } else {
            vec!["O/Enter reopen"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct KeyboardHelpInteractor {
    state: KeyboardHelpState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl KeyboardHelpInteractor {
    pub(crate) fn new() -> Self {
        let mut state = KeyboardHelpState::new();
        state.set_focused(true);
        state.set_accepts_input(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn entries(&self, system: &DesignSystem) -> Vec<HelpEntry> {
        let all = example_help_entries(system);
        filter_help_entries(&all, self.state.query_text())
    }
    fn apply(&mut self, outcome: KeyboardHelpOutcome) -> bool {
        record(&mut self.outcome, "KeyboardHelp", outcome)
    }
}
impl StoryInteraction for KeyboardHelpInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let entries = self.entries(&system);
        KeyboardHelp::new(&entries, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let entries = self.entries(&system);
        let outcome = self.state.handle_key(key, &entries);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = self.system.clone();
        let entries = self.entries(&system);
        let outcome = self.state.handle_mouse(mouse, &entries);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "? open help",
            "type to filter",
            "↑↓ navigate",
            "wheel scroll",
            "Esc close",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.is_open()
    }
}

pub(crate) struct LogStreamInteractor {
    state: LogStreamState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl LogStreamInteractor {
    pub(crate) fn new() -> Self {
        let mut state = LogStreamState::new();
        state.set_accepts_input(true);
        state.set_following(false);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: LogStreamOutcome) -> bool {
        record(&mut self.outcome, "LogStream", outcome)
    }
}
impl StoryInteraction for LogStreamInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let lines = log_stream_sample_lines();
        self.state
            .on_append(lines.len() as u16, area.height.saturating_sub(1));
        LogStream::new(&lines, &system).title("app.log").render(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let lines = log_stream_sample_lines();
        let outcome = self.state.handle_key(key, &lines);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let lines = log_stream_sample_lines();
        let outcome = self.state.handle_mouse(mouse, &lines);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "wheel scroll",
            "F follow",
            "M pin",
            "/ search",
            "Enter inspect",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct MarkdownViewInteractor {
    state: MarkdownViewState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl MarkdownViewInteractor {
    pub(crate) fn new() -> Self {
        let mut state = MarkdownViewState::new();
        state.focused = true;
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn source() -> &'static str {
        "# TermRock\n\nUse [Ratatui](https://ratatui.rs/) as the paint engine.\n\n- composable widgets\n- real interaction\n\n```rust\nlet ui = TermRock::new();\n```\n"
    }
    fn apply(&mut self, outcome: MarkdownOutcome) -> bool {
        record(&mut self.outcome, "MarkdownView", outcome)
    }
}
impl StoryInteraction for MarkdownViewInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let blocks = project_markdown(Self::source());
        let _ =
            MarkdownView::new(&blocks, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = self.system.clone();
        let blocks = project_markdown(Self::source());
        let outcome = MarkdownView::new(&blocks, &system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = self.system.clone();
        let blocks = project_markdown(Self::source());
        let outcome = MarkdownView::new(&blocks, &system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ scroll",
            "Tab choose link",
            "Enter open link",
            "wheel scroll",
            "click link",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ObjectInspectorInteractor {
    state: ObjectInspectorState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl ObjectInspectorInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ObjectInspectorState::new();
        state.set_accepts_input(true);
        state.set_cursor(1);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn fields() -> [InspectorField<'static>; 4] {
        [
            InspectorField::container("spec", "spec", InspectKind::Object)
                .child_count(2)
                .expanded(),
            InspectorField::new("name", "api-gateway")
                .path("spec.name")
                .depth(1)
                .kind(InspectKind::String)
                .editable(),
            InspectorField::new("replicas", "3")
                .path("spec.replicas")
                .depth(1)
                .kind(InspectKind::Number)
                .editable(),
            InspectorField::new("status", "Running")
                .path("status")
                .kind(InspectKind::String),
        ]
    }
    fn apply(&mut self, outcome: ObjectInspectorOutcome) -> bool {
        record(&mut self.outcome, "ObjectInspector", outcome)
    }
}
impl StoryInteraction for ObjectInspectorInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let fields = Self::fields();
        ObjectInspector::new(&fields, &system).render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &Self::fields());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &Self::fields());
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "←→ collapse/expand",
            "E edit",
            "/ search",
            "C copy",
            "click field",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.editing || self.state.search().is_some()
    }
}

pub(crate) struct TerminalOutputInteractor {
    state: TerminalOutputState,
    system: DesignSystem,
    outcome: Option<String>,
}
impl TerminalOutputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TerminalOutputState::new();
        state.set_accepts_input(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn meta() -> TerminalCommandMeta<'static> {
        TerminalCommandMeta::new("cargo test -p termrock --lib")
            .cwd("/workspace/termrock")
            .status(TerminalRunStatus::Running)
            .duration_ms(3400)
            .pid(4242)
    }
    fn apply(&mut self, outcome: TerminalOutputOutcome) -> bool {
        record(&mut self.outcome, "TerminalOutput", outcome)
    }
}
impl StoryInteraction for TerminalOutputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let lines = terminal_output_sample_lines();
        let meta = Self::meta();
        self.state
            .on_append(lines.len() as u16, area.height.saturating_sub(4));
        TerminalOutput::new(&meta, &lines, &system)
            .title("build")
            .render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let lines = terminal_output_sample_lines();
        let meta = Self::meta();
        let outcome = self.state.handle_key(key, &lines, &meta);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let lines = terminal_output_sample_lines();
        let outcome = self.state.handle_mouse(mouse, &lines);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "wheel scroll",
            "F follow",
            "C cancel",
            "Y copy command",
            "click stream filter",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TimelineInteractor {
    state: TimelineState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}
impl TimelineInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TimelineState::new();
        state.set_accepts_input(true);
        state.following = false;
        state.cursor = 1;
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn events() -> [TimelineEvent<'static, &'static str>; 3] {
        [
            TimelineEvent::with_id("a", "12:01", "Started deploy")
                .status(TimelineStatus::Success)
                .actor("ci")
                .duration("12s"),
            TimelineEvent::with_id("b", "12:02", "Running tests")
                .status(TimelineStatus::Running)
                .active()
                .actor("ci"),
            TimelineEvent::with_id("c", "12:03", "Open PR")
                .status(TimelineStatus::Pending)
                .actor("bot"),
        ]
    }
    fn apply(&mut self, outcome: TimelineOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Timeline", outcome)
    }
}
impl StoryInteraction for TimelineInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let events = Self::events();
        Timeline::with_events(&events, &system)
            .focused(true)
            .render(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &Self::events());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &Self::events());
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "Enter select",
            "/ filter",
            "F follow",
            "wheel scroll",
            "click event",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.filter().is_some()
    }
}
