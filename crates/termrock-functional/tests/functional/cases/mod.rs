mod g05_reference;
mod h01_alternatives;
mod h01_finder;
mod h01_memory;
mod h01_scope;
mod h01_why;
mod h02_args;
mod h02_config;
mod h02_reload;
mod h02_trust;
mod h02_validation;
mod h03_attach;
mod h03_guards;
mod h03_input;
mod h03_output;
mod h03_picker;
mod h04_actions;
mod h04_plan;
#[path = "h04_snapshots-docker.rs"]
mod h04_snapshots_docker;
#[path = "h04_snapshots-git.rs"]
mod h04_snapshots_git;
#[path = "h04_snapshots-host.rs"]
mod h04_snapshots_host;
#[path = "h04_snapshots-services.rs"]
mod h04_snapshots_services;
mod h05_disk;
mod h05_files;
mod h05_find;
mod h05_previews;
mod h05_scan;
mod h06_cleanup;
mod h06_gate;
mod h06_log;
mod h06_quit;
mod h06_report;
mod s01_breadcrumbsrails;
mod s01_chrome;
mod s01_focusnavigation;
mod s01_helpkeys;
mod s01_overview;
mod s01_quitleave;
mod s01_shell;
mod s01_statushints;
mod s02_buttons;
mod s02_checksradios;
#[path = "../../../../../tests/functional/cases/s02_chipsselects.rs"]
mod s02_chipsselects;
#[path = "../../../../../tests/functional/cases/s02_forms.rs"]
mod s02_forms;
#[path = "../../../../../tests/functional/cases/s02_inputs.rs"]
mod s02_inputs;
#[path = "../../../../../tests/functional/cases/s02_textareas.rs"]
mod s02_textareas;
#[path = "../../../../../tests/functional/cases/s02_toolbars.rs"]
mod s02_toolbars;
#[path = "../../../../../tests/functional/cases/s03_basictable.rs"]
mod s03_basictable;
#[path = "../../../../../tests/functional/cases/s03_celleditors.rs"]
mod s03_celleditors;
#[path = "../../../../../tests/functional/cases/s03_columns.rs"]
mod s03_columns;
#[path = "../../../../../tests/functional/cases/s03_datagrid.rs"]
mod s03_datagrid;
#[path = "../../../../../tests/functional/cases/s03_editable.rs"]
mod s03_editable;
#[path = "../../../../../tests/functional/cases/s03_selection.rs"]
mod s03_selection;
#[path = "../../../../../tests/functional/cases/s04_lists.rs"]
mod s04_lists;
#[path = "../../../../../tests/functional/cases/s04_menus.rs"]
mod s04_menus;
#[path = "../../../../../tests/functional/cases/s04_sidebars.rs"]
mod s04_sidebars;
#[path = "../../../../../tests/functional/cases/s04_tabs.rs"]
mod s04_tabs;
#[path = "../../../../../tests/functional/cases/s04_trees.rs"]
mod s04_trees;
#[path = "../../../../../tests/functional/cases/s05_dialogkinds.rs"]
mod s05_dialogkinds;
#[path = "../../../../../tests/functional/cases/s05_dialogs.rs"]
mod s05_dialogs;
#[path = "../../../../../tests/functional/cases/s05_filebrowser.rs"]
mod s05_filebrowser;
#[path = "../../../../../tests/functional/cases/s05_pickers.rs"]
mod s05_pickers;
#[path = "../../../../../tests/functional/cases/s05_popupstooltips.rs"]
mod s05_popupstooltips;
#[path = "../../../../../tests/functional/cases/s06_emptyerror.rs"]
mod s06_emptyerror;
#[path = "../../../../../tests/functional/cases/s06_meters.rs"]
mod s06_meters;
#[path = "../../../../../tests/functional/cases/s06_notices.rs"]
mod s06_notices;
#[path = "../../../../../tests/functional/cases/s06_progress.rs"]
mod s06_progress;
#[path = "../../../../../tests/functional/cases/s07_masterdetail.rs"]
mod s07_masterdetail;
#[path = "../../../../../tests/functional/cases/s07_panels.rs"]
mod s07_panels;
#[path = "../../../../../tests/functional/cases/s07_panetree.rs"]
mod s07_panetree;
#[path = "../../../../../tests/functional/cases/s07_scrolling.rs"]
mod s07_scrolling;
#[path = "../../../../../tests/functional/cases/s07_splitsstacks.rs"]
mod s07_splitsstacks;
#[path = "../../../../../tests/functional/cases/s08_codeeditor.rs"]
mod s08_codeeditor;
#[path = "../../../../../tests/functional/cases/s08_diff.rs"]
mod s08_diff;
#[path = "../../../../../tests/functional/cases/s08_terminal.rs"]
mod s08_terminal;
#[path = "../../../../../tests/functional/cases/s08_textutilities.rs"]
mod s08_textutilities;
#[path = "../../../../../tests/functional/cases/s09_connections.rs"]
mod s09_connections;
#[path = "../../../../../tests/functional/cases/s09_finder.rs"]
mod s09_finder;
#[path = "../../../../../tests/functional/cases/s09_queryworkbench.rs"]
mod s09_queryworkbench;
#[path = "../../../../../tests/functional/cases/s09_settings.rs"]
mod s09_settings;
#[path = "../../../../../tests/functional/cases/s09_taskrunner.rs"]
mod s09_taskrunner;
#[path = "../../../../../tests/functional/cases/s10_accountsusage.rs"]
mod s10_accountsusage;
#[path = "../../../../../tests/functional/cases/s10_activity.rs"]
mod s10_activity;
#[path = "../../../../../tests/functional/cases/s10_diskcleanup.rs"]
mod s10_diskcleanup;
#[path = "../../../../../tests/functional/cases/s10_launchcockpit.rs"]
mod s10_launchcockpit;
#[path = "../../../../../tests/functional/cases/s10_readonlyreport.rs"]
mod s10_readonlyreport;
#[path = "../../../../../tests/functional/cases/s10_workspaces.rs"]
mod s10_workspaces;
#[path = "../../../../../tests/functional/cases/t01_advanced.rs"]
mod t01_advanced;
#[path = "../../../../../tests/functional/cases/t01_basic.rs"]
mod t01_basic;
#[path = "../../../../../tests/functional/cases/t01_connections.rs"]
mod t01_connections;
#[path = "../../../../../tests/functional/cases/t01_identity.rs"]
mod t01_identity;
#[path = "../../../../../tests/functional/cases/t01_leave.rs"]
mod t01_leave;
#[path = "../../../../../tests/functional/cases/t01_test.rs"]
mod t01_test;
#[path = "../../../../../tests/functional/cases/t02_commit.rs"]
mod t02_commit;
#[path = "../../../../../tests/functional/cases/t02_data.rs"]
mod t02_data;
#[path = "../../../../../tests/functional/cases/t02_discard.rs"]
mod t02_discard;
#[path = "../../../../../tests/functional/cases/t02_editors.rs"]
mod t02_editors;
#[path = "../../../../../tests/functional/cases/t02_explorer.rs"]
mod t02_explorer;
#[path = "../../../../../tests/functional/cases/t02_filters.rs"]
mod t02_filters;
#[path = "../../../../../tests/functional/cases/t02_structure.rs"]
mod t02_structure;
#[path = "../../../../../tests/functional/cases/t02_viewers.rs"]
mod t02_viewers;
#[path = "../../../../../tests/functional/cases/t03_affected.rs"]
mod t03_affected;
#[path = "../../../../../tests/functional/cases/t03_cancelled.rs"]
mod t03_cancelled;
#[path = "../../../../../tests/functional/cases/t03_completion.rs"]
mod t03_completion;
#[path = "../../../../../tests/functional/cases/t03_confirm.rs"]
mod t03_confirm;
#[path = "../../../../../tests/functional/cases/t03_error.rs"]
mod t03_error;
#[path = "../../../../../tests/functional/cases/t03_plan.rs"]
mod t03_plan;
#[path = "../../../../../tests/functional/cases/t03_query.rs"]
mod t03_query;
#[path = "../../../../../tests/functional/cases/t03_rows.rs"]
mod t03_rows;
#[path = "../../../../../tests/functional/cases/t03_running.rs"]
mod t03_running;
#[path = "../../../../../tests/functional/cases/t04_close.rs"]
mod t04_close;
#[path = "../../../../../tests/functional/cases/t04_history.rs"]
mod t04_history;
#[path = "../../../../../tests/functional/cases/t04_preview.rs"]
mod t04_preview;
#[path = "../../../../../tests/functional/cases/t04_quickly.rs"]
mod t04_quickly;
#[path = "../../../../../tests/functional/cases/t04_tabs.rs"]
mod t04_tabs;
#[path = "../../../../../tests/functional/cases/t05_context.rs"]
mod t05_context;
#[path = "../../../../../tests/functional/cases/t05_disconnect.rs"]
mod t05_disconnect;
#[path = "../../../../../tests/functional/cases/t05_help.rs"]
mod t05_help;
#[path = "../../../../../tests/functional/cases/t05_maximize.rs"]
mod t05_maximize;
#[path = "../../../../../tests/functional/cases/t05_menus.rs"]
mod t05_menus;
#[path = "../../../../../tests/functional/cases/t05_quit.rs"]
mod t05_quit;
#[path = "../../../../../tests/functional/cases/t05_viewers.rs"]
mod t05_viewers;
#[path = "../../../../../tests/functional/cases/x01_input.rs"]
mod x01_input;
#[path = "../../../../../tests/functional/cases/x02_suspend.rs"]
mod x02_suspend;
