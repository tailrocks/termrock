// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent application-pattern demos built only from public pattern APIs.

use ratatui::{Frame, layout::Rect};
use termrock::{
    input::{KeyCode, KeyEvent, MouseEvent},
    patterns::{
        AgentWorkbenchState, AppShellConfig, AppShellZone, AuthEntryField, AuthEntryOutcome,
        AuthEntryState, AuthEntrySurfaces, ConnectionManager, ConnectionManagerOutcome,
        ConnectionManagerPhase, ConnectionManagerPresentation, ConnectionManagerState,
        DatabaseConnGate, DatabaseWorkbenchOutcome, DatabaseWorkbenchState,
        DatabaseWorkbenchSurfaces, FileManagerOutcome, FileManagerState, FileManagerSurfaces,
        GitRepoStatus, GitWorkbenchOutcome, GitWorkbenchState, GitWorkbenchSurfaces,
        HelpCenterOutcome, HelpCenterState, HelpCenterSurfaces, MetricAlert, MetricAlertSeverity,
        MetricsDashboard, MetricsDashboardOutcome, MetricsDashboardState,
        ObservabilityDashboardOutcome, ObservabilityDashboardState, ObservabilityDashboardSurfaces,
        ObservabilityLiveState, ProjectLauncherOutcome, ProjectLauncherState,
        ProjectLauncherSurfaces, SchemaBrowser, SchemaBrowserEntry, SchemaBrowserOutcome,
        SchemaBrowserState, SchemaConnStatus, SchemaNodeKind, SessionPicker, SettingsBodyMode,
        SettingsRegion, SettingsScreenOutcome, SettingsScreenState, SettingsScreenSurfaces,
        SetupStepKind, SetupWizardOutcome, SetupWizardState, SetupWizardSurfaces,
        WorkbenchKeyOutcome, WorkbenchSurfaces, command_entries_from_help, default_modes,
        example_auth_aside_lines, example_capability_lines, example_connections,
        example_db_commands, example_db_history, example_file_entries, example_file_ops,
        example_file_preview, example_git_branches, example_git_commits, example_git_diff_files,
        example_git_diff_lines, example_git_files, example_git_help_entries, example_git_hunks,
        example_git_terminal_lines, example_git_terminal_meta, example_help_center_entries,
        example_help_doctor_report, example_help_topics, example_inspect_fields,
        example_log_inspect_fields, example_observability_alerts, example_observability_events,
        example_observability_logs, example_observability_tiles, example_project_preview,
        example_project_quick_open, example_projects, example_quick_open_from_entries,
        example_result_columns, example_result_row_refs, example_result_rows,
        example_schema_entries, example_sessions, example_settings_appearance_fields,
        example_settings_categories, example_setup_steps, example_setup_summary_lines,
        example_workbench_activities, example_workbench_tasks, layout_app_shell,
        render_agent_workbench, render_auth_entry, render_database_workbench, render_file_manager,
        render_git_workbench, render_help_center, render_observability_dashboard,
        render_project_launcher, render_settings_screen, render_setup_wizard,
    },
    style::{DesignSystem, PanelChrome, RolePalette},
    widgets::{
        BUILTIN_THEME_PRESETS, Fieldset, ListRow, MetricTile, MetricTileHealth, Panel,
        PromptComposer, PromptComposerState, StatusBarState, StatusSlot, Transcript,
        TranscriptBlock, TranscriptKind, TranscriptState,
    },
};

use super::StoryInteraction;

pub(crate) fn application_interactor(id: &str) -> Option<Box<dyn StoryInteraction>> {
    match id {
        "auth-entry/basic" => Some(Box::new(AuthEntryDemo::new())),
        "connection-manager/full" => Some(Box::new(ConnectionManagerDemo::new())),
        "app-shell/workbench" => Some(Box::new(AppShellDemo::new())),
        "setup-wizard/welcome" => Some(Box::new(SetupWizardDemo::new())),
        "file-manager/basic" => Some(Box::new(FileManagerDemo::new())),
        "project-launcher/basic" => Some(Box::new(ProjectLauncherDemo::new())),
        "help-center/basic" => Some(Box::new(HelpCenterDemo::new())),
        "metrics-dashboard/basic" => Some(Box::new(MetricsDashboardDemo::new())),
        "schema-browser/basic" => Some(Box::new(SchemaBrowserDemo::new())),
        "settings-screen/basic" => Some(Box::new(SettingsScreenDemo::new())),
        "observability-dashboard/basic" => Some(Box::new(ObservabilityDashboardDemo::new())),
        "database-workbench/basic" => Some(Box::new(DatabaseWorkbenchDemo::new())),
        "git-workbench/basic" => Some(Box::new(GitWorkbenchDemo::new())),
        "agent-workbench/basic" => Some(Box::new(AgentWorkbenchDemo::new())),
        _ => None,
    }
}

struct AuthEntryDemo {
    state: AuthEntryState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl AuthEntryDemo {
    fn new() -> Self {
        Self {
            state: AuthEntryState::sign_up(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: AuthEntryOutcome) -> bool {
        if matches!(value, AuthEntryOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            AuthEntryOutcome::FieldChanged { field } => {
                format!("Edited {}", field.id())
            }
            AuthEntryOutcome::FocusMoved { field } => format!("Focused {}", field.id()),
            AuthEntryOutcome::TermsToggled { accepted } => {
                format!("Terms {}", if *accepted { "accepted" } else { "cleared" })
            }
            AuthEntryOutcome::ValidationFailed { errors } => {
                format!("Validation failed: {}", errors[0].message)
            }
            AuthEntryOutcome::Submitted { identity, .. } => {
                format!("Submitted locally for {identity}; no authentication performed")
            }
            AuthEntryOutcome::Cancelled => "Authentication form cancelled".into(),
            AuthEntryOutcome::ModeSwitched { mode } => format!("Mode switched to {mode:?}"),
            other => format!("Auth entry: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for AuthEntryDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let mut surfaces = AuthEntrySurfaces::english(&system, &mut self.state);
        surfaces.aside_lines = example_auth_aside_lines();
        render_auth_entry(frame.buffer_mut(), area, surfaces);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let value = self.state.handle_key(key);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type/paste field",
            "Tab next field",
            "Ctrl+Enter submit",
            "Ctrl+G switch mode",
            "Esc cancel",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        !matches!(self.state.focus(), AuthEntryField::Terms)
    }
}

struct ConnectionManagerDemo {
    state: ConnectionManagerState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ConnectionManagerDemo {
    fn new() -> Self {
        let mut state = ConnectionManagerState::new();
        state.set_connections(example_connections());
        state.set_presentation(ConnectionManagerPresentation::Full);
        state.set_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: ConnectionManagerOutcome) -> bool {
        if matches!(value, ConnectionManagerOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            ConnectionManagerOutcome::Selected { id } => format!("Selected connection {id}"),
            ConnectionManagerOutcome::ConnectRequested { id } => {
                format!("Connect requested for {id}; demo performed no network I/O")
            }
            ConnectionManagerOutcome::ConfirmOpened { id } => {
                format!("Delete confirmation opened for {id}")
            }
            ConnectionManagerOutcome::ConfirmCancelled => "Delete cancelled".into(),
            ConnectionManagerOutcome::DeleteRequested { id } => {
                format!("Delete requested for {id}; sample data unchanged")
            }
            ConnectionManagerOutcome::QueryChanged { query } => format!("Filter: {query}"),
            ConnectionManagerOutcome::FavoriteToggled { id, favorite } => {
                format!("{id} favorite: {favorite}")
            }
            other => format!("Connection manager: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for ConnectionManagerDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        frame.render_stateful_widget(&ConnectionManager::new(&system), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let value = self.state.handle_key(key);
        self.apply(value)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let value = self.state.handle_mouse(mouse);
        self.apply(value)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        match self.state.phase {
            ConnectionManagerPhase::ConfirmDelete => {
                vec!["←→ choose", "Enter resolve", "Esc cancel", "click action"]
            }
            ConnectionManagerPhase::Add | ConnectionManagerPhase::Edit => {
                vec!["type field", "Tab next field", "Enter save", "Esc cancel"]
            }
            ConnectionManagerPhase::TestBusy => vec!["Esc cancel test"],
            ConnectionManagerPhase::Browse if self.state.search_mode() => {
                vec![
                    "type/paste filter",
                    "↑↓ select",
                    "Enter connect",
                    "Esc clear",
                ]
            }
            ConnectionManagerPhase::Browse => vec![
                "↑↓/wheel select",
                "Enter connect",
                "/ filter",
                "D delete dialog",
                "N new",
            ],
            _ => vec!["Esc return"],
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        if !matches!(self.state.phase, ConnectionManagerPhase::Browse) || self.state.search_mode() {
            self.handle_key(key)
        } else {
            false
        }
    }

    fn captures_text_input(&self) -> bool {
        self.state.search_mode()
            || matches!(
                self.state.phase,
                ConnectionManagerPhase::Add | ConnectionManagerPhase::Edit
            )
    }
}

struct AppShellDemo {
    sidebar: bool,
    focus: usize,
    theme: RolePalette,
    outcome: Option<String>,
}

impl AppShellDemo {
    fn new() -> Self {
        Self {
            sidebar: true,
            focus: 0,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

impl StoryInteraction for AppShellDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let mut config = AppShellConfig::workbench();
        if !self.sidebar {
            config.sidebar_width = 0;
        }
        let slots = layout_app_shell(area, config);
        let zones = [
            (AppShellZone::Header, slots.header),
            (AppShellZone::Sidebar, slots.sidebar),
            (AppShellZone::Main, Some(slots.main)),
            (AppShellZone::Inspector, slots.inspector),
            (AppShellZone::Footer, slots.footer),
        ];
        for (zone, rect) in zones {
            let Some(rect) = rect.filter(|rect| !rect.is_empty()) else {
                continue;
            };
            let active = slots.focus_order.get(self.focus).copied() == Some(zone);
            let _ = Panel::new(&system)
                .title(zone.id())
                .emphasis(if active {
                    PanelChrome::Focused
                } else {
                    PanelChrome::Normal
                })
                .paint(rect, frame.buffer_mut(), None);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('s' | 'S') => {
                self.sidebar = !self.sidebar;
                self.outcome = Some(format!(
                    "Sidebar {}",
                    if self.sidebar {
                        "expanded"
                    } else {
                        "collapsed"
                    }
                ));
                true
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                self.focus = (self.focus + 1) % if self.sidebar { 5 } else { 4 };
                self.outcome = Some("Focus moved to next visible shell zone".into());
                true
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
                let count = if self.sidebar { 5 } else { 4 };
                self.focus = (self.focus + count - 1) % count;
                self.outcome = Some("Focus moved to previous visible shell zone".into());
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Tab/arrow traverse zones",
            "S toggle sidebar",
            "resize host",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

struct SetupWizardDemo {
    state: SetupWizardState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl SetupWizardDemo {
    fn new() -> Self {
        Self {
            state: SetupWizardState::from_steps(example_setup_steps()).with_title("First run"),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: SetupWizardOutcome) -> bool {
        if matches!(value, SetupWizardOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            SetupWizardOutcome::Finished => "Setup submitted locally; no settings persisted".into(),
            SetupWizardOutcome::CancelConfirmOpen => "Cancel confirmation opened".into(),
            SetupWizardOutcome::CancelConfirmDismissed => "Cancel confirmation dismissed".into(),
            SetupWizardOutcome::CancelConfirmed => "Setup cancelled".into(),
            other => format!("Setup wizard: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for SetupWizardDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let capabilities = example_capability_lines();
        let summary = example_setup_summary_lines();
        render_setup_wizard(
            frame.buffer_mut(),
            area,
            SetupWizardSurfaces {
                system: &system,
                state: &mut self.state,
                fieldsets: &[],
                capabilities: &capabilities,
                summary_lines: &summary,
                welcome_title: "TermRock setup",
                welcome_detail: "Configure once. Keyboard-first.",
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: Some(&system),
                permission: None,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let value = self.state.handle_key(key, &[], BUILTIN_THEME_PRESETS);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.cancel_confirm {
            return vec!["Enter/Y confirm cancel", "Esc/N keep setup"];
        }
        match self.state.current_kind() {
            SetupStepKind::Theme => vec!["↑↓ choose theme", "Enter next", "Esc cancel"],
            _ => vec!["Enter/→ next", "← back", "S skip optional", "Esc cancel"],
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.handle_key(key)
    }
}

struct FileManagerDemo {
    state: FileManagerState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl FileManagerDemo {
    fn new() -> Self {
        let mut state = FileManagerState::new();
        state.cwd = "/project".into();
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: FileManagerOutcome) -> bool {
        if matches!(value, FileManagerOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            FileManagerOutcome::OpenRequested { id } => {
                format!("Open requested for {id}; demo did not touch the filesystem")
            }
            FileManagerOutcome::FilterChanged { query } => format!("File filter: {query}"),
            FileManagerOutcome::Toggle { id } => format!("Toggled tree node {id}"),
            FileManagerOutcome::ConfirmDestructive { paths } => {
                format!("Confirmation opened for {} sample path(s)", paths.len())
            }
            FileManagerOutcome::ConfirmCancelled => "File action cancelled".into(),
            other => format!("File manager: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for FileManagerDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let entries = example_file_entries();
        let ops = example_file_ops();
        let (preview, _, _) = example_file_preview();
        let quick_open = example_quick_open_from_entries(&entries);
        render_file_manager(
            frame.buffer_mut(),
            area,
            FileManagerSurfaces {
                system: &system,
                state: &mut self.state,
                entries: &entries,
                ops: &ops,
                preview: Some(preview),
                quick_open_items: &quick_open,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let entries = example_file_entries();
        let ops = example_file_ops();
        let quick_open = example_quick_open_from_entries(&entries);
        let value = self.state.handle_key(key, &entries, &ops, &quick_open);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ navigate tree",
            "←→ collapse/expand",
            "Enter open",
            "/ filter",
            "Ctrl+P quick open",
            "Tab panes",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

struct ProjectLauncherDemo {
    state: ProjectLauncherState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ProjectLauncherDemo {
    fn new() -> Self {
        Self {
            state: ProjectLauncherState::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: ProjectLauncherOutcome) -> bool {
        if matches!(value, ProjectLauncherOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            ProjectLauncherOutcome::OpenRequested { id } => {
                format!("Open requested for {id}; demo launched nothing")
            }
            ProjectLauncherOutcome::FilterChanged { query } => format!("Project filter: {query}"),
            ProjectLauncherOutcome::SelectionChanged { id } => format!("Selected project {id}"),
            ProjectLauncherOutcome::QuickOpenOpened => "Project quick open opened".into(),
            ProjectLauncherOutcome::QuickOpenClosed => "Project quick open closed".into(),
            other => format!("Project launcher: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for ProjectLauncherDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let projects = example_projects();
        let sessions = example_sessions();
        let (preview, _, _) = example_project_preview();
        let quick_open = example_project_quick_open(&projects);
        render_project_launcher(
            frame.buffer_mut(),
            area,
            ProjectLauncherSurfaces {
                system: &system,
                state: &mut self.state,
                projects: &projects,
                sessions: &sessions,
                preview: Some(preview),
                quick_open_items: &quick_open,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let projects = example_projects();
        let sessions = example_sessions();
        let quick_open = example_project_quick_open(&projects);
        let value = self
            .state
            .handle_key(key, &projects, &sessions, &quick_open);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select project",
            "Enter open",
            "/ filter",
            "Ctrl+P quick open",
            "Tab panes",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

struct HelpCenterDemo {
    state: HelpCenterState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl HelpCenterDemo {
    fn new() -> Self {
        let mut state = HelpCenterState::new();
        state.selected_topic = Some("getting-started".into());
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: HelpCenterOutcome) -> bool {
        if matches!(value, HelpCenterOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            HelpCenterOutcome::TopicOpened { id } => format!("Opened help topic {id}"),
            HelpCenterOutcome::FilterChanged { query } => format!("Help filter: {query}"),
            HelpCenterOutcome::CommandRun { id } => {
                format!("Command {id} requested; demo ran no command")
            }
            HelpCenterOutcome::DoctorOpened => "Opened deterministic diagnostics".into(),
            other => format!("Help center: {other:?}"),
        });
        true
    }

    fn fixtures(
        system: &DesignSystem,
    ) -> (
        Vec<termrock::patterns::HelpTopic>,
        Vec<termrock::widgets::HelpEntry>,
        Vec<termrock::widgets::CommandEntry<String>>,
    ) {
        let topics = example_help_topics();
        let help = example_help_center_entries(system);
        let commands = command_entries_from_help(&help);
        (topics, help, commands)
    }
}

impl StoryInteraction for HelpCenterDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let (topics, help, commands) = Self::fixtures(&system);
        let doctor = example_help_doctor_report();
        let components = vec!["keyboard-help".into(), "command-palette".into()];
        render_help_center(
            frame.buffer_mut(),
            area,
            HelpCenterSurfaces {
                system: &system,
                state: &mut self.state,
                topics: &topics,
                help_entries: &help,
                commands: &commands,
                doctor: Some(&doctor),
                component_ids: &components,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let (topics, help, commands) = Self::fixtures(&system);
        let doctor = example_help_doctor_report();
        let components = vec!["keyboard-help".into(), "command-palette".into()];
        let value =
            self.state
                .handle_key(key, &topics, &help, &commands, Some(&doctor), &components);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "Enter open",
            "/ filter",
            "Tab panes",
            "D diagnostics",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

struct MetricsDashboardDemo {
    state: MetricsDashboardState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl MetricsDashboardDemo {
    fn new() -> Self {
        Self {
            state: MetricsDashboardState::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn fixtures() -> (Vec<MetricTile<'static>>, Vec<MetricAlert<'static>>) {
        static SAMPLES: &[f64] = &[1.0, 2.0, 3.0, 2.5, 4.0, 3.5, 5.0, 4.2];
        let tiles = vec![
            MetricTile::new("cpu", "CPU", "42%")
                .samples(SAMPLES)
                .health(MetricTileHealth::Ok),
            MetricTile::new("mem", "Memory", "71%")
                .gauge(71.0)
                .health(MetricTileHealth::Warning),
            MetricTile::new("rps", "RPS", "1.2k").samples(SAMPLES),
            MetricTile::new("lat", "p99", "48ms").samples(SAMPLES),
        ];
        let alerts = vec![MetricAlert::new(
            "memory",
            MetricAlertSeverity::Warning,
            "memory > 70%",
        )];
        (tiles, alerts)
    }

    fn apply(&mut self, value: MetricsDashboardOutcome) -> bool {
        if matches!(value, MetricsDashboardOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            MetricsDashboardOutcome::DrillDownRequested { id } => {
                format!("Opened metric detail for {id}")
            }
            MetricsDashboardOutcome::RefreshRequested => {
                "Refresh requested; deterministic samples retained".into()
            }
            MetricsDashboardOutcome::TimeRangeChanged(range) => {
                format!("Time range: {}", range.label())
            }
            other => format!("Metrics dashboard: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for MetricsDashboardDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let (tiles, alerts) = Self::fixtures();
        MetricsDashboard::new(&tiles, &alerts, &system)
            .title("ops")
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let (tiles, alerts) = Self::fixtures();
        let value = self.state.handle_key(key, &tiles, &alerts);
        self.apply(value)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let (tiles, alerts) = Self::fixtures();
        let value = self.state.handle_mouse(mouse, &tiles, &alerts);
        self.apply(value)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "arrows select metric",
            "Enter inspect",
            "Tab focus region",
            "Ctrl+T range",
            "Ctrl+R refresh",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

struct SchemaBrowserDemo {
    state: SchemaBrowserState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl SchemaBrowserDemo {
    fn new() -> Self {
        let mut state = SchemaBrowserState::with_selected(Some("users"));
        state.sync_expanded_from_entries(&Self::entries());
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn entries() -> Vec<SchemaBrowserEntry<'static, &'static str>> {
        vec![
            SchemaBrowserEntry::connection("conn", "prod", "prod")
                .expanded()
                .conn_status(SchemaConnStatus::Connected),
            SchemaBrowserEntry::database("db", "app", "prod/app", 1)
                .parent("conn")
                .expanded(),
            SchemaBrowserEntry::schema("schema", "public", "prod/app/public", 2)
                .parent("db")
                .expanded(),
            SchemaBrowserEntry::table("users", "users", "prod/app/public/users", 3)
                .parent("schema")
                .expanded(),
            SchemaBrowserEntry::column("users.id", "id", "prod/app/public/users.id", 4)
                .parent("users")
                .type_label("int8")
                .key_badge("PK"),
            SchemaBrowserEntry::column("users.email", "email", "prod/app/public/users.email", 4)
                .parent("users")
                .type_label("text"),
            SchemaBrowserEntry::table("orders", "orders", "prod/app/public/orders", 3)
                .parent("schema")
                .lazy(),
            SchemaBrowserEntry::new(
                "idx_email",
                "idx_email",
                "prod/app/public/users/idx_email",
                SchemaNodeKind::Index,
                4,
            )
            .parent("users"),
        ]
    }

    fn apply(&mut self, value: SchemaBrowserOutcome<&'static str>) -> bool {
        if matches!(value, SchemaBrowserOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            SchemaBrowserOutcome::SelectionChanged(id) => {
                format!("Selected schema object {id}")
            }
            SchemaBrowserOutcome::OpenRequested(id) => format!("Activated schema object {id}"),
            SchemaBrowserOutcome::Toggle(id) => format!("Toggled schema branch {id}"),
            SchemaBrowserOutcome::FilterChanged(query) => format!("Schema filter: {query}"),
            other => format!("Schema browser: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for SchemaBrowserDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let entries = Self::entries();
        SchemaBrowser::new(&entries, &system)
            .title("catalog")
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let entries = Self::entries();
        let value = self.state.handle_key(&entries, key);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.filter.is_some() {
            vec![
                "type/paste filter",
                "↑↓ select",
                "Enter activate",
                "Esc clear",
            ]
        } else {
            vec![
                "↑↓ select",
                "←→ collapse/expand",
                "Enter activate",
                "/ filter",
            ]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        self.state.filter.is_some()
    }
}

struct SettingsScreenDemo {
    state: SettingsScreenState<&'static str>,
    status: StatusBarState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl SettingsScreenDemo {
    fn new() -> Self {
        let mut state = SettingsScreenState::new();
        state.region = SettingsRegion::Body;
        state.body_mode = SettingsBodyMode::Form;
        let _ = state.select_section("appearance");
        Self {
            state,
            status: StatusBarState::default(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: SettingsScreenOutcome<&'static str, &'static str>) -> bool {
        if matches!(value, SettingsScreenOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            SettingsScreenOutcome::SaveRequested => {
                "Save requested; demo persisted no settings".into()
            }
            SettingsScreenOutcome::SearchChanged => "Settings search updated".into(),
            SettingsScreenOutcome::SectionSelected(id) => format!("Selected settings {id}"),
            SettingsScreenOutcome::DrawerToggled { open } => {
                format!(
                    "Settings drawer {}",
                    if *open { "opened" } else { "closed" }
                )
            }
            SettingsScreenOutcome::HelpOpened => "Settings help opened".into(),
            SettingsScreenOutcome::HelpClosed => "Settings help closed".into(),
            other => format!("Settings: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for SettingsScreenDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let nav = example_settings_categories();
        let fields = example_settings_appearance_fields();
        let fieldsets = [Fieldset::new("Appearance", &fields)];
        render_settings_screen(
            frame.buffer_mut(),
            area,
            SettingsScreenSurfaces {
                system: &system,
                state: &mut self.state,
                nav: &nav,
                fieldsets: &fieldsets,
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: Some(&system),
                status_slots: &[],
                status_state: &mut self.status,
                section_title: "Appearance",
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let nav = example_settings_categories();
        let fields = example_settings_appearance_fields();
        let fieldsets = [Fieldset::new("Appearance", &fields)];
        let value = self
            .state
            .handle_key(key, &nav, &fieldsets, BUILTIN_THEME_PRESETS);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.help_open {
            vec!["Esc/? close help"]
        } else {
            vec![
                "Tab cycle regions",
                "↑↓ select/edit",
                "/ search",
                "Ctrl+B drawer",
                "Ctrl+S save",
                "? help",
            ]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.state.help_open && self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        self.state.region == SettingsRegion::Search
    }
}

struct ObservabilityDashboardDemo {
    state: ObservabilityDashboardState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ObservabilityDashboardDemo {
    fn new() -> Self {
        let mut state = ObservabilityDashboardState::new();
        state.live = ObservabilityLiveState::Live;
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: ObservabilityDashboardOutcome) -> bool {
        if matches!(value, ObservabilityDashboardOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            ObservabilityDashboardOutcome::QueryChanged { query } => {
                format!("Observability filter: {query}")
            }
            ObservabilityDashboardOutcome::ReconnectRequested => {
                "Reconnect requested; demo opened no connection".into()
            }
            ObservabilityDashboardOutcome::LiveToggled { live } => {
                format!("Live stream {}", if *live { "resumed" } else { "paused" })
            }
            other => format!("Observability: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for ObservabilityDashboardDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let logs = example_observability_logs();
        let events = example_observability_events();
        let tiles = example_observability_tiles();
        let alerts = example_observability_alerts();
        let inspect = example_log_inspect_fields();
        render_observability_dashboard(
            frame.buffer_mut(),
            area,
            ObservabilityDashboardSurfaces {
                system: &system,
                state: &mut self.state,
                logs: &logs,
                events: &events,
                tiles: &tiles,
                alerts: &alerts,
                inspect_fields: &inspect,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let logs = example_observability_logs();
        let events = example_observability_events();
        let tiles = example_observability_tiles();
        let alerts = example_observability_alerts();
        let inspect = example_log_inspect_fields();
        let value = self
            .state
            .handle_key(key, &logs, &events, &tiles, &alerts, &inspect);
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Tab cycle panes",
            "↑↓ navigate",
            "Space pause/resume",
            "Ctrl+R reconnect",
            "/ search",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn captures_text_input(&self) -> bool {
        self.state.focus == "search"
    }
}

struct DatabaseWorkbenchDemo {
    state: DatabaseWorkbenchState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl DatabaseWorkbenchDemo {
    fn new() -> Self {
        let mut state = DatabaseWorkbenchState::new();
        state.conn_gate = DatabaseConnGate::Connected;
        state.finish_run_success(3, 12);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: DatabaseWorkbenchOutcome) -> bool {
        if matches!(value, DatabaseWorkbenchOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            DatabaseWorkbenchOutcome::RunRequested { tab_id, .. } => {
                format!("Run requested for {tab_id}; demo executed no query")
            }
            DatabaseWorkbenchOutcome::RunBlocked { reason, .. } => {
                format!("Query blocked: {reason:?}")
            }
            DatabaseWorkbenchOutcome::OpenPalette => "Database command palette opened".into(),
            DatabaseWorkbenchOutcome::OpenHistory => "Query history opened".into(),
            DatabaseWorkbenchOutcome::ExportRequested { format, .. } => {
                format!("{format:?} export requested; demo wrote no file")
            }
            other => format!("Database workbench: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for DatabaseWorkbenchDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let schema = example_schema_entries();
        let columns = example_result_columns();
        let data = example_result_rows();
        let mut cell_store = Vec::new();
        let rows = example_result_row_refs(&data, &mut cell_store);
        let inspect = example_inspect_fields();
        let history = example_db_history();
        let commands = example_db_commands();
        render_database_workbench(
            frame.buffer_mut(),
            area,
            DatabaseWorkbenchSurfaces {
                system: &system,
                state: &mut self.state,
                schema_entries: &schema,
                result_columns: &columns,
                result_rows: &rows,
                inspect_fields: &inspect,
                history: &history,
                commands: &commands,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let schema = example_schema_entries();
        let history = example_db_history();
        let commands = example_db_commands();
        let inspect = example_inspect_fields();
        let value = self.state.handle_key(
            key,
            &schema,
            &history,
            &commands,
            example_result_rows().len(),
            &inspect,
        );
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Tab cycle panes",
            "type SQL",
            "Ctrl+Enter run",
            "Ctrl+P palette",
            "Ctrl+H history",
            "Esc close layer",
        ]
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        self.state.focus == "query"
    }
}

struct GitWorkbenchDemo {
    state: GitWorkbenchState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl GitWorkbenchDemo {
    fn new() -> Self {
        let mut state = GitWorkbenchState::new();
        state.repo_status = GitRepoStatus::Dirty;
        state.branches = example_git_branches();
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn apply(&mut self, value: GitWorkbenchOutcome) -> bool {
        if matches!(value, GitWorkbenchOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            GitWorkbenchOutcome::RefreshRequested => {
                "Repository refresh requested; demo ran no git command".into()
            }
            GitWorkbenchOutcome::StageRequested { .. } => {
                "Stage requested; sample repository unchanged".into()
            }
            GitWorkbenchOutcome::CommitRequested { message } => {
                format!("Commit requested with message “{message}”; demo wrote nothing")
            }
            GitWorkbenchOutcome::OpenHelp => "Git workbench help opened".into(),
            GitWorkbenchOutcome::HelpClosed => "Git workbench help closed".into(),
            GitWorkbenchOutcome::ConfirmOpened(_) => "Safe confirmation opened".into(),
            GitWorkbenchOutcome::ConfirmCancelled => "Git action cancelled".into(),
            other => format!("Git workbench: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for GitWorkbenchDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let files = example_git_files();
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let diff_files = example_git_diff_files();
        let commits = example_git_commits();
        let diagnostics = Vec::new();
        let terminal_meta = example_git_terminal_meta();
        let terminal_lines = example_git_terminal_lines();
        let help = example_git_help_entries(&system);
        render_git_workbench(
            frame.buffer_mut(),
            area,
            GitWorkbenchSurfaces {
                system: &system,
                state: &mut self.state,
                files: &files,
                diff_lines: &lines,
                hunks: &hunks,
                diff_files: &diff_files,
                commits: &commits,
                diagnostics: &diagnostics,
                terminal_meta: &terminal_meta,
                terminal_lines: &terminal_lines,
                help_entries: &help,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let files = example_git_files();
        let lines = example_git_diff_lines();
        let hunks = example_git_hunks();
        let diff_files = example_git_diff_files();
        let diagnostics = Vec::new();
        let terminal_meta = example_git_terminal_meta();
        let terminal_lines = example_git_terminal_lines();
        let help = example_git_help_entries(&system);
        let value = self.state.handle_key(
            key,
            &files,
            &hunks,
            &lines,
            &diff_files,
            &help,
            &diagnostics,
            &terminal_lines,
            &terminal_meta,
        );
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.help_open {
            vec!["Esc/? close help"]
        } else if self.state.confirm.is_some() {
            vec!["←→ choose", "Enter resolve", "Esc cancel"]
        } else {
            vec![
                "Tab cycle panes",
                "↑↓ navigate",
                "T stage/unstage",
                "? help overlay",
                "Ctrl+F full diff",
            ]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        (self.state.help_open || self.state.confirm.is_some()) && self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        self.state.focus == "commit"
    }
}

struct AgentWorkbenchDemo {
    state: AgentWorkbenchState,
    prompt: PromptComposerState,
    transcript: TranscriptState<&'static str>,
    status: StatusBarState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl AgentWorkbenchDemo {
    fn new() -> Self {
        let mut state = AgentWorkbenchState::new();
        state.session.set_sessions(example_sessions());
        let mut prompt = PromptComposerState::new();
        prompt.set_text("draft survives overlays");
        Self {
            state,
            prompt,
            transcript: TranscriptState::new(),
            status: StatusBarState::default(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn blocks() -> [TranscriptBlock<'static, &'static str>; 3] {
        static USER: &[&str] = &["Plan the cutover", "Compose public TermRock only"];
        static TOOL: &[&str] = &["cargo test --lib agent_workbench", "ok"];
        static ASSISTANT: &[&str] = &["Tool finished; ready for review"];
        [
            TranscriptBlock::new("user", TranscriptKind::User, USER),
            TranscriptBlock::new("tool", TranscriptKind::Tool, TOOL),
            TranscriptBlock::new("assistant", TranscriptKind::Assistant, ASSISTANT),
        ]
    }

    fn apply(&mut self, value: WorkbenchKeyOutcome) -> bool {
        if matches!(value, WorkbenchKeyOutcome::Ignored) {
            return false;
        }
        self.outcome = Some(match &value {
            WorkbenchKeyOutcome::FocusChanged(id) => format!("Focused workbench pane {id}"),
            WorkbenchKeyOutcome::Prompt(_) => {
                format!("Prompt draft: {}", self.prompt.text())
            }
            WorkbenchKeyOutcome::Session => {
                if self.state.session_open() {
                    "Session overlay updated".into()
                } else {
                    "Session overlay closed; prompt draft preserved".into()
                }
            }
            other => format!("Agent workbench: {other:?}"),
        });
        true
    }
}

impl StoryInteraction for AgentWorkbenchDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let blocks = Self::blocks();
        let transcript = Transcript::new(&blocks, &system);
        let prompt = PromptComposer::new(&system);
        let modes = default_modes("build");
        let status = [StatusSlot::connection("status", "ready")];
        let tasks = example_workbench_tasks();
        let activities = example_workbench_activities();
        let legacy: [ListRow<'_, &'static str>; 0] = [];
        let session = SessionPicker::new(&system);
        let session_open = self.state.session_open();
        render_agent_workbench(
            frame.buffer_mut(),
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut self.state,
                task_models: Some(&tasks),
                tasks: &legacy,
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut self.transcript,
                activities: Some(&activities),
                prompt: &prompt,
                prompt_state: &mut self.prompt,
                status_slots: &status,
                status_state: &mut self.status,
                permission: None,
                question: None,
                plan: None,
                diff: None,
                session: session_open.then_some(&session),
                working: None,
            },
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Char('o' | 'O'))
            && !self.state.any_overlay_open()
        {
            self.state.set_session_open(true);
            self.outcome = Some("Session overlay opened; prompt draft preserved".into());
            return true;
        }
        let blocks = Self::blocks();
        let tasks = example_workbench_tasks();
        let activities = example_workbench_activities();
        let legacy: [ListRow<'_, &'static str>; 0] = [];
        let value = self.state.handle_key(
            key,
            &mut self.prompt,
            &mut self.transcript,
            &blocks,
            None,
            Some(&tasks),
            Some(&legacy),
            Some(&activities),
            None,
        );
        self.apply(value)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        if self.state.session_open() {
            vec!["↑↓ select session", "Enter activate", "Esc close overlay"]
        } else {
            vec![
                "Tab cycle panes",
                "type in prompt",
                "O open session overlay",
                "Esc peel top layer",
            ]
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.state.any_overlay_open() && self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        self.state.focused_pane() == Some("prompt")
    }
}
