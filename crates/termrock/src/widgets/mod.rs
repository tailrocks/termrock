//! Product-neutral terminal widgets with borrowed render data and stable IDs.

pub use crate::ansi_text::{
    AnsiLine, AnsiParseOptions, AnsiSegment, AnsiStream, AnsiText, AnsiTextMode, AnsiTextState,
    is_paint_safe, line_from_ansi, lines_for_log, parse_lines, parse_to_line, strip_bytes,
    strip_str, styled_spans,
};
pub use crate::interaction::Outcome;

mod action_bar;
mod button_group;
mod segmented_control;
mod slider;
mod toggle;
mod agent;
mod agent_blocks;
mod blocks;
mod charts;
mod code_block;
mod command_palette;
mod quick_open;
mod completion_menu;
mod composed_row;
mod connectivity;
mod content;
mod accordion;
mod collapsible;
mod section;
mod badge;
mod highlighted_text;
mod icon;
mod identity;
mod label;
mod kbd;
mod key_value_list;
mod link;
mod tag_chip;
mod attachment_chips;
mod mention;
mod text;
mod separator;
mod toolbar;
mod controls;
mod data_table;
mod data_view;
mod design_inspector;
mod detail_table;
mod dialog;
mod alert_dialog;
mod callout;
mod diff;
mod edit_core;
mod form;
mod form_wizard;
mod stepper;
mod hint_bar;
mod keyboard_help;
mod image_surface;
mod jump_overlay;
mod list;
mod loading_overlay;
mod log_pane;
mod markdown;
mod menu_nav;
mod drawer;
mod dropdown_menu;
mod empty_state;
mod error_state;
mod fullscreen_viewer;
mod preview_card;
mod popover;
mod tooltip;
mod menu_bar;
mod sidebar;
mod breadcrumbs;
mod pagination;
mod card;
mod panel;
mod permission;
mod picker;
mod history_picker;
mod primitives;
mod progress;
mod progress_steps;
mod prompt_composer;
mod review;
mod scroll_area;
mod selection;
mod split_pane;
mod resizable_panel_group;
mod skeleton;
mod spinner;
mod status_bar;
mod semantic_status;
mod status_indicator;
mod surface;
mod table;
mod tabs;
mod text_area;
mod text_input;
mod password_input;
mod number_input;
mod search_input;
mod path_input;
mod token_field;
mod select;
mod multi_select;
mod combobox;
mod file_picker;
mod date_time_picker;
mod keybinding_recorder;
mod theme_picker;
mod notification_center;
mod toast;
mod transcript;
mod tree;
mod tree_table;
mod key_value_table;
mod object_inspector;
mod timeline;
mod event_stream;
mod log_stream;
mod diagnostic;
mod terminal_output;
mod hex_viewer;
mod file_tree;
mod process_table;
mod query_editor;
mod result_grid;
mod schema_browser;
mod search_results;
mod metrics_dashboard;
mod trace_waterfall;
mod dependency_graph;
mod tree_navigation;
mod view_state;
mod viewport;
mod virtual_grid;
mod virtual_list;
mod virtualizer;

pub use crate::style::PanelChrome;
pub use action_bar::{Action, ActionBar, ActionBarState};
pub use button_group::{
    ButtonGroup, ButtonGroupItem, ButtonGroupItemParts, ButtonGroupOrientation, ButtonGroupOutcome,
    ButtonGroupParts, ButtonGroupRecipe, ButtonGroupState,
};
pub use segmented_control::{
    SegmentedControl, SegmentedControlOutcome, SegmentedControlParts, SegmentedControlState,
    SegmentedItem, SegmentedItemParts, SegmentedPresentation,
};
pub use toggle::{
    Toggle, ToggleGroup, ToggleGroupItem, ToggleGroupItemParts, ToggleGroupOrientation,
    ToggleGroupOutcome, ToggleGroupParts, ToggleGroupRecipe, ToggleGroupState, ToggleGroupType,
    ToggleOutcome, ToggleParts, ToggleRecipe, ToggleSize, ToggleState, ToggleValue,
};
pub use agent::{ThinkingBlock, TokenMeter, ToolCard, ToolStatus};
pub use timeline::{
    CheckpointTimeline, Timeline, TimelineEvent, TimelineOutcome, TimelineRecipe, TimelineRegion,
    TimelineRowKind, TimelineState, TimelineStatus, filter_timeline_events,
};
pub use event_stream::{
    EventSeverity, EventStream, EventStreamOutcome, EventStreamRegion, EventStreamState,
    StreamEvent, StreamRowKind, filter_stream_events,
};
pub use event_stream::bench as event_stream_bench;
pub use log_stream::{
    LogLevel, LogLine, LogLineRecipe, LogStream, LogStreamOutcome, LogStreamRegion, LogStreamState,
    LogWrap, escape_log_text, filter_log_lines, log_lines_from_plain,
};
pub use log_stream::bench as log_stream_bench;
pub use diagnostic::{
    CodeFrame, CodeFrameLine, Diagnostic, DiagnosticNote, DiagnosticOutcome, DiagnosticRecipe,
    DiagnosticRegion, DiagnosticSeverity, DiagnosticState, DiagnosticView, FixApplicability,
    RelatedLocation, SourceLabel, SourceRange, SpanStyle, SuggestedFix, CODE_FRAME_TAB_STOP,
    code_frame_window, diagnostics_to_gutter_marks, diagnostics_to_highlights, expand_tabs,
    format_diagnostic_plain, format_diagnostics_plain,
};
pub use diagnostic::bench as diagnostic_bench;
pub use terminal_output::{
    TerminalCommandMeta, TerminalEnvEntry, TerminalLine, TerminalOutput, TerminalOutputOutcome,
    TerminalOutputRecipe, TerminalOutputRegion, TerminalOutputState, TerminalPaintMode,
    TerminalRunStatus, TerminalStream, escape_raw_terminal, filter_terminal_lines,
    format_duration_ms, redact_env_value,
};
pub use terminal_output::bench as terminal_output_bench;
pub use hex_viewer::{
    HexAsciiMode, HexEndian, HexInspectorValues, HexRegion, HexViewer, HexViewerOutcome,
    HexViewerState, HexWindow, HEX_DEFAULT_BYTES_PER_ROW, HEX_MAX_BYTES_PER_ROW,
    HEX_MIN_BYTES_PER_ROW, auto_bytes_per_row, col_for_offset, find_in_window, format_byte_hex,
    format_hex_dump, format_inspector_line, format_offset, in_selection, inspect_at,
    interpret_byte, min_width_for_bpr, normalize_range, offset_for_row, offset_width_chars,
    parse_search_query, row_count, row_for_offset,
};
pub use hex_viewer::bench as hex_viewer_bench;
pub use file_tree::{
    FileGitStatus, FileTree, FileTreeDestructiveConfirm, FileTreeDraft, FileTreeEntry,
    FileTreeKind, FileTreeOutcome, FileTreeState, breadcrumbs_from_path, file_entries_to_tree_nodes,
    file_tree_to_quick_open_items, filter_file_tree_entries, normalize_path_display, path_segments,
};
pub use file_tree::bench as file_tree_bench;
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
pub use search_results::{
    SearchFlatRow, SearchResultGroup, SearchResultItem, SearchResultKind, SearchResults,
    SearchResultsOutcome, SearchResultsState, SearchResultsStatus, collect_match_targets,
    flatten_search_results, keep_first_match_slice, search_results_to_quick_open,
    truncate_snippet_keep_match,
};
pub use search_results::bench as search_results_bench;
pub use metrics_dashboard::{
    METRICS_DASHBOARD_DEFAULT_REFRESH_MS, METRICS_DASHBOARD_NARROW_MAX_WIDTH, MetricAlert,
    MetricAlertSeverity, MetricTile, MetricTileHealth, MetricViz, MetricsComparison,
    MetricsDashboard, MetricsDashboardLayoutMode, MetricsDashboardOutcome, MetricsDashboardSlots,
    MetricsDashboardState, MetricsFocus, MetricsTimeRange, apply_metrics_command, commands,
    layout_metrics_dashboard, metrics_dashboard_commands,
};
pub use metrics_dashboard::bench as metrics_dashboard_bench;
pub use trace_waterfall::{
    TRACE_NAME_COL_DEFAULT, TRACE_NAME_COL_MAX, TRACE_NAME_COL_MIN, TraceNavMode, TraceSpan,
    TraceSpanStatus, TraceWaterfall, TraceWaterfallOutcome, TraceWaterfallState,
    filter_critical_path, filter_trace_spans, format_trace_duration_ms, format_trace_offset_ms,
    span_bar_cols, span_to_inspector_fields, span_to_timeline_event, trace_total_ms,
};
pub use trace_waterfall::bench as trace_waterfall_bench;
pub use dependency_graph::{
    DEP_GRAPH_AUTO_TREE_NODES, DEP_GRAPH_CELL_H, DEP_GRAPH_CELL_W, DEP_GRAPH_NARROW_MAX_WIDTH,
    DepEdge, DepEdgeKind, DepLayoutNode, DepNode, DepNodeKind, DepNodeStatus, DependencyGraph,
    DependencyGraphOutcome, DependencyGraphState, DependencyGraphView, GraphUnreadableReason,
    choose_dependency_view, dep_node_to_inspector_fields, dependency_tree_column_model,
    filter_dep_edges, filter_dep_nodes, layout_content_size, layout_dependency_layers,
    project_dep_tree_rows,
};
pub use dependency_graph::bench as dependency_graph_bench;
pub use agent_blocks::{
    ModeRibbon, ModeRibbonOutcome, ModeRibbonState, PlanReview, PlanReviewOutcome, PlanReviewState,
    PlanStep, QuestionFlow, QuestionFlowOutcome, QuestionFlowState, QuestionOption, QuestionStep,
    SessionItem, SessionPicker, SessionPickerOutcome, TaskRail, WorkbenchMode,
    session_picker_handle_key,
};
pub use blocks::{
    BlockChrome, OpsDashboardOutcome, OpsDashboardState, OpsRegion, ResourceBrowserOutcome,
    ResourceBrowserState, SettingsShellOutcome, SettingsShellState,
};
pub use charts::{
    BarDatum, BarSeries, Chart, ChartSeries, Gauge, HistBucket, Histogram, MeterSegment,
    ScaleDomain, ScaleMode, SegmentedMeter, Sparkline, VizGlyphSet, glyph_for_fraction,
    resolve_domain, window_samples,
};
pub use charts::bench as charts_bench;
pub use code_block::{
    AnsiSyntax, CodeBlock, CodeBlockOutcome, CodeBlockParts, CodeBlockState, CodeGutterMark,
    CodeHighlight, CodeHighlightKind, CodeSourceMeta, CodeWrap, ControlRender, PlainSyntax,
    RoleTokenSyntax, SyntaxHighlighter, TokenSyntax, prepare_code_display, syntax_role_style,
};
pub use command_palette::{
    COMMAND_PALETTE_FULLSCREEN_MAX_HEIGHT, COMMAND_PALETTE_FULLSCREEN_MAX_WIDTH,
    COMMAND_PALETTE_HISTORY_CAP, COMMAND_PALETTE_OVERLAY_ID, CommandEntry, CommandPalette,
    CommandPaletteOutcome, CommandPalettePhase, CommandPalettePresentation, CommandPaletteSize,
    CommandPaletteState, command_palette_presentation_for_bounds, default_command_palette_intent,
    dismiss_command_palette_overlay, entries_from_keymap, example_command_catalog,
    filter_command_entries, fuzzy_match_label, open_command_palette_overlay, place_command_palette,
};
pub use quick_open::{
    QUICK_OPEN_DEFAULT_LIMIT, QUICK_OPEN_FULLSCREEN_MAX_HEIGHT, QUICK_OPEN_FULLSCREEN_MAX_WIDTH,
    QUICK_OPEN_OVERLAY_ID, QUICK_OPEN_PROVIDER_STRIP_COMPACT_MAX, ParsedQuickOpenQuery,
    QuickOpen, QuickOpenItem, QuickOpenOutcome, QuickOpenPresentation, QuickOpenPreview,
    QuickOpenProvider, QuickOpenSearchRequest, QuickOpenSize, QuickOpenState,
    default_quick_open_intent, dismiss_quick_open_overlay, example_quick_open_files,
    example_quick_open_providers, example_quick_open_symbols, filter_quick_open_items,
    open_quick_open_fullscreen, open_quick_open_overlay, parse_quick_open_query, place_quick_open,
    quick_open_jump_targets, quick_open_presentation_for_bounds,
};
pub use completion_menu::{
    COMPLETION_DOCS_DEFAULT_WIDTH, COMPLETION_FULLSCREEN_MAX_HEIGHT, COMPLETION_FULLSCREEN_MAX_WIDTH,
    COMPLETION_OVERLAY_ID, CompletionCandidate, CompletionMenu, CompletionMenuOutcome,
    CompletionMenuSize, CompletionMenuState, CompletionPresentation, CompletionSlots,
    CompletionStatus, completion_presentation_for, default_completion_intent,
    dismiss_completion_overlay, open_completion_configured, open_completion_overlay,
    place_completion_menu, place_completion_with_presentation,
};
pub use composed_row::{ComposedRow, ComposedRowParts};
pub use callout::{
    Alert, AlertOutcome, AlertRecipe, AlertSlots, AlertState, AlertTone, Callout, CalloutOutcome,
    CalloutRecipe, CalloutSlots, CalloutTone,
};
pub use content::{
    Heading, HeadingLevel, HeadingParts, HeadingRecipe, Paragraph, ParagraphKind, ParagraphParts,
    ParagraphRecipe,
};
pub use badge::{
    Badge, BadgeCount, BadgeFill, BadgeOutcome, BadgeParts, BadgeState, BadgeVariant,
};
pub use kbd::{
    ChordFormat, Kbd, KbdVariant, ModifierStyle, Platform, ShortcutForm, ShortcutHint,
    format_alternatives, format_binding, format_chord, format_sequence, kbd_from_chord,
};
pub use key_value_list::{
    KeyValueList, KeyValueListOutcome, KeyValueListParts, KeyValueListState, KvDensity, KvEntry,
    KvEntryParts, KvLayout, KvStatus,
};
pub use key_value_table::{
    KeyValueTable, KeyValueTableOutcome, KeyValueTableState, KvtField, KvtMode, KvtRegion,
    KvtRowKind, KvtValidation,
};
pub use link::{
    ActionLink, ActionLinkOutcome, DestinationDisplay, Link, LinkDestination, LinkOutcome,
    LinkParts, LinkState, LinkVariant,
};
pub use highlighted_text::{
    HighlightVisual, HighlightedText, HighlightedTextParts, MatchKind, MatchRange, MatchRanges,
    MatchTruncate, match_range_from_display_cols, substring_ranges,
    substring_ranges_ignore_ascii_case,
};
pub use icon::{Icon, IconParts};
pub use identity::{
    AvatarFace, AvatarGlyph, AvatarGlyphParts, AvatarSize, Identity, IdentityParts, IdentityRole,
    PresenceStatus, identity_seed, initials_from_name, role_for_seed,
};
pub use label::{
    CaptionLayout, CaptionParts, DROP_DESCRIPTION_WIDTH, DROP_MARK_WIDTH, Description,
    DescriptionKind, DescriptionParts, FieldCaption, Label, LabelMark, LabelParts, LabelTone,
    line_plain,
};
pub use text::{
    SelectablePolicy, Text, TextAlign, TextEmphasis, TextLayout, TextLine, TextOverflow, TextSegment,
    TextSpan, ascii_ellipsis,
};
pub use accordion::{
    Accordion, AccordionItem, AccordionItemParts, AccordionMode, AccordionOutcome, AccordionParts,
    AccordionRecipe, AccordionState,
};
pub use collapsible::{
    CollapsedContentPolicy, Collapsible, CollapsibleOutcome, CollapsibleParts, CollapsibleState,
    CollapsibleVariant,
};
pub use section::{
    Section, SectionAction, SectionOutcome, SectionParts, SectionState, SectionVariant,
};
pub use surface::{
    Surface, SurfaceElevation, SurfaceFill, SurfacePaintPlan, SurfaceParts, SurfaceRecipe,
};
pub use controls::{
    Checkbox, CheckboxOutcome, CheckboxParts, CheckboxState, CheckboxValue, RadioGroup,
    RadioGroupOrientation, RadioGroupParts, RadioOption, RadioOptionParts, RadioOutcome,
    RadioSelectionPolicy, RadioState, Switch, SwitchOutcome, SwitchParts, SwitchRecipe, SwitchState,
};
pub use data_table::{
    DataTable, DataTableCellRegion, DataTableHeaderRegion, DataTableNavMode, DataTableOutcome,
    DataTableState, DataTableToolbar,
};
pub use data_view::bench as data_view_bench;
pub use data_view::{
    CellCoord, ColumnModel, ColumnPin, CopyPayload, DataColumn, DataColumnWidth, DataDensity,
    DataViewOutcome, ExpandState, FilterSpec, GroupHeader, LoadState, SelectionMode,
    SelectionModel, SortSpec, VirtualWindow,
};
pub use design_inspector::{DesignInspector, DesignInspectorFrame, InspectorPanel};
pub use detail_table::{
    DetailCapability, DetailRow, DetailTable, DetailTableOutcome, DetailTableState,
};
pub use dialog::{
    Backdrop, ChoiceDialog, ChoiceDialogState, DIALOG_FULLSCREEN_MAX_HEIGHT,
    DIALOG_FULLSCREEN_MAX_WIDTH, DIALOG_NESTED_OVERLAY_PREFIX, DIALOG_OVERLAY_ID, Dialog,
    DialogClosePolicy, DialogFocusZone, DialogOutcome, DialogRecipe, DialogSize, DialogSlots,
    DialogState, DialogVariant, MessageDialog, default_dialog_intent, dialog_recipe_for_bounds,
    dismiss_dialog_overlay, open_alert_dialog_overlay, open_dialog_child_overlay,
    open_dialog_configured, open_dialog_overlay, place_dialog, place_dialog_recipe,
};
pub use alert_dialog::{
    ALERT_DIALOG_DEFAULT_HEIGHT, ALERT_DIALOG_DEFAULT_WIDTH, ALERT_DIALOG_OVERLAY_ID, AlertConfirmGates,
    AlertDialog, AlertDialogOutcome, AlertDialogState, AlertKind, AlertReversibility, AlertScope,
    dismiss_alert_dialog_overlay, open_alert_dialog_widget_overlay,
};
pub use diff::{
    DiffEffectiveMode, DiffFile, DiffHunk, DiffKind, DiffLine, DiffMode, DiffRegion,
    DiffState, DiffSyntaxSpan, DiffView, DiffViewOutcome, DiffViewState, DiffWordKind,
    DiffWordSpan, escape_diff_text, filter_diff_lines,
};
pub use diff::bench as diff_bench;
pub use form::{
    any_dirty, any_touched, collect_errors, first_invalid_id, required_filled, Field, FieldStatus,
    Fieldset, Form, FormField, FormFieldRegion, FormLayout, FormOutcome, FormSection, FormState,
};
pub use form_wizard::{
    FORM_WIZARD_COMPACT_MAX_HEIGHT, FORM_WIZARD_NARROW_MAX_WIDTH, FormWizard, FormWizardOutcome,
    FormWizardPresentation, FormWizardState, StepChangeReason, WizardGate, WizardPhase,
    WizardProgress, WizardStep, WizardStepStatus,
};
pub use stepper::{
    STEPPER_COMPACT_MAX_HEIGHT, STEPPER_COMPACT_MAX_WIDTH, STEPPER_NARROW_MAX_WIDTH, StepItem,
    StepStatus, Stepper, StepperNavPolicy, StepperOrientation, StepperOutcome,
    StepperPresentation, StepperState, default_stepper_intent, example_onboarding_steps,
    step_items_from_titles, stepper_presentation_for_bounds,
};
pub use hint_bar::{
    Hint, HintBar, HintSpan, hint_row_cols, render_hint_bar, styled_hint_spans, wrapped_hint_lines,
};
pub use keyboard_help::{
    KEYBOARD_HELP_COMPACT_MAX_WIDTH, KEYBOARD_HELP_OVERLAY_ID, KEYBOARD_HELP_TINY_MAX_HEIGHT,
    KEYBOARD_HELP_TINY_MAX_WIDTH, DemoHelpAction, HelpEntry, HelpEntrySource, KeyboardHelp,
    KeyboardHelpMode, KeyboardHelpOutcome, KeyboardHelpPresentation, KeyboardHelpSize,
    KeyboardHelpState, contract_help_entries, default_keyboard_help_intent,
    dismiss_keyboard_help_overlay, example_help_entries, example_help_keymap, filter_help_entries,
    help_entries_from_conflicts, help_entries_from_keymap, help_entries_from_overlays,
    help_entries_from_semantics, help_entries_to_hints, keyboard_help_presentation_for_bounds,
    mark_remapped_help_entries, merge_help_entries, open_keyboard_help_overlay, place_keyboard_help,
};
pub use image_surface::{ImageMeta, ImageProtocol, ImageSurface, protocol_emission_hint};
pub use jump_overlay::{
    JUMP_LABEL_ALPHABET, JUMP_OVERLAY_ID, JumpCandidate, JumpFilter, JumpMode, JumpModeState,
    JumpOutcome, JumpOverlay, JumpOverlayState, JumpTarget, assign_jump_badges,
    assign_jump_badges_from_semantics, assign_jump_labels, assign_jump_labels_from_semantics,
    collect_jump_candidates, dismiss_jump_overlay, generate_jump_labels, jump_status_line,
    open_jump_overlay, replay_jump_keys,
};
pub use list::{
    LIST_NARROW_DROP_ORDER, List, ListClickPolicy, ListDensity, ListRow, ListSelectionMode,
    ListState, RowRole, filter_list_rows,
};
pub use log_pane::{LogPane, LogPaneState};
pub use markdown::{
    MarkdownBlock, MarkdownBlockKind, MarkdownInline, MarkdownInlineKind, MarkdownLinkRegion,
    MarkdownOutcome, MarkdownParts, MarkdownView, MarkdownViewState, SourceAnchor,
    project_markdown, project_plain_lines,
};
pub use menu_nav::{Menu, MenuOutcome, MenuState};
pub use drawer::{
    DRAWER_DEFAULT_HEIGHT, DRAWER_DEFAULT_WIDTH, DRAWER_FULLSCREEN_MAX_HEIGHT,
    DRAWER_FULLSCREEN_MAX_WIDTH, DRAWER_HANDLE_CELLS, DRAWER_NESTED_OVERLAY_PREFIX, DRAWER_OVERLAY_ID,
    Drawer, DrawerEdge, DrawerModality, DrawerOutcome, DrawerPresentation, DrawerSlots, DrawerState,
    Sheet, SheetState, dismiss_drawer_overlay, drawer_presentation_for, open_drawer_configured,
    open_drawer_nested_overlay, open_drawer_overlay, place_drawer, place_drawer_on_edge,
};
pub use fullscreen_viewer::{
    FULLSCREEN_VIEWER_HINT, FULLSCREEN_VIEWER_NESTED_PREFIX, FULLSCREEN_VIEWER_OVERLAY_ID,
    FullscreenViewer, FullscreenViewerOutcome, FullscreenViewerSlots, FullscreenViewerState,
    ScrollAnchor, SemanticZoomBadge, SemanticZoomState, SourceContext, ViewerChromeFocus,
    ViewerContentKind, ZoomLevel, dismiss_fullscreen_viewer_overlay,
    fullscreen_viewer_has_nested_top, open_fullscreen_viewer_child_overlay,
    open_fullscreen_viewer_overlay,
};
pub use dropdown_menu::{
    CONTEXT_MENU_OVERLAY_ID, CONTEXT_MENU_SUBMENU_PREFIX, DROPDOWN_MENU_OVERLAY_ID,
    DROPDOWN_MENU_SUBMENU_PREFIX, MENU_PROMOTE_MAX_HEIGHT, MENU_PROMOTE_MAX_ITEMS,
    MENU_PROMOTE_MAX_WIDTH, MENU_PROMOTE_MIN_DEPTH, ContextMenuState, ContextMenuWidget,
    DropdownMenu, DropdownMenuOutcome, DropdownMenuPresentation, DropdownMenuState, MenuItem,
    MenuOpenTrigger, dismiss_context_menu_overlays, dismiss_dropdown_menu_overlays,
    dropdown_menu_presentation_for, flatten_menu_nodes, measure_menu_panel, menu_items_to_nodes,
    open_context_menu_overlay, open_dropdown_menu_overlay, open_menu_submenu_overlay,
    place_context_menu, place_dropdown_menu,
};
/// Context menu paint widget (same cascade engine as [`DropdownMenu`]).
pub type ContextMenu<'a, Id> = DropdownMenu<'a, Id>;
pub use preview_card::{
    PREVIEW_CARD_DEFAULT_DELAY_MS, PREVIEW_CARD_DEFAULT_MAX_HEIGHT, PREVIEW_CARD_DEFAULT_MAX_WIDTH,
    PREVIEW_CARD_HINT, PREVIEW_CARD_OVERLAY_ID, PREVIEW_CARD_PINNED_HINT,
    PREVIEW_CARD_SELECTION_DEBOUNCE_MS, PreviewCard, PreviewCardContent, PreviewCardOutcome,
    PreviewCardSlots, PreviewCardState, PreviewLoadState, PreviewMetadata, PreviewResourceKind,
    PreviewTrigger, dismiss_preview_card_overlay, example_command_preview, example_file_preview,
    example_session_preview, example_symbol_preview, open_preview_card_overlay,
    open_preview_card_pinned_overlay, place_preview_card, preview_card_overlay_size,
};
pub use popover::{
    POPOVER_CONTRACT_MAX_HEIGHT, POPOVER_CONTRACT_MAX_WIDTH, POPOVER_OVERLAY_ID, Popover,
    PopoverModality, PopoverOutcome, PopoverPresentation, PopoverSlots, PopoverState,
    dismiss_popover_overlay, open_popover_configured, open_popover_modal_overlay,
    open_popover_nested_overlay, open_popover_overlay, open_popover_with_presentation,
    place_popover, place_popover_with_modality, popover_presentation_for,
};
pub use tooltip::{
    TOOLTIP_DEFAULT_DELAY_MS, TOOLTIP_DEFAULT_MAX_WIDTH, TOOLTIP_OVERLAY_ID, Tooltip,
    TooltipContent, TooltipOutcome, TooltipPrefer, TooltipState, TooltipTrigger, TooltipVariant,
    dismiss_tooltip_overlay, open_tooltip_overlay, place_tooltip, tooltip_overlay_size,
};
pub use menu_bar::{
    MENU_BAR_NARROW_MAX_WIDTH, MENU_BAR_OVERLAY_ID, MENU_BAR_SUBMENU_OVERLAY_PREFIX, MenuBar,
    MenuBarMenu, MenuBarOutcome, MenuBarPresentation, MenuBarState, MenuCommandRef, MenuNode,
    MenuRowKind, default_menu_bar_intent, dismiss_menu_bar_overlays, example_app_menus,
    flatten_menu_commands, menu_bar_presentation_for_width, open_menu_bar_overlay,
    open_menu_bar_submenu_overlay, place_menu_bar_panel,
};
pub use breadcrumbs::{
    BREADCRUMBS_COLLAPSE_MAX_WIDTH, BREADCRUMBS_ELLIPSIS, BREADCRUMBS_ELLIPSIS_ASCII, BreadcrumbHit,
    BreadcrumbItem, BreadcrumbSeparator, BreadcrumbStatus, Breadcrumbs, BreadcrumbsMode,
    BreadcrumbsOutcome, BreadcrumbsPresentation, BreadcrumbsState, crumbs_from_labels,
};
pub use pagination::{
    PAGINATION_COMPACT_MAX_WIDTH, PAGINATION_MINIMAL_MAX_WIDTH, PageRequest, PageTotal, Pagination,
    PaginationOutcome, PaginationPart, PaginationPresentation, PaginationState,
    guidance as pagination_guidance,
};
pub use sidebar::{
    SIDEBAR_DRAWER_MAX_WIDTH, SIDEBAR_DRAWER_OVERLAY_ID, SIDEBAR_RAIL_MAX_WIDTH, NavItem,
    NavItemKind, NavItemStatus, NavigationList, NavigationListOutcome, NavigationListState, Sidebar,
    SidebarItem, SidebarOutcome, SidebarPresentation, SidebarState, example_agent_workbench_nav,
    example_database_nav, example_settings_nav, sidebar_presentation_for_width,
};
pub use card::{Card, CardParts};
pub use panel::{
    Panel, PanelAction, PanelBody, PanelOutcome, PanelParts, PanelSlots, PanelState, PanelVariant,
};
pub use permission::{
    DataMovement, EditField, ExecutionLocation, InitiatorKind, PERMISSION_OVERLAY_ID,
    PermissionAction, PermissionActionKind, PermissionActionRegion, PermissionAuditEntry,
    PermissionOutcome, PermissionPrompt, PermissionPromptState, PermissionProvenance,
    PermissionQueue, PermissionRequest, PermissionRisk, PermissionScope, PermissionTarget,
    PriorGrant, ProvenanceHop, StalePermission, StaleReason,
};
pub use picker::{
    PICKER_OVERLAY_ID, Picker, PickerOutcome, PickerSize, PickerState, dismiss_picker_overlay,
    open_picker_overlay, place_picker,
};
pub use history_picker::{
    HISTORY_PICKER_FULLSCREEN_MAX_HEIGHT, HISTORY_PICKER_FULLSCREEN_MAX_WIDTH,
    HISTORY_PICKER_OVERLAY_ID, HistoryEntry, HistoryKind, HistoryPicker, HistoryPickerOutcome,
    HistoryPickerPresentation, HistoryPickerSize, HistoryPickerState, HistoryRedaction,
    default_history_picker_intent, dismiss_history_picker_overlay, example_history_entries,
    filter_history_entries, history_picker_presentation_for_bounds, history_redaction_secret,
    open_history_picker_fullscreen, open_history_picker_overlay,
    open_history_picker_popover_overlay, place_history_picker, place_history_picker_popover,
    redact_history_text,
};
pub use primitives::{
    ActivationOutcome, ActivationState, Button, ButtonParts, ButtonSize, ButtonState, ButtonVariant,
    ICON_BUTTON_MIN_HIT, IconButton, IconButtonParts, IconButtonSize, IconButtonState,
    button_hit, toolbar_icon_action,
};
pub use tag_chip::{
    Chip, ChipOutcome, ChipState, Tag, TagOutcome, TagState, TokenItem, TokenPart, TokenParts,
    TokenStatus, TokenStrip, TokenStripLayout, TokenStripOutcome, TokenStripState, remove_label,
};
pub use attachment_chips::{
    PASTE_CHIP_THRESHOLD, PASTE_EXPAND_LINES, PASTE_PREVIEW_CHARS, PROGRESS_UNKNOWN,
    AttachmentChip, AttachmentChipOutcome, AttachmentChipState, AttachmentItem, AttachmentStatus,
    AttachmentStripEvent, AttachmentType, PasteChip, PasteChipOutcome, PasteChipState, PastePayload,
    attachment_semantic_summary, attachment_token_items, fill_attachment_strip_labels,
    map_strip_outcome, paint_attachment_strip, paste_preview_from, paste_semantic_summary,
};
pub use attachment_chips::bench as attachment_chips_bench;
pub use mention::{
    ENTITY_MENTION_OVERLAY_ID, FILE_MENTION_OVERLAY_ID, MENTION_DISAMBIG_MAX, MENTION_TRIGGER_AT,
    MENTION_TRIGGER_HASH, EntityMention, EntityMentionState, FileMention, FileMentionState,
    InlineMention, InlineMentionOutcome, InlineMentionState, MentionCandidate, MentionCursor,
    MentionDisambiguator, MentionDraft, MentionFamily, MentionQuery, MentionRef, MentionSegment,
    MentionType, MentionValidity, apply_mention_insert, detect_entity_mention_query,
    detect_file_mention_query, detect_mention_query, filter_mention_candidates,
    mention_candidates_as_completion, mention_semantic_description, mention_to_completion_candidate,
    mention_to_token_item, parse_draft_with_mentions, parse_mention_markup,
};
pub use mention::bench as mention_bench;
pub use separator::{
    Separator, SeparatorLine, SeparatorOrientation, SeparatorThickness, SeparatorVariant,
};
pub use toolbar::{
    Toolbar, ToolbarItem, ToolbarItemKind, ToolbarOrientation, ToolbarOutcome, ToolbarPlan,
    ToolbarState, ToolbarVariant,
};
pub use progress_steps::{
    PROGRESS_STEPS_COMPACT_MAX_WIDTH, PROGRESS_STEPS_HINT, PROGRESS_STEPS_SUMMARY_MAX_WIDTH,
    ProgressStep, ProgressStepStatus, ProgressSteps, ProgressStepsMode, ProgressStepsOutcome,
    ProgressStepsPresentation, ProgressStepsState, example_agent_plan_steps, example_build_pipeline,
    paint_progress_steps_as_timeline, progress_steps_as_list_rows, progress_steps_as_timeline_events,
};
pub use progress::{

    DEFAULT_PROGRESS_FRAMES, MIN_WIDTH_WITH_PERCENTAGE, PROGRESS_ASCII_FRAMES,
    PROGRESS_DEFAULT_THROTTLE_MS, Progress, ProgressBar, ProgressBarState, ProgressKind,
    ProgressRecipe, ProgressStatus, ProgressUnit,
};
pub use slider::{
    RangeSlider, RangeSliderOutcome, RangeSliderParts, RangeSliderState, RangeThumb, Slider,
    SliderBounds, SliderMark, SliderOrientation, SliderOutcome, SliderParts, SliderState,
    SLIDER_MIN_TRACK, SLIDER_NUMERIC_FALLBACK_WIDTH,
};
pub use prompt_composer::{
    ChipKind, CompletionKind, CompletionQuery, ComposerChip, ComposerConnection,
    ComposerPresentation, ContextEstimate, LARGE_PASTE_THRESHOLD, ModeIndicator, ModelIndicator,
    PROMPT_COMPLETION_OVERLAY_ID, PROMPT_FULLSCREEN_OVERLAY_ID, PROMPT_HISTORY_LIMIT,
    PROMPT_UNDO_LIMIT, PromptComposer, PromptComposerLayout, PromptComposerOutcome,
    PromptComposerState, QueuedPrompt, SubmitPolicy, attachment_to_composer_chip,
    composer_chip_to_attachment, composer_chip_to_paste, detect_completion, paste_to_composer_chip,
    prompt_composer_help_entries, submit_history_to_entries,
};
pub use prompt_composer::bench as prompt_composer_bench;
pub use object_inspector::{
    InspectKind, InspectMode, InspectNodeStatus, InspectPresentation, InspectRegion,
    InspectorField, ObjectInspector, ObjectInspectorOutcome, ObjectInspectorState,
    escape_inspect_value, filter_inspect_fields,
};
pub use review::{
    DiffComment, DiffCommentAnchor, DiffDecision, DiffDestructiveConfirm, DiffReview,
    DiffReviewFileRow, DiffReviewOutcome, DiffReviewRegion, DiffReviewState, DiffReviewSummary,
    DiffReviewUnit, DiffReviewUnitKind, DIFF_REVIEW_UNDO_LIMIT,
};
pub use review::bench as diff_review_bench;
pub use scroll_area::{
    ScrollArea, ScrollAreaState, ScrollBarVisibility, ScrollChain, ScrollOutcome, VisibleRange,
};
pub use selection::Selection;
pub use split_pane::{
    SplitDirection, SplitPane, SplitPaneLayout, SplitPaneOutcome, SplitPaneState, SplitRatio,
    SplitSide,
};
pub use resizable_panel_group::{
    dashboard_panels, workbench_panels, PanelDock, PanelId, PanelLayoutPreset, PanelGroupRecipe,
    PanelRect, ResizablePanelGroup, ResizablePanelGroupLayout, ResizablePanelGroupState,
    ResizablePanelOutcome, ResizablePanelSpec,
};
pub use skeleton::{
    SKELETON_FILL_ASCII, SKELETON_FILL_UNICODE, SKELETON_PULSE_PERIOD_MS, Skeleton, SkeletonLayout,
    SkeletonRecipe, SkeletonShape, SkeletonState,
};
pub use spinner::{

    SPINNER_ASCII_FRAMES, SPINNER_BRAILLE_FRAMES, SPINNER_DEFAULT_PERIOD_MS,
    SPINNER_RECONNECT_UNICODE, SPINNER_WAITING_ASCII, SPINNER_WAITING_UNICODE, ActivityIndicator,
    ActivityPhase, Spinner, SpinnerGlyphSet, SpinnerState, SpinnerVariant,
};
pub use semantic_status::SemanticStatus;
pub use status_indicator::{
    StatusIndicator, StatusIndicatorState, StatusIndicatorVariant, example_status_catalog,
};
pub use status_bar::{

    StatusBar, StatusBarRecipe, StatusBarState, StatusKind, StatusRegion, StatusSlot,
    TransientStatus,
};
pub use table::{
    CellAlignment, CellOverflow, Column, ColumnWidth, SortDirection, Table, TableBodyState,
    TableHeaderRegion, TableOutcome, TableRecipe, TableRow, TableRowRegion, TableState,
    resolve_widths,
};
pub use tabs::{
    TAB_GAP, TABS_OVERFLOW_MAX_WIDTH, TABS_SELECT_MAX_WIDTH, Tab, TabCell, TabStatus, Tabs,
    TabsActivation, TabsOrientation, TabsOutcome, TabsPresentation, TabsState, lay_out_tabs,
    tab_at_column,
};
pub use text_area::{
    TextArea, TextAreaOutcome, TextAreaState, TextAreaVariant, TextCursor, TextWrap,
};
pub use text_input::{
    EditAction, TextInput, TextInputOutcome, TextInputParts, TextInputState, TextInputValidity,
    Validation,
};
pub use password_input::{
    ClipboardPolicy, PasswordConfirmState, PasswordInput, PasswordInputOutcome, PasswordInputParts,
    PasswordInputState, PasswordStrengthHint, RevealPolicy,
};
pub use number_input::{
    NumberConstraints, NumberInput, NumberInputOutcome, NumberInputParts, NumberInputState,
    NumberKind, NumberParse, NumberValidity,
};
pub use search_input::{
    DEFAULT_DEBOUNCE, DEFAULT_HISTORY_LIMIT, SearchFilterChip, SearchInput, SearchInputOutcome,
    SearchInputParts, SearchInputState, SearchStatus, SearchSyntax,
};
pub use path_input::{
    DEFAULT_PATH_HISTORY_LIMIT, PathCompletionPrefix, PathExpect, PathFsStatus, PathInput,
    PathInputOutcome, PathInputParts, PathInputState, PathRisk, PathStyle, completion_prefix,
    expand_env_vars, expand_tilde, is_absolute_path, join_path, normalize_separators,
};
pub use token_field::{
    CommitSeparators, DuplicatePolicy, FieldToken, TokenField, TokenFieldOutcome, TokenFieldParts,
    TokenFieldState, TokenFieldZone,
};
pub use select::{
    SELECT_FULLSCREEN_MAX_HEIGHT, SELECT_FULLSCREEN_MAX_WIDTH, Select, SelectOption, SelectOutcome,
    SelectPresentation, SelectRecipe, SelectRowKind, SelectState,
};
pub use multi_select::{MultiSelect, MultiSelectOutcome, MultiSelectState};
pub use combobox::{
    Autocomplete, AutocompleteState, ComboMode, Combobox, ComboboxOutcome, ComboboxState,
    DEFAULT_COMBO_RECENT_LIMIT, SuggestionStatus,
};
pub use file_picker::{
    FILE_PICKER_FULLSCREEN_MAX_WIDTH, FILE_PICKER_OVERLAY_ID, FILE_PICKER_PREVIEW_MIN_HEIGHT,
    FileBreadcrumb, FileEntry, FileEntryKind, FileListingStatus, FilePicker, FilePickerMode,
    FilePickerOutcome, FilePickerPane, FilePickerPresentation, FilePickerState, FilePreview,
    FileSortKey,
};
pub use date_time_picker::{
    CivilDate, CivilDateRange, CivilDateTime, CivilTime, DATE_TIME_PICKER_FULLSCREEN_MAX_HEIGHT,
    DATE_TIME_PICKER_LIST_MAX_WIDTH, DATE_TIME_PICKER_OVERLAY_ID, DateDisplayFormat,
    DateTimePicker, DateTimePickerKind, DateTimePickerOutcome, DateTimePickerPresentation,
    DateTimePickerState, DateTimePickerView, DateTimeValidity, TimeDisplayFormat, WeekStart,
    guidance as date_time_picker_guidance,
};
pub use keybinding_recorder::{
    KEYBINDING_SEQUENCE_SEP, BindingLimit, KeybindingRecorder, KeybindingRecorderMode,
    KeybindingRecorderOutcome, KeybindingRecorderState, binding_from_recorder,
    default_reserved_chords, protocol_limitations,
};
pub use theme_picker::{
    BUILTIN_THEME_PRESETS, ThemePicker, ThemePickerOutcome, ThemePickerState, ThemePreset,
    system_from_preset_id, theme_from_preset_id,
};
pub use notification_center::{
    NOTIFICATION_CENTER_DEFAULT_CAPACITY, NOTIFICATION_CENTER_HINT, NOTIFICATION_CENTER_OVERLAY_ID,
    NotificationCenter, NotificationCenterOutcome, NotificationCenterSlots, NotificationCenterState,
    NotificationFilter, NotificationItem, NotificationRecipe, dismiss_notification_center_overlay,
    example_notifications, open_notification_center_drawer, open_notification_center_overlay,
};
pub use toast::{

    Anchor, Severity, TOAST_DEFAULT_H_MARGIN, TOAST_DEFAULT_MAX_VISIBLE, TOAST_DEFAULT_TTL,
    TOAST_DEFAULT_V_MARGIN, TOAST_STACK_GAP, Toast, ToastArchive, ToastArchiveReason, ToastKind,
    ToastLifetime, ToastOutcome, ToastPriority, ToastQueue, ToastSpec, ToastStack, ToastState,
};
pub use transcript::{
    Transcript, TranscriptAnchor, TranscriptBlock, TranscriptKind, TranscriptOutcome,
    TranscriptState,
};
pub use tree::{
    TREE_DEFAULT_OVERSCAN, Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState,
    filter_tree_with_ancestors,
};
pub use tree_table::{
    TreeTable, TreeTableHeaderRegion, TreeTableNavMode, TreeTableOutcome, TreeTableRow,
    TreeTableRowKind, TreeTableRowRegion, TreeTableState, default_tree_table_intent,
    filter_tree_table_with_ancestors,
};
pub use tree_navigation::{
    TREE_NAV_INDENT, TREE_NAV_MAX_INDENT_DEPTH, TREE_NAV_NARROW_MAX_WIDTH, TreeNavNode,
    TreeNavStatus, TreeNavigation, TreeNavigationOutcome, TreeNavigationState,
    example_docs_tree, example_project_tree, example_schema_tree, example_settings_tree,
};
pub use empty_state::{
    EMPTY_STATE_INLINE_MAX_HEIGHT, EMPTY_STATE_INLINE_MAX_WIDTH, EmptyAction, EmptyDensity,
    EmptyFocus, EmptyKind, EmptyState, EmptyStateOutcome, EmptyStateState,
    example_empty_logs, example_empty_permission, example_empty_projects, example_empty_search,
    example_empty_sessions, example_empty_table,
};
pub use error_state::{
    ERROR_STATE_COMPACT_MAX_HEIGHT, ERROR_STATE_INLINE_MAX_WIDTH, ErrorFocus, ErrorKind,
    ErrorRecipe, ErrorState, ErrorStateOutcome, ErrorStateState, ErrorView, Recovery,
    RecoveryAction, RetrySafety, example_error_conflict, example_error_crash,
    example_error_dialog, example_error_network, example_error_not_found,
    example_error_permission, example_error_unsupported, example_error_validation,
};
pub use connectivity::{
    ConnectivityFocus, ConnectivityOutcome, ConnectivityPhase, ConnectivityPresentation,
    OfflineBanner, OfflineCapability, OfflineChrome, OfflineSurface, QueuedConnectivityAction,
    ReconnectingState, example_auth_required, example_disconnected, example_reconnecting_agent,
    example_server_unavailable,
};
pub use loading_overlay::{
    BUSY_BOUNDARY_MAX_NEST, BusyBoundary, BusyBoundaryOutcome, BusyBoundaryState, BusyMode,
    BusyRoute, LOADING_OVERLAY_MIN_SHOW_MS, LOADING_OVERLAY_SHORT_OP_HINT_MS, LoadingOverlay,
    example_busy_blocking, example_busy_cancellable, example_busy_non_blocking,
    example_busy_optimistic, example_busy_stale,
};
pub use view_state::{Banner, LoadingView};
pub use viewport::Viewport;
pub use virtual_grid::{
    GridCell, GridCellRegion, GridColumn, GridColumnWidth, GridHeaderRegion, GridRow, VirtualGrid,
    VirtualGridOutcome, VirtualGridState,
};
pub use virtual_list::{
    VIRTUAL_LIST_BENCH_ROWS, VIRTUAL_LIST_DEFAULT_OVERSCAN, VirtualList, VirtualListDiagnostics,
    VirtualListFollow, VirtualListItem, VirtualListState, VirtualPageStatus, example_project_million,
    project_index_window,
};
pub use virtualizer::{
    ExtentPolicy, StickyRegion, VirtRange, VirtSlice, Virtualizer, Virtualizer2D,
    fixed_visible_range,
};

#[cfg(test)]
mod tests;
