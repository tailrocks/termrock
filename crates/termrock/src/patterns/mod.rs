// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composition recipes (shadcn-style blocks) built from TermRock widgets.
//!
//! Patterns are product-neutral layouts: consumers supply wording, domain
//! data, and effects. TermRock owns geometry and chrome roles.
//!
//! **Canonical shell:** [`layout_app_shell`] / [`AppShellConfig`]. Specialized
//! helpers (`layout_agent_shell`, `layout_studio_shell`, …) are thin recipe
//! wrappers over AppShell slots.

mod agent_shell;
mod agent_workbench;
mod app_shell;
mod database_workbench;
mod file_manager;
mod git_workbench;
mod observability_dashboard;
mod ops_dashboard;
mod project_launcher;
mod resource_browser;
mod settings_screen;
mod setup_wizard;
mod studio_shell;

pub use agent_shell::{AgentShellLayout, AgentShellSlots, layout_agent_shell};
pub use agent_workbench::{
    AgentWorkbenchState, WorkbenchDensity, WorkbenchKeyOutcome, WorkbenchModals, WorkbenchPane,
    WorkbenchSurfaces, agent_workbench_layout, agent_workbench_layout_density, default_modes,
    dialog_modal_rect, diff_modal_rect, empty_task_row, example_workbench_activities,
    example_workbench_tasks, permission_modal_rect, register_workbench_scene,
    render_agent_workbench, sync_workbench_scene,
};
pub use database_workbench::{
    database_workbench_layout, database_workbench_layout_density, example_db_commands,
    example_db_history, example_disconnected_connections, example_inspect_fields,
    example_query_tabs, example_result_columns, example_result_row_refs, example_result_rows,
    example_schema_entries, example_workbench_connections, large_result_row_data,
    render_database_workbench, DatabaseConnGate, DatabaseQueryTab, DatabaseRunBlockReason,
    DatabaseTxStatus, DatabaseWorkbenchDensity, DatabaseWorkbenchOutcome, DatabaseWorkbenchPane,
    DatabaseWorkbenchState, DatabaseWorkbenchSurfaces,
};
pub use database_workbench::bench as database_workbench_bench;
pub use git_workbench::{
    example_clean_files, example_conflict_diagnostics, example_conflict_files,
    example_empty_files, example_git_branches, example_git_commits, example_git_diff_files,
    example_git_diff_lines, example_git_files, example_git_help_entries, example_git_hunks,
    example_git_terminal_lines, example_git_terminal_meta, git_workbench_layout,
    git_workbench_layout_density, large_git_diff, render_git_workbench, GitBranch,
    GitDestructiveKind, GitRepoStatus, GitWorkbenchDensity, GitWorkbenchOutcome,
    GitWorkbenchPane, GitWorkbenchState, GitWorkbenchSurfaces,
};
pub use git_workbench::bench as git_workbench_bench;
pub use observability_dashboard::{
    burst_observability_logs, example_log_inspect_fields, example_observability_alerts,
    example_observability_events, example_observability_logs, example_observability_tiles,
    observability_dashboard_layout, observability_dashboard_layout_density,
    render_observability_dashboard, seed_failure_state, ObservabilityDashboardOutcome,
    ObservabilityDashboardState, ObservabilityDashboardSurfaces, ObservabilityDensity,
    ObservabilityLiveState, ObservabilityPane, OBSERVABILITY_SEARCH_HEIGHT,
};
pub use observability_dashboard::bench as observability_dashboard_bench;
pub use file_manager::{
    burst_file_entries, default_quick_open_providers, dialog_rect, example_empty_ops,
    example_file_entries, example_file_ops, example_file_preview, example_quick_open_from_entries,
    file_manager_layout, file_manager_layout_density, quick_open_rect, render_file_manager,
    seed_conflict_state, seed_delete_confirm, FileClipboardMode, FileConflictResolution,
    FileManagerDensity, FileManagerDialog, FileManagerOutcome, FileManagerPane, FileManagerState,
    FileManagerSurfaces, FileOpItem, FileOpKind, FileOpStatus, FILE_MANAGER_BREADCRUMBS_HEIGHT,
    FILE_MANAGER_SEARCH_HEIGHT,
};
pub use file_manager::bench as file_manager_bench;
pub use project_launcher::{
    burst_project_entries, default_project_quick_open_providers, example_project_preview,
    example_project_quick_open, example_projects, filter_project_entries, project_launcher_layout,
    project_launcher_layout_density, project_list_rows, project_quick_open_rect,
    render_project_launcher, seed_error_state, seed_onboarding_state, seed_stale_state,
    ProjectEntry, ProjectGroup, ProjectLauncherDensity, ProjectLauncherMode,
    ProjectLauncherOutcome, ProjectLauncherPane, ProjectLauncherState, ProjectLauncherSurfaces,
    ProjectLocation, ProjectPathStatus, PROJECT_LAUNCHER_SEARCH_HEIGHT,
};
pub use project_launcher::bench as project_launcher_bench;
pub use app_shell::{
    app_shell_viewport, layout_app_shell, AppShellConfig, AppShellLifecycle, AppShellRecipe,
    AppShellSlots, AppShellZone,
};
pub use ops_dashboard::{OpsDashboardLayout, OpsDashboardSlots, layout_ops_dashboard};
pub use resource_browser::{
    ResourceBrowserLayout, ResourceBrowserSlots, layout_resource_browser, wire_resource_preview,
};
pub use settings_screen::{
    filter_settings_fieldsets, filter_settings_nav, layout_settings_screen,
    example_settings_appearance_fields, example_settings_categories, example_settings_help_entries,
    example_settings_keys_fields, example_settings_profile_fields, render_settings_screen,
    settings_query_matches, SettingsBodyMode, SettingsDensity, SettingsRegion,
    SettingsScreenOutcome, SettingsScreenSlots, SettingsScreenState, SettingsScreenSurfaces,
    SettingsShellOutcome, SettingsShellState,
};
pub use setup_wizard::{
    example_capability_lines, example_onboarding_setup_steps, example_setup_account_fields,
    example_setup_choices_fields, example_setup_connection_fields, example_setup_steps,
    example_setup_summary_lines, layout_setup_wizard, render_setup_wizard,
    setup_steps_to_wizard_steps, CapabilityLine, SetupStep, SetupStepKind, SetupWizardMode,
    SetupWizardOutcome, SetupWizardSlots, SetupWizardState, SetupWizardSurfaces,
};
pub use studio_shell::{StudioShellLayout, StudioShellSlots, layout_studio_shell};
