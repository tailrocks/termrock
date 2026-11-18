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
mod s02_chipsselects;
mod s02_forms;
mod s02_inputs;
mod s02_textareas;
mod s02_toolbars;
mod s03_basictable;
mod s03_celleditors;
mod s03_columns;
mod s03_datagrid;
mod s03_editable;
mod s03_selection;
mod s04_lists;
mod s04_menus;
mod s04_sidebars;
mod s04_tabs;
mod s04_trees;
mod s05_dialogkinds;
mod s05_dialogs;
mod s05_filebrowser;
mod s05_pickers;
mod s05_popupstooltips;
mod s06_emptyerror;
mod s06_meters;
mod s06_notices;
mod s06_progress;
mod s07_masterdetail;
mod s07_panels;
mod s07_panetree;
mod s07_scrolling;
mod s07_splitsstacks;
mod s08_codeeditor;
mod s08_diff;
mod s08_terminal;
mod s08_textutilities;
mod s09_connections;
mod s09_finder;
mod s09_queryworkbench;
mod s09_settings;
mod s09_taskrunner;
mod s10_accountsusage;
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
