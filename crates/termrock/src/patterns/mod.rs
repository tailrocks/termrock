// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Example compositions** — product recipes built from generic
//! [`crate::widgets`] building blocks.
//!
//! This module is **not** the default widgets catalog. It holds connection
//! managers, auth/login entry, workbenches, dashboards, session pickers, and
//! similar multi-widget demos so consumers can copy the assembly pattern.
//! Import building blocks from `termrock::widgets`; import these recipes from
//! `termrock::patterns` only when you want a ready-made example composite.
//!
//! **Canonical shell:** [`layout_app_shell`] / [`AppShellConfig`]. Specialized
//! helpers (`layout_agent_shell`, `layout_studio_shell`, …) are thin recipe
//! wrappers over AppShell slots.

mod agent_shell;
mod agent_workbench;
mod app_dashboard;
mod app_shell;
mod auth_entry;
mod database_workbench;
mod file_manager;
mod git_workbench;
mod observability_dashboard;
mod ops_dashboard;
mod error_recovery;
mod help_center;
mod project_launcher;
mod resource_browser;
mod settings_screen;
mod setup_wizard;
mod studio_shell;

// Product example composites (moved out of widgets)
mod activity_shelf;
mod agent_status_header;
mod approval_queue;
mod background_task_panel;
mod connection_manager;
mod integration_status;
mod metrics_dashboard;
mod plan_review;
mod process_table;
mod prompt_queue;
mod query_editor;
mod result_grid;
mod schema_browser;
mod session_picker;
mod subagent_card;
mod task_rail;
mod terminal_run_card;
mod working_state_card;

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
pub use help_center::{
    burst_help_topics, command_entries_from_help, command_list_rows, component_inspect_rows,
    diagnostics_rows, doctor_finding_rows, example_help_center_commands,
    example_help_center_entries, example_help_doctor_report, example_help_topics,
    filter_help_topics, help_center_layout, help_center_layout_density, help_topic_rows,
    render_help_center, seed_compact_mode, seed_diagnostics_state, HelpCenterDensity,
    HelpCenterMode, HelpCenterOutcome, HelpCenterPane, HelpCenterState, HelpCenterSurfaces,
    HelpTopic, HelpTopicGroup, HELP_CENTER_SEARCH_HEIGHT,
};
pub use help_center::bench as help_center_bench;
pub use error_recovery::{
    build_redacted_crash_report, burst_crash_snapshot, error_recovery_layout,
    error_recovery_layout_density, example_crash_snapshot_with_secrets,
    example_recovery_snapshot, example_terminal_restore_failed_snapshot, recovery_action_rows,
    redact_crash_report_text, render_error_recovery, seed_inline_fallback, seed_partial_init,
    seed_terminal_restore_failed, CrashReportSnapshot, ErrorRecoveryDensity, ErrorRecoveryMode,
    ErrorRecoveryOutcome, ErrorRecoveryPane, ErrorRecoveryState, ErrorRecoverySurfaces,
    FailureClass, RecoveryActionId,
};
pub use error_recovery::bench as error_recovery_bench;
pub use app_dashboard::{
    example_dashboard_nav, layout_app_dashboard, render_app_dashboard, AppDashboardLayout,
    AppDashboardOutcome, AppDashboardPane, AppDashboardSlots, AppDashboardState,
    AppDashboardSurfaces,
};
pub use app_shell::{
    app_shell_viewport, layout_app_shell, AppShellConfig, AppShellLifecycle, AppShellRecipe,
    AppShellSlots, AppShellZone,
};
pub use auth_entry::{
    auth_entry_form_width, example_auth_aside_lines, render_auth_entry, AuthEntryField,
    AuthEntryMode, AuthEntryOutcome, AuthEntryState, AuthEntrySurfaces, AuthFieldError,
};
pub use ops_dashboard::{
    layout_ops_dashboard, OpsDashboardLayout, OpsDashboardOutcome, OpsDashboardSlots,
    OpsDashboardState, OpsRegion,
};
pub use resource_browser::{
    layout_resource_browser, wire_resource_preview, ResourceBrowserLayout, ResourceBrowserOutcome,
    ResourceBrowserSlots, ResourceBrowserState,
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

// ── Example composites (from widgets) ─────────────────────────
pub use terminal_run_card::{
    TERMINAL_RUN_COMPACT_BODY_LINES, TERMINAL_RUN_ENV_CAP, TERMINAL_RUN_FULLSCREEN_OVERLAY_ID,
    TerminalCommandPhase, TerminalRun, TerminalRunCard, TerminalRunCardOutcome,
    TerminalRunCardState, TerminalRunEnv, TerminalRunPresentation, example_terminal_run_lines,
    example_terminal_runs, project_terminal_run_lines, terminal_run_env_entries,
    terminal_run_to_meta, terminal_run_to_tool_call,
};
pub use terminal_run_card::bench as terminal_run_card_bench;
pub use activity_shelf::{
    ACTIVITY_SHELF_CHIP_CAP, ACTIVITY_SHELF_CHIP_MIN_COLS, ACTIVITY_SHELF_NARROW_WIDTH,
    ACTIVITY_SHELF_TINY_WIDTH, ActivityCounts, ActivityItem, ActivityKind, ActivityShelf,
    ActivityShelfOrientation, ActivityShelfOutcome, ActivityShelfPlan, ActivityShelfPresentation,
    ActivityShelfState, ActivityStatusProjection, activities_to_notifications, activity_badge_label,
    activity_counts, activity_status_slot, activity_status_summary, activity_to_notification,
    example_activities, plan_activity_shelf, project_activities_for_status_bar, sort_activity_items,
};
pub use activity_shelf::bench as activity_shelf_bench;
pub use process_table::{
    ProcessKey, ProcessRow, ProcessSignal, ProcessSignalConfirm, ProcessSortKey, ProcessStatus,
    ProcessTable, ProcessTableOutcome, ProcessTableState, ProcessViewMode, cmp_process,
    filter_processes, filter_tree_preserve, format_cpu_pct, format_elapsed_ms, format_mem_bytes,
    process_column_model, sort_processes_flat,
};
pub use process_table::bench as process_table_bench;
pub use query_editor::{
    QueryEditor, QueryEditorMode, QueryEditorOutcome, QueryEditorSlots, QueryEditorState,
    QueryFocus, QueryLanguage, QueryParameter, QueryResultSummary, QueryRunStatus, SavedQuery,
    diagnostic_summary, draft_code_frame_lines, query_editor_help_entries,
    saved_queries_to_history, token_at_cursor,
};
pub use query_editor::bench as query_editor_bench;
pub use result_grid::{
    ResultCell, ResultCellKind, ResultColumn, ResultColumnStats, ResultExportFormat, ResultGrid,
    ResultGridOutcome, ResultGridState, ResultQueryStatus, ResultRedaction, ResultRow,
    RESULT_CELL_MAX_DISPLAY, RESULT_NULL_ASCII, RESULT_NULL_GLYPH, RESULT_SECRET_MASK,
    RESULT_TRUNC_MARK, clamp_cell_display, export_result_window_tsv, format_result_cell,
    project_result_rows, result_column_model, result_row_to_inspector_fields,
};
pub use result_grid::bench as result_grid_bench;
pub use schema_browser::{
    SchemaBrowser, SchemaBrowserEntry, SchemaBrowserOutcome, SchemaBrowserPresentation,
    SchemaBrowserState, SchemaConnStatus, SchemaContextAction, SchemaNodeKind,
    apply_expanded_set, expanded_ids_from_entries, filter_schema_entries,
    schema_breadcrumbs_from_path, schema_entries_to_tree_nodes, schema_to_quick_open_items,
};
pub use schema_browser::bench as schema_browser_bench;
pub use metrics_dashboard::{
    METRICS_DASHBOARD_DEFAULT_REFRESH_MS, METRICS_DASHBOARD_NARROW_MAX_WIDTH, MetricAlert,
    MetricAlertSeverity, MetricTile, MetricTileHealth, MetricViz, MetricsComparison,
    MetricsDashboard, MetricsDashboardLayoutMode, MetricsDashboardOutcome, MetricsDashboardSlots,
    MetricsDashboardState, MetricsFocus, MetricsTimeRange, apply_metrics_command, commands,
    layout_metrics_dashboard, metrics_dashboard_commands,
};
pub use metrics_dashboard::bench as metrics_dashboard_bench;
pub use session_picker::{
    SESSION_PICKER_OVERLAY_ID, SESSION_PICKER_POPOVER_OVERLAY_ID, SESSION_PICKER_PROVIDER_SEARCH_MIN,
    SESSION_PICKER_WINDOW, SessionConfirmAction, SessionEntry, SessionLoadState, SessionLocation,
    SessionPicker, SessionPickerOutcome, SessionPickerPhase, SessionPickerPresentation,
    SessionPickerState, SessionStatus, example_sessions, filter_sessions,
};
pub use session_picker::bench as session_picker_bench;
pub use connection_manager::{
    CONNECTION_MANAGER_LAUNCHER_OVERLAY_ID, CONNECTION_MANAGER_OVERLAY_ID,
    CONNECTION_MANAGER_RECENT_CAP, CONNECTION_MANAGER_WINDOW, CONNECTION_SECRET_REDACTED,
    CONNECTION_SECRET_REDACTED_ASCII, ConnectionCredentialMeta, ConnectionDiagnosticSummary,
    ConnectionEntry, ConnectionFormDraft, ConnectionFormField, ConnectionKind, ConnectionListView,
    ConnectionManager, ConnectionManagerOutcome, ConnectionManagerPhase,
    ConnectionManagerPresentation, ConnectionManagerState, ConnectionStatus,
    connection_error_diagnostic, connection_to_reconnecting_state, example_connections,
    filter_connections,
};
pub use connection_manager::bench as connection_manager_bench;
pub use plan_review::{
    PLAN_REVIEW_BODY_WINDOW, PLAN_REVIEW_FULLSCREEN_OVERLAY_ID, PLAN_REVIEW_OVERLAY_ID,
    PlanAction, PlanAffectedFile, PlanAssumption, PlanComment, PlanCommentAnchor, PlanDocument,
    PlanFileChange, PlanReview, PlanReviewOutcome, PlanReviewPane, PlanReviewPhase,
    PlanReviewState, PlanRiskItem, PlanSection, PlanSourceRef, PlanTask, PlanTaskStatus,
    example_high_risk_plan, example_plan_document, remap_plan_comments,
};
pub use plan_review::bench as plan_review_bench;
pub use task_rail::{
    TASK_RAIL_COMPACT_WIDTH, TASK_RAIL_DEP_CAP, TASK_RAIL_DRAWER_OVERLAY_ID, TASK_RAIL_DRAWER_WIDTH,
    ActivityActionKind, ActivityDependency, ActivityModel, ActivityScope, TaskRail,
    TaskRailCounts, TaskRailOutcome, TaskRailPresentation, TaskRailRow, TaskRailState,
    TaskRailZoom, activity_model_from_shelf, activity_models_to_shelf, build_task_rail_rows,
    example_activity_models, filter_activity_models, project_task_rail_for_status_bar,
    project_task_rail_list_rows, sort_activity_models, task_rail_counts, task_rail_status_slot,
    task_rail_status_summary,
};
pub use task_rail::bench as task_rail_bench;
pub use subagent_card::{
    SUBAGENT_FULLSCREEN_OVERLAY_ID, SUBAGENT_PREVIEW_LINE_CAP, SUBAGENT_PROVENANCE_CAP,
    SubagentAction, SubagentCard, SubagentCardOutcome, SubagentCardState, SubagentPhase,
    SubagentPresentation, SubagentRun, example_subagent_runs, project_subagent_lines,
    subagent_actions_for, subagent_to_activity_model,
};
pub use subagent_card::bench as subagent_card_bench;
pub use background_task_panel::{
    BACKGROUND_TASKS_OVERLAY_ID, BACKGROUND_TASK_DEFAULT_HISTORY, BACKGROUND_TASK_RAIL_WIDTH,
    BackgroundOutputBuffer, BackgroundOutputLine, BackgroundTask, BackgroundTaskKind,
    BackgroundTaskPanel, BackgroundTaskPanelOutcome, BackgroundTaskPanelState,
    BackgroundTaskPresentation, BackgroundTaskStatus, background_task_to_activity,
    background_task_to_notification, example_background_tasks,
};
pub use background_task_panel::bench as background_task_panel_bench;
pub use prompt_queue::{
    PROMPT_QUEUE_OVERLAY_ID, PROMPT_QUEUE_SUMMARY_PREVIEW, PROMPT_QUEUE_WINDOW, AgentBusyState,
    PromptQueue, PromptQueueItem, PromptQueueOutcome, PromptQueuePhase, PromptQueuePresentation,
    PromptQueueRef, PromptQueueState, PromptQueueStatus, example_prompt_queue,
    project_prompt_queue_from_items,
    pending_queue_len, queue_item_from_composer,
};
pub use prompt_queue::PromptQueueItem as QueuedPrompt;
pub use prompt_queue::bench as prompt_queue_bench;
pub use agent_status_header::{
    AGENT_STATUS_ACTION_CAP, AGENT_STATUS_HEADER_ID, AGENT_STATUS_NARROW_WIDTH, AgentConnectionStatus,
    AgentStatusAction, AgentStatusHeader, AgentStatusHeaderOutcome, AgentStatusHeaderState,
    AgentStatusPresentation, AgentStatusSnapshot, AgentWorkStatus, example_agent_status,
    example_agent_status_idle,
};
pub use agent_status_header::bench as agent_status_header_bench;
pub use integration_status::{
    INTEGRATION_LIST_WINDOW, INTEGRATION_LOG_WINDOW, INTEGRATION_STATUS_OVERLAY_ID,
    IntegrationAction, IntegrationCapability, IntegrationDetailTab, IntegrationEntry,
    IntegrationHealth, IntegrationKind, IntegrationPermission, IntegrationProvenance,
    IntegrationStatus, IntegrationStatusOutcome, IntegrationStatusPresentation,
    IntegrationStatusState, example_integrations,
};
pub use integration_status::bench as integration_status_bench;
pub use working_state_card::{
    WORKING_STATE_FILE_WINDOW, WORKING_STATE_OVERLAY_ID, WorkingAction, WorkingPhase,
    WorkingResource, WorkingState, WorkingStateCard, WorkingStateCardState, WorkingStateOutcome,
    WorkingStatePresentation, example_working_state, example_working_waiting,
    merge_working_into_shelf, working_state_to_shelf_items,
};
pub use working_state_card::bench as working_state_card_bench;
pub use approval_queue::{
    APPROVAL_QUEUE_DRAWER_OVERLAY_ID, APPROVAL_QUEUE_OVERLAY_ID, APPROVAL_QUEUE_WINDOW,
    ApprovalAction, ApprovalBlocking, ApprovalItem, ApprovalKind, ApprovalQueue,
    ApprovalQueueOutcome, ApprovalQueuePresentation, ApprovalQueueState,
    approval_items_to_activity_models, approval_items_to_notifications, approval_queue_badge,
    example_approval_queue,
};
pub use approval_queue::bench as approval_queue_bench;
