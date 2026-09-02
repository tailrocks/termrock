// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent demos for richer public widgets.
use std::fmt::Debug;

use ratatui::{Frame, layout::Rect, widgets::StatefulWidget};
use termrock::{
    input::{Event, KeyEvent, MouseEvent},
    style::{DesignSystem, RolePalette},
    widgets::{
        CivilDate, ColumnModel, Combobox, ComboboxOutcome, ComboboxState, CompletionCandidate,
        CompletionMenu, CompletionMenuOutcome, CompletionMenuSize, CompletionMenuState, DataColumn,
        DataColumnWidth, DataTable, DataTableOutcome, DataTableState, DataTableToolbar,
        DateTimePicker, DateTimePickerKind, DateTimePickerOutcome, DateTimePickerState, FileEntry,
        FilePicker, FilePickerMode, FilePickerOutcome, FilePickerState, FilePreview, MenuBar,
        MenuBarMenu, MenuBarOutcome, MenuBarState, NotificationCenter, NotificationCenterOutcome,
        NotificationCenterState, NotificationRecipe, PathFsStatus, PathInput, PathInputOutcome,
        PathInputState, PathStyle, QuickOpen, QuickOpenItem, QuickOpenOutcome, QuickOpenProvider,
        QuickOpenState, SearchInput, SearchInputOutcome, SearchInputState, SearchStatus,
        TreeNavNode, TreeNavigation, TreeNavigationOutcome, TreeNavigationState, example_app_menus,
        example_notifications, example_project_tree, example_quick_open_files,
        example_quick_open_providers, filter_quick_open_items,
    },
};

use super::StoryInteraction;

pub(super) fn record<T: Debug>(slot: &mut Option<String>, component: &str, outcome: T) -> bool {
    let raw = format!("{outcome:?}");
    if raw == "Ignored" {
        return false;
    }
    *slot = Some(format!("{component}: {raw}"));
    true
}

fn candidates() -> Vec<CompletionCandidate<'static, &'static str>> {
    vec![
        CompletionCandidate::new("rs", "Rust")
            .kind("language")
            .documentation("Memory-safe systems language."),
        CompletionCandidate::new("go", "Go")
            .kind("language")
            .documentation("Concurrent service language."),
        CompletionCandidate::new("ts", "TypeScript")
            .kind("language")
            .documentation("Typed JavaScript."),
    ]
}

pub(crate) struct SearchInputInteractor {
    state: SearchInputState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl SearchInputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SearchInputState::new().with_query("table");
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: SearchInputOutcome) -> bool {
        record(&mut self.outcome, "Search", outcome)
    }
}

impl StoryInteraction for SearchInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = SearchInput::new(&system)
            .placeholder("Search…")
            .status(SearchStatus::Results { count: 12 })
            .paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &[]);
        self.apply(outcome)
    }

    fn handle_event(&mut self, event: Event, area: Rect) -> bool {
        match event {
            Event::Paste(text) => {
                let outcome = self.state.insert_str(&text);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, area),
            _ => false,
        }
    }

    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type or paste query",
            "←→ move caret",
            "Esc clear",
            "click clear",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct PathInputInteractor {
    state: PathInputState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl PathInputInteractor {
    pub(crate) fn new() -> Self {
        let mut state = PathInputState::new()
            .with_style(PathStyle::Unix)
            .with_path("/usr/local/bin");
        state.set_focused(true);
        state.set_fs_status(PathFsStatus::Directory);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: PathInputOutcome) -> bool {
        record(&mut self.outcome, "Path", outcome)
    }
}

impl StoryInteraction for PathInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let _ = PathInput::new(&system).label("Install dir").paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn handle_event(&mut self, event: Event, area: Rect) -> bool {
        match event {
            Event::Paste(text) => {
                let outcome = self.state.insert_str(&text);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, area),
            _ => false,
        }
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type or paste path",
            "Enter validate",
            "click Browse",
            "click clear",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct ComboboxInteractor {
    state: ComboboxState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl ComboboxInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ComboboxState::new()
            .with_creatable(false)
            .with_exact_required(true);
        state.set_focused(true);
        state.set_value(Some("rs"), Some("Rust".into()));
        state.set_draft("Rust");
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: ComboboxOutcome<&'static str>) -> bool {
        match &outcome {
            ComboboxOutcome::DraftChanged { generation, .. }
            | ComboboxOutcome::MenuOpened { generation } => {
                let values = candidates();
                let _ = self.state.apply_suggestions(*generation, &values);
            }
            _ => {}
        }
        record(&mut self.outcome, "Combobox", outcome)
    }
}

impl StoryInteraction for ComboboxInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let values = candidates();
        Combobox::new(&system).label("Language").paint_with_menu(
            area,
            frame.buffer_mut(),
            &mut self.state,
            &values,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let values = candidates();
        let outcome = self.state.handle_key(key, &values);
        self.apply(outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let values = candidates();
        let menu = self.state.place_menu(area, CompletionMenuSize::default());
        let outcome = self.state.handle_mouse(mouse, &values, menu);
        self.apply(outcome)
    }

    fn handle_event(&mut self, event: Event, area: Rect) -> bool {
        match event {
            Event::Paste(text) => {
                let outcome = self.state.insert_str(&text);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, area),
            _ => false,
        }
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type to filter",
            "↓ open suggestions",
            "↑↓ choose",
            "Enter commit",
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

pub(crate) struct CompletionMenuInteractor {
    state: CompletionMenuState<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl CompletionMenuInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: CompletionMenuState::new(Some("rs")),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: CompletionMenuOutcome<&'static str>) -> bool {
        if matches!(
            outcome,
            CompletionMenuOutcome::Committed(_) | CompletionMenuOutcome::CommitWithChar { .. }
        ) {
            self.state.set_open(false);
        }
        record(&mut self.outcome, "Completion", outcome)
    }
}

impl StoryInteraction for CompletionMenuInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let values = candidates();
        let anchor = Rect::new(area.x.saturating_add(4), area.y.saturating_add(2), 1, 1);
        CompletionMenu::new(&values, &system, area, anchor)
            .preferred_size(CompletionMenuSize {
                width: 44,
                height: 8,
            })
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let values = candidates();
        let outcome = self.state.handle_key(key, &values);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let values = candidates();
        let outcome = self.state.handle_mouse(mouse, &values);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec![
                "↑↓ select",
                "Enter/Tab commit",
                "Esc dismiss",
                "click candidate",
            ]
        } else {
            vec!["Reset to reopen"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DataTableInteractor {
    state: DataTableState<u64, &'static str>,
    columns: ColumnModel<&'static str>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DataTableInteractor {
    pub(crate) fn new() -> Self {
        let mut state = DataTableState::new();
        state.set_accepts_input(true);
        Self {
            state,
            columns: ColumnModel::new(vec![
                DataColumn::new("id", "ID", DataColumnWidth::Min(4)),
                DataColumn::new("name", "Name", DataColumnWidth::Min(8)),
                DataColumn::new("status", "Status", DataColumnWidth::Min(8)),
            ]),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: DataTableOutcome<u64, &'static str>) -> bool {
        record(&mut self.outcome, "Data table", outcome)
    }
}

impl StoryInteraction for DataTableInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let cells = [
            ["1", "alpha", "ready"],
            ["2", "beta", "running"],
            ["3", "gamma", "queued"],
        ];
        let rows = [
            (1, cells[0].as_slice()),
            (2, cells[1].as_slice()),
            (3, cells[2].as_slice()),
        ];
        let toolbar = DataTableToolbar {
            actions: &["Refresh", "Export"],
        };
        DataTable::new(&system, &self.columns, &rows)
            .toolbar(&toolbar)
            .focused(true)
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &[1, 2, 3], &self.columns);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let mut columns = self.columns.clone();
        let outcome = self.state.handle_mouse(mouse, &[1, 2, 3], &mut columns);
        self.columns = columns;
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓←→ move cell",
            "Enter activate row",
            "Space select",
            "click cell",
            "wheel scroll",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DateTimePickerInteractor {
    state: DateTimePickerState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl DateTimePickerInteractor {
    pub(crate) fn new() -> Self {
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
            .with_date(CivilDate::new(2026, 8, 15).expect("valid fixture date"))
            .with_min_date(CivilDate::new(2026, 8, 1).expect("valid fixture date"))
            .with_max_date(CivilDate::new(2026, 8, 31).expect("valid fixture date"))
            .with_timezone_label("UTC");
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).expect("valid fixture date"));
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: DateTimePickerOutcome) -> bool {
        record(&mut self.outcome, "Date picker", outcome)
    }
}

impl StoryInteraction for DateTimePickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        DateTimePicker::new(&system).label("Due date").paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Alt+↓ open",
            "Enter commit",
            "arrows choose date",
            "PageUp/Down month",
            "Esc close",
            "click day",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

fn seed_file_picker(state: &mut FilePickerState) {
    let entries = vec![
        FileEntry::directory("d1", "src", "/home/u/proj/src"),
        FileEntry::file("f1", "README.md", "/home/u/proj/README.md").size(512),
        FileEntry::file("f2", "Cargo.toml", "/home/u/proj/Cargo.toml").size(920),
    ];
    if let FilePickerOutcome::ListRequested { generation, .. } = state.request_list("/home/u/proj")
    {
        let _ = state.apply_listing(generation, "/home/u/proj", entries, None);
    }
    let _ = state.apply_preview(
        state.preview_generation(),
        FilePreview::text(
            "README.md",
            ["# TermRock".into(), "Interactive picker".into()],
        ),
    );
}

pub(crate) struct FilePickerInteractor {
    state: FilePickerState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl FilePickerInteractor {
    pub(crate) fn new() -> Self {
        let mut state = FilePickerState::new("/home/u/proj")
            .with_mode(FilePickerMode::OpenFile)
            .with_preview(true)
            .with_path_style(PathStyle::Unix);
        state.set_focused(true);
        seed_file_picker(&mut state);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: FilePickerOutcome) -> bool {
        record(&mut self.outcome, "File picker", outcome)
    }
}

impl StoryInteraction for FilePickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        FilePicker::new(&system).title("Open file").paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "Enter open/choose",
            "Tab change pane",
            "/ filter",
            "click item",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct QuickOpenInteractor {
    state: QuickOpenState<&'static str>,
    providers: Vec<QuickOpenProvider>,
    catalog: Vec<QuickOpenItem<&'static str>>,
    visible: Vec<QuickOpenItem<&'static str>>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl QuickOpenInteractor {
    pub(crate) fn new() -> Self {
        let providers = example_quick_open_providers();
        let catalog = example_quick_open_files();
        let visible = catalog.clone();
        let mut state = QuickOpenState::new();
        state.set_focused(true);
        let _ = state.apply_results(0, &visible, true, Some(visible.len() as u64));
        Self {
            state,
            providers,
            catalog,
            visible,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }

    fn apply(&mut self, outcome: QuickOpenOutcome<&'static str>) -> bool {
        let request = match &outcome {
            QuickOpenOutcome::SearchRequested { request }
            | QuickOpenOutcome::ProviderChanged { request, .. } => Some(request.clone()),
            _ => None,
        };
        let changed = record(&mut self.outcome, "Quick open", outcome);
        if let Some(request) = request {
            self.visible = filter_quick_open_items(&self.catalog, &request.filter);
            let _ = self.state.apply_results(
                request.generation,
                &self.visible,
                true,
                Some(self.visible.len() as u64),
            );
        }
        changed
    }
}

impl StoryInteraction for QuickOpenInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        QuickOpen::new(&self.providers, &self.visible, &system).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &self.providers, &self.visible);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self
            .state
            .handle_mouse(mouse, &self.providers, &self.visible);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type to filter",
            "↑↓ select",
            "Enter open",
            "Ctrl+N/P provider",
            "click result",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct MenuBarInteractor {
    state: MenuBarState,
    menus: Vec<MenuBarMenu<&'static str>>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl MenuBarInteractor {
    pub(crate) fn new() -> Self {
        let mut state = MenuBarState::new();
        state.set_focused(true);
        Self {
            state,
            menus: example_app_menus(),
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: MenuBarOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Menu bar", outcome)
    }
}

impl StoryInteraction for MenuBarInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        MenuBar::new(&self.menus, &system).paint_all(
            bar,
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &self.menus);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &self.menus);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "←→ choose menu",
            "Enter/↓ open",
            "↑↓ choose item",
            "Esc close layer",
            "click menu",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TreeNavigationInteractor {
    state: TreeNavigationState<&'static str>,
    nodes: Vec<TreeNavNode<&'static str>>,
    system: DesignSystem,
    outcome: Option<String>,
}

impl TreeNavigationInteractor {
    pub(crate) fn new() -> Self {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("main"));
        state.set_focused(true);
        state.reconcile_route(&nodes);
        state.focus_route(&nodes);
        Self {
            state,
            nodes,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: TreeNavigationOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Tree navigation", outcome)
    }
}

impl StoryInteraction for TreeNavigationInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        TreeNavigation::new(&self.nodes, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &self.nodes);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &self.nodes);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "←→ collapse/expand",
            "Enter activate",
            "/ filter",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct NotificationCenterInteractor {
    state: NotificationCenterState,
    system: DesignSystem,
    outcome: Option<String>,
}

impl NotificationCenterInteractor {
    pub(crate) fn new() -> Self {
        let mut state = NotificationCenterState::new();
        state.replace_items(example_notifications(1_700_000_000));
        state.set_recipe(NotificationRecipe::Drawer);
        let _ = state.open();
        state.set_focused(true);
        Self {
            state,
            system: crate::design::lookbook_system(RolePalette::default()),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: NotificationCenterOutcome) -> bool {
        record(&mut self.outcome, "Notifications", outcome)
    }
}

impl StoryInteraction for NotificationCenterInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = self.system.clone();
        NotificationCenter::new(&system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn set_system(&mut self, system: DesignSystem) {
        self.system = system;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "Enter open",
            "Space read/unread",
            "f filter",
            "Esc close",
            "click item",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}
