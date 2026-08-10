// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral stories rendered through TermRock's public widget API.

use std::num::NonZeroU16;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use termrock::{
    interaction::{
        OverlayKind, OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, place_overlay,
    },
    scroll::DialogScroll,
    style::{ColorCapability, Density, DesignSystem, Role, RolePalette},
    widgets::{
        Action, ActionBar, ActionBarState, ActionLink, Anchor, AnsiParseOptions, AnsiText,
        ButtonGroup, ButtonGroupItem, ButtonGroupState,
        RangeSlider, RangeSliderState, SegmentedControl, SegmentedControlState, SegmentedItem,
        Slider, SliderBounds, SliderMark, SliderState, Toggle, ToggleGroup, ToggleGroupItem,
        ToggleGroupState, ToggleState, ToggleValue,
        AnsiTextMode, AnsiTextState, AvatarFace, AvatarGlyph, AvatarSize, BUILTIN_THEME_PRESETS,
        Backdrop, Badge, Banner,
        Alert, AlertState, AlertTone, BarDatum, BarSeries, Chart, ChartSeries, Gauge, HistBucket,
        Histogram, ScaleMode, VizGlyphSet, Button, ButtonState, Callout, CalloutTone,
        CellAlignment, Checkbox,
        CheckboxState, CheckboxValue, ChoiceDialog, ChoiceDialogState, CodeBlock, CodeBlockState,
        CodeHighlight,
        CodeHighlightKind, CodeWrap, Column, ColumnWidth,
        CommandEntry, CommandPalette, CommandPaletteState, example_command_catalog,
        QuickOpen, QuickOpenState, example_quick_open_files,
        example_quick_open_providers, example_quick_open_symbols, filter_quick_open_items,
        CompletionCandidate, CompletionMenu,
        CompletionMenuSize, CompletionMenuState, DataTable, DataTableState, DataTableToolbar,
        TreeTable, TreeTableRow, TreeTableState,
        DesignInspector, DesignInspectorFrame, DetailCapability, DetailRow, DetailTable,
        InspectorPanel,
        DetailTableState, Dialog, DialogRecipe, AlertDialog, AlertDialogState, AlertKind,
        AlertScope, AlertConfirmGates, DiffHunk, DiffKind, DiffLine, DiffMode, DiffReview,
        DiffReviewState, DiffReviewFileRow, DiffDecision, DiffReviewUnit, DiffViewState, DiffView,
        DiffWordKind, DiffWordSpan, Drawer, EmptyState, EmptyKind, EmptyAction, EmptyDensity,
        example_empty_table, example_empty_logs, example_empty_sessions, example_empty_projects,
        example_empty_search, example_empty_permission,
        ErrorView, ErrorState, ErrorKind, ErrorRecipe, Recovery, RecoveryAction, RetrySafety,
        example_error_network, example_error_validation, example_error_permission,
        example_error_crash, example_error_dialog, example_error_unsupported,
        Field, Fieldset, Form, FormState,
        FormWizard, FormWizardState, WizardStep, GridCell, GridColumn, GridRow, Heading, HeadingLevel, Hint,
        HighlightedText, HintBar, Identity, IdentityRole, ImageMeta, ImageProtocol, ImageSurface,
        InspectorField, JumpFilter, JumpOverlay, JumpOverlayState, JumpTarget,
        assign_jump_labels_from_semantics, generate_jump_labels, Kbd, KeyValueList,
        KeyValueListState, KvEntry, KeyValueTable, KeyValueTableState, KvtField, KvtMode,
        KvtValidation, KvLayout, KvStatus, Link, LinkState, List, MatchKind, MatchRange, MatchRanges,
        MatchTruncate, PresenceStatus,
        ListRow, ListState, ListDensity, ListSelectionMode, filter_list_rows,
        VirtualList, VirtualListState, VirtualListItem, VirtualListFollow, VirtualPageStatus,
        StickyRegion, filter_tree_with_ancestors,
        LoadingView, LoadingOverlay, BusyBoundary, BusyBoundaryState, BusyMode,
        example_busy_blocking, example_busy_cancellable, example_busy_non_blocking,
        example_busy_optimistic, example_busy_stale,
        ReconnectingState, OfflineBanner, OfflineSurface, ConnectivityPresentation,
        example_reconnecting_agent, example_auth_required, example_server_unavailable,
        example_disconnected,
        LogLevel, LogLine, LogPane,
        LogPaneState,
        LogStream, LogStreamState, EventStream, EventStreamState, StreamEvent, EventSeverity,
        Diagnostic, DiagnosticSeverity, DiagnosticView, DiagnosticState, DiagnosticRecipe,
        DiagnosticNote, SourceLabel, SourceRange, SpanStyle, SuggestedFix, CodeFrame, CodeFrameLine,
        TerminalOutput, TerminalOutputState, TerminalCommandMeta, TerminalLine, TerminalRunStatus,
        TerminalEnvEntry, TerminalOutputRecipe, TerminalPaintMode,
        HexViewer, HexViewerState, HexWindow, HexEndian, HexAsciiMode,
        FileTree, FileTreeState, FileTreeEntry, FileTreeKind, FileGitStatus,
        ProcessTable, ProcessTableState, ProcessRow, ProcessKey, ProcessStatus,
        ProcessViewMode, ProcessSignal, ProcessSignalConfirm,
        QueryEditor, QueryEditorState, QueryEditorMode, QueryFocus, QueryLanguage,
        QueryParameter, QueryResultSummary, QueryRunStatus,
        ResultGrid, ResultGridState, ResultColumn, ResultRow, ResultCell, ResultQueryStatus,
        ResultColumnStats, ResultRedaction, DataColumnWidth,
        SchemaBrowser, SchemaBrowserState, SchemaBrowserEntry, SchemaNodeKind, SchemaConnStatus,
        SchemaBrowserPresentation,
        SearchResults, SearchResultsState, SearchResultGroup, SearchResultItem, SearchResultKind,
        SearchResultsStatus,
        MetricsDashboard, MetricsDashboardState, MetricTile, MetricTileHealth, MetricAlert,
        MetricAlertSeverity, MetricViz,
        TraceWaterfall, TraceWaterfallState, TraceSpan, TraceSpanStatus,
        DependencyGraph, DependencyGraphState, DepNode, DepEdge, DepNodeKind, DepNodeStatus,
        DepEdgeKind, DependencyGraphView,
        MarkdownBlock, MarkdownBlockKind, MarkdownView, MarkdownViewState,
        Menu, MenuBar, MenuBarState, MenuItem, MenuNode, MenuState, DropdownMenu,
        DropdownMenuState, example_app_menus,
        MessageDialog, MeterSegment, ModeRibbon, ObjectInspector, ObjectInspectorState,
        Panel, PanelChrome, PermissionActionKind, PermissionPrompt, PermissionPromptState,
        PermissionProvenance, PermissionRequest, PermissionRisk, Picker, PickerState, PlanReview,
        PlanReviewState, Popover, PopoverState, Progress, ProgressKind, PromptComposer,
        PromptComposerState, QuestionFlow, QuestionFlowState,
        RadioGroup, RadioOption, RadioState, RowRole, SegmentedMeter, SeparatorLine, SessionPicker,
        SessionPickerState, Severity, Skeleton, SortDirection, Sparkline, SplitDirection, SplitPane,
        SplitPaneState, SplitRatio, StatusBar, StatusBarState, StatusSlot, Surface, SurfaceFill,
        SurfaceRecipe, Switch, SwitchState, Tab, TabStatus, Tabs, TabsActivation, TabsOrientation,
        TabsPresentation, TabsState, Table, TableRow, TableState, TaskRail, TextArea, TextAreaState,
        TextCursor,
        TextInput, TextInputState, TextWrap, PasswordInput, PasswordInputState, PasswordStrengthHint,
        RevealPolicy, NumberConstraints, NumberInput, NumberInputState, NumberKind,
        SearchFilterChip, SearchInput, SearchInputState, SearchStatus,
        PathExpect, PathFsStatus, PathInput, PathInputState, PathRisk, PathStyle,
        FieldToken, TokenField, TokenFieldState, TokenStatus,
        Select, SelectOption, SelectRecipe, SelectState,
        MultiSelect, MultiSelectState,
        Combobox, ComboboxState, SuggestionStatus,
        FileEntry, FileEntryKind, FilePicker, FilePickerMode, FilePickerState, FilePreview,
        FileSortKey,
        CivilDate, CivilDateRange, CivilTime, DateTimePicker, DateTimePickerKind,
        DateTimePickerState, TimeDisplayFormat,
        KeybindingRecorder, KeybindingRecorderState,
        NavItem, NavigationList, NavigationListState, Sidebar, SidebarPresentation, SidebarState,
        example_agent_workbench_nav, example_database_nav, example_settings_nav,
        TreeNavigation, TreeNavigationState, example_docs_tree, example_project_tree,
        example_schema_tree, example_settings_tree,
        BreadcrumbItem, BreadcrumbSeparator, BreadcrumbStatus, Breadcrumbs, BreadcrumbsState,
        PageTotal, Pagination, PaginationState,
        StepItem, StepStatus, Stepper, StepperNavPolicy, StepperOrientation, StepperPresentation,
        StepperState, example_onboarding_steps,
        HistoryEntry, HistoryKind, HistoryPicker, HistoryPickerState, HistoryRedaction,
        example_history_entries, filter_history_entries, history_redaction_secret,
        KeyboardHelp, KeyboardHelpState, example_help_entries,
        filter_help_entries,
        Tooltip, TooltipContent, TooltipState, TooltipVariant,
        FullscreenViewer, FullscreenViewerState, SemanticZoomBadge, SemanticZoomState,
        SourceContext, ScrollAnchor, ViewerContentKind,
        PreviewCard, PreviewCardState, PreviewCardContent, PreviewLoadState, PreviewResourceKind,
        example_command_preview, example_file_preview, example_session_preview,
        example_symbol_preview,
        ThemePicker, ThemePickerState, ThinkingBlock, Timeline, TimelineEvent, TimelineState,
        TimelineRecipe, TimelineStatus, CheckpointTimeline, CheckpointTimelineState, Toast,
        NotificationCenter,
        NotificationCenterState, NotificationRecipe,
        example_notifications, Spinner, SpinnerState, ActivityIndicator, ActivityPhase,
        ProgressSteps, ProgressStepsState, ProgressStepsMode, ProgressStepsPresentation,
        ProgressStep, ProgressStepStatus, example_build_pipeline, example_agent_plan_steps,
        StatusIndicator, SemanticStatus, example_status_catalog,
        TokenMeter, ToolCard, ToolStatus, Transcript, TranscriptBlock,
        TranscriptKind, TranscriptState, Tree, TreeNode, TreeNodeStatus, TreeState, Validation,
        Viewport, VirtualGrid, VirtualGridState, WorkbenchMode,
    },
};

use crate::interactors::{
    ChoiceDialogInteractor, CommandPaletteInteractor, DesignInspectorInteractor, FormInteractor,
    ListInteractor, LogPaneInteractor, PickerInteractor, PromptComposerInteractor,
    SplitPaneInteractor, StaticStory, StoryInteraction, TableInteractor, TabsInteractor,
    TextAreaInteractor, ThemePickerInteractor, ToastInteractor, TranscriptInteractor,
    TreeInteractor, VirtualGridInteractor,
};

type RenderFn = fn(&mut Frame<'_>, Rect, &DesignSystem);
type InteractorFactory = fn(RenderFn) -> Box<dyn StoryInteraction>;

pub(crate) const SPLIT_PANE_MIN: u16 = 12;
pub(crate) const SPLIT_PANE_MAX: u16 = 16;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Story {
    pub id: &'static str,
    pub title: &'static str,
    pub component: &'static str,
    pub description: &'static str,
    pub width: u16,
    pub height: u16,
    render: RenderFn,
    interactor: InteractorFactory,
}

impl Story {
    pub(crate) const fn new(
        id: &'static str,
        title: &'static str,
        component: &'static str,
        description: &'static str,
        width: u16,
        height: u16,
        render: RenderFn,
    ) -> Self {
        Self {
            id,
            title,
            component,
            description,
            width,
            height,
            render,
            interactor: static_interactor,
        }
    }
    const fn with_interactor(mut self, interactor: InteractorFactory) -> Self {
        self.interactor = interactor;
        self
    }
    pub(crate) fn render(self, frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
        (self.render)(frame, area, system);
    }
    pub(crate) fn make_interactor(&self) -> Box<dyn StoryInteraction> {
        (self.interactor)(self.render)
    }
}

fn static_interactor(render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(StaticStory {
        render_fn: render,
        theme: RolePalette::default(),
    })
}

fn tree_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TreeInteractor::new())
}

fn form_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(FormInteractor::new())
}

fn split_pane_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(SplitPaneInteractor::new())
}

fn choice_dialog_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ChoiceDialogInteractor::new())
}

fn list_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ListInteractor::new())
}

fn picker_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(PickerInteractor::new())
}

fn log_pane_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(LogPaneInteractor::new())
}

fn toast_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ToastInteractor::new())
}

fn text_area_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TextAreaInteractor::new())
}

fn tabs_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TabsInteractor::new())
}

fn table_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TableInteractor::new())
}

fn theme_picker_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ThemePickerInteractor::new())
}

fn command_palette_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(CommandPaletteInteractor::new())
}

fn design_inspector_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(DesignInspectorInteractor::new())
}

fn transcript_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TranscriptInteractor::new())
}

fn prompt_composer_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(PromptComposerInteractor::new())
}

fn virtual_grid_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(VirtualGridInteractor::new())
}

pub(crate) fn stories() -> Vec<Story> {
    vec![
        Story::new(
            "ui-context/frame",
            "UiContext frame",
            "UiContext",
            "Per-frame host: design + scene + focus + overlays + semantics + tick.",
            56,
            12,
            ui_context_frame_story,
        ),
        Story::new(
            "ui-context/nested",
            "UiContext nested register",
            "UiContext",
            "Nested components register into one scene/semantics via &mut UiContext.",
            48,
            10,
            ui_context_nested_story,
        ),
        Story::new(
            "design-system/presets",
            "DesignSystem presets",
            "DesignSystem",
            "Phosphor · Slate · Paper · ANSI · High Contrast ladder.",
            72,
            18,
            design_system_presets_story,
        ),
        Story::new(
            "design-system/no-color",
            "DesignSystem no-color",
            "DesignSystem",
            "Monochrome + ASCII glyphs; roles still carry meaning.",
            48,
            8,
            design_system_no_color_story,
        ),
        Story::new(
            "design-system/button-recipes",
            "Button recipes",
            "DesignSystem",
            "Primary/secondary/destructive × default/focused/disabled.",
            56,
            10,
            design_system_button_recipes_story,
        ),
        Story::new(
            "center/both",
            "Center both axes",
            "Center",
            "Child panel centered horizontally and vertically.",
            48,
            14,
            center_both_story,
        ),
        Story::new(
            "center/dialog",
            "Center dialog safe margin",
            "Center",
            "Dialog-style center with one-cell safe margin.",
            40,
            12,
            center_dialog_story,
        ),
        Story::new(
            "center/horizontal",
            "Center horizontal only",
            "Center",
            "Horizontal center; full height strip.",
            40,
            8,
            center_horizontal_story,
        ),
        Story::new(
            "center/max",
            "Center max-width",
            "Center",
            "Preferred width capped by max_width.",
            48,
            10,
            center_max_story,
        ),
        Story::new(
            "center/tiny",
            "Center tiny terminal",
            "Center",
            "No underflow when outer is smaller than preferred.",
            12,
            4,
            center_tiny_story,
        ),
        Story::new(
            "center/vertical",
            "Center vertical only",
            "Center",
            "Vertical axis: width fills; height preferred.",
            40,
            10,
            center_vertical_story,
        ),
        Story::new(
            "center/onboarding",
            "Center onboarding recipe",
            "Center",
            "Onboarding hero with max-width and safe margin.",
            56,
            14,
            center_onboarding_story,
        ),
        Story::new(
            "center/failure",
            "Center failure recipe",
            "Center",
            "Failure/error panel placement with caps.",
            48,
            12,
            center_failure_story,
        ),
        Story::new(
            "grid/columns",
            "Grid columns",
            "Grid",
            "Equal fr columns with gap; auto-flow cards.",
            56,
            12,
            grid_columns_story,
        ),
        Story::new(
            "grid/span",
            "Grid span",
            "Grid",
            "Header spans full width; detail cells below.",
            48,
            10,
            grid_span_story,
        ),
        Story::new(
            "grid/dashboard",
            "Grid dashboard template",
            "Grid",
            "Responsive card grid (up to 3 columns).",
            72,
            14,
            grid_dashboard_story,
        ),
        Story::new(
            "grid/form",
            "Grid form template",
            "Grid",
            "form_grid_template: 2-col wide / 1-col narrow.",
            70,
            12,
            grid_form_story,
        ),
        Story::new(
            "grid/narrow",
            "Grid narrow",
            "Grid",
            "Single-column collapse under narrow width.",
            28,
            10,
            grid_narrow_story,
        ),
        Story::new(
            "grid/settings",
            "Grid settings template",
            "Grid",
            "Label + value columns; host-measured label track.",
            48,
            8,
            grid_settings_story,
        ),
        Story::new(
            "grid/overflow",
            "Grid overflow clip-tail",
            "Grid",
            "ClipTail keeps head tracks; tail collapses.",
            20,
            6,
            grid_overflow_story,
        ),
        Story::new(
            "grid/nav",
            "Grid spatial neighbor",
            "Grid",
            "Focus index 0 → right/down neighbor (debug labels).",
            36,
            8,
            grid_nav_story,
        ),
        Story::new(
            "stack/vertical",
            "Stack vertical",
            "Stack",
            "Fixed + weight + fixed vertical packing with gap.",
            40,
            12,
            stack_vertical_story,
        ),
        Story::new(
            "stack/inline",
            "Inline horizontal",
            "Inline",
            "Equal-weight horizontal columns.",
            48,
            6,
            stack_inline_story,
        ),
        Story::new(
            "stack/wrap",
            "Inline wrap",
            "Inline",
            "Wrapping chips when children exceed width.",
            24,
            6,
            stack_wrap_story,
        ),
        Story::new(
            "stack/responsive",
            "Stack responsive direction",
            "Stack",
            "direction_for_width: stack narrow, inline wide.",
            60,
            8,
            stack_responsive_story,
        ),
        Story::new(
            "stack/narrow",
            "Stack narrow overflow",
            "Stack",
            "Fixed children shrink from end when area too small.",
            16,
            5,
            stack_narrow_story,
        ),
        Story::new(
            "stack/overflow-clip",
            "Stack overflow clip-tail",
            "Stack",
            "ClipTail keeps head children; tail collapses to zero.",
            16,
            5,
            stack_overflow_clip_story,
        ),
        Story::new(
            "stack/justify",
            "Inline space-around",
            "Inline",
            "Justify::SpaceAround distributes free main-axis cells.",
            40,
            4,
            stack_justify_story,
        ),
        Story::new(
            "section/quiet",
            "Section quiet",
            "Section",
            "Quiet editorial header + description + body.",
            48,
            8,
            section_quiet_story,
        ),
        Story::new(
            "section/emphasized",
            "Section emphasized",
            "Section",
            "Strong title + divider under header.",
            48,
            8,
            section_emphasized_story,
        ),
        Story::new(
            "section/collapsible",
            "Section collapsible",
            "Section",
            "Disclosure header; body collapses.",
            40,
            8,
            section_collapsible_story,
        ),
        Story::new(
            "section/actions",
            "Section header actions",
            "Section",
            "Status + actions; actions drop under narrow width.",
            48,
            7,
            section_actions_story,
        ),
        Story::new(
            "section/nested",
            "Section nested",
            "Section",
            "Depth indent for nested groups.",
            44,
            10,
            section_nested_story,
        ),
        Story::new(
            "section/narrow",
            "Section narrow",
            "Section",
            "Description and actions contract.",
            18,
            6,
            section_narrow_story,
        ),
        Story::new(
            "panel/focused",
            "Focused panel",
            "Panel",
            "A semantically focused bordered panel.",
            48,
            7,
            panel,
        ),
        Story::new(
            "panel/variants",
            "Panel variants",
            "Panel",
            "Bordered · quiet · divider · interactive · selected ladder.",
            56,
            16,
            panel_variants_story,
        ),
        Story::new(
            "panel/empty",
            "Panel empty body",
            "Panel",
            "Built-in empty body mode with non-color glyph.",
            36,
            8,
            panel_empty_story,
        ),
        Story::new(
            "panel/collapsible",
            "Panel collapsible",
            "Panel",
            "Collapsible header with disclosure; focus owns activate/toggle.",
            40,
            8,
            panel_collapsible_story,
        ),
        Story::new(
            "card/basic",
            "Card basic",
            "Card",
            "Raised card with title, description, body, footer.",
            36,
            9,
            card_basic_story,
        ),
        Story::new(
            "card/tool",
            "Card tool example",
            "Card",
            "Agent tool-style card (status leading + badge + summary).",
            42,
            7,
            card_tool_story,
        ),
        Story::new(
            "card/dashboard",
            "Card dashboard tiles",
            "Card",
            "Dashboard metric cards in a compact grid.",
            56,
            12,
            card_dashboard_story,
        ),
        Story::new(
            "panel/loading",
            "Panel loading body",
            "Panel",
            "Built-in loading body mode with detail copy.",
            36,
            8,
            panel_loading_story,
        ),
        Story::new(
            "panel/error",
            "Panel error body",
            "Panel",
            "Built-in error body mode with non-color title.",
            36,
            8,
            panel_error_story,
        ),
        Story::new(
            "panel/actions",
            "Panel header actions",
            "Panel",
            "Badge + header action band; actions drop under narrow width.",
            48,
            8,
            panel_actions_story,
        ),
        Story::new(
            "action-bar/basic",
            "Action bar",
            "ActionBar",
            "Stable-ID caller-defined actions.",
            48,
            2,
            action_bar,
        ),
        Story::new(
            "button-group/dialog",
            "ButtonGroup dialog",
            "ButtonGroup",
            "Cancel + Save primary + Delete; Enter submits Save.",
            48,
            3,
            button_group_dialog_story,
        ),
        Story::new(
            "button-group/connected",
            "ButtonGroup connected",
            "ButtonGroup",
            "Connected recipe for segmented actions.",
            36,
            3,
            button_group_connected_story,
        ),
        Story::new(
            "button-group/overflow",
            "ButtonGroup overflow",
            "ButtonGroup",
            "Secondary actions collapse into overflow at narrow width.",
            22,
            3,
            button_group_overflow_story,
        ),
        Story::new(
            "button-group/loading",
            "ButtonGroup loading",
            "ButtonGroup",
            "Default action loading; siblings remain.",
            40,
            3,
            button_group_loading_story,
        ),
        Story::new(
            "toggle/pressed",
            "Toggle pressed",
            "Toggle",
            "Single sticky toggle: unpressed vs pressed brackets.",
            36,
            3,
            toggle_pressed_story,
        ),
        Story::new(
            "toggle/icon",
            "Toggle icon-only",
            "Toggle",
            "Icon-only with mandatory accessible label.",
            24,
            3,
            toggle_icon_story,
        ),
        Story::new(
            "toggle/indeterminate",
            "Toggle indeterminate",
            "Toggle",
            "Mixed selection mark [~B].",
            24,
            3,
            toggle_indeterminate_story,
        ),
        Story::new(
            "toggle-group/format",
            "ToggleGroup multi format",
            "ToggleGroup",
            "Bold/Italic/Underline multi-select toolbar.",
            40,
            3,
            toggle_group_format_story,
        ),
        Story::new(
            "toggle-group/align",
            "ToggleGroup single align",
            "ToggleGroup",
            "Connected single-select L|C|R.",
            28,
            3,
            toggle_group_align_story,
        ),
        Story::new(
            "toggle-group/overflow",
            "ToggleGroup overflow",
            "ToggleGroup",
            "Low-priority toggles collapse to …",
            18,
            3,
            toggle_group_overflow_story,
        ),
        Story::new(
            "accordion/section",
            "Accordion section",
            "Accordion",
            "Multi-open section recipe with roving cursor.",
            44,
            14,
            accordion_section_story,
        ),
        Story::new(
            "accordion/settings",
            "Accordion settings",
            "Accordion",
            "Single-open settings groups.",
            44,
            12,
            accordion_settings_story,
        ),
        Story::new(
            "accordion/logs",
            "Accordion logs",
            "Accordion",
            "Multi-open log/tool streams; keep-mounted policy.",
            48,
            14,
            accordion_logs_story,
        ),
        Story::new(
            "accordion/faq",
            "Accordion FAQ",
            "Accordion",
            "Single-open FAQ help recipe.",
            48,
            12,
            accordion_faq_story,
        ),
        Story::new(
            "accordion/narrow",
            "Accordion narrow",
            "Accordion",
            "Triggers truncate; layout survives narrow width.",
            18,
            10,
            accordion_narrow_story,
        ),
        Story::new(
            "accordion/scroll-body",
            "Accordion scroll body",
            "Accordion",
            "Capped content height for nested scroll hosts.",
            44,
            12,
            accordion_scroll_body_story,
        ),
        Story::new(
            "collapsible/inline",
            "Collapsible inline",
            "Collapsible",
            "Compact disclosure; open body for optional detail.",
            40,
            6,
            collapsible_inline_story,
        ),
        Story::new(
            "collapsible/section",
            "Collapsible section",
            "Collapsible",
            "Section-style trigger with open-state rule fill.",
            44,
            7,
            collapsible_section_story,
        ),
        Story::new(
            "collapsible/nested",
            "Collapsible nested",
            "Collapsible",
            "Depth indent for nested disclosures.",
            44,
            10,
            collapsible_nested_story,
        ),
        Story::new(
            "collapsible/disabled",
            "Collapsible disabled",
            "Collapsible",
            "Disabled trigger ignores activate; · marker.",
            36,
            3,
            collapsible_disabled_story,
        ),
        Story::new(
            "collapsible/ascii",
            "Collapsible ascii",
            "Collapsible",
            "ASCII disclosure glyphs (v/>) without Unicode.",
            36,
            5,
            collapsible_ascii_story,
        ),
        Story::new(
            "collapsible/narrow",
            "Collapsible narrow",
            "Collapsible",
            "Trigger truncates; body still opens.",
            18,
            5,
            collapsible_narrow_story,
        ),
        Story::new(
            "toolbar/basic",
            "Toolbar basic",
            "Toolbar",
            "Roving-focus strip with actions, separator, toggle, hints.",
            64,
            1,
            toolbar_basic_story,
        ),
        Story::new(
            "toolbar/overflow",
            "Toolbar overflow",
            "Toolbar",
            "Low-priority items move to overflow chip.",
            28,
            1,
            toolbar_overflow_story,
        ),
        Story::new(
            "toolbar/vertical",
            "Toolbar vertical",
            "Toolbar",
            "Compact vertical orientation.",
            12,
            8,
            toolbar_vertical_story,
        ),
        Story::new(
            "toolbar/compact",
            "Toolbar compact icons",
            "Toolbar",
            "Compact variant prefers icons.",
            40,
            1,
            toolbar_compact_story,
        ),
        Story::new(
            "sidebar/settings",
            "Sidebar settings",
            "Sidebar",
            "Settings navigation with sections and badges.",
            28,
            14,
            sidebar_settings_story,
        ),
        Story::new(
            "sidebar/database",
            "Sidebar database",
            "Sidebar",
            "Database explorer hierarchy projection.",
            28,
            16,
            sidebar_database_story,
        ),
        Story::new(
            "sidebar/agent",
            "Sidebar agent workbench",
            "Sidebar",
            "Agent workbench nav with status and separator.",
            24,
            12,
            sidebar_agent_story,
        ),
        Story::new(
            "sidebar/rail",
            "Sidebar rail",
            "Sidebar",
            "Compact rail presentation.",
            8,
            12,
            sidebar_rail_story,
        ),
        Story::new(
            "navigation-list/basic",
            "NavigationList",
            "NavigationList",
            "Route distinct from focus; filterable list.",
            28,
            12,
            navigation_list_basic_story,
        ),
        Story::new(
            "pagination/full",
            "Pagination full",
            "Pagination",
            "Full control with page numbers and summary.",
            64,
            1,
            pagination_full_story,
        ),
        Story::new(
            "pagination/unknown",
            "Pagination unknown total",
            "Pagination",
            "Unknown total — next allowed without last.",
            40,
            1,
            pagination_unknown_story,
        ),
        Story::new(
            "pagination/loading",
            "Pagination loading",
            "Pagination",
            "Loading disables nav; compact width.",
            36,
            1,
            pagination_loading_story,
        ),
        Story::new(
            "pagination/minimal",
            "Pagination minimal",
            "Pagination",
            "Narrow minimal prev/next + page label.",
            18,
            1,
            pagination_minimal_story,
        ),
        Story::new(
            "pagination/jump",
            "Pagination jump",
            "Pagination",
            "Direct page entry draft active.",
            48,
            1,
            pagination_jump_story,
        ),
        Story::new(
            "stepper/horizontal",
            "Stepper horizontal",
            "Stepper",
            "Expanded horizontal multi-step progress.",
            72,
            2,
            stepper_horizontal_story,
        ),
        Story::new(
            "stepper/vertical",
            "Stepper vertical",
            "Stepper",
            "Vertical steps with descriptions.",
            28,
            14,
            stepper_vertical_story,
        ),
        Story::new(
            "stepper/error",
            "Stepper error state",
            "Stepper",
            "Error + complete marks without relying on color alone.",
            64,
            2,
            stepper_error_story,
        ),
        Story::new(
            "stepper/numeric",
            "Stepper numeric",
            "Stepper",
            "Narrow numeric current/total contraction.",
            20,
            1,
            stepper_numeric_story,
        ),
        Story::new(
            "stepper/menu",
            "Stepper menu",
            "Stepper",
            "Menu presentation open with step list.",
            24,
            8,
            stepper_menu_story,
        ),
        Story::new(
            "stepper/ascii",
            "Stepper ASCII",
            "Stepper",
            "ASCII marks + colorless roles.",
            56,
            2,
            stepper_ascii_story,
        ),
        Story::new(
            "history-picker/basic",
            "HistoryPicker basic",
            "HistoryPicker",
            "Recent history with pins, groups, and preview.",
            64,
            16,
            history_picker_basic,
        ),
        Story::new(
            "history-picker/search",
            "HistoryPicker search",
            "HistoryPicker",
            "Filter history entries.",
            56,
            12,
            history_picker_search,
        ),
        Story::new(
            "history-picker/redacted",
            "HistoryPicker redacted",
            "HistoryPicker",
            "Sensitive values masked in list.",
            56,
            12,
            history_picker_redacted,
        ),
        Story::new(
            "history-picker/draft",
            "HistoryPicker draft preserved",
            "HistoryPicker",
            "Draft stash banner while browsing.",
            56,
            12,
            history_picker_draft,
        ),
        Story::new(
            "history-picker/empty",
            "HistoryPicker empty",
            "HistoryPicker",
            "Empty history state.",
            40,
            8,
            history_picker_empty,
        ),
        Story::new(
            "history-picker/ascii",
            "HistoryPicker ASCII",
            "HistoryPicker",
            "ASCII chrome and colorless paint.",
            48,
            12,
            history_picker_ascii,
        ),
        Story::new(
            "keyboard-help/footer",
            "KeyboardHelp footer",
            "KeyboardHelp",
            "Compact footer generated from live keymap.",
            72,
            1,
            keyboard_help_footer,
        ),
        Story::new(
            "keyboard-help/modal",
            "KeyboardHelp modal",
            "KeyboardHelp",
            "Categorized searchable modal help.",
            64,
            16,
            keyboard_help_modal,
        ),
        Story::new(
            "keyboard-help/search",
            "KeyboardHelp search",
            "KeyboardHelp",
            "Modal filtered to save bindings.",
            56,
            12,
            keyboard_help_search,
        ),
        Story::new(
            "keyboard-help/tiny",
            "KeyboardHelp tiny",
            "KeyboardHelp",
            "Tiny-terminal priority contraction.",
            22,
            1,
            keyboard_help_tiny,
        ),
        Story::new(
            "keyboard-help/ascii",
            "KeyboardHelp ASCII",
            "KeyboardHelp",
            "ASCII footer + colorless roles.",
            56,
            1,
            keyboard_help_ascii,
        ),
        Story::new(
            "tooltip/plain",
            "Tooltip plain",
            "Tooltip",
            "Plain delayed help (forced visible for snapshot).",
            28,
            1,
            tooltip_plain_story,
        ),
        Story::new(
            "tooltip/shortcut",
            "Tooltip shortcut",
            "Tooltip",
            "Body + shortcut chord variant.",
            32,
            1,
            tooltip_shortcut_story,
        ),
        Story::new(
            "tooltip/rich",
            "Tooltip rich",
            "Tooltip",
            "Title + body compact rich variant.",
            36,
            2,
            tooltip_rich_story,
        ),
        Story::new(
            "tooltip/ascii",
            "Tooltip ASCII",
            "Tooltip",
            "ASCII / colorless tooltip paint.",
            28,
            2,
            tooltip_ascii_story,
        ),
        Story::new(
            "dropdown-menu/basic",
            "DropdownMenu basic",
            "DropdownMenu",
            "Trigger-opened cascade with shortcuts and checkbox.",
            40,
            14,
            dropdown_menu_basic_story,
        ),
        Story::new(
            "dropdown-menu/nested",
            "DropdownMenu nested",
            "DropdownMenu",
            "Submenu open (Export › Image).",
            56,
            14,
            dropdown_menu_nested_story,
        ),
        Story::new(
            "dropdown-menu/kinds",
            "DropdownMenu item kinds",
            "DropdownMenu",
            "Checkbox, radio, separator, label, loading, destructive.",
            44,
            16,
            dropdown_menu_kinds_story,
        ),
        Story::new(
            "context-menu/basic",
            "ContextMenu basic",
            "ContextMenu",
            "Pointer-origin context menu (AtOrigin placement).",
            36,
            12,
            context_menu_basic_story,
        ),
        Story::new(
            "context-menu/nested",
            "ContextMenu nested",
            "ContextMenu",
            "Nested context cascade with parent dismiss.",
            52,
            14,
            context_menu_nested_story,
        ),
        Story::new(
            "menu-bar/basic",
            "MenuBar basic",
            "MenuBar",
            "Top-level menus closed; single Tab-stop bar.",
            64,
            3,
            menu_bar_basic_story,
        ),
        Story::new(
            "menu-bar/open",
            "MenuBar open cascade",
            "MenuBar",
            "File menu open with nested Export path ready.",
            72,
            14,
            menu_bar_open_story,
        ),
        Story::new(
            "menu-bar/nested",
            "MenuBar nested dismiss",
            "MenuBar",
            "Submenu open; Esc peels one layer.",
            72,
            14,
            menu_bar_nested_story,
        ),
        Story::new(
            "menu-bar/mnemonic",
            "MenuBar mnemonic mode",
            "MenuBar",
            "Sticky mnemonic arm (host F10 maps here).",
            64,
            3,
            menu_bar_mnemonic_story,
        ),
        Story::new(
            "menu-bar/narrow",
            "MenuBar narrow palette",
            "MenuBar",
            "Narrow chip prefers CommandPalette.",
            28,
            2,
            menu_bar_narrow_story,
        ),
        Story::new(
            "menu-bar/unicode",
            "MenuBar Unicode",
            "MenuBar",
            "CJK labels and mnemonics.",
            48,
            10,
            menu_bar_unicode_story,
        ),
        Story::new(
            "menu-bar/ascii",
            "MenuBar ASCII",
            "MenuBar",
            "ASCII check/radio/mnemonic glyphs.",
            64,
            12,
            menu_bar_ascii_story,
        ),
        Story::new(
            "breadcrumbs/path",
            "Breadcrumbs path",
            "Breadcrumbs",
            "Full path trail with current segment underlined.",
            48,
            1,
            breadcrumbs_path_story,
        ),
        Story::new(
            "breadcrumbs/collapsed",
            "Breadcrumbs collapsed",
            "Breadcrumbs",
            "Narrow collapse keeps root and current; middle ellipsis.",
            28,
            1,
            breadcrumbs_collapsed_story,
        ),
        Story::new(
            "breadcrumbs/editable",
            "Breadcrumbs editable",
            "Breadcrumbs",
            "Editable path mode draft.",
            48,
            1,
            breadcrumbs_editable_story,
        ),
        Story::new(
            "breadcrumbs/status",
            "Breadcrumbs status",
            "Breadcrumbs",
            "ASCII status marks on segments.",
            40,
            1,
            breadcrumbs_status_story,
        ),
        Story::new(
            "breadcrumbs/schema",
            "Breadcrumbs schema",
            "Breadcrumbs",
            "Schema object path trail (master-detail).",
            48,
            1,
            breadcrumbs_schema_story,
        ),
        Story::new(
            "tree-navigation/project",
            "TreeNavigation project",
            "TreeNavigation",
            "Project explorer with lazy branch and dirty leaf.",
            36,
            14,
            tree_navigation_project_story,
        ),
        Story::new(
            "tree-navigation/schema",
            "TreeNavigation schema",
            "TreeNavigation",
            "Database schema browser with loading branch.",
            36,
            12,
            tree_navigation_schema_story,
        ),
        Story::new(
            "tree-navigation/settings",
            "TreeNavigation settings",
            "TreeNavigation",
            "Settings hierarchy with badges and dirty state.",
            36,
            12,
            tree_navigation_settings_story,
        ),
        Story::new(
            "tree-navigation/docs",
            "TreeNavigation docs",
            "TreeNavigation",
            "Documentation nav with error leaf.",
            36,
            12,
            tree_navigation_docs_story,
        ),
        Story::new(
            "tree-navigation/narrow",
            "TreeNavigation narrow",
            "TreeNavigation",
            "Narrow-terminal compact indent and labels.",
            14,
            12,
            tree_navigation_narrow_story,
        ),
        Story::new(
            "tabs/status",
            "Tabs",
            "Tabs",
            "Tabs with styled per-item glyphs and state.",
            52,
            2,
            tabs,
        )
        .with_interactor(tabs_interactor),
        Story::new(
            "tabs/overflow",
            "Tabs overflow",
            "Tabs",
            "Overflow presentation under width pressure.",
            28,
            2,
            tabs_overflow_story,
        ),
        Story::new(
            "tabs/vertical",
            "Tabs vertical",
            "Tabs",
            "Vertical orientation stack.",
            16,
            6,
            tabs_vertical_story,
        ),
        Story::new(
            "tabs/manual",
            "Tabs manual",
            "Tabs",
            "Manual activation: focus ≠ selection until Enter.",
            48,
            2,
            tabs_manual_story,
        ),
        Story::new(
            "tabs/closable",
            "Tabs closable",
            "Tabs",
            "Closable tabs with close affordance.",
            48,
            2,
            tabs_closable_story,
        ),
        Story::new(
            "hint-bar/wrapped",
            "Hint bar",
            "HintBar",
            "Prioritized caller-defined hints.",
            42,
            2,
            hint_bar,
        ),
        Story::new(
            "list/selection",
            "List",
            "List",
            "Stable-ID rows with checks and aligned metadata.",
            42,
            6,
            list,
        )
        .with_interactor(list_interactor),
        Story::new(
            "tree/navigation",
            "Tree navigation",
            "Tree",
            "Stable-ID hierarchy with checks, metadata, disclosure, and status.",
            42,
            7,
            tree,
        )
        .with_interactor(tree_interactor),
        Story::new(
            "tree/empty",
            "Empty tree",
            "Tree",
            "Empty-message projection when the flattened projection is empty.",
            32,
            4,
            tree_empty,
        ),
        Story::new(
            "tree/loading-error",
            "Tree loading and error",
            "Tree",
            "Loading muted and error danger status nodes.",
            40,
            6,
            tree_loading_error,
        ),
        Story::new(
            "tree/ascii",
            "ASCII tree glyphs",
            "Tree",
            "ASCII disclosure and selection gutter fallbacks.",
            36,
            6,
            tree_ascii,
        ),
        Story::new(
            "tree/composed",
            "Composed tree anatomy",
            "Tree",
            "Leading, secondary, badge, and shortcut on hierarchical rows.",
            48,
            6,
            tree_composed,
        ),
        Story::new(
            "tree/tiny",
            "Tiny tree",
            "Tree",
            "Disclosure + primary survive extreme width.",
            12,
            5,
            tree_tiny,
        ),
        Story::new(
            "tree/deep",
            "Deep indent tree",
            "Tree",
            "Deep hierarchy with density indent and clamp.",
            44,
            8,
            tree_deep,
        ),
        Story::new(
            "tree/lazy",
            "Tree lazy children",
            "Tree",
            "Lazy/unloaded branch, loading child, error child.",
            44,
            8,
            tree_lazy_story,
        ),
        Story::new(
            "tree/filter",
            "Tree filter ancestors",
            "Tree",
            "Filter keeps matching nodes and ancestors.",
            44,
            8,
            tree_filter_story,
        ),
        Story::new(
            "tree/actions",
            "Tree context actions",
            "Tree",
            "Context actions + typeahead-ready labels.",
            48,
            6,
            tree_actions_story,
        ),
        Story::new(
            "progress/determinate",
            "Progress",
            "Progress",
            "Caller-ticked determinate and indeterminate progress.",
            42,
            2,
            progress,
        ),
        Story::new(
            "progress/detailed",
            "ProgressBar detailed",
            "Progress",
            "Transfer units, rate, and ETA meta on one row.",
            56,
            1,
            progress_detailed_story,
        ),
        Story::new(
            "progress/multi-line",
            "ProgressBar multi-line",
            "Progress",
            "Title, track, and phase/rate meta stack.",
            48,
            3,
            progress_multiline_story,
        ),
        Story::new(
            "progress/failed",
            "ProgressBar failed",
            "Progress",
            "Failed status with danger fill role.",
            40,
            1,
            progress_failed_story,
        ),
        Story::new(
            "progress-steps/pipeline",
            "ProgressSteps pipeline",
            "ProgressSteps",
            "CI-style pipeline with running compile phase.",
            48,
            12,
            progress_steps_pipeline_story,
        ),
        Story::new(
            "progress-steps/agent",
            "ProgressSteps agent plan",
            "ProgressSteps",
            "Agent plan with warning, failed+retry, cancelled.",
            48,
            12,
            progress_steps_agent_story,
        ),
        Story::new(
            "progress-steps/summary",
            "ProgressSteps narrow summary",
            "ProgressSteps",
            "Contracted n/total · verb summary for narrow terminals.",
            20,
            1,
            progress_steps_summary_story,
        ),
        Story::new(
            "progress-steps/interactive",
            "ProgressSteps interactive",
            "ProgressSteps",
            "Interactive mode with cursor and retry affordance.",
            48,
            12,
            progress_steps_interactive_story,
        ),
        Story::new(
            "progress/narrow",
            "Narrow progress",
            "Progress",
            "Percentage elision and custom ASCII frames in fourteen columns.",
            14,
            2,
            progress_narrow,
        ),
        Story::new(
            "progress/unicode",
            "Unicode progress labels",
            "Progress",
            "Wide CJK and emoji labels clipped on grapheme boundaries.",
            34,
            2,
            progress_unicode_story,
        ),
        Story::new(
            "log-pane/follow",
            "Following log pane",
            "LogPane",
            "Tail-following output; scroll up to freeze and End to resume.",
            52,
            8,
            log_pane,
        )
        .with_interactor(log_pane_interactor),
        Story::new(
            "log-pane/scrolled",
            "Frozen log scrollback",
            "LogPane",
            "Scrolled-back distance plus wide CJK and emoji output.",
            52,
            8,
            log_pane_scrolled,
        ),
        Story::new(
            "form/responsive",
            "Responsive form",
            "Form",
            "Sections, validation, disabled state, and stable-ID focus.",
            68,
            12,
            form,
        )
        .with_interactor(form_interactor),
        Story::new(
            "form/compact",
            "Form compact",
            "Form",
            "Compact layout recipe.",
            44,
            12,
            form_compact_story,
        ),
        Story::new(
            "form/validation",
            "Form validation",
            "Form",
            "Error summary + first invalid focus target.",
            44,
            14,
            form_validation_story,
        ),
        Story::new(
            "split-pane/horizontal",
            "Horizontal split pane",
            "SplitPane",
            "Bounded resizable panes with focus, drag, and collapse.",
            68,
            10,
            split_pane,
        )
        .with_interactor(split_pane_interactor),
        Story::new(
            "resizable-panel-group/workbench",
            "ResizablePanelGroup workbench",
            "ResizablePanelGroup",
            "Sidebar | main | inspector with resize handles.",
            80,
            16,
            resizable_workbench_story,
        ),
        Story::new(
            "resizable-panel-group/dashboard",
            "ResizablePanelGroup dashboard",
            "ResizablePanelGroup",
            "Main | log horizontal dashboard split.",
            72,
            14,
            resizable_dashboard_story,
        ),
        Story::new(
            "resizable-panel-group/drawers",
            "ResizablePanelGroup drawers",
            "ResizablePanelGroup",
            "Narrow workbench: side docks flagged as drawers.",
            48,
            12,
            resizable_drawers_story,
        ),
        Story::new(
            "picker/basic",
            "Filterable picker",
            "Picker",
            "Caller-filtered rows with stable selection and semantic activation.",
            42,
            7,
            picker_basic,
        )
        .with_interactor(picker_interactor),
        Story::new(
            "picker/empty",
            "Empty picker",
            "Picker",
            "Product-neutral empty-result cue.",
            30,
            4,
            picker_empty,
        ),
        Story::new(
            "picker/narrow-unicode",
            "Narrow Unicode picker",
            "Picker",
            "Wide and combining labels clipped in a narrow result list.",
            22,
            5,
            picker_narrow_unicode,
        ),
        Story::new(
            "detail-table/basic",
            "Detail table",
            "DetailTable",
            "Neutral label/value rows with capabilities.",
            54,
            5,
            detail_table,
        ),
        Story::new(
            "object-inspector/flat",
            "Object inspector flat",
            "ObjectInspector",
            "Flat key/value fields with list-local cursor gutter.",
            48,
            6,
            object_inspector_flat,
        ),
        Story::new(
            "object-inspector/nested",
            "Object inspector nested",
            "ObjectInspector",
            "Nested depth pad + cursor on a child field.",
            48,
            8,
            object_inspector_nested,
        ),
        Story::new(
            "object-inspector/empty",
            "Object inspector empty",
            "ObjectInspector",
            "Empty-object non-color mark.",
            32,
            3,
            object_inspector_empty,
        ),
        Story::new(
            "object-inspector/narrow",
            "Object inspector narrow",
            "ObjectInspector",
            "Narrow key=value geometry (22 cols).",
            22,
            5,
            object_inspector_flat,
        ),
        Story::new(
            "object-inspector/ascii",
            "Object inspector ASCII",
            "ObjectInspector",
            "ASCII cursor and empty glyphs.",
            40,
            5,
            object_inspector_ascii,
        ),
        Story::new(
            "object-inspector/json",
            "Object inspector JSON",
            "ObjectInspector",
            "Typed JSON-like tree with secret redaction.",
            56,
            12,
            object_inspector_json,
        ),
        Story::new(
            "object-inspector/compare",
            "Object inspector compare",
            "ObjectInspector",
            "Compare/diff mode for local vs remote values.",
            56,
            8,
            object_inspector_compare,
        ),
        Story::new(
            "object-inspector/lazy",
            "Object inspector lazy",
            "ObjectInspector",
            "Lazy container expansion for huge trees.",
            48,
            6,
            object_inspector_lazy,
        ),
        Story::new(
            "object-inspector/fullscreen",
            "Object inspector fullscreen",
            "ObjectInspector",
            "Fullscreen presentation chrome for deep inspection.",
            64,
            10,
            object_inspector_fullscreen,
        ),
        Story::new(
            "log-stream/follow",
            "Log stream follow",
            "LogStream",
            "Tail-follow chip with timestamp/source/level recipe.",
            72,
            10,
            log_stream_follow,
        ),
        Story::new(
            "log-stream/structured",
            "Log stream detailed",
            "LogStream",
            "Detailed recipe + pin + multi-level projection.",
            72,
            10,
            log_stream_structured,
        ),
        Story::new(
            "log-stream/filter",
            "Log stream filter",
            "LogStream",
            "Search + level floor chrome on follow chip.",
            72,
            10,
            log_stream_filter,
        ),
        Story::new(
            "log-stream/dropped",
            "Log stream dropped",
            "LogStream",
            "Bounded history drop + reconnect banner + batch.",
            72,
            10,
            log_stream_dropped,
        ),
        Story::new(
            "log-stream/empty",
            "Log stream empty",
            "LogStream",
            "Empty-log non-color mark.",
            32,
            3,
            log_stream_empty,
        ),
        Story::new(
            "log-stream/narrow",
            "Log stream narrow",
            "LogStream",
            "Narrow log geometry (22 cols).",
            22,
            6,
            log_stream_follow,
        ),
        Story::new(
            "log-stream/ascii",
            "Log stream ASCII",
            "LogStream",
            "ASCII level letters and follow chip.",
            40,
            6,
            log_stream_ascii,
        ),
        Story::new(
            "event-stream/basic",
            "EventStream",
            "EventStream",
            "Structured k8s/agent events with severity and source.",
            80,
            12,
            event_stream_basic,
        ),
        Story::new(
            "event-stream/burst",
            "EventStream burst",
            "EventStream",
            "Batch markers and backpressure drop chip.",
            72,
            10,
            event_stream_burst,
        ),
        Story::new(
            "event-stream/filter",
            "EventStream filter",
            "EventStream",
            "Severity floor + text filter chrome.",
            72,
            10,
            event_stream_filter,
        ),
        Story::new(
            "event-stream/detail",
            "EventStream detail",
            "EventStream",
            "Selected event with inline inspector detail.",
            72,
            10,
            event_stream_detail,
        ),
        Story::new(
            "event-stream/narrow",
            "EventStream narrow",
            "EventStream",
            "Narrow-terminal contraction for structured events.",
            28,
            10,
            event_stream_narrow,
        ),
        Story::new(
            "diff-review/hunks",
            "Diff review workbench",
            "DiffReview",
            "File tree, decisions, summary strip over DiffView.",
            80,
            16,
            diff_review_hunks,
        ),
        Story::new(
            "diff-review/decisions",
            "Diff review decisions",
            "DiffReview",
            "Approved/staged marks and multi-select.",
            80,
            14,
            diff_review_decisions,
        ),
        Story::new(
            "diff-review/comments",
            "Diff review comments",
            "DiffReview",
            "Comment draft and anchors on lines.",
            72,
            14,
            diff_review_comments,
        ),
        Story::new(
            "diff-review/confirm",
            "Diff review confirm",
            "DiffReview",
            "Safe destructive confirm banner for bulk reject.",
            72,
            12,
            diff_review_confirm,
        ),
        Story::new(
            "diff-review/empty",
            "Diff review empty",
            "DiffReview",
            "Empty-diff non-color mark.",
            32,
            3,
            diff_review_empty,
        ),
        Story::new(
            "diff-review/narrow",
            "Diff review narrow",
            "DiffReview",
            "Narrow layout hides file tree (22 cols).",
            22,
            10,
            diff_review_hunks,
        ),
        Story::new(
            "diff-review/ascii",
            "Diff review ASCII",
            "DiffReview",
            "ASCII decision glyphs and colorless paint.",
            72,
            12,
            diff_review_ascii,
        ),
        Story::new(
            "diagnostic/list",
            "Diagnostic list",
            "DiagnosticView",
            "Problems panel with severity letters (not color alone).",
            72,
            10,
            diagnostic_list,
        ),
        Story::new(
            "diagnostic/full",
            "Diagnostic code frame",
            "DiagnosticView",
            "Full recipe: source, carets, notes, fixes.",
            72,
            16,
            diagnostic_full,
        ),
        Story::new(
            "diagnostic/inline",
            "Diagnostic inline",
            "DiagnosticView",
            "Inline form/editor recipe.",
            48,
            1,
            diagnostic_inline,
        ),
        Story::new(
            "diagnostic/code-frame",
            "CodeFrame",
            "CodeFrame",
            "Standalone code frame with overlapping spans.",
            56,
            10,
            code_frame_story,
        ),
        Story::new(
            "diagnostic/empty",
            "Diagnostic empty",
            "DiagnosticView",
            "Empty problems panel mark.",
            32,
            3,
            diagnostic_empty,
        ),
        Story::new(
            "diagnostic/narrow",
            "Diagnostic narrow",
            "DiagnosticView",
            "Narrow list geometry (22 cols).",
            22,
            8,
            diagnostic_list,
        ),
        Story::new(
            "diagnostic/ascii",
            "Diagnostic ASCII",
            "DiagnosticView",
            "ASCII severity letters and underlines.",
            64,
            12,
            diagnostic_ascii,
        ),
        Story::new(
            "terminal-output/running",
            "TerminalOutput running",
            "TerminalOutput",
            "Live follow-tail command pane with stdout/stderr.",
            72,
            14,
            terminal_output_running,
        ),
        Story::new(
            "terminal-output/failed",
            "TerminalOutput failed",
            "TerminalOutput",
            "Failed exit status, duration, stderr emphasis.",
            72,
            12,
            terminal_output_failed,
        ),
        Story::new(
            "terminal-output/compact",
            "TerminalOutput compact",
            "TerminalOutput",
            "Compact card recipe for agent tools.",
            48,
            6,
            terminal_output_compact,
        ),
        Story::new(
            "terminal-output/env",
            "TerminalOutput env",
            "TerminalOutput",
            "Environment summary with redacted secrets.",
            72,
            14,
            terminal_output_env,
        ),
        Story::new(
            "terminal-output/pinned",
            "TerminalOutput pinned",
            "TerminalOutput",
            "Detached scroll while streaming (unread chip).",
            72,
            12,
            terminal_output_pinned,
        ),
        Story::new(
            "terminal-output/empty",
            "TerminalOutput empty",
            "TerminalOutput",
            "Pending empty output mark.",
            40,
            5,
            terminal_output_empty,
        ),
        Story::new(
            "terminal-output/narrow",
            "TerminalOutput narrow",
            "TerminalOutput",
            "Narrow pane geometry (22 cols).",
            22,
            10,
            terminal_output_running,
        ),
        Story::new(
            "terminal-output/ascii",
            "TerminalOutput ASCII",
            "TerminalOutput",
            "ASCII status/stream glyphs and plain paint.",
            64,
            10,
            terminal_output_ascii,
        ),
        Story::new(
            "hex-viewer/basic",
            "HexViewer basic",
            "HexViewer",
            "Offset + hex + ASCII with cursor brackets.",
            72,
            14,
            hex_viewer_basic,
        ),
        Story::new(
            "hex-viewer/selection",
            "HexViewer selection",
            "HexViewer",
            "Selected range braces without color dependence.",
            72,
            12,
            hex_viewer_selection,
        ),
        Story::new(
            "hex-viewer/inspector",
            "HexViewer inspector",
            "HexViewer",
            "Endian-aware value strip at cursor.",
            72,
            12,
            hex_viewer_inspector,
        ),
        Story::new(
            "hex-viewer/search",
            "HexViewer search",
            "HexViewer",
            "Hex search query chrome.",
            64,
            10,
            hex_viewer_search,
        ),
        Story::new(
            "hex-viewer/empty",
            "HexViewer empty",
            "HexViewer",
            "Empty buffer mark.",
            40,
            4,
            hex_viewer_empty,
        ),
        Story::new(
            "hex-viewer/narrow",
            "HexViewer narrow",
            "HexViewer",
            "Tiny-terminal compact mode (18 cols).",
            18,
            8,
            hex_viewer_basic,
        ),
        Story::new(
            "hex-viewer/ascii",
            "HexViewer ASCII",
            "HexViewer",
            "ASCII chrome and colorless selection marks.",
            64,
            10,
            hex_viewer_ascii,
        ),
        Story::new(
            "file-tree/basic",
            "FileTree basic",
            "FileTree",
            "Git status, kinds, lazy dir chrome.",
            40,
            14,
            file_tree_basic,
        ),
        Story::new(
            "file-tree/filter",
            "FileTree filter",
            "FileTree",
            "Search filter with ancestor retention.",
            40,
            12,
            file_tree_filter,
        ),
        Story::new(
            "file-tree/hidden",
            "FileTree hidden",
            "FileTree",
            "Show hidden and ignored entries.",
            40,
            12,
            file_tree_hidden,
        ),
        Story::new(
            "file-tree/confirm",
            "FileTree delete confirm",
            "FileTree",
            "Safe multi-delete confirm banner.",
            48,
            10,
            file_tree_confirm,
        ),
        Story::new(
            "file-tree/empty",
            "FileTree empty",
            "FileTree",
            "Empty tree mark.",
            28,
            4,
            file_tree_empty,
        ),
        Story::new(
            "file-tree/narrow",
            "FileTree narrow",
            "FileTree",
            "Narrow file tree (22 cols).",
            22,
            10,
            file_tree_basic,
        ),
        Story::new(
            "file-tree/ascii",
            "FileTree ASCII",
            "FileTree",
            "ASCII kind glyphs.",
            36,
            10,
            file_tree_ascii,
        ),
        Story::new(
            "process-table/basic",
            "ProcessTable basic",
            "ProcessTable",
            "Flat CPU-sorted process monitor.",
            72,
            14,
            process_table_basic,
        ),
        Story::new(
            "process-table/tree",
            "ProcessTable tree",
            "ProcessTable",
            "Parent/child hierarchy mode.",
            72,
            14,
            process_table_tree,
        ),
        Story::new(
            "process-table/filter",
            "ProcessTable filter",
            "ProcessTable",
            "Search filter on command/user.",
            64,
            12,
            process_table_filter,
        ),
        Story::new(
            "process-table/confirm",
            "ProcessTable signal confirm",
            "ProcessTable",
            "Safe TERM/KILL confirmation banner.",
            64,
            12,
            process_table_confirm,
        ),
        Story::new(
            "process-table/empty",
            "ProcessTable empty",
            "ProcessTable",
            "Empty process list mark.",
            40,
            6,
            process_table_empty,
        ),
        Story::new(
            "process-table/narrow",
            "ProcessTable narrow",
            "ProcessTable",
            "Narrow process table (36 cols).",
            36,
            12,
            process_table_basic,
        ),
        Story::new(
            "process-table/ascii",
            "ProcessTable ASCII",
            "ProcessTable",
            "ASCII selection and tree glyphs.",
            64,
            12,
            process_table_ascii,
        ),
        Story::new(
            "query-editor/basic",
            "QueryEditor basic",
            "QueryEditor",
            "SQL draft with results slot chrome.",
            72,
            18,
            query_editor_basic,
        ),
        Story::new(
            "query-editor/running",
            "QueryEditor running",
            "QueryEditor",
            "In-flight run status chrome.",
            72,
            16,
            query_editor_running,
        ),
        Story::new(
            "query-editor/diagnostics",
            "QueryEditor diagnostics",
            "QueryEditor",
            "Diagnostic strip with severity letters.",
            72,
            16,
            query_editor_diagnostics,
        ),
        Story::new(
            "query-editor/parameters",
            "QueryEditor parameters",
            "QueryEditor",
            "Parameter chips including secret redaction.",
            72,
            14,
            query_editor_parameters,
        ),
        Story::new(
            "query-editor/compact",
            "QueryEditor compact",
            "QueryEditor",
            "Compact mode hides results slot.",
            56,
            10,
            query_editor_compact,
        ),
        Story::new(
            "query-editor/empty",
            "QueryEditor empty",
            "QueryEditor",
            "Empty draft with placeholder.",
            48,
            10,
            query_editor_empty,
        ),
        Story::new(
            "query-editor/narrow",
            "QueryEditor narrow",
            "QueryEditor",
            "Narrow query workbench (36 cols).",
            36,
            14,
            query_editor_basic,
        ),
        Story::new(
            "query-editor/ascii",
            "QueryEditor ASCII",
            "QueryEditor",
            "ASCII focus and status glyphs.",
            64,
            14,
            query_editor_ascii,
        ),
        Story::new(
            "result-grid/basic",
            "ResultGrid basic",
            "ResultGrid",
            "Typed cells, nulls, binary, secrets.",
            80,
            14,
            result_grid_basic,
        ),
        Story::new(
            "result-grid/streaming",
            "ResultGrid streaming",
            "ResultGrid",
            "Partial/streaming load chrome.",
            72,
            12,
            result_grid_streaming,
        ),
        Story::new(
            "result-grid/stats",
            "ResultGrid stats",
            "ResultGrid",
            "Column statistics strip.",
            72,
            12,
            result_grid_stats,
        ),
        Story::new(
            "result-grid/wide",
            "ResultGrid wide schema",
            "ResultGrid",
            "Many columns under priority pressure.",
            60,
            12,
            result_grid_wide,
        ),
        Story::new(
            "result-grid/empty",
            "ResultGrid empty",
            "ResultGrid",
            "Empty result set.",
            40,
            8,
            result_grid_empty,
        ),
        Story::new(
            "result-grid/error",
            "ResultGrid error",
            "ResultGrid",
            "Failed query status.",
            48,
            8,
            result_grid_error,
        ),
        Story::new(
            "result-grid/narrow",
            "ResultGrid narrow",
            "ResultGrid",
            "Narrow results (36 cols).",
            36,
            12,
            result_grid_basic,
        ),
        Story::new(
            "result-grid/ascii",
            "ResultGrid ASCII",
            "ResultGrid",
            "ASCII null glyphs and safe redaction.",
            72,
            12,
            result_grid_ascii,
        ),
        Story::new(
            "schema-browser/basic",
            "SchemaBrowser basic",
            "SchemaBrowser",
            "Connection → db → schema → tables/columns.",
            40,
            16,
            schema_browser_basic,
        ),
        Story::new(
            "schema-browser/lazy",
            "SchemaBrowser lazy",
            "SchemaBrowser",
            "Lazy table and offline connection.",
            40,
            14,
            schema_browser_lazy,
        ),
        Story::new(
            "schema-browser/filter",
            "SchemaBrowser filter",
            "SchemaBrowser",
            "Search with ancestor retention.",
            40,
            12,
            schema_browser_filter,
        ),
        Story::new(
            "schema-browser/error",
            "SchemaBrowser error",
            "SchemaBrowser",
            "Load error on branch.",
            40,
            10,
            schema_browser_error,
        ),
        Story::new(
            "schema-browser/drawer",
            "SchemaBrowser drawer",
            "SchemaBrowser",
            "Drawer presentation mode.",
            36,
            12,
            schema_browser_drawer,
        ),
        Story::new(
            "schema-browser/empty",
            "SchemaBrowser empty",
            "SchemaBrowser",
            "Empty catalog.",
            28,
            6,
            schema_browser_empty,
        ),
        Story::new(
            "schema-browser/narrow",
            "SchemaBrowser narrow",
            "SchemaBrowser",
            "Narrow side pane (24 cols).",
            24,
            12,
            schema_browser_basic,
        ),
        Story::new(
            "schema-browser/ascii",
            "SchemaBrowser ASCII",
            "SchemaBrowser",
            "ASCII kind glyphs.",
            36,
            12,
            schema_browser_ascii,
        ),
        Story::new(
            "search-results/basic",
            "SearchResults basic",
            "SearchResults",
            "Grouped hits with match snippets.",
            64,
            14,
            search_results_basic,
        ),
        Story::new(
            "search-results/loading",
            "SearchResults loading",
            "SearchResults",
            "In-flight search chrome.",
            48,
            8,
            search_results_loading,
        ),
        Story::new(
            "search-results/empty",
            "SearchResults empty",
            "SearchResults",
            "No matches.",
            40,
            6,
            search_results_empty,
        ),
        Story::new(
            "search-results/stale",
            "SearchResults stale",
            "SearchResults",
            "Stale generation banner.",
            48,
            8,
            search_results_stale,
        ),
        Story::new(
            "search-results/collapsed",
            "SearchResults collapsed group",
            "SearchResults",
            "Collapsed group band.",
            56,
            12,
            search_results_collapsed,
        ),
        Story::new(
            "search-results/streaming",
            "SearchResults streaming",
            "SearchResults",
            "Partial streaming status.",
            56,
            12,
            search_results_streaming,
        ),
        Story::new(
            "search-results/narrow",
            "SearchResults narrow",
            "SearchResults",
            "Narrow results (32 cols).",
            32,
            12,
            search_results_basic,
        ),
        Story::new(
            "search-results/ascii",
            "SearchResults ASCII",
            "SearchResults",
            "ASCII selection glyphs.",
            56,
            12,
            search_results_ascii,
        ),
        Story::new(
            "metrics-dashboard/basic",
            "MetricsDashboard basic",
            "MetricsDashboard",
            "Metric cards, sparklines, gauges, alerts.",
            88,
            20,
            metrics_dashboard_basic,
        ),
        Story::new(
            "metrics-dashboard/narrow",
            "MetricsDashboard narrow",
            "MetricsDashboard",
            "Vertical summary under 48 cols.",
            40,
            14,
            metrics_dashboard_basic,
        ),
        Story::new(
            "metrics-dashboard/partial-fail",
            "MetricsDashboard partial fail",
            "MetricsDashboard",
            "One failed tile among healthy metrics.",
            80,
            18,
            metrics_dashboard_partial,
        ),
        Story::new(
            "metrics-dashboard/paused",
            "MetricsDashboard paused",
            "MetricsDashboard",
            "Paused auto-refresh chrome.",
            72,
            16,
            metrics_dashboard_paused,
        ),
        Story::new(
            "metrics-dashboard/empty",
            "MetricsDashboard empty",
            "MetricsDashboard",
            "No tiles yet.",
            48,
            8,
            metrics_dashboard_empty,
        ),
        Story::new(
            "metrics-dashboard/ascii",
            "MetricsDashboard ASCII",
            "MetricsDashboard",
            "ASCII glyphs and borders.",
            72,
            16,
            metrics_dashboard_ascii,
        ),
        Story::new(
            "trace-waterfall/basic",
            "TraceWaterfall basic",
            "TraceWaterfall",
            "Nested spans with duration bars and critical path.",
            80,
            14,
            trace_waterfall_basic,
        ),
        Story::new(
            "trace-waterfall/error",
            "TraceWaterfall error span",
            "TraceWaterfall",
            "Failed tool span in hierarchy.",
            72,
            12,
            trace_waterfall_error,
        ),
        Story::new(
            "trace-waterfall/critical",
            "TraceWaterfall critical only",
            "TraceWaterfall",
            "Critical-path filter.",
            72,
            12,
            trace_waterfall_critical,
        ),
        Story::new(
            "trace-waterfall/zoomed",
            "TraceWaterfall zoomed",
            "TraceWaterfall",
            "Time window zoomed into mid-trace.",
            72,
            12,
            trace_waterfall_zoomed,
        ),
        Story::new(
            "trace-waterfall/empty",
            "TraceWaterfall empty",
            "TraceWaterfall",
            "Empty span set.",
            40,
            6,
            trace_waterfall_empty,
        ),
        Story::new(
            "trace-waterfall/narrow",
            "TraceWaterfall narrow",
            "TraceWaterfall",
            "Narrow waterfall (36 cols).",
            36,
            12,
            trace_waterfall_basic,
        ),
        Story::new(
            "trace-waterfall/ascii",
            "TraceWaterfall ASCII",
            "TraceWaterfall",
            "ASCII bars and markers.",
            72,
            12,
            trace_waterfall_ascii,
        ),
        Story::new(
            "dependency-graph/basic",
            "DependencyGraph basic",
            "DependencyGraph",
            "Layered package/service deps with ASCII connectors.",
            72,
            16,
            dependency_graph_basic,
        ),
        Story::new(
            "dependency-graph/tree",
            "DependencyGraph tree fallback",
            "DependencyGraph",
            "TreeTable-shaped fallback view.",
            56,
            14,
            dependency_graph_tree,
        ),
        Story::new(
            "dependency-graph/list",
            "DependencyGraph list",
            "DependencyGraph",
            "Flat list representation.",
            56,
            12,
            dependency_graph_list,
        ),
        Story::new(
            "dependency-graph/filter",
            "DependencyGraph filter",
            "DependencyGraph",
            "Filtered node set.",
            56,
            12,
            dependency_graph_filter,
        ),
        Story::new(
            "dependency-graph/narrow",
            "DependencyGraph narrow",
            "DependencyGraph",
            "Auto tree under 40 cols.",
            36,
            12,
            dependency_graph_basic,
        ),
        Story::new(
            "dependency-graph/ascii",
            "DependencyGraph ASCII",
            "DependencyGraph",
            "ASCII connectors and glyphs.",
            64,
            14,
            dependency_graph_ascii,
        ),
        Story::new(
            "completion-menu/basic",
            "Completion menu",
            "CompletionMenu",
            "Groups, glyphs, details, docs panel; active-descendant selection.",
            56,
            12,
            completion_menu_basic,
        ),
        Story::new(
            "completion-menu/loading",
            "Completion menu loading",
            "CompletionMenu",
            "Async loading chrome with generation-gated empty list.",
            40,
            6,
            completion_menu_loading_story,
        ),
        Story::new(
            "completion-menu/docs",
            "Completion menu docs",
            "CompletionMenu",
            "Documentation side preview for selected candidate.",
            64,
            12,
            completion_menu_docs_story,
        ),
        Story::new(
            "completion-menu/edge",
            "Completion menu edge flip",
            "CompletionMenu",
            "Bottom-right anchor flips above and clamps inside bounds.",
            40,
            12,
            completion_menu_edge,
        ),
        Story::new(
            "completion-menu/narrow",
            "Completion menu narrow",
            "CompletionMenu",
            "Narrow bounds promote fullscreen presentation.",
            22,
            8,
            completion_menu_basic,
        ),
        Story::new(
            "completion-menu/unicode",
            "Completion menu Unicode",
            "CompletionMenu",
            "Display-width clipping preserves complete Unicode candidates.",
            40,
            10,
            completion_menu_unicode_story,
        ),
        Story::new(
            "virtual-grid/basic",
            "Virtual grid",
            "VirtualGrid",
            "Two-axis virtualized grid with resident window and pending cells.",
            72,
            12,
            virtual_grid_basic,
        )
        .with_interactor(virtual_grid_interactor),
        Story::new(
            "virtual-grid/million",
            "Virtual grid million-row window",
            "VirtualGrid",
            "Viewport over a synthetic 1_000_000-row corpus (windowed only).",
            72,
            14,
            virtual_grid_million,
        ),
        Story::new(
            "virtual-grid/narrow",
            "Virtual grid narrow",
            "VirtualGrid",
            "Column clipping stays bounded in a narrow viewport.",
            28,
            8,
            virtual_grid_basic,
        ),
        Story::new(
            "virtual-grid/unicode",
            "Virtual grid Unicode",
            "VirtualGrid",
            "Headers and cells preserve Unicode display-column boundaries.",
            48,
            10,
            virtual_grid_unicode_story,
        ),
        Story::new(
            "table/basic",
            "Data table",
            "Table",
            "Stable-ID columnar data with selection and headers.",
            68,
            8,
            table_basic,
        )
        .with_interactor(table_interactor),
        Story::new(
            "table/sorted",
            "Sorted table",
            "Table",
            "Caller-owned descending sort projection.",
            68,
            8,
            table_sorted,
        ),
        Story::new(
            "table/narrow",
            "Narrow table",
            "Table",
            "Deterministic rightmost-column collapse.",
            20,
            6,
            table_narrow,
        ),
        Story::new(
            "table/unicode",
            "Unicode table",
            "Table",
            "Styled CJK and emoji cells clip at display boundaries.",
            42,
            6,
            table_unicode,
        ),
        Story::new(
            "table/disabled",
            "Disabled table row",
            "Table",
            "Disabled rows remain visible but non-interactive.",
            52,
            6,
            table_disabled,
        ),
        Story::new(
            "table/empty",
            "Empty table",
            "Table",
            "Header plus empty body message.",
            42,
            4,
            table_empty,
        ),
        Story::new(
            "table/bordered",
            "Bordered table",
            "Table",
            "Quiet vertical separators and header rule.",
            68,
            8,
            table_bordered,
        ),
        Story::new(
            "table/striped",
            "Striped table",
            "Table",
            "Alternate-row muted text without heavy fill.",
            68,
            8,
            table_striped,
        ),
        Story::new(
            "table/compact",
            "Compact table",
            "Table",
            "Tight column gap recipe.",
            68,
            8,
            table_compact,
        ),
        Story::new(
            "table/loading",
            "Loading table",
            "Table",
            "Sticky header with loading body message.",
            52,
            5,
            table_loading,
        ),
        Story::new(
            "table/error",
            "Error table",
            "Table",
            "Sticky header with error body message.",
            52,
            5,
            table_error,
        ),
        Story::new(
            "table/priority",
            "Priority columns",
            "Table",
            "Low-priority columns drop first under width pressure.",
            28,
            6,
            table_priority,
        ),
        Story::new(
            "text-area/basic",
            "Text area",
            "TextArea",
            "Multi-line editing with caller-owned submission policy.",
            52,
            9,
            text_area_basic,
        )
        .with_interactor(text_area_interactor),
        Story::new(
            "text-area/narrow",
            "Narrow text area",
            "TextArea",
            "Horizontal viewport clips only complete graphemes.",
            18,
            7,
            text_area_narrow,
        ),
        Story::new(
            "text-area/unicode",
            "Unicode text area",
            "TextArea",
            "Combining, CJK, emoji, and remembered goal-column content.",
            38,
            8,
            text_area_unicode,
        ),
        Story::new(
            "text-area/empty",
            "Empty text area",
            "TextArea",
            "Product-neutral placeholder in an empty document.",
            38,
            6,
            text_area_empty,
        ),
        Story::new(
            "text-area/scrolled",
            "Scrolled text area",
            "TextArea",
            "Two-axis cursor-follow viewport over logical lines.",
            34,
            7,
            text_area_scrolled,
        ),
        Story::new(
            "text-area/line-numbers",
            "Text area line numbers",
            "TextArea",
            "Gutter line numbers beside multi-line body.",
            40,
            8,
            text_area_line_numbers,
        ),
        Story::new(
            "text-area/soft-wrap",
            "Text area soft wrap",
            "TextArea",
            "Soft-wrap long lines without horizontal scroll.",
            28,
            8,
            text_area_soft_wrap,
        ),
        Story::new(
            "text-area/review",
            "Text area review",
            "TextArea",
            "Review/comment muted chrome variant.",
            40,
            7,
            text_area_review,
        ),
        Story::new(
            "status-bar/basic",
            "Status bar",
            "StatusBar",
            "Mode, focus zone, selection, and shortcuts.",
            64,
            1,
            status_bar,
        ),
        Story::new(
            "status-bar/minimal",
            "Status bar minimal",
            "StatusBar",
            "Minimal recipe: mode + connection.",
            48,
            1,
            status_bar_minimal_story,
        ),
        Story::new(
            "status-bar/transient",
            "Status bar transient",
            "StatusBar",
            "Transient message without dropping essentials.",
            56,
            1,
            status_bar_transient_story,
        ),
        Story::new(
            "status-bar/rich",
            "Status bar rich",
            "StatusBar",
            "Rich recipe keeps key hints.",
            72,
            1,
            status_bar_rich_story,
        ),
        Story::new(
            "status-indicator/catalog",
            "StatusIndicator catalog",
            "StatusIndicator",
            "Shared vocabulary: all kinds with glyph + label.",
            40,
            12,
            status_indicator_catalog_story,
        ),
        Story::new(
            "status-indicator/compact",
            "StatusIndicator compact",
            "StatusIndicator",
            "Dot-like compact glyphs for rows and rails.",
            24,
            1,
            status_indicator_compact_story,
        ),
        Story::new(
            "status-indicator/elapsed",
            "StatusIndicator elapsed",
            "StatusIndicator",
            "Running status with elapsed-time suffix.",
            28,
            1,
            status_indicator_elapsed_story,
        ),
        Story::new(
            "status-indicator/ascii",
            "StatusIndicator ASCII",
            "StatusIndicator",
            "ASCII capability profile glyphs (no Unicode dots).",
            36,
            4,
            status_indicator_ascii_story,
        ),
        Story::new(
            "design-inspector/basic",
            "Design inspector",
            "DesignInspector",
            "Studio focus/layer/capability strip with Semantics panel.",
            48,
            4,
            design_inspector,
        )
        .with_interactor(design_inspector_interactor),
        Story::new(
            "semantic-scene/tree",
            "Semantic scene tree",
            "SemanticScene",
            "Frame-local parent tree, labels, and snapshot lines for Studio/AI.",
            48,
            8,
            semantic_scene_tree_story,
        ),
        Story::new(
            "semantic-scene/hit-jump",
            "Semantic hit and jump",
            "SemanticScene",
            "Hit-test focusable nodes; jump badges from jump_regions.",
            48,
            10,
            semantic_scene_hit_jump_story,
        ),
        Story::new(
            "semantic-scene/snapshot",
            "Semantic snapshot text",
            "SemanticScene",
            "Portable to_text / from_text for remote and AI-readable UI.",
            56,
            12,
            semantic_scene_snapshot_story,
        ),
        Story::new(
            "semantic-scene/virt-window",
            "Semantic virtualized window",
            "SemanticScene",
            "Only visible rows registered (not full logical length).",
            40,
            12,
            semantic_scene_virt_story,
        ),
        Story::new(
            "event-result/compose",
            "EventResult bubble compose",
            "EventResult",
            "Child Stop wins; parent only runs when child bubbles.",
            48,
            6,
            event_result_compose_story,
        ),
        Story::new(
            "focus-graph/workbench",
            "FocusGraph workbench zones",
            "FocusGraph",
            "Linear tab order, roving list, trap, Focus Lens markers.",
            56,
            12,
            focus_graph_workbench_story,
        ),
        Story::new(
            "roving-focus/group",
            "RovingFocusGroup",
            "RovingFocusGroup",
            "Active descendant skips disabled; typeahead; vertical orientation.",
            40,
            8,
            roving_focus_group_story,
        ),
        Story::new(
            "collection-state/headless",
            "CollectionState headless",
            "CollectionState",
            "Shared model: active id, virt offset, disabled skip (list paint).",
            40,
            8,
            collection_state_story,
        ),
        Story::new(
            "selection-model/multi",
            "SelectionModel multi + checks",
            "SelectionModel",
            "Ordered multi-select with check glyphs (not color alone).",
            40,
            8,
            selection_model_story,
        ),
        Story::new(
            "scroll-area/follow-paused",
            "ScrollArea follow + bars + new",
            "ScrollArea",
            "Vertical bars, paused follow, ↓ N new non-color indicator.",
            48,
            12,
            scroll_area_follow_story,
        ),
        Story::new(
            "virtual-list/million",
            "VirtualList million-row",
            "VirtualList",
            "1M logical rows; O(viewport) project+paint; sticky header; diagnostics.",
            52,
            16,
            virtual_list_million_story,
        ),
        Story::new(
            "virtual-list/follow-tail",
            "VirtualList follow-tail",
            "VirtualList",
            "Streaming tail follow with live growth.",
            48,
            12,
            virtual_list_follow_tail_story,
        ),
        Story::new(
            "virtual-list/loading",
            "VirtualList page loading",
            "VirtualList",
            "Async page loading chrome with placeholders.",
            48,
            12,
            virtual_list_loading_story,
        ),
        Story::new(
            "virtualizer/million-fixed",
            "Virtualizer 1M fixed slots",
            "Virtualizer",
            "O(viewport) window over 1_000_000 logical rows; semantic budget tiny.",
            48,
            12,
            virtualizer_million_story,
        ),
        Story::new(
            "capability/color-ladder",
            "Capability color ladder",
            "DesignInspector",
            "Truecolor / 256 / 16 / mono swatches for capability degradation.",
            56,
            14,
            capability_color_ladder_story,
        ),
        Story::new(
            "capability/no-color",
            "Capability no-color",
            "DesignInspector",
            "Monochrome system still conveys roles via structure.",
            48,
            10,
            capability_no_color_story,
        ),
        Story::new(
            "capability/ascii-glyphs",
            "Capability ASCII glyphs",
            "List",
            "ASCII glyph set for disclosure/selection without Unicode.",
            40,
            8,
            capability_ascii_glyphs_story,
        ),
        Story::new(
            "capability/headless",
            "Capability headless doctor",
            "DesignInspector",
            "Headless profile: keyboard off, mono-friendly chrome.",
            48,
            8,
            capability_headless_story,
        ),
        Story::new(
            "capability/profiles",
            "Capability profiles matrix",
            "TerminalCapabilities",
            "Modern / Compatible / Minimal / Inline / Headless boundary flags.",
            56,
            12,
            capability_profiles_story,
        ),
        Story::new(
            "motion/presence-spinner",
            "FrameClock presence + spinner",
            "FrameClock",
            "Motion::Full vs Off spinner; toast presence TTL; no idle redraw demand.",
            48,
            10,
            motion_presence_story,
        ),
        Story::new(
            "spinner/labeled",
            "Spinner labeled",
            "Spinner",
            "Indeterminate spinner with required verb label.",
            28,
            2,
            spinner_labeled_story,
        ),
        Story::new(
            "spinner/phases",
            "Spinner phases",
            "Spinner",
            "Indeterminate, waiting, queued, reconnecting phases.",
            40,
            8,
            spinner_phases_story,
        ),
        Story::new(
            "spinner/compact",
            "Spinner compact embedded",
            "Spinner",
            "Compact inline glyph when embedded in labeled control.",
            12,
            1,
            spinner_compact_story,
        ),
        Story::new(
            "spinner/ascii",
            "Spinner ASCII",
            "Spinner",
            "ASCII |/-\\ frames with Motion::Off static glyph.",
            24,
            2,
            spinner_ascii_story,
        ),
        Story::new(
            "activity-indicator/basic",
            "ActivityIndicator",
            "ActivityIndicator",
            "Phase activity with verb and detail line.",
            40,
            3,
            activity_indicator_story,
        ),
        Story::new(
            "registry/contracts",
            "ComponentContract catalog",
            "ComponentContract",
            "Official kernel contracts: kind, complete, stories, module.",
            56,
            12,
            registry_contracts_story,
        ),
        Story::new(
            "overlay/nested-escape",
            "Nested overlays",
            "OverlayStack",
            "Parent dialog + child menu; Esc peels one layer.",
            48,
            14,
            overlay_nested,
        ),
        Story::new(
            "dismissable/gestures",
            "DismissableLayer gestures",
            "DismissableLayer",
            "Press/release outside dismiss; trap critical; drag-cancel.",
            48,
            10,
            dismissable_gestures_story,
        ),
        Story::new(
            "responsive/ladder-inspector",
            "Responsive ladder inspector",
            "Responsive",
            "WIDTH_LADDER stages for Form + Table; essential always on.",
            56,
            14,
            responsive_ladder_story,
        ),
        Story::new(
            "overlay/edge-placement",
            "Overlay edges",
            "OverlayStack",
            "Menus near top/bottom/left/right clamp and flip.",
            50,
            16,
            overlay_edges,
        ),
        Story::new(
            "overlay/tiny",
            "Tiny overlay fallback",
            "OverlayStack",
            "Dialog promotes fullscreen; tooltip hides.",
            28,
            8,
            overlay_tiny,
        ),
        Story::new(
            "overlay/queued-dialogs",
            "Queued dialogs",
            "OverlayStack",
            "OpenMode::Queue: deferred dialog waits behind blocking top.",
            44,
            12,
            overlay_queued,
        ),
        Story::new(
            "overlay/fullscreen-promote",
            "Fullscreen promote",
            "OverlayStack",
            "Popover promoted to fullscreen bounds.",
            40,
            12,
            overlay_fullscreen_promote,
        ),
        Story::new(
            "dialog/message",
            "Message dialog",
            "Dialog",
            "Canonical modal shell with description and footer hint.",
            48,
            9,
            dialog,
        ),
        Story::new(
            "dialog/destructive",
            "Destructive dialog",
            "Dialog",
            "Destructive recipe with choice actions; Enter only on action zone.",
            48,
            10,
            dialog_destructive_story,
        ),
        Story::new(
            "dialog/compact",
            "Compact dialog",
            "Dialog",
            "Compact recipe for tight confirmations.",
            36,
            6,
            dialog_compact_story,
        ),
        Story::new(
            "alert-dialog/delete",
            "AlertDialog delete",
            "AlertDialog",
            "Permanent delete: scope, consequences, safer alternative; safe focus.",
            56,
            16,
            alert_dialog_delete_story,
        ),
        Story::new(
            "alert-dialog/overwrite",
            "AlertDialog overwrite",
            "AlertDialog",
            "Overwrite confirmation with recoverable reversibility.",
            52,
            14,
            alert_dialog_overwrite_story,
        ),
        Story::new(
            "alert-dialog/terminate",
            "AlertDialog terminate",
            "AlertDialog",
            "Process terminate with countdown gate.",
            52,
            14,
            alert_dialog_terminate_story,
        ),
        Story::new(
            "alert-dialog/egress",
            "AlertDialog data egress",
            "AlertDialog",
            "Data egress risk with typed confirmation phrase.",
            56,
            16,
            alert_dialog_egress_story,
        ),
        Story::new(
            "alert-dialog/locked",
            "AlertDialog locked critical",
            "AlertDialog",
            "Non-dismissable critical: Esc trapped; must choose action.",
            52,
            14,
            alert_dialog_locked_story,
        ),
        Story::new(
            "choice-dialog/basic",
            "Choice dialog",
            "ChoiceDialog",
            "Caller-owned stable actions in a neutral dialog shell.",
            48,
            7,
            choice_dialog,
        )
        .with_interactor(choice_dialog_interactor),
        Story::new(
            "message-dialog/details",
            "Detailed message dialog",
            "MessageDialog",
            "Caller-owned detail rows composed into a neutral message shell.",
            52,
            8,
            message_dialog,
        ),
        Story::new(
            "diff/basic",
            "Diff view unified",
            "DiffView",
            "Unified professional diff with line numbers and hunk chip.",
            72,
            12,
            diff_basic,
        ),
        Story::new(
            "diff/split",
            "Diff view split",
            "DiffView",
            "Side-by-side mode when width allows.",
            80,
            12,
            diff_split,
        ),
        Story::new(
            "diff/word",
            "Diff view word-level",
            "DiffView",
            "Word-level change spans within a line.",
            64,
            8,
            diff_word,
        ),
        Story::new(
            "diff/search",
            "Diff view search",
            "DiffView",
            "Search filter chrome on status chip.",
            64,
            10,
            diff_search,
        ),
        Story::new(
            "toast/success",
            "Toast success",
            "Toast",
            "Caller-owned transient success message (default top-right).",
            34,
            4,
            toast,
        )
        .with_interactor(toast_interactor),
        Story::new(
            "toast/kinds",
            "Toast kinds",
            "Toast",
            "Info/success/warning/error/progress/undo stacked kinds.",
            48,
            16,
            toast_kinds_story,
        ),
        Story::new(
            "toast/stack",
            "Toast stack",
            "Toast",
            "ToastQueue multi-notification stack with priority.",
            40,
            14,
            toast_stack_story,
        ),
        Story::new(
            "toast/persistent",
            "Toast persistent",
            "Toast",
            "Persistent toast until host dismiss (no TTL).",
            36,
            4,
            toast_persistent_story,
        ),
        Story::new(
            "notification-center/drawer",
            "NotificationCenter drawer",
            "NotificationCenter",
            "Right-edge drawer recipe with unread, kinds, progress, undo.",
            56,
            18,
            notification_center_drawer_story,
        ),
        Story::new(
            "notification-center/full-page",
            "NotificationCenter full page",
            "NotificationCenter",
            "Full-page recipe for dense history browsing.",
            64,
            18,
            notification_center_full_story,
        ),
        Story::new(
            "notification-center/filtered",
            "NotificationCenter unread filter",
            "NotificationCenter",
            "Unread filter applied to history list.",
            48,
            14,
            notification_center_filter_story,
        ),
        Story::new(
            "notification-center/empty",
            "NotificationCenter empty",
            "NotificationCenter",
            "Empty state after clear-all.",
            40,
            12,
            notification_center_empty_story,
        ),
        Story::new(
            "backdrop/basic",
            "Backdrop",
            "Backdrop",
            "Neutral modal backdrop policy.",
            34,
            4,
            backdrop,
        ),
        Story::new(
            "viewport/both-axes",
            "Scrollable viewport",
            "Viewport",
            "Borrowed lines with bounded horizontal and vertical scroll state.",
            44,
            7,
            viewport,
        ),
        Story::new(
            "list/narrow",
            "Narrow list",
            "List",
            "Narrow-terminal clipping and metadata priority.",
            14,
            6,
            list,
        ),
        Story::new(
            "tabs/narrow",
            "Narrow tabs",
            "Tabs",
            "Narrow-terminal tab clipping and selection cues.",
            16,
            2,
            tabs,
        ),
        Story::new(
            "form/narrow",
            "Narrow form",
            "Form",
            "Responsive single-column form at narrow width.",
            24,
            12,
            form,
        ),
        Story::new(
            "status-bar/narrow",
            "Narrow status bar",
            "StatusBar",
            "Priority-based slot elision at narrow width.",
            20,
            1,
            status_bar,
        ),
        Story::new(
            "dialog/narrow",
            "Narrow dialog",
            "Dialog",
            "Responsive dialog shell at narrow width.",
            20,
            7,
            dialog,
        ),
        Story::new(
            "toast/narrow",
            "Narrow toast",
            "Toast",
            "Bounded transient message at narrow width.",
            16,
            4,
            toast,
        ),
        Story::new(
            "list/unicode",
            "Unicode list",
            "List",
            "CJK, emoji, and combining-mark row geometry.",
            28,
            5,
            list_unicode,
        ),
        Story::new(
            "list/multi",
            "Multi-select list",
            "List",
            "Checked membership with glyph catalog chrome.",
            42,
            6,
            list_multi,
        ),
        Story::new(
            "list/empty",
            "Empty list",
            "List",
            "Empty-message projection when there are no rows.",
            32,
            4,
            list_empty,
        ),
        Story::new(
            "list/loading",
            "Loading list rows",
            "List",
            "Per-row loading leading glyph with muted primary.",
            40,
            5,
            list_loading,
        ),
        Story::new(
            "list/disabled",
            "Disabled list rows",
            "List",
            "Disabled rows skipped by keyboard; dim recipe.",
            36,
            5,
            list_disabled,
        ),
        Story::new(
            "list/ascii",
            "ASCII list glyphs",
            "List",
            "ASCII gutter and check fallbacks under GlyphSet::Ascii.",
            36,
            5,
            list_ascii,
        ),
        Story::new(
            "list/composed-row",
            "Composed list anatomy",
            "List",
            "Leading, secondary, badge, and shortcut parts.",
            48,
            5,
            list_composed,
        ),
        Story::new(
            "list/tiny",
            "Tiny list",
            "List",
            "Primary identity survives extreme width contraction.",
            10,
            4,
            list_tiny,
        ),
        Story::new(
            "list/comfortable",
            "List comfortable density",
            "List",
            "Secondary metadata on its own row under primary.",
            42,
            6,
            list_comfortable_story,
        ),
        Story::new(
            "list/groups",
            "List group headers",
            "List",
            "Group headers, status, trailing actions, typeahead-ready labels.",
            48,
            8,
            list_groups_story,
        ),
        Story::new(
            "list/search",
            "List search strip",
            "List",
            "Active search query strip with filtered projection.",
            40,
            6,
            list_search_story,
        ),
        Story::new(
            "text-input/unicode",
            "Unicode text input",
            "TextInput",
            "Wide and combining graphemes with a mid-string cursor.",
            28,
            1,
            text_input_unicode,
        ),
        Story::new(
            "detail-table/unicode",
            "Unicode detail table",
            "DetailTable",
            "CJK labels and emoji values under wrapping.",
            30,
            6,
            detail_table_unicode,
        ),
        Story::new(
            "empty-state/basic",
            "Empty state",
            "EmptyState",
            "Search no-results with primary clear action.",
            40,
            10,
            empty_state,
        ),
        Story::new(
            "empty-state/first-use",
            "EmptyState first-use",
            "EmptyState",
            "Sessions first-run welcome with primary New session.",
            42,
            10,
            empty_state_first_use_story,
        ),
        Story::new(
            "empty-state/table",
            "EmptyState table",
            "EmptyState",
            "Table no-data recipe with add/import actions.",
            42,
            10,
            empty_state_table_story,
        ),
        Story::new(
            "empty-state/permission",
            "EmptyState permission",
            "EmptyState",
            "Permission-limited empty with safe primary request.",
            42,
            10,
            empty_state_permission_story,
        ),
        Story::new(
            "empty-state/inline",
            "EmptyState inline",
            "EmptyState",
            "Concise inline form for small panes.",
            28,
            2,
            empty_state_inline_story,
        ),
        Story::new(
            "loading-view/basic",
            "Loading view",
            "LoadingView",
            "Centered loading label with spinner frame.",
            36,
            3,
            loading_view,
        ),
        Story::new(
            "loading-overlay/blocking",
            "LoadingOverlay blocking",
            "LoadingOverlay",
            "Regional blocking wash after min-show; content unavailable.",
            42,
            10,
            loading_overlay_blocking_story,
        ),
        Story::new(
            "loading-overlay/cancellable",
            "LoadingOverlay cancellable",
            "LoadingOverlay",
            "Cancellable long op with esc cancel routing.",
            42,
            10,
            loading_overlay_cancellable_story,
        ),
        Story::new(
            "loading-overlay/non-blocking",
            "LoadingOverlay non-blocking",
            "LoadingOverlay",
            "Non-blocking busy cue; input still delivered.",
            36,
            6,
            loading_overlay_non_blocking_story,
        ),
        Story::new(
            "loading-overlay/optimistic",
            "LoadingOverlay optimistic",
            "LoadingOverlay",
            "Optimistic update badge; content preserved.",
            36,
            5,
            loading_overlay_optimistic_story,
        ),
        Story::new(
            "loading-overlay/stale",
            "LoadingOverlay stale",
            "LoadingOverlay",
            "Stale-content presentation while revalidating.",
            40,
            8,
            loading_overlay_stale_story,
        ),
        Story::new(
            "loading-overlay/nested",
            "BusyBoundary nested",
            "BusyBoundary",
            "Parent + child regional busy without freezing whole app.",
            48,
            12,
            loading_overlay_nested_story,
        ),
        Story::new(
            "connectivity/banner",
            "Offline banner",
            "OfflineBanner",
            "Unobtrusive reconnecting banner with queue count.",
            56,
            1,
            connectivity_banner_story,
        ),
        Story::new(
            "connectivity/reconnecting",
            "Reconnecting full",
            "OfflineSurface",
            "Full reconnect surface: attempts, queue, offline caps, drafts.",
            52,
            14,
            connectivity_reconnecting_story,
        ),
        Story::new(
            "connectivity/auth",
            "Auth required",
            "OfflineSurface",
            "Authentication required full recovery surface.",
            48,
            12,
            connectivity_auth_story,
        ),
        Story::new(
            "connectivity/unavailable",
            "Server unavailable",
            "OfflineSurface",
            "Server unavailable with queued query and cached-read caps.",
            50,
            14,
            connectivity_unavailable_story,
        ),
        Story::new(
            "connectivity/status-bar",
            "Connectivity StatusBar",
            "ReconnectingState",
            "StatusBar connection slot projected from ReconnectingState.",
            64,
            1,
            connectivity_status_bar_story,
        ),
        Story::new(
            "connectivity/notification",
            "Connectivity notification",
            "ReconnectingState",
            "NotificationCenter ingest from connectivity state.",
            48,
            12,
            connectivity_notification_story,
        ),
        Story::new(
            "error-view/basic",
            "Error view",
            "ErrorView",
            "Network failure with recovery (ErrorState / ErrorView).",
            48,
            12,
            error_view,
        ),
        Story::new(
            "error-state/network",
            "ErrorState network",
            "ErrorState",
            "Network error: summary, safety, retry, copy diagnostics.",
            48,
            12,
            error_state_network_story,
        ),
        Story::new(
            "error-state/validation",
            "ErrorState validation",
            "ErrorState",
            "Validation failure: work preserved, alternative edit.",
            48,
            10,
            error_state_validation_story,
        ),
        Story::new(
            "error-state/permission",
            "ErrorState permission",
            "ErrorState",
            "Permission denied with report/copy recovery.",
            48,
            11,
            error_state_permission_story,
        ),
        Story::new(
            "error-state/details",
            "ErrorState technical details",
            "ErrorState",
            "Technical details expanded (collapsed by default).",
            50,
            12,
            error_state_details_story,
        ),
        Story::new(
            "error-state/inline",
            "ErrorState inline",
            "ErrorState",
            "Inline recipe for small panes.",
            36,
            2,
            error_state_inline_story,
        ),
        Story::new(
            "error-state/dialog",
            "ErrorState dialog",
            "ErrorState",
            "Dialog-sized recoverable error.",
            44,
            12,
            error_state_dialog_story,
        ),
        Story::new(
            "error-state/fullscreen",
            "ErrorState full-screen",
            "ErrorState",
            "Full-screen crash recovery surface.",
            56,
            16,
            error_state_fullscreen_story,
        ),
        Story::new(
            "banner/basic",
            "Banner",
            "Banner",
            "Single-line severity banner.",
            40,
            1,
            banner,
        ),
        Story::new(
            "skeleton/basic",
            "Skeleton lines",
            "Skeleton",
            "Staggered text-line placeholders (structure-known loading).",
            32,
            4,
            skeleton,
        ),
        Story::new(
            "skeleton/card",
            "Skeleton card",
            "Skeleton",
            "Card header + body line placeholders.",
            28,
            6,
            skeleton_card_story,
        ),
        Story::new(
            "skeleton/table",
            "Skeleton table",
            "Skeleton",
            "Multi-column table row placeholders.",
            36,
            6,
            skeleton_table_story,
        ),
        Story::new(
            "skeleton/tiny",
            "Skeleton tiny",
            "Skeleton",
            "Capability/tiny geometry without panic.",
            8,
            3,
            skeleton_tiny_story,
        ),
        Story::new(
            "skeleton/ascii",
            "Skeleton ASCII",
            "Skeleton",
            "ASCII # fill for no-Unicode terminals.",
            24,
            4,
            skeleton_ascii_story,
        ),
        Story::new(
            "jump-overlay/basic",
            "Jump overlay",
            "JumpOverlay",
            "Letter badges over target regions.",
            40,
            6,
            jump_overlay,
        ),
        Story::new(
            "jump-mode/multi",
            "JumpMode multi-key",
            "JumpMode",
            "Prefix-free multi-key labels for dense targets.",
            48,
            14,
            jump_mode_multi,
        ),
        Story::new(
            "jump-mode/filter",
            "JumpMode role filter",
            "JumpMode",
            "Buttons only via JumpFilter from SemanticScene.",
            48,
            10,
            jump_mode_filter,
        ),
        Story::new(
            "jump-mode/ascii",
            "JumpMode ASCII",
            "JumpMode",
            "ASCII badges + colorless paint.",
            40,
            8,
            jump_mode_ascii,
        ),
        Story::new(
            "focus-lens/combined",
            "FocusLens combined",
            "FocusLens",
            "Tab-order indices + focused marker.",
            48,
            10,
            focus_lens_combined,
        ),
        Story::new(
            "command-palette/basic",
            "Command palette",
            "CommandPalette",
            "Flagship command surface with groups and shortcuts.",
            48,
            14,
            command_palette,
        )
        .with_interactor(command_palette_interactor),
        Story::new(
            "command-palette/empty",
            "Command palette empty",
            "CommandPalette",
            "Empty catalog with non-color mark and footer.",
            42,
            10,
            command_palette_empty,
        ),
        Story::new(
            "command-palette/no-result",
            "Command palette no result",
            "CommandPalette",
            "Query with zero matches — polished empty state.",
            42,
            10,
            command_palette_no_result,
        ),
        Story::new(
            "command-palette/loading",
            "Command palette loading",
            "CommandPalette",
            "Async loading state before results apply.",
            42,
            10,
            command_palette_loading,
        ),
        Story::new(
            "command-palette/fuzzy",
            "Command palette fuzzy",
            "CommandPalette",
            "Fuzzy highlight ranges on matching labels.",
            48,
            12,
            command_palette_fuzzy,
        ),
        Story::new(
            "command-palette/nested",
            "Command palette nested page",
            "CommandPalette",
            "Nested page for keybindings.",
            48,
            12,
            command_palette_nested,
        ),
        Story::new(
            "command-palette/args",
            "Command palette arguments",
            "CommandPalette",
            "Argument phase for Go to line.",
            48,
            10,
            command_palette_args,
        ),
        Story::new(
            "command-palette/ascii",
            "Command palette ASCII",
            "CommandPalette",
            "ASCII empty cue and normal surface chrome.",
            40,
            8,
            command_palette_ascii,
        ),
        Story::new(
            "quick-open/basic",
            "QuickOpen files",
            "QuickOpen",
            "Multi-provider fuzzy opener with preview pane.",
            72,
            18,
            quick_open_basic,
        ),
        Story::new(
            "quick-open/symbols",
            "QuickOpen symbols",
            "QuickOpen",
            "Symbols provider active.",
            64,
            14,
            quick_open_symbols,
        ),
        Story::new(
            "quick-open/fuzzy",
            "QuickOpen fuzzy",
            "QuickOpen",
            "Fuzzy highlight on filter.",
            64,
            14,
            quick_open_fuzzy,
        ),
        Story::new(
            "quick-open/loading",
            "QuickOpen loading",
            "QuickOpen",
            "Streaming search chrome.",
            48,
            12,
            quick_open_loading,
        ),
        Story::new(
            "quick-open/empty",
            "QuickOpen empty",
            "QuickOpen",
            "Empty query polish.",
            48,
            12,
            quick_open_empty,
        ),
        Story::new(
            "quick-open/narrow",
            "QuickOpen narrow",
            "QuickOpen",
            "Narrow / fullscreen-class geometry.",
            36,
            14,
            quick_open_narrow,
        ),
        Story::new(
            "quick-open/ascii",
            "QuickOpen ASCII",
            "QuickOpen",
            "ASCII glyphs and colorless roles.",
            56,
            14,
            quick_open_ascii,
        ),
        Story::new(
            "code-block/basic",
            "Code block",
            "CodeBlock",
            "Source listing with line numbers, path meta, and role syntax.",
            48,
            6,
            code_block,
        ),
        Story::new(
            "code-block/no-color",
            "CodeBlock no-color",
            "CodeBlock",
            "Monochrome syntax fallback — bold/dim/underline roles.",
            48,
            6,
            code_block_no_color_story,
        ),
        Story::new(
            "code-block/streaming",
            "CodeBlock streaming",
            "CodeBlock",
            "Unfinished fence with streaming cue.",
            40,
            5,
            code_block_streaming_story,
        ),
        Story::new(
            "code-block/wrap",
            "CodeBlock wrap",
            "CodeBlock",
            "Soft-wrap policy for long lines.",
            28,
            6,
            code_block_wrap_story,
        ),
        Story::new(
            "code-block/highlights",
            "CodeBlock highlights",
            "CodeBlock",
            "Diagnostic / selection highlight ranges and gutter marks.",
            48,
            6,
            code_block_highlights_story,
        ),
        Story::new(
            "markdown-view/basic",
            "Markdown view",
            "MarkdownView",
            "Editorial blocks: heading, list, task, fence, link, table.",
            48,
            14,
            markdown_view,
        ),
        Story::new(
            "markdown-view/streaming",
            "Markdown streaming fence",
            "MarkdownView",
            "Unfinished code fence with streaming cue.",
            40,
            8,
            markdown_streaming_story,
        ),
        Story::new(
            "markdown-view/table",
            "Markdown responsive table",
            "MarkdownView",
            "Pipe table with column contraction.",
            36,
            6,
            markdown_table_story,
        ),
        Story::new(
            "markdown-view/no-color",
            "Markdown no-color",
            "MarkdownView",
            "Compact headings and mono hierarchy cues.",
            40,
            10,
            markdown_no_color_story,
        ),
        Story::new(
            "sparkline/basic",
            "Sparkline",
            "Sparkline",
            "One-row density chart with threshold and selection.",
            40,
            1,
            sparkline,
        ),
        Story::new(
            "chart/basic",
            "Chart multi-series",
            "Chart",
            "Legend, axes, thresholds, selected point.",
            48,
            10,
            chart_basic,
        ),
        Story::new(
            "chart/nocolor",
            "Chart no-color",
            "Chart",
            "ASCII markers without color dependence.",
            40,
            8,
            chart_nocolor,
        ),
        Story::new(
            "gauge/basic",
            "Gauge",
            "Gauge",
            "Single-value gauge with thresholds.",
            36,
            1,
            gauge_basic,
        ),
        Story::new(
            "histogram/basic",
            "Histogram",
            "Histogram",
            "Vertical buckets with selection.",
            36,
            8,
            histogram_basic,
        ),
        Story::new(
            "bar-series/basic",
            "Bar series",
            "BarSeries",
            "Labeled horizontal bars.",
            36,
            3,
            bar_series,
        ),
        Story::new(
            "segmented-meter/basic",
            "Segmented meter",
            "SegmentedMeter",
            "Proportional stacked meter.",
            36,
            1,
            segmented_meter,
        ),
        Story::new(
            "token-meter/basic",
            "Token meter",
            "TokenMeter",
            "Usage meter with threshold roles.",
            36,
            1,
            token_meter,
        ),
        Story::new(
            "thinking-block/basic",
            "Thinking block",
            "ThinkingBlock",
            "Collapsible reasoning chrome.",
            40,
            3,
            thinking_block,
        ),
        Story::new(
            "tool-card/basic",
            "Tool card",
            "ToolCard",
            "Streaming tool invocation card.",
            44,
            4,
            tool_card,
        ),
        Story::new(
            "transcript/basic",
            "Transcript",
            "Transcript",
            "Variable-height multi-block transcript viewport.",
            48,
            10,
            transcript_basic,
        )
        .with_interactor(transcript_interactor),
        Story::new(
            "transcript/narrow",
            "Transcript narrow",
            "Transcript",
            "Transcript contraction at narrow widths.",
            24,
            8,
            transcript_basic,
        ),
        Story::new(
            "transcript/empty",
            "Transcript empty",
            "Transcript",
            "Empty-state label distinct from content.",
            40,
            6,
            transcript_empty,
        ),
        Story::new(
            "transcript/folded-follow",
            "Transcript folded follow",
            "Transcript",
            "Folded tool block + follow-tail chrome.",
            56,
            10,
            transcript_folded_follow,
        ),
        Story::new(
            "transcript/ascii-colorless",
            "Transcript ASCII colorless",
            "Transcript",
            "ASCII prefixes and colorless kind roles.",
            48,
            8,
            transcript_ascii_colorless,
        ),
        Story::new(
            "transcript/tiny",
            "Transcript tiny",
            "Transcript",
            "Tiny-terminal geometry for transcript.",
            12,
            4,
            transcript_basic,
        ),
        Story::new(
            "transcript/unicode",
            "Transcript unicode",
            "Transcript",
            "Grapheme-safe multi-block transcript lines.",
            48,
            8,
            transcript_unicode_story,
        ),
        Story::new(
            "timeline/basic",
            "Timeline",
            "Timeline",
            "Detailed activity timeline with live follow chrome.",
            64,
            10,
            timeline,
        ),
        Story::new(
            "timeline/rail",
            "Timeline rail",
            "Timeline",
            "Compact rail recipe for side panels.",
            36,
            8,
            timeline_rail,
        ),
        Story::new(
            "timeline/grouped",
            "Timeline grouped day",
            "Timeline",
            "Grouped-day headers with deploy/test events.",
            64,
            12,
            timeline_grouped,
        ),
        Story::new(
            "timeline/streaming",
            "Timeline streaming",
            "Timeline",
            "Follow-tail live stream with newest active event.",
            56,
            10,
            timeline_streaming,
        ),
        Story::new(
            "timeline/checkpoint",
            "Timeline checkpoint rows",
            "Timeline",
            "Timeline events with Checkpoint row kind (substrate).",
            52,
            8,
            timeline_checkpoint_rows,
        ),
        Story::new(
            "checkpoint-timeline/basic",
            "CheckpointTimeline",
            "CheckpointTimeline",
            "Rewindable session history — browse mode, draft preserved.",
            64,
            16,
            checkpoint_timeline_story,
        ),
        Story::new(
            "checkpoint-timeline/preview",
            "CheckpointTimeline preview",
            "CheckpointTimeline",
            "Preview checkpoint without mutation.",
            64,
            16,
            checkpoint_timeline_preview_story,
        ),
        Story::new(
            "checkpoint-timeline/confirm",
            "CheckpointTimeline confirm",
            "CheckpointTimeline",
            "Confirm restore with Cancel default focus.",
            64,
            14,
            checkpoint_timeline_confirm_story,
        ),
        Story::new(
            "checkpoint-timeline/boundaries",
            "CheckpointTimeline boundaries",
            "CheckpointTimeline",
            "Dirty / external / irreversible boundary warnings.",
            64,
            16,
            checkpoint_timeline_boundaries_story,
        ),
        Story::new(
            "prompt-composer/basic",
            "Prompt composer",
            "PromptComposer",
            "Flagship agent input: chips, mode, model, context, draft.",
            56,
            8,
            prompt_composer_basic,
        )
        .with_interactor(prompt_composer_interactor),
        Story::new(
            "prompt-composer/busy-queue",
            "Prompt composer busy",
            "PromptComposer",
            "Busy agent queues submit; stop chrome.",
            56,
            8,
            prompt_composer_busy,
        ),
        Story::new(
            "approval-queue/basic",
            "ApprovalQueue",
            "ApprovalQueue",
            "Mixed permissions/questions/plans — Open default, no bulk high-risk.",
            64,
            16,
            approval_queue_story,
        ),
        Story::new(
            "approval-queue/badge",
            "ApprovalQueue badge",
            "ApprovalQueue",
            "Compact pending badge with high-risk count.",
            40,
            1,
            approval_queue_badge_story,
        ),
        Story::new(
            "approval-queue/drawer",
            "ApprovalQueue drawer",
            "ApprovalQueue",
            "Drawer presentation list.",
            48,
            12,
            approval_queue_drawer_story,
        ),
        Story::new(
            "working-state-card/basic",
            "WorkingStateCard",
            "WorkingStateCard",
            "Public status summary — phase, files, inspect/cancel.",
            56,
            12,
            working_state_card_story,
        ),
        Story::new(
            "working-state-card/waiting",
            "WorkingStateCard waiting",
            "WorkingStateCard",
            "Waiting phase with public reason (not CoT).",
            56,
            10,
            working_state_card_waiting_story,
        ),
        Story::new(
            "working-state-card/collapsed",
            "WorkingStateCard collapsed",
            "WorkingStateCard",
            "Collapsed line for ActivityShelf composition.",
            48,
            1,
            working_state_card_collapsed_story,
        ),
        Story::new(
            "integration-status/list",
            "IntegrationStatus list",
            "IntegrationStatus",
            "MCP/plugin inventory with provenance and egress cues.",
            64,
            14,
            integration_status_list_story,
        ),
        Story::new(
            "integration-status/panel",
            "IntegrationStatus panel",
            "IntegrationStatus",
            "Diagnostic panel — permissions and egress language.",
            64,
            16,
            integration_status_panel_story,
        ),
        Story::new(
            "integration-status/badge",
            "IntegrationStatus badge",
            "IntegrationStatus",
            "Compact single-line badge.",
            48,
            1,
            integration_status_badge_story,
        ),
        Story::new(
            "agent-status-header/basic",
            "AgentStatusHeader",
            "AgentStatusHeader",
            "Action-required header with quick actions.",
            72,
            3,
            agent_status_header_story,
        ),
        Story::new(
            "agent-status-header/idle",
            "AgentStatusHeader idle",
            "AgentStatusHeader",
            "Idle connected session chrome.",
            72,
            3,
            agent_status_header_idle_story,
        ),
        Story::new(
            "agent-status-header/narrow",
            "AgentStatusHeader narrow",
            "AgentStatusHeader",
            "Contracts into StatusBar projection.",
            40,
            1,
            agent_status_header_story,
        ),
        Story::new(
            "prompt-queue/compact",
            "PromptQueue compact",
            "PromptQueue",
            "Composer summary strip while agent busy.",
            56,
            2,
            prompt_queue_compact_story,
        ),
        Story::new(
            "prompt-queue/expanded",
            "PromptQueue expanded",
            "PromptQueue",
            "Management list: reorder, edit, send, interrupt.",
            64,
            14,
            prompt_queue_expanded_story,
        ),
        Story::new(
            "prompt-queue/failed",
            "PromptQueue failed held",
            "PromptQueue",
            "Failed entry held (no auto-drain) with retry.",
            56,
            12,
            prompt_queue_failed_story,
        ),
        Story::new(
            "prompt-composer/compact",
            "Prompt composer compact",
            "PromptComposer",
            "Narrow-terminal compact presentation.",
            36,
            5,
            prompt_composer_compact,
        ),
        Story::new(
            "prompt-composer/paste-chip",
            "Prompt composer paste chip",
            "PromptComposer",
            "Large paste becomes a chip with payload (not wall-of-text).",
            56,
            8,
            prompt_composer_paste_chip,
        ),
        Story::new(
            "prompt-composer/disconnected",
            "Prompt composer disconnected",
            "PromptComposer",
            "Offline connection blocks submit with validation chrome.",
            56,
            8,
            prompt_composer_disconnected,
        ),
        Story::new(
            "prompt-composer/fullscreen",
            "Prompt composer fullscreen",
            "PromptComposer",
            "Fullscreen presentation for long prompts.",
            72,
            16,
            prompt_composer_fullscreen,
        ),
        Story::new(
            "system-picker/basic",
            "Theme picker",
            "ThemePicker",
            "Live system preset selection list.",
            36,
            6,
            theme_picker,
        )
        .with_interactor(theme_picker_interactor),
        Story::new(
            "image-surface/basic",
            "Image surface",
            "ImageSurface",
            "Placeholder image frame with protocol label.",
            28,
            8,
            image_surface,
        ),
        Story::new(
            "button/activation",
            "Button primary",
            "Button",
            "Primary button with surface input chrome.",
            28,
            3,
            button_story,
        ),
        Story::new(
            "button/variants",
            "Button variants",
            "Button",
            "Primary, secondary, quiet, outline, destructive, link, success, command.",
            56,
            10,
            button_variants_story,
        ),
        Story::new(
            "button/destructive",
            "Button destructive",
            "Button",
            "Destructive not granted default surface input.",
            28,
            3,
            button_destructive_story,
        ),
        Story::new(
            "button/toolbar",
            "Button toolbar row",
            "Button",
            "Compact quiet actions in a toolbar row.",
            48,
            3,
            button_toolbar_story,
        ),
        Story::new(
            "button/icon",
            "Icon button",
            "IconButton",
            "Icon-only with required accessible label.",
            12,
            3,
            button_icon_story,
        ),
        Story::new(
            "icon-button/toolbar",
            "IconButton toolbar",
            "IconButton",
            "Compact toolbar icons with toggle and badge.",
            24,
            3,
            icon_button_toolbar_story,
        ),
        Story::new(
            "icon-button/destructive",
            "IconButton destructive",
            "IconButton",
            "Destructive icon; not default-focused.",
            8,
            3,
            icon_button_destructive_story,
        ),
        Story::new(
            "icon-button/loading",
            "IconButton loading",
            "IconButton",
            "Loading blocks activation; distinct from disabled.",
            8,
            3,
            icon_button_loading_story,
        ),
        Story::new(
            "icon-button/row",
            "IconButton data row",
            "IconButton",
            "Compact row action with hit slop.",
            28,
            2,
            icon_button_row_story,
        ),
        Story::new(
            "button/dialog",
            "Button dialog actions",
            "Button",
            "Cancel (secondary) + Save (primary); destructive not default.",
            48,
            3,
            button_dialog_story,
        ),
        Story::new(
            "button/form",
            "Button form full-width",
            "Button",
            "Full-width primary submit for form footers.",
            40,
            3,
            button_form_story,
        ),
        Story::new(
            "button/inline",
            "Button inline link",
            "Button",
            "Link-like inline action among prose.",
            48,
            3,
            button_inline_story,
        ),
        Story::new(
            "button/pending",
            "Button pending confirm",
            "Button",
            "Destructive awaiting second Activate (?).",
            28,
            3,
            button_pending_story,
        ),
        Story::new(
            "button/no-color",
            "Button no-color",
            "Button",
            "Weight/underline affordance without color fill.",
            40,
            4,
            button_no_color_story,
        ),
        Story::new(
            "checkbox/switch",
            "Checkbox and Switch",
            "Checkbox",
            "Controlled checkbox and switch projections.",
            40,
            4,
            checkbox_switch_story,
        ),
        Story::new(
            "checkbox/states",
            "Checkbox states",
            "Checkbox",
            "Unchecked, checked, indeterminate, invalid, read-only.",
            48,
            8,
            checkbox_states_story,
        ),
        Story::new(
            "checkbox/indeterminate",
            "Checkbox indeterminate",
            "Checkbox",
            "Mixed-group parent with child list.",
            44,
            6,
            checkbox_indeterminate_story,
        ),
        Story::new(
            "checkbox/description",
            "Checkbox description",
            "Checkbox",
            "Label + secondary description row.",
            48,
            3,
            checkbox_description_story,
        ),
        Story::new(
            "checkbox/list",
            "Checkbox list",
            "Checkbox",
            "List composition with independent controlled values.",
            40,
            5,
            checkbox_list_story,
        ),
        Story::new(
            "slider/basic",
            "Slider basic",
            "Slider",
            "Horizontal volume-style slider with value text.",
            44,
            3,
            slider_basic_story,
        ),
        Story::new(
            "slider/marks",
            "Slider marks",
            "Slider",
            "Marks at 0/50/100 with labels.",
            40,
            4,
            slider_marks_story,
        ),
        Story::new(
            "slider/vertical",
            "Slider vertical",
            "Slider",
            "Vertical orientation for side panels.",
            8,
            12,
            slider_vertical_story,
        ),
        Story::new(
            "slider/numeric",
            "Slider numeric fallback",
            "Slider",
            "Tiny width falls back to numeric face.",
            8,
            2,
            slider_numeric_story,
        ),
        Story::new(
            "range-slider/basic",
            "RangeSlider basic",
            "RangeSlider",
            "Dual-thumb filter range.",
            44,
            3,
            range_slider_basic_story,
        ),
        Story::new(
            "segmented-control/basic",
            "SegmentedControl basic",
            "SegmentedControl",
            "View mode List/Grid/Table exclusive segments.",
            44,
            3,
            segmented_control_basic_story,
        ),
        Story::new(
            "segmented-control/icons",
            "SegmentedControl icons",
            "SegmentedControl",
            "Icon + badge segments for density filters.",
            40,
            3,
            segmented_control_icons_story,
        ),
        Story::new(
            "segmented-control/overflow",
            "SegmentedControl overflow",
            "SegmentedControl",
            "Low-priority segments collapse to …",
            22,
            3,
            segmented_control_overflow_story,
        ),
        Story::new(
            "segmented-control/collapsed",
            "SegmentedControl collapsed",
            "SegmentedControl",
            "Select-like trigger when very narrow.",
            14,
            3,
            segmented_control_collapsed_story,
        ),
        Story::new(
            "switch/basic",
            "Switch basic",
            "Switch",
            "Settings-row On/Off with explicit value text.",
            44,
            3,
            switch_basic_story,
        ),
        Story::new(
            "switch/loading",
            "Switch loading",
            "Switch",
            "Busy track; activation blocked.",
            40,
            2,
            switch_loading_story,
        ),
        Story::new(
            "switch/states",
            "Switch states",
            "Switch",
            "Off, on, disabled, read-only, invalid.",
            44,
            7,
            switch_states_story,
        ),
        Story::new(
            "switch/compact",
            "Switch compact",
            "Switch",
            "Leading track + label density.",
            36,
            2,
            switch_compact_story,
        ),
        Story::new(
            "radio-group/basic",
            "RadioGroup basic",
            "RadioGroup",
            "Vertical exclusive choice with legend and description.",
            44,
            8,
            radio_group_basic_story,
        ),
        Story::new(
            "radio-group/horizontal",
            "RadioGroup horizontal",
            "RadioGroup",
            "Horizontal risk/permission style choices.",
            52,
            3,
            radio_group_horizontal_story,
        ),
        Story::new(
            "radio-group/disabled",
            "RadioGroup disabled option",
            "RadioGroup",
            "Disabled option skipped by roving; selected middle.",
            40,
            6,
            radio_group_disabled_story,
        ),
        Story::new(
            "radio-group/badges",
            "RadioGroup badges",
            "RadioGroup",
            "Recommended badge + long labels.",
            48,
            7,
            radio_group_badges_story,
        ),
        Story::new(
            "data-table/toolbar",
            "DataTable",
            "DataTable",
            "Toolbar, header, and visible projected rows.",
            60,
            10,
            data_table_story,
        ),
        Story::new(
            "data-table/rows-10",
            "DataTable 10 rows",
            "DataTable",
            "Baseline 10-row projected table.",
            56,
            14,
            data_table_rows_10,
        ),
        Story::new(
            "data-table/rows-10k",
            "DataTable 10k virtual",
            "DataTable",
            "10k logical rows; only viewport slice projected.",
            56,
            12,
            data_table_rows_10k,
        ),
        Story::new(
            "data-table/rows-1m-virtual",
            "DataTable 1M virtual",
            "DataTable",
            "1M logical rows; paint only visible window.",
            56,
            12,
            data_table_rows_1m,
        ),
        Story::new(
            "data-table/wide-64",
            "DataTable wide",
            "DataTable",
            "Many columns with pin + priority (wide content).",
            72,
            10,
            data_table_wide,
        ),
        Story::new(
            "data-table/cjk",
            "DataTable CJK",
            "DataTable",
            "CJK headers and cells; display_cols safe.",
            48,
            8,
            data_table_unicode_story,
        ),
        Story::new(
            "data-table/combining",
            "DataTable combining",
            "DataTable",
            "Combining marks / grapheme-safe cells.",
            48,
            8,
            data_table_combining,
        ),
        Story::new(
            "data-table/stream-partial",
            "DataTable streaming",
            "DataTable",
            "Partial load footer for rapid streaming updates.",
            56,
            10,
            data_table_stream_partial,
        ),
        Story::new(
            "data-table/narrow-priority",
            "DataTable narrow priority",
            "DataTable",
            "contract_to_budget drops low-priority columns.",
            22,
            10,
            data_table_narrow_priority,
        ),
        Story::new(
            "data-table/loading",
            "DataTable loading",
            "DataTable",
            "Loading chrome.",
            48,
            8,
            data_table_loading,
        ),
        Story::new(
            "data-table/visidata",
            "DataTable VisiData",
            "DataTable",
            "Cell nav, sort markers, pin, multi-select chrome.",
            68,
            12,
            data_table_visidata,
        ),
        Story::new(
            "data-table/range",
            "DataTable range select",
            "DataTable",
            "Range navigation mode with cell selection.",
            56,
            10,
            data_table_range,
        ),
        Story::new(
            "data-table/groups",
            "DataTable groups",
            "DataTable",
            "Group header bands in the projected stream.",
            56,
            10,
            data_table_groups,
        ),
        Story::new(
            "data-table/edit",
            "DataTable inline edit",
            "DataTable",
            "Inline edit draft on focused cell.",
            48,
            8,
            data_table_edit,
        ),
        Story::new(
            "data-table/error",
            "DataTable error",
            "DataTable",
            "Error load state with retry hint.",
            48,
            8,
            data_table_error,
        ),
        Story::new(
            "menu/roving",
            "Menu",
            "Menu",
            "Menu with disabled item skipped by roving focus.",
            36,
            8,
            menu_story,
        ),
        Story::new(
            "tag/removable",
            "Tag removable",
            "Tag",
            "Removable attachment tag with body/remove part focus.",
            36,
            2,
            tag_removable_story,
        ),
        Story::new(
            "chip/filter",
            "Chip filter",
            "Chip",
            "Selectable filter chips with selection marks.",
            48,
            3,
            chip_filter_story,
        ),
        Story::new(
            "chip/error-loading",
            "Chip error and loading",
            "Chip",
            "Error and loading chip status.",
            40,
            3,
            chip_status_story,
        ),
        Story::new(
            "token-strip/wrap",
            "Token strip wrap",
            "TokenStrip",
            "Wrap layout for filters and paste chips.",
            36,
            4,
            token_strip_wrap_story,
        ),
        Story::new(
            "token-strip/overflow",
            "Token strip overflow",
            "TokenStrip",
            "+N overflow summary when max_visible exceeded.",
            40,
            2,
            token_strip_overflow_story,
        ),
        Story::new(
            "attachment-chip/file",
            "AttachmentChip file",
            "AttachmentChip",
            "File attachment with size meta and remove chrome.",
            48,
            3,
            attachment_chip_file_story,
        ),
        Story::new(
            "attachment-chip/broken-path",
            "AttachmentChip error",
            "AttachmentChip",
            "Broken path / validation error still removable.",
            48,
            3,
            attachment_chip_broken_story,
        ),
        Story::new(
            "attachment-chip/upload",
            "AttachmentChip upload progress",
            "AttachmentChip",
            "Upload progress percent on attachment chip.",
            48,
            2,
            attachment_chip_upload_story,
        ),
        Story::new(
            "paste-chip/large",
            "PasteChip large",
            "PasteChip",
            "Collapsed large paste with byte size.",
            48,
            4,
            paste_chip_large_story,
        ),
        Story::new(
            "paste-chip/binary",
            "PasteChip binary",
            "PasteChip",
            "Binary paste badge; no auto-insert.",
            40,
            2,
            paste_chip_binary_story,
        ),
        Story::new(
            "paste-chip/expanded",
            "PasteChip expanded",
            "PasteChip",
            "Expanded paste preview lines under chip.",
            52,
            8,
            paste_chip_expanded_story,
        ),
        Story::new(
            "attachment-strip/wrap",
            "Attachment strip wrap",
            "AttachmentChip",
            "Mixed attachments + pastes with wrap layout.",
            40,
            5,
            attachment_strip_wrap_story,
        ),
        Story::new(
            "file-mention/basic",
            "FileMention basic",
            "FileMention",
            "Inline file mention tokens with type glyphs.",
            48,
            3,
            file_mention_basic_story,
        ),
        Story::new(
            "file-mention/missing",
            "FileMention missing",
            "FileMention",
            "Missing path validity on file mention.",
            40,
            2,
            file_mention_missing_story,
        ),
        Story::new(
            "file-mention/ambiguous",
            "FileMention ambiguous",
            "FileMention",
            "Ambiguous basename with disambiguation list.",
            48,
            8,
            file_mention_ambiguous_story,
        ),
        Story::new(
            "entity-mention/agent-tool",
            "EntityMention agent tool",
            "EntityMention",
            "Agent and tool entity mention tokens.",
            48,
            3,
            entity_mention_agent_tool_story,
        ),
        Story::new(
            "entity-mention/stale",
            "EntityMention stale",
            "EntityMention",
            "Stale session entity mention.",
            40,
            2,
            entity_mention_stale_story,
        ),
        Story::new(
            "mention-draft/atomic",
            "Mention draft atomic",
            "FileMention",
            "Text plus atomic mention segments (display form).",
            52,
            3,
            mention_draft_atomic_story,
        ),
        Story::new(
            "slash-command-menu/filter",
            "SlashCommandMenu filter",
            "SlashCommandMenu",
            "Filtered slash commands with docs pane.",
            48,
            12,
            slash_command_menu_filter_story,
        ),
        Story::new(
            "slash-command-menu/loading",
            "SlashCommandMenu loading",
            "SlashCommandMenu",
            "Async plugin loading chrome.",
            40,
            10,
            slash_command_menu_loading_story,
        ),
        Story::new(
            "slash-command-menu/arguments",
            "SlashCommandMenu arguments",
            "SlashCommandMenu",
            "Nested argument completion for /model.",
            40,
            10,
            slash_command_menu_arguments_story,
        ),
        Story::new(
            "slash-command-menu/narrow",
            "SlashCommandMenu narrow",
            "SlashCommandMenu",
            "Compact slash menu on narrow width.",
            28,
            10,
            slash_command_menu_narrow_story,
        ),
        Story::new(
            "slash-command-menu/disabled",
            "SlashCommandMenu disabled",
            "SlashCommandMenu",
            "Disabled command with reason detail.",
            40,
            10,
            slash_command_menu_disabled_story,
        ),
        Story::new(
            "model-selector/compact",
            "ModelSelector compact",
            "ModelSelector",
            "Compact model status for composer chrome.",
            40,
            2,
            model_selector_compact_story,
        ),
        Story::new(
            "model-selector/expanded",
            "ModelSelector expanded",
            "ModelSelector",
            "Searchable model list with cost/context meta.",
            48,
            12,
            model_selector_expanded_story,
        ),
        Story::new(
            "model-selector/empty",
            "ModelSelector empty",
            "ModelSelector",
            "Empty filter result.",
            36,
            6,
            model_selector_empty_story,
        ),
        Story::new(
            "agent-mode-selector/ribbon",
            "AgentModeSelector ribbon",
            "AgentModeSelector",
            "Mode ribbon with FullAuto warning.",
            48,
            2,
            agent_mode_ribbon_story,
        ),
        Story::new(
            "agent-mode-selector/menu",
            "AgentModeSelector menu",
            "AgentModeSelector",
            "Mode menu with consequence text.",
            40,
            12,
            agent_mode_menu_story,
        ),
        Story::new(
            "agent-mode-selector/compact",
            "AgentModeSelector compact",
            "AgentModeSelector",
            "Compact mode badge for composer.",
            24,
            2,
            agent_mode_compact_story,
        ),
        Story::new(
            "composer-selectors/strip",
            "ComposerSelectors strip",
            "ModelSelector",
            "Composed mode · model status line.",
            48,
            2,
            composer_selectors_strip_story,
        ),
        Story::new(
            "message-thread/basic",
            "MessageThread basic",
            "MessageThread",
            "Mixed user/assistant/tool/event/error session.",
            56,
            16,
            message_thread_basic_story,
        ),
        Story::new(
            "message-thread/follow",
            "MessageThread follow",
            "MessageThread",
            "Follow-tail session at end.",
            48,
            12,
            message_thread_follow_story,
        ),
        Story::new(
            "message-thread/unread",
            "MessageThread unread",
            "MessageThread",
            "New-content indicator when not following.",
            48,
            12,
            message_thread_unread_story,
        ),
        Story::new(
            "message-thread/compact-zoom",
            "MessageThread compact zoom",
            "MessageThread",
            "Semantic zoom compact (folded tools).",
            48,
            12,
            message_thread_compact_zoom_story,
        ),
        Story::new(
            "message-thread/narrow",
            "MessageThread narrow",
            "MessageThread",
            "Narrow-terminal thread paint.",
            28,
            10,
            message_thread_narrow_story,
        ),
        Story::new(
            "message-thread/ascii",
            "MessageThread ascii",
            "MessageThread",
            "ASCII/colorless prefixes.",
            48,
            12,
            message_thread_ascii_story,
        ),
        Story::new(
            "streaming-markdown/mid-fence",
            "StreamingMarkdown mid-fence",
            "StreamingMarkdown",
            "Incomplete code fence while streaming.",
            48,
            14,
            streaming_markdown_mid_fence_story,
        ),
        Story::new(
            "streaming-markdown/complete",
            "StreamingMarkdown complete",
            "StreamingMarkdown",
            "Finished stream with closed fences.",
            48,
            14,
            streaming_markdown_complete_story,
        ),
        Story::new(
            "streaming-markdown/failed",
            "StreamingMarkdown failed",
            "StreamingMarkdown",
            "Failed phase with raw fallback cue.",
            40,
            10,
            streaming_markdown_failed_story,
        ),
        Story::new(
            "streaming-markdown/citations",
            "StreamingMarkdown citations",
            "StreamingMarkdown",
            "Sources footer + tool insertion.",
            48,
            12,
            streaming_markdown_citations_story,
        ),
        Story::new(
            "streaming-markdown/narrow",
            "StreamingMarkdown narrow",
            "StreamingMarkdown",
            "Narrow width streaming paint.",
            28,
            12,
            streaming_markdown_narrow_story,
        ),
        Story::new(
            "source-citation/inline",
            "SourceCitation inline",
            "SourceCitation",
            "Inline citation chips with dest fallback.",
            48,
            3,
            source_citation_inline_story,
        ),
        Story::new(
            "source-citation/offline",
            "SourceCitation offline",
            "SourceCitation",
            "Offline/unavailable citation chrome.",
            40,
            2,
            source_citation_offline_story,
        ),
        Story::new(
            "citation-list/expanded",
            "CitationList expanded",
            "CitationList",
            "Expanded source list with duplicates grouped.",
            56,
            12,
            citation_list_expanded_story,
        ),
        Story::new(
            "citation-list/collapsed",
            "CitationList collapsed",
            "CitationList",
            "Collapsed sources summary.",
            40,
            2,
            citation_list_collapsed_story,
        ),
        Story::new(
            "citation-list/narrow",
            "CitationList narrow",
            "CitationList",
            "Narrow citation list.",
            28,
            10,
            citation_list_narrow_story,
        ),
        Story::new(
            "tool-call-card/running",
            "ToolCallCard running",
            "ToolCallCard",
            "Running tool with risk and actor.",
            48,
            6,
            tool_call_card_running_story,
        ),
        Story::new(
            "tool-call-card/error",
            "ToolCallCard error",
            "ToolCallCard",
            "Failed tool with redacted args detail.",
            48,
            8,
            tool_call_card_error_story,
        ),
        Story::new(
            "tool-call-card/expanded",
            "ToolCallCard expanded",
            "ToolCallCard",
            "Success card expanded with action strip.",
            52,
            10,
            tool_call_card_expanded_story,
        ),
        Story::new(
            "tool-call-card/permission",
            "ToolCallCard permission",
            "ToolCallCard",
            "Waiting permission + network egress note.",
            48,
            7,
            tool_call_card_permission_story,
        ),
        Story::new(
            "tool-call-card/narrow",
            "ToolCallCard narrow",
            "ToolCallCard",
            "Narrow ASCII tool card.",
            28,
            6,
            tool_call_card_narrow_story,
        ),
        Story::new(
            "terminal-run-card/running",
            "TerminalRunCard running",
            "TerminalRunCard",
            "Live shell run with follow stream.",
            56,
            14,
            terminal_run_card_running_story,
        ),
        Story::new(
            "terminal-run-card/permission",
            "TerminalRunCard permission",
            "TerminalRunCard",
            "Proposed high-risk command awaiting permission.",
            52,
            10,
            terminal_run_card_permission_story,
        ),
        Story::new(
            "terminal-run-card/edited",
            "TerminalRunCard edited",
            "TerminalRunCard",
            "Proposed vs executed after edited approval.",
            56,
            12,
            terminal_run_card_edited_story,
        ),
        Story::new(
            "terminal-run-card/failed",
            "TerminalRunCard failed",
            "TerminalRunCard",
            "Non-zero exit with stderr.",
            52,
            12,
            terminal_run_card_failed_story,
        ),
        Story::new(
            "terminal-run-card/narrow",
            "TerminalRunCard narrow",
            "TerminalRunCard",
            "Narrow ASCII terminal run card.",
            28,
            10,
            terminal_run_card_narrow_story,
        ),
        Story::new(
            "activity-shelf/statuses",
            "ActivityShelf statuses",
            "ActivityShelf",
            "Chips prioritize action-required and blocked.",
            72,
            2,
            activity_shelf_statuses_story,
        ),
        Story::new(
            "activity-shelf/many-overflow",
            "ActivityShelf overflow",
            "ActivityShelf",
            "Many activities with +N overflow.",
            48,
            2,
            activity_shelf_overflow_story,
        ),
        Story::new(
            "activity-shelf/summary",
            "ActivityShelf summary",
            "ActivityShelf",
            "Narrow one-line summary contraction.",
            32,
            1,
            activity_shelf_summary_story,
        ),
        Story::new(
            "activity-shelf/badge",
            "ActivityShelf badge",
            "ActivityShelf",
            "Tiny badge count.",
            12,
            1,
            activity_shelf_badge_story,
        ),
        Story::new(
            "activity-shelf/statusbar",
            "ActivityShelf StatusBar",
            "ActivityShelf",
            "Projected activity summary as StatusBar slot.",
            64,
            1,
            activity_shelf_statusbar_story,
        ),
        Story::new(
            "badge/basic",
            "Badge variants",
            "Badge",
            "Neutral info success warning destructive outline count.",
            48,
            8,
            badge_story,
        ),
        Story::new(
            "badge/table",
            "Badge in table context",
            "Badge",
            "Dense row badges without fill dominance.",
            48,
            5,
            badge_table_story,
        ),
        Story::new(
            "badge/task",
            "Badge task status",
            "Badge",
            "Agent/task status chips with glyphs.",
            40,
            5,
            badge_task_story,
        ),
        Story::new(
            "badge/settings",
            "Badge settings",
            "Badge",
            "Settings category badges: outline, optional interactive filter.",
            44,
            4,
            badge_settings_story,
        ),
        Story::new(
            "badge/count",
            "Badge counts",
            "Badge",
            "Count variant with 99+ clamp.",
            32,
            3,
            badge_count_story,
        ),
        Story::new(
            "callout/basic",
            "Callout warning",
            "Callout",
            "Inline callout with tone gutter rail and non-color glyph.",
            44,
            4,
            callout_story,
        ),
        Story::new(
            "callout/tones",
            "Callout tones",
            "Callout",
            "Info/success/warning/danger/destructive/neutral compact stack.",
            48,
            12,
            callout_tones_story,
        ),
        Story::new(
            "callout/section",
            "Callout section",
            "Callout",
            "Prominent section recipe with border, source, body.",
            48,
            7,
            callout_section_story,
        ),
        Story::new(
            "alert/danger",
            "Alert danger",
            "Alert",
            "Strong danger alert with description, details, source.",
            52,
            8,
            alert_danger_story,
        ),
        Story::new(
            "alert/banner",
            "Alert banner",
            "Alert",
            "Section banner alert with actions and dismiss chrome.",
            52,
            8,
            alert_banner_story,
        ),
        Story::new(
            "alert/compact",
            "Alert compact",
            "Alert",
            "Compact inline success alert.",
            40,
            3,
            alert_compact_story,
        ),
        Story::new(
            "drawer/basic",
            "Drawer right",
            "Drawer",
            "Right-edge inspector with handle, header, body, footer.",
            28,
            14,
            drawer_story,
        ),
        Story::new(
            "drawer/left",
            "Drawer left",
            "Drawer",
            "Left-edge navigation rail drawer.",
            24,
            12,
            drawer_left_story,
        ),
        Story::new(
            "drawer/sheet",
            "Sheet bottom",
            "Sheet",
            "Bottom sheet (DrawerEdge::Bottom) for mobile-style secondary content.",
            48,
            10,
            drawer_sheet_story,
        ),
        Story::new(
            "drawer/non-modal",
            "Drawer non-modal",
            "Drawer",
            "Non-modal task rail — no focus trap; main selection preserved by host.",
            28,
            12,
            drawer_non_modal_story,
        ),
        Story::new(
            "fullscreen-viewer/basic",
            "FullscreenViewer code",
            "FullscreenViewer",
            "Fullscreen chrome over CodeBlock host body; breadcrumbs + actions.",
            56,
            18,
            fullscreen_viewer_code_story,
        ),
        Story::new(
            "fullscreen-viewer/diff",
            "FullscreenViewer diff",
            "FullscreenViewer",
            "Diff content kind with search strip open.",
            56,
            16,
            fullscreen_viewer_diff_story,
        ),
        Story::new(
            "fullscreen-viewer/log",
            "FullscreenViewer log",
            "FullscreenViewer",
            "Log stream body slot; help strip open.",
            52,
            14,
            fullscreen_viewer_log_story,
        ),
        Story::new(
            "fullscreen-viewer/zoom-badge",
            "SemanticZoom badge",
            "FullscreenViewer",
            "Compact/detail/full zoom level badges (host paints row→detail).",
            40,
            5,
            semantic_zoom_badge_story,
        ),
        Story::new(
            "fullscreen-viewer/narrow",
            "FullscreenViewer narrow",
            "FullscreenViewer",
            "Narrow terminal chrome contraction (title + body + footer).",
            28,
            12,
            fullscreen_viewer_narrow_story,
        ),
        Story::new(
            "fullscreen-viewer/unicode",
            "FullscreenViewer unicode",
            "FullscreenViewer",
            "Unicode path breadcrumbs and title safe under CJK width.",
            48,
            14,
            fullscreen_viewer_unicode_story,
        ),
        Story::new(
            "preview-card/file",
            "PreviewCard file",
            "PreviewCard",
            "File resource preview with metadata and snippet body.",
            44,
            12,
            preview_card_file_story,
        ),
        Story::new(
            "preview-card/command",
            "PreviewCard command",
            "PreviewCard",
            "Command preview with shell/cwd metadata.",
            44,
            10,
            preview_card_command_story,
        ),
        Story::new(
            "preview-card/symbol",
            "PreviewCard symbol",
            "PreviewCard",
            "Symbol definition preview.",
            44,
            10,
            preview_card_symbol_story,
        ),
        Story::new(
            "preview-card/session",
            "PreviewCard session",
            "PreviewCard",
            "Session transcript excerpt preview.",
            44,
            10,
            preview_card_session_story,
        ),
        Story::new(
            "preview-card/loading",
            "PreviewCard loading",
            "PreviewCard",
            "Async loading chrome while host fetch is in flight.",
            36,
            8,
            preview_card_loading_story,
        ),
        Story::new(
            "preview-card/error",
            "PreviewCard error",
            "PreviewCard",
            "Error state for failed preview fetch.",
            36,
            8,
            preview_card_error_story,
        ),
        Story::new(
            "preview-card/pinned",
            "PreviewCard pinned",
            "PreviewCard",
            "Pinned sticky preview (popover policy; pin mark).",
            44,
            12,
            preview_card_pinned_story,
        ),
        Story::new(
            "text/basic",
            "Text basic",
            "Text",
            "Semantic body text with preserve-bg paint.",
            40,
            3,
            text_basic_story,
        ),
        Story::new(
            "text/spans",
            "Text spans",
            "Text",
            "Multi-role spans, emphasis, annotation, highlight.",
            48,
            3,
            text_spans_story,
        ),
        Story::new(
            "text/wrap",
            "Text wrap",
            "Text",
            "Soft-wrap on display columns.",
            28,
            6,
            text_wrap_story,
        ),
        Story::new(
            "text/truncate",
            "Text truncate",
            "Text",
            "End ellipsis truncation.",
            24,
            1,
            text_truncate_story,
        ),
        Story::new(
            "text/unicode",
            "Text unicode",
            "Text",
            "CJK, combining marks, emoji, tabs.",
            36,
            3,
            text_unicode_story,
        ),
        Story::new(
            "text/narrow",
            "Text narrow",
            "Text",
            "Narrow clip and center align.",
            14,
            3,
            text_narrow_story,
        ),
        Story::new(
            "heading/basic",
            "Heading H1 reading",
            "Heading",
            "H1 with underline weight and rule row.",
            40,
            3,
            heading_story,
        ),
        Story::new(
            "heading/levels",
            "Heading levels",
            "Heading",
            "H1 / H2 / H3 hierarchy in one stack.",
            40,
            6,
            heading_levels_story,
        ),
        Story::new(
            "heading/compact",
            "Heading compact",
            "Heading",
            "ASCII # prefixes for no-color hierarchy.",
            36,
            3,
            heading_compact_story,
        ),
        Story::new(
            "icon/browser",
            "Icon glyph browser",
            "Icon",
            "Semantic glyph catalog by group (Unicode).",
            56,
            18,
            icon_browser_story,
        ),
        Story::new(
            "icon/ascii",
            "Icon ASCII fallback",
            "Icon",
            "Same semantic names under GlyphSet::Ascii.",
            48,
            10,
            icon_ascii_story,
        ),
        Story::new(
            "icon/enhanced",
            "Icon enhanced",
            "Icon",
            "Enhanced profile (richer file/status cells).",
            48,
            6,
            icon_enhanced_story,
        ),
        Story::new(
            "icon/labeled",
            "Icon with labels",
            "Icon",
            "Glyph + text so meaning is not glyph-only.",
            40,
            5,
            icon_labeled_story,
        ),
        Story::new(
            "avatar-glyph/basic",
            "AvatarGlyph initials",
            "AvatarGlyph",
            "Two-cell initials avatar.",
            12,
            3,
            avatar_glyph_basic_story,
        ),
        Story::new(
            "avatar-glyph/compact",
            "AvatarGlyph compact",
            "AvatarGlyph",
            "One-cell compact faces.",
            16,
            3,
            avatar_glyph_compact_story,
        ),
        Story::new(
            "avatar-glyph/presence",
            "AvatarGlyph presence",
            "AvatarGlyph",
            "Face plus presence status cell.",
            16,
            3,
            avatar_glyph_presence_story,
        ),
        Story::new(
            "avatar-glyph/no-color",
            "AvatarGlyph no-color",
            "AvatarGlyph",
            "Monochrome face remains legible.",
            12,
            3,
            avatar_glyph_no_color_story,
        ),
        Story::new(
            "identity/basic",
            "Identity row",
            "Identity",
            "Avatar, name, secondary, role badge.",
            48,
            3,
            identity_basic_story,
        ),
        Story::new(
            "identity/thread",
            "Identity thread roles",
            "Identity",
            "User / agent / service in a thread list.",
            48,
            5,
            identity_thread_story,
        ),
        Story::new(
            "highlighted-text/basic",
            "HighlightedText matches",
            "HighlightedText",
            "Substring matches with keep-first truncation.",
            40,
            4,
            highlighted_text_basic_story,
        ),
        Story::new(
            "highlighted-text/selected",
            "HighlightedText selected",
            "HighlightedText",
            "Selected row visual with focused match.",
            40,
            2,
            highlighted_text_selected_story,
        ),
        Story::new(
            "highlighted-text/no-color",
            "HighlightedText no-color",
            "HighlightedText",
            "Monochrome match emphasis (underline/bold).",
            40,
            2,
            highlighted_text_no_color_story,
        ),
        Story::new(
            "highlighted-text/overlap",
            "HighlightedText overlaps",
            "HighlightedText",
            "Overlapping soft/match/focused ranges.",
            36,
            2,
            highlighted_text_overlap_story,
        ),
        Story::new(
            "label/basic",
            "Label basic",
            "Label",
            "Required label with help description.",
            40,
            3,
            label_basic_story,
        ),
        Story::new(
            "label/states",
            "Label states",
            "Label",
            "Required, optional, disabled, invalid, warning.",
            44,
            6,
            label_states_story,
        ),
        Story::new(
            "label/layouts",
            "Label layouts",
            "Label",
            "Stacked vs compact caption recipes.",
            40,
            5,
            label_layouts_story,
        ),
        Story::new(
            "label/narrow",
            "Label narrow",
            "Label",
            "Description contracts before the label.",
            20,
            3,
            label_narrow_story,
        ),
        Story::new(
            "description/kinds",
            "Description kinds",
            "Description",
            "Help, error, warning, meta descriptions.",
            40,
            5,
            description_kinds_story,
        ),
        Story::new(
            "kbd/basic",
            "Kbd keycap",
            "Kbd",
            "Keycap form for a compact chord.",
            16,
            3,
            kbd_story,
        ),
        Story::new(
            "key-value-list/basic",
            "KeyValueList reading",
            "KeyValueList",
            "Groups, status, copy, secret, and link rows.",
            48,
            12,
            key_value_list_basic_story,
        ),
        Story::new(
            "key-value-list/dense",
            "KeyValueList dense",
            "KeyValueList",
            "Dense recipe for settings drawers.",
            40,
            8,
            key_value_list_dense_story,
        ),
        Story::new(
            "key-value-list/stacked",
            "KeyValueList stacked",
            "KeyValueList",
            "Forced stacked anatomy (narrow/read).",
            28,
            12,
            key_value_list_stacked_story,
        ),
        Story::new(
            "key-value-list/secret",
            "KeyValueList secret",
            "KeyValueList",
            "Redacted secret with reveal affordance.",
            40,
            4,
            key_value_list_secret_story,
        ),
        Story::new(
            "kbd/platform",
            "Kbd platforms",
            "Kbd",
            "Emacs / spelled / Mac symbol modifier styles.",
            48,
            4,
            kbd_platform_story,
        ),
        Story::new(
            "link/basic",
            "Link external",
            "Link",
            "External URL with visible destination and external cue.",
            56,
            3,
            link_basic_story,
        ),
        Story::new(
            "link/no-hyperlink",
            "Link no-hyperlink",
            "Link",
            "OSC 8 off — destination always painted as text fallback.",
            56,
            3,
            link_no_hyperlink_story,
        ),
        Story::new(
            "link/no-color",
            "Link no-color",
            "Link",
            "No-color path: underline + focus bold; destination still visible.",
            56,
            3,
            link_no_color_story,
        ),
        Story::new(
            "link/app-route",
            "Link app route",
            "Link",
            "Application-routed link (no OSC 8, no external cue).",
            40,
            3,
            link_app_route_story,
        ),
        Story::new(
            "action-link/basic",
            "ActionLink",
            "ActionLink",
            "Inline action with link chrome and visible risk note.",
            40,
            3,
            action_link_story,
        ),
        Story::new(
            "ansi-text/basic",
            "AnsiText SGR",
            "AnsiText",
            "SGR colors and styles from command output.",
            48,
            5,
            ansi_text_basic_story,
        ),
        Story::new(
            "ansi-text/no-color",
            "AnsiText no-color",
            "AnsiText",
            "No-color mode keeps bold/dim cues only.",
            48,
            4,
            ansi_text_no_color_story,
        ),
        Story::new(
            "ansi-text/cr-bs",
            "AnsiText CR/BS",
            "AnsiText",
            "Carriage return overwrite and backspace erase.",
            32,
            3,
            ansi_text_cr_bs_story,
        ),
        Story::new(
            "ansi-text/hyperlink",
            "AnsiText OSC-8",
            "AnsiText",
            "OSC 8 hyperlink styled as Link role.",
            40,
            2,
            ansi_text_hyperlink_story,
        ),
        Story::new(
            "shortcut-hint/footer",
            "ShortcutHint footer",
            "ShortcutHint",
            "Footer form derived from a Keymap binding.",
            40,
            3,
            shortcut_hint_footer_story,
        ),
        Story::new(
            "shortcut-hint/inline",
            "ShortcutHint inline docs",
            "ShortcutHint",
            "Inline documentation form with command first.",
            44,
            3,
            shortcut_hint_inline_story,
        ),
        Story::new(
            "shortcut-hint/narrow",
            "ShortcutHint narrow",
            "ShortcutHint",
            "Command contracts before chord when narrow.",
            14,
            2,
            shortcut_hint_narrow_story,
        ),
        Story::new(
            "paragraph/basic",
            "Paragraph body",
            "Paragraph",
            "Body paragraph wrap.",
            40,
            4,
            paragraph_story,
        ),
        Story::new(
            "paragraph/quote",
            "Paragraph quote",
            "Paragraph",
            "Block quote with hanging gutter.",
            40,
            4,
            paragraph_quote_story,
        ),
        Story::new(
            "paragraph/list",
            "Paragraph list",
            "Paragraph",
            "List and ordered items with hanging wrap.",
            40,
            6,
            paragraph_list_story,
        ),
        Story::new(
            "paragraph/reading",
            "Paragraph reading",
            "Paragraph",
            "Reading recipe body + quote.",
            44,
            6,
            paragraph_reading_story,
        ),
        Story::new(
            "surface/basic",
            "Surface raised",
            "Surface",
            "Raised surface with border (card body).",
            24,
            6,
            surface_story,
        ),
        Story::new(
            "surface/ladder",
            "Surface recipe ladder",
            "Surface",
            "Canvas · inset · raised · overlay · interactive · focused · selected · warning · destructive.",
            72,
            22,
            surface_ladder_story,
        ),
        Story::new(
            "surface/focused",
            "Surface focused",
            "Surface",
            "Focused recipe uses Role::BorderFocused, not border weight.",
            28,
            6,
            surface_focused_story,
        ),
        Story::new(
            "surface/terminal-default",
            "Surface terminal-default",
            "Surface",
            "Canvas + transparent fill; compatible with host terminal bg / no-color.",
            28,
            5,
            surface_terminal_default_story,
        ),
        Story::new(
            "separator/basic",
            "Separator quiet",
            "Separator",
            "Quiet horizontal rule.",
            48,
            1,
            separator_story,
        ),
        Story::new(
            "separator/strong",
            "Separator strong",
            "Separator",
            "Strong horizontal rule.",
            48,
            1,
            separator_strong_story,
        ),
        Story::new(
            "separator/labeled",
            "Separator labeled",
            "Separator",
            "Labeled horizontal divider.",
            48,
            1,
            separator_labeled_story,
        ),
        Story::new(
            "separator/section-break",
            "Separator section break",
            "Separator",
            "Band spacing recipe: pad + rule + pad.",
            48,
            3,
            separator_section_break_story,
        ),
        Story::new(
            "separator/vertical",
            "Separator vertical",
            "Separator",
            "Vertical quiet rule.",
            3,
            8,
            separator_vertical_story,
        ),
        Story::new(
            "separator/focus-zone",
            "Separator focus zone",
            "Separator",
            "Non-color zone boundary (not BorderFocused).",
            48,
            1,
            separator_focus_zone_story,
        ),
        Story::new(
            "popover/basic",
            "Popover basic",
            "Popover",
            "Anchored non-modal popover with header/body slots.",
            32,
            10,
            popover_story,
        ),
        Story::new(
            "popover/slots",
            "Popover slots",
            "Popover",
            "Header / body / footer slots without Panel.",
            36,
            12,
            popover_slots_story,
        ),
        Story::new(
            "popover/modal",
            "Popover modal",
            "Popover",
            "Modal modality: focus trap + dim chrome.",
            36,
            12,
            popover_modal_story,
        ),
        Story::new(
            "app-shell/workbench",
            "AppShell workbench",
            "AppShell",
            "Header + sidebar + main + inspector + footer; multi-pane recipe.",
            100,
            28,
            app_shell_workbench_story,
        ),
        Story::new(
            "app-shell/dashboard",
            "AppShell dashboard",
            "AppShell",
            "Metrics strip + main + log + footer ops recipe.",
            80,
            28,
            app_shell_dashboard_story,
        ),
        Story::new(
            "app-shell/master-detail",
            "AppShell master-detail",
            "AppShell",
            "Sidebar master list + detail main + footer.",
            80,
            24,
            app_shell_master_detail_story,
        ),
        Story::new(
            "app-shell/minimal",
            "AppShell minimal",
            "AppShell",
            "Main + footer only (inline/tiny tools).",
            48,
            12,
            app_shell_minimal_story,
        ),
        Story::new(
            "app-shell/narrow-drawer",
            "AppShell narrow drawer",
            "AppShell",
            "Workbench under drawer/single-pane pressure; collapsed rails listed.",
            48,
            20,
            app_shell_narrow_story,
        ),
        Story::new(
            "app-shell/offline",
            "AppShell offline lifecycle",
            "AppShell",
            "Offline lifecycle compact chrome on workbench.",
            80,
            24,
            app_shell_offline_story,
        ),
        Story::new(
            "agent-workbench/basic",
            "Agent workbench",
            "AgentWorkbench",
            "North-star block: TaskRail, transcript, ActivityShelf, PromptComposer.",
            100,
            28,
            agent_workbench_basic,
        ),
        Story::new(
            "agent-workbench/tool-running",
            "Agent workbench tools",
            "AgentWorkbench",
            "Tool-running: TaskRail + ActivityShelf concurrent jobs.",
            100,
            28,
            agent_workbench_tool_running,
        ),
        Story::new(
            "agent-workbench/permission",
            "Agent workbench permission",
            "AgentWorkbench",
            "PermissionPrompt overlay — default-deny; draft preserved.",
            80,
            24,
            agent_workbench_permission,
        ),
        Story::new(
            "agent-workbench/plan",
            "Agent workbench plan",
            "AgentWorkbench",
            "PlanReview overlay on composed workbench.",
            80,
            24,
            agent_workbench_plan,
        ),
        Story::new(
            "agent-workbench/diff",
            "Agent workbench diff",
            "AgentWorkbench",
            "DiffReview overlay on composed workbench.",
            80,
            24,
            agent_workbench_diff,
        ),
        Story::new(
            "agent-workbench/session",
            "Agent workbench session",
            "AgentWorkbench",
            "SessionPicker overlay — cancel keeps composer draft.",
            80,
            24,
            agent_workbench_session,
        ),
        Story::new(
            "agent-workbench/multi-agent",
            "Agent workbench multi-agent",
            "AgentWorkbench",
            "Multi-agent: subagent tasks + activity shelf waiting chips.",
            100,
            28,
            agent_workbench_multi_agent,
        ),
        Story::new(
            "agent-workbench/narrow",
            "Agent workbench narrow",
            "AgentWorkbench",
            "Narrow density — activity strip, no west rail.",
            50,
            20,
            agent_workbench_narrow,
        ),
        Story::new(
            "agent-workbench/tiny",
            "Agent workbench tiny",
            "AgentWorkbench",
            "Tiny density — transcript + composer only.",
            30,
            16,
            agent_workbench_tiny,
        ),
        Story::new(
            "agent-workbench/ascii",
            "Agent workbench ASCII",
            "AgentWorkbench",
            "ASCII glyph preference on elevated surfaces.",
            100,
            28,
            agent_workbench_ascii,
        ),
        Story::new(
            "agent-workbench/no-color",
            "Agent workbench no-color",
            "AgentWorkbench",
            "Colorless preference — mono status cues.",
            100,
            28,
            agent_workbench_no_color,
        ),
        Story::new(
            "database-workbench/basic",
            "Database workbench",
            "DatabaseWorkbench",
            "Flagship DB composition: connections, schema, query, results, inspector.",
            120,
            36,
            database_workbench_basic,
        ),
        Story::new(
            "database-workbench/disconnected",
            "Database workbench disconnected",
            "DatabaseWorkbench",
            "Disconnected gate blocks run; offline chrome.",
            100,
            28,
            database_workbench_disconnected,
        ),
        Story::new(
            "database-workbench/error",
            "Database workbench error",
            "DatabaseWorkbench",
            "Query error + failed transaction status projection.",
            100,
            28,
            database_workbench_error,
        ),
        Story::new(
            "database-workbench/running",
            "Database workbench running",
            "DatabaseWorkbench",
            "In-flight query run chrome.",
            100,
            28,
            database_workbench_running,
        ),
        Story::new(
            "database-workbench/narrow",
            "Database workbench narrow",
            "DatabaseWorkbench",
            "Narrow density — inspector collapsed.",
            70,
            24,
            database_workbench_narrow,
        ),
        Story::new(
            "database-workbench/unicode",
            "Database workbench unicode",
            "DatabaseWorkbench",
            "Unicode titles / schema path paint.",
            100,
            28,
            database_workbench_unicode,
        ),
        Story::new(
            "git-workbench/basic",
            "Git workbench",
            "GitWorkbench",
            "Dirty repo: files, diff, history, branches, output.",
            120,
            36,
            git_workbench_basic,
        ),
        Story::new(
            "git-workbench/conflict",
            "Git workbench conflict",
            "GitWorkbench",
            "Conflict status + diagnostics chrome.",
            100,
            28,
            git_workbench_conflict,
        ),
        Story::new(
            "git-workbench/narrow",
            "Git workbench narrow",
            "GitWorkbench",
            "Narrow density — history/branches collapsed.",
            70,
            24,
            git_workbench_narrow,
        ),
        Story::new(
            "git-workbench/fullscreen-diff",
            "Git workbench fullscreen diff",
            "GitWorkbench",
            "Fullscreen diff promotion.",
            100,
            28,
            git_workbench_fullscreen,
        ),
        Story::new(
            "git-workbench/unicode",
            "Git workbench unicode",
            "GitWorkbench",
            "Unicode-safe diff path paint.",
            100,
            28,
            git_workbench_unicode,
        ),
        Story::new(
            "git-workbench/clean",
            "Git workbench clean",
            "GitWorkbench",
            "Clean worktree fixture paint path.",
            100,
            28,
            git_workbench_clean,
        ),
        Story::new(
            "git-workbench/empty",
            "Git workbench empty",
            "GitWorkbench",
            "Empty repo / no files projected.",
            80,
            20,
            git_workbench_empty,
        ),
        Story::new(
            "observability-dashboard/basic",
            "Observability dashboard",
            "ObservabilityDashboard",
            "Live logs + events + metrics composition.",
            120,
            36,
            observability_dashboard_basic,
        ),
        Story::new(
            "observability-dashboard/failure",
            "Observability reconnect/dropped",
            "ObservabilityDashboard",
            "Reconnecting acquisition + dropped-data warning.",
            100,
            28,
            observability_dashboard_failure,
        ),
        Story::new(
            "observability-dashboard/narrow",
            "Observability narrow",
            "ObservabilityDashboard",
            "Narrow density — inspector collapsed.",
            70,
            24,
            observability_dashboard_narrow,
        ),
        Story::new(
            "observability-dashboard/unicode",
            "Observability unicode",
            "ObservabilityDashboard",
            "Unicode-safe log line paint path.",
            100,
            28,
            observability_dashboard_unicode,
        ),
        Story::new(
            "file-manager/basic",
            "File manager",
            "FileManager",
            "Browse tree + preview + operation queue.",
            120,
            36,
            file_manager_basic,
        ),
        Story::new(
            "file-manager/conflict",
            "File manager conflict",
            "FileManager",
            "Conflict dialog + queue progress/failure.",
            100,
            28,
            file_manager_conflict,
        ),
        Story::new(
            "file-manager/narrow",
            "File manager narrow",
            "FileManager",
            "Narrow density — queue collapsed; preview drawer.",
            70,
            24,
            file_manager_narrow,
        ),
        Story::new(
            "file-manager/unicode",
            "File manager unicode",
            "FileManager",
            "Unicode path/name paint path.",
            100,
            28,
            file_manager_unicode,
        ),
        Story::new(
            "project-launcher/basic",
            "Project launcher home",
            "ProjectLauncher",
            "Home: projects + sessions + preview.",
            120,
            36,
            project_launcher_basic,
        ),
        Story::new(
            "project-launcher/stale",
            "Project launcher stale",
            "ProjectLauncher",
            "Missing/stale paths + offline status.",
            100,
            28,
            project_launcher_stale,
        ),
        Story::new(
            "project-launcher/narrow",
            "Project launcher narrow",
            "ProjectLauncher",
            "Narrow density — preview collapsed.",
            70,
            24,
            project_launcher_narrow,
        ),
        Story::new(
            "project-launcher/inline",
            "Project launcher inline",
            "ProjectLauncher",
            "Inline quick launcher (search + list).",
            64,
            16,
            project_launcher_inline,
        ),
        Story::new(
            "project-launcher/unicode",
            "Project launcher unicode",
            "ProjectLauncher",
            "Unicode project name paint path.",
            100,
            28,
            project_launcher_unicode,
        ),
        Story::new(
            "help-center/basic",
            "Help center full docs",
            "HelpCenter",
            "Full multi-pane help + keyboard + commands.",
            120,
            40,
            help_center_basic,
        ),
        Story::new(
            "help-center/compact",
            "Help center compact",
            "HelpCenter",
            "Compact overlay mode.",
            72,
            20,
            help_center_compact,
        ),
        Story::new(
            "help-center/narrow",
            "Help center narrow",
            "HelpCenter",
            "Narrow density — keyboard collapsed.",
            70,
            24,
            help_center_narrow,
        ),
        Story::new(
            "help-center/doctor",
            "Help center doctor",
            "HelpCenter",
            "Diagnostics pane with doctor findings.",
            100,
            32,
            help_center_doctor,
        ),
        Story::new(
            "help-center/unicode",
            "Help center unicode",
            "HelpCenter",
            "Unicode topic paint path.",
            100,
            28,
            help_center_unicode,
        ),
        Story::new(
            "error-recovery/basic",
            "Error recovery full",
            "ErrorRecovery",
            "Full recovery surface with actions.",
            100,
            32,
            error_recovery_basic,
        ),
        Story::new(
            "error-recovery/redacted",
            "Error recovery redacted crash",
            "ErrorRecovery",
            "Crash report with secrets redacted.",
            100,
            32,
            error_recovery_redacted,
        ),
        Story::new(
            "error-recovery/inline",
            "Error recovery inline fallback",
            "ErrorRecovery",
            "Inline fallback when full-screen compromised.",
            64,
            12,
            error_recovery_inline,
        ),
        Story::new(
            "error-recovery/unicode",
            "Error recovery unicode",
            "ErrorRecovery",
            "Unicode summary paint path.",
            80,
            24,
            error_recovery_unicode,
        ),
        Story::new(
            "input-otp/basic",
            "Input OTP",
            "InputOtp",
            "Six-digit OTP slots with label.",
            32,
            4,
            input_otp_basic,
        ),
        Story::new(
            "carousel/basic",
            "Carousel",
            "Carousel",
            "Three-slide keyboard carousel.",
            48,
            10,
            carousel_basic,
        ),
        Story::new(
            "input-group/basic",
            "Input group",
            "InputGroup",
            "URL prefix + field + suffix action.",
            48,
            3,
            input_group_basic,
        ),
        Story::new(
            "permission-prompt/basic",
            "Permission prompt",
            "PermissionPrompt",
            "Default-deny permission surface.",
            48,
            10,
            permission_prompt_story,
        ),
        Story::new(
            "permission-prompt/low-read",
            "Permission low-risk read",
            "PermissionPrompt",
            "Low-risk file read with prior grant hint; focus Deny.",
            52,
            11,
            permission_prompt_low_read,
        ),
        Story::new(
            "permission-prompt/destructive-nested",
            "Permission destructive nested",
            "PermissionPrompt",
            "High-risk shell via main > subagent > MCP with DESTRUCTIVE banner.",
            56,
            12,
            permission_prompt_destructive_nested,
        ),
        Story::new(
            "permission-prompt/egress",
            "Permission data egress",
            "PermissionPrompt",
            "Critical network egress warning; default-deny focus.",
            56,
            12,
            permission_prompt_egress,
        ),
        Story::new(
            "mode-ribbon/basic",
            "Mode ribbon",
            "ModeRibbon",
            "Agent mode strip.",
            48,
            3,
            mode_ribbon_story,
        ),
        Story::new(
            "plan-review/basic",
            "Plan review",
            "PlanReview",
            "Markdown plan with tasks, risks, safe action focus.",
            56,
            16,
            plan_review_story,
        ),
        Story::new(
            "plan-review/high-risk",
            "PlanReview high risk",
            "PlanReview",
            "Critical plan defaults action focus to Abandon.",
            56,
            14,
            plan_review_high_risk_story,
        ),
        Story::new(
            "plan-review/diff",
            "PlanReview version diff",
            "PlanReview",
            "Structural changes between plan revisions.",
            56,
            14,
            plan_review_diff_story,
        ),
        Story::new(
            "plan-review/comments",
            "PlanReview comments",
            "PlanReview",
            "Line comments and notes pane.",
            56,
            14,
            plan_review_comments_story,
        ),
        Story::new(
            "question-flow/basic",
            "Question flow",
            "QuestionFlow",
            "Multi-step interview with provenance.",
            52,
            14,
            question_flow_story,
        ),
        Story::new(
            "question-flow/review",
            "QuestionFlow review",
            "QuestionFlow",
            "Review-before-submit phase.",
            52,
            12,
            question_flow_review_story,
        ),
        Story::new(
            "question-flow/multi",
            "QuestionFlow multi",
            "QuestionFlow",
            "Multi-select question step.",
            48,
            12,
            question_flow_multi_story,
        ),
        Story::new(
            "question-flow/text",
            "QuestionFlow free text",
            "QuestionFlow",
            "Free-text question.",
            48,
            8,
            question_flow_text_story,
        ),
        Story::new(
            "session-picker/basic",
            "Session picker",
            "SessionPicker",
            "Create/resume sessions with search, pin, preview.",
            64,
            16,
            session_picker_story,
        ),
        Story::new(
            "session-picker/search",
            "SessionPicker search",
            "SessionPicker",
            "Filtered session list by query.",
            56,
            14,
            session_picker_search_story,
        ),
        Story::new(
            "session-picker/confirm",
            "SessionPicker delete confirm",
            "SessionPicker",
            "Safe delete confirm with Cancel default.",
            56,
            12,
            session_picker_confirm_story,
        ),
        Story::new(
            "session-picker/empty",
            "SessionPicker empty",
            "SessionPicker",
            "Empty catalog — create prompt.",
            48,
            10,
            session_picker_empty_story,
        ),
        Story::new(
            "connection-manager/full",
            "ConnectionManager full",
            "ConnectionManager",
            "Full management view: list, status, detail, offline banner.",
            72,
            18,
            connection_manager_full_story,
        ),
        Story::new(
            "connection-manager/launcher",
            "ConnectionManager launcher",
            "ConnectionManager",
            "Compact launcher presentation for quick connect.",
            48,
            12,
            connection_manager_launcher_story,
        ),
        Story::new(
            "connection-manager/empty",
            "ConnectionManager empty",
            "ConnectionManager",
            "Empty catalog — add prompt.",
            48,
            10,
            connection_manager_empty_story,
        ),
        Story::new(
            "connection-manager/error",
            "ConnectionManager error",
            "ConnectionManager",
            "Connection with last_error + diagnostic projection.",
            64,
            14,
            connection_manager_error_story,
        ),
        Story::new(
            "connection-manager/secret",
            "ConnectionManager secret form",
            "ConnectionManager",
            "Add form with masked secret field (never paints raw secret).",
            56,
            14,
            connection_manager_secret_story,
        ),
        Story::new(
            "connection-manager/confirm",
            "ConnectionManager delete confirm",
            "ConnectionManager",
            "Safe delete confirm with Cancel default focus.",
            56,
            12,
            connection_manager_confirm_story,
        ),
        Story::new(
            "task-rail/basic",
            "Task rail",
            "TaskRail",
            "Grouped ActivityModel rail with needs-input first.",
            32,
            16,
            task_rail_story,
        ),
        Story::new(
            "task-rail/input",
            "TaskRail needs input",
            "TaskRail",
            "Permission/input prioritized selection.",
            32,
            14,
            task_rail_input_story,
        ),
        Story::new(
            "task-rail/filter",
            "TaskRail filter",
            "TaskRail",
            "Filter query in title chrome.",
            32,
            14,
            task_rail_filter_story,
        ),
        Story::new(
            "task-rail/drawer-narrow",
            "TaskRail drawer-narrow",
            "TaskRail",
            "Narrow width recommends Drawer presentation.",
            20,
            14,
            task_rail_drawer_narrow_story,
        ),
        Story::new(
            "task-rail/statusbar",
            "TaskRail StatusBar",
            "TaskRail",
            "Collapsed rail summary as StatusBar slot.",
            64,
            1,
            task_rail_statusbar_story,
        ),
        Story::new(
            "subagent-card/running",
            "SubagentCard running",
            "SubagentCard",
            "Live nested subagent with provenance.",
            56,
            12,
            subagent_card_running_story,
        ),
        Story::new(
            "subagent-card/failed",
            "SubagentCard failed",
            "SubagentCard",
            "Artifact failed result.",
            52,
            10,
            subagent_card_failed_story,
        ),
        Story::new(
            "subagent-card/nested-provenance",
            "SubagentCard nested",
            "SubagentCard",
            "Deep provenance hops + depth.",
            56,
            12,
            subagent_card_nested_story,
        ),
        Story::new(
            "subagent-card/row",
            "SubagentCard compact row",
            "SubagentCard",
            "Compact row presentation.",
            56,
            1,
            subagent_card_row_story,
        ),
        Story::new(
            "subagent-card/result",
            "SubagentCard result",
            "SubagentCard",
            "Success artifact promote-ready.",
            52,
            10,
            subagent_card_result_story,
        ),
        Story::new(
            "background-tasks/mixed-statuses",
            "BackgroundTaskPanel mixed",
            "BackgroundTaskPanel",
            "Runners, lost, reconnect, failed with output.",
            88,
            20,
            background_tasks_mixed_story,
        ),
        Story::new(
            "background-tasks/clear-completed",
            "BackgroundTaskPanel clear",
            "BackgroundTaskPanel",
            "Hide completed filter view.",
            72,
            16,
            background_tasks_clear_story,
        ),
        Story::new(
            "background-tasks/rail",
            "BackgroundTaskPanel rail",
            "BackgroundTaskPanel",
            "Compact rail list.",
            26,
            14,
            background_tasks_rail_story,
        ),
        Story::new(
            "background-tasks/lost",
            "BackgroundTaskPanel lost",
            "BackgroundTaskPanel",
            "Lost process detail chrome.",
            80,
            16,
            background_tasks_lost_story,
        ),
        Story::new(
            "background-tasks/dropped-lines",
            "BackgroundTaskPanel drops",
            "BackgroundTaskPanel",
            "Bounded history dropped-line banner.",
            80,
            16,
            background_tasks_dropped_story,
        ),
        Story::new(
            "context-meter/low-mid-high",
            "ContextMeter pressure",
            "ContextMeter",
            "Low / mid / high token pressure.",
            48,
            6,
            context_meter_pressure_story,
        ),
        Story::new(
            "context-meter/indeterminate",
            "ContextMeter indeterminate",
            "ContextMeter",
            "Unknown totals — never claim 100%.",
            40,
            4,
            context_meter_indeterminate_story,
        ),
        Story::new(
            "context-meter/approximate",
            "ContextMeter approximate",
            "ContextMeter",
            "Approximate estimates with ~ formatting.",
            44,
            6,
            context_meter_approximate_story,
        ),
        Story::new(
            "context-meter/expanded",
            "ContextMeter expanded",
            "ContextMeter",
            "Breakdown, threshold, compact action.",
            48,
            12,
            context_meter_expanded_story,
        ),
        Story::new(
            "context-meter/mono",
            "ContextMeter mono",
            "ContextMeter",
            "Monochrome density bar.",
            40,
            3,
            context_meter_mono_story,
        ),
        Story::new(
            "context-meter/bytes",
            "ContextMeter bytes",
            "ContextMeter",
            "Non-token byte budget.",
            40,
            4,
            context_meter_bytes_story,
        ),
        Story::new(
            "blocks/form-wizard",
            "FormWizard",
            "FormWizard",
            "Multi-step wizard with stepper and nav.",
            56,
            12,
            form_wizard_story,
        ),
        Story::new(
            "form-wizard/review",
            "FormWizard review",
            "FormWizard",
            "Review phase before submit.",
            56,
            12,
            form_wizard_review_story,
        ),
        Story::new(
            "form-wizard/failure",
            "FormWizard failure",
            "FormWizard",
            "Failure/retry surface.",
            56,
            10,
            form_wizard_failure_story,
        ),
        Story::new(
            "form-wizard/resume",
            "FormWizard resume",
            "FormWizard",
            "Restored progress mid-flow.",
            56,
            12,
            form_wizard_resume_story,
        ),
        Story::new(
            "settings-screen/basic",
            "Settings screen",
            "SettingsScreen",
            "Searchable settings: Sidebar, SearchInput, Form, footer.",
            100,
            28,
            settings_screen_basic,
        ),
        Story::new(
            "settings-screen/search",
            "Settings search filter",
            "SettingsScreen",
            "Search focused with filtered categories.",
            100,
            28,
            settings_screen_search,
        ),
        Story::new(
            "settings-screen/validation",
            "Settings validation",
            "SettingsScreen",
            "Required field error + dirty modified cue.",
            100,
            28,
            settings_screen_validation,
        ),
        Story::new(
            "settings-screen/conflicts",
            "Settings conflicts",
            "SettingsScreen",
            "Keybinding conflict + restart-required banner.",
            100,
            28,
            settings_screen_conflicts,
        ),
        Story::new(
            "settings-screen/theme",
            "Settings theme preview",
            "SettingsScreen",
            "ThemePicker body mode with live paint system.",
            100,
            28,
            settings_screen_theme,
        ),
        Story::new(
            "settings-screen/keybinding",
            "Settings keybinding",
            "SettingsScreen",
            "KeybindingRecorder integrated body.",
            80,
            16,
            settings_screen_keybinding,
        ),
        Story::new(
            "settings-screen/narrow",
            "Settings narrow",
            "SettingsScreen",
            "Narrow density with category drawer open.",
            60,
            22,
            settings_screen_narrow,
        ),
        Story::new(
            "settings-screen/tiny",
            "Settings tiny",
            "SettingsScreen",
            "Tiny density — body + search only.",
            40,
            16,
            settings_screen_tiny,
        ),
        Story::new(
            "settings-screen/no-results",
            "Settings no results",
            "SettingsScreen",
            "Empty search guidance.",
            80,
            20,
            settings_screen_no_results,
        ),
        Story::new(
            "settings-screen/help",
            "Settings keyboard help",
            "SettingsScreen",
            "Keyboard help overlay.",
            80,
            22,
            settings_screen_help,
        ),
        Story::new(
            "setup-wizard/welcome",
            "Setup wizard welcome",
            "SetupWizard",
            "Dense first-run welcome — no marketing splash.",
            80,
            22,
            setup_wizard_welcome,
        ),
        Story::new(
            "setup-wizard/capability",
            "Setup wizard capability",
            "SetupWizard",
            "Terminal capability doctor projection.",
            80,
            22,
            setup_wizard_capability,
        ),
        Story::new(
            "setup-wizard/account",
            "Setup wizard account",
            "SetupWizard",
            "Account form step with validation gate.",
            80,
            22,
            setup_wizard_account,
        ),
        Story::new(
            "setup-wizard/permission",
            "Setup wizard trust",
            "SetupWizard",
            "Permissions / trust step.",
            80,
            20,
            setup_wizard_permission,
        ),
        Story::new(
            "setup-wizard/theme",
            "Setup wizard theme",
            "SetupWizard",
            "Theme preview step.",
            80,
            22,
            setup_wizard_theme,
        ),
        Story::new(
            "setup-wizard/summary",
            "Setup wizard summary",
            "SetupWizard",
            "Review summary before finish.",
            80,
            22,
            setup_wizard_summary,
        ),
        Story::new(
            "setup-wizard/recovery",
            "Setup wizard recovery",
            "SetupWizard",
            "Failure recovery with retry.",
            72,
            16,
            setup_wizard_recovery,
        ),
        Story::new(
            "setup-wizard/inline",
            "Setup wizard inline",
            "SetupWizard",
            "Inline mode in a tight pane.",
            48,
            14,
            setup_wizard_inline,
        ),
        Story::new(
            "setup-wizard/resume",
            "Setup wizard resume",
            "SetupWizard",
            "Restored WizardProgress mid-flow.",
            80,
            22,
            setup_wizard_resume,
        ),
        Story::new(
            "setup-wizard/cancel-confirm",
            "Setup wizard cancel confirm",
            "SetupWizard",
            "Safe two-step cancel strip.",
            72,
            16,
            setup_wizard_cancel_confirm,
        ),
        // --- Catalog state-axis stories (narrow/unicode/depth) ---
        Story::new(
            "action-bar/narrow",
            "Narrow ActionBar",
            "ActionBar",
            "Narrow-terminal geometry for ActionBar (22 cols).",
            22,
            2,
            action_bar,
        ),
        Story::new(
            "button-group/narrow",
            "Narrow ButtonGroup",
            "ButtonGroup",
            "Overflow + keep primary at 20 cols.",
            20,
            2,
            button_group_overflow_story,
        ),
        Story::new(
            "button-group/unicode",
            "Unicode ButtonGroup",
            "ButtonGroup",
            "Unicode action labels.",
            40,
            2,
            button_group_unicode_story,
        ),
        Story::new(
            "toggle/narrow",
            "Narrow Toggle",
            "Toggle",
            "Compact toggle at 16 cols.",
            16,
            2,
            toggle_pressed_story,
        ),
        Story::new(
            "toggle/unicode",
            "Unicode Toggle",
            "Toggle",
            "Unicode face label.",
            28,
            2,
            toggle_unicode_story,
        ),
        Story::new(
            "toggle-group/narrow",
            "Narrow ToggleGroup",
            "ToggleGroup",
            "Overflow recipe at 16 cols.",
            16,
            2,
            toggle_group_overflow_story,
        ),
        Story::new(
            "toggle-group/unicode",
            "Unicode ToggleGroup",
            "ToggleGroup",
            "CJK labels in multi group.",
            36,
            2,
            toggle_group_unicode_story,
        ),
        Story::new(
            "action-bar/unicode",
            "Unicode ActionBar",
            "ActionBar",
            "Unicode-safe paint path for ActionBar (CJK/emoji-capable layout).",
            48,
            2,
            action_bar_unicode_story,
        ),
        Story::new(
            "backdrop/narrow",
            "Narrow Backdrop",
            "Backdrop",
            "Narrow-terminal geometry for Backdrop (17 cols).",
            17,
            4,
            backdrop,
        ),
        Story::new(
            "backdrop/unicode",
            "Unicode Backdrop",
            "Backdrop",
            "Unicode-safe paint path for Backdrop (CJK/emoji-capable layout).",
            34,
            4,
            backdrop_unicode_story,
        ),
        Story::new(
            "badge/narrow",
            "Narrow Badge",
            "Badge",
            "Narrow-terminal geometry for Badge (12 cols).",
            12,
            3,
            badge_story,
        ),
        Story::new(
            "badge/unicode",
            "Unicode Badge",
            "Badge",
            "Unicode-safe paint path for Badge (CJK/emoji-capable layout).",
            28,
            3,
            badge_unicode_story,
        ),
        Story::new(
            "banner/narrow",
            "Narrow Banner",
            "Banner",
            "Narrow-terminal geometry for Banner (20 cols).",
            20,
            1,
            banner,
        ),
        Story::new(
            "banner/unicode",
            "Unicode Banner",
            "Banner",
            "Unicode-safe paint path for Banner (CJK/emoji-capable layout).",
            40,
            1,
            banner_unicode_story,
        ),
        Story::new(
            "bar-series/narrow",
            "Narrow BarSeries",
            "BarSeries",
            "Narrow-terminal geometry for BarSeries (18 cols).",
            18,
            3,
            bar_series,
        ),
        Story::new(
            "bar-series/unicode",
            "Unicode BarSeries",
            "BarSeries",
            "Unicode-safe paint path for BarSeries (CJK/emoji-capable layout).",
            36,
            3,
            bar_series_unicode_story,
        ),
        Story::new(
            "button/narrow",
            "Narrow Button",
            "Button",
            "Narrow-terminal geometry for Button (16 cols).",
            16,
            3,
            button_story,
        ),
        Story::new(
            "button/unicode",
            "Unicode Button",
            "Button",
            "Unicode-safe paint path for Button (CJK/emoji-capable layout).",
            32,
            3,
            button_unicode_story,
        ),
        Story::new(
            "icon-button/narrow",
            "Narrow IconButton",
            "IconButton",
            "Hit slop preserved at 3 cols; glyph unstretched.",
            5,
            2,
            button_icon_story,
        ),
        Story::new(
            "icon-button/unicode",
            "Unicode IconButton",
            "IconButton",
            "Unicode glyph with ASCII fallback available.",
            16,
            2,
            icon_button_toolbar_story,
        ),
        Story::new(
            "callout/narrow",
            "Narrow Callout",
            "Callout",
            "Narrow-terminal geometry for Callout (20 cols).",
            20,
            4,
            callout_story,
        ),
        Story::new(
            "callout/unicode",
            "Unicode Callout",
            "Callout",
            "Unicode-safe paint path for Callout (CJK/emoji-capable layout).",
            40,
            4,
            callout_unicode_story,
        ),
        Story::new(
            "checkbox/narrow",
            "Narrow Checkbox",
            "Checkbox",
            "Narrow-terminal geometry for Checkbox (20 cols).",
            20,
            4,
            checkbox_switch_story,
        ),
        Story::new(
            "checkbox/unicode",
            "Unicode Checkbox",
            "Checkbox",
            "Unicode-safe paint path for Checkbox (CJK/emoji-capable layout).",
            40,
            4,
            checkbox_unicode_story,
        ),
        Story::new(
            "radio-group/narrow",
            "Narrow RadioGroup",
            "RadioGroup",
            "Horizontal collapses to vertical under stack_below.",
            22,
            6,
            radio_group_narrow_story,
        ),
        Story::new(
            "radio-group/unicode",
            "Unicode RadioGroup",
            "RadioGroup",
            "CJK labels and descriptions.",
            40,
            6,
            radio_group_unicode_story,
        ),
        Story::new(
            "slider/narrow",
            "Narrow Slider",
            "Slider",
            "Numeric fallback at narrow width.",
            8,
            2,
            slider_numeric_story,
        ),
        Story::new(
            "slider/unicode",
            "Unicode Slider",
            "Slider",
            "CJK label with track.",
            36,
            3,
            slider_unicode_story,
        ),
        Story::new(
            "range-slider/narrow",
            "Narrow RangeSlider",
            "RangeSlider",
            "Range numeric fallback.",
            8,
            2,
            range_slider_narrow_story,
        ),
        Story::new(
            "segmented-control/narrow",
            "Narrow SegmentedControl",
            "SegmentedControl",
            "Overflow at 20 cols.",
            20,
            2,
            segmented_control_overflow_story,
        ),
        Story::new(
            "segmented-control/unicode",
            "Unicode SegmentedControl",
            "SegmentedControl",
            "CJK mode labels.",
            36,
            2,
            segmented_control_unicode_story,
        ),
        Story::new(
            "switch/narrow",
            "Narrow Switch",
            "Switch",
            "Settings row at 22 cols; keep track.",
            22,
            2,
            switch_basic_story,
        ),
        Story::new(
            "switch/unicode",
            "Unicode Switch",
            "Switch",
            "CJK settings label.",
            36,
            3,
            switch_unicode_story,
        ),
        Story::new(
            "choice-dialog/narrow",
            "Narrow ChoiceDialog",
            "ChoiceDialog",
            "Narrow-terminal geometry for ChoiceDialog (22 cols).",
            22,
            7,
            choice_dialog,
        ),
        Story::new(
            "choice-dialog/unicode",
            "Unicode ChoiceDialog",
            "ChoiceDialog",
            "Unicode-safe paint path for ChoiceDialog (CJK/emoji-capable layout).",
            48,
            7,
            choice_dialog_unicode_story,
        ),
        Story::new(
            "code-block/narrow",
            "Narrow CodeBlock",
            "CodeBlock",
            "Narrow-terminal geometry for CodeBlock (20 cols).",
            20,
            5,
            code_block,
        ),
        Story::new(
            "code-block/unicode",
            "Unicode CodeBlock",
            "CodeBlock",
            "Unicode-safe paint path for CodeBlock (CJK/emoji-capable layout).",
            40,
            5,
            code_block_unicode_story,
        ),
        Story::new(
            "command-palette/narrow",
            "Narrow CommandPalette",
            "CommandPalette",
            "Narrow-terminal geometry for CommandPalette (21 cols).",
            21,
            10,
            command_palette,
        ),
        Story::new(
            "command-palette/unicode",
            "Unicode CommandPalette",
            "CommandPalette",
            "Unicode-safe paint path for CommandPalette (CJK/emoji-capable layout).",
            42,
            10,
            command_palette_unicode_story,
        ),
        Story::new(
            "data-table/narrow",
            "Narrow DataTable",
            "DataTable",
            "Narrow-terminal geometry for DataTable (22 cols).",
            22,
            10,
            data_table_story,
        ),
        Story::new(
            "data-table/unicode",
            "Unicode DataTable",
            "DataTable",
            "Unicode-safe paint path for DataTable (CJK/emoji-capable layout).",
            60,
            10,
            data_table_unicode_story,
        ),
        Story::new(
            "design-inspector/narrow",
            "Narrow DesignInspector",
            "DesignInspector",
            "Narrow-terminal geometry for DesignInspector (22 cols).",
            22,
            4,
            design_inspector,
        ),
        Story::new(
            "design-inspector/unicode",
            "Unicode DesignInspector",
            "DesignInspector",
            "Unicode-safe paint path for DesignInspector (CJK/emoji-capable layout).",
            48,
            4,
            design_inspector_unicode_story,
        ),
        Story::new(
            "detail-table/narrow",
            "Narrow DetailTable",
            "DetailTable",
            "Narrow-terminal geometry for DetailTable (22 cols).",
            22,
            5,
            detail_table,
        ),
        Story::new(
            "dialog/unicode",
            "Unicode Dialog",
            "Dialog",
            "Unicode-safe paint path for Dialog (CJK/emoji-capable layout).",
            48,
            7,
            dialog_unicode_story,
        ),
        Story::new(
            "diff/narrow",
            "Narrow DiffView",
            "DiffView",
            "Narrow forces unified mode (22 cols).",
            22,
            8,
            diff_basic,
        ),
        Story::new(
            "diff/unicode",
            "Unicode DiffView",
            "DiffView",
            "Unicode-safe paint path for DiffView (CJK/emoji-capable layout).",
            64,
            10,
            diff_unicode_story,
        ),
        Story::new(
            "drawer/narrow",
            "Narrow Drawer",
            "Drawer",
            "Compact/fullscreen contraction path (16 cols).",
            16,
            12,
            drawer_story,
        ),
        Story::new(
            "drawer/unicode",
            "Unicode Drawer",
            "Drawer",
            "Unicode-safe paint path for Drawer (CJK/emoji-capable layout).",
            28,
            12,
            drawer_unicode_story,
        ),
        Story::new(
            "empty-state/narrow",
            "Narrow EmptyState",
            "EmptyState",
            "Narrow-terminal geometry contracts toward inline form.",
            18,
            5,
            empty_state_narrow_story,
        ),
        Story::new(
            "empty-state/unicode",
            "Unicode EmptyState",
            "EmptyState",
            "Unicode-safe paint path for EmptyState (CJK/emoji-capable layout).",
            40,
            8,
            empty_state_unicode_story,
        ),
        Story::new(
            "empty-state/logs",
            "EmptyState logs",
            "EmptyState",
            "Log stream empty recipe.",
            42,
            9,
            empty_state_logs_story,
        ),
        Story::new(
            "empty-state/projects",
            "EmptyState projects",
            "EmptyState",
            "Projects first-use recipe.",
            42,
            10,
            empty_state_projects_story,
        ),
        Story::new(
            "error-view/narrow",
            "Narrow ErrorView",
            "ErrorView",
            "Narrow-terminal geometry contracts toward inline error.",
            18,
            5,
            error_view_narrow_story,
        ),
        Story::new(
            "error-view/unicode",
            "Unicode ErrorView",
            "ErrorView",
            "Unicode-safe paint path for ErrorView (CJK/emoji-capable layout).",
            40,
            8,
            error_view_unicode_story,
        ),
        Story::new(
            "form/unicode",
            "Unicode Form",
            "Form",
            "Unicode-safe paint path for Form (CJK/emoji-capable layout).",
            68,
            12,
            form_unicode_story,
        ),
        Story::new(
            "blocks/narrow",
            "Narrow FormWizard",
            "FormWizard",
            "Narrow-terminal single-step layout (20 cols).",
            20,
            10,
            form_wizard_story,
        ),
        Story::new(
            "blocks/unicode",
            "Unicode FormWizard",
            "FormWizard",
            "Unicode-safe paint path for FormWizard (CJK/emoji-capable layout).",
            56,
            12,
            form_wizard_unicode_story,
        ),
        Story::new(
            "heading/narrow",
            "Narrow Heading",
            "Heading",
            "Narrow-terminal geometry for Heading (20 cols).",
            20,
            3,
            heading_story,
        ),
        Story::new(
            "heading/unicode",
            "Unicode Heading",
            "Heading",
            "Unicode-safe paint path for Heading (CJK/emoji-capable layout).",
            40,
            3,
            heading_unicode_story,
        ),
        Story::new(
            "hint-bar/narrow",
            "Narrow HintBar",
            "HintBar",
            "Narrow-terminal geometry for HintBar (21 cols).",
            21,
            2,
            hint_bar,
        ),
        Story::new(
            "hint-bar/unicode",
            "Unicode HintBar",
            "HintBar",
            "Unicode-safe paint path for HintBar (CJK/emoji-capable layout).",
            42,
            2,
            hint_bar_unicode_story,
        ),
        Story::new(
            "image-surface/narrow",
            "Narrow ImageSurface",
            "ImageSurface",
            "Narrow-terminal geometry for ImageSurface (14 cols).",
            14,
            8,
            image_surface,
        ),
        Story::new(
            "image-surface/unicode",
            "Unicode ImageSurface",
            "ImageSurface",
            "Unicode-safe paint path for ImageSurface (CJK/emoji-capable layout).",
            28,
            8,
            image_surface_unicode_story,
        ),
        Story::new(
            "jump-overlay/narrow",
            "Narrow JumpOverlay",
            "JumpOverlay",
            "Narrow-terminal geometry for JumpOverlay (20 cols).",
            20,
            6,
            jump_overlay,
        ),
        Story::new(
            "jump-overlay/unicode",
            "Unicode JumpOverlay",
            "JumpOverlay",
            "Unicode-safe paint path for JumpOverlay (CJK/emoji-capable layout).",
            40,
            6,
            jump_overlay_unicode_story,
        ),
        Story::new(
            "ansi-text/narrow",
            "Narrow AnsiText",
            "AnsiText",
            "Narrow-terminal geometry for AnsiText (16 cols).",
            16,
            4,
            ansi_text_basic_story,
        ),
        Story::new(
            "ansi-text/unicode",
            "Unicode AnsiText",
            "AnsiText",
            "Unicode-safe paint path for AnsiText.",
            40,
            3,
            ansi_text_unicode_story,
        ),
        Story::new(
            "avatar-glyph/narrow",
            "Narrow AvatarGlyph",
            "AvatarGlyph",
            "One-cell footprint in tight gutters.",
            8,
            2,
            avatar_glyph_compact_story,
        ),
        Story::new(
            "avatar-glyph/unicode",
            "Unicode AvatarGlyph",
            "AvatarGlyph",
            "Unicode-safe initials seed.",
            12,
            2,
            avatar_glyph_unicode_story,
        ),
        Story::new(
            "highlighted-text/narrow",
            "Narrow HighlightedText",
            "HighlightedText",
            "Match-preserving truncate at 16 cols.",
            16,
            2,
            highlighted_text_basic_story,
        ),
        Story::new(
            "highlighted-text/unicode",
            "Unicode HighlightedText",
            "HighlightedText",
            "Grapheme-safe matches on CJK/emoji.",
            32,
            2,
            highlighted_text_unicode_story,
        ),
        Story::new(
            "identity/narrow",
            "Narrow Identity",
            "Identity",
            "Compact identity in narrow columns.",
            20,
            2,
            identity_narrow_story,
        ),
        Story::new(
            "identity/unicode",
            "Unicode Identity",
            "Identity",
            "Unicode display names.",
            40,
            2,
            identity_unicode_story,
        ),
        Story::new(
            "key-value-list/narrow",
            "Narrow KeyValueList",
            "KeyValueList",
            "Auto-stacks on narrow terminals (24 cols).",
            24,
            12,
            key_value_list_basic_story,
        ),
        Story::new(
            "key-value-list/unicode",
            "Unicode KeyValueList",
            "KeyValueList",
            "Unicode-safe keys and values.",
            40,
            6,
            key_value_list_unicode_story,
        ),
        Story::new(
            "kbd/narrow",
            "Narrow Kbd",
            "Kbd",
            "Narrow-terminal geometry for Kbd (12 cols).",
            12,
            3,
            kbd_story,
        ),
        Story::new(
            "kbd/unicode",
            "Unicode Kbd",
            "Kbd",
            "Unicode-safe paint path for Kbd (CJK/emoji-capable layout).",
            28,
            3,
            kbd_unicode_story,
        ),
        Story::new(
            "link/narrow",
            "Narrow Link",
            "Link",
            "Narrow-terminal geometry for Link (16 cols).",
            16,
            2,
            link_basic_story,
        ),
        Story::new(
            "link/unicode",
            "Unicode Link",
            "Link",
            "Unicode-safe paint path for Link (CJK/emoji-capable layout).",
            48,
            2,
            link_unicode_story,
        ),
        Story::new(
            "action-link/narrow",
            "Narrow ActionLink",
            "ActionLink",
            "Narrow-terminal geometry for ActionLink (14 cols).",
            14,
            2,
            action_link_story,
        ),
        Story::new(
            "action-link/unicode",
            "Unicode ActionLink",
            "ActionLink",
            "Unicode-safe paint path for ActionLink.",
            36,
            2,
            action_link_unicode_story,
        ),
        Story::new(
            "loading-view/narrow",
            "Narrow LoadingView",
            "LoadingView",
            "Narrow-terminal geometry for LoadingView (18 cols).",
            18,
            3,
            loading_view,
        ),
        Story::new(
            "loading-view/unicode",
            "Unicode LoadingView",
            "LoadingView",
            "Unicode-safe paint path for LoadingView (CJK/emoji-capable layout).",
            36,
            3,
            loading_view_unicode_story,
        ),
        Story::new(
            "log-pane/narrow",
            "Narrow LogPane",
            "LogPane",
            "Narrow-terminal geometry for LogPane (22 cols).",
            22,
            8,
            log_pane,
        ),
        Story::new(
            "log-pane/unicode",
            "Unicode LogPane",
            "LogPane",
            "Unicode-safe paint path for LogPane (CJK/emoji-capable layout).",
            52,
            8,
            log_pane_unicode_story,
        ),
        Story::new(
            "markdown-view/narrow",
            "Narrow MarkdownView",
            "MarkdownView",
            "Narrow-terminal geometry for MarkdownView (20 cols).",
            20,
            6,
            markdown_view,
        ),
        Story::new(
            "markdown-view/unicode",
            "Unicode MarkdownView",
            "MarkdownView",
            "Unicode-safe paint path for MarkdownView (CJK/emoji-capable layout).",
            40,
            6,
            markdown_view_unicode_story,
        ),
        Story::new(
            "menu/narrow",
            "Narrow Menu",
            "Menu",
            "Narrow-terminal geometry for Menu (18 cols).",
            18,
            8,
            menu_story,
        ),
        Story::new(
            "menu/unicode",
            "Unicode Menu",
            "Menu",
            "Unicode-safe paint path for Menu (CJK/emoji-capable layout).",
            36,
            8,
            menu_unicode_story,
        ),
        Story::new(
            "message-dialog/narrow",
            "Narrow MessageDialog",
            "MessageDialog",
            "Narrow-terminal geometry for MessageDialog (22 cols).",
            22,
            8,
            message_dialog,
        ),
        Story::new(
            "message-dialog/unicode",
            "Unicode MessageDialog",
            "MessageDialog",
            "Unicode-safe paint path for MessageDialog (CJK/emoji-capable layout).",
            52,
            8,
            message_dialog_unicode_story,
        ),
        Story::new(
            "mode-ribbon/narrow",
            "Narrow ModeRibbon",
            "ModeRibbon",
            "Narrow-terminal geometry for ModeRibbon (22 cols).",
            22,
            3,
            mode_ribbon_story,
        ),
        Story::new(
            "mode-ribbon/unicode",
            "Unicode ModeRibbon",
            "ModeRibbon",
            "Unicode-safe paint path for ModeRibbon (CJK/emoji-capable layout).",
            48,
            3,
            mode_ribbon_unicode_story,
        ),
        Story::new(
            "panel/narrow",
            "Narrow Panel",
            "Panel",
            "Title contraction drops trailing then subtitle then leading.",
            18,
            6,
            panel_narrow_story,
        ),
        Story::new(
            "panel/unicode",
            "Unicode Panel",
            "Panel",
            "Unicode-safe paint path for Panel (CJK/emoji-capable layout).",
            48,
            7,
            panel_unicode_story,
        ),
        Story::new(
            "paragraph/narrow",
            "Narrow Paragraph",
            "Paragraph",
            "Narrow-terminal geometry for Paragraph (20 cols).",
            20,
            4,
            paragraph_story,
        ),
        Story::new(
            "paragraph/unicode",
            "Unicode Paragraph",
            "Paragraph",
            "Unicode-safe paint path for Paragraph (CJK/emoji-capable layout).",
            40,
            4,
            paragraph_unicode_story,
        ),
        Story::new(
            "permission-prompt/narrow",
            "Narrow PermissionPrompt",
            "PermissionPrompt",
            "Narrow-terminal geometry for PermissionPrompt (22 cols).",
            22,
            10,
            permission_prompt_story,
        ),
        Story::new(
            "permission-prompt/unicode",
            "Unicode PermissionPrompt",
            "PermissionPrompt",
            "Unicode-safe paint path for PermissionPrompt (CJK/emoji-capable layout).",
            48,
            10,
            permission_prompt_unicode_story,
        ),
        Story::new(
            "plan-review/narrow",
            "Narrow PlanReview",
            "PlanReview",
            "Narrow-terminal geometry for PlanReview (22 cols).",
            22,
            14,
            plan_review_story,
        ),
        Story::new(
            "plan-review/unicode",
            "Unicode PlanReview",
            "PlanReview",
            "Unicode-safe paint path for PlanReview (CJK/emoji-capable layout).",
            48,
            14,
            plan_review_unicode_story,
        ),
        Story::new(
            "popover/narrow",
            "Narrow Popover",
            "Popover",
            "Contract path toward drawer/fullscreen (14 cols).",
            14,
            12,
            popover_narrow_story,
        ),
        Story::new(
            "popover/unicode",
            "Unicode Popover",
            "Popover",
            "Unicode-safe paint path for Popover (CJK/emoji-capable layout).",
            32,
            10,
            popover_unicode_story,
        ),
        Story::new(
            "prompt-queue/narrow",
            "Narrow PromptQueue",
            "PromptQueue",
            "Narrow-terminal geometry for PromptQueue (22 cols).",
            22,
            12,
            prompt_queue_expanded_story,
        ),
        Story::new(
            "approval-queue/narrow",
            "Narrow ApprovalQueue",
            "ApprovalQueue",
            "Narrow-terminal geometry (22 cols).",
            22,
            12,
            approval_queue_story,
        ),
        Story::new(
            "approval-queue/unicode",
            "Unicode ApprovalQueue",
            "ApprovalQueue",
            "Unicode-safe paint path.",
            48,
            10,
            approval_queue_unicode_story,
        ),
        Story::new(
            "working-state-card/narrow",
            "Narrow WorkingStateCard",
            "WorkingStateCard",
            "Narrow-terminal geometry (22 cols).",
            22,
            10,
            working_state_card_story,
        ),
        Story::new(
            "working-state-card/unicode",
            "Unicode WorkingStateCard",
            "WorkingStateCard",
            "Unicode-safe paint path.",
            40,
            8,
            working_state_card_unicode_story,
        ),
        Story::new(
            "integration-status/narrow",
            "Narrow IntegrationStatus",
            "IntegrationStatus",
            "Narrow-terminal geometry (22 cols).",
            22,
            12,
            integration_status_list_story,
        ),
        Story::new(
            "integration-status/unicode",
            "Unicode IntegrationStatus",
            "IntegrationStatus",
            "Unicode-safe paint path.",
            48,
            10,
            integration_status_unicode_story,
        ),
        Story::new(
            "agent-status-header/unicode",
            "Unicode AgentStatusHeader",
            "AgentStatusHeader",
            "Unicode-safe paint path for AgentStatusHeader.",
            48,
            3,
            agent_status_header_unicode_story,
        ),
        Story::new(
            "prompt-queue/unicode",
            "Unicode PromptQueue",
            "PromptQueue",
            "Unicode-safe paint path for PromptQueue.",
            48,
            10,
            prompt_queue_unicode_story,
        ),
        Story::new(
            "prompt-composer/narrow",
            "Narrow PromptComposer",
            "PromptComposer",
            "Narrow-terminal geometry for PromptComposer (22 cols).",
            22,
            8,
            prompt_composer_basic,
        ),
        Story::new(
            "prompt-composer/unicode",
            "Unicode PromptComposer",
            "PromptComposer",
            "Unicode-safe paint path for PromptComposer (CJK/emoji-capable layout).",
            56,
            8,
            prompt_composer_unicode_story,
        ),
        Story::new(
            "question-flow/narrow",
            "Narrow QuestionFlow",
            "QuestionFlow",
            "Narrow-terminal geometry for QuestionFlow (22 cols).",
            22,
            8,
            question_flow_story,
        ),
        Story::new(
            "question-flow/unicode",
            "Unicode QuestionFlow",
            "QuestionFlow",
            "Unicode-safe paint path for QuestionFlow (CJK/emoji-capable layout).",
            48,
            8,
            question_flow_unicode_story,
        ),
        Story::new(
            "segmented-meter/narrow",
            "Narrow SegmentedMeter",
            "SegmentedMeter",
            "Narrow-terminal geometry for SegmentedMeter (18 cols).",
            18,
            1,
            segmented_meter,
        ),
        Story::new(
            "segmented-meter/unicode",
            "Unicode SegmentedMeter",
            "SegmentedMeter",
            "Unicode-safe paint path for SegmentedMeter (CJK/emoji-capable layout).",
            36,
            1,
            segmented_meter_unicode_story,
        ),
        Story::new(
            "separator/narrow",
            "Narrow SeparatorLine",
            "SeparatorLine",
            "Narrow-terminal geometry for SeparatorLine (15 cols).",
            15,
            3,
            separator_story,
        ),
        Story::new(
            "separator/unicode",
            "Unicode SeparatorLine",
            "SeparatorLine",
            "Unicode-safe paint path for SeparatorLine (CJK/emoji-capable layout).",
            30,
            3,
            separator_unicode_story,
        ),
        Story::new(
            "session-picker/narrow",
            "Narrow SessionPicker",
            "SessionPicker",
            "Narrow-terminal geometry for SessionPicker (20 cols).",
            20,
            14,
            session_picker_story,
        ),
        Story::new(
            "session-picker/unicode",
            "Unicode SessionPicker",
            "SessionPicker",
            "Unicode-safe paint path for SessionPicker (CJK/emoji-capable layout).",
            48,
            12,
            session_picker_unicode_story,
        ),
        Story::new(
            "connection-manager/narrow",
            "Narrow ConnectionManager",
            "ConnectionManager",
            "Narrow-terminal geometry for ConnectionManager (20 cols).",
            20,
            14,
            connection_manager_full_story,
        ),
        Story::new(
            "connection-manager/unicode",
            "Unicode ConnectionManager",
            "ConnectionManager",
            "Unicode-safe paint path for ConnectionManager (CJK/emoji-capable layout).",
            48,
            12,
            connection_manager_unicode_story,
        ),
        Story::new(
            "skeleton/narrow",
            "Narrow Skeleton",
            "Skeleton",
            "Narrow-terminal geometry for Skeleton (16 cols).",
            16,
            4,
            skeleton,
        ),
        Story::new(
            "skeleton/unicode",
            "Unicode Skeleton",
            "Skeleton",
            "Unicode-safe paint path for Skeleton (CJK/emoji-capable layout).",
            32,
            4,
            skeleton_unicode_story,
        ),
        Story::new(
            "sparkline/narrow",
            "Narrow Sparkline",
            "Sparkline",
            "Narrow-terminal geometry for Sparkline (16 cols).",
            16,
            1,
            sparkline,
        ),
        Story::new(
            "sparkline/unicode",
            "Unicode Sparkline",
            "Sparkline",
            "Unicode-safe paint path for Sparkline (CJK/emoji-capable layout).",
            32,
            1,
            sparkline_unicode_story,
        ),
        Story::new(
            "split-pane/narrow",
            "Narrow SplitPane",
            "SplitPane",
            "Narrow-terminal geometry for SplitPane (22 cols).",
            22,
            10,
            split_pane,
        ),
        Story::new(
            "split-pane/unicode",
            "Unicode SplitPane",
            "SplitPane",
            "Unicode-safe paint path for SplitPane (CJK/emoji-capable layout).",
            68,
            10,
            split_pane_unicode_story,
        ),
        Story::new(
            "status-bar/unicode",
            "Unicode StatusBar",
            "StatusBar",
            "Unicode-safe paint path for StatusBar (CJK/emoji-capable layout).",
            60,
            1,
            status_bar_unicode_story,
        ),
        Story::new(
            "surface/narrow",
            "Narrow Surface",
            "Surface",
            "Narrow-terminal geometry for Surface (12 cols).",
            12,
            5,
            surface_story,
        ),
        Story::new(
            "surface/unicode",
            "Unicode Surface",
            "Surface",
            "Unicode-safe paint path for Surface (CJK/emoji-capable layout).",
            28,
            5,
            surface_unicode_story,
        ),
        Story::new(
            "tabs/unicode",
            "Unicode Tabs",
            "Tabs",
            "Unicode-safe paint path for Tabs (CJK/emoji-capable layout).",
            52,
            2,
            tabs_unicode_story,
        ),
        Story::new(
            "task-rail/narrow",
            "Narrow TaskRail",
            "TaskRail",
            "Narrow-terminal geometry for TaskRail (14 cols).",
            14,
            10,
            task_rail_story,
        ),
        Story::new(
            "task-rail/unicode",
            "Unicode TaskRail",
            "TaskRail",
            "Unicode-safe paint path for TaskRail (CJK/emoji-capable layout).",
            28,
            10,
            task_rail_unicode_story,
        ),
        Story::new(
            "text-input/narrow",
            "Narrow TextInput",
            "TextInput",
            "Narrow-terminal geometry for TextInput (14 cols).",
            14,
            1,
            text_input_unicode,
        ),
        Story::new(
            "system-picker/narrow",
            "Narrow ThemePicker",
            "ThemePicker",
            "Narrow-terminal geometry for ThemePicker (18 cols).",
            18,
            6,
            theme_picker,
        ),
        Story::new(
            "system-picker/unicode",
            "Unicode ThemePicker",
            "ThemePicker",
            "Unicode-safe paint path for ThemePicker (CJK/emoji-capable layout).",
            36,
            6,
            theme_picker_unicode_story,
        ),
        Story::new(
            "thinking-block/narrow",
            "Narrow ThinkingBlock",
            "ThinkingBlock",
            "Narrow-terminal geometry for ThinkingBlock (20 cols).",
            20,
            3,
            thinking_block,
        ),
        Story::new(
            "thinking-block/unicode",
            "Unicode ThinkingBlock",
            "ThinkingBlock",
            "Unicode-safe paint path for ThinkingBlock (CJK/emoji-capable layout).",
            40,
            3,
            thinking_block_unicode_story,
        ),
        Story::new(
            "checkpoint-timeline/narrow",
            "Narrow CheckpointTimeline",
            "CheckpointTimeline",
            "Narrow-terminal geometry for CheckpointTimeline (22 cols).",
            22,
            14,
            checkpoint_timeline_story,
        ),
        Story::new(
            "checkpoint-timeline/unicode",
            "Unicode CheckpointTimeline",
            "CheckpointTimeline",
            "Unicode-safe paint path for CheckpointTimeline.",
            48,
            12,
            checkpoint_timeline_unicode_story,
        ),
        Story::new(
            "timeline/narrow",
            "Narrow Timeline",
            "Timeline",
            "Narrow-terminal geometry for Timeline (20 cols).",
            20,
            4,
            timeline,
        ),
        Story::new(
            "timeline/unicode",
            "Unicode Timeline",
            "Timeline",
            "Unicode-safe paint path for Timeline (CJK/emoji-capable layout).",
            40,
            4,
            timeline_unicode_story,
        ),
        Story::new(
            "toast/unicode",
            "Unicode Toast",
            "Toast",
            "Unicode-safe paint path for Toast (CJK/emoji-capable layout).",
            34,
            4,
            toast_unicode_story,
        ),
        Story::new(
            "token-meter/narrow",
            "Narrow TokenMeter",
            "TokenMeter",
            "Narrow-terminal geometry for TokenMeter (18 cols).",
            18,
            1,
            token_meter,
        ),
        Story::new(
            "token-meter/unicode",
            "Unicode TokenMeter",
            "TokenMeter",
            "Unicode-safe paint path for TokenMeter (CJK/emoji-capable layout).",
            36,
            1,
            token_meter_unicode_story,
        ),
        Story::new(
            "tool-card/narrow",
            "Narrow ToolCard",
            "ToolCard",
            "Narrow-terminal geometry for ToolCard (22 cols).",
            22,
            4,
            tool_card,
        ),
        Story::new(
            "tool-card/unicode",
            "Unicode ToolCard",
            "ToolCard",
            "Unicode-safe paint path for ToolCard (CJK/emoji-capable layout).",
            44,
            4,
            tool_card_unicode_story,
        ),
        Story::new(
            "tree/narrow",
            "Narrow Tree",
            "Tree",
            "Narrow-terminal geometry for Tree (21 cols).",
            21,
            7,
            tree,
        ),
        Story::new(
            "tree/unicode",
            "Unicode Tree",
            "Tree",
            "Unicode-safe paint path for Tree (CJK/emoji-capable layout).",
            42,
            7,
            tree_unicode_story,
        ),
        Story::new(
            "viewport/narrow",
            "Narrow Viewport",
            "Viewport",
            "Narrow-terminal geometry for Viewport (22 cols).",
            22,
            7,
            viewport,
        ),
        Story::new(
            "viewport/unicode",
            "Unicode Viewport",
            "Viewport",
            "Unicode-safe paint path for Viewport (CJK/emoji-capable layout).",
            44,
            7,
            viewport_unicode_story,
        ),
        Story::new(
            "button/disabled",
            "Disabled button",
            "Button",
            "Disabled control never activates.",
            32,
            3,
            button_disabled_story,
        ),
        Story::new(
            "button/loading",
            "Loading button",
            "Button",
            "Loading control never activates.",
            32,
            3,
            button_loading_story,
        ),
        Story::new(
            "checkbox/disabled",
            "Disabled checkbox",
            "Checkbox",
            "Disabled checkbox ignores toggle.",
            40,
            3,
            checkbox_disabled_story,
        ),
        Story::new(
            "data-table/empty",
            "Empty data table",
            "DataTable",
            "Empty/load projection chrome.",
            60,
            8,
            data_table_empty_story,
        ),
        Story::new(
            "tree-table/process",
            "TreeTable process",
            "TreeTable",
            "Process tree with CPU/MEM columns; hierarchy nav.",
            64,
            12,
            tree_table_process,
        ),
        Story::new(
            "tree-table/schema",
            "TreeTable schema",
            "TreeTable",
            "Database schema browser with types and nullability.",
            64,
            12,
            tree_table_schema,
        ),
        Story::new(
            "tree-table/tasks",
            "TreeTable tasks",
            "TreeTable",
            "Task hierarchy with status and owner columns.",
            60,
            12,
            tree_table_tasks,
        ),
        Story::new(
            "tree-table/deps",
            "TreeTable dependencies",
            "TreeTable",
            "Package dependency tree with versions.",
            60,
            12,
            tree_table_deps,
        ),
        Story::new(
            "tree-table/narrow",
            "TreeTable narrow",
            "TreeTable",
            "Priority drop under width pressure; compact indent.",
            28,
            10,
            tree_table_narrow,
        ),
        Story::new(
            "tree-table/aggregate",
            "TreeTable aggregate",
            "TreeTable",
            "Group band plus aggregate totals row.",
            56,
            10,
            tree_table_aggregate,
        ),
        Story::new(
            "key-value-table/http",
            "KeyValueTable HTTP",
            "KeyValueTable",
            "Request headers with secret redaction and types.",
            72,
            14,
            key_value_table_http,
        ),
        Story::new(
            "key-value-table/database",
            "KeyValueTable database",
            "KeyValueTable",
            "Column metadata with validation and source.",
            68,
            12,
            key_value_table_database,
        ),
        Story::new(
            "key-value-table/process",
            "KeyValueTable process",
            "KeyValueTable",
            "Process facts for ops detail panel.",
            60,
            12,
            key_value_table_process,
        ),
        Story::new(
            "key-value-table/permission",
            "KeyValueTable permission",
            "KeyValueTable",
            "Permission claim details with status tones.",
            64,
            12,
            key_value_table_permission,
        ),
        Story::new(
            "key-value-table/agent",
            "KeyValueTable agent",
            "KeyValueTable",
            "Agent/tool detail with editable fields.",
            64,
            12,
            key_value_table_agent,
        ),
        Story::new(
            "key-value-table/compare",
            "KeyValueTable compare",
            "KeyValueTable",
            "Compare/diff mode before vs after.",
            72,
            10,
            key_value_table_compare,
        ),
        Story::new(
            "key-value-table/narrow",
            "KeyValueTable narrow",
            "KeyValueTable",
            "Stacked contraction under width pressure.",
            28,
            12,
            key_value_table_narrow,
        ),
        Story::new(
            "text-input/basic",
            "Text input",
            "TextInput",
            "Default focused text input.",
            40,
            2,
            text_input_basic_story,
        ),
        Story::new(
            "text-input/secret",
            "TextInput secret",
            "TextInput",
            "Paint-only mask (prefer PasswordInput for credentials).",
            36,
            2,
            text_input_secret_story,
        ),
        Story::new(
            "password-input/basic",
            "Password input",
            "PasswordInput",
            "Masked secret field with redacted diagnostics.",
            36,
            2,
            password_input_basic_story,
        ),
        Story::new(
            "password-input/reveal",
            "Password reveal",
            "PasswordInput",
            "Explicit reveal policy with toggle glyph.",
            36,
            2,
            password_input_reveal_story,
        ),
        Story::new(
            "password-input/invalid",
            "Password invalid",
            "PasswordInput",
            "Invalid + strength status without leaking secret.",
            36,
            3,
            password_input_invalid_story,
        ),
        Story::new(
            "password-input/pending",
            "Password pending",
            "PasswordInput",
            "Pending verification blocks edits.",
            36,
            3,
            password_input_pending_story,
        ),
        Story::new(
            "number-input/basic",
            "Number input",
            "NumberInput",
            "Integer field with steppers and unit.",
            36,
            2,
            number_input_basic_story,
        ),
        Story::new(
            "number-input/decimal",
            "Number decimal",
            "NumberInput",
            "Decimal kind with fraction digits.",
            36,
            2,
            number_input_decimal_story,
        ),
        Story::new(
            "number-input/invalid",
            "Number invalid",
            "NumberInput",
            "Out-of-range draft chrome.",
            36,
            3,
            number_input_invalid_story,
        ),
        Story::new(
            "number-input/narrow",
            "Number narrow",
            "NumberInput",
            "Compact steppers at narrow width.",
            16,
            2,
            number_input_narrow_story,
        ),
        Story::new(
            "search-input/basic",
            "Search input",
            "SearchInput",
            "Query field with clear and result count.",
            48,
            2,
            search_input_basic_story,
        ),
        Story::new(
            "search-input/searching",
            "Search searching",
            "SearchInput",
            "In-progress search status.",
            48,
            2,
            search_input_searching_story,
        ),
        Story::new(
            "search-input/filters",
            "Search filters",
            "SearchInput",
            "Active filter chips before query text.",
            48,
            2,
            search_input_filters_story,
        ),
        Story::new(
            "search-input/empty",
            "Search empty",
            "SearchInput",
            "No-results status projection.",
            40,
            2,
            search_input_empty_story,
        ),
        Story::new(
            "path-input/basic",
            "Path input",
            "PathInput",
            "Path field with directory status and browse.",
            52,
            2,
            path_input_basic_story,
        ),
        Story::new(
            "path-input/missing",
            "Path missing",
            "PathInput",
            "Missing path status for new targets.",
            52,
            2,
            path_input_missing_story,
        ),
        Story::new(
            "path-input/destructive",
            "Path destructive",
            "PathInput",
            "Destructive target warning chrome.",
            52,
            3,
            path_input_destructive_story,
        ),
        Story::new(
            "path-input/relative",
            "Path relative",
            "PathInput",
            "Relative path with base context.",
            52,
            3,
            path_input_relative_story,
        ),
        Story::new(
            "token-field/basic",
            "Token field",
            "TokenField",
            "Recipients-style tokens with draft input.",
            52,
            2,
            token_field_basic_story,
        ),
        Story::new(
            "token-field/overflow",
            "Token field overflow",
            "TokenField",
            "Overflow +N when many tokens.",
            40,
            2,
            token_field_overflow_story,
        ),
        Story::new(
            "token-field/error",
            "Token field error",
            "TokenField",
            "Invalid token status chrome.",
            48,
            2,
            token_field_error_story,
        ),
        Story::new(
            "token-field/multiselect",
            "Token field multi-select",
            "TokenField",
            "Filter chips multi-select mode.",
            48,
            2,
            token_field_multiselect_story,
        ),
        Story::new(
            "select/basic",
            "Select",
            "Select",
            "Closed form select with value.",
            36,
            2,
            select_basic_story,
        ),
        Story::new(
            "select/open",
            "Select open",
            "Select",
            "Open list with groups and highlight.",
            40,
            12,
            select_open_story,
        ),
        Story::new(
            "select/search",
            "Select search",
            "Select",
            "Searchable open list.",
            40,
            12,
            select_search_story,
        ),
        Story::new(
            "select/compact",
            "Select compact",
            "Select",
            "Toolbar compact recipe.",
            24,
            1,
            select_compact_story,
        ),
        Story::new(
            "multi-select/basic",
            "MultiSelect",
            "MultiSelect",
            "Closed summary with chips.",
            40,
            2,
            multi_select_basic_story,
        ),
        Story::new(
            "multi-select/open",
            "MultiSelect open",
            "MultiSelect",
            "Open checklist with highlight vs checks.",
            42,
            14,
            multi_select_open_story,
        ),
        Story::new(
            "multi-select/overflow",
            "MultiSelect overflow",
            "MultiSelect",
            "Summary +N overflow chips.",
            32,
            2,
            multi_select_overflow_story,
        ),
        Story::new(
            "multi-select/search",
            "MultiSelect search",
            "MultiSelect",
            "Searchable open multi-select.",
            42,
            14,
            multi_select_search_story,
        ),
        Story::new(
            "keybinding-recorder/idle",
            "KeybindingRecorder idle",
            "KeybindingRecorder",
            "Idle binding with default and restore hints.",
            48,
            8,
            keybinding_recorder_idle_story,
        ),
        Story::new(
            "keybinding-recorder/recording",
            "KeybindingRecorder recording",
            "KeybindingRecorder",
            "Recording mode with escape-law caption.",
            48,
            8,
            keybinding_recorder_recording_story,
        ),
        Story::new(
            "keybinding-recorder/conflict",
            "KeybindingRecorder conflict",
            "KeybindingRecorder",
            "Conflict validation against occupied chords.",
            48,
            8,
            keybinding_recorder_conflict_story,
        ),
        Story::new(
            "keybinding-recorder/reserved",
            "KeybindingRecorder reserved",
            "KeybindingRecorder",
            "Reserved chord (Ctrl+C) blocked.",
            48,
            8,
            keybinding_recorder_reserved_story,
        ),
        Story::new(
            "date-time-picker/date",
            "DateTimePicker date",
            "DateTimePicker",
            "Open calendar with today, selected, and focus marks.",
            48,
            16,
            date_time_picker_date_story,
        ),
        Story::new(
            "date-time-picker/time",
            "DateTimePicker time",
            "DateTimePicker",
            "Stepped time list with timezone label.",
            36,
            14,
            date_time_picker_time_story,
        ),
        Story::new(
            "date-time-picker/range",
            "DateTimePicker range",
            "DateTimePicker",
            "Inclusive date range selection.",
            48,
            16,
            date_time_picker_range_story,
        ),
        Story::new(
            "date-time-picker/narrow",
            "DateTimePicker narrow",
            "DateTimePicker",
            "Tiny-terminal day-list fallback.",
            28,
            12,
            date_time_picker_narrow_story,
        ),
        Story::new(
            "file-picker/unix",
            "FilePicker Unix",
            "FilePicker",
            "Unix browser with breadcrumbs, list, and preview.",
            72,
            18,
            file_picker_unix_story,
        ),
        Story::new(
            "file-picker/windows",
            "FilePicker Windows",
            "FilePicker",
            "Windows path style listing.",
            72,
            16,
            file_picker_windows_story,
        ),
        Story::new(
            "file-picker/ssh",
            "FilePicker SSH",
            "FilePicker",
            "SSH-like remote path provider projection.",
            72,
            16,
            file_picker_ssh_story,
        ),
        Story::new(
            "file-picker/no-preview",
            "FilePicker no preview",
            "FilePicker",
            "List-only layout without preview pane.",
            56,
            14,
            file_picker_no_preview_story,
        ),
        Story::new(
            "combobox/basic",
            "Combobox",
            "Combobox",
            "Closed combobox field with value.",
            36,
            2,
            combobox_basic_story,
        ),
        Story::new(
            "combobox/open",
            "Combobox open",
            "Combobox",
            "Open suggestions via CompletionMenu.",
            40,
            10,
            combobox_open_story,
        ),
        Story::new(
            "combobox/loading",
            "Combobox loading",
            "Combobox",
            "Async suggestion loading status.",
            36,
            2,
            combobox_loading_story,
        ),
        Story::new(
            "autocomplete/basic",
            "Autocomplete",
            "Autocomplete",
            "Creatable free-text autocomplete.",
            40,
            10,
            autocomplete_basic_story,
        ),
        Story::new(
            "text-input/invalid",
            "TextInput invalid",
            "TextInput",
            "Validation error chrome.",
            40,
            3,
            text_input_invalid_story,
        ),
        Story::new(
            "text-input/prefix",
            "TextInput prefix/suffix",
            "TextInput",
            "Prefix and suffix adornments.",
            40,
            2,
            text_input_prefix_story,
        ),
    ]
}

/// Interactive-gallery entries, including compile-proven design prototypes.
/// Catalog generation deliberately uses [`stories`] instead.
pub(crate) fn gallery_stories() -> Vec<Story> {
    stories()
}

fn ui_context_frame_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::context::UiHost;
    use termrock::interaction::{
        InteractionElement, InteractionLayer, LayerDismissPolicy, LayerKind, SemanticRole,
    };
    use termrock::widgets::Panel;
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum Id {
        Root,
        Body,
    }
    let mut host = UiHost::<Id, Id>::test_with_design(system.clone());
    host.scene.ensure_root(InteractionLayer {
        id: Id::Root,
        kind: LayerKind::Root,
        owns_input: true,
        esc: LayerDismissPolicy::Ignore,
        outside: LayerDismissPolicy::Ignore,
        focus_return: None,
    });
    let mut ctx = host.begin_frame();
    let _ = ctx.scene_mut().register(
        InteractionElement::control(Id::Body, Id::Root, area)
            .role(SemanticRole::Dialog)
            .focusable(true),
    );
    let body = Panel::new(ctx.design())
        .title("UiContext")
        .subtitle(&format!("frame {}", ctx.frame_index()))
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "design+scene+focus+overlays+tick",
            usize::from(body.width),
            ctx.design().style(Role::TextMuted),
        );
    }
}

fn ui_context_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::context::UiHost;
    use termrock::interaction::{
        InteractionElement, InteractionLayer, LayerDismissPolicy, LayerKind, SemanticNode,
        SemanticRole,
    };
    use termrock::layout::{FlexSize, Stack};
    use termrock::widgets::Panel;
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum Id {
        Root,
        A,
        B,
    }
    impl std::fmt::Display for Id {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Root => write!(f, "root"),
                Self::A => write!(f, "a"),
                Self::B => write!(f, "b"),
            }
        }
    }
    let mut host = UiHost::<Id, Id>::test_with_design(system.clone());
    host.scene.ensure_root(InteractionLayer {
        id: Id::Root,
        kind: LayerKind::Root,
        owns_input: true,
        esc: LayerDismissPolicy::Ignore,
        outside: LayerDismissPolicy::Ignore,
        focus_return: None,
    });
    let mut ctx = host.begin_frame();
    let layout = Stack::new()
        .gap(1)
        .layout(area, &[FlexSize::Weight(1), FlexSize::Weight(1)]);
    if let Some(ra) = layout.get(0) {
        let _ = ctx.scene_mut().register(
            InteractionElement::control(Id::A, Id::Root, ra)
                .role(SemanticRole::Button)
                .focusable(true),
        );
        let _ = ctx.semantics_mut().register(
            SemanticNode::control(Id::A, ra)
                .role(SemanticRole::Button)
                .label("Nested A"),
        );
        let _ = Panel::new(ctx.design())
            .title("child A")
            .paint(ra, frame.buffer_mut(), None);
    }
    if let Some(rb) = layout.get(1) {
        let _ = ctx.scene_mut().register(
            InteractionElement::control(Id::B, Id::Root, rb)
                .role(SemanticRole::Input)
                .focusable(true),
        );
        let _ = ctx.semantics_mut().register(
            SemanticNode::control(Id::B, rb)
                .role(SemanticRole::Input)
                .label("Nested B"),
        );
        let _ = Panel::new(ctx.design())
            .title("child B")
            .paint(rb, frame.buffer_mut(), None);
    }
    let n = ctx.semantics().nodes().len();
    ctx.note(format!("semantic_nodes={n}"));
}

fn design_system_presets_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::style::{DesignSystem, Role};
    use termrock::widgets::Panel;
    let presets = [
        ("phosphor", DesignSystem::phosphor()),
        ("slate", DesignSystem::slate()),
        ("paper", DesignSystem::paper()),
        ("ansi", DesignSystem::ansi()),
        ("hi-con", DesignSystem::high_contrast()),
    ];
    let sizes: Vec<FlexSize> = presets.iter().map(|_| FlexSize::Weight(1)).collect();
    let layout = Stack::new().gap(0).layout(area, &sizes);
    for (i, (name, ds)) in presets.iter().enumerate() {
        if let Some(r) = layout.get(i) {
            let body = Panel::new(ds)
                .title(name)
                .paint(r, frame.buffer_mut(), None);
            if body.width > 2 {
                frame.buffer_mut().set_stringn(
                    body.x,
                    body.y,
                    "Aa 01",
                    usize::from(body.width),
                    ds.style(Role::Accent),
                );
            }
        }
    }
    let _ = system;
}

fn design_system_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::{ControlState, DesignSystem, ButtonRecipeVariant, Role};
    use termrock::widgets::Panel;
    let ds = DesignSystem::phosphor().no_color();
    let body = Panel::new(&ds)
        .title("no-color")
        .paint(area, frame.buffer_mut(), None);
    let recipe = ds.button_recipe(ButtonRecipeVariant::Primary, ControlState::Focused);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "[ primary focused ]",
            usize::from(body.width),
            recipe.label,
        );
    }
    if body.height > 1 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y.saturating_add(1),
            "glyphs: ascii",
            usize::from(body.width),
            ds.style(Role::TextMuted),
        );
    }
    let _ = system;
}

fn design_system_button_recipes_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::style::{ButtonRecipeVariant, ControlState};
    let variants = [
        ButtonRecipeVariant::Primary,
        ButtonRecipeVariant::Secondary,
        ButtonRecipeVariant::Destructive,
    ];
    let states = [
        ControlState::Default,
        ControlState::Focused,
        ControlState::Disabled,
    ];
    let layout = Stack::new().gap(0).layout(
        area,
        &[
            FlexSize::Fixed(1),
            FlexSize::Fixed(1),
            FlexSize::Fixed(1),
            FlexSize::Weight(1),
        ],
    );
    for (row, &state) in states.iter().enumerate() {
        if let Some(r) = layout.get(row) {
            let mut x = r.x;
            for &variant in &variants {
                let recipe = system.button_recipe(variant, state);
                let label = match variant {
                    ButtonRecipeVariant::Primary => "prim",
                    ButtonRecipeVariant::Secondary => "sec",
                    ButtonRecipeVariant::Destructive => "dest",
                    _ => "btn",
                };
                let text = format!(" {label} ");
                frame.buffer_mut().set_stringn(
                    x,
                    r.y,
                    &text,
                    usize::from(r.width.saturating_sub(x.saturating_sub(r.x))),
                    recipe.label,
                );
                x = x.saturating_add(text.len() as u16 + 1);
            }
        }
    }
}

fn center_both_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::{EmptyState, Panel};
    let child = Center::new(28, 6).layout(area).child;
    let _ = Panel::new(system).title("centered").paint(child, frame.buffer_mut(), None);
    let inner = Panel::new(system).title("centered").inner(child);
    Widget::render(
        &EmptyState::new("No selection", system).detail("Pick a story"),
        inner,
        frame.buffer_mut(),
    );
}

fn center_dialog_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::{Panel, PanelChrome};
    let child = Center::dialog(24, 7).layout(area).child;
    let body = Panel::new(system)
        .title("Confirm")
        .emphasis(PanelChrome::Focused)
        .footer("esc cancel")
        .paint(child, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "Apply changes?",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn center_horizontal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{Center, CenterAxis};
    use termrock::widgets::Panel;
    let child = Center::new(16, 1)
        .axis(CenterAxis::Horizontal)
        .layout(area)
        .child;
    let _ = Panel::new(system)
        .title("h-center")
        .paint(child, frame.buffer_mut(), None);
}

fn center_max_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::Panel;
    let child = Center::new(80, 5)
        .max_width(20)
        .layout(area)
        .child;
    let _ = Panel::new(system)
        .title("max 20")
        .paint(child, frame.buffer_mut(), None);
}

fn center_tiny_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::Panel;
    let child = Center::new(40, 20).safe_margin(true).layout(area).child;
    let _ = Panel::new(system)
        .title("tiny")
        .paint(child, frame.buffer_mut(), None);
}

fn center_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{Center, CenterAxis};
    use termrock::widgets::Panel;
    let child = Center::new(10, 3)
        .axis(CenterAxis::Vertical)
        .layout(area)
        .child;
    let body = Panel::new(system)
        .title("v-center")
        .paint(child, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "fills width",
            usize::from(body.width),
            system.style(Role::TextMuted),
        );
    }
}

fn center_onboarding_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::Panel;
    let child = Center::onboarding(48, 8).layout(area).child;
    let body = Panel::new(system)
        .title("Welcome")
        .subtitle("onboarding")
        .paint(child, frame.buffer_mut(), None);
    if body.width > 2 && body.height > 0 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "Get started with TermRock",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn center_failure_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::Center;
    use termrock::widgets::{Panel, PanelBody, PanelChrome};
    let child = Center::failure(40, 6).layout(area).child;
    let _ = Panel::new(system)
        .title("Failed")
        .emphasis(PanelChrome::Danger)
        .body(PanelBody::Error)
        .body_title("Timeout")
        .body_detail("upstream 30s")
        .paint(child, frame.buffer_mut(), None);
}

fn section_quiet_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Section;
    let body = Section::new("Appearance", system)
        .description("Theme and density")
        .quiet()
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 && body.height > 0 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "Theme: phosphor",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn section_emphasized_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Section;
    let body = Section::new("Security", system)
        .description("Trust boundary")
        .emphasized()
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 && body.height > 0 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "Require confirmation",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn section_collapsible_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Section, SectionState};
    let mut state = SectionState::new();
    state.set_focused(true);
    let body = Section::new("Advanced", system)
        .description("rarely needed")
        .collapsible(true)
        .emphasized()
        .paint(area, frame.buffer_mut(), Some(&mut state));
    if body.height > 0 && body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "debug logs: off",
            usize::from(body.width),
            system.style(Role::TextMuted),
        );
    }
}

fn section_actions_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Section, SectionAction};
    let actions = [
        SectionAction::new("reset", "Reset"),
        SectionAction::new("docs", "Docs"),
    ];
    let body = Section::new("Network", system)
        .status("live")
        .actions(&actions)
        .variant(termrock::widgets::SectionVariant::Divided)
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "proxy: system",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn section_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::widgets::Section;
    let layout = Stack::new().gap(0).layout(
        area,
        &[FlexSize::Fixed(3), FlexSize::Weight(1)],
    );
    if let Some(top) = layout.get(0) {
        let body = Section::new("Root", system)
            .emphasized()
            .paint(top, frame.buffer_mut(), None);
        let _ = body;
    }
    if let Some(bottom) = layout.get(1) {
        let body = Section::new("Nested", system)
            .description("child group")
            .depth(1)
            .quiet()
            .paint(bottom, frame.buffer_mut(), None);
        if body.width > 2 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "item",
                usize::from(body.width),
                system.style(Role::TextMuted),
            );
        }
    }
}

fn section_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Section, SectionAction};
    let actions = [SectionAction::new("more", "More")];
    let _ = Section::new("Title long", system)
        .description("this drops first")
        .status("n")
        .actions(&actions)
        .emphasized()
        .paint(area, frame.buffer_mut(), None);
}

fn grid_columns_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{Grid, GridAutoFlow};
    use termrock::widgets::Panel;
    let layout = Grid::columns(3)
        .gaps(1, 1)
        .auto_row(termrock::layout::TrackSize::Fixed(4))
        .layout_flow(area, 6, GridAutoFlow::Row);
    for (i, r) in layout.cells.iter().enumerate() {
        if r.width == 0 {
            continue;
        }
        let labels = ["A", "B", "C", "D", "E", "F"];
        let _ = Panel::new(system)
            .title(labels.get(i).copied().unwrap_or("x"))
            .paint(*r, frame.buffer_mut(), None);
    }
}

fn grid_span_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{layout_grid, GridItem, GridSpec, TrackSize};
    use termrock::widgets::{Panel, PanelChrome};
    let spec = GridSpec::columns_fr(2)
        .gaps(1, 1)
        .rows([TrackSize::Fixed(3), TrackSize::Weight(1)]);
    let items = [
        GridItem::span(0, 0, 2, 1),
        GridItem::cell(0, 1),
        GridItem::cell(1, 1),
    ];
    let layout = layout_grid(area, &spec, &items);
    if let Some(r) = layout.get(0) {
        let _ = Panel::new(system)
            .title("header span")
            .emphasis(PanelChrome::Focused)
            .paint(r, frame.buffer_mut(), None);
    }
    if let Some(r) = layout.get(1) {
        let _ = Panel::new(system)
            .title("left")
            .paint(r, frame.buffer_mut(), None);
    }
    if let Some(r) = layout.get(2) {
        let _ = Panel::new(system)
            .title("right")
            .paint(r, frame.buffer_mut(), None);
    }
}

fn grid_dashboard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{dashboard_grid_template, layout_grid, auto_flow_items, GridAutoFlow};
    use termrock::widgets::Card;
    let spec = dashboard_grid_template(area.width, 3, 18, 1);
    let items = auto_flow_items(spec.col_count(), 6, GridAutoFlow::Row);
    let layout = layout_grid(area, &spec, &items);
    let titles = ["CPU", "MEM", "NET", "DISK", "QPS", "ERR"];
    for (i, r) in layout.cells.iter().enumerate() {
        if r.height < 2 {
            continue;
        }
        let _ = Card::new(system)
            .title(titles.get(i).copied().unwrap_or("m"))
            .description("metric")
            .paint(*r, frame.buffer_mut(), None);
    }
}

fn grid_form_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{form_grid_template, layout_grid, auto_flow_items, GridAutoFlow};
    use termrock::widgets::Panel;
    let spec = form_grid_template(area.width);
    let items = auto_flow_items(spec.col_count(), 4, GridAutoFlow::Row);
    let layout = layout_grid(area, &spec, &items);
    let labels = ["Name", "Email", "Role", "Team"];
    for (i, r) in layout.cells.iter().enumerate() {
        if r.height == 0 {
            continue;
        }
        let body = Panel::new(system)
            .title(labels.get(i).copied().unwrap_or("f"))
            .paint(*r, frame.buffer_mut(), None);
        if body.width > 2 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "value",
                usize::from(body.width),
                system.style(Role::Input),
            );
        }
    }
}

fn grid_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{form_grid_template, layout_grid, auto_flow_items, GridAutoFlow};
    use termrock::widgets::Panel;
    let spec = form_grid_template(area.width);
    let items = auto_flow_items(spec.col_count(), 3, GridAutoFlow::Row);
    let layout = layout_grid(area, &spec, &items);
    for (i, r) in layout.cells.iter().enumerate() {
        if r.height == 0 {
            continue;
        }
        let labels = ["1", "2", "3"];
        let _ = Panel::new(system)
            .title(labels.get(i).copied().unwrap_or("x"))
            .paint(*r, frame.buffer_mut(), None);
    }
}

fn grid_settings_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{layout_grid, settings_grid_template, GridItem};
    let spec = settings_grid_template(area.width, 14, 2);
    let pairs = [
        ("Theme", "phosphor"),
        ("Density", "compact"),
        ("Keymap", "default"),
    ];
    let items: Vec<GridItem> = if spec.col_count() >= 2 {
        pairs
            .iter()
            .enumerate()
            .flat_map(|(row, _)| {
                [
                    GridItem::cell(0, row as u16),
                    GridItem::cell(1, row as u16),
                ]
            })
            .collect()
    } else {
        // Stacked: two auto-rows per setting
        pairs
            .iter()
            .enumerate()
            .flat_map(|(i, _)| {
                let r = (i as u16).saturating_mul(2);
                [GridItem::cell(0, r), GridItem::cell(0, r + 1)]
            })
            .collect()
    };
    let layout = layout_grid(area, &spec, &items);
    for (i, r) in layout.cells.iter().enumerate() {
        if r.width == 0 || r.height == 0 {
            continue;
        }
        let pair_i = i / 2;
        let is_label = i % 2 == 0;
        let (label, value) = pairs.get(pair_i).copied().unwrap_or(("", ""));
        let text = if is_label { label } else { value };
        let role = if is_label {
            Role::TextMuted
        } else {
            Role::Text
        };
        frame.buffer_mut().set_stringn(
            r.x,
            r.y,
            text,
            usize::from(r.width),
            system.style(role),
        );
    }
    if area.height > 0 {
        let dbg = layout.debug_summary();
        let clipped = if dbg.len() > usize::from(area.width) {
            &dbg[..usize::from(area.width)]
        } else {
            &dbg
        };
        // Only paint debug when there is spare bottom row (tiny Studio inspector cue).
        if layout.content.bottom() < area.bottom() {
            frame.buffer_mut().set_stringn(
                area.x,
                area.bottom().saturating_sub(1),
                clipped,
                usize::from(area.width),
                system.style(Role::TextDisabled),
            );
        }
    }
}

fn grid_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{
        layout_grid, GridItem, GridSpec, OverflowPolicy, TrackSize,
    };
    use termrock::widgets::Panel;
    let spec = GridSpec::default()
        .overflow(OverflowPolicy::ClipTail)
        .columns([
            TrackSize::Fixed(10),
            TrackSize::Fixed(10),
            TrackSize::Fixed(10),
        ])
        .rows([TrackSize::Fixed(4)]);
    let items = [
        GridItem::cell(0, 0),
        GridItem::cell(1, 0),
        GridItem::cell(2, 0),
    ];
    let layout = layout_grid(area, &spec, &items);
    for (i, label) in ["keep", "mid", "drop"].iter().enumerate() {
        if let Some(r) = layout.get(i) {
            if r.width == 0 {
                continue;
            }
            let _ = Panel::new(system)
                .title(label)
                .paint(r, frame.buffer_mut(), None);
        }
    }
}

fn grid_nav_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{
        auto_flow_items, grid_neighbor_2d, layout_grid, GridAutoFlow, GridSpec, TrackSize,
    };
    use termrock::widgets::Panel;
    let spec = GridSpec::columns_fr(3)
        .gaps(1, 1)
        .rows([TrackSize::Fixed(3), TrackSize::Fixed(3)]);
    let items = auto_flow_items(3, 6, GridAutoFlow::Row);
    let layout = layout_grid(area, &spec, &items);
    let focus = 0usize;
    let right = grid_neighbor_2d(&items, focus, 1, 0);
    let down = grid_neighbor_2d(&items, focus, 0, 1);
    for (i, r) in layout.cells.iter().enumerate() {
        if r.height == 0 {
            continue;
        }
        let mark = if i == focus {
            "F"
        } else if Some(i) == right {
            "→"
        } else if Some(i) == down {
            "↓"
        } else {
            "·"
        };
        let body = Panel::new(system)
            .title(&format!("{i}"))
            .paint(*r, frame.buffer_mut(), None);
        if body.width > 0 && body.height > 0 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                mark,
                1,
                system.style(Role::Accent),
            );
        }
    }
}

fn stack_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::widgets::{Panel, PanelChrome};
    let layout = Stack::new().gap(1).layout(
        area,
        &[FlexSize::Fixed(2), FlexSize::Weight(1), FlexSize::Fixed(2)],
    );
    for (i, label) in ["header", "body", "footer"].iter().enumerate() {
        if let Some(r) = layout.get(i) {
            let chrome = if *label == "body" {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            };
            let _ = Panel::new(system)
                .title(label)
                .emphasis(chrome)
                .paint(r, frame.buffer_mut(), None);
        }
    }
}

fn stack_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Inline};
    use termrock::widgets::Panel;
    let layout = Inline::new().gap(1).layout(
        area,
        &[FlexSize::Weight(1), FlexSize::Weight(1), FlexSize::Weight(1)],
    );
    for (i, label) in ["A", "B", "C"].iter().enumerate() {
        if let Some(r) = layout.get(i) {
            let body = Panel::new(system)
                .title(label)
                .paint(r, frame.buffer_mut(), None);
            if body.width > 1 {
                frame.buffer_mut().set_stringn(
                    body.x,
                    body.y,
                    label,
                    usize::from(body.width),
                    system.style(Role::TextStrong),
                );
            }
        }
    }
}

fn stack_wrap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Inline};
    let chips = [
        FlexSize::Fixed(5),
        FlexSize::Fixed(5),
        FlexSize::Fixed(5),
        FlexSize::Fixed(5),
        FlexSize::Fixed(5),
        FlexSize::Fixed(5),
    ];
    let layout = Inline::new().wrap(true).gap(1).layout(area, &chips);
    for (i, r) in layout.children.iter().enumerate() {
        if r.width == 0 {
            continue;
        }
        let label = format!("c{i}");
        frame.buffer_mut().set_stringn(
            r.x,
            r.y,
            &label,
            usize::from(r.width),
            system.style(Role::Accent),
        );
    }
}

fn stack_overflow_clip_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, OverflowPolicy, Stack};
    use termrock::widgets::Panel;
    let layout = Stack::new()
        .overflow(OverflowPolicy::ClipTail)
        .layout(
            area,
            &[
                FlexSize::Fixed(3),
                FlexSize::Fixed(3),
                FlexSize::Fixed(3),
            ],
        );
    for (i, label) in ["keep", "partial", "drop"].iter().enumerate() {
        if let Some(r) = layout.get(i) {
            if r.height == 0 {
                continue;
            }
            let _ = Panel::new(system)
                .title(label)
                .paint(r, frame.buffer_mut(), None);
        }
    }
}

fn stack_justify_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Inline, Justify};
    let layout = Inline::new()
        .justify(Justify::SpaceAround)
        .layout(
            area,
            &[FlexSize::Fixed(6), FlexSize::Fixed(6), FlexSize::Fixed(6)],
        );
    for (i, r) in layout.children.iter().enumerate() {
        if r.width == 0 {
            continue;
        }
        frame.buffer_mut().set_stringn(
            r.x,
            r.y,
            &format!("[{i}]"),
            usize::from(r.width),
            system.style(Role::Accent),
        );
    }
}

fn stack_responsive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::widgets::Panel;
    let layout = Stack::new()
        .responsive(area.width, 50)
        .gap(1)
        .layout(area, &[FlexSize::Weight(1), FlexSize::Weight(1)]);
    let labels = [layout.direction.id(), "child"];
    for (i, label) in labels.iter().enumerate() {
        if let Some(r) = layout.get(i) {
            let _ = Panel::new(system)
                .title(label)
                .paint(r, frame.buffer_mut(), None);
        }
    }
}

fn stack_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Stack};
    use termrock::widgets::Panel;
    let layout = Stack::new().layout(
        area,
        &[
            FlexSize::Fixed(3),
            FlexSize::Fixed(3),
            FlexSize::Fixed(3),
        ],
    );
    const LABELS: [&str; 3] = ["0", "1", "2"];
    for (i, r) in layout.children.iter().enumerate() {
        if r.height == 0 {
            continue;
        }
        let _ = Panel::new(system)
            .title(LABELS.get(i).copied().unwrap_or("x"))
            .paint(*r, frame.buffer_mut(), None);
    }
    if layout.overflowed && area.height > 0 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            "overflow",
            usize::from(area.width),
            system.style(Role::Warning),
        );
    }
}

fn panel_variants_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelVariant};
    let variants = [
        PanelVariant::Bordered,
        PanelVariant::Quiet,
        PanelVariant::DividerOnly,
        PanelVariant::Interactive,
        PanelVariant::Selected,
    ];
    let h = (area.height / variants.len() as u16).max(2);
    for (i, v) in variants.iter().enumerate() {
        let y = area.y.saturating_add((i as u16).saturating_mul(h));
        if y >= area.bottom() {
            break;
        }
        let row = Rect::new(area.x, y, area.width, h.min(area.bottom().saturating_sub(y)));
        let body = Panel::new(system)
            .title(v.id())
            .variant(*v)
            .paint(row, frame.buffer_mut(), None);
        if body.width > 2 && body.height > 0 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                v.id(),
                usize::from(body.width),
                system.style(Role::TextMuted),
            );
        }
    }
}

fn panel_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelBody};
    let _ = Panel::new(system)
        .title("Inbox")
        .body(PanelBody::Empty)
        .body_title("No messages")
        .body_detail("Try another filter")
        .paint(area, frame.buffer_mut(), None);
}

fn panel_collapsible_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelState};
    let mut state = PanelState::new();
    state.set_focused(true);
    let body = Panel::new(system)
        .title("Section")
        .subtitle("details")
        .collapsible(true)
        .emphasis(PanelChrome::Focused)
        .paint(area, frame.buffer_mut(), Some(&mut state));
    if body.height > 0 && body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "expanded body",
            usize::from(body.width),
            system.style(Role::Text),
        );
    }
}

fn panel_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Panel;
    let _ = Panel::new(system)
        .title("Main title")
        .subtitle("subtitle")
        .leading("*")
        .trailing("act")
        .footer("hint")
        .emphasis(PanelChrome::Focused)
        .paint(area, frame.buffer_mut(), None);
}

fn card_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Card;
    let body = Card::new(system)
        .title("Latency")
        .description("p99 over last hour")
        .footer("dashboard")
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "42ms",
            usize::from(body.width),
            system.style(Role::TextStrong),
        );
    }
}

fn card_tool_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Card, ToolStatus};
    let status = ToolStatus::Running;
    let body = Card::new(system)
        .title("shell")
        .leading(status.glyph())
        .badge("run")
        .subtitle("cargo test")
        .emphasis(PanelChrome::Focused)
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "running…",
            usize::from(body.width),
            system.style(status.role()),
        );
    }
}

fn card_dashboard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{FlexSize, Inline, Stack};
    use termrock::widgets::{Card, PanelVariant};

    let rows = Stack::new().gap(1).layout(
        area,
        &[FlexSize::Weight(1), FlexSize::Weight(1)],
    );
    let top = rows.get(0).unwrap_or(area);
    let bottom = rows.get(1).unwrap_or(area);
    let top_cols = Inline::new()
        .gap(1)
        .layout(top, &[FlexSize::Weight(1), FlexSize::Weight(1)]);
    let metrics = [
        (top_cols.get(0).unwrap_or(top), "Latency", "42ms", "p99", "live"),
        (top_cols.get(1).unwrap_or(top), "Errors", "0.2%", "5m", "ok"),
        (bottom, "Throughput", "1.2k rps", "rolling", "hot"),
    ];
    for (rect, title, value, desc, badge) in metrics {
        if rect.width < 8 || rect.height < 3 {
            continue;
        }
        let body = Card::new(system)
            .title(title)
            .badge(badge)
            .description(desc)
            .variant(PanelVariant::Bordered)
            .paint(rect, frame.buffer_mut(), None);
        if body.width > 2 && body.height > 0 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                value,
                usize::from(body.width),
                system.style(Role::TextStrong),
            );
        }
    }
}

fn panel_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelBody};
    let _ = Panel::new(system)
        .title("Jobs")
        .badge("sync")
        .body(PanelBody::Loading)
        .body_detail("Fetching queue…")
        .paint(area, frame.buffer_mut(), None);
}

fn panel_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelBody, PanelAction};
    let actions = [PanelAction::new("retry", "Retry")];
    let _ = Panel::new(system)
        .title("Deploy")
        .leading("!")
        .body(PanelBody::Error)
        .body_title("Failed")
        .body_detail("timeout after 30s")
        .header_actions(&actions)
        .emphasis(PanelChrome::Danger)
        .paint(area, frame.buffer_mut(), None);
}

fn panel_actions_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelAction, PanelBody};
    let actions = [
        PanelAction::new("retry", "Retry"),
        PanelAction::new("logs", "Logs"),
    ];
    let body = Panel::new(system)
        .title("Build")
        .subtitle("main")
        .badge("failed")
        .header_actions(&actions)
        .footer("esc close")
        .body(PanelBody::Host)
        .paint(area, frame.buffer_mut(), None);
    if body.width > 2 && body.height > 0 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "error: link failed",
            usize::from(body.width),
            system.style(Role::Danger),
        );
    }
}

fn panel(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(
        Panel::new(&panel_tokens)
            .title("Summary")
            .emphasis(PanelChrome::Focused),
        area,
    );
    if area.width > 2 && area.height > 2 {
        frame.render_widget(
            Paragraph::new("State   Ready\nMode    Interactive"),
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2),
        );
    }
}

fn progress(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let determinate = Rect::new(area.x, area.y, area.width, area.height.min(1));
    frame.render_widget(
        Progress::new(ProgressKind::Determinate { fraction: 0.62 }, system).label("Processing"),
        determinate,
    );
    if area.height > 1 {
        frame.render_widget(
            Progress::new(ProgressKind::Indeterminate { tick: 3 }, system).label("Waiting"),
            Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        );
    }
}

fn progress_detailed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    use termrock::widgets::{ProgressBar, ProgressBarState, ProgressRecipe, ProgressStatus};
    let mut state = ProgressBarState::transfer(12_582_912, 31_457_280);
    state.set_label("Download");
    state.set_rate(Some(2_200_000.0));
    state.recompute_eta();
    state.set_recipe(ProgressRecipe::Detailed);
    state.set_status(ProgressStatus::Running);
    ProgressBar::paint_state(
        system,
        area,
        frame.buffer_mut(),
        &mut state,
        FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO),
        Motion::Off,
    );
}

fn progress_multiline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ProgressBar, ProgressKind, ProgressRecipe, ProgressStatus};
    ProgressBar::new(ProgressKind::Determinate { fraction: 0.4 }, system)
        .label("Compile")
        .recipe(ProgressRecipe::MultiLine)
        .meta("phase: codegen · 40/100 · ETA 12s")
        .status(ProgressStatus::Running)
        .paint(area, frame.buffer_mut());
}

fn progress_failed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ProgressBar, ProgressKind, ProgressStatus};
    ProgressBar::new(ProgressKind::Determinate { fraction: 0.72 }, system)
        .label("Build")
        .status(ProgressStatus::Failed)
        .paint(area, frame.buffer_mut());
}

fn progress_steps_pipeline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let steps = example_build_pipeline();
    let mut state = ProgressStepsState::new();
    ProgressSteps::new(&steps, system)
        .title("Build pipeline")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn progress_steps_agent_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let steps = example_agent_plan_steps();
    let mut state = ProgressStepsState::new();
    ProgressSteps::new(&steps, system)
        .title("Agent plan")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn progress_steps_summary_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let steps = example_build_pipeline();
    let mut state = ProgressStepsState::new();
    state.set_presentation(Some(ProgressStepsPresentation::Summary));
    ProgressSteps::new(&steps, system).paint(area, frame.buffer_mut(), &mut state);
}

fn progress_steps_interactive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let steps = example_agent_plan_steps();
    let mut state = ProgressStepsState::interactive();
    state.set_cursor(Some("build".into()));
    ProgressSteps::new(&steps, system)
        .title("Recover")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn progress_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    const ASCII_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let [bar, spinner] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(
        Progress::new(ProgressKind::Determinate { fraction: 0.62 }, system).label("Build"),
        bar,
    );
    frame.render_widget(
        Progress::new(ProgressKind::Indeterminate { tick: 3 }, system)
            .frames(&ASCII_FRAMES)
            .label("Waiting"),
        spinner,
    );
}

fn log_pane(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LogPaneState::new().with_max_lines(200);
    for line in [
        "[12:04:01] resolving workspace",
        "[12:04:02] compiling termrock",
        "[12:04:03] running 205 tests",
        "[12:04:04] result: ok ✓",
    ] {
        state.append(line);
    }
    frame.render_stateful_widget(&LogPane::new(system).title("Build log"), area, &mut state);
}

fn log_pane_scrolled(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LogPaneState::new();
    for line in [
        "[12:04:01] resolving workspace",
        "[12:04:02] 東京 worker ready 🪨",
        "[12:04:03] compiling termrock",
        "[12:04:04] running tests",
        "[12:04:05] rendering previews",
        "[12:04:06] result: ok ✓",
        "[12:04:07] waiting for changes",
    ] {
        state.append(line);
    }
    let pane = LogPane::new(system).title("Frozen build log");
    state.scroll_to_oldest();
    frame.render_stateful_widget(&pane, area, &mut state);
}

fn accordion_section_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("gen", "General").content_height(2),
        AccordionItem::new("net", "Network").content_height(2),
        AccordionItem::new("adv", "Advanced").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["gen", "adv"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("gen"));
    let parts = Accordion::section(&items, system).paint(area, frame.buffer_mut(), &mut state);
    for (id, body) in [
        ("gen", "Theme · density"),
        ("net", "Proxy · DNS"),
        ("adv", "Experimental flags"),
    ] {
        if let Some(r) = parts.content_of(&id)
            && r.height > 0
        {
            Paragraph::new(body)
                .style(system.style(Role::TextMuted))
                .render(r, frame.buffer_mut());
        }
    }
}

fn accordion_settings_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("profile", "Profile").content_height(3),
        AccordionItem::new("keys", "API keys").content_height(3),
        AccordionItem::new("danger", "Danger zone").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["profile"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("profile"));
    let parts = Accordion::settings(&items, system).paint(area, frame.buffer_mut(), &mut state);
    if let Some(r) = parts.content_of(&"profile")
        && r.height > 0
    {
        Paragraph::new("Name: Ada\nEmail: ada@example.com")
            .style(system.style(Role::Text))
            .render(r, frame.buffer_mut());
    }
}

fn accordion_logs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("build", "build.log").content_height(3),
        AccordionItem::new("test", "test.log").content_height(3),
        AccordionItem::new("agent", "agent.trace").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["build", "test"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("test"));
    let parts = Accordion::logs(&items, system)
        .keep_mounted()
        .paint(area, frame.buffer_mut(), &mut state);
    for (id, body) in [
        ("build", "compiling termrock…\nfinished in 4.2s"),
        ("test", "running 19 tests\nok"),
        ("agent", "tool: cargo test"),
    ] {
        if let Some(r) = parts.content_of(&id)
            && r.height > 0
        {
            Paragraph::new(body)
                .style(system.style(Role::TextMuted))
                .render(r, frame.buffer_mut());
        }
    }
}

fn accordion_faq_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("q1", "How do I pin a version?").content_height(3),
        AccordionItem::new("q2", "Where is focus painted?").content_height(3),
        AccordionItem::new("q3", "Can I theme phosphor away?").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["q1"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("q1"));
    let parts = Accordion::faq(&items, system).paint(area, frame.buffer_mut(), &mut state);
    if let Some(r) = parts.content_of(&"q1")
        && r.height > 0
    {
        Paragraph::new("Pin an exact Git revision and migrate\nforward with migrations/.")
            .style(system.style(Role::TextMuted))
            .render(r, frame.buffer_mut());
    }
}

fn accordion_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("a", "Very long settings category title").content_height(2),
        AccordionItem::new("b", "Another long optional group").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["a"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("a"));
    let parts = Accordion::settings(&items, system).paint(area, frame.buffer_mut(), &mut state);
    if let Some(r) = parts.content_of(&"a")
        && r.height > 0
    {
        Paragraph::new("body")
            .style(system.style(Role::TextMuted))
            .render(r, frame.buffer_mut());
    }
}

fn accordion_scroll_body_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Accordion, AccordionItem, AccordionState};
    use ratatui::widgets::Widget as _;
    let items = [
        AccordionItem::new("long", "Long transcript").content_height(20),
        AccordionItem::new("short", "Notes").content_height(2),
    ];
    let mut state = AccordionState::new().initially_open(["long"]);
    state.set_surface_focused(true);
    state.set_cursor(Some("long"));
    let parts = Accordion::logs(&items, system)
        .max_content_height(5)
        .paint(area, frame.buffer_mut(), &mut state);
    if let Some(r) = parts.content_of(&"long")
        && r.height > 0
    {
        // Host would nest ScrollArea here; show capped viewport cue.
        Paragraph::new("line 1\nline 2\nline 3\nline 4\n… scroll")
            .style(system.style(Role::TextMuted))
            .render(r, frame.buffer_mut());
    }
}

fn collapsible_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Collapsible, CollapsibleState};
    use ratatui::widgets::Widget as _;
    let mut state = CollapsibleState::new().initially_open(true);
    state.set_focused(true);
    let body = Collapsible::new("Tool details", system).paint(area, frame.buffer_mut(), &mut state);
    if body.height > 0 {
        Paragraph::new("args: --json\nstatus: ok")
            .style(system.style(Role::TextMuted))
            .render(body, frame.buffer_mut());
    }
}

fn collapsible_section_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Collapsible, CollapsibleState};
    use ratatui::widgets::Widget as _;
    let mut state = CollapsibleState::new().initially_open(true);
    let body = Collapsible::new("Advanced options", system)
        .section()
        .paint(area, frame.buffer_mut(), &mut state);
    if body.height > 0 {
        Paragraph::new("retries: 3\ntimeout: 30s")
            .style(system.style(Role::Text))
            .render(body, frame.buffer_mut());
    }
}

fn collapsible_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Collapsible, CollapsibleState};
    use ratatui::widgets::Widget as _;
    let chunks = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]).split(area);
    let mut outer = CollapsibleState::new().initially_open(true);
    let body = Collapsible::new("Outer group", system).paint(
        chunks[0],
        frame.buffer_mut(),
        &mut outer,
    );
    if body.height > 0 {
        let mut inner = CollapsibleState::new().initially_open(true);
        let inner_body = Collapsible::new("Nested detail", system)
            .depth(1)
            .paint(body, frame.buffer_mut(), &mut inner);
        if inner_body.height > 0 {
            Paragraph::new("leaf content")
                .style(system.style(Role::TextMuted))
                .render(inner_body, frame.buffer_mut());
        }
    }
    let mut closed = CollapsibleState::new();
    let _ = Collapsible::new("Sibling closed", system).paint(
        chunks[1],
        frame.buffer_mut(),
        &mut closed,
    );
}

fn collapsible_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Collapsible, CollapsibleState};
    let mut state = CollapsibleState::new();
    let _ = Collapsible::new("Locked detail", system)
        .disabled(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn collapsible_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::GlyphSet;
    use termrock::widgets::{Collapsible, CollapsibleState};
    use ratatui::widgets::Widget as _;
    let ascii = system.clone().glyphs(GlyphSet::Ascii);
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Length(2)]).split(area);
    let mut open = CollapsibleState::new().initially_open(true);
    let body = Collapsible::new("Open ascii", &ascii).paint(
        chunks[0],
        frame.buffer_mut(),
        &mut open,
    );
    if body.height > 0 {
        Paragraph::new("body")
            .style(ascii.style(Role::TextMuted))
            .render(body, frame.buffer_mut());
    }
    let mut closed = CollapsibleState::new();
    let _ = Collapsible::new("Closed ascii", &ascii).paint(
        chunks[1],
        frame.buffer_mut(),
        &mut closed,
    );
}

fn collapsible_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Collapsible, CollapsibleState};
    use ratatui::widgets::Widget as _;
    let mut state = CollapsibleState::new().initially_open(true);
    let body = Collapsible::new("Very long optional detail title", system).paint(
        area,
        frame.buffer_mut(),
        &mut state,
    );
    if body.height > 0 {
        Paragraph::new("ok")
            .style(system.style(Role::TextMuted))
            .render(body, frame.buffer_mut());
    }
}

fn toolbar_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Toolbar, ToolbarItem, ToolbarState};
    let items = [
        ToolbarItem::action("save", "Save").hint("C-s").priority(90),
        ToolbarItem::action("open", "Open").priority(50),
        ToolbarItem::separator("s1"),
        ToolbarItem::toggle("wrap", "Wrap", true).priority(40),
        ToolbarItem::action("find", "Find").priority(30),
    ];
    let mut state = ToolbarState::horizontal();
    state.set_surface_focused(true);
    state.set_cursor(Some("save"));
    frame.render_stateful_widget(
        &Toolbar::new(&items, system).overflow_id("more"),
        area,
        &mut state,
    );
}

fn toolbar_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Toolbar, ToolbarItem, ToolbarState};
    let items = [
        ToolbarItem::action("save", "Save").priority(90),
        ToolbarItem::action("open", "Open").priority(40),
        ToolbarItem::action("find", "Find").priority(20),
        ToolbarItem::action("help", "Help").priority(10),
        ToolbarItem::action("prefs", "Prefs").priority(5),
    ];
    let mut state = ToolbarState::horizontal();
    state.set_surface_focused(true);
    frame.render_stateful_widget(
        &Toolbar::new(&items, system).overflow_id("more"),
        area,
        &mut state,
    );
}

fn toolbar_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Toolbar, ToolbarItem, ToolbarState};
    let items = [
        ToolbarItem::action("a", "Cut"),
        ToolbarItem::action("b", "Copy"),
        ToolbarItem::action("c", "Paste"),
    ];
    let mut state = ToolbarState::vertical();
    state.set_surface_focused(true);
    state.set_cursor(Some("a"));
    frame.render_stateful_widget(
        &Toolbar::new(&items, system).vertical().compact(),
        area,
        &mut state,
    );
}

fn toolbar_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Toolbar, ToolbarItem, ToolbarState};
    let items = [
        ToolbarItem::action("save", "Save").icon("💾").priority(90),
        ToolbarItem::action("open", "Open").icon("📂").priority(50),
        ToolbarItem::action("find", "Find").icon("🔍").priority(30),
    ];
    let mut state = ToolbarState::horizontal();
    state.set_surface_focused(true);
    frame.render_stateful_widget(
        &Toolbar::new(&items, system).compact().overflow_id("more"),
        area,
        &mut state,
    );
}

fn button_group_dialog_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ButtonGroupItem::new("cancel", "Cancel"),
        ButtonGroupItem::destructive("delete", "Delete"),
        ButtonGroupItem::primary("save", "Save").leading("✓"),
    ];
    let mut state = ButtonGroupState::new();
    state.set_surface_focused(true);
    state.cursor = Some("save");
    let _ = ButtonGroup::new(&items, system)
        .separated()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn button_group_connected_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ButtonGroupItem::quiet("day", "Day"),
        ButtonGroupItem::quiet("week", "Week"),
        ButtonGroupItem::quiet("month", "Month"),
    ];
    let mut state = ButtonGroupState::new();
    state.set_surface_focused(true);
    state.cursor = Some("week");
    let _ = ButtonGroup::new(&items, system)
        .connected()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn button_group_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ButtonGroupItem::quiet("more", "Details").priority(10),
        ButtonGroupItem::new("cancel", "Cancel").priority(60),
        ButtonGroupItem::primary("apply", "Apply"),
        ButtonGroupItem::destructive("reset", "Reset").priority(15),
    ];
    let mut state = ButtonGroupState::new();
    state.set_surface_focused(true);
    let _ = ButtonGroup::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn button_group_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ButtonGroupItem::new("cancel", "Cancel"),
        ButtonGroupItem::primary("save", "Save").loading(true),
    ];
    let mut state = ButtonGroupState::new();
    state.set_surface_focused(true);
    let _ = ButtonGroup::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn button_group_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ButtonGroupItem::new("cancel", "取消"),
        ButtonGroupItem::primary("ok", "保存 ✨"),
    ];
    let mut state = ButtonGroupState::new();
    state.set_surface_focused(true);
    let _ = ButtonGroup::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_pressed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut off = ToggleState::new();
    let mut on = ToggleState::with_value(ToggleValue::Pressed);
    on.set_focused(true);
    let row = Layout::horizontal([Constraint::Length(14), Constraint::Length(14)]).split(area);
    let _ = Toggle::new("Bold", system).paint(row[0], frame.buffer_mut(), &mut off);
    let _ = Toggle::new("Bold", system).paint(row[1], frame.buffer_mut(), &mut on);
}

fn toggle_icon_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ToggleState::with_value(ToggleValue::Pressed);
    state.set_focused(true);
    let _ = Toggle::new("", system)
        .icon("B")
        .accessible_label("Bold")
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_indeterminate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ToggleState::with_value(ToggleValue::Indeterminate);
    state.set_focused(true);
    let _ = Toggle::new("B", system)
        .accessible_label("Bold mixed")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ToggleState::with_value(ToggleValue::Pressed);
    state.set_focused(true);
    let _ = Toggle::new("強調 ✨", system).paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_group_format_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ToggleGroupItem::new("b", "B").pressed(true).priority(90),
        ToggleGroupItem::new("i", "I").pressed(true).priority(80),
        ToggleGroupItem::new("u", "U").priority(70),
    ];
    let mut state = ToggleGroupState::new();
    state.set_surface_focused(true);
    state.cursor = Some("i");
    let _ = ToggleGroup::new(&items, system)
        .multiple()
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_group_align_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ToggleGroupItem::new("l", "L").pressed(true),
        ToggleGroupItem::new("c", "C"),
        ToggleGroupItem::new("r", "R"),
    ];
    let mut state = ToggleGroupState::new();
    state.set_surface_focused(true);
    state.cursor = Some("l");
    let _ = ToggleGroup::new(&items, system)
        .single()
        .connected()
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_group_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ToggleGroupItem::new("x", "Extra").priority(10),
        ToggleGroupItem::new("b", "Bold").pressed(true).priority(90),
        ToggleGroupItem::new("i", "Italic").priority(40),
        ToggleGroupItem::new("u", "Under").priority(20),
        ToggleGroupItem::new("s", "Strike").priority(15),
    ];
    let mut state = ToggleGroupState::new();
    state.set_surface_focused(true);
    let _ = ToggleGroup::new(&items, system)
        .multiple()
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn toggle_group_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ToggleGroupItem::new("b", "粗").pressed(true),
        ToggleGroupItem::new("i", "斜"),
        ToggleGroupItem::new("u", "下"),
    ];
    let mut state = ToggleGroupState::new();
    state.set_surface_focused(true);
    let _ = ToggleGroup::new(&items, system)
        .multiple()
        .connected()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn action_bar(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [
        Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let mut state = ActionBarState {
        cursor: Some("accept"),
        ..ActionBarState::default()
    };
    frame.render_stateful_widget(
        &ActionBar::new(&actions, system).gap("  "),
        area,
        &mut state,
    );
}

pub(crate) fn tree_nodes() -> Vec<TreeNode<'static, &'static str>> {
    vec![
        TreeNode {
            id: "workspace",
            label: Line::from("Workspace"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: Some(Line::from("4 items")),
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
            parent: None,
        },
        TreeNode {
            id: "documents",
            label: Line::from("Documents"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: Some(Line::from("2 items")),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            parent: None,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Remote items"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
            parent: None,
        },
        TreeNode {
            id: "notes",
            label: Line::from("Wide 🧪 notes"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: Some(Line::from("12 KiB")),
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            parent: None,
        },
    ]
}

pub(crate) fn form_fields() -> Vec<Field<'static, &'static str>> {
    vec![
        Field::new("name", "Name", "Example profile")
            .help("A recognizable display name")
            .required(true)
            .dirty(true)
            .touched(true),
        Field::new("endpoint", "Endpoint", "localhost")
            .error("Enter a reachable address")
            .required(true)
            .touched(true),
        Field::new("mode", "Managed mode", "Unavailable").enabled(false),
    ]
}

fn form(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let fields = form_fields();
    let sections = [Fieldset::new("General", &fields).description("Profile settings")];
    let mut state = FormState::new();
    frame.render_stateful_widget(
        &Form::new(&sections, system).focused_field(Some(&"name")),
        area,
        &mut state,
    );
}

fn form_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let fields = form_fields();
    let sections = [Fieldset::new("General", &fields)];
    let mut state = FormState::new();
    frame.render_stateful_widget(
        &Form::new(&sections, system)
            .compact()
            .focused_field(Some(&"endpoint")),
        area,
        &mut state,
    );
}

fn form_validation_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let fields = [
        Field::new("email", "Email", "bad")
            .error("invalid email")
            .required(true)
            .touched(true),
        Field::new("token", "Token", "")
            .pending("checking…")
            .required(true),
        Field::new("note", "Note", "ok").warning("optional warning"),
    ];
    let sections = [Fieldset::new("Security", &fields)];
    let mut state = FormState::new();
    let _ = state.focus_first_invalid(&sections);
    frame.render_stateful_widget(
        &Form::new(&sections, system).focused_field(Some(&"email")),
        area,
        &mut state,
    );
}

pub(crate) fn render_split_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut SplitPaneState,
    system: &DesignSystem,
) {
    let split = SplitPane::new(
        SplitDirection::Horizontal,
        SPLIT_PANE_MIN,
        SPLIT_PANE_MAX,
        system,
    );
    let layout = split.layout(area, state);
    if !layout.first.is_empty() {
        frame.render_widget(
            Paragraph::new("First pane\nCaller-owned content"),
            layout.first,
        );
    }
    if !layout.second.is_empty() {
        frame.render_widget(
            Paragraph::new("Second pane\nDrag the divider"),
            layout.second,
        );
    }
    frame.render_stateful_widget(&split, area, state);
}

fn split_pane(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SplitPaneState::new(SplitRatio::from_percent(38));
    render_split_pane(frame, area, &mut state, system);
}

fn resizable_workbench_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        workbench_panels, Panel, ResizablePanelGroup, ResizablePanelGroupState,
    };
    let panels = workbench_panels();
    let group = ResizablePanelGroup::new(&panels, system).workbench();
    let mut state = ResizablePanelGroupState::new();
    let layout = group.layout(area, &mut state);
    group.paint_handles(area, frame.buffer_mut(), &mut state);
    for p in &layout.panels {
        if p.drawer || p.collapsed || p.area.width < 3 || p.area.height < 2 {
            continue;
        }
        let _ = Panel::new(system)
            .title(p.id.0.as_str())
            .paint(p.area, frame.buffer_mut(), None);
    }
}

fn resizable_dashboard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        dashboard_panels, Panel, ResizablePanelGroup, ResizablePanelGroupState,
    };
    let panels = dashboard_panels();
    let group = ResizablePanelGroup::new(&panels, system).dashboard();
    let mut state = ResizablePanelGroupState::new();
    let layout = group.layout(area, &mut state);
    group.paint_handles(area, frame.buffer_mut(), &mut state);
    for p in &layout.panels {
        if p.area.width < 3 || p.area.height < 2 {
            continue;
        }
        let _ = Panel::new(system)
            .title(p.id.0.as_str())
            .paint(p.area, frame.buffer_mut(), None);
    }
}

fn resizable_drawers_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        workbench_panels, Panel, ResizablePanelGroup, ResizablePanelGroupState,
    };
    let panels = workbench_panels();
    let group = ResizablePanelGroup::new(&panels, system)
        .workbench()
        .drawer_threshold(90);
    let mut state = ResizablePanelGroupState::new();
    let layout = group.layout(area, &mut state);
    group.paint_handles(area, frame.buffer_mut(), &mut state);
    for p in &layout.panels {
        if p.drawer {
            // Mark drawer candidates in footer of remaining main
            continue;
        }
        if p.area.width < 4 || p.area.height < 2 {
            continue;
        }
        let title = if state.drawer_ids().is_empty() {
            p.id.0.as_str()
        } else {
            "main (+drawers)"
        };
        let body = Panel::new(system)
            .title(title)
            .paint(p.area, frame.buffer_mut(), None);
        if body.width > 2 && !state.drawer_ids().is_empty() {
            let names: String = state
                .drawer_ids()
                .iter()
                .map(|d| d.0.as_str())
                .collect::<Vec<_>>()
                .join(",");
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                &format!("drawer:{names}"),
                usize::from(body.width),
                system.style(Role::Warning),
            );
        }
    }
}

fn tree_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let nodes: [TreeNode<'_, &str>; 0] = [];
    let mut state = TreeState::<&str>::default();
    frame.render_stateful_widget(
        &Tree::new(&nodes, &tokens).empty_message("No files in this folder"),
        area,
        &mut state,
    );
}

fn tree_loading_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::default())
        .selection(termrock::style::SelectionChrome::Gutter);
    let nodes = [
        TreeNode::new("root", Line::from("Workspace"), 0)
            .branch()
            .expanded(),
        TreeNode::new("pending", Line::from("Fetching children"), 1)
            .branch()
            .loading(),
        TreeNode::new("bad", Line::from("Permission denied"), 1).error(),
        TreeNode::new("ok", Line::from("src"), 1)
            .branch()
            .expanded(),
        TreeNode::new("leaf", Line::from("main.rs"), 2).badge(Line::from("rs")),
    ];
    let mut state = TreeState::new(Some("ok"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::default())
        .glyphs(termrock::style::GlyphSet::Ascii)
        .selection(termrock::style::SelectionChrome::Gutter);
    let nodes = tree_nodes();
    let mut state = TreeState::new(Some("workspace"));
    state.enable_multi_select();
    if let Some(sel) = state.selection_mut() {
        sel.toggle(&"workspace");
    }
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_composed(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let nodes = [
        TreeNode::new("pkg", Line::from("termrock"), 0)
            .branch()
            .expanded()
            .leading(Line::from("📦"))
            .badge(Line::from("crate")),
        TreeNode::new("lib", Line::from("lib.rs"), 1)
            .leading(Line::from("·"))
            .secondary(Line::from("public API"))
            .shortcut("⌘O"),
        TreeNode::new("mod", Line::from("widgets"), 1)
            .branch()
            .expanded()
            .secondary(Line::from("module")),
        TreeNode::new("tree", Line::from("tree.rs"), 2).badge(Line::from("rs")),
    ];
    let mut state = TreeState::new(Some("lib"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::Compact)
        .selection(termrock::style::SelectionChrome::Gutter);
    let nodes = [
        TreeNode::new("r", Line::from("Root"), 0)
            .branch()
            .expanded(),
        TreeNode::new("c", Line::from("Child"), 1)
            .badge(Line::from("99"))
            .shortcut("⌘K"),
    ];
    let mut state = TreeState::new(Some("c"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_deep(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::Comfortable)
        .selection(termrock::style::SelectionChrome::Gutter);
    let nodes = [
        TreeNode::new("d0", Line::from("depth-0"), 0)
            .branch()
            .expanded(),
        TreeNode::new("d1", Line::from("depth-1"), 1)
            .branch()
            .expanded(),
        TreeNode::new("d2", Line::from("depth-2"), 2)
            .branch()
            .expanded(),
        TreeNode::new("d3", Line::from("depth-3"), 3)
            .branch()
            .expanded(),
        TreeNode::new("d4", Line::from("depth-4 leaf"), 4).secondary(Line::from("file")),
        TreeNode::new("d4b", Line::from("depth-4 error"), 4).error(),
    ];
    let mut state = TreeState::new(Some("d4"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_lazy_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().selection(termrock::style::SelectionChrome::Gutter);
    let nodes = [
        TreeNode::new("root", Line::from("project"), 0)
            .branch()
            .expanded(),
        TreeNode::new("lazy", Line::from("node_modules"), 1)
            .lazy_branch()
            .parent("root")
            .secondary(Line::from("not loaded")),
        TreeNode::new("load", Line::from("fetching…"), 1)
            .branch()
            .loading()
            .parent("root"),
        TreeNode::new("err", Line::from("broken link"), 1)
            .error()
            .parent("root"),
        TreeNode::new("src", Line::from("src"), 1)
            .branch()
            .expanded()
            .parent("root"),
        TreeNode::new("main", Line::from("main.rs"), 2)
            .parent("src")
            .actions(Line::from("open")),
    ];
    let mut state = TreeState::new(Some("lazy"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tree_filter_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let all = [
        TreeNode::new("root", Line::from("Workspace"), 0)
            .branch()
            .expanded(),
        TreeNode::new("src", Line::from("src"), 1)
            .branch()
            .expanded()
            .parent("root"),
        TreeNode::new("lib", Line::from("lib.rs"), 2).parent("src"),
        TreeNode::new("mod", Line::from("mod.rs"), 2).parent("src"),
        TreeNode::new("docs", Line::from("docs"), 1).parent("root"),
    ];
    let mut state = TreeState::new(Some("lib"));
    state.set_filter_query(Some("lib".into()));
    let filtered: Vec<_> = filter_tree_with_ancestors(&all, "lib")
        .into_iter()
        .cloned()
        .collect();
    frame.render_stateful_widget(&Tree::new(&filtered, system), area, &mut state);
}

fn tree_actions_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = [
        TreeNode::new("f1", Line::from("README.md"), 0)
            .leading(Line::from("·"))
            .actions(Line::from("⏎ ⋯"))
            .shortcut("o")
            .badge(Line::from("md")),
        TreeNode::new("f2", Line::from("Cargo.toml"), 0)
            .actions(Line::from("edit"))
            .secondary(Line::from("manifest")),
    ];
    let mut state = TreeState::new(Some("f1"));
    frame.render_stateful_widget(&Tree::new(&nodes, system), area, &mut state);
}

fn tree(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let nodes = tree_nodes();
    let mut state = TreeState::new(Some("workspace"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"notes");
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn sidebar_settings_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_settings_nav();
    let mut state = SidebarState::new(Some("profile"));
    state.set_focused(true);
    Sidebar::new(&items, system)
        .ascii(true)
        .show_panel(true)
        .title("Settings")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn sidebar_database_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_database_nav();
    let mut state = SidebarState::new(Some("users"));
    state.set_focused(true);
    Sidebar::new(&items, system)
        .ascii(true)
        .show_panel(true)
        .title("Database")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn sidebar_agent_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_agent_workbench_nav();
    let mut state = SidebarState::new(Some("plan"));
    state.set_focused(true);
    Sidebar::new(&items, system)
        .ascii(true)
        .show_panel(true)
        .title("Workbench")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn sidebar_rail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_agent_workbench_nav();
    let mut state = SidebarState::new(Some("chat")).with_presentation(SidebarPresentation::Rail);
    state.set_focused(true);
    Sidebar::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn navigation_list_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        NavItem::new("a", "Inbox").badge("3"),
        NavItem::new("b", "Starred"),
        NavItem::new("c", "Archive"),
    ];
    let mut state = NavigationListState::new(Some("a"));
    state.set_focused(true);
    // focus moved to b, route still a
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Down,
            termrock::input::KeyModifiers::NONE,
        ),
        &items,
    );
    NavigationList::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn pagination_full_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PaginationState::new(3, 25, PageTotal::Known(240));
    state.set_focused(true);
    Pagination::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn pagination_unknown_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PaginationState::new(2, 50, PageTotal::Unknown);
    state.set_focused(true);
    Pagination::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn pagination_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PaginationState::new(4, 10, PageTotal::AtLeast(100));
    state.set_focused(true);
    state.set_loading(true);
    Pagination::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn pagination_minimal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PaginationState::new(1, 25, PageTotal::Known(500));
    state.set_focused(true);
    Pagination::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn pagination_jump_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PaginationState::new(1, 25, PageTotal::Known(1000));
    state.set_focused(true);
    let _ = state.handle_key(termrock::input::KeyEvent::new(
        termrock::input::KeyCode::Char('g'),
        termrock::input::KeyModifiers::NONE,
    ));
    let _ = state.handle_key(termrock::input::KeyEvent::new(
        termrock::input::KeyCode::Char('1'),
        termrock::input::KeyModifiers::NONE,
    ));
    let _ = state.handle_key(termrock::input::KeyEvent::new(
        termrock::input::KeyCode::Char('2'),
        termrock::input::KeyModifiers::NONE,
    ));
    Pagination::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_horizontal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len()).policy(StepperNavPolicy::Linear);
    state.set_focused(true);
    state.set_status(0, StepStatus::Complete);
    state.set_current(1, items.len(), true);
    Stepper::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len())
        .policy(StepperNavPolicy::Linear)
        .orientation(StepperOrientation::Vertical);
    state.set_focused(true);
    state.set_status(0, StepStatus::Complete);
    state.set_current(1, items.len(), true);
    state.set_status(2, StepStatus::Optional);
    Stepper::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len());
    state.set_focused(true);
    state.set_status(0, StepStatus::Complete);
    state.set_status(1, StepStatus::Error);
    state.set_current(1, items.len(), true);
    Stepper::new(&items, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_numeric_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len());
    state.set_focused(true);
    state.set_current(2, items.len(), false);
    state.set_presentation_override(Some(StepperPresentation::Numeric));
    Stepper::new(&items, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_menu_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len()).policy(StepperNavPolicy::Host);
    state.set_focused(true);
    state.set_current(0, items.len(), false);
    state.set_presentation_override(Some(StepperPresentation::Menu));
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Enter,
            termrock::input::KeyModifiers::NONE,
        ),
        &items,
    );
    Stepper::new(&items, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn stepper_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = example_onboarding_steps();
    let mut state = StepperState::with_len(items.len());
    state.set_focused(true);
    state.set_status(0, StepStatus::Complete);
    state.set_status(1, StepStatus::Skipped);
    state.set_current(2, items.len(), true);
    Stepper::new(&items, system)
        .ascii(true)
        .colorless(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_history_entries();
    let visible = filter_history_entries(&entries, "");
    let mut state = HistoryPickerState::new();
    let _ = state.open(None);
    state.reconcile(&visible);
    HistoryPicker::new(&visible, system).paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_search(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_history_entries();
    let visible = filter_history_entries(&entries, "git");
    let mut state = HistoryPickerState::new();
    let _ = state.open(None);
    *state.query_mut() = TextInputState::new("git").with_allow_empty(true);
    state.reconcile(&visible);
    HistoryPicker::new(&visible, system).paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_redacted(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_history_entries();
    let visible = filter_history_entries(&entries, "");
    let mut state = HistoryPickerState::new();
    let _ = state.open(None);
    state.set_redaction(history_redaction_secret());
    state.reconcile(&visible);
    HistoryPicker::new(&visible, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_draft(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_history_entries();
    let visible = filter_history_entries(&entries, "");
    let mut state = HistoryPickerState::new();
    let _ = state.open(Some("in-progress draft…".into()));
    state.reconcile(&visible);
    HistoryPicker::new(&visible, system).paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = HistoryPickerState::<&str>::new();
    let _ = state.open(None);
    HistoryPicker::new(&[], system).paint(area, frame.buffer_mut(), &mut state);
}

fn history_picker_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_history_entries();
    let visible = filter_history_entries(&entries, "");
    let mut state = HistoryPickerState::new();
    let _ = state.open(None);
    state.reconcile(&visible);
    HistoryPicker::new(&visible, system)
        .ascii(true)
        .colorless(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn keyboard_help_footer(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_help_entries(system);
    let mut state = KeyboardHelpState::new();
    KeyboardHelp::new(&entries, system).paint(area, frame.buffer_mut(), &mut state);
}

fn keyboard_help_modal(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_help_entries(system);
    let mut state = KeyboardHelpState::modal();
    let visible = filter_help_entries(&entries, "");
    KeyboardHelp::new(&visible, system).paint(area, frame.buffer_mut(), &mut state);
}

fn keyboard_help_search(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_help_entries(system);
    let visible = filter_help_entries(&entries, "save");
    let mut state = KeyboardHelpState::modal();
    *state.query_mut() = TextInputState::new("save").with_allow_empty(true);
    KeyboardHelp::new(&visible, system).paint(area, frame.buffer_mut(), &mut state);
}

fn keyboard_help_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_help_entries(system);
    let mut state = KeyboardHelpState::new();
    state.set_presentation_override(Some(termrock::widgets::KeyboardHelpPresentation::Tiny));
    KeyboardHelp::new(&entries, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn keyboard_help_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = example_help_entries(system);
    let mut state = KeyboardHelpState::new();
    KeyboardHelp::new(&entries, system)
        .ascii(true)
        .colorless(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tooltip_visible_state() -> TooltipState {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    let mut state = TooltipState::with_delay(Duration::ZERO);
    state.set_pointer_over(true);
    let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
    let _ = state.advance(tick, Motion::Off);
    state
}

fn tooltip_plain_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let state = tooltip_visible_state();
    Tooltip::new("Truncated path help", system).paint(area, frame.buffer_mut(), &state);
}

fn tooltip_shortcut_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let state = tooltip_visible_state();
    Tooltip::content(
        TooltipContent::plain("Save document")
            .shortcut("C-s")
            .essential_elsewhere(true),
        system,
    )
    .variant(TooltipVariant::Shortcut)
    .paint(area, frame.buffer_mut(), &state);
}

fn tooltip_rich_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let state = tooltip_visible_state();
    Tooltip::content(
        TooltipContent::plain("Writes the buffer to disk")
            .title("Save")
            .shortcut("C-s")
            .essential_elsewhere(true),
        system,
    )
    .rich()
    .paint(area, frame.buffer_mut(), &state);
}

fn tooltip_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let state = tooltip_visible_state();
    Tooltip::content(
        TooltipContent::plain("Status detail")
            .title("Info")
            .essential_elsewhere(true),
        system,
    )
    .rich()
    .ascii(true)
    .colorless(true)
    .paint(area, frame.buffer_mut(), &state);
}

fn menu_bar_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).paint(bar, frame.buffer_mut(), &mut state);
}

fn menu_bar_open_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let _ = state.open_menu_at(&menus, 0);
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).paint_all(bar, area, frame.buffer_mut(), &mut state);
}

fn menu_bar_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let _ = state.open_menu_at(&menus, 0);
    if let Some(export_idx) = menus[0].items.iter().position(|n| n.id == "export") {
        for _ in 0..12 {
            if state.panel_cursor(0) == Some(export_idx) {
                break;
            }
            let _ = state.handle_key(
                termrock::input::KeyEvent::new(
                    termrock::input::KeyCode::Down,
                    termrock::input::KeyModifiers::NONE,
                ),
                &menus,
            );
        }
        let _ = state.handle_key(
            termrock::input::KeyEvent::new(
                termrock::input::KeyCode::Right,
                termrock::input::KeyModifiers::NONE,
            ),
            &menus,
        );
    }
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).paint_all(bar, area, frame.buffer_mut(), &mut state);
}

fn menu_bar_mnemonic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let _ = state.set_mnemonic_mode(true);
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).paint(bar, frame.buffer_mut(), &mut state);
}

fn menu_bar_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    state.set_presentation_override(Some(termrock::widgets::MenuBarPresentation::CommandPalette));
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).ascii(true).paint(bar, frame.buffer_mut(), &mut state);
}

fn menu_bar_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{MenuBarMenu, MenuNode};
    let menus = vec![MenuBarMenu::new(
        "file",
        "ファイル",
        vec![
            MenuNode::command("open", "開く 📂").mnemonic('開'),
            MenuNode::command("save", "保存 ✨").mnemonic('保'),
            MenuNode::checkbox("wrap", "折り返し", true),
        ],
    )
    .mnemonic('フ')];
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let _ = state.open_menu_at(&menus, 0);
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system).paint_all(bar, area, frame.buffer_mut(), &mut state);
}

fn menu_bar_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let menus = example_app_menus();
    let mut state = MenuBarState::new();
    state.set_focused(true);
    let _ = state.open_menu_at(&menus, 2); // View: radio + checkbox
    let bar = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    MenuBar::new(&menus, system)
        .ascii(true)
        .paint_all(bar, area, frame.buffer_mut(), &mut state);
}

fn breadcrumbs_path_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        BreadcrumbItem::new("home", "home"),
        BreadcrumbItem::new("proj", "projects"),
        BreadcrumbItem::new("tr", "termrock"),
        BreadcrumbItem::new("src", "src").current(true),
    ];
    let mut state = BreadcrumbsState::new();
    state.set_focused(true);
    state.set_focus_index(2);
    let _ = Breadcrumbs::new(&items, system)
        .ascii(true)
        .separator(BreadcrumbSeparator::Slash)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn breadcrumbs_collapsed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        BreadcrumbItem::new("r", "root"),
        BreadcrumbItem::new("a", "alpha"),
        BreadcrumbItem::new("b", "beta"),
        BreadcrumbItem::new("c", "gamma"),
        BreadcrumbItem::new("d", "current").current(true),
    ];
    let mut state = BreadcrumbsState::new();
    state.set_focused(true);
    state.set_focus_ellipsis(true);
    let _ = Breadcrumbs::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn breadcrumbs_editable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        BreadcrumbItem::new("h", "home"),
        BreadcrumbItem::new("p", "proj").current(true),
    ];
    let mut state = BreadcrumbsState::new().with_editable(true);
    state.set_focused(true);
    let _ = state.start_edit(&items);
    let _ = Breadcrumbs::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn breadcrumbs_status_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        BreadcrumbItem::new("db", "analytics"),
        BreadcrumbItem::new("sch", "public").status(BreadcrumbStatus::Warning),
        BreadcrumbItem::new("t", "users")
            .status(BreadcrumbStatus::Error)
            .current(true),
    ];
    let mut state = BreadcrumbsState::new();
    state.set_focused(true);
    let _ = Breadcrumbs::new(&items, system)
        .ascii(true)
        .separator(BreadcrumbSeparator::Chevron)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn breadcrumbs_schema_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        BreadcrumbItem::new("c", "cluster"),
        BreadcrumbItem::new("d", "db"),
        BreadcrumbItem::new("s", "schema"),
        BreadcrumbItem::new("t", "table").current(true),
    ];
    let mut state = BreadcrumbsState::new();
    state.set_focused(true);
    let _ = Breadcrumbs::new(&items, system)
        .ascii(false)
        .separator(BreadcrumbSeparator::Chevron)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tree_navigation_project_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_project_tree();
    let mut state = TreeNavigationState::new(Some("main"));
    state.set_focused(true);
    state.reconcile_route(&nodes);
    state.focus_route(&nodes);
    TreeNavigation::new(&nodes, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tree_navigation_schema_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_schema_tree();
    let mut state = TreeNavigationState::new(Some("users"));
    state.set_focused(true);
    state.reconcile_route(&nodes);
    state.focus_route(&nodes);
    TreeNavigation::new(&nodes, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tree_navigation_settings_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_settings_tree();
    let mut state = TreeNavigationState::new(Some("tools"));
    state.set_focused(true);
    state.reconcile_route(&nodes);
    state.focus_route(&nodes);
    TreeNavigation::new(&nodes, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tree_navigation_docs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_docs_tree();
    let mut state = TreeNavigationState::new(Some("intro"));
    state.set_focused(true);
    state.reconcile_route(&nodes);
    state.focus_route(&nodes);
    TreeNavigation::new(&nodes, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tree_navigation_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_project_tree();
    let mut state = TreeNavigationState::new(Some("lib"));
    state.set_focused(true);
    state.reconcile_route(&nodes);
    TreeNavigation::new(&nodes, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tabs(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab {
            id: "overview",
            label: "Overview",
            glyph: Some(Span::styled("●", system.style(Role::Success))),
            badge: None,
            status: TabStatus::Success,
            active: true,
            enabled: true,
            closable: false,
        },
        Tab {
            id: "details",
            label: "Details",
            glyph: None,
            badge: Some("2"),
            status: TabStatus::None,
            active: false,
            enabled: true,
            closable: false,
        },
        Tab {
            id: "logs",
            label: "Logs",
            glyph: None,
            badge: None,
            status: TabStatus::Running,
            active: false,
            enabled: true,
            closable: false,
        },
    ];
    let mut state = TabsState::new().with_selected("overview");
    state.set_focused(true);
    frame.render_stateful_widget(&Tabs::new(&items, system).gap(1), area, &mut state);
}

fn tabs_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("a", "Overview"),
        Tab::new("b", "Metrics"),
        Tab::new("c", "Logs"),
        Tab::new("d", "Traces"),
        Tab::new("e", "Settings"),
        Tab::new("f", "History"),
    ];
    let mut state = TabsState::new().with_selected("a");
    state.set_focused(true);
    Tabs::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
    let _ = state.presentation(); // exercise presentation for story
}

fn tabs_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("overview", "Overview").status(TabStatus::Success),
        Tab::new("details", "Details"),
        Tab::new("logs", "Logs").status(TabStatus::Running),
    ];
    let mut state = TabsState::new()
        .with_selected("overview")
        .with_orientation(TabsOrientation::Vertical);
    state.set_focused(true);
    Tabs::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tabs_manual_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("a", "First"),
        Tab::new("b", "Second"),
        Tab::new("c", "Third"),
    ];
    let mut state = TabsState::new()
        .with_selected("a")
        .with_activation(TabsActivation::Manual);
    state.set_focused(true);
    // focus moved to second without selecting
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Right,
            termrock::input::KeyModifiers::NONE,
        ),
        &items,
    );
    Tabs::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn tabs_closable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("main", "main.rs").closable(true).status(TabStatus::Dirty),
        Tab::new("lib", "lib.rs").closable(true),
        Tab::new("mod", "mod.rs").closable(true).status(TabStatus::Error),
    ];
    let mut state = TabsState::new().with_selected("main");
    state.set_focused(true);
    Tabs::new(&items, system)
        .ascii(true)
        .show_close(true)
        .paint(area, frame.buffer_mut(), &mut state);
    let _ = TabsPresentation::Expanded;
}

fn hint_bar(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let system = if system.palette() == &RolePalette::tailrocks_phosphor() {
        DesignSystem::from_palette(
            system
                .palette()
                .clone()
                .with_role(Role::HintKey, Style::new().bold())
                .with_role(Role::HintText, Style::new())
                .with_role(Role::HintDim, Style::new())
                .with_role(Role::HintSeparator, Style::new()),
        )
    } else {
        system.clone()
    };
    let hints = [
        Hint {
            chord: "↑↓",
            label: "navigate",
            priority: 1,
            visible: true,
        },
        Hint {
            chord: "Enter",
            label: "choose",
            priority: 1,
            visible: true,
        },
        Hint {
            chord: "Esc",
            label: "close",
            priority: 2,
            visible: true,
        },
    ];
    frame.render_widget(HintBar::new(&hints, &system).separator("  "), area);
}

fn list(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = list_rows();
    let mut state = ListState::new(Some("beta"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"alpha");
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_multi(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::default())
        .selection(termrock::style::SelectionChrome::Gutter);
    let rows = list_rows();
    let mut state = ListState::new(Some("beta"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"alpha");
    state.selection_mut().unwrap().toggle(&"beta");
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows: [ListRow<'_, &str>; 0] = [];
    let mut state = ListState::<&str>::default();
    let list = List::new(&rows, &tokens).empty_message(Line::from("No matching items"));
    frame.render_stateful_widget(&list, area, &mut state);
}

fn list_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow::item("ready", Line::from("Ready job")).badge(Line::from("ok")),
        ListRow::item("busy", Line::from("Fetching metrics")).loading(),
        ListRow::item("queued", Line::from("Queued deploy"))
            .secondary(Line::from("waiting"))
            .loading(),
    ];
    let mut state = ListState::new(Some("busy"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_disabled(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow::item("live", Line::from("Live service")),
        ListRow::item("off", Line::from("Suspended")).disabled(),
        ListRow::item("next", Line::from("Next target")),
    ];
    let mut state = ListState::new(Some("live"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(termrock::style::Density::default())
        .glyphs(termrock::style::GlyphSet::Ascii)
        .selection(termrock::style::SelectionChrome::Gutter);
    let rows = list_rows();
    let mut state = ListState::new(Some("beta"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"beta");
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_composed(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow::item("build", Line::from("Build"))
            .leading(Line::from("*"))
            .secondary(Line::from("src/lib.rs"))
            .badge(Line::from("ok"))
            .shortcut("⌘B"),
        ListRow::item("test", Line::from("Test"))
            .leading(Line::from("›"))
            .secondary(Line::from("crates/termrock"))
            .badge(Line::from("12"))
            .shortcut("⌘T"),
        ListRow::item("lint", Line::from("Lint"))
            .loading()
            .shortcut("⌘L"),
    ];
    let mut state = ListState::new(Some("build"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::Compact);
    let rows = [
        ListRow::item("id", Line::from("Identity"))
            .badge(Line::from("99"))
            .shortcut("⌘K"),
        ListRow::item("meta", Line::from("Metadata")).secondary(Line::from("path")),
    ];
    let mut state = ListState::new(Some("id"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn list_comfortable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = [
        ListRow::item("a", Line::from("Agent run"))
            .secondary(Line::from("prod · 2m ago"))
            .status(Line::from("ok"))
            .shortcut("a"),
        ListRow::item("b", Line::from("Sync workspace"))
            .secondary(Line::from("queued behind network"))
            .status(Line::from("wait")),
    ];
    let mut state = ListState::new(Some("a"));
    frame.render_stateful_widget(
        &List::new(&rows, system).comfortable(),
        area,
        &mut state,
    );
}

fn list_groups_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = [
        ListRow::group_header("g1", Line::from("Running")),
        ListRow::item("r1", Line::from("build"))
            .leading(Line::from("◉"))
            .status(Line::from("run"))
            .actions(Line::from("stop"))
            .shortcut("b"),
        ListRow::item("r2", Line::from("test")).status(Line::from("run")),
        ListRow::group_header("g2", Line::from("Queued")),
        ListRow::item("q1", Line::from("lint")).badge(Line::from("1")),
    ];
    let mut state = ListState::new(Some("r1"));
    state.set_selection_mode(ListSelectionMode::Range);
    frame.render_stateful_widget(&List::new(&rows, system), area, &mut state);
}

fn list_search_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let all = [
        ListRow::item("alpha", Line::from("Alpha")),
        ListRow::item("beta", Line::from("Beta")),
        ListRow::item("gamma", Line::from("Gamma")),
    ];
    let mut state = ListState::new(Some("beta"));
    state.set_search_query(Some("be".into()));
    let filtered: Vec<_> = filter_list_rows(&all, state.search_query().unwrap_or(""))
        .into_iter()
        .cloned()
        .collect();
    frame.render_stateful_widget(&List::new(&filtered, system), area, &mut state);
}

fn list_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow {
            id: "cjk",
            label: Line::from("東京 設定"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("日本語")),
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "emoji",
            label: Line::from("🧪 Laboratory"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("✅")),
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "combining",
            label: Line::from("Cafe\u{301} profile"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: Some(Line::from("e\u{301}")),
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let mut state = ListState::new(Some("cjk"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

pub(crate) fn list_rows() -> [ListRow<'static, &'static str>; 4] {
    [
        ListRow {
            id: "section",
            label: Line::from("Workspace"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("3 entries")),
                custom: None,
            role: RowRole::Separator,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "alpha",
            label: Line::from("Alpha"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("12 ms")),
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "beta",
            label: Line::from("Beta"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("28 ms")),
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "gamma",
            label: Line::from("Gamma"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: false,
            loading: false,
        },
    ]
}

fn picker_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = picker_rows("");
    let mut state = PickerState::new(Some("alpha"));
    frame.render_stateful_widget(&Picker::new(&rows, &tokens), area, &mut state);
}

fn picker_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let mut state = PickerState::<&str>::new(None);
    frame.render_stateful_widget(&Picker::new(&[], &tokens), area, &mut state);
}

fn picker_narrow_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow {
            id: "tokyo",
            label: Line::from("東京デプロイ 🧪"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: Some(Line::from("操作")),
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "cafe",
            label: Line::from("Cafe\u{301} logs"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: Some(Line::from("表示")),
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let mut state = PickerState::new(Some("tokyo"));
    let _ = state.query_mut().insert_str("東");
    frame.render_stateful_widget(&Picker::new(&rows, &tokens), area, &mut state);
}

fn text_input_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("東京🧪 Cafe\u{301}");
    assert!(state.set_cursor_byte("東京".len()));
    frame.render_stateful_widget(
        &TextInput::new("Query", system).validation(Validation::Valid),
        area,
        &mut state,
    );
}

pub(crate) fn picker_rows(query: &str) -> Vec<ListRow<'static, &'static str>> {
    let query = query.to_ascii_lowercase();
    [
        ("alpha", "Alpha project", "workspace"),
        ("beta", "Beta release", "command"),
        ("gamma", "Gamma logs", "view"),
        ("delta", "Delta settings", "command"),
    ]
    .into_iter()
    .filter(|(_, label, _)| label.to_ascii_lowercase().contains(&query))
    .map(|(id, label, kind)| {
        let mut row = ListRow::item(id, Line::from(label));
        row.trailing = Some(Line::from(kind));
        row
    })
    .collect()
}

fn detail_table(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
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
            id: "link",
            label: "Reference",
            value: "https://example.invalid",
            href: Some("https://example.invalid"),
            capability: DetailCapability::CopyAndLink,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &DetailTable::new(&rows, system).label_width(14).wrap(true),
        area,
        &mut state,
    );
}

fn detail_table_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        DetailRow {
            id: "region",
            label: "地域",
            value: "東京 🇯🇵",
            href: None,
            capability: DetailCapability::None,
            emphasis: true,
            style: None,
        },
        DetailRow {
            id: "status",
            label: "状態",
            value: "準備完了 ✅ Cafe\u{301}",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &DetailTable::new(&rows, system).label_width(8).wrap(true),
        area,
        &mut state,
    );
}

fn object_inspector_flat(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::InspectKind;
    let fields = [
        InspectorField::new("id", "pod-7f3a")
            .path("id")
            .kind(InspectKind::String),
        InspectorField::new("name", "api-gateway")
            .path("name")
            .kind(InspectKind::String),
        InspectorField::new("status", "Running")
            .path("status")
            .kind(InspectKind::String),
        InspectorField::new("restarts", "0")
            .path("restarts")
            .kind(InspectKind::Number),
    ];
    let mut state = ObjectInspectorState::new();
    state.set_cursor(1);
    ObjectInspector::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_nested(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::InspectKind;
    let fields = [
        InspectorField::container("spec", "spec", InspectKind::Object)
            .child_count(2)
            .expanded(),
        InspectorField::container("containers", "spec.containers", InspectKind::Array)
            .depth(1)
            .child_count(1)
            .expanded(),
        InspectorField::new("image", "ghcr.io/app:1.2")
            .path("spec.containers[0].image")
            .depth(2)
            .kind(InspectKind::String),
        InspectorField::new("ports", "8080/TCP")
            .path("spec.containers[0].ports")
            .depth(2)
            .kind(InspectKind::String),
        InspectorField::new("地域", "東京 🇯🇵")
            .path("spec.region")
            .depth(1)
            .kind(InspectKind::String),
    ];
    let mut state = ObjectInspectorState::new();
    state.set_expanded("spec", true);
    state.set_expanded("spec.containers", true);
    state.set_cursor(2);
    ObjectInspector::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ObjectInspectorState::new();
    ObjectInspector::new(&[], system).render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let fields = [
        InspectorField::new("id", "42").path("id"),
        InspectorField::new("kind", "blob").path("kind"),
    ];
    let mut state = ObjectInspectorState::new();
    ObjectInspector::new(&fields, system)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_json(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::InspectKind;
    let fields = [
        InspectorField::container("root", "$", InspectKind::Object)
            .child_count(3)
            .expanded(),
        InspectorField::new("ok", "true")
            .path("$.ok")
            .depth(1)
            .kind(InspectKind::Bool),
        InspectorField::new("count", "3")
            .path("$.count")
            .depth(1)
            .kind(InspectKind::Number),
        InspectorField::container("items", "$.items", InspectKind::Array)
            .depth(1)
            .child_count(2)
            .expanded(),
        InspectorField::new("0", "\"alpha\"")
            .path("$.items[0]")
            .depth(2)
            .kind(InspectKind::String),
        InspectorField::new("1", "\"beta\\nline\"")
            .path("$.items[1]")
            .depth(2)
            .kind(InspectKind::String),
        InspectorField::new("token", "sk-live-secret")
            .path("$.token")
            .depth(1)
            .kind(InspectKind::String)
            .secret(),
    ];
    let mut state = ObjectInspectorState::new();
    state.set_expanded("$", true);
    state.set_expanded("$.items", true);
    state.set_cursor(6);
    ObjectInspector::new(&fields, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_compare(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{InspectKind, InspectMode};
    let fields = [
        InspectorField::new("host", "api.prod.example")
            .path("host")
            .kind(InspectKind::String)
            .compare("api.staging.example"),
        InspectorField::new("port", "443")
            .path("port")
            .kind(InspectKind::Number)
            .compare("443"),
        InspectorField::new("tls", "1.3")
            .path("tls")
            .kind(InspectKind::String)
            .compare("1.2"),
    ];
    let mut state = ObjectInspectorState::new();
    state.mode = InspectMode::Compare;
    state.set_cursor(0);
    ObjectInspector::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_lazy(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::InspectKind;
    let fields = [
        InspectorField::container("payload", "payload", InspectKind::Object)
            .child_count(1000)
            .lazy(),
        InspectorField::new("meta", "partial")
            .path("meta")
            .kind(InspectKind::String),
    ];
    let mut state = ObjectInspectorState::new();
    state.set_cursor(0);
    ObjectInspector::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn object_inspector_fullscreen(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{InspectKind, InspectPresentation};
    let fields = [
        InspectorField::container("debug", "debug", InspectKind::Object)
            .child_count(2)
            .expanded(),
        InspectorField::new("thread", "main")
            .path("debug.thread")
            .depth(1)
            .kind(InspectKind::String),
        InspectorField::new("frames", "48")
            .path("debug.frames")
            .depth(1)
            .kind(InspectKind::Number)
            .editable(),
    ];
    let mut state = ObjectInspectorState::new();
    state.presentation = InspectPresentation::Fullscreen;
    state.set_expanded("debug", true);
    ObjectInspector::new(&fields, system)
        .presentation(InspectPresentation::Fullscreen)
        .render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_sample_lines() -> [LogLine<'static>; 8] {
    [
        LogLine::new("1", LogLevel::Info, "scheduler start")
            .timestamp("12:00:00")
            .source("main"),
        LogLine::new("2", LogLevel::Debug, "load config 東京")
            .timestamp("12:00:01")
            .source("cfg"),
        LogLine::new("3", LogLevel::Warn, "retry connect")
            .timestamp("12:00:02")
            .source("net")
            .batch_count(3),
        LogLine::new("4", LogLevel::Error, "upstream timeout")
            .timestamp("12:00:03")
            .source("api"),
        LogLine::new("5", LogLevel::Info, "recovered")
            .timestamp("12:00:04")
            .source("api"),
        LogLine::new("6", LogLevel::Trace, "tick 1")
            .timestamp("12:00:05")
            .batch_count(8),
        LogLine::new("7", LogLevel::Info, "ready 🧪")
            .timestamp("12:00:06")
            .source("main"),
        LogLine::new("8", LogLevel::Info, "serving :8080")
            .timestamp("12:00:07")
            .source("http"),
    ]
}

fn log_stream_follow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = log_stream_sample_lines();
    let mut state = LogStreamState::new();
    state.recipe = termrock::widgets::LogLineRecipe::Detailed;
    state.on_append(lines.len() as u16, area.height.saturating_sub(1));
    LogStream::new(&lines, system)
        .title("app.log")
        .render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_structured(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = log_stream_sample_lines();
    let mut state = LogStreamState::new();
    state.recipe = termrock::widgets::LogLineRecipe::Detailed;
    state.on_append(lines.len() as u16, area.height.saturating_sub(1));
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Home,
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
    );
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('m'),
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
    );
    LogStream::new(&lines, system)
        .title("stern · pods/*")
        .render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = log_stream_sample_lines();
    let mut state = LogStreamState::new();
    state.set_following(false);
    state.search = Some("timeout".into());
    state.level_floor = LogLevel::Warn;
    state.on_append(lines.len() as u16, area.height.saturating_sub(1));
    LogStream::new(&lines, system).render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_dropped(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = log_stream_sample_lines();
    let mut state = LogStreamState::new();
    state.report_dropped(128);
    state.report_batched(64);
    state.set_reconnect_message(Some("stream resumed after gap".into()));
    state.on_append(lines.len() as u16, area.height.saturating_sub(1));
    LogStream::new(&lines, system).render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LogStreamState::new();
    LogStream::new(&[], system).render(area, frame.buffer_mut(), &mut state);
}

fn log_stream_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = log_stream_sample_lines();
    let mut state = LogStreamState::new();
    state.recipe = termrock::widgets::LogLineRecipe::Compact;
    state.on_append(lines.len() as u16, area.height.saturating_sub(1));
    LogStream::new(&lines, system)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn event_stream_sample() -> Vec<StreamEvent<'static, &'static str>> {
    vec![
        StreamEvent::group("ns", "kube-system"),
        StreamEvent::with_id("e1", "Normal", "12:01:00", "Scheduled pod api-7")
            .severity(EventSeverity::Info)
            .source("scheduler")
            .fields("node=n1")
            .correlation("deploy-9")
            .group_key("kube-system"),
        StreamEvent::with_id("e2", "Warning", "12:01:01", "FailedMount")
            .severity(EventSeverity::Warn)
            .source("kubelet")
            .fields("vol=cfg")
            .detail("MountVolume.SetUp failed for volume \"cfg\"")
            .batch_count(3)
            .group_key("kube-system"),
        StreamEvent::with_id("e3", "tool.call", "12:01:02", "run_terminal_command")
            .severity(EventSeverity::Info)
            .source("agent")
            .correlation("turn-4")
            .detail("cargo test -p termrock --lib"),
        StreamEvent::with_id("e4", "Error", "12:01:03", "CrashLoopBackOff")
            .severity(EventSeverity::Error)
            .source("kubelet")
            .group_key("kube-system"),
        StreamEvent::with_id("e5", "Normal", "12:01:04", "Pulled image")
            .severity(EventSeverity::Info)
            .source("kubelet")
            .group_key("kube-system"),
    ]
}

fn event_stream_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = event_stream_sample();
    let mut state = EventStreamState::new();
    state.set_following(false);
    state.cursor = 2;
    state.on_append(events.len() as u16, area.height.saturating_sub(1));
    EventStream::with_events(&events, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn event_stream_burst(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        StreamEvent::with_id("b1", "Warning", "12:02:00", "Probe failed")
            .severity(EventSeverity::Warn)
            .source("kubelet")
            .batch_count(64),
        StreamEvent::with_id("b2", "Warning", "12:02:01", "Probe failed")
            .severity(EventSeverity::Warn)
            .source("kubelet")
            .batch_count(64),
        StreamEvent::with_id("b3", "Error", "12:02:02", "Unhealthy")
            .severity(EventSeverity::Error)
            .source("kubelet")
            .batch_count(12),
    ];
    let mut state = EventStreamState::new();
    state.report_backpressure(120, 64);
    state.set_following(true);
    state.on_append(events.len() as u16, area.height.saturating_sub(1));
    EventStream::with_events(&events, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn event_stream_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = event_stream_sample();
    let mut state = EventStreamState::new();
    state.set_following(false);
    state.set_severity_floor(EventSeverity::Warn);
    state.filter = Some("Mount".into());
    state.on_append(events.len() as u16, area.height.saturating_sub(1));
    EventStream::with_events(&events, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn event_stream_detail(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = event_stream_sample();
    let mut state = EventStreamState::new();
    state.set_following(false);
    state.cursor = 2;
    state.selected = Some("e2");
    state.detail_open = true;
    state.on_append(events.len() as u16, area.height.saturating_sub(1));
    EventStream::with_events(&events, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn event_stream_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = event_stream_sample();
    let mut state = EventStreamState::new();
    state.set_following(false);
    state.on_append(events.len() as u16, area.height.saturating_sub(1));
    EventStream::with_events(&events, system)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_sample() -> (
    Vec<DiffLine<'static>>,
    [DiffHunk; 2],
    [DiffReviewFileRow<'static>; 2],
) {
    let lines = vec![
        DiffLine::hunk_header("h0", "@@ -1,4 +1,5 @@")
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::context("c1", "fn main() {")
            .old_no(1)
            .new_no(1)
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::removed("r1", "    println!(\"hi\");")
            .old_no(2)
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::added("a1", "    println!(\"hello 東京\");")
            .new_no(2)
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::added("a2", "    // ready 🧪")
            .new_no(3)
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::context("c2", "}")
            .old_no(3)
            .new_no(4)
            .hunk_id("h0")
            .file_id("main.rs"),
        DiffLine::hunk_header("h1", "@@ -20,3 +21,3 @@")
            .hunk_id("h1")
            .file_id("lib.rs"),
        DiffLine::removed("r2", "old")
            .old_no(20)
            .hunk_id("h1")
            .file_id("lib.rs"),
        DiffLine::added("a3", "new")
            .new_no(21)
            .hunk_id("h1")
            .file_id("lib.rs"),
        DiffLine::context("c3", "context")
            .old_no(21)
            .new_no(22)
            .hunk_id("h1")
            .file_id("lib.rs"),
    ];
    let hunks = [
        DiffHunk::new(0, 6, "@@ -1,4 +1,5 @@")
            .id("h0")
            .file_id("main.rs"),
        DiffHunk::new(6, 4, "@@ -20,3 +21,3 @@")
            .id("h1")
            .file_id("lib.rs"),
    ];
    let files = [
        DiffReviewFileRow::new("main.rs", "src/main.rs").stats(2, 1),
        DiffReviewFileRow::new("lib.rs", "src/lib.rs").stats(1, 1),
    ];
    (lines, hunks, files)
}

fn diff_review_hunks(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks, files) = diff_review_sample();
    let mut state = DiffReviewState::new();
    state.set_hunk_cursor(0);
    DiffReview::new(&lines, system)
        .hunks(&hunks)
        .files(&files)
        .title("PR · agent review")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_decisions(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks, files) = diff_review_sample();
    let mut state = DiffReviewState::new();
    state.hydrate_decision(DiffReviewUnit::hunk("h0"), DiffDecision::Approved);
    state.hydrate_decision(DiffReviewUnit::hunk("h1"), DiffDecision::Staged);
    // select h0 via public path
    let _ = state.handle_key_lines(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char(' '),
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
        &hunks,
        &files,
    );
    DiffReview::new(&lines, system)
        .hunks(&hunks)
        .files(&files)
        .title("decisions")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_comments(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks, files) = diff_review_sample();
    let mut state = DiffReviewState::new();
    state.view.cursor = 3;
    state.comment_draft = Some("prefer format! here".into());
    state.region = termrock::widgets::DiffReviewRegion::Comments;
    DiffReview::new(&lines, system)
        .hunks(&hunks)
        .files(&files)
        .title("comments")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_confirm(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks, files) = diff_review_sample();
    let mut state = DiffReviewState::new();
    let _ = state.handle_key_lines(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char(' '),
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
        &hunks,
        &files,
    );
    state.view.hunk_cursor = 1;
    let _ = state.handle_key_lines(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char(' '),
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
        &hunks,
        &files,
    );
    let _ = state.handle_key_lines(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('r'),
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
        &hunks,
        &files,
    );
    DiffReview::new(&lines, system)
        .hunks(&hunks)
        .files(&files)
        .title("confirm reject")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = DiffReviewState::new();
    DiffReview::new(&[], system).render(area, frame.buffer_mut(), &mut state);
}

fn diff_review_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks, files) = diff_review_sample();
    let mut state = DiffReviewState::new();
    state.set_hunk_cursor(1);
    state.hydrate_decision(DiffReviewUnit::hunk("h1"), DiffDecision::Rejected);
    DiffReview::new(&lines, system)
        .hunks(&hunks)
        .files(&files)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn diagnostic_list(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let labels = [
        SourceLabel::primary(SourceRange::line_span(2, 5, 12)).label("expected `i32`"),
    ];
    let items = [
        Diagnostic::new("d1", DiagnosticSeverity::Error, "mismatched types")
            .code("E0308")
            .source("rustc")
            .file("src/main.rs")
            .labels(&labels),
        Diagnostic::new("d2", DiagnosticSeverity::Warning, "unused variable: `y`")
            .code("unused_variables")
            .source("rustc")
            .file("src/main.rs"),
        Diagnostic::new("d3", DiagnosticSeverity::Info, "build finished with warnings")
            .source("cargo"),
    ];
    let mut state = DiagnosticState::new();
    DiagnosticView::new(&items, system)
        .recipe(DiagnosticRecipe::List)
        .title("Problems")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diagnostic_full(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let labels = [
        SourceLabel::primary(SourceRange::line_span(2, 5, 12)).label("expected `i32`"),
        SourceLabel::secondary(SourceRange::line_span(2, 14, 18)).label("found here"),
    ];
    let notes = [DiagnosticNote::note("type annotations needed")];
    let fixes = [SuggestedFix::new("f1", "add type ascription").replacement("let x: i32 = foo()")];
    let lines = [
        CodeFrameLine::new(1, "fn main() {"),
        CodeFrameLine::new(2, "    let x = foo();"),
        CodeFrameLine::new(3, "    println!(\"{x} 東京\");"),
        CodeFrameLine::new(4, "}"),
    ];
    let items = [Diagnostic::new("d1", DiagnosticSeverity::Error, "mismatched types")
        .code("E0308")
        .source("rustc")
        .file("src/main.rs")
        .labels(&labels)
        .notes(&notes)
        .help("consider specifying the type explicitly")
        .docs_url("https://doc.rust-lang.org/error_codes/E0308.html")
        .fixes(&fixes)];
    let mut state = DiagnosticState::new();
    state.set_expanded("d1", true);
    DiagnosticView::new(&items, system)
        .recipe(DiagnosticRecipe::Full)
        .source_lines(&lines)
        .title("error[E0308]")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diagnostic_inline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [Diagnostic::new(
        "f1",
        DiagnosticSeverity::Error,
        "email is required",
    )
    .source("form")];
    let mut state = DiagnosticState::new();
    DiagnosticView::new(&items, system)
        .recipe(DiagnosticRecipe::Inline)
        .render(area, frame.buffer_mut(), &mut state);
}

fn code_frame_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let labels = [
        SourceLabel::primary(SourceRange::line_span(2, 5, 12)).label("expected `i32`"),
        SourceLabel::secondary(SourceRange::line_span(2, 14, 18)).label("found"),
    ];
    let lines = [
        CodeFrameLine::new(1, "fn main() {"),
        CodeFrameLine::new(2, "    let x = foo();"),
        CodeFrameLine::new(3, "}"),
    ];
    let _ = CodeFrame::new(&lines, system)
        .labels(&labels)
        .file("src/main.rs")
        .truncated_above(true)
        .truncated_below(true)
        .render(area, frame.buffer_mut());
}

fn diagnostic_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = DiagnosticState::new();
    DiagnosticView::new(&[], system).render(area, frame.buffer_mut(), &mut state);
}

fn diagnostic_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let labels = [SourceLabel::primary(SourceRange::line_span(1, 1, 4))];
    let lines = [CodeFrameLine::new(1, "\tlet x = 1;")];
    let items = [Diagnostic::new("d1", DiagnosticSeverity::Error, "bad indent")
        .code("E0001")
        .labels(&labels)];
    let mut state = DiagnosticState::new();
    state.set_expanded("d1", true);
    DiagnosticView::new(&items, system)
        .recipe(DiagnosticRecipe::Full)
        .source_lines(&lines)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_sample_lines() -> [TerminalLine<'static>; 6] {
    [
        TerminalLine::system("s0", "spawned pid 4242"),
        TerminalLine::stdout("o1", "running 3 tests"),
        TerminalLine::stdout("o2", "test widgets::list ... ok"),
        TerminalLine::stderr("e1", "warning: unused import"),
        TerminalLine::stdout("o3", "test widgets::tree ... ok"),
        TerminalLine::stdout("o4", "done 東京 🧪"),
    ]
}

fn terminal_output_running(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("cargo test -p termrock --lib")
        .cwd("/Users/dev/termrock")
        .status(TerminalRunStatus::Running)
        .duration_ms(3400)
        .pid(4242);
    let lines = terminal_output_sample_lines();
    let mut state = TerminalOutputState::new();
    state.on_append(lines.len() as u16, area.height.saturating_sub(4));
    TerminalOutput::new(&meta, &lines, system)
        .title("build")
        .render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_failed(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("cargo test -p boom")
        .cwd("/proj")
        .status(TerminalRunStatus::Failed)
        .exit_code(101)
        .duration_ms(890);
    let lines = [
        TerminalLine::stdout("o1", "running 1 test"),
        TerminalLine::stderr("e1", "thread 't' panicked at 'assert'"),
        TerminalLine::system("s1", "exit 101"),
    ];
    let mut state = TerminalOutputState::new();
    TerminalOutput::new(&meta, &lines, system)
        .title("failed")
        .render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_compact(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("git status -sb")
        .status(TerminalRunStatus::Succeeded)
        .exit_code(0)
        .duration_ms(42);
    let lines = [
        TerminalLine::stdout("o1", "## main"),
        TerminalLine::stdout("o2", " M src/lib.rs"),
    ];
    let mut state = TerminalOutputState::new();
    state.recipe = TerminalOutputRecipe::Compact;
    TerminalOutput::new(&meta, &lines, system).render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_env(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let env = [
        TerminalEnvEntry::secret("API_TOKEN"),
        TerminalEnvEntry::new("PATH", "/usr/bin:/bin"),
        TerminalEnvEntry::new("RUST_LOG", "info"),
    ];
    let meta = TerminalCommandMeta::new("curl https://api.example")
        .cwd("/tmp")
        .env(&env)
        .status(TerminalRunStatus::Succeeded)
        .exit_code(0)
        .duration_ms(220);
    let lines = [TerminalLine::stdout("o1", "{\"ok\":true}")];
    let mut state = TerminalOutputState::new();
    state.show_env = true;
    TerminalOutput::new(&meta, &lines, system)
        .title("http")
        .render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_pinned(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("tail -f app.log")
        .status(TerminalRunStatus::Running)
        .duration_ms(12_000)
        .pid(99);
    let lines = terminal_output_sample_lines();
    let mut state = TerminalOutputState::new();
    state.on_append(40, 6);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Home,
            termrock::input::KeyModifiers::NONE,
        ),
        &lines,
        &meta,
    );
    state.on_append(80, 6); // unread while pinned
    TerminalOutput::new(&meta, &lines, system)
        .title("stream")
        .render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("sleep 10").status(TerminalRunStatus::Pending);
    let mut state = TerminalOutputState::new();
    TerminalOutput::new(&meta, &[], system).render(area, frame.buffer_mut(), &mut state);
}

fn terminal_output_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let meta = TerminalCommandMeta::new("make")
        .status(TerminalRunStatus::Succeeded)
        .exit_code(0)
        .duration_ms(100);
    let lines = terminal_output_sample_lines();
    let mut state = TerminalOutputState::new();
    state.paint_mode = TerminalPaintMode::Plain;
    TerminalOutput::new(&meta, &lines, system)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_sample() -> Vec<u8> {
    let mut v: Vec<u8> = (0..48u8).collect();
    v.extend_from_slice(b"Hello, xxd!\n");
    v.extend_from_slice("東京🧪".as_bytes());
    v.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0xff]);
    v
}

fn hex_viewer_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let data = hex_viewer_sample();
    let win = HexWindow::new(0, &data, data.len() as u64);
    let mut state = HexViewerState::new();
    state.bytes_per_row = 16;
    state.cursor = 0x10;
    HexViewer::new(win, system)
        .title("blob.bin")
        .render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_selection(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let data = hex_viewer_sample();
    let win = HexWindow::new(0, &data, data.len() as u64);
    let mut state = HexViewerState::new();
    state.bytes_per_row = 16;
    state.cursor = 0x08;
    state.sel_anchor = Some(0x04);
    state.sel_end = Some(0x0b);
    state.bookmarks.insert(0x00);
    HexViewer::new(win, system)
        .title("selection")
        .render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_inspector(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let data = hex_viewer_sample();
    let win = HexWindow::new(0, &data, data.len() as u64);
    let mut state = HexViewerState::new();
    state.bytes_per_row = 16;
    state.cursor = data.len().saturating_sub(6) as u64; // near deadbeef
    state.endian = HexEndian::Little;
    state.show_inspector = true;
    HexViewer::new(win, system)
        .title("inspector LE")
        .render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_search(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let data = hex_viewer_sample();
    let win = HexWindow::new(0, &data, data.len() as u64);
    let mut state = HexViewerState::new();
    state.bytes_per_row = 16;
    state.search = Some("dead".into());
    HexViewer::new(win, system)
        .title("search")
        .render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let win = HexWindow::new(0, &[], 0);
    let mut state = HexViewerState::new();
    HexViewer::new(win, system).render(area, frame.buffer_mut(), &mut state);
}

fn hex_viewer_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let data = hex_viewer_sample();
    let win = HexWindow::new(0, &data, data.len() as u64);
    let mut state = HexViewerState::new();
    state.bytes_per_row = 8;
    state.cursor = 3;
    state.sel_anchor = Some(1);
    state.sel_end = Some(5);
    state.ascii_mode = HexAsciiMode::Ascii;
    HexViewer::new(win, system)
        .ascii(true)
        .colorless(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_sample() -> Vec<FileTreeEntry<'static, &'static str>> {
    vec![
        FileTreeEntry::dir("src", "src", "src", 0).expanded(),
        FileTreeEntry::file("src/main.rs", "main.rs", "src/main.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Modified),
        FileTreeEntry::file("src/lib.rs", "lib.rs", "src/lib.rs", 1)
            .parent("src")
            .file_type("rs")
            .git(FileGitStatus::Added),
        FileTreeEntry::dir("src/widgets", "widgets", "src/widgets", 1)
            .parent("src")
            .lazy_dir(),
        FileTreeEntry::file("README.md", "README.md", "README.md", 0).file_type("md"),
        FileTreeEntry::file(".env", ".env", ".env", 0)
            .hidden(true)
            .git(FileGitStatus::Untracked),
        FileTreeEntry::dir("target", "target", "target", 0)
            .ignored(true)
            .git(FileGitStatus::Ignored),
        FileTreeEntry::file("link", "link", "link", 0)
            .kind(FileTreeKind::SymlinkFile)
            .symlink_target("src/main.rs"),
        FileTreeEntry::file("locked", "locked", "locked", 0).error_msg("permission denied"),
    ]
}

fn file_tree_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = file_tree_sample();
    let mut state = FileTreeState::with_selected(Some("src/main.rs"));
    FileTree::new(&entries, system)
        .title("repo")
        .render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = file_tree_sample();
    let mut state = FileTreeState::new();
    state.filter = Some("main".into());
    FileTree::new(&entries, system)
        .title("filter")
        .render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_hidden(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = file_tree_sample();
    let mut state = FileTreeState::new();
    state.show_hidden = true;
    state.show_ignored = true;
    FileTree::new(&entries, system)
        .title("all")
        .render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_confirm(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = file_tree_sample();
    let mut state = FileTreeState::new();
    state.select(Some("src/main.rs"));
    state.pending_confirm = Some(termrock::widgets::FileTreeDestructiveConfirm {
        subject: "2 items".into(),
        verb: "permanently delete",
        ids: vec!["src/main.rs", "src/lib.rs"],
    });
    FileTree::new(&entries, system)
        .title("delete")
        .render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FileTreeState::<&str>::new();
    FileTree::new(&[], system).render(area, frame.buffer_mut(), &mut state);
}

fn file_tree_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = file_tree_sample();
    let mut state = FileTreeState::with_selected(Some("src"));
    state.ascii = true;
    FileTree::new(&entries, system)
        .title("ascii")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_sample() -> Vec<ProcessRow<'static>> {
    vec![
        ProcessRow::new(ProcessKey::new(1, 100), "systemd")
            .cpu(0.1)
            .mem(4_000_000)
            .user("root")
            .elapsed_ms(86_400_000)
            .branch()
            .expanded(),
        ProcessRow::new(ProcessKey::new(482, 200), "sshd")
            .parent(ProcessKey::new(1, 100))
            .depth(1)
            .cpu(0.0)
            .mem(8_000_000)
            .user("root")
            .elapsed_ms(3_600_000)
            .branch()
            .expanded(),
        ProcessRow::new(ProcessKey::new(1204, 300), "bash")
            .parent(ProcessKey::new(482, 200))
            .depth(2)
            .cpu(1.2)
            .mem(12_000_000)
            .user("alice")
            .elapsed_ms(600_000)
            .status(ProcessStatus::Sleeping),
        ProcessRow::new(ProcessKey::new(1888, 400), "cargo test")
            .parent(ProcessKey::new(1204, 300))
            .depth(3)
            .cpu(42.0)
            .mem(640_000_000)
            .user("alice")
            .elapsed_ms(30_000)
            .branch()
            .expanded(),
        ProcessRow::new(ProcessKey::new(1902, 500), "rustc")
            .parent(ProcessKey::new(1888, 400))
            .depth(4)
            .cpu(88.4)
            .mem(1_100_000_000)
            .user("alice")
            .elapsed_ms(12_000),
        ProcessRow::new(ProcessKey::new(2201, 600), "btop")
            .cpu(3.5)
            .mem(48_000_000)
            .user("alice")
            .elapsed_ms(120_000)
            .status(ProcessStatus::Running),
        ProcessRow::new(ProcessKey::new(3001, 700), "zombie-demo")
            .cpu(0.0)
            .mem(0)
            .user("alice")
            .elapsed_ms(1_000)
            .status(ProcessStatus::Zombie),
    ]
}

fn process_table_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = process_table_sample();
    let mut state = ProcessTableState::with_selected(Some(ProcessKey::new(1902, 500)));
    ProcessTable::new(&rows, system)
        .title("procs")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_tree(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = process_table_sample();
    let mut state = ProcessTableState::with_selected(Some(ProcessKey::new(1888, 400)));
    state.view_mode = ProcessViewMode::Tree;
    ProcessTable::new(&rows, system)
        .title("tree")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = process_table_sample();
    let mut state = ProcessTableState::new();
    state.filter = Some("cargo".into());
    ProcessTable::new(&rows, system)
        .title("filter")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_confirm(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = process_table_sample();
    let mut state = ProcessTableState::with_selected(Some(ProcessKey::new(1902, 500)));
    state.pending_confirm = Some(ProcessSignalConfirm {
        signal: ProcessSignal::Term,
        subject: "rustc (1902)".into(),
        targets: vec![ProcessKey::new(1902, 500)],
    });
    ProcessTable::new(&rows, system)
        .title("signal")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ProcessTableState::new();
    ProcessTable::new(&[], system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn process_table_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows = process_table_sample();
    let mut state = ProcessTableState::with_selected(Some(ProcessKey::new(1888, 400)));
    state.view_mode = ProcessViewMode::Tree;
    state.ascii = true;
    ProcessTable::new(&rows, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::with_text(
        "select u.id, u.name\nfrom users u\nwhere u.active = true\nlimit 20;",
    );
    state.language = QueryLanguage::sql();
    state.set_results(
        QueryResultSummary::new("ok · 20 rows · 12ms")
            .rows(20)
            .columns(2),
    );
    state.set_parameters(vec![QueryParameter::new("limit", "20").type_hint("int")]);
    QueryEditor::new(system)
        .title("sql")
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_running(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::with_text("select count(*) from events;");
    state.set_run(QueryRunStatus::Running {
        run_id: "run-42".into(),
    });
    QueryEditor::new(system)
        .title("running")
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_diagnostics(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    static LABELS: &[SourceLabel<'static>] = &[SourceLabel {
        range: SourceRange {
            start_line: 2,
            start_col: 1,
            end_line: 2,
            end_col: 4,
        },
        label: Some("expected identifier"),
        style: SpanStyle::Primary,
    }];
    let diags = [Diagnostic::new(
        "d1",
        DiagnosticSeverity::Error,
        "syntax error near FROM",
    )
    .code("SQL-001")
    .labels(LABELS)];
    let mut state = QueryEditorState::with_text("select\nfrom t");
    let _ = state.set_focus(QueryFocus::Diagnostics);
    QueryEditor::new(system)
        .title("diag")
        .diagnostics(&diags)
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_parameters(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::with_text("select * from t where id = :id");
    state.set_parameters(vec![
        QueryParameter::new("id", "42").type_hint("uuid").required(),
        QueryParameter::new("token", "s3cr3t").secret(),
    ]);
    let _ = state.set_focus(QueryFocus::Parameters);
    QueryEditor::new(system)
        .title("params")
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_compact(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::with_text("select 1");
    state.mode = QueryEditorMode::Compact;
    QueryEditor::new(system)
        .title("compact")
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::new();
    QueryEditor::new(system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn query_editor_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = QueryEditorState::with_text("select 1");
    state.ascii = true;
    state.set_run(QueryRunStatus::Success {
        rows: Some(1),
        duration_ms: Some(3),
    });
    QueryEditor::new(system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_schema() -> Vec<ResultColumn> {
    vec![
        ResultColumn::new("id", "ID")
            .type_name("int8")
            .not_null()
            .width(DataColumnWidth::Fixed(6))
            .priority(100)
            .pin_start(),
        ResultColumn::new("name", "Name")
            .type_name("text")
            .width(DataColumnWidth::Min(12))
            .priority(90)
            .editable(),
        ResultColumn::new("blob", "Blob")
            .type_name("bytea")
            .binary()
            .priority(40),
        ResultColumn::new("token", "Token")
            .type_name("text")
            .secret()
            .priority(30),
        ResultColumn::new("meta", "Meta")
            .type_name("jsonb")
            .priority(50),
    ]
}

fn result_grid_rows() -> Vec<ResultRow<'static>> {
    static R0: [ResultCell<'static>; 5] = [
        ResultCell::integer("1"),
        ResultCell::text("alpha"),
        ResultCell::binary(128),
        ResultCell::secret_value("s3cr3t"),
        ResultCell::json(r#"{"a":1}"#),
    ];
    static R1: [ResultCell<'static>; 5] = [
        ResultCell::integer("2"),
        ResultCell::text("beta"),
        ResultCell::null(),
        ResultCell::secret_value("x"),
        ResultCell::json("[]"),
    ];
    static R2: [ResultCell<'static>; 5] = [
        ResultCell::integer("3"),
        ResultCell::text("gamma"),
        ResultCell::binary(1_048_576),
        ResultCell::null(),
        ResultCell::json(r#"{"ok":true}"#),
    ];
    vec![
        ResultRow::new(1, 1, &R0),
        ResultRow::new(2, 2, &R1),
        ResultRow::new(3, 3, &R2),
    ]
}

fn result_grid_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let rows = result_grid_rows();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(
        ResultQueryStatus::Ready {
            total: Some(3),
            duration_ms: Some(8),
        },
        rows.len(),
    );
    ResultGrid::new(system, &cols, &rows)
        .title("q1")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_streaming(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let rows = result_grid_rows();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(
        ResultQueryStatus::Streaming {
            resident: 1_250,
            total: None,
        },
        rows.len(),
    );
    ResultGrid::new(system, &cols, &rows)
        .title("stream")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_stats(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let rows = result_grid_rows();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(
        ResultQueryStatus::Ready {
            total: Some(3),
            duration_ms: Some(4),
        },
        rows.len(),
    );
    state.show_stats = true;
    let mut st = ResultColumnStats::new("id");
    st.non_null = 3;
    st.min = Some("1".into());
    st.max = Some("3".into());
    state.stats = vec![st];
    ResultGrid::new(system, &cols, &rows)
        .title("stats")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_wide(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut cols = vec![ResultColumn::new("id", "ID")
        .width(DataColumnWidth::Fixed(5))
        .priority(255)
        .pin_start()];
    for i in 0..12 {
        cols.push(
            ResultColumn::new(format!("c{i}"), format!("C{i}"))
                .priority(80u8.saturating_sub(i as u8 * 5)),
        );
    }
    static CELLS: [ResultCell<'static>; 13] = [
        ResultCell::integer("1"),
        ResultCell::text("a"),
        ResultCell::text("b"),
        ResultCell::text("c"),
        ResultCell::text("d"),
        ResultCell::text("e"),
        ResultCell::text("f"),
        ResultCell::text("g"),
        ResultCell::text("h"),
        ResultCell::text("i"),
        ResultCell::text("j"),
        ResultCell::text("k"),
        ResultCell::text("l"),
    ];
    let rows = vec![ResultRow::new(1, 1, &CELLS)];
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(
        ResultQueryStatus::Ready {
            total: Some(1),
            duration_ms: Some(1),
        },
        1,
    );
    ResultGrid::new(system, &cols, &rows)
        .title("wide")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(ResultQueryStatus::Idle, 0);
    ResultGrid::new(system, &cols, &[])
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.set_status(
        ResultQueryStatus::Failed {
            message: "relation \"users\" does not exist".into(),
        },
        0,
    );
    ResultGrid::new(system, &cols, &[])
        .title("err")
        .render(area, frame.buffer_mut(), &mut state);
}

fn result_grid_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cols = result_grid_schema();
    let rows = result_grid_rows();
    let mut state = ResultGridState::with_schema(cols.clone());
    state.ascii = true;
    state.redaction = ResultRedaction::Safe;
    state.set_status(
        ResultQueryStatus::Ready {
            total: Some(3),
            duration_ms: Some(2),
        },
        rows.len(),
    );
    ResultGrid::new(system, &cols, &rows)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_sample() -> Vec<SchemaBrowserEntry<'static, &'static str>> {
    vec![
        SchemaBrowserEntry::connection("conn", "prod", "prod")
            .expanded()
            .conn_status(SchemaConnStatus::Connected),
        SchemaBrowserEntry::database("db", "app", "prod/app", 1)
            .parent("conn")
            .expanded(),
        SchemaBrowserEntry::schema("sch", "public", "prod/app/public", 2)
            .parent("db")
            .expanded(),
        SchemaBrowserEntry::table("users", "users", "prod/app/public/users", 3)
            .parent("sch")
            .expanded(),
        SchemaBrowserEntry::column("users.id", "id", "prod/app/public/users.id", 4)
            .parent("users")
            .type_label("int8")
            .nullable(false)
            .key_badge("PK"),
        SchemaBrowserEntry::column("users.email", "email", "prod/app/public/users.email", 4)
            .parent("users")
            .type_label("text")
            .nullable(false),
        SchemaBrowserEntry::table("orders", "orders", "prod/app/public/orders", 3)
            .parent("sch")
            .lazy(),
        SchemaBrowserEntry::view("v_active", "v_active", "prod/app/public/v_active", 3)
            .parent("sch"),
        SchemaBrowserEntry::new(
            "idx_email",
            "idx_email",
            "prod/app/public/users/idx_email",
            SchemaNodeKind::Index,
            4,
        )
        .parent("users")
        .type_label("btree"),
        SchemaBrowserEntry::connection("offline", "staging", "staging")
            .conn_status(SchemaConnStatus::Offline)
            .lazy(),
    ]
}

fn schema_browser_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = schema_browser_sample();
    let mut state = SchemaBrowserState::with_selected(Some("users"));
    SchemaBrowser::new(&entries, system)
        .title("catalog")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_lazy(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = schema_browser_sample();
    let mut state = SchemaBrowserState::with_selected(Some("orders"));
    SchemaBrowser::new(&entries, system)
        .title("lazy")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = schema_browser_sample();
    let mut state = SchemaBrowserState::new();
    state.filter = Some("email".into());
    SchemaBrowser::new(&entries, system)
        .title("filter")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut entries = schema_browser_sample();
    entries.push(
        SchemaBrowserEntry::database("bad", "broken", "prod/broken", 1)
            .parent("conn")
            .error_msg("permission denied"),
    );
    let mut state = SchemaBrowserState::with_selected(Some("bad"));
    SchemaBrowser::new(&entries, system)
        .title("error")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_drawer(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = schema_browser_sample();
    let mut state = SchemaBrowserState::with_selected(Some("sch"));
    state.presentation = SchemaBrowserPresentation::Drawer;
    state.presentation_override = Some(SchemaBrowserPresentation::Drawer);
    SchemaBrowser::new(&entries, system)
        .title("drawer")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SchemaBrowserState::<&str>::new();
    SchemaBrowser::new(&[], system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn schema_browser_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = schema_browser_sample();
    let mut state = SchemaBrowserState::with_selected(Some("users"));
    state.ascii = true;
    SchemaBrowser::new(&entries, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_sample() -> (Vec<SearchResultGroup>, Vec<SearchResultItem<'static>>) {
    static T0: &[MatchRange] = &[MatchRange::new(0, 4)];
    static S0: &[MatchRange] = &[MatchRange::new(10, 14)];
    static T1: &[MatchRange] = &[MatchRange::new(0, 6)];
    let groups = vec![
        SearchResultGroup::new("src", "src/", 2),
        SearchResultGroup::new("docs", "docs/", 1),
    ];
    let items = vec![
        SearchResultItem::new("f1", "main.rs")
            .group("src")
            .source("src/main.rs")
            .snippet("fn main() { search(); }")
            .title_matches(T0)
            .snippet_matches(S0)
            .line(12)
            .kind(SearchResultKind::File),
        SearchResultItem::new("f2", "search.rs")
            .group("src")
            .source("src/search.rs")
            .snippet("pub fn search() {}")
            .title_matches(T1)
            .line(1)
            .kind(SearchResultKind::File),
        SearchResultItem::new("d1", "SearchResults")
            .group("docs")
            .source("docs/handbook/search-results.mdx")
            .snippet("grouped navigable search results")
            .kind(SearchResultKind::Doc),
        SearchResultItem::new("c1", "termrock search")
            .snippet("run workspace search")
            .kind(SearchResultKind::Command),
    ];
    (groups, items)
}

fn search_results_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (groups, items) = search_results_sample();
    let mut state = SearchResultsState::new();
    state.apply_results(1, SearchResultsStatus::Ready { total: Some(4) }, 4);
    state.cursor = 1;
    SearchResults::new(&groups, &items, system)
        .title("find")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchResultsState::new();
    let _ = state.begin_search();
    SearchResults::new(&[], &[], system)
        .title("loading")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchResultsState::new();
    state.apply_results(
        0,
        SearchResultsStatus::Empty {
            message: Some("no matches for 'zzzz'".into()),
        },
        0,
    );
    SearchResults::new(&[], &[], system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_stale(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchResultsState::new();
    let g1 = state.begin_search();
    let _ = state.begin_search();
    state.apply_results(g1, SearchResultsStatus::Ready { total: Some(1) }, 1);
    SearchResults::new(&[], &[], system)
        .title("stale")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_collapsed(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (groups, items) = search_results_sample();
    let mut state = SearchResultsState::new();
    state.apply_results(1, SearchResultsStatus::Ready { total: Some(4) }, 4);
    state.collapsed.insert("src".into());
    SearchResults::new(&groups, &items, system)
        .title("collapsed")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_streaming(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (groups, items) = search_results_sample();
    let mut state = SearchResultsState::new();
    state.apply_results(
        2,
        SearchResultsStatus::Partial {
            resident: 128,
            total: None,
        },
        items.len(),
    );
    SearchResults::new(&groups, &items, system)
        .title("stream")
        .render(area, frame.buffer_mut(), &mut state);
}

fn search_results_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (groups, items) = search_results_sample();
    let mut state = SearchResultsState::new();
    state.ascii = true;
    state.apply_results(1, SearchResultsStatus::Ready { total: Some(4) }, 4);
    SearchResults::new(&groups, &items, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn metrics_dashboard_tiles() -> (Vec<MetricTile<'static>>, Vec<MetricAlert<'static>>) {
    static S: &[f64] = &[1.0, 2.0, 3.0, 2.5, 4.0, 3.5, 5.0, 4.2, 6.0, 5.1];
    static THR: &[f64] = &[70.0, 90.0];
    let tiles = vec![
        MetricTile::new("cpu", "CPU", "42%")
            .unit("util")
            .delta("+2.1%", true)
            .samples(S)
            .thresholds(THR)
            .health(MetricTileHealth::Ok),
        MetricTile::new("mem", "Memory", "71%")
            .gauge(71.0)
            .thresholds(THR)
            .delta("+1%", true)
            .health(MetricTileHealth::Warning),
        MetricTile::new("rps", "RPS", "1.2k")
            .samples(S)
            .delta("−3%", false)
            .health(MetricTileHealth::Ok),
        MetricTile::new("lat", "p99", "48ms")
            .samples(S)
            .health(MetricTileHealth::Ok)
            .viz(MetricViz::Sparkline),
    ];
    let alerts = vec![
        MetricAlert::new("a1", MetricAlertSeverity::Warning, "mem > 70%").metric("mem"),
        MetricAlert::new("a2", MetricAlertSeverity::Critical, "error budget burning")
            .metric("lat"),
    ];
    (tiles, alerts)
}

fn metrics_dashboard_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (tiles, alerts) = metrics_dashboard_tiles();
    let mut state = MetricsDashboardState::new();
    state.focus_tile = 1;
    MetricsDashboard::new(&tiles, &alerts, system)
        .title("ops")
        .render(area, frame.buffer_mut(), &mut state);
}

fn metrics_dashboard_partial(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    static S: &[f64] = &[1.0, 2.0, 3.0, 4.0];
    let tiles = vec![
        MetricTile::new("cpu", "CPU", "20%").samples(S),
        MetricTile::new("disk", "Disk", "—").failed("timeout"),
        MetricTile::new("net", "Net", "12MB/s").samples(S),
    ];
    let mut state = MetricsDashboardState::new();
    MetricsDashboard::new(&tiles, &[], system)
        .title("partial")
        .render(area, frame.buffer_mut(), &mut state);
}

fn metrics_dashboard_paused(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (tiles, alerts) = metrics_dashboard_tiles();
    let mut state = MetricsDashboardState::new();
    state.paused = true;
    MetricsDashboard::new(&tiles, &alerts, system)
        .title("paused")
        .render(area, frame.buffer_mut(), &mut state);
}

fn metrics_dashboard_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = MetricsDashboardState::new();
    MetricsDashboard::new(&[], &[], system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn metrics_dashboard_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (tiles, alerts) = metrics_dashboard_tiles();
    let mut state = MetricsDashboardState::new();
    state.ascii = true;
    MetricsDashboard::new(&tiles, &alerts, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_sample() -> Vec<TraceSpan<'static>> {
    vec![
        TraceSpan::new("root", "HTTP GET /api", 0, 420)
            .service("gateway")
            .branch()
            .expanded()
            .critical()
            .kind("http"),
        TraceSpan::new("auth", "authenticate", 5, 40)
            .parent("root")
            .service("auth")
            .depth(1)
            .kind("internal"),
        TraceSpan::new("db", "SELECT users", 50, 180)
            .parent("root")
            .service("postgres")
            .depth(1)
            .branch()
            .expanded()
            .critical()
            .kind("db"),
        TraceSpan::new("db.row", "row map", 60, 20)
            .parent("db")
            .service("postgres")
            .depth(2),
        TraceSpan::new("tool", "tool:fetch", 240, 150)
            .parent("root")
            .service("agent")
            .depth(1)
            .error("timeout")
            .kind("tool"),
        TraceSpan::new("render", "serialize", 390, 30)
            .parent("root")
            .service("gateway")
            .depth(1)
            .critical(),
    ]
}

fn trace_waterfall_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let spans = trace_waterfall_sample();
    let mut state = TraceWaterfallState::with_selected("db");
    TraceWaterfall::new(&spans, system)
        .title("req")
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let spans = trace_waterfall_sample();
    let mut state = TraceWaterfallState::with_selected("tool");
    TraceWaterfall::new(&spans, system)
        .title("error")
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_critical(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let spans = trace_waterfall_sample();
    let mut state = TraceWaterfallState::new();
    state.critical_only = true;
    TraceWaterfall::new(&spans, system)
        .title("crit")
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_zoomed(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let spans = trace_waterfall_sample();
    let mut state = TraceWaterfallState::with_selected("db");
    state.sync_total(&spans);
    state.time_start_ms = 40;
    state.time_duration_ms = 200;
    TraceWaterfall::new(&spans, system)
        .title("zoom")
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TraceWaterfallState::new();
    state.empty_message = Some("no spans".into());
    TraceWaterfall::new(&[], system)
        .title("empty")
        .render(area, frame.buffer_mut(), &mut state);
}

fn trace_waterfall_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let spans = trace_waterfall_sample();
    let mut state = TraceWaterfallState::with_selected("root");
    state.ascii = true;
    TraceWaterfall::new(&spans, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn dependency_graph_sample() -> (Vec<DepNode<'static>>, Vec<DepEdge<'static>>) {
    let nodes = vec![
        DepNode::new("app", "app").kind(DepNodeKind::Package).detail("0.1.0"),
        DepNode::new("termrock", "termrock")
            .kind(DepNodeKind::Package)
            .detail("0.11"),
        DepNode::new("ratatui", "ratatui").kind(DepNodeKind::Package),
        DepNode::new("serde", "serde").kind(DepNodeKind::Package),
        DepNode::new("api", "api-svc")
            .kind(DepNodeKind::Service)
            .group("runtime"),
        DepNode::new("db", "postgres")
            .kind(DepNodeKind::Service)
            .status(DepNodeStatus::Warning),
        DepNode::new("missing", "lost-crate")
            .kind(DepNodeKind::Package)
            .status(DepNodeStatus::Missing),
    ];
    let edges = vec![
        DepEdge::new("app", "termrock"),
        DepEdge::new("app", "serde"),
        DepEdge::new("termrock", "ratatui"),
        DepEdge::new("api", "db").kind(DepEdgeKind::Calls),
        DepEdge::new("app", "api"),
        DepEdge::new("app", "missing"),
    ];
    (nodes, edges)
}

fn dependency_graph_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (nodes, edges) = dependency_graph_sample();
    let mut state = DependencyGraphState::with_selected("termrock");
    DependencyGraph::new(&nodes, &edges, system)
        .title("crates")
        .render(area, frame.buffer_mut(), &mut state);
}

fn dependency_graph_tree(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (nodes, edges) = dependency_graph_sample();
    let mut state = DependencyGraphState::with_selected("app");
    state.force_tree = true;
    DependencyGraph::new(&nodes, &edges, system)
        .title("tree")
        .render(area, frame.buffer_mut(), &mut state);
}

fn dependency_graph_list(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (nodes, edges) = dependency_graph_sample();
    let mut state = DependencyGraphState::new();
    state.preferred_view = DependencyGraphView::List;
    DependencyGraph::new(&nodes, &edges, system)
        .title("list")
        .render(area, frame.buffer_mut(), &mut state);
}

fn dependency_graph_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (nodes, edges) = dependency_graph_sample();
    let mut state = DependencyGraphState::new();
    state.filter = Some("serde".into());
    DependencyGraph::new(&nodes, &edges, system)
        .title("filter")
        .render(area, frame.buffer_mut(), &mut state);
}

fn dependency_graph_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (nodes, edges) = dependency_graph_sample();
    let mut state = DependencyGraphState::with_selected("app");
    state.ascii = true;
    DependencyGraph::new(&nodes, &edges, system)
        .title("ascii")
        .ascii(true)
        .render(area, frame.buffer_mut(), &mut state);
}

#[derive(Clone, Copy)]
enum TableVariant {
    Basic,
    Sorted,
    Narrow,
    Unicode,
    Disabled,
    Empty,
    Bordered,
    Striped,
    Compact,
    Loading,
    Error,
    Priority,
}

fn completion_menu_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(Panel::new(&panel_tokens).title("Editor"), area);
    let candidates = [
        CompletionCandidate::new("select", "SELECT")
            .kind("keyword")
            .kind_glyph("⌘")
            .group("Keywords")
            .documentation("Select rows from a relation."),
        CompletionCandidate::new("from", "FROM")
            .kind("keyword")
            .group("Keywords"),
        CompletionCandidate::new("users", "users")
            .kind("table")
            .detail("public")
            .group("Tables"),
        CompletionCandidate::new("orders", "orders")
            .kind("table")
            .group("Tables"),
        CompletionCandidate::new("where", "WHERE")
            .kind("keyword")
            .group("Keywords"),
    ];
    let anchor = Rect::new(area.x.saturating_add(4), area.y.saturating_add(2), 1, 1);
    let mut state = CompletionMenuState::new(Some("select"));
    state.set_show_docs(true);
    frame.render_stateful_widget(
        &CompletionMenu::new(&candidates, system, area, anchor).preferred_size(
            CompletionMenuSize {
                width: 48,
                height: 8,
            },
        ),
        area,
        &mut state,
    );
}

fn completion_menu_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(Panel::new(system).title("Editor"), area);
    let candidates: [CompletionCandidate<'_, &str>; 0] = [];
    let anchor = Rect::new(area.x.saturating_add(2), area.y.saturating_add(1), 1, 1);
    let mut state = CompletionMenuState::new(None);
    let _ = state.begin_async();
    frame.render_stateful_widget(
        &CompletionMenu::new(&candidates, system, area, anchor).preferred_size(
            CompletionMenuSize {
                width: 28,
                height: 4,
            },
        ),
        area,
        &mut state,
    );
}

fn completion_menu_docs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(Panel::new(system).title("LSP"), area);
    let candidates = [
        CompletionCandidate::new("map", "map")
            .kind("fn")
            .kind_glyph("ƒ")
            .detail("Iterator")
            .documentation("Transforms each element with a closure.\n\nReturns a new iterator."),
        CompletionCandidate::new("filter", "filter")
            .kind("fn")
            .detail("Iterator")
            .documentation("Retains elements that match a predicate."),
    ];
    let anchor = Rect::new(area.x.saturating_add(3), area.y.saturating_add(2), 1, 1);
    let mut state = CompletionMenuState::new(Some("map"));
    state.set_show_docs(true);
    frame.render_stateful_widget(
        &CompletionMenu::new(&candidates, system, area, anchor).preferred_size(
            CompletionMenuSize {
                width: 52,
                height: 8,
            },
        ),
        area,
        &mut state,
    );
}

fn completion_menu_edge(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(Panel::new(&panel_tokens).title("Edge"), area);
    let candidates = [
        CompletionCandidate::new("alpha", "αlpha-wide-label"),
        CompletionCandidate::new("beta", "βeta"),
        CompletionCandidate::new("gamma", "γamma").kind("fn"),
    ];
    // Anchor near bottom-right so the menu must flip and clamp.
    let anchor = Rect::new(
        area.x.saturating_add(area.width.saturating_sub(2)),
        area.y.saturating_add(area.height.saturating_sub(2)),
        1,
        1,
    );
    let mut state = CompletionMenuState::new(Some("beta"));
    frame.render_stateful_widget(
        &CompletionMenu::new(&candidates, system, area, anchor).preferred_size(
            CompletionMenuSize {
                width: 24,
                height: 5,
            },
        ),
        area,
        &mut state,
    );
}

fn virtual_grid_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_virtual_grid(frame, area, system, 20);
}

fn virtual_grid_million(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_virtual_grid(frame, area, system, 1_000_000);
}

fn render_virtual_grid(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem, total_rows: u64) {
    let columns = [
        GridColumn::fixed("id", "id", 8),
        GridColumn::fixed("name", "name", 16),
        GridColumn::min("value", "value", 10),
        GridColumn::fixed("flag", "flag", 6),
    ];
    let cells0 = [
        GridCell::text("0"),
        GridCell::text("alpha"),
        GridCell::text("1"),
        GridCell::text("y"),
    ];
    let cells1 = [
        GridCell::text("1"),
        GridCell::text("beta"),
        GridCell::pending(),
        GridCell::text("n"),
    ];
    let cells2 = [
        GridCell::text("2"),
        GridCell::text("gamma"),
        GridCell::text("3"),
        GridCell::text("y"),
    ];
    let cells3 = [
        GridCell::text("3"),
        GridCell::text("delta"),
        GridCell::text("4"),
        GridCell::text("y"),
    ];
    let cells4 = [
        GridCell::text("4"),
        GridCell::text("eps"),
        GridCell::pending(),
        GridCell::text("n"),
    ];
    let cells5 = [
        GridCell::text("5"),
        GridCell::text("zeta"),
        GridCell::text("6"),
        GridCell::text("y"),
    ];
    let rows = [
        GridRow::new(0u64, 0, &cells0),
        GridRow::new(1, 1, &cells1),
        GridRow::new(2, 2, &cells2),
        GridRow::new(3, 3, &cells3),
        GridRow::new(4, 4, &cells4),
        GridRow::new(5, 5, &cells5),
    ];
    let grid = VirtualGrid::new(&columns, &rows, system).total_rows(total_rows);
    let mut state = VirtualGridState::new();
    frame.render_stateful_widget(&grid, area, &mut state);
}

fn table_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Basic);
}
fn table_sorted(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Sorted);
}
fn table_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Narrow);
}
fn table_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Unicode);
}
fn table_disabled(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Disabled);
}
fn table_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Empty);
}
fn table_bordered(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Bordered);
}
fn table_striped(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Striped);
}
fn table_compact(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Compact);
}
fn table_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Loading);
}
fn table_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Error);
}
fn table_priority(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_table(frame, area, system, TableVariant::Priority);
}

fn render_table(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem, variant: TableVariant) {
    use termrock::widgets::{TableBodyState, TableRecipe};
    let tokens = system.clone().density(termrock::style::Density::default());
    let sorted = matches!(variant, TableVariant::Sorted);
    let columns = [
        Column::new("pid", "PID", ColumnWidth::Fixed(7))
            .alignment(CellAlignment::Right)
            .sortable(None)
            .priority(100),
        Column::new(
            "process",
            "Process",
            ColumnWidth::Fill(NonZeroU16::new(2).unwrap()),
        )
        .sortable(None)
        .priority(90),
        Column::new("region", "Region", ColumnWidth::Min(10))
            .alignment(CellAlignment::Center)
            .priority(if matches!(variant, TableVariant::Priority) {
                10
            } else {
                40
            }),
        Column::new("cpu", "CPU", ColumnWidth::Fixed(8))
            .alignment(CellAlignment::Right)
            .sortable(sorted.then_some(SortDirection::Descending))
            .priority(80),
        Column::new(
            "state",
            "State",
            ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
        )
        .alignment(CellAlignment::Center)
        .priority(if matches!(variant, TableVariant::Priority) {
            5
        } else {
            30
        }),
    ];
    let cells = [
        [
            Line::from("101"),
            Line::from(Span::styled("termrock", system.style(Role::Accent))),
            Line::from("東京🧪alpha"),
            Line::from("82.4%"),
            Line::from("run"),
        ],
        [
            Line::from("208"),
            Line::from("cargo-nextest"),
            Line::from("eu-west"),
            Line::from("31.0%"),
            Line::from("run"),
        ],
        [
            Line::from("317"),
            Line::from("rust-analyzer"),
            Line::from("local"),
            Line::from("17.8%"),
            Line::from("idle"),
        ],
        [
            Line::from("422"),
            Line::from("bun-docs"),
            Line::from("us-east"),
            Line::from("9.2%"),
            Line::from("wait"),
        ],
        [
            Line::from("509"),
            Line::from("shell"),
            Line::from("東京"),
            Line::from("4.4%"),
            Line::from("done"),
        ],
        [
            Line::from("612"),
            Line::from("indexer"),
            Line::from("ap-south"),
            Line::from("2.7%"),
            Line::from("idle"),
        ],
        [
            Line::from("734"),
            Line::from("preview-worker"),
            Line::from("eu-north"),
            Line::from("1.8%"),
            Line::from("run"),
        ],
    ];
    let rows = cells
        .iter()
        .enumerate()
        .map(|(index, cells)| {
            TableRow::new(index, cells)
                .enabled(!(matches!(variant, TableVariant::Disabled) && index == 2))
                .emphasis(index == 0 && matches!(variant, TableVariant::Unicode))
        })
        .collect::<Vec<_>>();
    let empty_body = matches!(
        variant,
        TableVariant::Empty | TableVariant::Loading | TableVariant::Error
    );
    let visible = if empty_body { &rows[..0] } else { &rows };
    let mut state = TableState::new((!visible.is_empty()).then_some(
        if matches!(variant, TableVariant::Disabled) {
            1
        } else {
            3
        },
    ));
    let recipe = match variant {
        TableVariant::Bordered => TableRecipe::Bordered,
        TableVariant::Striped => TableRecipe::Striped,
        TableVariant::Compact => TableRecipe::Compact,
        _ => TableRecipe::Quiet,
    };
    let body_state = match variant {
        TableVariant::Loading => TableBodyState::Loading,
        TableVariant::Error => TableBodyState::Error,
        _ => TableBodyState::Ready,
    };
    let mut table = Table::new(&columns, visible, &tokens)
        .recipe(recipe)
        .body_state(body_state)
        .empty_message(Line::from("No processes"));
    if matches!(variant, TableVariant::Loading) {
        table = table.loading_message(Line::from("Loading processes…"));
    }
    if matches!(variant, TableVariant::Error) {
        table = table.error_message(Line::from("Failed to load processes"));
    }
    frame.render_stateful_widget(&table, area, &mut state);
}

fn text_area_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_text_area(
        frame,
        area,
        system,
        "Compose",
        "First line\nSecond line\nThird line",
        None,
    );
}
fn text_area_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_text_area(
        frame,
        area,
        system,
        "Narrow",
        "prefix 東京🧪 trailing content",
        None,
    );
}
fn text_area_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_text_area(
        frame,
        area,
        system,
        "Unicode",
        "e\u{301} cafe\n東京 region\n👩\u{200d}💻 builds",
        None,
    );
}
fn text_area_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    render_text_area(frame, area, system, "Notes", "", Some("Write a note…"));
}
fn text_area_scrolled(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let text = "zero\none\ntwo\nthree\nfour\nfive: deliberately wide content beyond the viewport";
    let mut state = TextAreaState::new(text);
    state.set_cursor(TextCursor {
        line: 5,
        byte: text.lines().last().unwrap().len(),
    });
    frame.render_stateful_widget(&TextArea::new(system).title("Scrolled"), area, &mut state);
}
fn text_area_line_numbers(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextAreaState::new("fn main() {\n    println!(\"hi\");\n}");
    state.set_accepts_input(true);
    frame.render_stateful_widget(
        &TextArea::new(system)
            .title("Source")
            .line_numbers(true),
        area,
        &mut state,
    );
}
fn text_area_soft_wrap(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextAreaState::new(
        "A single very long paragraph that must soft-wrap across several visual rows without a horizontal scrollbar when wrap is Soft.",
    );
    state.set_accepts_input(true);
    state.set_wrap(TextWrap::Soft);
    frame.render_stateful_widget(
        &TextArea::new(system).title("Wrap").soft_wrap(),
        area,
        &mut state,
    );
}
fn text_area_review(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextAreaState::new("Looks good overall.\nNit: rename this helper.");
    state.set_accepts_input(true);
    frame.render_stateful_widget(
        &TextArea::new(system)
            .title("Comment")
            .placeholder("Leave a review…")
            .review(),
        area,
        &mut state,
    );
}
fn render_text_area(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    title: &str,
    text: &str,
    placeholder: Option<&str>,
) {
    let mut state = TextAreaState::new(text);
    let mut widget = TextArea::new(system).title(title);
    if let Some(placeholder) = placeholder {
        widget = widget.placeholder(placeholder);
    }
    frame.render_stateful_widget(&widget, area, &mut state);
}

fn status_bar(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StatusRegion, StatusSlot};
    let left = [StatusSlot::mode("mode", "NOR")
        .style(Style::new().reversed())
        .hover_style(Style::new().bold().reversed())];
    let center = [StatusSlot::focus_zone("focus", "main")];
    let right = [
        StatusSlot::selection("sel", "3/12")
            .style(Style::new().dim())
            .hover_style(Style::new().bold()),
        StatusSlot::shortcut("hint", "? help").region(StatusRegion::Right),
    ];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::with_center(&left, &center, &right, system)
            .rich()
            .alpha(1.0),
        area,
        &mut state,
    );
}

fn status_bar_minimal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::StatusSlot;
    let left = [StatusSlot::mode("mode", "INS")];
    let right = [
        StatusSlot::shortcut("h", "lots of help text"),
        StatusSlot::connection("c", "live"),
    ];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::new(&left, &right, system).minimal(),
        area,
        &mut state,
    );
}

fn status_bar_transient_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StatusSlot, TransientStatus};
    let left = [StatusSlot::mode("mode", "NOR")];
    let right = [StatusSlot::connection("c", "ok")];
    let msg = TransientStatus::new("file saved");
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::new(&left, &right, system).transient(&msg),
        area,
        &mut state,
    );
}

fn status_bar_rich_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StatusRegion, StatusSlot};
    let left = [StatusSlot::mode("m", "NOR"), StatusSlot::context("p", "crates/termrock")];
    let center = [StatusSlot::focus_zone("f", "transcript")];
    let right = [
        StatusSlot::selection("s", "2 sel"),
        StatusSlot::connection("c", "ssh"),
        StatusSlot::shortcut("h", "C-p palette").region(StatusRegion::Right),
    ];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::with_center(&left, &center, &right, system).rich(),
        area,
        &mut state,
    );
}

fn status_indicator_catalog_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut y = area.y;
    for (kind, label) in example_status_catalog() {
        if y >= area.bottom() {
            break;
        }
        StatusIndicator::new(kind, system)
            .label(label)
            .paint(Rect::new(area.x, y, area.width, 1), frame.buffer_mut());
        y = y.saturating_add(1);
    }
}

fn status_indicator_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut x = area.x;
    for kind in SemanticStatus::ALL {
        if x >= area.right() {
            break;
        }
        StatusIndicator::compact(kind, system).paint(
            Rect::new(x, area.y, 2, 1),
            frame.buffer_mut(),
        );
        x = x.saturating_add(2);
    }
}

fn status_indicator_elapsed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    StatusIndicator::new(SemanticStatus::Running, system)
        .label("agent")
        .elapsed_secs(125)
        .paint(area, frame.buffer_mut());
}

fn status_indicator_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut y = area.y;
    for kind in [
        SemanticStatus::Online,
        SemanticStatus::Running,
        SemanticStatus::Failed,
        SemanticStatus::Unknown,
    ] {
        if y >= area.bottom() {
            break;
        }
        StatusIndicator::new(kind, system)
            .ascii(true)
            .label(kind.default_label())
            .paint(Rect::new(area.x, y, area.width, 1), frame.buffer_mut());
        y = y.saturating_add(1);
    }
}

fn virtual_list_million_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Panel, PanelChrome, VIRTUAL_LIST_BENCH_ROWS};
    let mut state = VirtualListState::<u64>::million_fixed();
    state.set_sticky(StickyRegion {
        leading: 1,
        trailing: 0,
    });
    state.set_offset(250_000);
    state.set_viewport_extent(area.height.saturating_sub(2).max(8));
    let mut idx = Vec::new();
    state.projection_indices(&mut idx);
    let projected: Vec<_> = idx
        .iter()
        .map(|&i| {
            let label: &'static str = if i == 0 {
                "★ sticky header"
            } else {
                // Stable short labels without leak: format into owned via Box::leak for story
                Box::leak(format!("row {i:>9} · O(viewport)").into_boxed_str())
            };
            let row = if i == 0 {
                ListRow::group_header(i, Line::from(label))
            } else {
                ListRow::item(i, Line::from(label))
            };
            VirtualListItem::new(i, row)
        })
        .collect();
    let title = format!("VirtualList · {VIRTUAL_LIST_BENCH_ROWS} logical");
    let _ = Panel::new(system)
        .title(title.as_str())
        .chrome(PanelChrome::Focused)
        .paint(area, frame.buffer_mut(), None);
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    VirtualList::new(&projected, system)
        .show_diagnostics(true)
        .paint(inner, frame.buffer_mut(), &mut state);
}

fn virtual_list_follow_tail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = VirtualListState::<u64>::new();
    state.set_logical_len(5000);
    state.set_viewport_extent(area.height.max(4));
    state.set_follow(VirtualListFollow::Tail);
    let mut idx = Vec::new();
    state.projection_indices(&mut idx);
    let projected: Vec<_> = idx
        .iter()
        .map(|&i| {
            let label: &'static str =
                Box::leak(format!("stream event {i:>5}").into_boxed_str());
            VirtualListItem::new(i, ListRow::item(i, Line::from(label)))
        })
        .collect();
    VirtualList::new(&projected, system)
        .show_diagnostics(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn virtual_list_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = VirtualListState::<u64>::new();
    state.set_logical_len(10_000);
    state.set_viewport_extent(area.height.max(6));
    state.set_offset(100);
    state.set_page_status(VirtualPageStatus::Loading);
    let mut idx = Vec::new();
    state.projection_indices(&mut idx);
    let projected: Vec<_> = idx
        .iter()
        .map(|&i| VirtualListItem::placeholder(i, i))
        .collect();
    VirtualList::new(&projected, system).paint(area, frame.buffer_mut(), &mut state);
}

fn virtualizer_million_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        Panel, PanelChrome, StickyRegion, Virtualizer, data_view_bench,
    };

    let mut v = Virtualizer::fixed(1)
        .with_len(data_view_bench::ROWS_1M)
        .with_viewport(area.height.saturating_sub(2).max(1))
        .with_overscan(3)
        .with_sticky(StickyRegion {
            leading: 1,
            trailing: 0,
        });
    v.set_offset(250_000);
    let slice = v.visible_slice();
    let semantic = v.semantic_count();

    frame.render_widget(
        Panel::new(system)
            .title("Virtualizer · 1M logical · O(viewport)")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let mut y = inner.y;
    let meta = format!(
        "len={} off={} vis={}..{} sem={} (<<1M)",
        v.logical_len(),
        v.offset(),
        slice.start,
        slice.end,
        semantic
    );
    frame.buffer_mut().set_stringn(
        inner.x,
        y,
        &meta,
        usize::from(inner.width),
        system.style(termrock::style::Role::TextMuted),
    );
    y = y.saturating_add(1);
    // Sticky row 0 always in set.
    frame.buffer_mut().set_stringn(
        inner.x,
        y,
        "★ sticky header (semantic always)",
        usize::from(inner.width),
        system.style(termrock::style::Role::TextStrong),
    );
    y = y.saturating_add(1);
    for row in slice.start..slice.end {
        if y >= inner.bottom() {
            break;
        }
        let line = format!("row {row:>9} · projected only");
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            &line,
            usize::from(inner.width),
            system.style(termrock::style::Role::Text),
        );
        y = y.saturating_add(1);
    }
}

fn scroll_area_follow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        Panel, PanelChrome, ScrollArea, ScrollAreaState, ScrollBarVisibility,
    };

    let mut state = ScrollAreaState::new().axes(true, true);
    state.set_content_size(80, 200);
    state.set_viewport(area.width.saturating_sub(4), area.height.saturating_sub(4));
    state.follow_tail();
    // User scrolls away then content grows → new-content badge.
    let _ = state.scroll_by(-40, 0);
    state.set_content_size(80, 260);

    frame.render_widget(
        Panel::new(system)
            .title("ScrollArea · paused · new content")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let sa = ScrollArea::new(system).bar(ScrollBarVisibility::Always);
    let body = sa.body_area(inner, &state);
    // Synthetic lines for visible range.
    let range = state.visible_range_y();
    for (i, row) in (range.start..range.end).enumerate() {
        let y = body.y.saturating_add(i as u16);
        if y >= body.bottom() {
            break;
        }
        let line = format!("L{row:04} stream body · unicode 日本語 🧪");
        frame.buffer_mut().set_stringn(
            body.x,
            y,
            &line,
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
    sa.render_bars(inner, frame.buffer_mut(), &state);
    sa.render_new_content(inner, frame.buffer_mut(), &state);
}

fn selection_model_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SelectionModel, SelectionVisual};
    use termrock::style::SelectionChrome;
    use termrock::widgets::{List, ListRow, ListState, Panel, PanelChrome, RowRole};

    let rows = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "c",
            label: Line::from("Gamma"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let mut state = ListState::new(Some("a"));
    state.enable_multi_select();
    if let Some(sel) = state.selection_mut() {
        let _ = sel.toggle(&"a");
        let _ = sel.toggle(&"c");
    }
    let visual = SelectionVisual::from_chrome(SelectionChrome::Gutter);
    frame.render_widget(
        Panel::new(system)
            .title(if visual.requires_glyph() {
                "SelectionModel (gutter+check)"
            } else {
                "SelectionModel"
            })
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_stateful_widget(
        List::new(&rows, system).focused(true),
        inner,
        &mut state,
    );
    let _ = SelectionModel::<&str>::multiple();
}

fn collection_state_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{NavigationMove, UiIntent};
    use termrock::widgets::{List, ListRow, ListState, Panel, PanelChrome, RowRole};

    let rows = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: false,
            loading: false,
        },
        ListRow {
            id: "c",
            label: Line::from("Gamma"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "d",
            label: Line::from("Delta"),
            leading: None,
            secondary: None,
                status: None,
            badge: None,
            shortcut: None,
                actions: None,
            trailing: None,
                custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let mut state = ListState::new(Some("a"));
    let _ = state.handle_intent(&rows, UiIntent::Move(NavigationMove::Next));
    frame.render_widget(
        Panel::new(system)
            .title("CollectionState → List")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_stateful_widget(List::new(&rows, system).focused(true), inner, &mut state);
}

fn roving_focus_group_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{RovingEntry, RovingFocusGroup, RovingOrientation};
    use termrock::widgets::{Panel, PanelChrome};

    let entries = vec![
        RovingEntry::new("a", "Alpha"),
        RovingEntry::new("b", "Beta").enabled(false),
        RovingEntry::new("c", "Gamma"),
        RovingEntry::new("d", "Delta"),
    ];
    let mut g = RovingFocusGroup::new().orientation(RovingOrientation::Vertical);
    let _ = g.reconcile(&entries);
    let _ = g.move_next(&entries); // a -> c (skip b)
    let active = g.active().copied().unwrap_or("—");
    frame.render_widget(
        Panel::new(system)
            .title("RovingFocusGroup")
            .chrome(PanelChrome::Focused),
        area,
    );
    let mut y = area.y.saturating_add(1);
    for e in &entries {
        if y >= area.bottom() {
            break;
        }
        let mark = if Some(&e.id) == g.active() { "›" } else { " " };
        let dis = if e.enabled { "" } else { " (disabled)" };
        let line = format!("{mark} {}{dis}", e.label);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(area.x.saturating_add(1), y, area.width.saturating_sub(2), 1),
        );
        y = y.saturating_add(1);
    }
    if y < area.bottom() {
        frame.render_widget(
            Paragraph::new(format!("active={active}  hints: ↑↓ Home/End typeahead")),
            Rect::new(area.x.saturating_add(1), y, area.width.saturating_sub(2), 1),
        );
    }
    let _ = system;
}

fn focus_graph_workbench_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{
        FocusGraph, FocusLens, FocusNavMode, FocusNode, FocusOutcome,
    };
    use termrock::widgets::{Panel, PanelChrome};

    let mut g = FocusGraph::new().mode(FocusNavMode::Hybrid);
    let sidebar = Rect::new(area.x, area.y.saturating_add(1), 12, area.height.saturating_sub(2));
    let list = Rect::new(
        area.x.saturating_add(13),
        area.y.saturating_add(1),
        18,
        area.height.saturating_sub(2),
    );
    let editor = Rect::new(
        area.x.saturating_add(32),
        area.y.saturating_add(1),
        area.width.saturating_sub(33),
        area.height.saturating_sub(2),
    );
    g.register(
        FocusNode::leaf("sidebar", sidebar)
            .zone("sidebar")
            .tab_index(0),
    );
    g.register(
        FocusNode::roving_collection("files", list)
            .zone("main")
            .tab_index(1),
    );
    g.register(
        FocusNode::leaf("editor", editor)
            .zone("main")
            .tab_index(2),
    );
    let _ = g.reconcile();
    let _ = g.request_focus("files");

    frame.render_widget(
        Panel::new(system)
            .title("FocusGraph")
            .chrome(g.panel_chrome_for(&"files")),
        area,
    );
    // Zone panels
    for (title, r, id) in [
        ("sidebar", sidebar, "sidebar"),
        ("files*", list, "files"),
        ("editor", editor, "editor"),
    ] {
        frame.render_widget(
            Panel::new(system)
                .title(title)
                .chrome(g.panel_chrome_for(&id)),
            r,
        );
    }
    frame.render_widget(FocusLens::new(&g, system), area);
    let snap = g.debug_snapshot();
    let lines = snap.summary_lines(3);
    let status_y = area.bottom().saturating_sub(1);
    if status_y >= area.y {
        let msg = lines.first().cloned().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(msg),
            Rect::new(area.x.saturating_add(1), status_y, area.width.saturating_sub(2), 1),
        );
    }
    let _ = FocusOutcome::Unchanged::<&str>;
}

fn event_result_compose_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{EventResult, compose_bubble};
    use termrock::widgets::{Panel, PanelChrome};

    #[derive(Clone, Copy, Debug)]
    enum DemoMsg {
        Child,
        Parent,
    }

    let child: EventResult<DemoMsg> = EventResult::emit(DemoMsg::Child);
    let merged = compose_bubble(child, || EventResult::emit(DemoMsg::Parent));
    let line = format!(
        "bubble: child-stop → msg={:?} consumed={} redraw={:?}",
        merged.message(),
        merged.is_consumed(),
        merged.redraw()
    );
    let bubbled = compose_bubble(EventResult::<DemoMsg>::ignored(), || {
        EventResult::emit(DemoMsg::Parent)
    });
    let line2 = format!(
        "bubble: child-ignore → msg={:?} consumed={}",
        bubbled.message(),
        bubbled.is_consumed()
    );
    frame.render_widget(
        Panel::new(system)
            .title("EventResult")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_widget(Paragraph::new(line), Rect::new(inner.x, inner.y, inner.width, 1));
    if inner.height > 1 {
        frame.render_widget(
            Paragraph::new(line2),
            Rect::new(inner.x, inner.y.saturating_add(1), inner.width, 1),
        );
    }
    let _ = system;
}

fn design_inspector(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let layers = ["root"];
    let recipes = ["list_row", "panel"];
    let mut semantic = termrock::interaction::SemanticScene::<&str>::new();
    let _ = semantic.register(
        termrock::interaction::SemanticNode::content("list", area)
            .role(termrock::interaction::SemanticRole::List)
            .label("Files"),
    );
    let _ = semantic.register_child(
        "list",
        termrock::interaction::SemanticNode::control(
            "row0",
            Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        )
        .role(termrock::interaction::SemanticRole::ListItem)
        .label("a.rs"),
    );
    let summary = semantic.snapshot().summary_lines(8);
    let semantics: Vec<&str> = summary.iter().map(String::as_str).collect();
    let snap = DesignInspectorFrame {
        focused: Some("row0"),
        layer: Some("root"),
        capability: ColorCapability::Truecolor,
        density: "comfortable",
        layers: &layers,
        recipes: &recipes,
        selection_chrome: "gutter",
        semantics: &semantics,
    focus_graph: &[],
    };
    frame.render_widget(
        DesignInspector::new(snap, system).panel(InspectorPanel::Semantics),
        area,
    );
}

fn semantic_scene_tree_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState};
    use termrock::widgets::{Panel, PanelChrome};

    let mut scene = SemanticScene::<&str, &str>::new();
    let _ = scene.register(
        SemanticNode::content("app", area)
            .role(SemanticRole::Chrome)
            .label("Workbench"),
    );
    let list_area = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let _ = scene.register_child(
        "app",
        SemanticNode::content("files", list_area)
            .role(SemanticRole::List)
            .label("Files"),
    );
    for (i, name) in ["main.rs", "lib.rs", "scene.rs"].iter().enumerate() {
        let y = list_area
            .y
            .saturating_add(u16::try_from(i).unwrap_or(0).saturating_add(1));
        let _ = scene.register_child(
            "files",
            SemanticNode::control(
                *name,
                Rect::new(list_area.x, y, list_area.width, 1),
            )
            .role(SemanticRole::ListItem)
            .label(*name)
            .state(SemanticState {
                selected: i == 0,
                ..SemanticState::default()
            })
            .actions(vec!["open", "copy"]),
        );
    }
    let summary = scene.snapshot_with(|a| (*a).to_string()).summary_lines(12);
    let semantics: Vec<&str> = summary.iter().map(String::as_str).collect();
    let layers = ["root"];
    let recipes = ["semantic_tree"];
    let snap = DesignInspectorFrame {
        focused: Some("main.rs"),
        layer: Some("root"),
        capability: ColorCapability::Truecolor,
        density: "compact",
        layers: &layers,
        recipes: &recipes,
        selection_chrome: "gutter",
        semantics: &semantics,
        focus_graph: &[],
    };
    // Panel chrome + semantics body.
    frame.render_widget(
        Panel::new(system)
            .title("SemanticScene")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_widget(
        DesignInspector::new(snap, system).panel(InspectorPanel::Semantics),
        inner,
    );
}

fn semantic_scene_hit_jump_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SemanticNode, SemanticRole, SemanticScene};
    use termrock::widgets::{
        JumpOverlay, Panel, PanelChrome, assign_jump_badges_from_semantics,
    };
    use ratatui::layout::Position;

    let mut scene = SemanticScene::<&str>::new();
    let mid = area.y.saturating_add(area.height / 2);
    let _ = scene.register(
        SemanticNode::control("left", Rect::new(area.x, mid, 8, 1))
            .role(SemanticRole::Button)
            .label("Left"),
    );
    let _ = scene.register(
        SemanticNode::control(
            "right",
            Rect::new(area.x.saturating_add(12), mid, 8, 1),
        )
        .role(SemanticRole::Button)
        .label("Right"),
    );
    let hit = scene
        .hit_test_focusable(Position::new(area.x.saturating_add(13), mid))
        .map(|n| n.id)
        .unwrap_or("—");
    let targets = assign_jump_badges_from_semantics(&scene);
    frame.render_widget(
        Panel::new(system)
            .title("hit+jump")
            .subtitle(hit)
            .chrome(PanelChrome::Focused),
        area,
    );
    frame.render_widget(JumpOverlay::new(&targets, system), area);
}

fn semantic_scene_snapshot_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticSnapshot};
    use termrock::widgets::Panel;

    let mut scene = SemanticScene::<&str, &str>::new();
    let _ = scene.register(
        SemanticNode::content("root", area)
            .role(SemanticRole::Chrome)
            .label("App"),
    );
    let _ = scene.register_child(
        "root",
        SemanticNode::control(
            "btn",
            Rect::new(area.x.saturating_add(1), area.y.saturating_add(1), 10, 1),
        )
        .role(SemanticRole::Button)
        .label("Run")
        .actions(vec!["activate"]),
    );
    let text = scene.snapshot_with(|a| (*a).to_string()).to_text();
    let parsed = SemanticSnapshot::from_text(&text);
    let round = parsed.to_text();
    frame.render_widget(Panel::new(system).title("snapshot"), area);
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let mut y = inner.y;
    for line in round.lines().take(usize::from(inner.height)) {
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            line,
            usize::from(inner.width),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
    }
}

fn semantic_scene_virt_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SemanticNode, SemanticRole, SemanticScene};
    use termrock::widgets::{Panel, PanelChrome};

    let logical = 1_000_000usize;
    let visible = usize::from(area.height.saturating_sub(2)).max(1).min(20);
    let offset = 42_000usize;
    let mut scene = SemanticScene::<usize>::new();
    scene.reserve(visible);
    scene.register_many((0..visible).map(|i| {
        let id = offset + i;
        SemanticNode::control(
            id,
            Rect::new(
                area.x.saturating_add(1),
                area.y.saturating_add(1).saturating_add(i as u16),
                area.width.saturating_sub(2),
                1,
            ),
        )
        .role(SemanticRole::ListItem)
        .label(format!("row {id}"))
    }));
    let title = format!("virt {}/{}", scene.len(), logical);
    frame.render_widget(
        Panel::new(system)
            .title(&title)
            .chrome(PanelChrome::Normal),
        area,
    );
    let summary = scene.snapshot().summary_lines(visible.min(8));
    let mut y = area.y.saturating_add(1);
    for line in summary {
        if y >= area.bottom().saturating_sub(1) {
            break;
        }
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            y,
            &line,
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
    }
}

fn capability_color_ladder_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    // Four stacked inspector strips: truecolor → 256 → 16 → mono.
    let caps = [
        (ColorCapability::Truecolor, "truecolor"),
        (ColorCapability::Indexed256, "256"),
        (ColorCapability::Ansi16, "ansi16"),
        (ColorCapability::Monochrome, "mono"),
    ];
    let h = (area.height / 4).max(1);
    let layers = ["root"];
    let recipes = ["role_swatch"];
    for (i, (cap, label)) in caps.iter().enumerate() {
        let y = area
            .y
            .saturating_add(u16::try_from(i).unwrap_or(0).saturating_mul(h));
        let row = Rect::new(
            area.x,
            y,
            area.width,
            h.min(area.bottom().saturating_sub(y)),
        );
        if row.is_empty() {
            continue;
        }
        let q = system.clone().quantize(*cap);
        let snap = DesignInspectorFrame {
            focused: Some(label),
            layer: Some("root"),
            capability: *cap,
            density: "compact",
            layers: &layers,
            recipes: &recipes,
            selection_chrome: "gutter",
        semantics: &[],
        focus_graph: &[],
        };
        frame.render_widget(DesignInspector::new(snap, &q), row);
    }
}

fn capability_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mono = system.clone().quantize(ColorCapability::Monochrome);
    let layers = ["root"];
    let recipes = ["panel", "list_row"];
    let snap = DesignInspectorFrame {
        focused: Some("focus"),
        layer: Some("root"),
        capability: ColorCapability::Monochrome,
        density: "comfortable",
        layers: &layers,
        recipes: &recipes,
        selection_chrome: "gutter",
    semantics: &[],
    focus_graph: &[],
    };
    frame.render_widget(DesignInspector::new(snap, &mono), area);
}

fn capability_ascii_glyphs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system
        .clone()
        .density(Density::Compact)
        .glyphs(termrock::style::GlyphSet::Ascii);
    let rows = [
        ListRow::item("a", Line::from("ASCII disclosure")),
        ListRow::item("b", Line::from("Selected row")).badge(Line::from("ok")),
        ListRow::item("c", Line::from("Loading…")).loading(),
    ];
    let mut state = ListState::<&str>::new(Some("b"));
    frame.render_stateful_widget(&List::new(&rows, &tokens), area, &mut state);
}

fn capability_headless_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mono = system.clone().quantize(ColorCapability::Monochrome);
    let layers = ["headless"];
    let recipes = ["buffer_only"];
    let snap = DesignInspectorFrame {
        focused: None,
        layer: Some("headless"),
        capability: ColorCapability::Monochrome,
        density: "compact",
        layers: &layers,
        recipes: &recipes,
        selection_chrome: "none",
    semantics: &[],
    focus_graph: &[],
    };
    frame.render_widget(DesignInspector::new(snap, &mono), area);
}

fn registry_contracts_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::registry::{official_kernel_contracts, validate_contracts};
    use termrock::widgets::{Panel, PanelChrome};

    let catalog = official_kernel_contracts();
    let report = validate_contracts(&catalog);
    let _ = report.ok();
    frame.render_widget(
        Panel::new(system)
            .title("ComponentContract · official catalog")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let mut y = inner.y;
    for c in catalog.into_iter().take(usize::from(inner.height)) {
        let line = format!(
            "{:<18} {:<10} s={} t={} {}",
            c.id,
            c.kind.id(),
            c.stories.len(),
            c.tests.len(),
            if c.complete { "✓" } else { "…" }
        );
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            &line,
            usize::from(inner.width),
            system.style(termrock::style::Role::Text),
        );
        y = y.saturating_add(1);
    }
}

fn spinner_labeled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    let state = SpinnerState::new();
    let tick = FrameTick::manual(Instant::now(), Duration::from_millis(400), Duration::ZERO);
    Spinner::labeled("Fetching packages", system).paint(
        area,
        frame.buffer_mut(),
        &state,
        tick,
        Motion::Full,
    );
}

fn spinner_phases_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    let tick = FrameTick::manual(Instant::now(), Duration::from_millis(320), Duration::ZERO);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);
    for (i, (phase, label)) in [
        (ActivityPhase::Indeterminate, "Working"),
        (ActivityPhase::Waiting, "Waiting"),
        (ActivityPhase::Queued, "Queued"),
        (ActivityPhase::Reconnecting, "Reconnecting"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut state = SpinnerState::new();
        state.set_phase(phase);
        Spinner::labeled(label, system)
            .phase(phase)
            .paint(chunks[i], frame.buffer_mut(), &state, tick, Motion::Full);
    }
}

fn spinner_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    use termrock::widgets::SpinnerVariant;
    let mut state = SpinnerState::new();
    state.set_embedded_in_labeled_control(true);
    state.set_variant(SpinnerVariant::CompactInline);
    let tick = FrameTick::manual(Instant::now(), Duration::from_millis(240), Duration::ZERO);
    Spinner::new(system)
        .embedded(true)
        .variant(SpinnerVariant::CompactInline)
        .paint(area, frame.buffer_mut(), &state, tick, Motion::Full);
}

fn spinner_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    let mut state = SpinnerState::new();
    state.set_ascii(true);
    let tick = FrameTick::manual(Instant::now(), Duration::from_millis(160), Duration::ZERO);
    Spinner::labeled("Loading", system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &state, tick, Motion::Full);
}

fn activity_indicator_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    use termrock::style::Motion;
    let mut state = SpinnerState::new();
    state.set_phase(ActivityPhase::Reconnecting);
    let tick = FrameTick::manual(Instant::now(), Duration::from_millis(200), Duration::ZERO);
    ActivityIndicator::new("Reconnecting to agent", system)
        .detail("attempt 2/5 · backoff 1.2s")
        .paint(area, frame.buffer_mut(), &state, tick, Motion::Full);
}

fn motion_presence_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use std::time::{Duration, Instant};
    use termrock::runtime::{FrameClock, FrameTick, Presence, spinner_demand};
    use termrock::style::Motion;
    use termrock::widgets::{Panel, PanelChrome, Spinner};

    frame.render_widget(
        Panel::new(system)
            .title("FrameClock · Presence · Motion")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let start = Instant::now();
    let mut clock = FrameClock::from_start(start);
    let tick = clock.tick_at(start + Duration::from_millis(560));
    let spin_full = Spinner::new(system).frame_glyph(tick, Motion::Full);
    let spin_off = Spinner::new(system).frame_glyph(tick, Motion::Off);
    let mut toast = Presence::toast(Duration::from_secs(2));
    toast.request_show(FrameTick::manual(start, Duration::ZERO, Duration::ZERO));
    let demand = spinner_demand(tick, Motion::Full, true);
    let idle = spinner_demand(tick, Motion::Full, false);
    let lines = [
        format!("spinner Full={spin_full} Off={spin_off}"),
        format!(
            "toast visible={} focusable={}",
            toast.is_visible(),
            toast.is_focusable()
        ),
        format!(
            "demand active={} idle={}",
            demand.needs_redraw, idle.needs_redraw
        ),
    ];
    let mut y = inner.y;
    for line in lines {
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            &line,
            usize::from(inner.width),
            system.style(termrock::style::Role::Text),
        );
        y = y.saturating_add(1);
    }
}

fn capability_profiles_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::capability::{CapabilityBoundary, CapabilityProfile, TerminalCapabilities};
    use termrock::widgets::{Panel, PanelChrome};

    frame.render_widget(
        Panel::new(system)
            .title("TerminalCapabilities · profiles")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let profiles = [
        CapabilityProfile::Modern,
        CapabilityProfile::Compatible,
        CapabilityProfile::Minimal,
        CapabilityProfile::Inline,
        CapabilityProfile::Headless,
    ];
    let mut y = inner.y;
    for p in profiles {
        let caps = TerminalCapabilities::for_profile(p);
        let b = CapabilityBoundary::from_capabilities(&caps);
        let h = b.component_hints();
        let line = format!(
            "{:<11} color={:?} ascii={} mouse={} alt={} kbd={}",
            p.id(),
            caps.set.color,
            h.ascii as u8,
            h.mouse as u8,
            caps.set.alternate_screen as u8,
            h.interactive as u8,
        );
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            &line,
            usize::from(inner.width),
            system.style(termrock::style::Role::Text),
        );
        y = y.saturating_add(1);
        if y >= inner.bottom() {
            break;
        }
    }
}

fn responsive_ladder_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::layout::{ResponsiveSnapshot, ResponsiveSurface, WIDTH_LADDER};
    use termrock::widgets::{Panel, PanelChrome};

    frame.render_widget(
        Panel::new(system)
            .title("Responsive · Form@40 + ladder")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let snap = ResponsiveSnapshot::for_surface(ResponsiveSurface::Form, 40, 20);
    let mut y = inner.y;
    for line in snap.lines().into_iter().take(usize::from(inner.height)) {
        frame.buffer_mut().set_stringn(
            inner.x,
            y,
            &line,
            usize::from(inner.width),
            system.style(termrock::style::Role::Text),
        );
        y = y.saturating_add(1);
        if y >= inner.bottom() {
            break;
        }
    }
    let _ = WIDTH_LADDER;
}

fn dismissable_gestures_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{
        DismissDecision, DismissEventId, DismissGuard, DismissPolicy, DismissableLayer,
    };
    use termrock::widgets::{Panel, PanelChrome};

    let mut menu = DismissableLayer::new(DismissPolicy::dismissible());
    menu.set_rect(Rect::new(
        area.x.saturating_add(4),
        area.y.saturating_add(2),
        area.width.saturating_sub(16).max(12),
        area.height.saturating_sub(5).max(3),
    ));
    let mut alert = DismissableLayer::new(DismissPolicy::critical());
    alert.set_rect(Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4).max(10),
        area.height.saturating_sub(3).max(4),
    ));
    let mut g = DismissGuard::new();
    let outside = menu.on_outside_click(
        ratatui::layout::Position::new(area.x, area.y),
        &mut g,
        DismissEventId(1),
    );
    let mut g2 = DismissGuard::new();
    let trap = alert.on_escape(&mut g2, DismissEventId(2));
    let note = match (outside, trap) {
        (DismissDecision::Dismiss { .. }, DismissDecision::Consumed) => {
            "outside→dismiss · Esc on critical→trap"
        }
        _ => "dismiss policy demo",
    };
    frame.render_widget(
        Panel::new(system)
            .title("DismissableLayer")
            .chrome(PanelChrome::Focused),
        area,
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_widget(
        Panel::new(system)
            .title("menu body")
            .chrome(PanelChrome::Normal),
        menu.rect(),
    );
    frame.buffer_mut().set_stringn(
        inner.x,
        inner.bottom().saturating_sub(1),
        note,
        usize::from(inner.width),
        system.style(termrock::style::Role::TextMuted),
    );
}

fn overlay_nested(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut stack = OverlayStack::<()>::new();
    let _ = stack.open(
        area,
        OverlaySpec::dialog("parent", OverlaySize::dialog(36, 10), None),
    );
    let parent = stack.top().unwrap().rect;
    let anchor = Rect::new(parent.x.saturating_add(2), parent.y.saturating_add(4), 6, 1);
    let _ = stack.open(
        area,
        OverlaySpec::menu("child", anchor, OverlaySize::menu(18, 4), None).with_parent("parent"),
    );
    paint_stack_rects(frame, area, &stack, &tokens, system);
    frame.render_widget(
        Paragraph::new(Line::from("Esc peels child then parent")),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
}

fn overlay_edges(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let size = OverlaySize::menu(14, 3);
    let policy = OverlayPolicy::for_kind(OverlayKind::Menu);
    let anchors = [
        Rect::new(area.x, area.y, 4, 1),
        Rect::new(area.right().saturating_sub(6), area.y, 4, 1),
        Rect::new(area.x, area.bottom().saturating_sub(2), 4, 1),
        Rect::new(
            area.right().saturating_sub(6),
            area.bottom().saturating_sub(2),
            4,
            1,
        ),
    ];
    for (i, anchor) in anchors.iter().enumerate() {
        let r = place_overlay(area, Some(*anchor), size, policy);
        frame.render_widget(
            Panel::new(&tokens)
                .title(match i {
                    0 => "TL",
                    1 => "TR",
                    2 => "BL",
                    _ => "BR",
                })
                .emphasis(PanelChrome::Normal),
            r,
        );
    }
}

fn overlay_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::Compact);
    let mut stack = OverlayStack::<()>::new();
    let _ = stack.open(
        area,
        OverlaySpec::dialog("d", OverlaySize::dialog(48, 12), None),
    );
    let dialog_rect = stack.top().map(|e| e.rect).unwrap_or(area);
    frame.render_widget(
        Dialog::new("Tiny", Line::from("fullscreen promote").into(), &tokens)
            .emphasis(PanelChrome::Focused),
        dialog_rect,
    );
    let tip = place_overlay(
        area,
        Some(Rect::new(area.x, area.y, 2, 1)),
        OverlaySize::menu(20, 1),
        OverlayPolicy::for_kind(OverlayKind::Tooltip),
    );
    let note = if tip.width == 0 {
        "tooltip hidden"
    } else {
        "tooltip shown"
    };
    frame.render_widget(
        Paragraph::new(Line::from(note)),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
}

fn overlay_queued(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::OpenMode;
    let tokens = system.clone().density(Density::default());
    let mut stack = OverlayStack::<()>::new();
    let _ = stack.open_with(
        area,
        OverlaySpec::dialog("d1", OverlaySize::dialog(30, 6), None),
        OpenMode::Queue,
    );
    let _ = stack.open_with(
        area,
        OverlaySpec::dialog("d2", OverlaySize::dialog(28, 6), None),
        OpenMode::Queue,
    );
    // Only d1 open; d2 waits in queue until d1 dismisses.
    paint_stack_rects(frame, area, &stack, &tokens, system);
    frame.render_widget(
        Paragraph::new(Line::from(format!(
            "open={} queue={} (Esc peels one)",
            stack.entries().len(),
            stack.queue_len()
        ))),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
}

fn overlay_fullscreen_promote(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut stack = OverlayStack::<()>::new();
    let _ = stack.open(
        area,
        OverlaySpec::popover(
            "pop",
            Rect::new(area.x.saturating_add(2), area.y.saturating_add(2), 4, 1),
            OverlaySize::menu(20, 5),
            None,
        ),
    );
    let _ = stack.promote_top_fullscreen(area);
    if let Some(top) = stack.top() {
        frame.render_widget(
            Panel::new(&tokens)
                .title("promoted")
                .emphasis(PanelChrome::Focused),
            top.rect,
        );
    }
}

fn paint_stack_rects(
    frame: &mut Frame<'_>,
    _area: Rect,
    stack: &OverlayStack<()>,
    tokens: &DesignSystem,
    _system: &DesignSystem,
) {
    for (i, entry) in stack.entries().iter().enumerate() {
        if entry.rect.width == 0 || entry.rect.height == 0 {
            continue;
        }
        let title = match entry.kind {
            OverlayKind::Dialog => "dialog",
            OverlayKind::Menu => "menu",
            OverlayKind::Popover => "popover",
            _ => "layer",
        };
        frame.render_widget(
            Panel::new(tokens)
                .title(title)
                .emphasis(if i + 1 == stack.entries().len() {
                    PanelChrome::Focused
                } else {
                    PanelChrome::Normal
                }),
            entry.rect,
        );
    }
}

fn dialog(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    frame.render_widget(
        Dialog::new(
            "Notice",
            Line::from("The operation completed.").into(),
            &tokens,
        )
        .description("All changes were written successfully.")
        .style(Style::new())
        .emphasis(termrock::widgets::PanelChrome::Focused)
        .recipe(DialogRecipe::Normal)
        .footer_hint("esc dismiss"),
        area,
    );
}

fn dialog_destructive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let actions = [
        Action {
            id: "delete",
            label: "Delete",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let mut state = ChoiceDialogState::new(Some("cancel"));
    state
        .dialog_mut()
        .set_recipe(DialogRecipe::Destructive);
    frame.render_stateful_widget(
        &ChoiceDialog::new(
            Dialog::new(
                "Delete project",
                Line::from("This cannot be undone.").into(),
                &tokens,
            )
            .description("All files and history will be removed.")
            .recipe(DialogRecipe::Destructive)
            .footer_hint("esc cancel · enter confirms focused"),
            &actions,
        ),
        area,
        &mut state,
    );
}

fn dialog_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    frame.render_widget(
        Dialog::new("Saved", Line::from("OK").into(), &tokens)
            .recipe(DialogRecipe::Compact)
            .emphasis(termrock::widgets::PanelChrome::Focused)
            .footer_hint("esc"),
        area,
    );
}

fn alert_dialog_delete_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertDialogState::new(
        AlertKind::Delete,
        AlertScope::example_delete(),
        "delete",
        "keep",
    );
    AlertDialog::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn alert_dialog_overwrite_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertDialogState::new(
        AlertKind::Overwrite,
        AlertScope::example_overwrite(),
        "overwrite",
        "keep",
    );
    AlertDialog::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn alert_dialog_terminate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertDialogState::new(
        AlertKind::Terminate,
        AlertScope::example_terminate(),
        "terminate",
        "keep",
    );
    state.set_gates(AlertConfirmGates::countdown(5_000));
    // Mid-countdown for preview
    let _ = state.tick(2_000);
    AlertDialog::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn alert_dialog_egress_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertDialogState::new(
        AlertKind::DataEgress,
        AlertScope::example_data_egress(),
        "allow",
        "deny",
    );
    state.set_gates(AlertConfirmGates::typed("EXPORT"));
    // Partial type for preview
    for c in "EXP".chars() {
        let _ = state.handle_key(termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char(c),
            termrock::input::KeyModifiers::NONE,
        ));
    }
    AlertDialog::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn alert_dialog_locked_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertDialogState::new(
        AlertKind::Delete,
        AlertScope::example_delete().safer_alternative("Contact an admin to unlock."),
        "delete",
        "keep",
    );
    state.set_locked(true);
    state.set_title("Critical: acknowledge deletion");
    AlertDialog::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn choice_dialog(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ChoiceDialogState::new(Some("continue"));
    render_choice_dialog(frame, area, &mut state, system);
}

pub(crate) fn choice_actions() -> [Action<'static, &'static str>; 2] {
    [
        Action {
            id: "continue",
            label: "Continue",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: Some(Style::new().bold()),
        },
    ]
}

pub(crate) fn render_choice_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut ChoiceDialogState<&'static str>,
    system: &DesignSystem,
) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let actions = choice_actions();
    frame.render_stateful_widget(
        &ChoiceDialog::new(
            Dialog::new(
                "Choose",
                Line::from("Continue with this operation?").into(),
                &tokens,
            )
            .style(Style::new())
            .emphasis(termrock::widgets::PanelChrome::Focused),
            &actions,
        )
        .gap(" "),
        area,
        state,
    );
}

fn message_dialog(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let details = [
        DetailRow {
            id: "state",
            label: "State",
            value: "Ready",
            href: None,
            capability: DetailCapability::None,
            emphasis: false,
            style: None,
        },
        DetailRow {
            id: "reference",
            label: "Reference",
            value: "example-42",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &MessageDialog::new(
            Dialog::new(
                "Result",
                Line::from("The operation completed.").into(),
                &tokens,
            )
            .style(Style::new())
            .emphasis(termrock::widgets::PanelChrome::Focused),
            &details,
            system,
        )
        .label_width(14)
        .wrap(true),
        area,
        &mut state,
    );
}

fn diff_sample_lines() -> (Vec<DiffLine<'static>>, [DiffHunk; 2]) {
    let lines = vec![
        DiffLine::file_header("f", "diff --git a/main.rs b/main.rs").file_id("main.rs"),
        DiffLine::hunk_header("h0", "@@ -1,4 +1,5 @@")
            .file_id("main.rs")
            .hunk_id("h0"),
        DiffLine::context("c1", "fn main() {")
            .old_no(1)
            .new_no(1)
            .file_id("main.rs")
            .hunk_id("h0"),
        DiffLine::removed("r1", "    println!(\"hi\");")
            .old_no(2)
            .file_id("main.rs")
            .hunk_id("h0")
            .trailing_ws(true),
        DiffLine::added("a1", "    println!(\"hello 東京\");")
            .new_no(2)
            .file_id("main.rs")
            .hunk_id("h0"),
        DiffLine::added("a2", "    // ready 🧪")
            .new_no(3)
            .file_id("main.rs")
            .hunk_id("h0"),
        DiffLine::context("c2", "}")
            .old_no(3)
            .new_no(4)
            .file_id("main.rs")
            .hunk_id("h0"),
        DiffLine::hunk_header("h1", "@@ -20,2 +21,2 @@")
            .file_id("main.rs")
            .hunk_id("h1"),
        DiffLine::removed("r2", "old_path")
            .old_no(20)
            .file_id("main.rs")
            .hunk_id("h1"),
        DiffLine::added("a3", "new_path")
            .new_no(21)
            .file_id("main.rs")
            .hunk_id("h1"),
    ];
    let hunks = [
        DiffHunk::new(1, 6, "@@ -1,4 +1,5 @@")
            .id("h0")
            .file_id("main.rs"),
        DiffHunk::new(7, 3, "@@ -20,2 +21,2 @@")
            .id("h1")
            .file_id("main.rs"),
    ];
    (lines, hunks)
}

fn diff_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks) = diff_sample_lines();
    let mut state = DiffViewState::new();
    state.mode = DiffMode::Unified;
    DiffView::new(&lines, system)
        .hunks(&hunks)
        .title("main.rs")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_split(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks) = diff_sample_lines();
    let mut state = DiffViewState::new();
    state.mode = DiffMode::Split;
    DiffView::new(&lines, system)
        .hunks(&hunks)
        .title("main.rs · split")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_word(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let words_rm = [
        DiffWordSpan::new(DiffWordKind::Equal, "let x = "),
        DiffWordSpan::new(DiffWordKind::Delete, "1"),
        DiffWordSpan::new(DiffWordKind::Equal, ";"),
    ];
    let words_add = [
        DiffWordSpan::new(DiffWordKind::Equal, "let x = "),
        DiffWordSpan::new(DiffWordKind::Insert, "42"),
        DiffWordSpan::new(DiffWordKind::Equal, ";"),
    ];
    let lines = [
        DiffLine::hunk_header("h", "@@ -1,1 +1,1 @@").hunk_id("h"),
        DiffLine::removed("r", "let x = 1;")
            .old_no(1)
            .hunk_id("h")
            .words(&words_rm),
        DiffLine::added("a", "let x = 42;")
            .new_no(1)
            .hunk_id("h")
            .words(&words_add),
    ];
    let hunks = [DiffHunk::new(0, 3, "@@ -1,1 +1,1 @@").id("h")];
    let mut state = DiffViewState::new();
    state.word_diff = true;
    DiffView::new(&lines, system)
        .hunks(&hunks)
        .title("word-level")
        .render(area, frame.buffer_mut(), &mut state);
}

fn diff_search(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (lines, hunks) = diff_sample_lines();
    let mut state = DiffViewState::new();
    state.search = Some("hello".into());
    DiffView::new(&lines, system)
        .hunks(&hunks)
        .title("search")
        .render(area, frame.buffer_mut(), &mut state);
}

fn toast(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Toast::new(system, "Updated", Severity::Success).anchor(Anchor::TopRight),
        area,
    );
}

fn toast_kinds_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ToastKind, ToastQueue, ToastSpec, ToastStack};
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    let mut q = ToastQueue::new();
    q.set_anchor(Anchor::TopRight);
    let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
    let _ = q.push(
        tick,
        ToastSpec::message("i", "Heads up").severity(Severity::Info),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("s", "Saved").severity(Severity::Success),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("w", "Disk low").severity(Severity::Warning),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("e", "Failed").severity(Severity::Error),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("p", "Uploading").progress(45).group("up"),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("u", "Deleted draft").undo("Undo"),
    );
    let _ = ToastKind::Undo;
    ToastStack::new(system).paint(area, frame.buffer_mut(), &mut q);
}

fn toast_stack_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ToastPriority, ToastQueue, ToastSpec, ToastStack};
    use std::time::{Duration, Instant};
    use termrock::runtime::FrameTick;
    let mut q = ToastQueue::new();
    q.set_max_visible(4);
    q.set_anchor(Anchor::BottomRight);
    let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
    let _ = q.push(
        tick,
        ToastSpec::message("1", "Agent finished step 1").priority(ToastPriority::Normal),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("2", "Agent finished step 2").priority(ToastPriority::High),
    );
    let _ = q.push(
        tick,
        ToastSpec::message("3", "Background sync").priority(ToastPriority::Low),
    );
    ToastStack::new(system).paint(area, frame.buffer_mut(), &mut q);
}

fn toast_persistent_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Toast::new(system, "Pinned notice — dismiss in host", Severity::Warning)
            .anchor(Anchor::TopLeft)
            .title("Persistent"),
        area,
    );
}

fn notification_center_drawer_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NotificationCenterState::new();
    state.replace_items(example_notifications(1_700_000_000));
    state.set_recipe(NotificationRecipe::Drawer);
    let _ = state.open();
    state.set_focused(true);
    NotificationCenter::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn notification_center_full_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NotificationCenterState::new();
    state.replace_items(example_notifications(1_700_000_000));
    state.set_recipe(NotificationRecipe::FullPage);
    let _ = state.open();
    state.set_focused(true);
    NotificationCenter::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn notification_center_filter_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::NotificationFilter;
    let mut state = NotificationCenterState::new();
    state.replace_items(example_notifications(1_700_000_000));
    state.set_recipe(NotificationRecipe::FullPage);
    let _ = state.open();
    let _ = state.set_filter(NotificationFilter::Unread);
    state.set_focused(true);
    NotificationCenter::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn notification_center_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NotificationCenterState::new();
    let _ = state.open();
    state.set_recipe(NotificationRecipe::Drawer);
    state.set_focused(true);
    NotificationCenter::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn backdrop(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let style = if system.palette() == &RolePalette::tailrocks_phosphor() {
        Style::new().dim()
    } else {
        system.style(Role::Backdrop)
    };
    frame.render_widget(Backdrop::new().symbol('░').style(style), area);
}

fn viewport(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = [
        Line::from("alpha: short"),
        Line::from("beta: a deliberately wide borrowed row for horizontal scrolling"),
        Line::from("gamma: 🧪 Unicode"),
        Line::from("delta: fourth row"),
        Line::from("epsilon: fifth row"),
        Line::from("zeta: sixth row"),
    ];
    let border_style = system.style(Role::BorderFocused);
    let system = DesignSystem::from_palette(
        system
            .palette()
            .clone()
            .with_role(Role::Border, border_style),
    );
    let mut state = DialogScroll::default();
    frame.render_stateful_widget(
        &Viewport::new(&lines, &system)
            .title("Viewport")
            .content_style(Style::new()),
        area,
        &mut state,
    );
}

fn empty_state(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_search(system).paint(area, frame.buffer_mut());
}

fn empty_state_first_use_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_sessions(system).paint(area, frame.buffer_mut());
}

fn empty_state_table_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_table(system).paint(area, frame.buffer_mut());
}

fn empty_state_permission_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_permission(system).paint(area, frame.buffer_mut());
}

fn empty_state_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    EmptyState::new("No selection", system)
        .kind(EmptyKind::NoData)
        .explanation("Pick a story")
        .primary(EmptyAction::new("Browse"))
        .density(EmptyDensity::Inline)
        .paint(area, frame.buffer_mut());
}

fn empty_state_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    EmptyState::new("No results", system)
        .kind(EmptyKind::NoResults)
        .explanation("Try another query")
        .primary(EmptyAction::new("Clear"))
        .paint(area, frame.buffer_mut());
}

fn empty_state_logs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_logs(system).paint(area, frame.buffer_mut());
}

fn empty_state_projects_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_empty_projects(system).paint(area, frame.buffer_mut());
}

fn loading_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(LoadingView::new("Loading…", "⠋", system), area);
}

fn loading_tick() -> termrock::runtime::FrameTick {
    use std::time::{Duration, Instant};
    termrock::runtime::FrameTick::manual(
        Instant::now(),
        Duration::from_millis(400),
        Duration::from_millis(16),
    )
}

fn loading_overlay_blocking_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    use termrock::widgets::Panel;
    let _ = Panel::new(system).title("table").paint(area, frame.buffer_mut(), None);
    let (overlay, mut st) = example_busy_blocking(system);
    overlay.paint(
        area,
        frame.buffer_mut(),
        &mut st,
        loading_tick(),
        Motion::Off,
    );
}

fn loading_overlay_cancellable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    let (overlay, mut st) = example_busy_cancellable(system);
    overlay.paint(
        area,
        frame.buffer_mut(),
        &mut st,
        loading_tick(),
        Motion::Off,
    );
}

fn loading_overlay_non_blocking_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    frame.buffer_mut().set_stringn(
        area.x,
        area.y.saturating_add(1),
        "rows still interactive",
        22,
        system.style(Role::TextMuted),
    );
    let (overlay, mut st) = example_busy_non_blocking(system);
    overlay.paint(
        area,
        frame.buffer_mut(),
        &mut st,
        loading_tick(),
        Motion::Off,
    );
}

fn loading_overlay_optimistic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    frame.buffer_mut().set_stringn(
        area.x,
        area.y.saturating_add(1),
        "saved draft body",
        16,
        system.style(Role::Text),
    );
    let (overlay, mut st) = example_busy_optimistic(system);
    overlay.paint(
        area,
        frame.buffer_mut(),
        &mut st,
        loading_tick(),
        Motion::Off,
    );
}

fn loading_overlay_stale_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    frame.buffer_mut().set_stringn(
        area.x,
        area.y.saturating_add(2),
        "old cached rows",
        15,
        system.style(Role::TextMuted),
    );
    let (overlay, mut st) = example_busy_stale(system);
    overlay.paint(
        area,
        frame.buffer_mut(),
        &mut st,
        loading_tick(),
        Motion::Off,
    );
}

fn loading_overlay_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Motion;
    use termrock::widgets::Panel;
    let _ = Panel::new(system)
        .title("workbench")
        .paint(area, frame.buffer_mut(), None);
    let mut parent = BusyBoundaryState::new();
    let _ = parent.begin(BusyMode::Blocking, "Outer load");
    parent.set_elapsed_ms(400);
    parent.set_expected_ms(Some(5_000));
    let mut child = BusyBoundaryState::nested_under(&parent);
    let _ = child.begin(BusyMode::Cancellable, "Pane fetch");
    child.set_elapsed_ms(400);
    child.set_expected_ms(Some(5_000));
    let child_area = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(2),
        area.width.saturating_sub(4).max(1),
        area.height.saturating_sub(4).max(1),
    );
    BusyBoundary::paint_nested(
        area,
        child_area,
        frame.buffer_mut(),
        &mut parent,
        &mut child,
        system,
        loading_tick(),
        Motion::Off,
    );
}

fn connectivity_banner_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let s = example_reconnecting_agent();
    OfflineBanner::new(&s, system).paint(area, frame.buffer_mut());
}

fn connectivity_reconnecting_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut s = example_reconnecting_agent();
    s.set_presentation(ConnectivityPresentation::Full);
    OfflineSurface::new(system).paint(area, frame.buffer_mut(), &mut s);
}

fn connectivity_auth_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut s = example_auth_required();
    OfflineSurface::new(system).paint(area, frame.buffer_mut(), &mut s);
}

fn connectivity_unavailable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut s = example_server_unavailable();
    OfflineSurface::new(system).paint(area, frame.buffer_mut(), &mut s);
}

fn connectivity_status_bar_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StatusBar, StatusBarState, StatusSlot};
    let s = example_reconnecting_agent();
    let content = s.status_bar_content();
    let left = [StatusSlot::mode("m", "NOR")];
    let right = [s.status_slot_template("c", content.as_str())];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::new(&left, &right, system),
        area,
        &mut state,
    );
}

fn connectivity_notification_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{NotificationCenter, NotificationCenterState, NotificationRecipe};
    let conn = example_reconnecting_agent();
    let mut state = NotificationCenterState::new();
    state.replace_items(vec![conn.to_notification_item("conn-1")]);
    state.set_recipe(NotificationRecipe::Drawer);
    let _ = state.open();
    state.set_focused(true);
    NotificationCenter::new(system).paint(area, frame.buffer_mut(), &mut state);
}

fn error_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_network(system).paint(area, frame.buffer_mut());
}

fn error_state_network_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_network(system).paint(area, frame.buffer_mut());
}

fn error_state_validation_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_validation(system).paint(area, frame.buffer_mut());
}

fn error_state_permission_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_permission(system).paint(area, frame.buffer_mut());
}

fn error_state_details_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::ErrorStateState::new();
    state.set_details_expanded(true);
    example_error_network(system).paint_with_state(area, frame.buffer_mut(), &mut state);
}

fn error_state_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_unsupported(system).paint(area, frame.buffer_mut());
}

fn error_state_dialog_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_dialog(system).paint(area, frame.buffer_mut());
}

fn error_state_fullscreen_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    example_error_crash(system).paint(area, frame.buffer_mut());
}

fn error_view_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    ErrorState::new("Request failed", system)
        .kind(ErrorKind::Network)
        .explanation("Timed out")
        .recovery(Recovery::retry_only("Retry", RetrySafety::Safe))
        .paint(area, frame.buffer_mut());
}

fn banner(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Banner::new("Deployed successfully", Severity::Success, system),
        area,
    );
}

fn skeleton(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(Skeleton::new(4, system), area);
}

fn skeleton_card_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Skeleton;
    Skeleton::card(3, system).paint(area, frame.buffer_mut());
}

fn skeleton_table_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Skeleton;
    Skeleton::table(3, 5, system).paint(area, frame.buffer_mut());
}

fn skeleton_tiny_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Skeleton;
    Skeleton::new(3, system).ascii(true).paint(area, frame.buffer_mut());
}

fn skeleton_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Skeleton, SkeletonRecipe};
    Skeleton::recipe(SkeletonRecipe::Rows, 4, system)
        .ascii(true)
        .paint(area, frame.buffer_mut());
}

fn jump_overlay(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(
        Panel::new(&panel_tokens)
            .title("Jump targets")
            .emphasis(PanelChrome::Normal),
        area,
    );
    let targets = [
        JumpTarget::new(
            "files",
            Rect::new(area.x.saturating_add(2), area.y.saturating_add(1), 12, 1),
            "f",
        ),
        JumpTarget::new(
            "main",
            Rect::new(area.x.saturating_add(2), area.y.saturating_add(3), 12, 1),
            "m",
        ),
    ];
    frame.render_widget(
        JumpOverlay::new(&targets, system).ascii(false),
        area,
    );
}

fn jump_mode_multi(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Panel::new(system)
            .title("Jump multi-key")
            .emphasis(PanelChrome::Normal),
        area,
    );
    let labels = generate_jump_labels(30);
    let targets: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let row = (i as u16) % area.height.saturating_sub(2).max(1);
            let col = ((i as u16) / area.height.saturating_sub(2).max(1)) * 6;
            JumpTarget::new(
                i,
                Rect::new(
                    area.x.saturating_add(1).saturating_add(col),
                    area.y.saturating_add(1).saturating_add(row),
                    5,
                    1,
                ),
                k.clone(),
            )
        })
        .collect();
    let mut state = JumpOverlayState::new();
    state.open();
    // Prefix first letter of a multi-key label for dim demo
    if let Some(t) = targets.iter().find(|t| t.keys.len() >= 2) {
        let ch = t.keys.chars().next().unwrap();
        let _ = state.handle_key(
            termrock::input::KeyEvent::new(
                termrock::input::KeyCode::Char(ch),
                termrock::input::KeyModifiers::NONE,
            ),
            &targets,
        );
    }
    frame.render_widget(
        JumpOverlay::from_state(&targets, system, &state).ascii(false),
        area,
    );
}

fn jump_mode_filter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{SemanticNode, SemanticRole, SemanticScene};
    frame.render_widget(
        Panel::new(system)
            .title("Jump filter: buttons")
            .emphasis(PanelChrome::Normal),
        area,
    );
    let mut scene = SemanticScene::<&str, &str>::new();
    let _ = scene.register(
        SemanticNode::control(
            "run",
            Rect::new(area.x.saturating_add(2), area.y.saturating_add(2), 8, 1),
        )
        .role(SemanticRole::Button)
        .label("Run")
        .actions(vec!["activate"]),
    );
    let _ = scene.register(
        SemanticNode::control(
            "query",
            Rect::new(area.x.saturating_add(2), area.y.saturating_add(4), 12, 1),
        )
        .role(SemanticRole::Input)
        .label("Query"),
    );
    let filter = JumpFilter::new().roles([SemanticRole::Button]);
    let targets = assign_jump_labels_from_semantics(&scene, &filter);
    frame.render_widget(JumpOverlay::new(&targets, system), area);
}

fn jump_mode_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let targets = [
        JumpTarget::new(
            "a",
            Rect::new(area.x.saturating_add(1), area.y.saturating_add(1), 10, 1),
            "a",
        ),
        JumpTarget::new(
            "b",
            Rect::new(area.x.saturating_add(1), area.y.saturating_add(3), 10, 1),
            "b",
        ),
    ];
    frame.render_widget(
        JumpOverlay::new(&targets, system).ascii(true).colorless(true),
        area,
    );
}

fn focus_lens_combined(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::interaction::{FocusGraph, FocusLens, FocusLensMode, FocusNavMode, FocusNode};
    let mut g = FocusGraph::new().mode(FocusNavMode::Hybrid);
    g.register(
        FocusNode::leaf(
            "side",
            Rect::new(area.x, area.y.saturating_add(1), 10, 3),
        )
        .tab_index(0),
    );
    g.register(
        FocusNode::leaf(
            "main",
            Rect::new(
                area.x.saturating_add(12),
                area.y.saturating_add(1),
                16,
                5,
            ),
        )
        .tab_index(1),
    );
    let _ = g.reconcile();
    let _ = g.request_focus("main");
    frame.render_widget(
        Panel::new(system)
            .title("FocusLens")
            .emphasis(PanelChrome::Normal),
        area,
    );
    frame.render_widget(
        FocusLens::new(&g, system)
            .mode(FocusLensMode::Combined)
            .ascii(false),
        area,
    );
}

fn command_palette(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::new(None);
    state.set_focused(true);
    let mut entries = example_command_catalog();
    entries.push(
        CommandEntry::new("tokyo", "Open 東京 workspace")
            .group("Navigation")
            .keywords(["tokyo", "東京"]),
    );
    let visible = state.refilter(&entries);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &visible, system),
        area,
        &mut state,
    );
}

fn command_palette_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::<&str>::new(None);
    state.set_focused(true);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &[], system),
        area,
        &mut state,
    );
}

fn command_palette_no_result(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::<&str>::new(None);
    state.set_focused(true);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('z'),
            termrock::input::KeyModifiers::NONE,
        ),
        &[],
    );
    // Force query without matching catalog
    *state.query_mut() = termrock::widgets::TextInputState::new("zzz").with_allow_empty(true);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &[], system),
        area,
        &mut state,
    );
}

fn command_palette_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::<&str>::new(None);
    state.set_focused(true);
    let _ = state.set_loading(true);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &[], system),
        area,
        &mut state,
    );
}

fn command_palette_fuzzy(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::new(None);
    state.set_focused(true);
    let cat = example_command_catalog();
    *state.query_mut() = termrock::widgets::TextInputState::new("thm").with_allow_empty(true);
    let visible = state.refilter(&cat);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &visible, system),
        area,
        &mut state,
    );
}

fn command_palette_nested(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::new(None);
    state.set_focused(true);
    let cat = example_command_catalog();
    let _ = state.open_page("keys", "Keyboard shortcuts");
    let visible = state.refilter(&cat);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &visible, system),
        area,
        &mut state,
    );
}

fn command_palette_args(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::new(None);
    state.set_focused(true);
    let cat = example_command_catalog();
    let mut visible = state.refilter(&cat);
    if let Some(idx) = visible.iter().position(|e| e.id == "goto-line") {
        // Activate via key path after moving cursor — use open by refilter order.
        let _ = idx;
        let _ = state.handle_key(
            termrock::input::KeyEvent::new(
                termrock::input::KeyCode::Enter,
                termrock::input::KeyModifiers::NONE,
            ),
            &visible,
        );
        // Ensure argument phase: if first item isn't goto-line, force NeedArguments path.
        if !matches!(
            state.phase(),
            termrock::widgets::CommandPalettePhase::Argument { .. }
        ) {
            visible = state.refilter(&cat);
            if let Some(i) = visible.iter().position(|e| e.id == "goto-line") {
                // activate_at is private — use mouse-free public open via open_page no
                // Simulate by opening argument through handle after setting active via Down
                for _ in 0..i {
                    let _ = state.handle_key(
                        termrock::input::KeyEvent::new(
                            termrock::input::KeyCode::Down,
                            termrock::input::KeyModifiers::NONE,
                        ),
                        &visible,
                    );
                }
                let _ = state.handle_key(
                    termrock::input::KeyEvent::new(
                        termrock::input::KeyCode::Enter,
                        termrock::input::KeyModifiers::NONE,
                    ),
                    &visible,
                );
            }
        }
    }
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &visible, system),
        area,
        &mut state,
    );
}

fn command_palette_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CommandPaletteState::<&str>::new(None);
    state.set_focused(true);
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &[], system)
            .ascii(true)
            .colorless(true)
            .focused(true),
        area,
        &mut state,
    );
}

fn quick_open_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let items = example_quick_open_files();
    let mut state = QuickOpenState::new();
    state.set_focused(true);
    let _ = state.apply_results(0, &items, true, Some(items.len() as u64));
    QuickOpen::new(&providers, &items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_symbols(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let items = example_quick_open_symbols();
    let mut state = QuickOpenState::new();
    state.set_focused(true);
    let _ = state.set_provider(&providers, 1, &[]);
    let _ = state.apply_results(state.generation(), &items, true, None);
    QuickOpen::new(&providers, &items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_fuzzy(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let catalog = example_quick_open_files();
    let items = filter_quick_open_items(&catalog, "qck");
    let mut state = QuickOpenState::new();
    state.set_focused(true);
    *state.query_mut() = TextInputState::new("qck").with_allow_empty(true);
    let _ = state.apply_results(0, &items, true, None);
    QuickOpen::new(&providers, &items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let mut state = QuickOpenState::<&str>::new();
    state.set_focused(true);
    let _ = state.set_loading(true);
    QuickOpen::new(&providers, &[], system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let mut state = QuickOpenState::<&str>::new();
    state.set_focused(true);
    QuickOpen::new(&providers, &[], system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let items = example_quick_open_files();
    let mut state = QuickOpenState::new();
    state.set_focused(true);
    state.set_presentation_override(Some(termrock::widgets::QuickOpenPresentation::Fullscreen));
    let _ = state.apply_results(0, &items, true, None);
    QuickOpen::new(&providers, &items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn quick_open_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let providers = example_quick_open_providers();
    let items = example_quick_open_files();
    let mut state = QuickOpenState::new();
    state.set_focused(true);
    let _ = state.apply_results(0, &items, true, None);
    QuickOpen::new(&providers, &items, system)
        .ascii(true)
        .colorless(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn code_block(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::RoleTokenSyntax;
    let lines = ["fn main() {", "    println!(\"hi\");", "}"];
    let hi = RoleTokenSyntax::rust(system);
    let mut state = CodeBlockState::new();
    let _ = CodeBlock::new(&lines, system)
        .language("rust")
        .path("src/main.rs")
        .line_numbers(true)
        .highlighter(&hi)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn code_block_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::RoleTokenSyntax;
    let system = system.clone().no_color();
    let lines = ["fn main() {", "    // comment", "    let x = 1;", "}"];
    let hi = RoleTokenSyntax::rust(&system);
    let mut state = CodeBlockState::new();
    let _ = CodeBlock::new(&lines, &system)
        .language("rust")
        .line_numbers(true)
        .highlighter(&hi)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn code_block_streaming_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = ["```rust", "fn partial() {"];
    let mut state = CodeBlockState::new();
    let _ = CodeBlock::new(&lines, system)
        .language("rust")
        .streaming(true)
        .line_numbers(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn code_block_wrap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = [
        "let very_long_identifier_that_should_wrap_when_narrow = 42;",
    ];
    let mut state = CodeBlockState::new();
    let _ = CodeBlock::new(&lines, system)
        .wrap(CodeWrap::Wrap)
        .line_numbers(true)
        .language("rust")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn code_block_highlights_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{CodeGutterMark, RoleTokenSyntax};
    let lines = ["ok line", "error here", "ok line"];
    let marks = [CodeGutterMark::new(1, '!', Role::Danger)];
    let highs = [
        CodeHighlight::line(1, CodeHighlightKind::Diagnostic),
        CodeHighlight::line(0, CodeHighlightKind::Selection),
    ];
    let hi = RoleTokenSyntax::rust(system);
    let mut state = CodeBlockState::new();
    state.set_focused(true);
    state.set_cursor_line(Some(1));
    let _ = CodeBlock::new(&lines, system)
        .line_numbers(true)
        .highlighter(&hi)
        .gutter_marks(&marks)
        .highlights(&highs)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn markdown_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::project_markdown;
    let src = "\
# Plan

Implement the **markdown** surface with [links](https://example.invalid).

- [ ] tasks remain open
- [x] done items check

1. First ordered
2. Second ordered

> Quote with hanging indent.

```rust
fn main() {}
```

| col | val |
|-----|-----|
| a   | 1   |
| b   | 2   |
";
    // Leak-free: project into static-ish by using function-local owned string via once
    // Stories use short literals only — project_markdown borrows src.
    let blocks = project_markdown(src);
    let mut state = MarkdownViewState::new();
    let _ = MarkdownView::new(&blocks, system).paint(area, frame.buffer_mut(), &mut state);
}

fn markdown_streaming_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::project_markdown;
    let src = "```rust\nfn partial() {\n    // still streaming";
    let blocks = project_markdown(src);
    let mut state = MarkdownViewState::new();
    let _ = MarkdownView::new(&blocks, system)
        .fence_line_numbers(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn markdown_table_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::project_markdown;
    let src = "\
| name | status | detail |
|------|--------|--------|
| alpha | ok | ready |
| beta | warn | slow |
";
    let blocks = project_markdown(src);
    let mut state = MarkdownViewState::new();
    let _ = MarkdownView::new(&blocks, system).paint(area, frame.buffer_mut(), &mut state);
}

fn markdown_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::project_markdown;
    let system = system.clone().no_color();
    let src = "# Title\n\nBody with `code` and a list:\n\n- one\n- two\n";
    let blocks = project_markdown(src);
    let mut state = MarkdownViewState::new();
    let _ = MarkdownView::new(&blocks, &system)
        .compact_headings(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn sparkline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let samples = [0.1, 0.3, 0.2, 0.7, 0.9, f64::NAN, 0.8, 0.4, 0.6, 0.95];
    frame.render_widget(
        Sparkline::new(&samples, system)
            .pre_normalized(true)
            .threshold(0.75)
            .selected(4),
        area,
    );
}

fn chart_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cpu = [12.0, 18.0, 40.0, 55.0, 48.0, 62.0, 70.0, 58.0, 45.0, 30.0];
    let mem = [40.0, 42.0, 44.0, 50.0, 52.0, 55.0, 60.0, 58.0, 57.0, 56.0];
    let series = [
        ChartSeries::new("cpu", &cpu),
        ChartSeries::new("mem", &mem),
    ];
    let thr = [65.0];
    frame.render_widget(
        Chart::new(&series, system)
            .title("host")
            .thresholds(&thr)
            .selected_series(0)
            .selected_index(6)
            .scale(ScaleMode::Fixed { min: 0.0, max: 100.0 }),
        area,
    );
}

fn chart_nocolor(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let system = DesignSystem::from_palette(system.palette().clone())
        .glyphs(termrock::style::GlyphSet::Ascii)
        .no_color();
    let a = [1.0, 3.0, 2.0, 5.0, 4.0, 6.0, 3.0];
    let b = [5.0, 4.0, 4.0, 3.0, 2.0, 2.0, 1.0];
    let series = [
        ChartSeries::new("in", &a),
        ChartSeries::new("out", &b),
    ];
    frame.render_widget(
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .title("io"),
        area,
    );
}

fn gauge_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let thr = [70.0, 90.0];
    frame.render_widget(
        Gauge::percent(82.0, system)
            .label("cpu")
            .unit("%")
            .thresholds(&thr),
        area,
    );
}

fn histogram_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let buckets = [
        HistBucket::new("0", 1.0),
        HistBucket::new("1", 3.0),
        HistBucket::new("2", 7.0),
        HistBucket::new("3", 4.0),
        HistBucket::new("4", 2.0),
    ];
    frame.render_widget(
        Histogram::new(&buckets, system)
            .title("latency")
            .selected(2),
        area,
    );
}

fn bar_series(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let bars = [
        BarDatum {
            label: "cpu",
            fraction: 0.72,
        },
        BarDatum {
            label: "mem",
            fraction: 0.41,
        },
        BarDatum {
            label: "disk",
            fraction: 0.88,
        },
    ];
    frame.render_widget(BarSeries::new(&bars, system).selected(2), area);
}

fn segmented_meter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let segments = [
        MeterSegment {
            label: "used",
            weight: 3.0,
            role: Role::Success,
        },
        MeterSegment {
            label: "cache",
            weight: 1.0,
            role: Role::Info,
        },
        MeterSegment {
            label: "free",
            weight: 2.0,
            role: Role::TextDisabled,
        },
    ];
    frame.render_widget(
        SegmentedMeter::new(&segments, system).selected(0),
        area,
    );
}

fn token_meter(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(TokenMeter::new(128_000, 200_000, system), area);
}

fn thinking_block(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        ThinkingBlock::new("Planning edits", system)
            .frame("·")
            .expanded(true)
            .body("Inspect contracts, then implement."),
        area,
    );
}

fn tool_card(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        ToolCard::new(
            "shell",
            "cargo test -p termrock",
            ToolStatus::Running,
            system,
        )
        .expanded(true)
        .detail("running suite…"),
        area,
    );
}

fn transcript_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let user = ["Run the suite", "with unicode: 日本語 🚀"];
    let assistant = [
        "Sure — preparing the environment.",
        "Running tests…",
        "All green.",
    ];
    let tool = ["cargo test — passed"];
    let blocks = [
        TranscriptBlock::new("u1", TranscriptKind::User, &user),
        TranscriptBlock::new("a1", TranscriptKind::Assistant, &assistant),
        TranscriptBlock::new("t1", TranscriptKind::Tool, &tool).folded(false),
    ];
    let mut state = TranscriptState::new();
    frame.render_stateful_widget(&Transcript::new(&blocks, system), area, &mut state);
}

fn timeline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        TimelineEvent::with_id("a", "12:01", "Started deploy")
            .status(TimelineStatus::Success)
            .actor("ci")
            .relative("1h ago")
            .duration("12s"),
        TimelineEvent::with_id("b", "12:02", "Running tests")
            .status(TimelineStatus::Running)
            .active()
            .actor("ci")
            .correlation("trace-9"),
        TimelineEvent::with_id("c", "12:03", "Open PR")
            .status(TimelineStatus::Pending)
            .actor("bot"),
    ];
    let mut state = TimelineState::new();
    state.following = false;
    state.cursor = 1;
    frame.render_stateful_widget(
        Timeline::with_events(&events, system).focused(true),
        area,
        &mut state,
    );
}

fn timeline_rail(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        TimelineEvent::with_id("1", "t0", "plan"),
        TimelineEvent::with_id("2", "t1", "tool").status(TimelineStatus::Running).active(),
        TimelineEvent::with_id("3", "t2", "done").status(TimelineStatus::Success),
    ];
    let mut state = TimelineState::new();
    state.following = false;
    frame.render_stateful_widget(
        Timeline::with_events(&events, system)
            .recipe(TimelineRecipe::Rail)
            .focused(true),
        area,
        &mut state,
    );
}

fn timeline_grouped(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        TimelineEvent::group("d0", "Yesterday"),
        TimelineEvent::with_id("y1", "18:00", "Nightly build")
            .status(TimelineStatus::Success)
            .group_key("Yesterday"),
        TimelineEvent::group("d1", "Today"),
        TimelineEvent::with_id("t1", "09:00", "Deploy staging")
            .status(TimelineStatus::Success)
            .actor("cd")
            .group_key("Today"),
        TimelineEvent::with_id("t2", "09:12", "Smoke tests")
            .status(TimelineStatus::Failed)
            .actor("ci")
            .group_key("Today")
            .detail("health check timeout"),
        TimelineEvent::with_id("t3", "09:20", "Rollback")
            .status(TimelineStatus::Warning)
            .active()
            .group_key("Today"),
    ];
    let mut state = TimelineState::new();
    state.following = false;
    state.cursor = 4;
    frame.render_stateful_widget(
        Timeline::with_events(&events, system)
            .recipe(TimelineRecipe::GroupedDay)
            .focused(true),
        area,
        &mut state,
    );
}

fn timeline_streaming(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        TimelineEvent::with_id("1", "12:00:01", "session start").status(TimelineStatus::Info),
        TimelineEvent::with_id("2", "12:00:02", "tool:search").status(TimelineStatus::Success),
        TimelineEvent::with_id("3", "12:00:03", "tool:edit").status(TimelineStatus::Running).active(),
        TimelineEvent::with_id("4", "12:00:04", "awaiting approval").status(TimelineStatus::Warning),
    ];
    let mut state = TimelineState::new();
    state.following = true;
    state.on_append(events.len());
    frame.render_stateful_widget(
        Timeline::with_events(&events, system).focused(true),
        area,
        &mut state,
    );
}

fn timeline_checkpoint_rows(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let events = [
        TimelineEvent::checkpoint("c0", "10:00", "session open"),
        TimelineEvent::checkpoint("c1", "10:05", "after plan").active(),
        TimelineEvent::checkpoint("c2", "10:12", "pre-apply"),
    ];
    let mut state = TimelineState::new();
    state.following = false;
    state.set_checkpoint_mode(true);
    state.cursor = 1;
    frame.render_stateful_widget(
        Timeline::with_events(&events, system).focused(true),
        area,
        &mut state,
    );
}

fn checkpoint_timeline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_checkpoints, CheckpointTimeline, CheckpointTimelineState};
    let mut state = CheckpointTimelineState::new();
    state.set_checkpoints(example_checkpoints());
    state.focused = true;
    frame.render_stateful_widget(&CheckpointTimeline::new(system), area, &mut state);
}

fn checkpoint_timeline_preview_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_checkpoints, CheckpointTimeline, CheckpointTimelineMode, CheckpointTimelineState,
    };
    let mut state = CheckpointTimelineState::new();
    state.set_checkpoints(example_checkpoints());
    state.cursor = 2;
    state.selected = Some("c2".into());
    state.mode = CheckpointTimelineMode::Preview;
    state.focus_id = Some("c2".into());
    state.focused = true;
    frame.render_stateful_widget(&CheckpointTimeline::new(system), area, &mut state);
}

fn checkpoint_timeline_confirm_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_checkpoints, CheckpointConfirmAction, CheckpointTimeline, CheckpointTimelineMode,
        CheckpointTimelineState,
    };
    let mut state = CheckpointTimelineState::new();
    state.set_checkpoints(example_checkpoints());
    state.cursor = 1;
    state.selected = Some("c1".into());
    state.mode = CheckpointTimelineMode::Confirm;
    state.confirm_action = Some(CheckpointConfirmAction::Restore);
    state.confirm_proceed_focused = false;
    state.last_warning = Some("restore files/state to checkpoint (host executes)".into());
    state.focused = true;
    frame.render_stateful_widget(&CheckpointTimeline::new(system), area, &mut state);
}

fn checkpoint_timeline_boundaries_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_checkpoints, CheckpointTimeline, CheckpointTimelineState,
    };
    let mut state = CheckpointTimelineState::new();
    state.set_checkpoints(example_checkpoints());
    // Focus dirty workspace checkpoint
    if let Some(i) = state.checkpoints.iter().position(|c| c.id == "c3") {
        state.cursor = i;
        state.selected = Some("c3".into());
    }
    state.focused = true;
    frame.render_stateful_widget(&CheckpointTimeline::new(system), area, &mut state);
}

fn checkpoint_timeline_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Checkpoint, CheckpointTimeline, CheckpointTimelineState};
    let mut state = CheckpointTimelineState::new();
    state.set_checkpoints(vec![
        Checkpoint::new("u0", "12:00", "検査 🔍")
            .summary("ファイル状態")
            .files(["src/日本語.rs"]),
        Checkpoint::new("u1", "12:05", "分岐 ⑂").branch("探索", Some("u0")),
        Checkpoint::new("u2", "12:10", "現在").head(),
    ]);
    state.focused = true;
    frame.render_stateful_widget(&CheckpointTimeline::new(system), area, &mut state);
}

fn prompt_composer_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ComposerChip, ContextEstimate, ModeIndicator, ModelIndicator};
    let tokens = system
        .clone()
        .density(termrock::style::Density::Comfortable);
    let mut state = PromptComposerState::new();
    state.set_placeholder("Ask anything…");
    state.set_text("Explain this module");
    state.set_mode(Some(ModeIndicator {
        label: "EDIT".into(),
        warning: false,
    }));
    state.set_model(Some(ModelIndicator {
        label: "grok".into(),
    }));
    state.set_context(ContextEstimate {
        used: 24_000,
        limit: 128_000,
    });
    state.add_chip(ComposerChip::file("a", "lib.rs"));
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}

fn prompt_composer_busy(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ModeIndicator, ModelIndicator};
    let tokens = system.clone().density(termrock::style::Density::Compact);
    let mut state = PromptComposerState::new();
    state.set_busy(true);
    state.set_text("follow-up while running");
    state.set_mode(Some(ModeIndicator {
        label: "AUTO".into(),
        warning: true,
    }));
    state.set_model(Some(ModelIndicator {
        label: "model".into(),
    }));
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}




fn approval_queue_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_approval_queue, ApprovalQueue, ApprovalQueuePresentation, ApprovalQueueState,
    };
    let mut state = ApprovalQueueState::new();
    state.set_items(example_approval_queue());
    state.presentation = ApprovalQueuePresentation::Full;
    state.focused = true;
    frame.render_stateful_widget(&ApprovalQueue::new(system), area, &mut state);
}

fn approval_queue_badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_approval_queue, ApprovalQueue, ApprovalQueuePresentation, ApprovalQueueState,
    };
    let mut state = ApprovalQueueState::new();
    state.set_items(example_approval_queue());
    state.presentation = ApprovalQueuePresentation::Badge;
    state.focused = true;
    frame.render_stateful_widget(&ApprovalQueue::new(system), area, &mut state);
}

fn approval_queue_drawer_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_approval_queue, ApprovalQueue, ApprovalQueuePresentation, ApprovalQueueState,
    };
    let mut state = ApprovalQueueState::new();
    state.set_items(example_approval_queue());
    state.presentation = ApprovalQueuePresentation::Drawer;
    state.focused = true;
    frame.render_stateful_widget(&ApprovalQueue::new(system), area, &mut state);
}

fn approval_queue_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ApprovalItem, ApprovalKind, ApprovalQueue, ApprovalQueuePresentation, ApprovalQueueState,
        PermissionRisk,
    };
    let mut state = ApprovalQueueState::new();
    state.set_items(vec![ApprovalItem::new(
        "u",
        ApprovalKind::Question,
        "続行しますか？",
        PermissionRisk::Low,
    )
    .actor("エージェント")]);
    state.presentation = ApprovalQueuePresentation::Full;
    state.focused = true;
    frame.render_stateful_widget(&ApprovalQueue::new(system), area, &mut state);
}

fn working_state_card_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_working_state, WorkingStateCard, WorkingStateCardState,
        WorkingStatePresentation,
    };
    let mut state = WorkingStateCardState::new();
    state.set_work(Some(example_working_state()));
    state.presentation = WorkingStatePresentation::Expanded;
    state.focused = true;
    frame.render_stateful_widget(&WorkingStateCard::new(system), area, &mut state);
}

fn working_state_card_waiting_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_working_waiting, WorkingStateCard, WorkingStateCardState,
        WorkingStatePresentation,
    };
    let mut state = WorkingStateCardState::new();
    state.set_work(Some(example_working_waiting()));
    state.presentation = WorkingStatePresentation::Expanded;
    state.focused = true;
    frame.render_stateful_widget(&WorkingStateCard::new(system), area, &mut state);
}

fn working_state_card_collapsed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_working_state, WorkingStateCard, WorkingStateCardState,
        WorkingStatePresentation,
    };
    let mut state = WorkingStateCardState::new();
    state.set_work(Some(example_working_state()));
    state.presentation = WorkingStatePresentation::Collapsed;
    state.focused = true;
    frame.render_stateful_widget(&WorkingStateCard::new(system), area, &mut state);
}

fn working_state_card_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        WorkingPhase, WorkingResource, WorkingState, WorkingStateCard, WorkingStateCardState,
        WorkingStatePresentation,
    };
    let mut state = WorkingStateCardState::new();
    state.set_work(Some(
        WorkingState::new("u", WorkingPhase::Searching, "ファイルを検索 🔍")
            .resources(vec![WorkingResource::new("f", "日本語.rs")]),
    ));
    state.presentation = WorkingStatePresentation::Expanded;
    state.focused = true;
    frame.render_stateful_widget(&WorkingStateCard::new(system), area, &mut state);
}

fn integration_status_list_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_integrations, IntegrationStatus, IntegrationStatusPresentation,
        IntegrationStatusState,
    };
    let mut state = IntegrationStatusState::new();
    state.set_entries(example_integrations());
    state.presentation = IntegrationStatusPresentation::CompactList;
    state.focused = true;
    frame.render_stateful_widget(&IntegrationStatus::new(system), area, &mut state);
}

fn integration_status_panel_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_integrations, IntegrationDetailTab, IntegrationStatus,
        IntegrationStatusPresentation, IntegrationStatusState,
    };
    let mut state = IntegrationStatusState::new();
    state.set_entries(example_integrations());
    state.presentation = IntegrationStatusPresentation::Panel;
    if let Some(i) = state.entries.iter().position(|e| e.id == "mcp-web") {
        state.cursor = i;
    }
    state.tab = IntegrationDetailTab::Permissions;
    state.focused = true;
    frame.render_stateful_widget(&IntegrationStatus::new(system), area, &mut state);
}

fn integration_status_badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_integrations, IntegrationStatus, IntegrationStatusPresentation,
        IntegrationStatusState,
    };
    let mut state = IntegrationStatusState::new();
    state.set_entries(example_integrations());
    state.presentation = IntegrationStatusPresentation::Badge;
    state.focused = true;
    frame.render_stateful_widget(&IntegrationStatus::new(system), area, &mut state);
}

fn integration_status_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        IntegrationEntry, IntegrationHealth, IntegrationKind, IntegrationProvenance,
        IntegrationStatus, IntegrationStatusPresentation, IntegrationStatusState,
    };
    let mut state = IntegrationStatusState::new();
    state.set_entries(vec![
        IntegrationEntry::new("u1", "検査 MCP 🔍", IntegrationKind::McpServer)
            .health(IntegrationHealth::Connected)
            .provenance(IntegrationProvenance::third_party("発行者", "pkg:日本語", "1.0")),
    ]);
    state.presentation = IntegrationStatusPresentation::CompactList;
    state.focused = true;
    frame.render_stateful_widget(&IntegrationStatus::new(system), area, &mut state);
}

fn agent_status_header_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_agent_status, AgentStatusHeader, AgentStatusHeaderState, AgentStatusPresentation,
    };
    let mut state = AgentStatusHeaderState::new();
    state.set_snapshot(example_agent_status());
    if area.width < 56 {
        state.auto_contract = true;
    } else {
        state.presentation = AgentStatusPresentation::Header;
        state.auto_contract = false;
    }
    state.focused = true;
    frame.render_stateful_widget(&AgentStatusHeader::new(system), area, &mut state);
}

fn agent_status_header_idle_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_agent_status_idle, AgentStatusHeader, AgentStatusHeaderState,
        AgentStatusPresentation,
    };
    let mut state = AgentStatusHeaderState::new();
    state.set_snapshot(example_agent_status_idle());
    state.presentation = AgentStatusPresentation::Header;
    state.auto_contract = false;
    state.focused = true;
    frame.render_stateful_widget(&AgentStatusHeader::new(system), area, &mut state);
}

fn agent_status_header_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        AgentStatusHeader, AgentStatusHeaderState, AgentStatusPresentation, AgentStatusSnapshot,
        AgentWorkStatus,
    };
    let mut state = AgentStatusHeaderState::new();
    state.set_snapshot(
        AgentStatusSnapshot::new()
            .project("プロジェクト")
            .session("検査 🔍")
            .branch("機能")
            .mode("編集")
            .model("モデル")
            .work(AgentWorkStatus::Working),
    );
    state.presentation = AgentStatusPresentation::Header;
    state.auto_contract = false;
    state.focused = true;
    frame.render_stateful_widget(&AgentStatusHeader::new(system), area, &mut state);
}

fn prompt_queue_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_prompt_queue, AgentBusyState, PromptQueue, PromptQueuePresentation,
        PromptQueueState,
    };
    let mut state = PromptQueueState::new();
    state.set_items(example_prompt_queue());
    state.set_agent(AgentBusyState::Busy);
    state.presentation = PromptQueuePresentation::Compact;
    state.focused = true;
    frame.render_stateful_widget(&PromptQueue::new(system), area, &mut state);
}

fn prompt_queue_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_prompt_queue, AgentBusyState, PromptQueue, PromptQueuePresentation,
        PromptQueueState,
    };
    let mut state = PromptQueueState::new();
    state.set_items(example_prompt_queue());
    state.set_agent(AgentBusyState::Busy);
    state.presentation = PromptQueuePresentation::Expanded;
    state.cursor = 1;
    state.focused = true;
    frame.render_stateful_widget(&PromptQueue::new(system), area, &mut state);
}

fn prompt_queue_failed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_prompt_queue, AgentBusyState, PromptQueue, PromptQueuePresentation,
        PromptQueueState, PromptQueueStatus,
    };
    let mut state = PromptQueueState::new();
    state.set_items(example_prompt_queue());
    state.set_agent(AgentBusyState::Idle);
    state.presentation = PromptQueuePresentation::Expanded;
    if let Some(i) = state
        .items
        .iter()
        .position(|e| e.status == PromptQueueStatus::Failed)
    {
        state.cursor = i;
    }
    state.focused = true;
    frame.render_stateful_widget(&PromptQueue::new(system), area, &mut state);
}

fn prompt_queue_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        AgentBusyState, PromptQueue, PromptQueueItem, PromptQueuePresentation, PromptQueueRef,
        PromptQueueState,
    };
    let mut state = PromptQueueState::new();
    state.set_items(vec![
        PromptQueueItem::new("u1", "検査して 🔍")
            .attachments(vec![PromptQueueRef::file("f", "日本語.rs")]),
        PromptQueueItem::new("u2", "次のメッセージ"),
    ]);
    state.set_agent(AgentBusyState::Busy);
    state.presentation = PromptQueuePresentation::Expanded;
    state.focused = true;
    frame.render_stateful_widget(&PromptQueue::new(system), area, &mut state);
}

fn prompt_composer_compact(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::ComposerPresentation;
    let tokens = system.clone().density(termrock::style::Density::Dashboard);
    let mut state = PromptComposerState::new();
    state.set_presentation(ComposerPresentation::Compact);
    state.set_ascii_fallback(true);
    state.set_placeholder("msg");
    state.set_text("compact draft");
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}

fn prompt_composer_paste_chip(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ComposerChip, ContextEstimate, ModeIndicator};
    let tokens = system
        .clone()
        .density(termrock::style::Density::Comfortable);
    let mut state = PromptComposerState::new();
    state.set_placeholder("Message…");
    state.set_mode(Some(ModeIndicator {
        label: "EDIT".into(),
        warning: false,
    }));
    state.set_context(ContextEstimate {
        used: 8_000,
        limit: 128_000,
    });
    let body = "x".repeat(termrock::widgets::LARGE_PASTE_THRESHOLD);
    state.add_chip(ComposerChip::paste_with_body(
        "paste-1",
        "stack dump…",
        body,
    ));
    state.set_text("see attached paste");
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}

fn prompt_composer_disconnected(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ComposerConnection, ModeIndicator, ModelIndicator};
    let tokens = system
        .clone()
        .density(termrock::style::Density::Comfortable);
    let mut state = PromptComposerState::new();
    state.set_connection(ComposerConnection::Disconnected);
    state.set_text("cannot send while offline");
    state.set_mode(Some(ModeIndicator {
        label: "ASK".into(),
        warning: false,
    }));
    state.set_model(Some(ModelIndicator {
        label: "model".into(),
    }));
    // Surface validation chrome as if user hit Enter
    let _ = state.handle_key(termrock::input::KeyEvent::new(
        termrock::input::KeyCode::Enter,
        termrock::input::KeyModifiers::NONE,
    ));
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}

fn prompt_composer_fullscreen(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ComposerPresentation, ModeIndicator, ModelIndicator};
    let tokens = system
        .clone()
        .density(termrock::style::Density::Comfortable);
    let mut state = PromptComposerState::new();
    state.set_presentation(ComposerPresentation::Fullscreen);
    state.set_placeholder("Long prompt…");
    state.set_text("Fullscreen draft for multi-paragraph agent instructions.\n\nSecond block.");
    state.set_mode(Some(ModeIndicator {
        label: "PLAN".into(),
        warning: false,
    }));
    state.set_model(Some(ModelIndicator {
        label: "model".into(),
    }));
    frame.render_stateful_widget(&PromptComposer::new(&tokens), area, &mut state);
}

fn theme_picker(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ThemePickerState::new(0);
    frame.render_stateful_widget(
        &ThemePicker::new(BUILTIN_THEME_PRESETS, system),
        area,
        &mut state,
    );
}

fn image_surface(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut meta = ImageMeta::new("preview.png", ImageProtocol::Kitty);
    meta.pixel_width = Some(128);
    meta.pixel_height = Some(96);
    frame.render_widget(ImageSurface::new(meta, system), area);
}

fn button_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    Button::new("Save", system)
        .variant(termrock::widgets::ButtonVariant::Primary)
        .leading("✓")
        .render(area, frame.buffer_mut(), &mut state);
}

fn button_variants_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ButtonSize, ButtonVariant};
    let variants = [
        (ButtonVariant::Primary, "Primary"),
        (ButtonVariant::Secondary, "Secondary"),
        (ButtonVariant::Quiet, "Quiet"),
        (ButtonVariant::Outline, "Outline"),
        (ButtonVariant::Destructive, "Delete"),
        (ButtonVariant::Link, "Link"),
        (ButtonVariant::Success, "Success"),
        (ButtonVariant::Command, "Command"),
    ];
    let mut y = area.y;
    for (v, label) in variants {
        if y >= area.bottom() {
            break;
        }
        let mut state = ButtonState::new();
        state
            .activation
            .set_accepts_input(v != ButtonVariant::Destructive);
        let row = Rect::new(area.x, y, area.width, 1);
        Button::new(label, system)
            .variant(v)
            .size(ButtonSize::Compact)
            .render(row, frame.buffer_mut(), &mut state);
        y = y.saturating_add(1);
    }
}

fn button_destructive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::ButtonVariant;
    let mut state = ButtonState::new();
    // Host must not grant default focus to destructive.
    state.activation.set_accepts_input(false);
    Button::new("Delete forever", system)
        .variant(ButtonVariant::Destructive)
        .render(area, frame.buffer_mut(), &mut state);
}

fn button_toolbar_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ButtonSize, ButtonVariant};
    let labels = ["Cut", "Copy", "Paste"];
    let mut x = area.x;
    for label in labels {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        let w = 10u16.min(area.right().saturating_sub(x));
        if w == 0 {
            break;
        }
        Button::new(label, system)
            .variant(ButtonVariant::Quiet)
            .size(ButtonSize::Compact)
            .render(Rect::new(x, area.y, w, 1), frame.buffer_mut(), &mut state);
        x = x.saturating_add(w.saturating_add(1));
    }
}

fn button_icon_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::IconButtonState;
    let mut state = IconButtonState::new();
    state.activation.set_accepts_input(true);
    termrock::widgets::IconButton::new("×", "Close dialog", system)
        .help("Close the dialog (Esc)")
        .render(area, frame.buffer_mut(), &mut state);
}

fn icon_button_toolbar_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{IconButton, IconButtonState};
    let items = [("✂", "Cut"), ("⧉", "Copy"), ("📋", "Paste")];
    let ascii = [("/", "Cut"), ("c", "Copy"), ("p", "Paste")];
    let mut x = area.x;
    for (i, ((g, label), (ag, _))) in items.iter().zip(ascii.iter()).enumerate() {
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        if i == 1 {
            state.set_pressed(true);
        }
        let w = 4u16.min(area.right().saturating_sub(x));
        if w == 0 {
            break;
        }
        let mut btn = IconButton::new(g, label, system)
            .ascii_glyph(ag)
            .toggle(i == 1);
        if i == 2 {
            btn = btn.badge("2");
        }
        btn.paint(
            Rect::new(x, area.y, w, 1),
            frame.buffer_mut(),
            &mut state,
        );
        x = x.saturating_add(w.saturating_add(1));
    }
}

fn icon_button_destructive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{IconButton, IconButtonState};
    let mut state = IconButtonState::new();
    // Host must not default-focus destructive
    state.activation.set_accepts_input(false);
    IconButton::new("🗑", "Delete", system)
        .destructive()
        .ascii_glyph("x")
        .help("Delete permanently")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn icon_button_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{IconButton, IconButtonState};
    let mut state = IconButtonState::new();
    state.activation.set_accepts_input(true);
    state.activation.set_loading(true);
    IconButton::new("↻", "Refresh", system)
        .ascii_glyph("R")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn icon_button_row_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{IconButton, IconButtonSize, IconButtonState};
    let chunks = Layout::horizontal([Constraint::Length(4), Constraint::Min(10)]).split(area);
    let mut state = IconButtonState::new();
    state.activation.set_accepts_input(true);
    IconButton::new("›", "Open row", system)
        .size(IconButtonSize::Compact)
        .ascii_glyph(">")
        .paint(chunks[0], frame.buffer_mut(), &mut state);
    frame.render_widget(
        ratatui::widgets::Paragraph::new("data-row action"),
        chunks[1],
    );
}

fn button_dialog_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::ButtonVariant;
    let chunks = Layout::horizontal([
        Constraint::Length(12),
        Constraint::Length(1),
        Constraint::Length(12),
        Constraint::Length(1),
        Constraint::Length(14),
    ])
    .split(area);
    let mut cancel = ButtonState::new();
    cancel.activation.set_accepts_input(true);
    Button::new("Cancel", system)
        .as_secondary()
        .render(chunks[0], frame.buffer_mut(), &mut cancel);
    let mut save = ButtonState::new();
    save.activation.set_accepts_input(true);
    Button::new("Save", system)
        .as_primary()
        .leading("✓")
        .render(chunks[2], frame.buffer_mut(), &mut save);
    let mut del = ButtonState::new();
    // Destructive never default-focused
    del.activation.set_accepts_input(false);
    Button::new("Delete", system)
        .variant(ButtonVariant::Destructive)
        .render(chunks[4], frame.buffer_mut(), &mut del);
}

fn button_form_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    Button::new("Create workspace", system)
        .as_primary()
        .full_width(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn button_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use ratatui::widgets::Paragraph;
    let chunks = Layout::horizontal([Constraint::Length(18), Constraint::Min(8)]).split(area);
    frame.render_widget(Paragraph::new("See also "), chunks[0]);
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    Button::new("documentation", system)
        .as_link()
        .render(chunks[1], frame.buffer_mut(), &mut state);
}

fn button_pending_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    state.activation.set_pending_confirmation(true);
    // Simulate first activate
    let _ = state.handle_intent(termrock::interaction::UiIntent::Activate);
    Button::new("Delete", system)
        .as_destructive()
        .render(area, frame.buffer_mut(), &mut state);
}

fn button_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::ButtonVariant;
    let system = system.clone().no_color();
    let chunks = Layout::vertical([Constraint::Length(1); 3]).split(area);
    for (i, (v, label)) in [
        (ButtonVariant::Primary, "Primary"),
        (ButtonVariant::Outline, "Outline"),
        (ButtonVariant::Link, "Link action"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        Button::new(label, &system)
            .variant(v)
            .colorless(true)
            .render(chunks[i], frame.buffer_mut(), &mut state);
    }
}

fn checkbox_switch_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut cb = CheckboxState::new(true);
    cb.set_focused(true);
    let _ = Checkbox::new("enable", "Enable", &tokens).paint(
        Rect::new(area.x, area.y, area.width, 1),
        frame.buffer_mut(),
        &mut cb,
    );
    let mut sw = SwitchState::new(false);
    sw.set_focused(false);
    let _ = Switch::new("dark", "Dark mode", &tokens).paint(
        Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        frame.buffer_mut(),
        &mut sw,
    );
}

fn slider_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SliderState::new(62.0);
    state.set_focused(true);
    let _ = Slider::new(SliderBounds::percent(), system)
        .label("Volume")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn slider_marks_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let marks = [
        SliderMark::labeled(0.0, "lo"),
        SliderMark::labeled(50.0, "mid"),
        SliderMark::labeled(100.0, "hi"),
    ];
    let mut state = SliderState::new(50.0);
    state.set_focused(true);
    let _ = Slider::new(SliderBounds::percent(), system)
        .label("Gain")
        .marks(&marks)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn slider_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SliderState::new(70.0);
    state.set_focused(true);
    let _ = Slider::new(SliderBounds::percent(), system)
        .vertical()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn slider_numeric_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SliderState::new(42.0);
    state.set_focused(true);
    let _ = Slider::new(SliderBounds::percent(), system)
        .label("n")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn slider_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SliderState::new(33.0);
    state.set_focused(true);
    let _ = Slider::new(SliderBounds::percent(), system)
        .label("音量 ✨")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn range_slider_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = RangeSliderState::new(20.0, 80.0);
    state.set_focused(true);
    let _ = RangeSlider::new(SliderBounds::percent(), system)
        .label("Price filter")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn range_slider_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = RangeSliderState::new(10.0, 90.0);
    state.set_focused(true);
    let _ = RangeSlider::new(SliderBounds::percent(), system)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn segmented_control_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        SegmentedItem::new("list", "List").priority(90),
        SegmentedItem::new("grid", "Grid").priority(80),
        SegmentedItem::new("table", "Table").priority(70),
    ];
    let mut state = SegmentedControlState::new(Some("grid"));
    state.set_surface_focused(true);
    let _ = SegmentedControl::new(&items, system)
        .collapse_below(0)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn segmented_control_icons_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        SegmentedItem::new("comfy", "Comfort").icon("▣").priority(90),
        SegmentedItem::new("compact", "Compact").icon("▤").badge("def").priority(80),
        SegmentedItem::new("dash", "Dash").icon("▥").priority(70),
    ];
    let mut state = SegmentedControlState::new(Some("compact"));
    state.set_surface_focused(true);
    let _ = SegmentedControl::new(&items, system)
        .collapse_below(0)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn segmented_control_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        SegmentedItem::new("list", "List").priority(90),
        SegmentedItem::new("grid", "Grid").priority(80),
        SegmentedItem::new("table", "Table").priority(40),
        SegmentedItem::new("graph", "Graph").priority(10),
        SegmentedItem::new("raw", "Raw").priority(5),
    ];
    let mut state = SegmentedControlState::new(Some("list"));
    state.set_surface_focused(true);
    let _ = SegmentedControl::new(&items, system)
        .collapse_below(0)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn segmented_control_collapsed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        SegmentedItem::new("list", "List").priority(90),
        SegmentedItem::new("grid", "Grid").priority(80),
        SegmentedItem::new("table", "Table").priority(70),
    ];
    let mut state = SegmentedControlState::new(Some("grid"));
    state.set_surface_focused(true);
    state.menu_open = true;
    let _ = SegmentedControl::new(&items, system)
        .collapse_below(40)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn segmented_control_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        SegmentedItem::new("list", "一覧"),
        SegmentedItem::new("grid", "格子"),
        SegmentedItem::new("table", "表 ✨"),
    ];
    let mut state = SegmentedControlState::new(Some("grid"));
    state.set_surface_focused(true);
    let _ = SegmentedControl::new(&items, system)
        .collapse_below(0)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn switch_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut off = SwitchState::new(false);
    let mut on = SwitchState::new(true);
    on.set_focused(true);
    let h = area.height.max(1);
    let row0 = Rect::new(area.x, area.y, area.width, 1.min(h));
    let _ = Switch::new("dark", "Dark mode", system)
        .description("Follow system appearance")
        .paint(row0, frame.buffer_mut(), &mut off);
    if area.height >= 3 {
        let row1 = Rect::new(area.x, area.y.saturating_add(2), area.width, 1);
        let _ = Switch::new("sync", "Background sync", system).paint(
            row1,
            frame.buffer_mut(),
            &mut on,
        );
    } else if area.height >= 2 {
        let row1 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        let _ = Switch::new("sync", "Background sync", system).paint(
            row1,
            frame.buffer_mut(),
            &mut on,
        );
    }
}

fn switch_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SwitchState::new(true);
    state.set_loading(true);
    state.set_focused(true);
    let _ = Switch::new("cloud", "Cloud sync", system)
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn switch_states_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows: [(&str, bool, bool, bool, bool, bool); 5] = [
        ("Off", false, true, false, false, false),
        ("On focused", true, true, false, false, true),
        ("Disabled", true, false, false, false, false),
        ("Read-only", true, true, true, false, false),
        ("Invalid", false, true, false, true, false),
    ];
    for (i, (label, on, enabled, ro, invalid, focused)) in rows.iter().enumerate() {
        let y = area.y.saturating_add(u16::try_from(i).unwrap_or(0));
        if y >= area.bottom() {
            break;
        }
        let mut st = SwitchState::new(*on);
        st.set_enabled(*enabled);
        st.set_read_only(*ro);
        st.set_invalid(*invalid);
        st.set_focused(*focused);
        let _ = Switch::new(*label, *label, system).paint(
            Rect::new(area.x, y, area.width, 1),
            frame.buffer_mut(),
            &mut st,
        );
    }
}

fn switch_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SwitchState::new(true);
    state.set_focused(true);
    let _ = Switch::new("wrap", "Soft wrap", system)
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn switch_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SwitchState::new(true);
    state.set_focused(true);
    let _ = Switch::new("dark", "ダークモード", system)
        .description("システムに従う")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn checkbox_states_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let rows: [(&str, CheckboxValue, bool, bool, bool); 5] = [
        ("Unchecked", CheckboxValue::Unchecked, true, false, false),
        ("Checked", CheckboxValue::Checked, true, false, true),
        (
            "Indeterminate",
            CheckboxValue::Indeterminate,
            true,
            false,
            false,
        ),
        ("Invalid", CheckboxValue::Unchecked, true, true, false),
        ("Read-only", CheckboxValue::Checked, false, false, false),
    ];
    for (i, (label, value, enabled, invalid, focused)) in rows.iter().enumerate() {
        let y = area.y.saturating_add(u16::try_from(i).unwrap_or(0));
        if y >= area.bottom() {
            break;
        }
        let mut state = CheckboxState::with_value(*value);
        state.set_enabled(*enabled);
        if !enabled {
            state.set_read_only(true);
        }
        state.set_invalid(*invalid);
        state.set_focused(*focused);
        let _ = Checkbox::new(*label, *label, system).paint(
            Rect::new(area.x, y, area.width, 1),
            frame.buffer_mut(),
            &mut state,
        );
    }
}

fn checkbox_indeterminate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let children = [true, false, true];
    let parent = CheckboxValue::from_children(children);
    let mut parent_state = CheckboxState::with_value(parent);
    parent_state.set_focused(true);
    let _ = Checkbox::new("all", "Select all", system)
        .description("Mixed children")
        .paint(
            Rect::new(area.x, area.y, area.width, 2.min(area.height)),
            frame.buffer_mut(),
            &mut parent_state,
        );
    let labels = ["Alpha", "Beta", "Gamma"];
    for (i, (label, on)) in labels.iter().zip(children).enumerate() {
        let y = area.y.saturating_add(2).saturating_add(u16::try_from(i).unwrap_or(0));
        if y >= area.bottom() {
            break;
        }
        let mut st = CheckboxState::new(on);
        let _ = Checkbox::new(*label, *label, system).paint(
            Rect::new(area.x.saturating_add(2), y, area.width.saturating_sub(2), 1),
            frame.buffer_mut(),
            &mut st,
        );
    }
}

fn checkbox_description_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = CheckboxState::new(false);
    state.set_focused(true);
    let _ = Checkbox::new("notify", "Email notifications", system)
        .description("Send a summary when long jobs complete")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn checkbox_list_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        ("docs", "Documentation", true),
        ("tests", "Tests", false),
        ("bench", "Benchmarks", true),
    ];
    for (i, (id, label, on)) in items.iter().enumerate() {
        let y = area.y.saturating_add(u16::try_from(i).unwrap_or(0));
        if y >= area.bottom() {
            break;
        }
        let mut st = CheckboxState::new(*on);
        if i == 1 {
            st.set_focused(true);
        }
        let _ = Checkbox::new(*id, *label, system).paint(
            Rect::new(area.x, y, area.width, 1),
            frame.buffer_mut(),
            &mut st,
        );
    }
}

fn radio_group_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("plan", "Plan").description("Read-only analysis"),
        RadioOption::new("build", "Build").description("Apply edits with approval"),
        RadioOption::new("ask", "Ask").description("Questions only"),
    ];
    let mut state = RadioState::new(Some("build"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .legend("Workbench mode")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn radio_group_horizontal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("low", "Low"),
        RadioOption::new("med", "Medium"),
        RadioOption::new("high", "High"),
    ];
    let mut state = RadioState::new(Some("med"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .legend("Risk")
        .horizontal()
        .stack_below(0)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn radio_group_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("a", "Available"),
        RadioOption::new("b", "Maintenance").enabled(false),
        RadioOption::new("c", "Alternative"),
    ];
    let mut state = RadioState::new(Some("a"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .legend("Endpoint")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn radio_group_badges_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("once", "Allow once"),
        RadioOption::new("session", "Allow for session").badge("recommended"),
        RadioOption::new("always", "Always allow").description("Persists across restarts"),
        RadioOption::new("deny", "Deny"),
    ];
    let mut state = RadioState::new(Some("session"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .legend("Permission")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn radio_group_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("l", "Low"),
        RadioOption::new("m", "Medium"),
        RadioOption::new("h", "High"),
    ];
    let mut state = RadioState::new(Some("m"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .horizontal()
        .stack_below(40)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn radio_group_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let options = [
        RadioOption::new("a", "計画").description("読み取り専用"),
        RadioOption::new("b", "構築").description("編集を適用 ✨"),
        RadioOption::new("c", "質問"),
    ];
    let mut state = RadioState::new(Some("b"));
    state.set_surface_focused(true);
    let _ = RadioGroup::new(&options, system)
        .legend("モード")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn data_table_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        termrock::widgets::DataColumn::new("id", "ID", termrock::widgets::DataColumnWidth::Min(4)),
        termrock::widgets::DataColumn::new(
            "name",
            "Name",
            termrock::widgets::DataColumnWidth::Min(8),
        ),
    ]);
    let cells0: &[&str] = &["1", "alpha"];
    let cells1: &[&str] = &["2", "beta"];
    let rows = [(1u64, cells0), (2u64, cells1)];
    let toolbar = DataTableToolbar {
        actions: &["Refresh", "Export"],
    };
    let mut state = DataTableState::<u64, &str>::new();
    DataTable::new(&tokens, &columns, &rows)
        .toolbar(&toolbar)
        .render(area, frame.buffer_mut(), &mut state);
}

fn data_table_project_window(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    logical: u64,
    offset: u64,
) {
    use termrock::widgets::{ColumnPin, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::Compact);
    let mut columns = termrock::widgets::ColumnModel::new(vec![
        termrock::widgets::DataColumn::new("id", "ID", DataColumnWidth::Fixed(8))
            .priority(100)
            .pin(ColumnPin::Start),
        termrock::widgets::DataColumn::new("name", "Name", DataColumnWidth::Min(12)).priority(80),
        termrock::widgets::DataColumn::new("meta", "Meta", DataColumnWidth::Min(8)).priority(20),
    ]);
    if area.width < 40 {
        columns.contract_to_budget(2, 90);
    }
    let mut state = DataTableState::<u64, &str>::new();
    state.set_logical_rows(logical);
    state.window.viewport = area.height.saturating_sub(3).max(1);
    state.window.offset = offset.min(state.window.max_offset());
    state.window.clamp();
    let (start, end) = state.window.visible_range();
    // Build projected slice only — never allocate `logical` rows.
    let owned: Vec<(String, String, String)> = (start..end)
        .map(|i| {
            (
                i.to_string(),
                format!("row-{i}"),
                if i % 3 == 0 { "hot" } else { "ok" }.into(),
            )
        })
        .collect();
    let cell_refs: Vec<[&str; 3]> = owned
        .iter()
        .map(|(a, b, c)| [a.as_str(), b.as_str(), c.as_str()])
        .collect();
    let rows: Vec<(u64, &[&str])> = cell_refs
        .iter()
        .enumerate()
        .map(|(i, cells)| (start + i as u64, cells.as_slice()))
        .collect();
    state.load = if logical > end {
        LoadState::Partial {
            resident: end,
            total: Some(logical),
        }
    } else {
        LoadState::Ready { count: logical }
    };
    DataTable::new(&tokens, &columns, &rows).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_rows_10(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    data_table_project_window(frame, area, system, 10, 0);
}

fn data_table_rows_10k(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    data_table_project_window(frame, area, system, 10_000, 250);
}

fn data_table_rows_1m(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    data_table_project_window(frame, area, system, 1_000_000, 500_000);
}

fn data_table_wide(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnPin, DataColumnWidth};
    let tokens = system.clone().density(Density::Compact);
    let mut cols = Vec::new();
    cols.push(
        termrock::widgets::DataColumn::new("id", "ID", DataColumnWidth::Fixed(4))
            .priority(100)
            .pin(ColumnPin::Start),
    );
    for i in 0..12 {
        cols.push(
            termrock::widgets::DataColumn::new(
                // static-ish ids via leak for story only
                Box::leak(format!("c{i}").into_boxed_str()) as &str,
                Box::leak(format!("C{i}").into_boxed_str()) as &str,
                DataColumnWidth::Min(6),
            )
            .priority(if i < 3 { 70 } else { 20 }),
        );
    }
    let columns = termrock::widgets::ColumnModel::new(cols);
    let cells: Vec<String> = (0..13).map(|i| i.to_string()).collect();
    let cell_refs: Vec<&str> = cells.iter().map(String::as_str).collect();
    let rows = [(1u64, cell_refs.as_slice())];
    let mut state = DataTableState::<u64, &str>::new();
    DataTable::new(&tokens, &columns, &rows).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_combining(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        termrock::widgets::DataColumn::new("id", "ID", termrock::widgets::DataColumnWidth::Min(4)),
        termrock::widgets::DataColumn::new(
            "name",
            "Name",
            termrock::widgets::DataColumnWidth::Min(12),
        ),
    ]);
    // e + combining acute, n + tilde style samples
    let cells0: &[&str] = &["1", "cafe\u{0301}"];
    let cells1: &[&str] = &["2", "n\u{0303}o"];
    let rows = [(1u64, cells0), (2u64, cells1)];
    let mut state = DataTableState::<u64, &str>::new();
    DataTable::new(&tokens, &columns, &rows).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_stream_partial(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    data_table_project_window(frame, area, system, 50_000, 100);
}

fn data_table_narrow_priority(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    data_table_project_window(frame, area, system, 20, 0);
}

fn data_table_loading(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![termrock::widgets::DataColumn::new(
        "id",
        "ID",
        termrock::widgets::DataColumnWidth::Min(4),
    )]);
    let rows: [(u64, &[&str]); 0] = [];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = termrock::widgets::LoadState::Loading {
        message: Some("Loading…".into()),
    };
    DataTable::new(&tokens, &columns, &rows).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![termrock::widgets::DataColumn::new(
        "id",
        "ID",
        termrock::widgets::DataColumnWidth::Min(4),
    )]);
    let rows: [(u64, &[&str]); 0] = [];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = termrock::widgets::LoadState::Error {
        message: "query failed".into(),
        retryable: true,
    };
    DataTable::new(&tokens, &columns, &rows).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_visidata(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ColumnPin, DataColumn, DataColumnWidth, DataTableNavMode, LoadState, SortSpec,
    };
    let tokens = system.clone().density(Density::Compact);
    let columns = termrock::widgets::ColumnModel::new(vec![
        DataColumn::new("pid", "PID", DataColumnWidth::Fixed(6))
            .priority(100)
            .pin(ColumnPin::Start)
            .sortable(),
        DataColumn::new("name", "NAME", DataColumnWidth::Min(14))
            .priority(90)
            .sortable()
            .editable(),
        DataColumn::new("cpu", "CPU%", DataColumnWidth::Fixed(6))
            .priority(70)
            .sortable(),
        DataColumn::new("mem", "MEM", DataColumnWidth::Min(8)).priority(40),
        DataColumn::new("status", "STAT", DataColumnWidth::Fixed(6)).priority(50),
    ]);
    let r0: &[&str] = &["101", "termrock", "42.1", "128M", "R"];
    let r1: &[&str] = &["208", "cargo", "11.0", "640M", "S"];
    let r2: &[&str] = &["317", "rustc", "88.4", "1.2G", "R"];
    let r3: &[&str] = &["422", "rg", "3.2", "48M", "S"];
    let r4: &[&str] = &["509", "vim", "0.4", "32M", "S"];
    let rows = [
        (101u64, r0),
        (208, r1),
        (317, r2),
        (422, r3),
        (509, r4),
    ];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = LoadState::Ready { count: 5 };
    state.nav_mode = DataTableNavMode::Cell;
    state.cursor_row = 2;
    state.cursor_col = 1;
    state.selection.toggle_row(101);
    state.selection.toggle_row(317);
    state.sort = Some(SortSpec {
        column: "cpu",
        ascending: false,
    });
    state.filter.query = "rust".into();
    DataTable::new(&tokens, &columns, &rows)
        .focused(true)
        .fullscreen_hint(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn data_table_range(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{CellCoord, DataColumn, DataColumnWidth, DataTableNavMode, LoadState};
    let tokens = system.clone().density(Density::Compact);
    let columns = termrock::widgets::ColumnModel::new(vec![
        DataColumn::new("a", "A", DataColumnWidth::Fixed(6)),
        DataColumn::new("b", "B", DataColumnWidth::Fixed(6)),
        DataColumn::new("c", "C", DataColumnWidth::Fixed(6)),
        DataColumn::new("d", "D", DataColumnWidth::Fixed(6)),
    ]);
    let r0: &[&str] = &["a0", "b0", "c0", "d0"];
    let r1: &[&str] = &["a1", "b1", "c1", "d1"];
    let r2: &[&str] = &["a2", "b2", "c2", "d2"];
    let rows = [(0u64, r0), (1, r1), (2, r2)];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = LoadState::Ready { count: 3 };
    state.set_nav_mode(DataTableNavMode::Range);
    state.selection.select_cell(CellCoord { row: 0, col: 1 });
    state.selection.extend_cell(CellCoord { row: 2, col: 2 });
    state.cursor_row = 2;
    state.cursor_col = 2;
    DataTable::new(&tokens, &columns, &rows)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn data_table_groups(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{DataColumn, DataColumnWidth, GroupHeader, LoadState};
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        DataColumn::new("name", "Name", DataColumnWidth::Min(12)),
        DataColumn::new("val", "Val", DataColumnWidth::Fixed(6)),
    ]);
    let g: &[&str] = &["group", ""];
    let r0: &[&str] = &["alpha", "10"];
    let r1: &[&str] = &["beta", "20"];
    let rows = [(900u64, g), (1, r0), (2, r1)];
    let groups = [GroupHeader {
        id: 900,
        label: "eu-west".into(),
        count: 2,
        expanded: true,
    }];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = LoadState::Ready { count: 2 };
    DataTable::new(&tokens, &columns, &rows)
        .groups(&groups)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn data_table_edit(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        DataColumn::new("id", "ID", DataColumnWidth::Fixed(4)),
        DataColumn::new("name", "Name", DataColumnWidth::Min(12)).editable(),
    ]);
    let r0: &[&str] = &["1", "alpha"];
    let rows = [(1u64, r0)];
    let mut state = DataTableState::<u64, &str>::new();
    state.load = LoadState::Ready { count: 1 };
    state.editing = true;
    state.edit_draft = "alpha-edited".into();
    state.cursor_col = 1;
    DataTable::new(&tokens, &columns, &rows)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn menu_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let items = [
        MenuItem::new("a", "Open"),
        MenuItem::new("b", "Disabled").enabled(false),
        MenuItem::new("c", "Save"),
    ];
    let mut state = MenuState::new();
    Menu::new(&items, &tokens).render(area, frame.buffer_mut(), &mut state);
}

fn example_dropdown_nodes() -> Vec<MenuNode<&'static str>> {
    vec![
        MenuNode::command("open", "Open")
            .shortcut("C-o")
            .mnemonic('O'),
        MenuNode::command("save", "Save").shortcut("C-s"),
        MenuNode::separator("sep"),
        MenuNode::checkbox("wrap", "Word wrap", true),
        MenuNode::submenu(
            "export",
            "Export",
            vec![
                MenuNode::command("pdf", "PDF"),
                MenuNode::submenu(
                    "image",
                    "Image",
                    vec![
                        MenuNode::command("png", "PNG"),
                        MenuNode::command("svg", "SVG"),
                    ],
                ),
            ],
        ),
        MenuNode::command("delete", "Delete").destructive(true),
    ]
}

fn dropdown_menu_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_dropdown_nodes();
    let mut state = DropdownMenuState::new();
    let _ = state.open_from_keyboard(&nodes, area);
    DropdownMenu::new(&nodes, system).paint(area, frame.buffer_mut(), &mut state);
}

fn dropdown_menu_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = example_dropdown_nodes();
    let mut state = DropdownMenuState::new();
    let _ = state.open_from_keyboard(&nodes, area);
    // Cursor on Export + open submenu + Image
    if let Some(i) = nodes.iter().position(|n| n.id == "export") {
        // set via repeated Down is fragile; use internal path via open then keys
        let _ = i;
    }
    // Walk: move to export (index 4 after open,save,sep,wrap)
    for _ in 0..4 {
        let _ = state.handle_key(
            termrock::input::KeyEvent::new(
                termrock::input::KeyCode::Down,
                termrock::input::KeyModifiers::NONE,
            ),
            &nodes,
        );
    }
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Right,
            termrock::input::KeyModifiers::NONE,
        ),
        &nodes,
    );
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Down,
            termrock::input::KeyModifiers::NONE,
        ),
        &nodes,
    );
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Right,
            termrock::input::KeyModifiers::NONE,
        ),
        &nodes,
    );
    DropdownMenu::new(&nodes, system).paint_cascade(area, area, frame.buffer_mut(), &mut state);
}

fn dropdown_menu_kinds_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = vec![
        MenuNode::section("lab", "Appearance"),
        MenuNode::checkbox("line", "Line numbers", true),
        MenuNode::radio("dark", "Dark", "theme", true),
        MenuNode::radio("light", "Light", "theme", false),
        MenuNode::separator("s1"),
        MenuNode::loading("load", "Syncing…"),
        MenuNode::custom_preview("prev", "Preview slot"),
        MenuNode::command("rm", "Remove")
            .destructive(true)
            .enabled(false)
            .disabled_reason("read-only"),
    ];
    let mut state = DropdownMenuState::new();
    let _ = state.open_from_keyboard(&nodes, area);
    DropdownMenu::new(&nodes, system).paint(area, frame.buffer_mut(), &mut state);
}

fn context_menu_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = vec![
        MenuNode::command("cut", "Cut").shortcut("C-x"),
        MenuNode::command("copy", "Copy").shortcut("C-c"),
        MenuNode::command("paste", "Paste").shortcut("C-v"),
        MenuNode::separator("s"),
        MenuNode::command("rename", "Rename"),
        MenuNode::command("delete", "Delete").destructive(true),
    ];
    let mut state = DropdownMenuState::context();
    let _ = state.open_from_context_pointer(&nodes, area);
    DropdownMenu::new(&nodes, system).paint(area, frame.buffer_mut(), &mut state);
}

fn context_menu_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let nodes = vec![
        MenuNode::command("open", "Open"),
        MenuNode::submenu(
            "git",
            "Git",
            vec![
                MenuNode::command("stage", "Stage"),
                MenuNode::command("commit", "Commit"),
                MenuNode::command("push", "Push").destructive(true),
            ],
        ),
        MenuNode::command("reveal", "Reveal in Finder"),
    ];
    let mut state = DropdownMenuState::context();
    let _ = state.open_from_context_key(&nodes, area);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Down,
            termrock::input::KeyModifiers::NONE,
        ),
        &nodes,
    );
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Right,
            termrock::input::KeyModifiers::NONE,
        ),
        &nodes,
    );
    DropdownMenu::new(&nodes, system).paint_cascade(area, area, frame.buffer_mut(), &mut state);
}

fn form_wizard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FormWizardState::with_steps([
        WizardStep::new("account", "Account"),
        WizardStep::new("region", "Region").optional(true),
        WizardStep::new("confirm", "Confirm"),
    ]);
    state.set_focused(true);
    FormWizard::new(system)
        .title("Connect")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn form_wizard_review_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FormWizardState::with_steps([
        WizardStep::new("account", "Account"),
        WizardStep::new("region", "Region"),
        WizardStep::new("confirm", "Confirm"),
    ]);
    state.set_focused(true);
    let _ = state.next();
    let _ = state.next();
    let _ = state.next(); // ReviewOpened
    FormWizard::new(system)
        .title("Connect")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn form_wizard_failure_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FormWizardState::with_steps([
        WizardStep::new("account", "Account"),
        WizardStep::new("region", "Region"),
    ]);
    state.set_focused(true);
    let _ = state.fail("Could not reach API — retry when ready");
    FormWizard::new(system)
        .title("Connect")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn form_wizard_resume_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FormWizardState::with_steps([
        WizardStep::new("account", "Account"),
        WizardStep::new("region", "Region").optional(true),
        WizardStep::new("confirm", "Confirm"),
    ]);
    state.set_focused(true);
    let _ = state.next();
    let snap = state.progress();
    let mut resumed = FormWizardState::with_steps([
        WizardStep::new("account", "Account"),
        WizardStep::new("region", "Region").optional(true),
        WizardStep::new("confirm", "Confirm"),
    ]);
    resumed.set_focused(true);
    let _ = resumed.restore_progress(&snap);
    FormWizard::new(system)
        .title("Resume setup")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut resumed);
}

fn tag_removable_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Tag, TagState};
    let chunks = Layout::horizontal([Constraint::Length(18), Constraint::Min(12)]).split(area);
    let tag = Tag::removable_tag("f1", "paste-body.txt", system);
    let mut st = TagState::new();
    st.set_focused(true);
    st.set_part(termrock::widgets::TokenPart::Remove);
    let _ = tag.paint(chunks[0], frame.buffer_mut(), &mut st);
    let tag2 = Tag::new("s", "static-entity", system);
    let mut st2 = TagState::new();
    let _ = tag2.paint(chunks[1], frame.buffer_mut(), &mut st2);
}

fn chip_filter_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Chip, ChipState};
    let chunks = Layout::horizontal([
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Min(12),
    ])
    .split(area);
    let mut a = ChipState::new(true);
    a.set_focused(true);
    let _ = Chip::new("rust", "rust", system).paint(chunks[0], frame.buffer_mut(), &mut a);
    let mut b = ChipState::new(false);
    let _ = Chip::new("go", "go", system).paint(chunks[1], frame.buffer_mut(), &mut b);
    let mut c = ChipState::new(false);
    let _ = Chip::new("ts", "typescript", system)
        .removable(true)
        .paint(chunks[2], frame.buffer_mut(), &mut c);
}

fn chip_status_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Chip, ChipState};
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    let mut e = ChipState::new(false);
    let _ = Chip::new("err", "invalid path", system)
        .error()
        .removable(true)
        .paint(chunks[0], frame.buffer_mut(), &mut e);
    let mut l = ChipState::new(false);
    let _ = Chip::new("load", "uploading…", system)
        .loading()
        .paint(chunks[1], frame.buffer_mut(), &mut l);
}

fn token_strip_wrap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{TokenItem, TokenStrip, TokenStripState};
    let items = [
        TokenItem::chip("a", "rust").selected(true),
        TokenItem::chip("b", "filters"),
        TokenItem::tag("c", "paste-1").removable(true),
        TokenItem::tag("d", "file.rs").removable(true),
        TokenItem::chip("e", "experimental"),
    ];
    let strip = TokenStrip::new(&items, system).wrap();
    let mut state = TokenStripState::new();
    state.set_surface_focused(true);
    state.set_cursor(Some("c"));
    strip.paint(area, frame.buffer_mut(), &mut state);
}

fn token_strip_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{TokenItem, TokenStrip, TokenStripState};
    let items = [
        TokenItem::chip("1", "one"),
        TokenItem::chip("2", "two"),
        TokenItem::chip("3", "three"),
        TokenItem::chip("4", "four"),
        TokenItem::chip("5", "five"),
    ];
    let strip = TokenStrip::new(&items, system).max_visible(3);
    let mut state = TokenStripState::new();
    strip.paint(area, frame.buffer_mut(), &mut state);
}

fn attachment_chip_file_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{AttachmentChip, AttachmentChipState, AttachmentItem};
    let item = AttachmentItem::file("f1", "main.rs")
        .bytes(4200)
        .line_count(128);
    let mut state = AttachmentChipState::new();
    state.set_focused(true);
    AttachmentChip::new(&item, system).paint(area, frame.buffer_mut(), &mut state);
}

fn attachment_chip_broken_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        AttachmentChip, AttachmentChipState, AttachmentItem, AttachmentStatus,
    };
    let item = AttachmentItem::file("f2", "missing/path.rs")
        .status(AttachmentStatus::Error)
        .validation("path not found");
    let mut state = AttachmentChipState::new();
    state.set_focused(true);
    AttachmentChip::new(&item, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn attachment_chip_upload_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        AttachmentChip, AttachmentChipState, AttachmentItem, AttachmentStatus,
    };
    let item = AttachmentItem::image("img1", "shot.png").status(AttachmentStatus::Uploading {
        progress: 67,
    });
    let mut state = AttachmentChipState::new();
    state.set_focused(true);
    AttachmentChip::new(&item, system).paint(area, frame.buffer_mut(), &mut state);
}

fn paste_chip_large_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{PasteChip, PasteChipState, PastePayload};
    let body = "fn main() {\n  println!(\"hello\");\n}\n".repeat(20);
    let paste = PastePayload::from_body("p1", body);
    let mut state = PasteChipState::new();
    state.set_focused(true);
    PasteChip::new(&paste, system).paint(area, frame.buffer_mut(), &mut state);
}

fn paste_chip_binary_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{PasteChip, PasteChipState, PastePayload};
    let paste = PastePayload::binary("bin1", 12_288);
    let mut state = PasteChipState::new();
    state.set_focused(true);
    PasteChip::new(&paste, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);
}

fn paste_chip_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{PasteChip, PasteChipState, PastePayload};
    let paste = PastePayload::from_body(
        "p2",
        "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\niota\n",
    );
    let mut state = PasteChipState::new();
    state.set_focused(true);
    state.expanded = true;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).split(area);
    PasteChip::new(&paste, system).paint(chunks[0], frame.buffer_mut(), &mut state);
    PasteChip::new(&paste, system).paint_expanded_preview(chunks[1], frame.buffer_mut());
}

fn attachment_strip_wrap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        paint_attachment_strip, AttachmentItem, AttachmentStatus, PastePayload, TokenStripLayout,
        TokenStripState,
    };
    let atts = [
        AttachmentItem::file("a", "lib.rs").bytes(900),
        AttachmentItem::url("b", "docs.rs/termrock"),
        AttachmentItem::code("c", "main::run").line_count(40),
        AttachmentItem::file("d", "secret.env")
            .sensitive(true)
            .status(AttachmentStatus::Indexing { progress: 20 }),
    ];
    let pastes = [
        PastePayload::preview_only("p1", "log dump…", 8192, 200),
        PastePayload::binary("p2", 4096),
    ];
    let mut state = TokenStripState::new();
    state.set_surface_focused(true);
    paint_attachment_strip(
        &atts,
        &pastes,
        area,
        frame.buffer_mut(),
        system,
        &mut state,
        TokenStripLayout::Wrap,
        0,
        true,
        &[],
    );
}

fn file_mention_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{FileMention, InlineMention, InlineMentionState};
    let a = FileMention::path("f1", "main.rs", "src/main.rs");
    let b = FileMention::symbol("s1", "run", "mod::run");
    let chunks = Layout::horizontal([Constraint::Length(20), Constraint::Min(16)]).split(area);
    let mut sa = InlineMentionState::new();
    sa.set_focused(true);
    let mut sb = InlineMentionState::new();
    InlineMention::file(&a, system).paint(chunks[0], frame.buffer_mut(), &mut sa);
    InlineMention::file(&b, system).ascii(true).paint(chunks[1], frame.buffer_mut(), &mut sb);
}

fn file_mention_missing_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{FileMention, InlineMention, InlineMentionState};
    let m = FileMention::missing("m1", "gone.rs", "old/gone.rs");
    let mut st = InlineMentionState::new();
    st.set_focused(true);
    InlineMention::file(&m, system).ascii(true).paint(area, frame.buffer_mut(), &mut st);
}

fn file_mention_ambiguous_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        FileMention, InlineMention, InlineMentionState, MentionDisambiguator,
    };
    let m = FileMention::ambiguous(
        "u1",
        "util.rs",
        vec![
            MentionDisambiguator::new("a/util.rs", "util.rs").detail("crate a"),
            MentionDisambiguator::new("b/util.rs", "util.rs").detail("crate b"),
            MentionDisambiguator::new("c/util.rs", "util.rs").detail("crate c"),
        ],
    );
    let mut st = InlineMentionState::new();
    st.set_focused(true);
    st.disambiguation_open = true;
    st.disambiguation_cursor = 1;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(4)]).split(area);
    InlineMention::file(&m, system).paint(chunks[0], frame.buffer_mut(), &mut st);
    InlineMention::file(&m, system).paint_disambiguation(chunks[1], frame.buffer_mut(), &st);
}

fn entity_mention_agent_tool_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{EntityMention, InlineMention, InlineMentionState};
    let agent = EntityMention::agent("a1", "planner", "agent:planner");
    let tool = EntityMention::tool("t1", "bash", "tool:bash");
    let chunks = Layout::horizontal([Constraint::Length(22), Constraint::Min(16)]).split(area);
    let mut sa = InlineMentionState::new();
    sa.set_focused(true);
    let mut st = InlineMentionState::new();
    InlineMention::entity(&agent, system).paint(chunks[0], frame.buffer_mut(), &mut sa);
    InlineMention::entity(&tool, system).ascii(true).paint(chunks[1], frame.buffer_mut(), &mut st);
}

fn entity_mention_stale_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{EntityMention, InlineMention, InlineMentionState, MentionValidity};
    let mut s = EntityMention::session("s1", "chat-42", "session:42");
    s.as_mut().validity = MentionValidity::Stale;
    let mut st = InlineMentionState::new();
    st.set_focused(true);
    InlineMention::entity(&s, system).paint(area, frame.buffer_mut(), &mut st);
}

fn mention_draft_atomic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        FileMention, InlineMention, InlineMentionState, MentionDraft, MentionSegment,
    };
    let mut d = MentionDraft::from_text("Review ");
    d.insert_mention(FileMention::path("f", "lib.rs", "src/lib.rs").mention);
    d.insert_text(" with ");
    d.insert_mention(FileMention::symbol("sy", "parse", "ast::parse").mention);
    // paint display string + first mention token
    let display = d.to_display_string();
    frame.buffer_mut().set_stringn(
        area.x,
        area.y,
        display.chars().take(usize::from(area.width)).collect::<String>(),
        usize::from(area.width),
        system.style(termrock::style::Role::Text),
    );
    if let Some(MentionSegment::Mention(m)) = d.parts.iter().find(|p| matches!(p, MentionSegment::Mention(_))) {
        let mut st = InlineMentionState::new();
        st.set_focused(true);
        let chip = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        InlineMention::new(m, system).paint(chip, frame.buffer_mut(), &mut st);
    }
}

fn slash_command_menu_filter_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_slash_catalog, SlashCommandMenu, SlashCommandMenuState,
    };
    let cat = example_slash_catalog();
    let mut st = SlashCommandMenuState::new();
    st.sync_from_draft("/p", 2);
    let anchor = Rect::new(area.x.saturating_add(2), area.y.saturating_add(1), 1, 1);
    SlashCommandMenu::new(&cat, system, area, anchor).paint(area, frame.buffer_mut(), &mut st);
}

fn slash_command_menu_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_slash_catalog, CompletionStatus, SlashCommandMenu, SlashCommandMenuState,
    };
    let cat = example_slash_catalog();
    let mut st = SlashCommandMenuState::new();
    st.sync_from_draft("/", 1);
    st.set_status(CompletionStatus::Loading);
    let _ = st.begin_async();
    let anchor = Rect::new(area.x, area.y, 1, 1);
    SlashCommandMenu::new(&cat, system, area, anchor).paint(area, frame.buffer_mut(), &mut st);
}

fn slash_command_menu_arguments_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_slash_catalog, SlashCommandMenu, SlashCommandMenuState, SlashMenuPhase, SlashQuery,
    };
    let cat = example_slash_catalog();
    let mut st = SlashCommandMenuState::new();
    st.open_with_query(SlashQuery {
        phase: SlashMenuPhase::Argument {
            command_id: "model".into(),
            command_name: "model".into(),
            arg_index: 0,
            arg_prefix: String::new(),
            prior_args: Vec::new(),
        },
        trigger_byte: 0,
        cursor_byte: 7,
    });
    let anchor = Rect::new(area.x, area.y, 1, 1);
    SlashCommandMenu::new(&cat, system, area, anchor).paint(area, frame.buffer_mut(), &mut st);
}

fn slash_command_menu_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_slash_catalog, SlashCommandMenu, SlashCommandMenuState,
    };
    let cat = example_slash_catalog();
    let mut st = SlashCommandMenuState::new();
    st.sync_from_draft("/he", 3);
    let anchor = Rect::new(area.x, area.y, 1, 1);
    SlashCommandMenu::new(&cat, system, area, anchor).paint(area, frame.buffer_mut(), &mut st);
}

fn slash_command_menu_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_slash_catalog, SlashCommandMenu, SlashCommandMenuState,
    };
    let cat = example_slash_catalog();
    let mut st = SlashCommandMenuState::new();
    st.sync_from_draft("/dep", 4);
    let anchor = Rect::new(area.x, area.y, 1, 1);
    SlashCommandMenu::new(&cat, system, area, anchor).paint(area, frame.buffer_mut(), &mut st);
}

fn model_selector_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_model_catalog, ModelSelector, ModelSelectorState};
    let cat = example_model_catalog();
    let mut st = ModelSelectorState::with_selected("smart");
    st.reasoning = termrock::widgets::ReasoningEffort::High;
    ModelSelector::new(&cat, system)
        .show_reasoning(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn model_selector_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_model_catalog, ModelSelector, ModelSelectorPresentation, ModelSelectorState,
    };
    let cat = example_model_catalog();
    let mut st = ModelSelectorState::with_selected("fast");
    st.presentation = ModelSelectorPresentation::Expanded;
    st.highlight = Some("smart".into());
    ModelSelector::new(&cat, system).paint(area, frame.buffer_mut(), &mut st);
}

fn model_selector_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_model_catalog, ModelSelector, ModelSelectorPresentation, ModelSelectorState,
    };
    let cat = example_model_catalog();
    let mut st = ModelSelectorState::new();
    st.presentation = ModelSelectorPresentation::Expanded;
    st.search = "zzzz-nope".into();
    ModelSelector::new(&cat, system).ascii(true).paint(area, frame.buffer_mut(), &mut st);
}

fn agent_mode_ribbon_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        default_agent_modes, AgentModePresentation, AgentModeSelector, AgentModeSelectorState,
    };
    let modes = default_agent_modes();
    let mut st = AgentModeSelectorState::with_selected("full-auto");
    st.presentation = AgentModePresentation::Ribbon;
    AgentModeSelector::new(&modes, system).paint(area, frame.buffer_mut(), &mut st);
}

fn agent_mode_menu_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        default_agent_modes, AgentModePresentation, AgentModeSelector, AgentModeSelectorState,
    };
    let modes = default_agent_modes();
    let mut st = AgentModeSelectorState::with_selected("edit");
    st.presentation = AgentModePresentation::Menu;
    st.highlight = Some("full-auto".into());
    AgentModeSelector::new(&modes, system).paint(area, frame.buffer_mut(), &mut st);
}

fn agent_mode_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        default_agent_modes, AgentModeSelector, AgentModeSelectorState, ExecutionPolicyKind,
    };
    let modes = default_agent_modes();
    let mut st = AgentModeSelectorState::with_selected("auto");
    st.policy = Some(ExecutionPolicyKind::Network);
    AgentModeSelector::new(&modes, system).ascii(true).paint(area, frame.buffer_mut(), &mut st);
}

fn composer_selectors_strip_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        default_agent_modes, example_model_catalog, AgentModeSelectorState, ComposerSelectors,
        ModelSelectorState,
    };
    let modes = default_agent_modes();
    let models = example_model_catalog();
    let ms = ModelSelectorState::with_selected("smart");
    let mut as_ = AgentModeSelectorState::with_selected("plan");
    as_.policy = Some(termrock::widgets::ExecutionPolicyKind::WorkspaceWrite);
    ComposerSelectors::new(&modes, &models, system).paint_compact(
        area,
        frame.buffer_mut(),
        &as_,
        &ms,
    );
}

fn message_thread_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_message_session, MessageThread, MessageThreadState};
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.set_focused(true);
    st.on_entries_len(entries.len());
    MessageThread::new(&entries, system).paint(area, frame.buffer_mut(), &mut st);
}

fn message_thread_follow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_message_session, MessageThread, MessageThreadState};
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.set_focused(true);
    st.transcript.set_follow(true);
    st.on_entries_len(entries.len());
    MessageThread::new(&entries, system).paint(area, frame.buffer_mut(), &mut st);
}

fn message_thread_unread_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_message_session, MessageThread, MessageThreadState};
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.set_focused(true);
    st.transcript.set_follow(false);
    st.on_entries_len(3);
    st.on_entries_len(entries.len());
    MessageThread::new(&entries, system).paint(area, frame.buffer_mut(), &mut st);
}

fn message_thread_compact_zoom_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_message_session, MessageThread, MessageThreadState, MessageZoom,
    };
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.zoom = MessageZoom::Compact;
    st.set_focused(true);
    MessageThread::new(&entries, system).paint(area, frame.buffer_mut(), &mut st);
}

fn message_thread_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_message_session, MessageThread, MessageThreadState};
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.set_focused(true);
    MessageThread::new(&entries, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn message_thread_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_message_session, MessageThread, MessageThreadState};
    let entries = example_message_session();
    let mut st = MessageThreadState::new();
    st.set_focused(true);
    MessageThread::new(&entries, system)
        .ascii(true)
        .colorless(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn streaming_markdown_mid_fence_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        streaming_markdown_fixtures, StreamingMarkdown, StreamingMarkdownState,
    };
    let mut st = StreamingMarkdownState::new();
    st.coalesce_deltas = 1;
    for c in streaming_markdown_fixtures::mid_fence_chunks() {
        st.push_delta(c);
        st.apply_pending();
    }
    StreamingMarkdown::new(system).paint(area, frame.buffer_mut(), &mut st);
}

fn streaming_markdown_complete_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        streaming_markdown_fixtures, StreamingMarkdown, StreamingMarkdownState,
    };
    let mut st = StreamingMarkdownState::new();
    st.coalesce_deltas = 1;
    for c in streaming_markdown_fixtures::mid_fence_chunks() {
        st.push_delta(c);
        st.apply_pending();
    }
    st.push_delta(streaming_markdown_fixtures::mid_fence_close());
    st.apply_pending();
    st.finish();
    StreamingMarkdown::new(system).paint(area, frame.buffer_mut(), &mut st);
}

fn streaming_markdown_failed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        streaming_markdown_fixtures, StreamingMarkdown, StreamingMarkdownState,
    };
    let mut st = StreamingMarkdownState::new();
    st.coalesce_deltas = 1;
    for c in streaming_markdown_fixtures::partial_table_chunks() {
        st.push_delta(c);
        st.apply_pending();
    }
    st.fail("stream interrupted");
    StreamingMarkdown::new(system).ascii(true).paint(area, frame.buffer_mut(), &mut st);
}

fn streaming_markdown_citations_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        StreamCitation, StreamInsertion, StreamingMarkdown, StreamingMarkdownState,
    };
    let mut st = StreamingMarkdownState::new();
    st.coalesce_deltas = 1;
    st.push_delta("Claim with support.\n\n");
    st.apply_pending();
    st.finish();
    st.add_citation(StreamCitation::new("1", "[1] RFC").href("https://example.com"));
    st.add_insertion(StreamInsertion::new("t", "tool", ["grep done"]));
    StreamingMarkdown::new(system).paint(area, frame.buffer_mut(), &mut st);
}

fn streaming_markdown_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StreamingMarkdown, StreamingMarkdownState};
    let mut st = StreamingMarkdownState::new();
    st.coalesce_deltas = 1;
    st.push_delta("## Narrow\n\nA longer paragraph that wraps on small widths.\n\n```\ncode\n```\n");
    st.apply_pending();
    st.finish();
    StreamingMarkdown::new(system).ascii(true).paint(area, frame.buffer_mut(), &mut st);
}

fn source_citation_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_citations, DestinationDisplay, SourceCitation, SourceCitationState,
    };
    let src = example_citations();
    let chunks = Layout::horizontal([
        Constraint::Length(14),
        Constraint::Length(22),
        Constraint::Min(12),
    ])
    .split(area);
    let mut a = SourceCitationState::new();
    a.focused = true;
    let mut b = SourceCitationState::new();
    let mut c = SourceCitationState::new();
    SourceCitation::new(&src[0], system).ascii(true).paint(chunks[0], frame.buffer_mut(), &mut a);
    SourceCitation::new(&src[1], system)
        .show_destination(DestinationDisplay::Always)
        .paint(chunks[1], frame.buffer_mut(), &mut b);
    SourceCitation::new(&src[3], system).paint(chunks[2], frame.buffer_mut(), &mut c);
}

fn source_citation_offline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_citations, SourceCitation, SourceCitationState};
    let src = example_citations();
    let mut st = SourceCitationState::new();
    st.focused = true;
    SourceCitation::new(&src[1], system)
        .offline(true)
        .no_hyperlink(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn citation_list_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_citations, CitationList, CitationListState};
    let src = example_citations();
    let mut st = CitationListState::new();
    st.expand();
    st.focused = true;
    st.cursor = 1;
    CitationList::new(&src, system)
        .title("Sources")
        .paint(area, frame.buffer_mut(), &mut st);
}

fn citation_list_collapsed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_citations, CitationList, CitationListState};
    let src = example_citations();
    let mut st = CitationListState::new();
    CitationList::new(&src, system)
        .title("Sources")
        .paint(area, frame.buffer_mut(), &mut st);
}

fn citation_list_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_citations, CitationList, CitationListState};
    let src = example_citations();
    let mut st = CitationListState::new();
    st.expand();
    st.no_hyperlink = true;
    CitationList::new(&src, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn tool_call_card_running_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_tool_calls, ToolCallCard, ToolCallCardState, ToolCallPresentation,
    };
    let calls = example_tool_calls();
    let call = calls
        .iter()
        .find(|c| c.id == "t2")
        .unwrap_or(&calls[1]);
    let mut st = ToolCallCardState::new();
    st.focused = true;
    st.presentation = ToolCallPresentation::Compact;
    ToolCallCard::new(call, system).paint(area, frame.buffer_mut(), &mut st);
}

fn tool_call_card_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_tool_calls, ToolCallCard, ToolCallCardState, ToolCallPresentation,
    };
    let calls = example_tool_calls();
    let call = calls
        .iter()
        .find(|c| c.id == "t4")
        .unwrap_or(&calls[3]);
    let mut st = ToolCallCardState::new();
    st.focused = true;
    st.presentation = ToolCallPresentation::Expanded;
    ToolCallCard::new(call, system).paint(area, frame.buffer_mut(), &mut st);
}

fn tool_call_card_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_tool_calls, ToolCallCard, ToolCallCardState, ToolCallPresentation,
    };
    let calls = example_tool_calls();
    let call = &calls[0];
    let mut st = ToolCallCardState::new();
    st.focused = true;
    st.presentation = ToolCallPresentation::Expanded;
    ToolCallCard::new(call, system).paint(area, frame.buffer_mut(), &mut st);
}

fn tool_call_card_permission_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_tool_calls, ToolCallCard, ToolCallCardState, ToolCallPresentation,
    };
    let calls = example_tool_calls();
    let call = calls
        .iter()
        .find(|c| c.id == "t3")
        .unwrap_or(&calls[2]);
    let mut st = ToolCallCardState::new();
    st.focused = true;
    st.presentation = ToolCallPresentation::Expanded;
    ToolCallCard::new(call, system).paint(area, frame.buffer_mut(), &mut st);
}

fn tool_call_card_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_tool_calls, ToolCallCard, ToolCallCardState, ToolCallPresentation,
    };
    let calls = example_tool_calls();
    let call = &calls[0];
    let mut st = ToolCallCardState::new();
    st.focused = true;
    st.presentation = ToolCallPresentation::Compact;
    ToolCallCard::new(call, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn terminal_run_card_running_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_terminal_run_lines, example_terminal_runs, TerminalRunCard,
        TerminalRunCardState, TerminalRunPresentation,
    };
    let runs = example_terminal_runs();
    let lines = example_terminal_run_lines();
    let run = runs.iter().find(|r| r.id == "r1").unwrap_or(&runs[0]);
    let mut st = TerminalRunCardState::new();
    st.focused = true;
    st.presentation = TerminalRunPresentation::Expanded;
    st.on_append(lines.len() as u16, 8);
    TerminalRunCard::new(run, &lines, system).paint(area, frame.buffer_mut(), &mut st);
}

fn terminal_run_card_permission_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_terminal_runs, TerminalRunCard, TerminalRunCardState, TerminalRunPresentation,
    };
    let runs = example_terminal_runs();
    let run = runs.iter().find(|r| r.id == "r2").unwrap_or(&runs[1]);
    let mut st = TerminalRunCardState::new();
    st.focused = true;
    st.presentation = TerminalRunPresentation::Expanded;
    TerminalRunCard::new(run, &[], system).paint(area, frame.buffer_mut(), &mut st);
}

fn terminal_run_card_edited_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_terminal_run_lines, example_terminal_runs, TerminalRunCard,
        TerminalRunCardState, TerminalRunPresentation,
    };
    let runs = example_terminal_runs();
    let lines = example_terminal_run_lines();
    let run = runs.iter().find(|r| r.id == "r3").unwrap_or(&runs[2]);
    let mut st = TerminalRunCardState::new();
    st.focused = true;
    st.presentation = TerminalRunPresentation::Expanded;
    TerminalRunCard::new(run, &lines, system).paint(area, frame.buffer_mut(), &mut st);
}

fn terminal_run_card_failed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_terminal_run_lines, example_terminal_runs, TerminalRunCard,
        TerminalRunCardState, TerminalRunPresentation,
    };
    let runs = example_terminal_runs();
    let lines = example_terminal_run_lines();
    let run = runs.iter().find(|r| r.id == "r4").unwrap_or(&runs[3]);
    let mut st = TerminalRunCardState::new();
    st.focused = true;
    st.presentation = TerminalRunPresentation::Expanded;
    TerminalRunCard::new(run, &lines, system).paint(area, frame.buffer_mut(), &mut st);
}

fn terminal_run_card_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_terminal_run_lines, example_terminal_runs, TerminalRunCard,
        TerminalRunCardState, TerminalRunPresentation,
    };
    let runs = example_terminal_runs();
    let lines = example_terminal_run_lines();
    let run = &runs[0];
    let mut st = TerminalRunCardState::new();
    st.focused = true;
    st.presentation = TerminalRunPresentation::Compact;
    TerminalRunCard::new(run, &lines, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn activity_shelf_statuses_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_activities, ActivityShelf, ActivityShelfPresentation, ActivityShelfState,
    };
    let items = example_activities();
    let mut st = ActivityShelfState::new();
    st.focused = true;
    st.force_presentation = Some(ActivityShelfPresentation::Chips);
    st.selected = 0;
    ActivityShelf::new(&items, system).paint(area, frame.buffer_mut(), &mut st);
}

fn activity_shelf_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_activities, ActivityItem, ActivityShelf, ActivityShelfPresentation,
        ActivityShelfState, SemanticStatus,
    };
    let mut items = example_activities();
    for i in 0..8 {
        items.push(
            ActivityItem::new(format!("extra{i}"), format!("job-{i}"))
                .status(SemanticStatus::Running),
        );
    }
    let mut st = ActivityShelfState::new();
    st.focused = true;
    st.force_presentation = Some(ActivityShelfPresentation::Chips);
    ActivityShelf::new(&items, system).paint(area, frame.buffer_mut(), &mut st);
}

fn activity_shelf_summary_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_activities, ActivityShelf, ActivityShelfPresentation, ActivityShelfState,
    };
    let items = example_activities();
    let mut st = ActivityShelfState::new();
    st.force_presentation = Some(ActivityShelfPresentation::Summary);
    ActivityShelf::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn activity_shelf_badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_activities, ActivityShelf, ActivityShelfPresentation, ActivityShelfState,
    };
    let items = example_activities();
    let mut st = ActivityShelfState::new();
    st.force_presentation = Some(ActivityShelfPresentation::Badge);
    ActivityShelf::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn activity_shelf_statusbar_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        activity_status_slot, example_activities, project_activities_for_status_bar, StatusBar,
        StatusBarState,
    };
    let items = example_activities();
    let proj = project_activities_for_status_bar(&items, true);
    let slot = activity_status_slot("activities", &proj, false);
    let right = [slot];
    let mut st = StatusBarState::<&str>::new();
    frame.render_stateful_widget(StatusBar::new(&[], &right, system), area, &mut st);
}

fn badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Badge;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Badge::new("meta", system)
        .neutral()
        .paint(chunks[0], frame.buffer_mut(), None);
    let _ = Badge::new("info", system)
        .info()
        .paint(chunks[1], frame.buffer_mut(), None);
    let _ = Badge::new("ok", system)
        .success()
        .paint(chunks[2], frame.buffer_mut(), None);
    let _ = Badge::new("warn", system)
        .warning()
        .paint(chunks[3], frame.buffer_mut(), None);
    let _ = Badge::new("fail", system)
        .destructive()
        .paint(chunks[4], frame.buffer_mut(), None);
    let _ = Badge::new("tag", system)
        .outline()
        .paint(chunks[5], frame.buffer_mut(), None);
    let _ = Badge::count(150, system).paint(chunks[6], frame.buffer_mut(), None);
}

fn badge_table_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Badge;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    // Simulated dense table trailing badges — no soft fill.
    for (i, (name, paint)) in [
        ("row-a", "active"),
        ("row-b", "idle"),
        ("row-c", "error"),
        ("row-d", "beta"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = chunks[i];
        frame.buffer_mut().set_stringn(
            row.x,
            row.y,
            name,
            12,
            system.style(Role::Text),
        );
        let badge_area = Rect {
            x: row.x.saturating_add(14),
            y: row.y,
            width: row.width.saturating_sub(14),
            height: 1,
        };
        let b = match paint {
            "active" => Badge::new(paint, system).success(),
            "error" => Badge::new(paint, system).destructive(),
            "beta" => Badge::new(paint, system).outline(),
            _ => Badge::new(paint, system).neutral(),
        };
        let _ = b.paint(badge_area, frame.buffer_mut(), None);
    }
}

fn badge_task_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Badge;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Badge::new("running", system)
        .info()
        .paint(chunks[0], frame.buffer_mut(), None);
    let _ = Badge::new("done", system)
        .success()
        .paint(chunks[1], frame.buffer_mut(), None);
    let _ = Badge::new("blocked", system)
        .warning()
        .paint(chunks[2], frame.buffer_mut(), None);
    let _ = Badge::new("failed", system)
        .destructive()
        .paint(chunks[3], frame.buffer_mut(), None);
}

fn badge_settings_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{Badge, BadgeState};
    let chunks = Layout::horizontal([
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Min(10),
    ])
    .split(area);
    let _ = Badge::new("experimental", system)
        .outline()
        .paint(chunks[0], frame.buffer_mut(), None);
    let mut state = BadgeState::new();
    state.set_focused(true);
    state.set_selected(true);
    let _ = Badge::new("filter", system)
        .interactive(true)
        .neutral()
        .paint(chunks[1], frame.buffer_mut(), Some(&mut state));
    let _ = Badge::new("default", system)
        .neutral()
        .paint(chunks[2], frame.buffer_mut(), None);
}

fn badge_count_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Badge;
    let chunks = Layout::horizontal([
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(12),
    ])
    .split(area);
    let _ = Badge::count(3, system).paint(chunks[0], frame.buffer_mut(), None);
    let _ = Badge::count(99, system).paint(chunks[1], frame.buffer_mut(), None);
    let _ = Badge::new("msgs", system)
        .with_count(120)
        .info()
        .paint(chunks[2], frame.buffer_mut(), None);
}

fn callout_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Callout::new("Heads up", &tokens)
            .body("Non-color risk glyph present.")
            .tone(CalloutTone::Warning),
        area,
        frame.buffer_mut(),
    );
}

fn callout_tones_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .split(area);
    let tones = [
        (CalloutTone::Info, "Info"),
        (CalloutTone::Success, "Success"),
        (CalloutTone::Warning, "Warning"),
        (CalloutTone::Danger, "Danger"),
        (CalloutTone::Destructive, "Destructive"),
        (CalloutTone::Neutral, "Neutral"),
    ];
    for (i, (tone, title)) in tones.into_iter().enumerate() {
        if i >= chunks.len() {
            break;
        }
        let _ = Callout::new(title, system)
            .tone(tone)
            .body("status readable without color")
            .paint(chunks[i], frame.buffer_mut());
    }
}

fn callout_section_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Callout::new("Diagnostics", system)
        .tone(CalloutTone::Info)
        .section()
        .body("Compose with forms and empty states.")
        .details("expanded detail line")
        .show_details(true)
        .source("termrock · callout")
        .paint(area, frame.buffer_mut());
}

fn alert_danger_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertState::<()>::new();
    state.set_focused(true);
    Alert::new("Deploy failed", system)
        .tone(AlertTone::Danger)
        .body("Rollout aborted at step 3.")
        .details("timeout waiting for health check")
        .source("pipeline #42")
        .banner()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn alert_banner_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [Action {
        id: "retry",
        label: "Retry",
        enabled: true,
        style: None,
    }];
    let mut state = AlertState::new();
    state.set_focused(true);
    state.set_action_cursor(Some("retry"));
    Alert::new("Write conflict", system)
        .tone(AlertTone::Warning)
        .body("Remote changed while editing.")
        .source("git status")
        .actions(&actions)
        .banner()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn alert_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = AlertState::<()>::new();
    Alert::new("Saved", system)
        .tone(AlertTone::Success)
        .body("checkpoint written")
        .compact()
        .paint(area, frame.buffer_mut(), &mut state);
}

fn drawer_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::DrawerState::new();
    state.open();
    state.set_header_rows(1);
    state.set_footer_rows(1);
    Drawer::new("Inspector", system)
        .footer(Some("esc · [ ] resize"))
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.body_area();
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "filters · details",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
}

fn drawer_left_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::DrawerState::new();
    state.set_edge(termrock::widgets::DrawerEdge::Left);
    state.open();
    Drawer::new("Nav", system).paint(area, frame.buffer_mut(), &mut state);
}

fn drawer_sheet_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::DrawerState::sheet();
    state.open();
    state.set_header_rows(1);
    termrock::widgets::Sheet::new("Sheet", system)
        .footer(Some("drag handle"))
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.body_area();
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "bottom sheet content",
            usize::from(body.width),
            system.style(termrock::style::Role::TextMuted),
        );
    }
}

fn drawer_non_modal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::DrawerState::non_modal();
    state.open();
    Drawer::new("Task rail", system)
        .footer(Some("non-modal"))
        .paint(area, frame.buffer_mut(), &mut state);
}

fn fv_source(id: &'static str, path: &[&str]) -> SourceContext<&'static str> {
    SourceContext::new(id)
        .selection(Some(id))
        .scroll(ScrollAnchor::at(12, 0).with_id("anchor"))
        .focus_token("list")
        .path(path.iter().copied())
}

fn paint_viewer_body(frame: &mut Frame<'_>, body: Rect, system: &DesignSystem, line: &str) {
    if body.is_empty() {
        return;
    }
    frame.buffer_mut().set_stringn(
        body.x,
        body.y,
        line,
        usize::from(body.width),
        system.style(Role::Text),
    );
    if body.height > 1 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y + 1,
            "… host content (no app-state copy)",
            usize::from(body.width),
            system.style(Role::TextMuted),
        );
    }
}

fn fullscreen_viewer_code_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [
        Action {
            id: "copy",
            label: "Copy",
            enabled: true,
            style: None,
        },
        Action {
            id: "raw",
            label: "Raw",
            enabled: true,
            style: None,
        },
    ];
    let mut state = FullscreenViewerState::new();
    state.zoom_mut().set_content_kind(ViewerContentKind::Code);
    let _ = state.enter_fullscreen(
        fv_source("main.rs", &["repo", "src", "main.rs"]),
        "main.rs",
    );
    FullscreenViewer::new(system, &actions).paint(area, frame.buffer_mut(), &mut state);
    paint_viewer_body(frame, state.body_area(), system, "fn main() { /* … */ }");
}

fn fullscreen_viewer_diff_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [Action {
        id: "stage",
        label: "Stage",
        enabled: true,
        style: None,
    }];
    let mut state = FullscreenViewerState::new();
    state.zoom_mut().set_content_kind(ViewerContentKind::Diff);
    let _ = state.enter_fullscreen(fv_source("hunk-1", &["diff", "a.rs"]), "a.rs");
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('/'),
            termrock::input::KeyModifiers::NONE,
        ),
        &actions,
    );
    FullscreenViewer::new(system, &actions).paint(area, frame.buffer_mut(), &mut state);
    paint_viewer_body(frame, state.body_area(), system, "+added line  ·  -removed");
}

fn fullscreen_viewer_log_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions: [Action<'_, &str>; 0] = [];
    let mut state = FullscreenViewerState::new();
    state.zoom_mut().set_content_kind(ViewerContentKind::Log);
    let _ = state.enter_fullscreen(fv_source("job-9", &["runs", "job-9"]), "job-9.log");
    let _ = state.handle_intent(termrock::interaction::UiIntent::Help, &actions);
    FullscreenViewer::new(system, &actions).paint(area, frame.buffer_mut(), &mut state);
    paint_viewer_body(frame, state.body_area(), system, "INFO ready · WARN retry");
}

fn semantic_zoom_badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);
    let mut z = SemanticZoomState::<&str>::new();
    SemanticZoomBadge::new(system).paint(chunks[0], frame.buffer_mut(), &z);
    let _ = z.promote(fv_source("row", &["list"]));
    SemanticZoomBadge::new(system).paint(chunks[1], frame.buffer_mut(), &z);
    let _ = z.enter_fullscreen(fv_source("row", &["list"]));
    SemanticZoomBadge::new(system).paint(chunks[2], frame.buffer_mut(), &z);
    let _ = z.level(); // Fullscreen
}

fn fullscreen_viewer_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions: [Action<'_, &str>; 0] = [];
    let mut state = FullscreenViewerState::new();
    state.zoom_mut().set_content_kind(ViewerContentKind::Object);
    let _ = state.enter_fullscreen(fv_source("obj", &["objs"]), "object");
    FullscreenViewer::new(system, &actions)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
    paint_viewer_body(frame, state.body_area(), system, "{ id: 1 }");
}

fn fullscreen_viewer_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [Action {
        id: "open",
        label: "開く",
        enabled: true,
        style: None,
    }];
    let mut state = FullscreenViewerState::new();
    state.zoom_mut().set_content_kind(ViewerContentKind::Media);
    let _ = state.enter_fullscreen(
        fv_source("画像", &["資料", "プレビュー", "画像.png"]),
        "画像.png",
    );
    FullscreenViewer::new(system, &actions).paint(area, frame.buffer_mut(), &mut state);
    paint_viewer_body(frame, state.body_area(), system, "メディア · 🖼");
}

fn preview_card_file_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (content, _, _) = example_file_preview();
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_command_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (content, _, _) = example_command_preview();
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_symbol_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (content, _, _) = example_symbol_preview();
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_session_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (content, _, _) = example_session_preview();
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let content = PreviewCardContent::title("main.rs", PreviewResourceKind::File)
        .subtitle("src/main.rs")
        .load(PreviewLoadState::Loading)
        .essential_elsewhere(true);
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let content = PreviewCardContent::title("main.rs", PreviewResourceKind::File)
        .error("preview timed out")
        .essential_elsewhere(true);
    let mut state = PreviewCardState::with_delay(std::time::Duration::ZERO);
    let _ = state.tick_hover(1, true);
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn preview_card_pinned_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let (content, _, _) = example_file_preview();
    let mut state = PreviewCardState::new();
    let _ = state.pin();
    PreviewCard::new(content, system).paint(area, frame.buffer_mut(), &mut state);
}

fn text_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Text;
    let _ = Text::new("Semantic body text through DesignSystem roles.", system)
        .copyable()
        .paint(area, frame.buffer_mut());
}

fn text_spans_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::Role;
    use termrock::widgets::{Text, TextSpan};
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);
    let _ = Text::spans(
        [
            TextSpan::new("Status: ").role(Role::TextMuted),
            TextSpan::new("READY").role(Role::Success).strong(),
            TextSpan::new(" · ").dim(),
            TextSpan::new("cached")
                .role(Role::TextMuted)
                .annotation("meta"),
        ],
        system,
    )
    .paint(chunks[0], frame.buffer_mut());
    let _ = Text::spans(
        [
            TextSpan::new("Search hit: "),
            TextSpan::new("termrock")
                .highlight(true)
                .annotation("search"),
        ],
        system,
    )
    .paint(chunks[1], frame.buffer_mut());
    let _ = Text::spans(
        [
            TextSpan::new("code").code(),
            TextSpan::new(" and "),
            TextSpan::new("emphasis").italic(),
        ],
        system,
    )
    .paint(chunks[2], frame.buffer_mut());
}

fn text_wrap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Text;
    let _ = Text::new(
        "Soft-wrapped prose uses grapheme-safe display columns so CJK and emoji never split mid-cell.",
        system,
    )
    .wrap()
    .paint(area, frame.buffer_mut());
}

fn text_truncate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Text;
    let _ = Text::new(
        "This very long line is truncated with an ellipsis at the end",
        system,
    )
    .truncate()
    .paint(area, frame.buffer_mut());
}

fn text_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Text;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let _ = Text::new("日本語 e\u{301} 🧪", system).paint(chunks[0], frame.buffer_mut());
    let _ = Text::new("tabs:\tone\ttwo", system).paint(chunks[1], frame.buffer_mut());
    let _ = Text::new("controls stripped:\u{1b}[0mok", system).paint(chunks[2], frame.buffer_mut());
}

fn text_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Text;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
    let _ = Text::new("Centered label", system)
        .center()
        .truncate()
        .paint(chunks[0], frame.buffer_mut());
    let _ = Text::new("wraps when height allows multi-line body", system)
        .wrap()
        .paint(chunks[1], frame.buffer_mut());
}

fn heading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Heading::new("Section title", system)
        .h1()
        .reading()
        .paint(area, frame.buffer_mut());
}

fn heading_levels_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Heading::new("Document", system)
        .h1()
        .reading()
        .paint(chunks[0], frame.buffer_mut());
    let _ = Heading::new("Chapter", system)
        .h2()
        .reading()
        .paint(chunks[1], frame.buffer_mut());
    let _ = Heading::new("Detail", system)
        .h3()
        .paint(chunks[2], frame.buffer_mut());
}

fn heading_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::GlyphSet;
    let ascii = system.clone().glyphs(GlyphSet::Ascii);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Heading::new("Title", &ascii)
        .h1()
        .compact()
        .paint(chunks[0], frame.buffer_mut());
    let _ = Heading::new("Section", &ascii)
        .h2()
        .compact()
        .paint(chunks[1], frame.buffer_mut());
    let _ = Heading::new("Note", &ascii)
        .h3()
        .compact()
        .paint(chunks[2], frame.buffer_mut());
}

fn icon_browser_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::{Glyph, GlyphGroup, Role};
    use termrock::widgets::Icon;
    let mut y = area.y;
    for group in GlyphGroup::ALL {
        if y >= area.bottom() {
            break;
        }
        frame.buffer_mut().set_stringn(
            area.x,
            y,
            group.id(),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
        let mut x = area.x;
        for g in Glyph::in_group(group) {
            if y >= area.bottom() {
                break;
            }
            let w = 4u16;
            if x.saturating_add(w) > area.right() {
                x = area.x;
                y = y.saturating_add(1);
                if y >= area.bottom() {
                    break;
                }
            }
            let _ = Icon::new(g, system).min_width(w).paint(
                Rect {
                    x,
                    y,
                    width: w,
                    height: 1,
                },
                frame.buffer_mut(),
            );
            x = x.saturating_add(w);
        }
        y = y.saturating_add(1);
    }
}

fn icon_ascii_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::{Glyph, GlyphSet, Role};
    use termrock::widgets::Icon;
    let ascii = system.clone().glyphs(GlyphSet::Ascii);
    let samples = [
        Glyph::DisclosureClosed,
        Glyph::DisclosureOpen,
        Glyph::CheckOn,
        Glyph::CheckOff,
        Glyph::Success,
        Glyph::Error,
        Glyph::Ellipsis,
        Glyph::Bullet,
    ];
    let mut y = area.y;
    for g in samples {
        if y >= area.bottom() {
            break;
        }
        let r = g.resolve(GlyphSet::Ascii);
        let line = format!("{} {} — {}", r.text, g.id(), g.meaning());
        frame.buffer_mut().set_stringn(
            area.x,
            y,
            &line,
            usize::from(area.width),
            ascii.style(Role::Text),
        );
        let _ = Icon::new(g, &ascii);
        y = y.saturating_add(1);
    }
}

fn icon_enhanced_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::{Glyph, GlyphSet};
    use termrock::widgets::Icon;
    let enhanced = system.clone().glyphs(GlyphSet::Enhanced);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Icon::new(Glyph::Folder, &enhanced)
        .label("folder")
        .paint(chunks[0], frame.buffer_mut());
    let _ = Icon::new(Glyph::File, &enhanced)
        .label("file")
        .paint(chunks[1], frame.buffer_mut());
    let _ = Icon::new(Glyph::Search, &enhanced)
        .label("search")
        .paint(chunks[2], frame.buffer_mut());
    let _ = Icon::new(Glyph::Warning, &enhanced)
        .role(termrock::style::Role::Warning)
        .label("warning")
        .paint(chunks[3], frame.buffer_mut());
}

fn avatar_glyph_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([Constraint::Length(1); 3]).split(area);
    let _ = AvatarGlyph::new("Ada Lovelace", system)
        .size(AvatarSize::Normal)
        .paint(chunks[0], frame.buffer_mut());
    let _ = AvatarGlyph::new("termrock", system)
        .role(IdentityRole::Agent)
        .size(AvatarSize::Normal)
        .paint(chunks[1], frame.buffer_mut());
    let _ = AvatarGlyph::new("svc", system)
        .role(IdentityRole::Service)
        .role_glyph()
        .size(AvatarSize::Normal)
        .paint(chunks[2], frame.buffer_mut());
}

fn avatar_glyph_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::horizontal([Constraint::Length(2); 6]).split(area);
    for (i, seed) in ["A", "Bo", "Cy", "D", "Ev", "Fx"].iter().enumerate() {
        if let Some(c) = chunks.get(i) {
            let _ = AvatarGlyph::new(seed, system)
                .compact()
                .paint(*c, frame.buffer_mut());
        }
    }
}

fn avatar_glyph_presence_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([Constraint::Length(1); 3]).split(area);
    let _ = AvatarGlyph::new("online", system)
        .with_presence(PresenceStatus::Online)
        .paint(chunks[0], frame.buffer_mut());
    let _ = AvatarGlyph::new("busy", system)
        .with_presence(PresenceStatus::Busy)
        .paint(chunks[1], frame.buffer_mut());
    let _ = AvatarGlyph::new("away", system)
        .with_presence(PresenceStatus::Away)
        .paint(chunks[2], frame.buffer_mut());
}

fn avatar_glyph_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let system = system.clone().no_color();
    let _ = AvatarGlyph::new("Ada", &system)
        .size(AvatarSize::Normal)
        .presence(PresenceStatus::Online)
        .paint(area, frame.buffer_mut());
}

fn avatar_glyph_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = AvatarGlyph::new("文档 用户", system)
        .size(AvatarSize::Normal)
        .paint(area, frame.buffer_mut());
}

fn highlighted_text_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{MatchRanges, substring_ranges};
    let lines = [
        "src/widgets/command_palette.rs",
        "crates/termrock/src/widgets/picker.rs",
        "docs/design/component-prompt-library.md",
    ];
    let chunks = Layout::vertical([Constraint::Length(1); 3]).split(area);
    for (i, src) in lines.iter().enumerate() {
        let ranges = substring_ranges(src, "pal");
        if ranges.is_empty() {
            let ranges = substring_ranges(src, "widget");
            let _ = HighlightedText::prepared(src, ranges.as_slice(), system)
                .truncate(MatchTruncate::KeepFirstMatch)
                .paint(chunks[i], frame.buffer_mut());
        } else {
            let _ = HighlightedText::prepared(src, ranges.as_slice(), system)
                .truncate(MatchTruncate::KeepFirstMatch)
                .paint(chunks[i], frame.buffer_mut());
        }
        let _ = MatchRanges::new();
    }
}

fn highlighted_text_selected_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{MatchKind, MatchRange, substring_ranges};
    let src = "Open Command Palette";
    let mut ranges = substring_ranges(src, "Pal");
    // Mark first range focused if present
    let slice = ranges.as_slice();
    let owned: Vec<MatchRange> = if let Some(r) = slice.first() {
        let mut v = vec![MatchRange::focused(r.start, r.end)];
        v.extend(slice.iter().skip(1).copied());
        v
    } else {
        slice.to_vec()
    };
    let prep = MatchRanges::from_ranges(owned).prepare(src);
    let _ = HighlightedText::prepared(src, prep.as_slice(), system)
        .selected()
        .paint(area, frame.buffer_mut());
    let _ = MatchKind::Match;
}

fn highlighted_text_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::substring_ranges;
    let system = system.clone().no_color();
    let src = "fuzzy_find_path";
    let ranges = substring_ranges(src, "find");
    let _ = HighlightedText::prepared(src, ranges.as_slice(), &system)
        .paint(area, frame.buffer_mut());
}

fn highlighted_text_overlap_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{MatchKind, MatchRange, MatchRanges};
    let src = "abcdefghi";
    let prep = MatchRanges::from_ranges([
        MatchRange::soft(0, 6),
        MatchRange::new(2, 8),
        MatchRange::focused(4, 5),
    ])
    .prepare(src);
    let _ = HighlightedText::prepared(src, prep.as_slice(), system)
        .paint(area, frame.buffer_mut());
    let _ = MatchKind::Soft;
}

fn highlighted_text_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::substring_ranges;
    let src = "文档/组件/palette.md";
    let ranges = substring_ranges(src, "palette");
    let _ = HighlightedText::prepared(src, ranges.as_slice(), system)
        .truncate(MatchTruncate::KeepFirstMatch)
        .paint(area, frame.buffer_mut());
}

fn identity_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Identity::new("Ada Lovelace", system)
        .role(IdentityRole::User)
        .secondary("@ada")
        .badge(true)
        .presence(PresenceStatus::Online)
        .paint(area, frame.buffer_mut());
}

fn identity_thread_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let chunks = Layout::vertical([Constraint::Length(1); 4]).split(area);
    let _ = Identity::new("you", system)
        .role(IdentityRole::User)
        .paint(chunks[0], frame.buffer_mut());
    let _ = Identity::new("termrock-agent", system)
        .role(IdentityRole::Agent)
        .secondary("planning")
        .badge(true)
        .presence(PresenceStatus::Busy)
        .paint(chunks[1], frame.buffer_mut());
    let _ = Identity::new("build-svc", system)
        .role(IdentityRole::Service)
        .face(AvatarFace::RoleGlyph)
        .paint(chunks[2], frame.buffer_mut());
    let _ = Identity::new("collab", system)
        .role(IdentityRole::Collaborator)
        .compact()
        .paint(chunks[3], frame.buffer_mut());
}

fn identity_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Identity::new("Agent", system)
        .role(IdentityRole::Agent)
        .compact()
        .paint(area, frame.buffer_mut());
}

fn identity_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Identity::new("ドキュメント", system)
        .secondary("日本語")
        .role(IdentityRole::User)
        .paint(area, frame.buffer_mut());
}

fn icon_labeled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::style::{Glyph, Role};
    use termrock::widgets::Icon;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Icon::new(Glyph::Success, system)
        .role(Role::Success)
        .label("ready")
        .paint(chunks[0], frame.buffer_mut());
    let _ = Icon::new(Glyph::Error, system)
        .role(Role::Danger)
        .label("failed")
        .paint(chunks[1], frame.buffer_mut());
    let _ = Icon::new(Glyph::Play, system)
        .role(Role::Accent)
        .label("run")
        .paint(chunks[2], frame.buffer_mut());
    let _ = Icon::new(Glyph::Settings, system)
        .label("settings")
        .paint(chunks[3], frame.buffer_mut());
}

fn label_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::FieldCaption;
    let _ = FieldCaption::<&str>::new("Display name", system)
        .for_id("name")
        .required()
        .help("Shown in the session list")
        .paint(area, frame.buffer_mut());
}

fn label_states_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{FieldCaption, Label};
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Label::<()>::new("Required", system)
        .required()
        .paint(chunks[0], frame.buffer_mut());
    let _ = Label::<()>::new("Optional", system)
        .optional()
        .paint(chunks[1], frame.buffer_mut());
    let _ = Label::<()>::new("Disabled", system)
        .disabled()
        .paint(chunks[2], frame.buffer_mut());
    let _ = FieldCaption::<()>::new("Invalid", system)
        .error("must be unique")
        .paint(chunks[3], frame.buffer_mut());
    let _ = Label::<()>::new("Warning", system)
        .warning()
        .paint(chunks[4], frame.buffer_mut());
}

fn label_layouts_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::FieldCaption;
    let chunks = Layout::vertical([Constraint::Length(2), Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let _ = FieldCaption::<()>::new("Stacked", system)
        .help("description under label")
        .stacked()
        .paint(chunks[0], frame.buffer_mut());
    let _ = FieldCaption::<()>::new("Compact", system)
        .help("dropped in compact")
        .compact()
        .paint(chunks[1], frame.buffer_mut());
    let _ = FieldCaption::<()>::new("Inline", system)
        .required()
        .inline()
        .paint(chunks[2], frame.buffer_mut());
}

fn label_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::FieldCaption;
    // width 20 < DROP_DESCRIPTION_WIDTH (28) → description contracts
    let _ = FieldCaption::<()>::new("Endpoint", system)
        .required()
        .help("this help should contract away")
        .paint(area, frame.buffer_mut());
}

fn description_kinds_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Description;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let _ = Description::<()>::new("Help: use a stable hostname", system)
        .paint(chunks[0], frame.buffer_mut());
    let _ = Description::<()>::error("Error: value is required", system)
        .paint(chunks[1], frame.buffer_mut());
    let _ = Description::<()>::warning("Warning: deprecated flag", system)
        .paint(chunks[2], frame.buffer_mut());
    let _ = Description::<()>::meta("Meta: last synced 2m ago", system)
        .paint(chunks[3], frame.buffer_mut());
}

fn kbd_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Kbd;
    let _ = Kbd::new("C-k", system).keycap().paint(area, frame.buffer_mut());
}

fn key_value_list_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = [
        KvEntry::group_header("id", "Identity"),
        KvEntry::pair("name", "Name", "termrock")
            .copyable()
            .annotation("crate"),
        KvEntry::pair("status", "Status", "active").status(KvStatus::Success),
        KvEntry::pair("token", "Token", "super-secret")
            .secret()
            .copyable(),
        KvEntry::pair("docs", "Docs", "handbook")
            .href("https://example.invalid")
            .annotation("external"),
        KvEntry::group_header("build", "Build").depth(0),
        KvEntry::pair("target", "Target", "aarch64-apple-darwin").depth(1),
    ];
    let mut state = KeyValueListState::new();
    state.set_focused(true);
    state.cursor = Some("status");
    let _ = KeyValueList::reading(&entries, system).paint(area, frame.buffer_mut(), &mut state);
}

fn key_value_list_dense_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = [
        KvEntry::pair("host", "Host", "localhost"),
        KvEntry::pair("port", "Port", "8080").status(KvStatus::Info),
        KvEntry::pair("tls", "TLS", "required").status(KvStatus::Warning),
        KvEntry::pair("pid", "PID", "4242").copyable(),
    ];
    let mut state = KeyValueListState::new();
    let _ = KeyValueList::dense(&entries, system).paint(area, frame.buffer_mut(), &mut state);
}

fn key_value_list_stacked_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = [
        KvEntry::pair("path", "Path", "/Users/example/Projects/termrock")
            .annotation("workspace root"),
        KvEntry::pair("branch", "Branch", "feat/terminal-design-system"),
    ];
    let mut state = KeyValueListState::new();
    let _ = KeyValueList::reading(&entries, system)
        .layout(KvLayout::Stacked)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn key_value_list_secret_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = [KvEntry::pair("api", "API key", "sk-live-very-secret-value")
        .secret()
        .copyable()];
    let mut state = KeyValueListState::new();
    state.set_focused(true);
    state.cursor = Some("api");
    let _ = KeyValueList::dense(&entries, system).paint(area, frame.buffer_mut(), &mut state);
}

fn key_value_list_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let entries = [
        KvEntry::pair("name", "名称", "文档 🔗"),
        KvEntry::pair("ok", "状态", "就绪").status(KvStatus::Success),
    ];
    let mut state = KeyValueListState::new();
    let _ = KeyValueList::reading(&entries, system).paint(area, frame.buffer_mut(), &mut state);
}

fn link_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    state.set_focused(true);
    let _ = Link::url("docs", "https://example.invalid/docs", system)
        .hyperlinks(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn link_no_hyperlink_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    let _ = Link::url("docs", "https://example.invalid/docs", system)
        .hyperlinks(false)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn link_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let system = system.clone().no_color();
    let mut state = LinkState::new();
    state.set_focused(true);
    let _ = Link::url("docs", "https://example.invalid", &system)
        .hyperlinks(false)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn link_app_route_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    state.set_hovered(true);
    let _ = Link::app_route("Settings", "app://settings", system)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn link_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    let _ = Link::url("文档 🔗", "https://example.invalid/文档", system)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn ansi_text_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::parse_lines;
    let src = "\
\x1b[1mbuild\x1b[0m \x1b[32mok\x1b[0m
\x1b[33mwarn\x1b[0m unused
\x1b[31merror\x1b[0m failed
plain trailing
";
    let lines = parse_lines(src, &AnsiParseOptions::for_system(system));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, system).paint(area, frame.buffer_mut(), &mut state);
}

fn ansi_text_no_color_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::parse_lines;
    let system = system.clone().no_color();
    let src = "\x1b[31;1mRED bold\x1b[0m \x1b[32mgreen\x1b[0m\nplain\n";
    let lines = parse_lines(src, &AnsiParseOptions::for_system(&system).no_color(true));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, &system)
        .mode(AnsiTextMode::NoColor)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn ansi_text_cr_bs_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::parse_lines;
    let src = "loading....\rDONE   \nabc\x08\x08X\n";
    let lines = parse_lines(src, &AnsiParseOptions::for_system(system));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, system).paint(area, frame.buffer_mut(), &mut state);
}

fn ansi_text_hyperlink_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::parse_lines;
    let src = "see \x1b]8;;https://example.invalid\x1b\\docs\x1b]8;;\x1b\\ here\n";
    let lines = parse_lines(src, &AnsiParseOptions::for_system(system));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, system).paint(area, frame.buffer_mut(), &mut state);
}

fn ansi_text_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::parse_lines;
    let src = "\x1b[36m文档\x1b[0m 🔗 \x1b[1mOK\x1b[0m\n";
    let lines = parse_lines(src, &AnsiParseOptions::for_system(system));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, system).paint(area, frame.buffer_mut(), &mut state);
}

fn action_link_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    state.set_focused(true);
    let _ = ActionLink::new("Run tests", system)
        .risk_note("cargo test")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn action_link_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = LinkState::new();
    let _ = ActionLink::new("运行测试", system)
        .risk_note("cargo test")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn kbd_platform_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::{
        ChordFormat, Kbd, ModifierStyle, Platform, format_chord,
    };
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let chord = KeyChord::ctrl(KeyCode::Char('s'));
    let emacs = format_chord(chord, ChordFormat::footer());
    let spelled = format_chord(
        chord,
        ChordFormat::docs().platform(Platform::Other),
    );
    let mac = format_chord(
        chord,
        ChordFormat {
            platform: Platform::Mac,
            modifiers: ModifierStyle::Symbols,
            ascii: false,
        },
    );
    let ascii = format_chord(KeyChord::plain(KeyCode::Up), ChordFormat::footer().ascii(true));
    let _ = Kbd::new(emacs, system).paint(chunks[0], frame.buffer_mut());
    let _ = Kbd::new(spelled, system).inline().paint(chunks[1], frame.buffer_mut());
    let _ = Kbd::new(mac, system).paint(chunks[2], frame.buffer_mut());
    let _ = Kbd::new(ascii, system).paint(chunks[3], frame.buffer_mut());
}

fn shortcut_hint_footer_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::{KeyBinding, KeyChord, Keymap, Visibility};
    use termrock::widgets::ShortcutHint;
    #[derive(Clone, Copy, PartialEq)]
    enum A {
        Save,
        Quit,
    }
    let map = Keymap::from_owned(vec![
        KeyBinding::owned(
            vec![KeyChord::ctrl(KeyCode::Char('s'))],
            A::Save,
            Some("Save".into()),
            Visibility::Shown,
            None,
        ),
        KeyBinding::owned(
            vec![KeyChord::ctrl(KeyCode::Char('q'))],
            A::Quit,
            Some("Quit".into()),
            Visibility::Shown,
            None,
        ),
    ]);
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    if let Some(h) = ShortcutHint::for_action(&map, A::Save, system) {
        h.footer().paint(chunks[0], frame.buffer_mut());
    }
    if let Some(h) = ShortcutHint::for_action(&map, A::Quit, system) {
        h.footer().paint(chunks[1], frame.buffer_mut());
    }
}

fn shortcut_hint_inline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::ShortcutHint;
    let h = ShortcutHint::from_chords(
        &[KeyChord::ctrl(KeyCode::Char('p'))],
        "Open command palette",
        system,
    )
    .inline_doc();
    h.paint(area, frame.buffer_mut());
}

fn shortcut_hint_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::ShortcutHint;
    let h = ShortcutHint::new("C-S", "Save the current document", system).footer();
    h.paint(area, frame.buffer_mut());
}

fn paragraph_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = termrock::widgets::Paragraph::new(
        "Body text wraps by display columns when height allows.",
        system,
    )
    .paint(area, frame.buffer_mut());
}

fn paragraph_quote_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = termrock::widgets::Paragraph::quote(
        "Quoted prose hangs under the gutter so wrapped lines stay aligned with the body.",
        system,
    )
    .reading()
    .paint(area, frame.buffer_mut());
}

fn paragraph_list_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Paragraph;
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .split(area);
    let _ = Paragraph::list_item(
        "First unordered item with hanging wrap for longer labels",
        system,
    )
    .paint(chunks[0], frame.buffer_mut());
    let _ = Paragraph::ordered_item("Second, numbered step in the plan", system, 2)
        .paint(chunks[1], frame.buffer_mut());
    let _ = Paragraph::list_item("Third bullet", system).paint(chunks[2], frame.buffer_mut());
}

fn paragraph_reading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Paragraph;
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(2)]).split(area);
    let _ = Paragraph::new(
        "Reading-mode body copy is intended for help, dialogs, and plan text that should wrap cleanly.",
        system,
    )
    .reading()
    .paint(chunks[0], frame.buffer_mut());
    let _ = Paragraph::quote("A call-out quote in the same recipe.", system)
        .reading()
        .paint(chunks[1], frame.buffer_mut());
}

fn surface_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let content = Surface::new(&tokens)
        .recipe(SurfaceRecipe::Raised)
        .paint(area, frame.buffer_mut());
    if content.width > 2 && content.height > 0 {
        frame.buffer_mut().set_stringn(
            content.x,
            content.y,
            "raised body",
            usize::from(content.width),
            system.style(Role::Text),
        );
    }
}

fn surface_ladder_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let recipes = [
        SurfaceRecipe::Canvas,
        SurfaceRecipe::Inset,
        SurfaceRecipe::Raised,
        SurfaceRecipe::Overlay,
        SurfaceRecipe::Interactive,
        SurfaceRecipe::Focused,
        SurfaceRecipe::Selected,
        SurfaceRecipe::Warning,
        SurfaceRecipe::Destructive,
    ];
    let rows = recipes.len() as u16;
    let row_h = (area.height / rows.max(1)).max(2);
    for (i, recipe) in recipes.iter().enumerate() {
        let y = area.y.saturating_add((i as u16).saturating_mul(row_h));
        if y >= area.bottom() {
            break;
        }
        let h = row_h.min(area.bottom().saturating_sub(y));
        let row = Rect::new(area.x, y, area.width, h);
        let content = Surface::new(system)
            .recipe(*recipe)
            .padding(1, 0)
            .paint(row, frame.buffer_mut());
        if content.width > 2 {
            frame.buffer_mut().set_stringn(
                content.x,
                content.y,
                recipe.id(),
                usize::from(content.width),
                system.style(Role::Text),
            );
        }
    }
}

fn surface_focused_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let content = Surface::new(system)
        .recipe(SurfaceRecipe::Focused)
        .paint(area, frame.buffer_mut());
    if content.width > 2 {
        frame.buffer_mut().set_stringn(
            content.x,
            content.y,
            "owns focus · BorderFocused",
            usize::from(content.width),
            system.style(Role::TextStrong),
        );
    }
}

fn surface_terminal_default_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    // Canvas underlay + transparent inset body (host terminal bg shows through).
    let _ = Surface::new(system)
        .recipe(SurfaceRecipe::Canvas)
        .paint(area, frame.buffer_mut());
    let inset = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    let content = Surface::new(system)
        .recipe(SurfaceRecipe::Inset)
        .fill(SurfaceFill::Transparent)
        .bordered(true)
        .paint(inset, frame.buffer_mut());
    if content.width > 2 {
        frame.buffer_mut().set_stringn(
            content.x,
            content.y,
            "terminal-default canvas",
            usize::from(content.width),
            system.style(Role::TextMuted),
        );
    }
}

fn separator_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .quiet()
        .paint(area, frame.buffer_mut());
}

fn separator_strong_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .strong()
        .paint(area, frame.buffer_mut());
}

fn separator_labeled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .label("OR")
        .paint(area, frame.buffer_mut());
}

fn separator_section_break_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .section_break()
        .with_density(system.density)
        .paint(area, frame.buffer_mut());
}

fn separator_vertical_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::vertical(system).paint(area, frame.buffer_mut());
}

fn separator_focus_zone_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .focus_zone()
        .paint(area, frame.buffer_mut());
}

fn popover_open_state() -> PopoverState {
    let mut state = PopoverState::new();
    state.set_open(true);
    state.set_focused(true);
    state.set_header_rows(1);
    state.set_footer_rows(0);
    state
}

fn popover_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = popover_open_state();
    Popover::new("Settings", system).paint(area, frame.buffer_mut(), &mut state);
    // Host paints body content into slots (not forced Panel).
    let body = state.slots().body;
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "filter · sort · density",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
}

fn popover_slots_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PopoverState::new();
    state.set_open(true);
    state.set_focused(true);
    state.set_header_rows(1);
    state.set_footer_rows(1);
    Popover::slots(system)
        .header(Some("Filters"))
        .footer(Some("esc · close"))
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.slots().body;
    if body.height > 0 {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "• status: open",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
        if body.height > 1 {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y + 1,
                "• owner: you",
                usize::from(body.width),
                system.style(termrock::style::Role::TextMuted),
            );
        }
    }
}

fn popover_modal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PopoverState::modal();
    state.set_open(true);
    state.set_focused(true);
    state.set_header_rows(1);
    state.set_footer_rows(1);
    Popover::new("Confirm scope", system)
        .footer(Some("modal · esc"))
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.slots().body;
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "focus trap + dim",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
}

fn popover_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PopoverState::new();
    state.set_open(true);
    state.set_focused(true);
    state.set_header_rows(1);
    state.set_footer_rows(1);
    // Presentation would be Drawer/Fullscreen at this width; paint still slots.
    Popover::new("More", system)
        .footer(Some("esc"))
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.slots().body;
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "drawer",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
}

fn permission_prompt_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let prompt = PermissionPrompt::new(system);
    let mut state = PermissionPromptState::new();
    let req = PermissionRequest::new("r1", "bash", "workspace")
        .risk(PermissionRisk::High)
        .action_kind(PermissionActionKind::Shell)
        .command("rm -rf build/")
        .provenance(PermissionProvenance::main_agent("a", "agent"));
    state.enqueue(req);
    frame.render_stateful_widget(&prompt, area, &mut state);
}

fn permission_nested_provenance() -> PermissionProvenance {
    use termrock::widgets::{InitiatorKind, ProvenanceHop};
    PermissionProvenance::main_agent("run-1", "main")
        .push(ProvenanceHop::new(
            InitiatorKind::Subagent,
            "sub-9",
            "reviewer",
        ))
        .push(ProvenanceHop::new(
            InitiatorKind::McpServer,
            "mcp-fs",
            "filesystem",
        ))
}

fn permission_prompt_low_read(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::PermissionScope;
    let prompt = PermissionPrompt::new(system);
    let mut state = PermissionPromptState::new();
    let req = PermissionRequest::new("r1", "read_file", "src/lib.rs")
        .risk(PermissionRisk::Low)
        .action_kind(PermissionActionKind::FileRead)
        .expected("file contents for analysis")
        .location("local", Some("~/proj".into()))
        .prior(PermissionScope::Session, "src/** previously Session")
        .provenance(PermissionProvenance::main_agent("a", "agent"));
    state.enqueue(req);
    frame.render_stateful_widget(&prompt, area, &mut state);
}

fn permission_prompt_destructive_nested(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let prompt = PermissionPrompt::new(system);
    let mut state = PermissionPromptState::new();
    // Queue noise ahead so chrome shows q depth after first enqueue of nested head
    state.enqueue(
        PermissionRequest::new("r0", "read_file", "README.md")
            .risk(PermissionRisk::Low)
            .action_kind(PermissionActionKind::FileRead)
            .provenance(PermissionProvenance::main_agent("a", "agent")),
    );
    let req = PermissionRequest::new("r2", "bash", "workspace")
        .risk(PermissionRisk::High)
        .action_kind(PermissionActionKind::Shell)
        .command("rm -rf build/")
        .expected("remove build artifacts")
        .location("local", Some("sandbox:off".into()))
        .irreversible()
        .provenance(permission_nested_provenance());
    state.enqueue(req);
    // Advance to destructive head for the story paint
    let _ = state.handle_key(termrock::input::KeyEvent::new(
        termrock::input::KeyCode::Enter,
        termrock::input::KeyModifiers::NONE,
    ));
    frame.render_stateful_widget(&prompt, area, &mut state);
}

fn permission_prompt_egress(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let prompt = PermissionPrompt::new(system);
    let mut state = PermissionPromptState::new();
    let req = PermissionRequest::new("r3", "http_post", "api.example.com")
        .risk(PermissionRisk::Critical)
        .action_kind(PermissionActionKind::Network)
        .egress("https://api.example.com/v1", "src/** + .env")
        .expected("upload diagnostics payload")
        .details(["payload≈48KB", "headers: redacted"])
        .provenance(permission_nested_provenance());
    state.enqueue(req);
    frame.render_stateful_widget(&prompt, area, &mut state);
}

fn mode_ribbon_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let modes = [
        WorkbenchMode {
            id: "plan",
            label: "Plan",
            active: true,
            enabled: true,
        },
        WorkbenchMode {
            id: "build",
            label: "Build",
            active: false,
            enabled: true,
        },
    ];
    Widget::render(ModeRibbon::new(&modes, &tokens), area, frame.buffer_mut());
}

fn plan_review_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_plan_document, PlanReview, PlanReviewState};
    let mut state = PlanReviewState::new();
    state.open(example_plan_document());
    state.focused = true;
    frame.render_stateful_widget(&PlanReview::new(system), area, &mut state);
}

fn plan_review_high_risk_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_high_risk_plan, PlanReview, PlanReviewState};
    let mut state = PlanReviewState::new();
    state.open(example_high_risk_plan());
    state.focused = true;
    frame.render_stateful_widget(&PlanReview::new(system), area, &mut state);
}

fn plan_review_diff_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_plan_document, PlanReview, PlanReviewPane, PlanReviewState,
    };
    let mut state = PlanReviewState::new();
    state.open(example_plan_document());
    state.pane = PlanReviewPane::Diff;
    state.show_version_diff = true;
    state.focused = true;
    frame.render_stateful_widget(&PlanReview::new(system), area, &mut state);
}

fn plan_review_comments_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_plan_document, PlanComment, PlanCommentAnchor, PlanReview, PlanReviewPane,
        PlanReviewState,
    };
    let mut state = PlanReviewState::new();
    state.open(example_plan_document());
    state.set_comments(vec![
        PlanComment::new(
            "c1",
            "Verify session TTL",
            PlanCommentAnchor::Line { line: 2 },
            2,
        ),
        PlanComment::new(
            "c2",
            "Section note",
            PlanCommentAnchor::Section {
                section_id: "steps".into(),
            },
            2,
        ),
    ]);
    state.pane = PlanReviewPane::Comments;
    state.focused = true;
    frame.render_stateful_widget(&PlanReview::new(system), area, &mut state);
}

fn question_flow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_question_set, QuestionFlow, QuestionFlowState};
    let mut state = QuestionFlowState::new();
    state.open_set(example_question_set());
    state.focused = true;
    frame.render_stateful_widget(&QuestionFlow::new(system), area, &mut state);
}

fn question_flow_review_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_question_set, QuestionAnswer, QuestionFlow, QuestionFlowPhase, QuestionFlowState,
    };
    let mut state = QuestionFlowState::new();
    state.open_set(example_question_set());
    state.answers.set(
        "q1",
        QuestionAnswer::Single {
            option_id: "canary".into(),
            other_text: None,
        },
    );
    state.answers.set("q2", QuestionAnswer::Skipped);
    state.answers.set(
        "q3",
        QuestionAnswer::FreeText {
            text: "latency".into(),
        },
    );
    state.phase = QuestionFlowPhase::Review;
    state.focused = true;
    frame.render_stateful_widget(&QuestionFlow::new(system), area, &mut state);
}

fn question_flow_multi_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_question_set, QuestionFlow, QuestionFlowState};
    let mut state = QuestionFlowState::new();
    state.open_set(example_question_set());
    // jump to multi step
    state.step_index = 1;
    state.step_states[1].multi_selected.insert("eng".into());
    state.step_states[1].multi_selected.insert("sre".into());
    state.focused = true;
    frame.render_stateful_widget(&QuestionFlow::new(system), area, &mut state);
}

fn question_flow_text_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_question_set, QuestionFlow, QuestionFlowState};
    let mut state = QuestionFlowState::new();
    state.open_set(example_question_set());
    state.step_index = 2;
    state.step_states[2].text = "risk: rollback window".into();
    state.step_states[2].text_mode = true;
    state.focused = true;
    frame.render_stateful_widget(&QuestionFlow::new(system), area, &mut state);
}

fn session_picker_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_sessions, SessionPicker, SessionPickerState};
    let mut state = SessionPickerState::new();
    state.set_sessions(example_sessions());
    state.focused = true;
    frame.render_stateful_widget(&SessionPicker::new(system), area, &mut state);
}

fn session_picker_search_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_sessions, SessionPicker, SessionPickerState};
    let mut state = SessionPickerState::new();
    state.set_sessions(example_sessions());
    state.set_query("auth");
    state.focused = true;
    frame.render_stateful_widget(&SessionPicker::new(system), area, &mut state);
}

fn session_picker_confirm_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_sessions, SessionConfirmAction, SessionPicker, SessionPickerPhase,
        SessionPickerState,
    };
    let mut state = SessionPickerState::new();
    state.set_sessions(example_sessions());
    state.phase = SessionPickerPhase::ConfirmDelete;
    state.confirm_action = Some(SessionConfirmAction::Delete);
    state.confirm_proceed_focused = false;
    state.focused = true;
    frame.render_stateful_widget(&SessionPicker::new(system), area, &mut state);
}

fn session_picker_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{SessionPicker, SessionPickerState};
    let mut state = SessionPickerState::new();
    state.focused = true;
    frame.render_stateful_widget(&SessionPicker::new(system), area, &mut state);
}

fn connection_manager_full_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_connections, ConnectionManager, ConnectionManagerPresentation,
        ConnectionManagerState,
    };
    let mut state = ConnectionManagerState::new();
    state.set_connections(example_connections());
    state.set_presentation(ConnectionManagerPresentation::Full);
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn connection_manager_launcher_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_connections, ConnectionManager, ConnectionManagerPresentation,
        ConnectionManagerState,
    };
    let mut state = ConnectionManagerState::new();
    state.set_connections(example_connections());
    state.set_presentation(ConnectionManagerPresentation::Launcher);
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn connection_manager_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ConnectionManager, ConnectionManagerState};
    let mut state = ConnectionManagerState::new();
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn connection_manager_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_connections, ConnectionManager, ConnectionManagerState,
    };
    let mut state = ConnectionManagerState::new();
    state.set_connections(example_connections());
    if let Some(i) = state
        .filtered_indices()
        .iter()
        .position(|&si| state.connections[si].id == "c4")
    {
        state.cursor = i;
    }
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn connection_manager_secret_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ConnectionFormField, ConnectionManager, ConnectionManagerPhase, ConnectionManagerState,
    };
    let mut state = ConnectionManagerState::new();
    state.phase = ConnectionManagerPhase::Add;
    state.form.name = "New DB".into();
    state.form.target = "localhost:5432".into();
    state.form.protocol_label = "postgres".into();
    state.form.environment = "local".into();
    state.form_field = ConnectionFormField::Secret;
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn connection_manager_confirm_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_connections, ConnectionManager, ConnectionManagerPhase, ConnectionManagerState,
    };
    let mut state = ConnectionManagerState::new();
    state.set_connections(example_connections());
    state.phase = ConnectionManagerPhase::ConfirmDelete;
    state.confirm_proceed_focused = false;
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn task_rail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_activity_models, TaskRail, TaskRailState};
    let items = example_activity_models();
    let mut st = TaskRailState::new();
    st.focused = true;
    st.list.select(Some("p1".into()));
    TaskRail::new(&items, system)
        .title("Tasks")
        .paint(area, frame.buffer_mut(), &mut st);
}

fn task_rail_input_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_activity_models, TaskRail, TaskRailState};
    let items = example_activity_models();
    let mut st = TaskRailState::new();
    st.focused = true;
    st.list.select(Some("p1".into()));
    // expand completed for visibility of mixed statuses
    st.collapsed.remove(&termrock::widgets::ActivityScope::Completed);
    TaskRail::new(&items, system).paint(area, frame.buffer_mut(), &mut st);
}

fn task_rail_filter_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_activity_models, TaskRail, TaskRailState};
    let items = example_activity_models();
    let mut st = TaskRailState::new();
    st.focused = true;
    st.filter = "cargo".into();
    st.filter_mode = true;
    TaskRail::new(&items, system).paint(area, frame.buffer_mut(), &mut st);
}

fn task_rail_drawer_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_activity_models, TaskRail, TaskRailState};
    let items = example_activity_models();
    let mut st = TaskRailState::new();
    st.focused = true;
    TaskRail::new(&items, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn task_rail_statusbar_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_activity_models, project_task_rail_for_status_bar, task_rail_status_slot, StatusBar,
        StatusBarState,
    };
    let items = example_activity_models();
    let proj = project_task_rail_for_status_bar(&items, true);
    let slot = task_rail_status_slot("tasks", &proj, false);
    let right = [slot];
    let mut st = StatusBarState::<&str>::new();
    frame.render_stateful_widget(StatusBar::new(&[], &right, system), area, &mut st);
}

fn subagent_card_running_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_subagent_runs, SubagentCard, SubagentCardState, SubagentPresentation,
    };
    let runs = example_subagent_runs();
    let run = runs.iter().find(|r| r.id == "sa1").unwrap_or(&runs[0]);
    let mut st = SubagentCardState::new();
    st.focused = true;
    st.presentation = SubagentPresentation::Card;
    SubagentCard::new(run, system).paint(area, frame.buffer_mut(), &mut st);
}

fn subagent_card_failed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_subagent_runs, SubagentCard, SubagentCardState, SubagentPresentation,
    };
    let runs = example_subagent_runs();
    let run = runs.iter().find(|r| r.id == "sa4").unwrap_or(&runs[3]);
    let mut st = SubagentCardState::new();
    st.focused = true;
    st.presentation = SubagentPresentation::Card;
    SubagentCard::new(run, system).paint(area, frame.buffer_mut(), &mut st);
}

fn subagent_card_nested_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_subagent_runs, SubagentCard, SubagentCardState, SubagentPresentation,
    };
    let runs = example_subagent_runs();
    let run = runs.iter().find(|r| r.id == "sa2").unwrap_or(&runs[1]);
    let mut st = SubagentCardState::new();
    st.focused = true;
    st.presentation = SubagentPresentation::Card;
    SubagentCard::new(run, system).paint(area, frame.buffer_mut(), &mut st);
}

fn subagent_card_row_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_subagent_runs, SubagentCard, SubagentCardState, SubagentPresentation,
    };
    let runs = example_subagent_runs();
    let mut st = SubagentCardState::new();
    st.focused = true;
    st.presentation = SubagentPresentation::CompactRow;
    SubagentCard::new(&runs[0], system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn subagent_card_result_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_subagent_runs, SubagentCard, SubagentCardState, SubagentPresentation,
    };
    let runs = example_subagent_runs();
    let run = runs.iter().find(|r| r.id == "sa3").unwrap_or(&runs[2]);
    let mut st = SubagentCardState::new();
    st.focused = true;
    st.presentation = SubagentPresentation::Card;
    SubagentCard::new(run, system).paint(area, frame.buffer_mut(), &mut st);
}

fn background_tasks_mixed_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_background_tasks, BackgroundTaskPanel, BackgroundTaskPanelState,
    };
    let tasks = example_background_tasks();
    let mut st = BackgroundTaskPanelState::new();
    st.focused = true;
    st.list.select(Some("b1".into()));
    BackgroundTaskPanel::new(&tasks, system).paint(area, frame.buffer_mut(), &mut st);
}

fn background_tasks_clear_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_background_tasks, BackgroundTaskPanel, BackgroundTaskPanelState,
    };
    let tasks = example_background_tasks();
    let mut st = BackgroundTaskPanelState::new();
    st.hide_completed = true;
    st.focused = true;
    BackgroundTaskPanel::new(&tasks, system).paint(area, frame.buffer_mut(), &mut st);
}

fn background_tasks_rail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_background_tasks, BackgroundTaskPanel, BackgroundTaskPanelState,
        BackgroundTaskPresentation,
    };
    let tasks = example_background_tasks();
    let mut st = BackgroundTaskPanelState::new();
    st.force_presentation = Some(BackgroundTaskPresentation::CompactRail);
    st.focused = true;
    BackgroundTaskPanel::new(&tasks, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn background_tasks_lost_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_background_tasks, BackgroundTaskPanel, BackgroundTaskPanelState,
    };
    let tasks = example_background_tasks();
    let mut st = BackgroundTaskPanelState::new();
    st.list.select(Some("b4".into()));
    st.focused = true;
    BackgroundTaskPanel::new(&tasks, system).paint(area, frame.buffer_mut(), &mut st);
}

fn background_tasks_dropped_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_background_tasks, BackgroundTaskPanel, BackgroundTaskPanelState,
    };
    let tasks = example_background_tasks();
    let mut st = BackgroundTaskPanelState::new();
    st.list.select(Some("b3".into())); // has dropped lines from small buffer
    st.focused = true;
    BackgroundTaskPanel::new(&tasks, system).paint(area, frame.buffer_mut(), &mut st);
}

fn context_meter_pressure_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let budgets = example_context_budgets();
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(area);
    for (i, b) in budgets.iter().take(2).enumerate() {
        let mut st = ContextMeterState::new();
        st.presentation = ContextMeterPresentation::Compact;
        ContextMeter::new(b, system).paint(chunks[i], frame.buffer_mut(), &mut st);
    }
    // high pressure is index 1
    let mut st = ContextMeterState::new();
    ContextMeter::new(&budgets[1], system).paint(chunks[2], frame.buffer_mut(), &mut st);
}

fn context_meter_indeterminate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let b = &example_context_budgets()[2];
    let mut st = ContextMeterState::new();
    st.presentation = ContextMeterPresentation::Expanded;
    ContextMeter::new(b, system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn context_meter_approximate_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let b = &example_context_budgets()[3];
    let mut st = ContextMeterState::new();
    st.presentation = ContextMeterPresentation::Expanded;
    ContextMeter::new(b, system).paint(area, frame.buffer_mut(), &mut st);
}

fn context_meter_expanded_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let b = &example_context_budgets()[1];
    let mut st = ContextMeterState::new();
    st.focused = true;
    st.presentation = ContextMeterPresentation::Expanded;
    ContextMeter::new(b, system).paint(area, frame.buffer_mut(), &mut st);
}

fn context_meter_mono_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let b = &example_context_budgets()[0];
    let mut st = ContextMeterState::new();
    st.presentation = ContextMeterPresentation::Compact;
    ContextMeter::new(b, system)
        .mono(true)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut st);
}

fn context_meter_bytes_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        example_context_budgets, ContextMeter, ContextMeterPresentation, ContextMeterState,
    };
    let b = &example_context_budgets()[4];
    let mut st = ContextMeterState::new();
    st.presentation = ContextMeterPresentation::Expanded;
    ContextMeter::new(b, system).paint(area, frame.buffer_mut(), &mut st);
}

// ── State-axis story helpers ────────────────────────────────────────────────

fn button_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    state.activation.set_enabled(false);
    frame.render_stateful_widget(
        &Button::new("Save", &tokens).primary(true),
        area,
        &mut state,
    );
}

fn button_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    state.activation.set_loading(true);
    frame.render_stateful_widget(&Button::new("Save", &tokens), area, &mut state);
}

fn button_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_accepts_input(true);
    frame.render_stateful_widget(
        &Button::new("保存 ✨", &tokens).primary(true),
        area,
        &mut state,
    );
}

fn checkbox_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = CheckboxState::new(true);
    state.set_enabled(false);
    let _ = Checkbox::new("enable", "Enable", &tokens).paint(area, frame.buffer_mut(), &mut state);
}

fn checkbox_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut cb = CheckboxState::new(true);
    cb.set_focused(true);
    let _ = Checkbox::new("jp", "有効化 🇯🇵", &tokens)
        .description("説明テキスト")
        .paint(
            Rect::new(area.x, area.y, area.width, 2.min(area.height)),
            frame.buffer_mut(),
            &mut cb,
        );
    let mut sw = SwitchState::new(false);
    let _ = Switch::new("dark", "暗色モード", &tokens).paint(
        Rect::new(area.x, area.y.saturating_add(2), area.width, 1),
        frame.buffer_mut(),
        &mut sw,
    );
}

fn data_table_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        termrock::widgets::DataColumn::new("id", "ID", termrock::widgets::DataColumnWidth::Min(4)),
        termrock::widgets::DataColumn::new(
            "name",
            "Name",
            termrock::widgets::DataColumnWidth::Min(8),
        ),
    ]);
    let rows: [(u64, &[&str]); 0] = [];
    let toolbar = DataTableToolbar {
        actions: &["Refresh"],
    };
    let mut state = DataTableState::<u64, &str>::new();
    state.load = termrock::widgets::LoadState::Empty {
        message: Some("No rows".into()),
    };
    DataTable::new(&tokens, &columns, &rows)
        .toolbar(&toolbar)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_process(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ColumnModel, DataColumn, DataColumnWidth, LoadState, SortSpec,
    };
    let tokens = system.clone().density(Density::Compact);
    let columns = ColumnModel::new(vec![
        DataColumn::new("name", "PROCESS", DataColumnWidth::Min(14)).priority(100),
        DataColumn::new("pid", "PID", DataColumnWidth::Fixed(6)).priority(90).sortable(),
        DataColumn::new("cpu", "CPU%", DataColumnWidth::Fixed(6)).priority(70).sortable(),
        DataColumn::new("mem", "MEM", DataColumnWidth::Fixed(7)).priority(50),
    ]);
    let r0: &[&str] = &["systemd", "1", "0.1", "4.2M"];
    let r1: &[&str] = &["sshd", "482", "0.0", "8.1M"];
    let r2: &[&str] = &["bash", "1204", "1.2", "12M"];
    let r3: &[&str] = &["cargo", "1888", "42.0", "640M"];
    let r4: &[&str] = &["rustc", "1902", "88.4", "1.1G"];
    let rows = [
        TreeTableRow::new(1u64, 0, r0).branch().expanded(),
        TreeTableRow::new(482, 1, r1).branch().expanded().parent(1),
        TreeTableRow::new(1204, 2, r2).branch().expanded().parent(482),
        TreeTableRow::new(1888, 3, r3).branch().expanded().parent(1204),
        TreeTableRow::new(1902, 4, r4).parent(1888),
    ];
    let mut state = TreeTableState::new(Some(1888));
    state.load = LoadState::Ready { count: 5 };
    state.sort = Some(SortSpec {
        column: "cpu",
        ascending: false,
    });
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .compact_indent(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_schema(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnModel, DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::default());
    let columns = ColumnModel::new(vec![
        DataColumn::new("name", "Name", DataColumnWidth::Min(16)).priority(100),
        DataColumn::new("type", "Type", DataColumnWidth::Min(10)).priority(80).sortable(),
        DataColumn::new("null", "Null", DataColumnWidth::Fixed(5)).priority(40),
        DataColumn::new("key", "Key", DataColumnWidth::Fixed(4)).priority(60),
    ]);
    let r0: &[&str] = &["public", "schema", "", ""];
    let r1: &[&str] = &["users", "table", "", ""];
    let r2: &[&str] = &["id", "uuid", "NO", "PK"];
    let r3: &[&str] = &["email", "text", "NO", "UQ"];
    let r4: &[&str] = &["orders", "table", "", ""];
    let r5: &[&str] = &["user_id", "uuid", "NO", "FK"];
    let rows = [
        TreeTableRow::new("s", 0, r0).branch().expanded(),
        TreeTableRow::new("t1", 1, r1).branch().expanded().parent("s"),
        TreeTableRow::new("c1", 2, r2).parent("t1"),
        TreeTableRow::new("c2", 2, r3).parent("t1"),
        TreeTableRow::new("t2", 1, r4).branch().expanded().parent("s"),
        TreeTableRow::new("c3", 2, r5).parent("t2"),
    ];
    let mut state = TreeTableState::new(Some("c1"));
    state.load = LoadState::Ready { count: 6 };
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_tasks(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnModel, DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::default());
    let columns = ColumnModel::new(vec![
        DataColumn::new("task", "Task", DataColumnWidth::Min(18)).priority(100),
        DataColumn::new("status", "Status", DataColumnWidth::Fixed(8)).priority(70).sortable(),
        DataColumn::new("owner", "Owner", DataColumnWidth::Min(8)).priority(50),
    ]);
    let r0: &[&str] = &["Release v0.13", "active", "team"];
    let r1: &[&str] = &["Ship TreeTable", "doing", "alex"];
    let r2: &[&str] = &["Write stories", "todo", "alex"];
    let r3: &[&str] = &["Docs pass", "todo", "docs"];
    let r4: &[&str] = &["lazy epic", "…", "—"];
    let rows = [
        TreeTableRow::new("e1", 0, r0).branch().expanded(),
        TreeTableRow::new("t1", 1, r1).branch().expanded().parent("e1"),
        TreeTableRow::new("t2", 2, r2).parent("t1"),
        TreeTableRow::new("t3", 1, r3).parent("e1"),
        TreeTableRow::new("lazy", 0, r4).lazy_branch(),
    ];
    let mut state = TreeTableState::new(Some("t1"));
    state.load = LoadState::Ready { count: 5 };
    state.enable_multi_select();
    state.selection.toggle_row("t2");
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_deps(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnModel, DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::Compact);
    let columns = ColumnModel::new(vec![
        DataColumn::new("pkg", "Package", DataColumnWidth::Min(16)).priority(100),
        DataColumn::new("ver", "Version", DataColumnWidth::Fixed(10)).priority(80).sortable(),
        DataColumn::new("lic", "License", DataColumnWidth::Fixed(8)).priority(40),
    ]);
    let r0: &[&str] = &["termrock", "0.11.0", "Apache"];
    let r1: &[&str] = &["ratatui-core", "0.1.2", "MIT"];
    let r2: &[&str] = &["unicode-width", "0.2", "MIT/Apache"];
    let r3: &[&str] = &["serde", "1.0", "MIT/Apache"];
    let rows = [
        TreeTableRow::new("root", 0, r0).branch().expanded(),
        TreeTableRow::new("rt", 1, r1).branch().expanded().parent("root"),
        TreeTableRow::new("uw", 2, r2).parent("rt"),
        TreeTableRow::new("se", 1, r3).parent("root"),
    ];
    let mut state = TreeTableState::new(Some("rt"));
    state.load = LoadState::Ready { count: 4 };
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .compact_indent(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnModel, DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::Compact);
    let mut columns = ColumnModel::new(vec![
        DataColumn::new("name", "Name", DataColumnWidth::Min(10)).priority(100),
        DataColumn::new("meta", "Meta", DataColumnWidth::Min(8)).priority(20),
        DataColumn::new("extra", "Extra", DataColumnWidth::Min(8)).priority(5),
    ]);
    if area.width < 40 {
        columns.contract_to_budget(2, 90);
    }
    let r0: &[&str] = &["root", "m0", "e0"];
    let r1: &[&str] = &["child", "m1", "e1"];
    let rows = [
        TreeTableRow::new(1u64, 0, r0).branch().expanded(),
        TreeTableRow::new(2, 1, r1).parent(1),
    ];
    let mut state = TreeTableState::new(Some(1));
    state.load = LoadState::Ready { count: 2 };
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .compact_indent(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn tree_table_aggregate(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{ColumnModel, DataColumn, DataColumnWidth, LoadState};
    let tokens = system.clone().density(Density::default());
    let columns = ColumnModel::new(vec![
        DataColumn::new("name", "Name", DataColumnWidth::Min(12)).priority(100),
        DataColumn::new("n", "N", DataColumnWidth::Fixed(5)).priority(80).sortable(),
        DataColumn::new("bytes", "Bytes", DataColumnWidth::Fixed(8)).priority(60),
    ]);
    let g: &[&str] = &["eu-west", "", ""];
    let r0: &[&str] = &["api", "12", "4.2M"];
    let r1: &[&str] = &["worker", "4", "1.1M"];
    let tot: &[&str] = &["TOTAL", "16", "5.3M"];
    let rows = [
        TreeTableRow::new("g", 0, g).group().expanded(),
        TreeTableRow::new("a", 1, r0).parent("g"),
        TreeTableRow::new("w", 1, r1).parent("g"),
        TreeTableRow::new("t", 0, tot).aggregate(),
    ];
    let mut state = TreeTableState::new(Some("a"));
    state.load = LoadState::Ready { count: 3 };
    TreeTable::new(&tokens, &columns, &rows)
        .focused(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_http(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::group("g", "Request"),
        KvtField::pair("m", "method", "GET")
            .value_type("string")
            .source("line")
            .copyable()
            .depth(1),
        KvtField::pair("h", "host", "api.tailrocks.dev")
            .value_type("host")
            .source("header")
            .copyable()
            .depth(1),
        KvtField::pair("a", "authorization", "Bearer sk-live-…")
            .value_type("secret")
            .source("header")
            .secret()
            .copyable()
            .depth(1),
        KvtField::pair("c", "content-type", "application/json")
            .value_type("mime")
            .source("header")
            .editable()
            .depth(1),
        KvtField::pair("u", "user-agent", "termrock/0.11")
            .value_type("string")
            .source("header")
            .depth(1),
    ];
    let mut state = KeyValueTableState::new().with_cursor("h");
    state.load = LoadState::Ready { count: 5 };
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_database(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::group("t", "users"),
        KvtField::pair("id", "id", "uuid")
            .value_type("uuid")
            .source("pk")
            .annotation("not null")
            .depth(1),
        KvtField::pair("em", "email", "text")
            .value_type("text")
            .source("col")
            .annotation("unique")
            .status(KvStatus::Success)
            .depth(1),
        KvtField::pair("ag", "age", "integer")
            .value_type("int")
            .source("col")
            .validation(KvtValidation::Warning)
            .validation_message("prefer smallint")
            .depth(1),
        KvtField::pair("pw", "password_hash", "bytea")
            .value_type("secret")
            .source("col")
            .secret()
            .depth(1),
    ];
    let mut state = KeyValueTableState::new().with_cursor("em");
    state.load = LoadState::Ready { count: 4 };
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_process(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::pair("pid", "pid", "1902").value_type("int").copyable(),
        KvtField::pair("cmd", "command", "rustc --crate-type lib")
            .value_type("string")
            .copyable(),
        KvtField::pair("cpu", "cpu", "88.4%").value_type("pct").status(KvStatus::Warning),
        KvtField::pair("mem", "rss", "1.1G").value_type("bytes"),
        KvtField::pair("user", "user", "donbeave").value_type("string").source("os"),
        KvtField::pair("cwd", "cwd", "/Users/donbeave/Projects/termrock")
            .value_type("path")
            .copyable(),
    ];
    let mut state = KeyValueTableState::new().with_cursor("cmd");
    state.load = LoadState::Ready { count: 6 };
    state.density = termrock::widgets::KvDensity::Dense;
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_permission(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::group("c", "Claim"),
        KvtField::pair("act", "action", "shell.exec")
            .value_type("enum")
            .source("tool")
            .depth(1),
        KvtField::pair("risk", "risk", "high")
            .value_type("enum")
            .status(KvStatus::Danger)
            .depth(1),
        KvtField::pair("scope", "scope", "session")
            .value_type("enum")
            .source("policy")
            .depth(1),
        KvtField::pair("reason", "reason", "network egress to untrusted host")
            .value_type("text")
            .depth(1),
        KvtField::pair("dec", "decision", "allow once")
            .value_type("enum")
            .status(KvStatus::Success)
            .editable()
            .depth(1),
    ];
    let mut state = KeyValueTableState::new().with_cursor("risk");
    state.load = LoadState::Ready { count: 5 };
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_agent(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::group("t", "Tool call"),
        KvtField::pair("name", "name", "run_terminal_command")
            .value_type("id")
            .copyable()
            .depth(1),
        KvtField::pair("status", "status", "running")
            .value_type("enum")
            .status(KvStatus::Info)
            .depth(1),
        KvtField::pair("timeout", "timeout_ms", "120000")
            .value_type("int")
            .editable()
            .depth(1),
        KvtField::pair("cmd", "command", "cargo test -p termrock")
            .value_type("string")
            .copyable()
            .depth(1),
        KvtField::pair("tok", "api_key", "xai-…")
            .value_type("secret")
            .secret()
            .depth(1),
    ];
    let mut state = KeyValueTableState::new().with_cursor("timeout");
    state.load = LoadState::Ready { count: 5 };
    state.editing = true;
    state.edit_draft = "180000".into();
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_compare(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::pair("h", "host", "api.prod.example")
            .compare("api.staging.example")
            .value_type("host")
            .copyable(),
        KvtField::pair("p", "port", "443").compare("443").value_type("int"),
        KvtField::pair("t", "tls", "1.3")
            .compare("1.2")
            .value_type("enum")
            .status(KvStatus::Warning),
        KvtField::pair("r", "region", "us-east-1")
            .compare("eu-west-1")
            .value_type("string"),
    ];
    let mut state = KeyValueTableState::new().with_cursor("h");
    state.mode = KvtMode::Compare;
    state.load = LoadState::Ready { count: 4 };
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn key_value_table_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::LoadState;
    let fields = [
        KvtField::group("g", "Meta"),
        KvtField::pair("a", "name", "termrock")
            .value_type("string")
            .source("cargo")
            .depth(1),
        KvtField::pair("b", "version", "0.11.0")
            .value_type("semver")
            .depth(1),
        KvtField::pair(
            "c",
            "description",
            "Terminal component library with dense detail panels",
        )
        .value_type("text")
        .depth(1),
    ];
    let mut state = KeyValueTableState::new().with_cursor("a");
    state.layout = KvLayout::Auto;
    state.density = termrock::widgets::KvDensity::Dense;
    state.load = LoadState::Ready { count: 3 };
    KeyValueTable::new(&fields, system).render(area, frame.buffer_mut(), &mut state);
}

fn data_table_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let columns = termrock::widgets::ColumnModel::new(vec![
        termrock::widgets::DataColumn::new(
            "id",
            "番号",
            termrock::widgets::DataColumnWidth::Min(4),
        ),
        termrock::widgets::DataColumn::new(
            "name",
            "名称 ✨",
            termrock::widgets::DataColumnWidth::Min(8),
        ),
    ]);
    let cells0: &[&str] = &["一", "アルファ 🚀"];
    let cells1: &[&str] = &["二", "ベータ"];
    let rows = [(1u64, cells0), (2u64, cells1)];
    let toolbar = DataTableToolbar {
        actions: &["更新", "出力"],
    };
    let mut state = DataTableState::<u64, &str>::new();
    DataTable::new(&tokens, &columns, &rows)
        .toolbar(&toolbar)
        .render(area, frame.buffer_mut(), &mut state);
}

fn menu_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let items = [
        MenuItem::new("a", "開く 📂"),
        MenuItem::new("b", "無効").enabled(false),
        MenuItem::new("c", "保存 ✨"),
    ];
    let mut state = MenuState::new();
    Menu::new(&items, &tokens).render(area, frame.buffer_mut(), &mut state);
}

fn action_bar_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [
        Action {
            id: "accept",
            label: "承認 ✅",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "取消 🚫",
            enabled: true,
            style: None,
        },
    ];
    let mut state = ActionBarState {
        cursor: Some("accept"),
        ..ActionBarState::default()
    };
    frame.render_stateful_widget(
        &ActionBar::new(&actions, system).gap("  "),
        area,
        &mut state,
    );
}

fn panel_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(
        Panel::new(&panel_tokens)
            .title("概要 ✨")
            .emphasis(PanelChrome::Focused),
        area,
    );
    if area.width > 2 && area.height > 2 {
        frame.render_widget(
            Paragraph::new("状態   準備完了\nモード 対話 🚀"),
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2),
        );
    }
}

fn tree_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let nodes = [
        TreeNode {
            id: "ws",
            label: Line::from("作業場 🗂️"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
            parent: None,
        },
        TreeNode {
            id: "src",
            label: Line::from("ソース 📦"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            parent: None,
        },
    ];
    let mut state = TreeState::new(Some("ws"));
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tabs_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab {
            id: "one",
            label: "概要 ✨",
            glyph: Some(Span::styled("●", system.style(Role::Success))),
            badge: None,
            status: TabStatus::Success,
            active: true,
            enabled: true,
            closable: false,
        },
        Tab {
            id: "two",
            label: "詳細 📋",
            glyph: None,
            badge: None,
            status: TabStatus::None,
            active: false,
            enabled: true,
            closable: false,
        },
    ];
    let mut state = TabsState::new().with_selected("one");
    state.set_focused(true);
    frame.render_stateful_widget(&Tabs::new(&items, system).gap(1), area, &mut state);
}

fn form_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    form(frame, area, system);
    // Overpaint a unicode title cue so SVG body differs deterministically.
    if area.height > 0 && area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "設定 ⚙️",
            usize::from(area.width),
            system.style(Role::TextStrong),
        );
    }
}

fn dialog_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    frame.render_widget(
        Panel::new(&tokens)
            .title("確認 ❓")
            .emphasis(PanelChrome::Focused),
        area,
    );
    if area.width > 2 && area.height > 2 {
        frame.render_widget(
            Paragraph::new("この操作を実行しますか？\n日本語 + emoji 🚀"),
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2),
        );
    }
}

fn choice_dialog_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    choice_dialog(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "選択 ✨",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn message_dialog_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    message_dialog(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "通知 📣",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn status_bar_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    status_bar(frame, area, system);
    if area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "準備完了 ✅ | 行 42",
            usize::from(area.width),
            system.style(Role::StatusBar),
        );
    }
}

fn toast_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    toast(frame, area, system);
    if area.width > 2 && area.height > 0 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(area.height / 2),
            "保存しました ✨",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Success),
        );
    }
}

fn log_pane_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    log_pane(frame, area, system);
    if area.width > 2 && area.height > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "情報: 接続完了 🌐",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Info),
        );
    }
}

fn command_palette_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    command_palette(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "コマンドを検索… 🔍",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Input),
        );
    }
}

fn prompt_composer_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    prompt_composer_basic(frame, area, system);
    if area.width > 4 && area.height > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(2),
            area.y.saturating_add(area.height.saturating_sub(3)),
            "こんにちは 世界 🌍",
            usize::from(area.width.saturating_sub(4)),
            system.style(Role::Text),
        );
    }
}

fn permission_prompt_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    permission_prompt_story(frame, area, system);
    if area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "権限要求: シェル ⚠️",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Warning),
        );
    }
}

fn timeline_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    timeline(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "開始 🚀",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn tool_card_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    tool_card(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "ツール: シェル 🔧",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn theme_picker_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    theme_picker(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "テーマ選択 🎨",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn diff_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    diff_basic(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "差分: 設定.json ✨",
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
    }
}

fn design_inspector_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    design_inspector(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "検査: フォーカス 🔍",
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
    }
}

fn hint_bar_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    hint_bar(frame, area, system);
    if area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "↑↓ 移動  ⏎ 決定  🌐",
            usize::from(area.width),
            system.style(Role::HintText),
        );
    }
}

fn split_pane_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    split_pane(frame, area, system);
    if area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "左ペイン 📁",
            usize::from(area.width / 2),
            system.style(Role::Text),
        );
    }
}

fn viewport_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    viewport(frame, area, system);
    if area.width > 2 && area.height > 1 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "日本語行 📜 絵文字",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Text),
        );
    }
}

fn backdrop_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    backdrop(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(area.height / 2),
            "モーダル背景 🌑",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextMuted),
        );
    }
}

fn empty_state_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    EmptyState::new("結果なし 🌀", system)
        .kind(EmptyKind::NoResults)
        .explanation("クエリを変更してください")
        .primary(EmptyAction::with_shortcut("クリア", "esc"))
        .example("status:失敗")
        .paint(area, frame.buffer_mut());
}

fn error_view_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    ErrorState::new("失敗しました 💥", system)
        .kind(ErrorKind::Network)
        .explanation("再試行してください")
        .technical("timeout: GET /v1/jobs")
        .recovery(
            Recovery::none()
                .with_retry(RecoveryAction::with_shortcut("再試行", "r"))
                .with_retry_safety(RetrySafety::Safe)
                .with_work_preserved(true, Some("下書きを保持")),
        )
        .paint(area, frame.buffer_mut());
}

fn loading_view_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(LoadingView::new("読込中… ⏳", "⠋", system), area);
}

fn banner_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Banner::new("警告: 接続不安定 ⚠️", Severity::Warning, system),
        area,
    );
}

fn skeleton_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    skeleton(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "読込プレースホルダ …",
            usize::from(area.width),
            system.style(Role::TextDisabled),
        );
    }
}

fn jump_overlay_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    jump_overlay(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "ジャンプ a→ファイル 🎯",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Accent),
        );
    }
}

fn code_block_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    code_block(frame, area, system);
    if area.width > 2 && area.height > 1 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "// こんにちは 世界",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Text),
        );
    }
}

fn markdown_view_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    markdown_view(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "# 見出し ✨",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn sparkline_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    sparkline(frame, area, system);
    if area.width > 4 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "負荷 📈",
            6.min(usize::from(area.width)),
            system.style(Role::TextMuted),
        );
    }
}

fn bar_series_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    bar_series(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "CPU 使用率 📊",
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
    }
}

fn segmented_meter_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    segmented_meter(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "配分 🧩",
            6.min(usize::from(area.width)),
            system.style(Role::TextMuted),
        );
    }
}

fn token_meter_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    token_meter(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "トークン 🧮",
            8.min(usize::from(area.width)),
            system.style(Role::TextMuted),
        );
    }
}

fn thinking_block_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    thinking_block(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "思考中 🤔",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextMuted),
        );
    }
}

fn image_surface_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    image_surface(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "画像: 写真.png 🖼️",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextMuted),
        );
    }
}

fn mode_ribbon_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let modes = [
        WorkbenchMode {
            id: "plan",
            label: "計画 📝",
            active: true,
            enabled: true,
        },
        WorkbenchMode {
            id: "build",
            label: "構築 🔨",
            active: false,
            enabled: true,
        },
    ];
    Widget::render(ModeRibbon::new(&modes, &tokens), area, frame.buffer_mut());
}

fn plan_review_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        PlanDocument, PlanReview, PlanReviewState, PlanTask, PermissionRisk,
    };
    let mut state = PlanReviewState::new();
    state.open(
        PlanDocument::new(
            "u1",
            1,
            "計画 🚀",
            "# 検査\n\n- ファイルを読む\n- 編集 ✏️\n",
            PermissionRisk::Medium,
        )
        .summary("Unicode plan body")
        .tasks(vec![
            PlanTask::new("s1", "検査 🔍"),
            PlanTask::new("s2", "編集 ✏️"),
        ]),
    );
    state.focused = true;
    frame.render_stateful_widget(&PlanReview::new(system), area, &mut state);
}

fn question_flow_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        Question, QuestionFlow, QuestionFlowState, QuestionOption, QuestionSet,
    };
    let set = QuestionSet::new(
        "u1",
        "確認",
        vec![Question::single(
            "q1",
            "続行しますか？",
            vec![
                QuestionOption::new("y", "はい ✅"),
                QuestionOption::new("n", "いいえ ❌"),
            ],
        )],
    );
    let mut state = QuestionFlowState::new();
    state.open_set(set);
    state.focused = true;
    frame.render_stateful_widget(&QuestionFlow::new(system), area, &mut state);
}

fn session_picker_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{SessionEntry, SessionPicker, SessionPickerState, SessionStatus};
    let mut state = SessionPickerState::new();
    state.set_sessions(vec![
        SessionEntry::new("s1", "セッション甲 🅰️")
            .project("プロジェクト")
            .recency("2分前")
            .status(SessionStatus::Active)
            .summary("概要テキスト"),
        SessionEntry::new("s2", "セッション乙 🅱️")
            .branch("機能")
            .pinned(true),
    ]);
    state.focused = true;
    frame.render_stateful_widget(&SessionPicker::new(system), area, &mut state);
}

fn connection_manager_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ConnectionEntry, ConnectionKind, ConnectionManager, ConnectionManagerState,
        ConnectionStatus, ConnectionCredentialMeta,
    };
    let mut state = ConnectionManagerState::new();
    state.set_connections(vec![
        ConnectionEntry::new(
            "u1",
            "本番DB 🔍",
            ConnectionKind::Database,
            "postgres",
            "db.東京:5432",
        )
        .environment("本番")
        .group("データベース")
        .status(ConnectionStatus::Connected)
        .favorite(true)
        .credential(ConnectionCredentialMeta::present("パスワード")),
        ConnectionEntry::new("u2", "堡垒机", ConnectionKind::Ssh, "ssh", "堡垒:22")
            .environment("运维")
            .status(ConnectionStatus::AuthRequired),
    ]);
    state.focused = true;
    frame.render_stateful_widget(&ConnectionManager::new(system), area, &mut state);
}

fn task_rail_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{
        ActivityKind, ActivityModel, ActivityScope, SemanticStatus, TaskRail, TaskRailState,
    };
    let items = vec![
        ActivityModel::new("u1", "タスク一 📌")
            .scope(ActivityScope::Foreground)
            .kind(ActivityKind::Tool)
            .status(SemanticStatus::Running)
            .elapsed("1s"),
        ActivityModel::new("u2", "サブエージェント 🔍")
            .scope(ActivityScope::Subagent)
            .status(SemanticStatus::Waiting)
            .needs_input(true)
            .waiting_reason("確認"),
    ];
    let mut st = TaskRailState::new();
    st.focused = true;
    TaskRail::new(&items, system)
        .title("任務")
        .paint(area, frame.buffer_mut(), &mut st);
}

fn drawer_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = termrock::widgets::DrawerState::new();
    state.open();
    Drawer::new("設定 ⚙️", system).paint(area, frame.buffer_mut(), &mut state);
}

fn popover_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = popover_open_state();
    state.set_footer_rows(1);
    Popover::new("ヒント 💡", system)
        .footer(Some("閉じる"))
        .paint(area, frame.buffer_mut(), &mut state);
    let body = state.slots().body;
    if !body.is_empty() {
        frame.buffer_mut().set_stringn(
            body.x,
            body.y,
            "設定 · フィルター",
            usize::from(body.width),
            system.style(termrock::style::Role::Text),
        );
    }
}

fn separator_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Separator;
    Separator::horizontal(system)
        .label("区切")
        .paint(area, frame.buffer_mut());
    // Distinct label cell so body differs even though rule glyphs match.
    if area.width > 4 && area.height > 0 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "区切 ─ 線",
            usize::from(area.width),
            system.style(Role::Border),
        );
    }
}

fn surface_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let content = Surface::new(&tokens)
        .recipe(SurfaceRecipe::Raised)
        .paint(area, frame.buffer_mut());
    if content.width > 2 {
        frame.buffer_mut().set_stringn(
            content.x,
            content.y,
            "面 🎴 raised",
            usize::from(content.width),
            system.style(Role::Text),
        );
    }
}

fn kbd_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Kbd;
    let _ = Kbd::new("⌘K", system).keycap().paint(area, frame.buffer_mut());
}

fn form_wizard_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FormWizardState::with_steps([
        WizardStep::new("a", "設定"),
        WizardStep::new("b", "接続 🪄"),
        WizardStep::new("c", "確認"),
    ]);
    state.set_focused(true);
    FormWizard::new(system)
        .title("ウィザード 🪄")
        .ascii(false)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn badge_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::Badge;
    let _ = Badge::new("新规 ✨", system)
        .warning()
        .paint(area, frame.buffer_mut(), None);
}

fn heading_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = Heading::new("見出し ✨", system)
        .h1()
        .reading()
        .paint(area, frame.buffer_mut());
}

fn paragraph_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let _ = termrock::widgets::Paragraph::new("日本語と絵文字 🚀 を含む本文。", system)
        .paint(area, frame.buffer_mut());
}

fn callout_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Callout::new("注意", &tokens)
            .body("絵文字付きの説明 ⚠️")
            .tone(CalloutTone::Warning),
        area,
        frame.buffer_mut(),
    );
}

fn text_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("filter term");
    state.set_focused(true);
    let _ = TextInput::new("Query", system)
        .placeholder("Search…")
        .show_clear(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn text_input_secret_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("hunter2");
    state.set_focused(true);
    let _ = TextInput::new("Password", system)
        .secret(true)
        .show_clear(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn password_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PasswordInputState::with_secret("hunter2");
    state.set_focused(true);
    let _ = PasswordInput::new("Password", system)
        .placeholder("Enter secret…")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn password_input_reveal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PasswordInputState::with_secret("hunter2")
        .with_reveal_policy(RevealPolicy::Explicit);
    state.set_focused(true);
    let _ = state.set_revealed(true);
    let _ = PasswordInput::new("Password", system)
        .ascii(true)
        .show_reveal(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn password_input_invalid_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PasswordInputState::with_secret("x");
    state.set_focused(true);
    let _ = PasswordInput::new("Password", system)
        .validation(Validation::Invalid("too short"))
        .strength(PasswordStrengthHint::Weak)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn password_input_pending_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PasswordInputState::with_secret("token");
    state.set_focused(true);
    state.set_pending(true);
    let _ = PasswordInput::new("Token", system)
        .strength(PasswordStrengthHint::Pending)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn number_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NumberInputState::new()
        .with_constraints(NumberConstraints::bounded(0.0, 100.0, 1.0))
        .with_value(42.0);
    state.set_focused(true);
    let _ = NumberInput::new("Opacity", system)
        .unit("%")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn number_input_decimal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NumberInputState::new()
        .with_kind(NumberKind::decimal2())
        .with_constraints(NumberConstraints::bounded(0.0, 10.0, 0.25))
        .with_value(1.5);
    state.set_focused(true);
    let _ = NumberInput::new("Scale", system)
        .unit("x")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn number_input_invalid_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NumberInputState::new()
        .with_constraints(NumberConstraints::bounded(0.0, 10.0, 1.0))
        .with_value(3.0);
    state.set_focused(true);
    // Draft out of range (committed remains until host commits)
    let _ = state.insert_str("99");
    let _ = NumberInput::new("Count", system)
        .validation(Validation::Invalid("out of range"))
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn number_input_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = NumberInputState::new().with_value(7.0);
    state.set_focused(true);
    let _ = NumberInput::new("N", system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn search_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchInputState::new().with_query("table");
    state.set_focused(true);
    let _ = SearchInput::new(system)
        .placeholder("Search…")
        .status(SearchStatus::Results { count: 12 })
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn search_input_searching_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchInputState::new().with_query("async");
    state.set_focused(true);
    let _ = SearchInput::new(system)
        .status(SearchStatus::Searching)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn search_input_filters_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchInputState::new().with_query("handler");
    state.set_focused(true);
    let chips = [
        SearchFilterChip::new("ext", "rs"),
        SearchFilterChip::new("path", "src"),
    ];
    let _ = SearchInput::new(system)
        .filters(&chips)
        .status(SearchStatus::Results { count: 4 })
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn search_input_empty_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = SearchInputState::new().with_query("zzz");
    state.set_focused(true);
    let _ = SearchInput::new(system)
        .status(SearchStatus::NoResults)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn path_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PathInputState::new()
        .with_style(PathStyle::Unix)
        .with_path("/usr/local/bin");
    state.set_focused(true);
    state.set_fs_status(PathFsStatus::Directory);
    let _ = PathInput::new(system)
        .label("Install dir")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn path_input_missing_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PathInputState::new()
        .with_style(PathStyle::Unix)
        .with_expect(PathExpect::File)
        .with_path("/tmp/new-file.txt");
    state.set_focused(true);
    state.set_fs_status(PathFsStatus::Missing);
    let _ = PathInput::new(system)
        .label("Output")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn path_input_destructive_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PathInputState::new()
        .with_style(PathStyle::Unix)
        .with_path("/etc/hosts");
    state.set_focused(true);
    state.set_fs_status(PathFsStatus::File);
    state.set_risk(PathRisk::Destructive);
    let _ = PathInput::new(system)
        .label("Overwrite")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn path_input_relative_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = PathInputState::new()
        .with_style(PathStyle::Unix)
        .with_base("/Users/dev/proj")
        .with_path("src/main.rs");
    state.set_focused(true);
    state.set_fs_status(PathFsStatus::File);
    let _ = PathInput::new(system)
        .label("Source")
        .show_base(true)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn token_field_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TokenFieldState::new();
    state.set_focused(true);
    let _ = state.push_token(FieldToken::new("1".into(), "alice@ex.com"));
    let _ = state.push_token(FieldToken::new("2".into(), "bob@ex.com"));
    let _ = TokenField::new(system)
        .label("To")
        .placeholder("Add recipient…")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn token_field_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TokenFieldState::new().with_max_visible(3);
    state.set_focused(true);
    for i in 0..8 {
        let _ = state.push_token(FieldToken::new(format!("{i}"), format!("tag{i}")));
    }
    let _ = TokenField::new(system)
        .label("Tags")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn token_field_error_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TokenFieldState::new();
    state.set_focused(true);
    let _ = state.push_token(
        FieldToken::new("1".into(), "bad@")
            .status(TokenStatus::Error),
    );
    let _ = state.push_token(FieldToken::new("2".into(), "ok@ex.com"));
    let _ = TokenField::new(system)
        .label("To")
        .validation(Validation::Invalid("invalid address"))
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn token_field_multiselect_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TokenFieldState::new().with_multi_select(true);
    state.set_focused(true);
    let _ = state.push_token(FieldToken::new("rs".into(), "rust").selected(true));
    let _ = state.push_token(FieldToken::new("go".into(), "go"));
    let _ = state.push_token(FieldToken::new("ts".into(), "typescript").selected(true));
    let _ = TokenField::new(system)
        .label("Filters")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn select_demo_options() -> Vec<SelectOption<&'static str>> {
    vec![
        SelectOption::group("g1", "Fruits"),
        SelectOption::option("apple", "Apple").description("crisp"),
        SelectOption::option("banana", "Banana"),
        SelectOption::separator("s1"),
        SelectOption::group("g2", "Other"),
        SelectOption::option("date", "Date"),
    ]
}

fn select_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = select_demo_options();
    let mut state = SelectState::new()
        .with_recipe(SelectRecipe::Form)
        .with_value("apple");
    state.set_focused(true);
    frame.render_stateful_widget(
        Select::new(&opts, system).label("Fruit").ascii(true),
        area,
        &mut state,
    );
}

fn select_open_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = select_demo_options();
    let mut state = SelectState::new().with_value("apple");
    state.set_focused(true);
    let _ = state.open(area, &opts);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Down,
            termrock::input::KeyModifiers::NONE,
        ),
        &opts,
        area,
    );
    Select::new(&opts, system)
        .label("Fruit")
        .ascii(true)
        .paint_stacked(area, frame.buffer_mut(), &mut state);
}

fn select_search_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = select_demo_options();
    let mut state = SelectState::new().with_searchable(true);
    state.set_focused(true);
    let _ = state.open(area, &opts);
    let _ = state.search_query(); // ensure API
    // seed filter via public insert on open search
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('b'),
            termrock::input::KeyModifiers::NONE,
        ),
        &opts,
        area,
    );
    Select::new(&opts, system)
        .ascii(true)
        .paint_stacked(area, frame.buffer_mut(), &mut state);
}

fn select_compact_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = select_demo_options();
    let mut state = SelectState::new()
        .with_recipe(SelectRecipe::Compact)
        .with_value("banana");
    state.set_focused(true);
    frame.render_stateful_widget(
        Select::new(&opts, system).ascii(true),
        area,
        &mut state,
    );
}

fn multi_select_demo_options() -> Vec<SelectOption<&'static str>> {
    vec![
        SelectOption::group("g", "Languages"),
        SelectOption::option("rs", "Rust"),
        SelectOption::option("go", "Go"),
        SelectOption::option("ts", "TypeScript"),
        SelectOption::option("py", "Python"),
    ]
}

fn multi_select_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = multi_select_demo_options();
    let mut state = MultiSelectState::new()
        .with_recipe(SelectRecipe::Form)
        .with_selected(["rs", "go"]);
    state.set_focused(true);
    frame.render_stateful_widget(
        MultiSelect::new(&opts, system)
            .label("Filters")
            .ascii(true),
        area,
        &mut state,
    );
}

fn multi_select_open_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = multi_select_demo_options();
    let mut state = MultiSelectState::new().with_selected(["rs"]);
    state.set_focused(true);
    let _ = state.open(area, &opts);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Down,
            termrock::input::KeyModifiers::NONE,
        ),
        &opts,
        area,
    );
    MultiSelect::new(&opts, system)
        .label("Filters")
        .ascii(true)
        .paint_stacked(area, frame.buffer_mut(), &mut state);
}

fn multi_select_overflow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = multi_select_demo_options();
    let mut state = MultiSelectState::new()
        .with_selected(["rs", "go", "ts", "py"])
        .with_max_summary_chips(2);
    state.set_focused(true);
    frame.render_stateful_widget(
        MultiSelect::new(&opts, system).ascii(true),
        area,
        &mut state,
    );
}

fn multi_select_search_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let opts = multi_select_demo_options();
    let mut state = MultiSelectState::new().with_searchable(true);
    state.set_focused(true);
    let _ = state.open(area, &opts);
    let _ = state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Char('p'),
            termrock::input::KeyModifiers::NONE,
        ),
        &opts,
        area,
    );
    MultiSelect::new(&opts, system)
        .ascii(true)
        .paint_stacked(area, frame.buffer_mut(), &mut state);
}

fn file_picker_unix_entries() -> Vec<FileEntry> {
    vec![
        FileEntry::directory("d1", "src", "/home/u/proj/src"),
        FileEntry::directory("d2", "docs", "/home/u/proj/docs"),
        FileEntry::file("f1", "README.md", "/home/u/proj/README.md").size(420),
        FileEntry::file("f2", "Cargo.toml", "/home/u/proj/Cargo.toml").size(180),
        FileEntry::file("f3", ".gitignore", "/home/u/proj/.gitignore")
            .hidden(true)
            .size(40),
        FileEntry::file("f4", "secret.env", "/home/u/proj/secret.env")
            .error("permission denied"),
    ]
}

fn keybinding_recorder_idle_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::ChordFormat;
    let mut state = KeybindingRecorderState::new("app.save", "Save")
        .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
        .with_format(ChordFormat::footer())
        .with_reserved(Vec::new());
    state.set_focused(true);
    KeybindingRecorder::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn keybinding_recorder_recording_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::ChordFormat;
    let mut state = KeybindingRecorderState::new("app.save", "Save")
        .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
        .with_format(ChordFormat::footer())
        .with_sequences(true)
        .with_reserved(Vec::new());
    state.set_focused(true);
    let _ = state.start_recording();
    let _ = state.capture_chord(KeyChord::ctrl(KeyCode::Char('x')));
    KeybindingRecorder::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn keybinding_recorder_conflict_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::ChordFormat;
    let mut state = KeybindingRecorderState::new("app.find", "Find")
        .with_chords([KeyChord::plain(KeyCode::Char('/'))])
        .with_format(ChordFormat::footer())
        .with_sequences(false)
        .with_reserved(Vec::new());
    state.set_occupied(vec![(
        KeyChord::ctrl(KeyCode::Char('s')),
        "Save".into(),
    )]);
    state.set_focused(true);
    let _ = state.start_recording();
    let _ = state.capture_chord(KeyChord::ctrl(KeyCode::Char('s')));
    KeybindingRecorder::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn keybinding_recorder_reserved_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::input::KeyCode;
    use termrock::keymap::KeyChord;
    use termrock::widgets::ChordFormat;
    let mut state = KeybindingRecorderState::new("app.interrupt", "Custom interrupt")
        .with_format(ChordFormat::footer())
        .with_sequences(false);
    state.set_focused(true);
    let _ = state.start_recording();
    let _ = state.capture_chord(KeyChord::ctrl(KeyCode::Char('c')));
    KeybindingRecorder::new(system)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn date_time_picker_date_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let today = CivilDate::new(2026, 8, 10).unwrap();
    let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
        .with_date(CivilDate::new(2026, 8, 15).unwrap())
        .with_min_date(CivilDate::new(2026, 8, 1).unwrap())
        .with_max_date(CivilDate::new(2026, 8, 31).unwrap())
        .with_timezone_label("UTC");
    state.set_focused(true);
    state.set_today(today);
    let _ = state.open(area);
    DateTimePicker::new(system)
        .label("Due date")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn date_time_picker_time_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = DateTimePickerState::new(DateTimePickerKind::Time)
        .with_time(CivilTime::new(9, 30, 0).unwrap())
        .with_time_step_minutes(30)
        .with_time_format(TimeDisplayFormat::Hm24)
        .with_timezone_label("America/New_York");
    state.set_focused(true);
    let _ = state.open(area);
    DateTimePicker::new(system)
        .label("Start time")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn date_time_picker_range_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let start = CivilDate::new(2026, 8, 5).unwrap();
    let end = CivilDate::new(2026, 8, 12).unwrap();
    let mut state = DateTimePickerState::new(DateTimePickerKind::DateRange)
        .with_range(CivilDateRange::new(start, end))
        .with_timezone_label("UTC");
    state.set_focused(true);
    state.set_today(CivilDate::new(2026, 8, 10).unwrap());
    let _ = state.open(area);
    DateTimePicker::new(system)
        .label("Window")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn date_time_picker_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
        .with_date(CivilDate::new(2026, 8, 10).unwrap());
    state.set_focused(true);
    state.set_today(CivilDate::new(2026, 8, 10).unwrap());
    let _ = state.open(area);
    DateTimePicker::new(system)
        .label("Day")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn file_picker_seed(
    state: &mut FilePickerState,
    cwd: &str,
    entries: Vec<FileEntry>,
) {
    match state.request_list(cwd) {
        termrock::widgets::FilePickerOutcome::ListRequested { generation, .. } => {
            assert!(state.apply_listing(generation, cwd, entries, None));
        }
        other => panic!("expected ListRequested, got {other:?}"),
    }
}

fn file_picker_unix_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FilePickerState::new("/home/u/proj")
        .with_mode(FilePickerMode::OpenFile)
        .with_preview(true)
        .with_path_style(PathStyle::Unix);
    state.set_focused(true);
    file_picker_seed(&mut state, "/home/u/proj", file_picker_unix_entries());
    state.apply_preview(FilePreview::text(
        "README.md",
        ["# proj".into(), "".into(), "Hello from Unix FS host.".into()],
    ));
    FilePicker::new(system)
        .title("Open file")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn file_picker_windows_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FilePickerState::new(r"C:\Users\me")
        .with_mode(FilePickerMode::OpenAny)
        .with_path_style(PathStyle::Windows)
        .with_preview(true);
    state.set_focused(true);
    let entries = vec![
        FileEntry::directory("d1", "Projects", r"C:\Users\me\Projects"),
        FileEntry::directory("d2", "Desktop", r"C:\Users\me\Desktop"),
        FileEntry::file("f1", "notes.txt", r"C:\Users\me\notes.txt").size(88),
        FileEntry::file("f2", "photo.jpg", r"C:\Users\me\photo.jpg").size(4096),
    ];
    file_picker_seed(&mut state, r"C:\Users\me", entries);
    state.apply_preview(FilePreview::text("notes.txt", ["todo: ship FilePicker".into()]));
    FilePicker::new(system)
        .title("Browse")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn file_picker_ssh_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    // Host projects remote paths as plain strings; picker stays FS-agnostic.
    let mut state = FilePickerState::new("ssh://host/home/u")
        .with_mode(FilePickerMode::OpenDirectory)
        .with_preview(true)
        .with_show_hidden(false);
    state.set_focused(true);
    let entries = vec![
        FileEntry::directory("d1", "bin", "ssh://host/home/u/bin"),
        FileEntry::directory("d2", "etc", "ssh://host/home/u/etc"),
        FileEntry::file("f1", "authorized_keys", "ssh://host/home/u/authorized_keys")
            .size(512)
            .kind(FileEntryKind::File),
        FileEntry::file("f2", ".ssh", "ssh://host/home/u/.ssh")
            .hidden(true)
            .kind(FileEntryKind::Directory),
    ];
    file_picker_seed(&mut state, "ssh://host/home/u", entries);
    state.apply_preview(FilePreview::text(
        "remote://host",
        [
            "provider: ssh".into(),
            "latency: 42ms".into(),
            "listing via host cancel token".into(),
        ],
    ));
    FilePicker::new(system)
        .title("Remote path")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn file_picker_no_preview_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = FilePickerState::new("/var/log")
        .with_mode(FilePickerMode::OpenFile)
        .with_preview(false)
        .with_multi(true)
        .with_sort(FileSortKey::Name);
    state.set_focused(true);
    let entries = vec![
        FileEntry::file("a", "syslog", "/var/log/syslog").size(2048),
        FileEntry::file("b", "kern.log", "/var/log/kern.log").size(1024),
        FileEntry::file("c", "auth.log", "/var/log/auth.log").size(512),
        FileEntry::directory("d", "journal", "/var/log/journal"),
    ];
    file_picker_seed(&mut state, "/var/log", entries);
    FilePicker::new(system)
        .title("Pick logs")
        .show_preview(false)
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn combobox_candidates() -> Vec<CompletionCandidate<'static, &'static str>> {
    vec![
        CompletionCandidate::new("rs", "Rust").kind("lang"),
        CompletionCandidate::new("go", "Go").kind("lang"),
        CompletionCandidate::new("ts", "TypeScript").kind("lang"),
    ]
}

fn combobox_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state: ComboboxState<&'static str> = ComboboxState::new()
        .with_creatable(false)
        .with_exact_required(true);
    state.set_focused(true);
    state.set_value(Some("rs"), Some("Rust".into()));
    state.set_draft("Rust");
    let _ = Combobox::new(system)
        .label("Language")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn combobox_open_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cands = combobox_candidates();
    let mut state: ComboboxState<&'static str> = ComboboxState::new();
    state.set_focused(true);
    let _ = state.insert_str("R");
    let g = state.suggestion_generation();
    let _ = state.apply_suggestions(g, &cands);
    Combobox::new(system)
        .label("Language")
        .ascii(true)
        .paint_with_menu(area, frame.buffer_mut(), &mut state, &cands);
}

fn combobox_loading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state: ComboboxState<&'static str> = ComboboxState::new();
    state.set_focused(true);
    let _ = state.insert_str("asyn");
    state.mark_loading();
    assert_eq!(state.suggestion_status(), SuggestionStatus::Loading);
    let _ = Combobox::new(system)
        .label("Search")
        .ascii(true)
        .paint(area, frame.buffer_mut(), &mut state);
}

fn autocomplete_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let cands = combobox_candidates();
    let mut state: ComboboxState<&'static str> = ComboboxState::autocomplete();
    state.set_focused(true);
    let _ = state.insert_str("G");
    let g = state.suggestion_generation();
    let _ = state.apply_suggestions(g, &cands);
    Combobox::new(system)
        .label("Complete")
        .placeholder("Type freely…")
        .ascii(true)
        .paint_with_menu(area, frame.buffer_mut(), &mut state, &cands);
}

fn text_input_invalid_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("taken");
    state.set_focused(true);
    let _ = TextInput::new("Username", system)
        .validation(Validation::Invalid("already taken"))
        .paint(area, frame.buffer_mut(), &mut state);
}

fn text_input_prefix_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("termrock");
    state.set_focused(true);
    let _ = TextInput::new("URL", system)
        .prefix("https://")
        .suffix(".dev")
        .paint(area, frame.buffer_mut(), &mut state);
}

fn completion_menu_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    completion_menu_basic(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "候補: 関数名 🔍",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Input),
        );
    }
}

fn virtual_grid_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    virtual_grid_basic(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y,
            "列: 名称 ✨",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::TextStrong),
        );
    }
}

fn transcript_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    transcript_basic(frame, area, system);
    if area.width > 2 && area.height > 1 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            "ユーザー: こんにちは 👋",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Text),
        );
    }
}

fn progress_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    progress(frame, area, system);
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            "処理中 ⏳ 62%",
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
    }
}

fn paint_app_shell_slots(
    frame: &mut Frame<'_>,
    slots: &termrock::patterns::AppShellSlots,
    system: &DesignSystem,
    title: &str,
) {
    let mut paint = |label: &str, rect: Option<Rect>, chrome: PanelChrome| {
        let Some(r) = rect.filter(|r| r.width > 0 && r.height > 0) else {
            return;
        };
        let dim = format!("{}×{}", r.width, r.height);
        frame.render_widget(
            Panel::new(system).title(label).subtitle(&dim).chrome(chrome),
            r,
        );
        if r.height > 2 && r.width > 4 {
            let inner = Rect::new(
                r.x.saturating_add(1),
                r.y.saturating_add(1),
                r.width.saturating_sub(2),
                r.height.saturating_sub(2),
            );
            frame.buffer_mut().set_stringn(
                inner.x,
                inner.y,
                label,
                usize::from(inner.width),
                system.style(Role::TextMuted),
            );
        }
    };
    paint("header", slots.header, PanelChrome::Normal);
    paint("sidebar", slots.sidebar, PanelChrome::Normal);
    paint("main", Some(slots.main), PanelChrome::Focused);
    paint("inspector", slots.inspector, PanelChrome::Normal);
    paint("command", slots.command, PanelChrome::Normal);
    paint("metrics", slots.metrics, PanelChrome::Normal);
    paint("log", slots.log, PanelChrome::Normal);
    paint("footer", slots.footer, PanelChrome::Normal);
    // Caption of recipe / drawers / focus order over main when room.
    if slots.main.height > 3 && slots.main.width > 12 {
        let drawers: String = slots
            .drawer_zones
            .iter()
            .map(|z| z.id())
            .collect::<Vec<_>>()
            .join(",");
        let focus: String = slots
            .focus_order
            .iter()
            .map(|z| z.id())
            .collect::<Vec<_>>()
            .join(">");
        let caption = format!(
            "{title} · {} · drawers=[{drawers}] · focus={focus}",
            slots.recipe.id(),
        );
        frame.buffer_mut().set_stringn(
            slots.main.x.saturating_add(2),
            slots.main.y.saturating_add(2),
            caption,
            usize::from(slots.main.width.saturating_sub(4)),
            system.style(Role::TextMuted),
        );
    }
}

fn app_shell_workbench_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig};
    let slots = layout_app_shell(area, AppShellConfig::workbench());
    paint_app_shell_slots(frame, &slots, system, "workbench");
}

fn app_shell_dashboard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig};
    let slots = layout_app_shell(area, AppShellConfig::dashboard());
    paint_app_shell_slots(frame, &slots, system, "dashboard");
}

fn app_shell_master_detail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig};
    let slots = layout_app_shell(area, AppShellConfig::master_detail());
    paint_app_shell_slots(frame, &slots, system, "master-detail");
}

fn app_shell_minimal_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig};
    let slots = layout_app_shell(area, AppShellConfig::minimal());
    paint_app_shell_slots(frame, &slots, system, "minimal");
}

fn app_shell_narrow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig};
    let slots = layout_app_shell(area, AppShellConfig::workbench());
    paint_app_shell_slots(frame, &slots, system, "narrow");
}

fn app_shell_offline_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{layout_app_shell, AppShellConfig, AppShellLifecycle};
    let mut cfg = AppShellConfig::workbench();
    cfg.lifecycle = AppShellLifecycle::Offline;
    let slots = layout_app_shell(area, cfg);
    paint_app_shell_slots(frame, &slots, system, "offline");
}

/// Shared elevated workbench paint for Studio stories.
fn paint_agent_workbench_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    story: AgentWorkbenchStoryKind,
) {
    use termrock::patterns::{
        AgentWorkbenchState, WorkbenchDensity, WorkbenchSurfaces, default_modes,
        example_workbench_activities, example_workbench_tasks, render_agent_workbench,
    };
    use termrock::widgets::{
        DiffLine, DiffReview, ListRow, PermissionPrompt, PermissionPromptState, PermissionRequest,
        PermissionRisk, PlanReview, PromptComposer, PromptComposerState, SessionPicker,
        StatusBarState, StatusSlot, Transcript, TranscriptBlock, TranscriptKind, TranscriptState,
        WorkingStateCard, example_plan_document, example_sessions, example_working_state,
    };

    let mut workbench = AgentWorkbenchState::new();
    match story {
        AgentWorkbenchStoryKind::Narrow => {
            workbench.density = Some(WorkbenchDensity::Narrow);
        }
        AgentWorkbenchStoryKind::Tiny => {
            workbench.density = Some(WorkbenchDensity::Tiny);
        }
        AgentWorkbenchStoryKind::Ascii => {
            workbench.ascii = true;
        }
        AgentWorkbenchStoryKind::NoColor => {
            workbench.colorless = true;
        }
        _ => {}
    }

    let user = ["Plan the cutover", "Compose public TermRock only"];
    let tool = ["cargo test --lib agent_workbench", "ok"];
    let asst = ["Tool finished; ready for review"];
    let blocks = [
        TranscriptBlock::new("b1", TranscriptKind::User, &user),
        TranscriptBlock::new("b2", TranscriptKind::Tool, &tool),
        TranscriptBlock::new("b3", TranscriptKind::Assistant, &asst),
    ];
    let transcript = Transcript::new(&blocks, system);
    let mut tstate = TranscriptState::new();
    let prompt = PromptComposer::new(system);
    let mut pstate = PromptComposerState::new();
    pstate.set_text("draft survives overlays");
    let modes = default_modes(match story {
        AgentWorkbenchStoryKind::Plan => "plan",
        _ => "build",
    });
    let slots = [StatusSlot::connection("s", "ready")];
    let mut sstate = StatusBarState::default();
    let models = example_workbench_tasks();
    let activities = example_workbench_activities();
    let legacy: [ListRow<'_, &str>; 0] = [];

    let mut perm_state = PermissionPromptState::new();
    let perm_w = PermissionPrompt::new(system);
    if matches!(story, AgentWorkbenchStoryKind::Permission) {
        let _ = perm_state.enqueue(
            PermissionRequest::new("r1", "bash", "workspace")
                .risk(PermissionRisk::High)
                .command("cargo test --all-features")
                .expected("tests pass"),
        );
    }

    let plan_w = PlanReview::new(system);
    if matches!(story, AgentWorkbenchStoryKind::Plan) {
        workbench.plan.open(example_plan_document());
    }

    let diff_lines = [
        DiffLine::context("1", " fn main() {"),
        DiffLine::removed("2", "-    let x = 1;"),
        DiffLine::added("3", "+    let x = 2;"),
        DiffLine::context("4", " }"),
    ];
    let diff_w = DiffReview::new(&diff_lines, system);

    let sessions = example_sessions();
    if matches!(story, AgentWorkbenchStoryKind::Session) {
        workbench.session.set_sessions(sessions.clone());
        workbench.set_session_open(true);
    }
    let session_w = SessionPicker::new(system);

    if matches!(
        story,
        AgentWorkbenchStoryKind::ToolRunning | AgentWorkbenchStoryKind::MultiAgent
    ) {
        workbench.working.set_work(Some(example_working_state()));
    }
    let working_w = WorkingStateCard::new(system);

    let use_elevated = !matches!(
        story,
        AgentWorkbenchStoryKind::Tiny | AgentWorkbenchStoryKind::Narrow
    );

    render_agent_workbench(
        frame.buffer_mut(),
        area,
        WorkbenchSurfaces {
            system,
            state: &mut workbench,
            task_models: if use_elevated {
                Some(models.as_slice())
            } else {
                None
            },
            tasks: &legacy,
            modes: &modes,
            transcript: &transcript,
            transcript_state: &mut tstate,
            activities: if use_elevated || matches!(story, AgentWorkbenchStoryKind::Narrow) {
                Some(activities.as_slice())
            } else {
                None
            },
            prompt: &prompt,
            prompt_state: &mut pstate,
            status_slots: &slots,
            status_state: &mut sstate,
            permission: if matches!(story, AgentWorkbenchStoryKind::Permission) {
                Some((&perm_w, &mut perm_state))
            } else {
                None
            },
            question: None,
            plan: if matches!(story, AgentWorkbenchStoryKind::Plan) {
                Some(&plan_w)
            } else {
                None
            },
            diff: if matches!(story, AgentWorkbenchStoryKind::Diff) {
                Some(&diff_w)
            } else {
                None
            },
            session: if matches!(story, AgentWorkbenchStoryKind::Session) {
                Some(&session_w)
            } else {
                None
            },
            working: if matches!(
                story,
                AgentWorkbenchStoryKind::ToolRunning | AgentWorkbenchStoryKind::MultiAgent
            ) {
                Some(&working_w)
            } else {
                None
            },
        },
    );
}

#[derive(Clone, Copy)]
enum AgentWorkbenchStoryKind {
    Basic,
    ToolRunning,
    Permission,
    Plan,
    Diff,
    Session,
    MultiAgent,
    Narrow,
    Tiny,
    Ascii,
    NoColor,
}

fn agent_workbench_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Basic);
}

fn agent_workbench_tool_running(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::ToolRunning);
}

fn agent_workbench_permission(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Permission);
}

fn agent_workbench_plan(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Plan);
}

fn agent_workbench_diff(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Diff);
}

fn agent_workbench_session(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Session);
}

fn agent_workbench_multi_agent(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::MultiAgent);
}

fn agent_workbench_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Narrow);
}

fn agent_workbench_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Tiny);
}

fn agent_workbench_ascii(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::Ascii);
}

fn agent_workbench_no_color(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_agent_workbench_story(frame, area, system, AgentWorkbenchStoryKind::NoColor);
}

#[derive(Clone, Copy)]
enum DatabaseWorkbenchStoryKind {
    Basic,
    Disconnected,
    Error,
    Running,
    Narrow,
    Unicode,
}

fn paint_database_workbench_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: DatabaseWorkbenchStoryKind,
) {
    use termrock::patterns::{
        example_db_commands, example_db_history, example_disconnected_connections,
        example_inspect_fields, example_result_columns, example_result_row_refs,
        example_result_rows, example_schema_entries, render_database_workbench,
        DatabaseConnGate, DatabaseTxStatus, DatabaseWorkbenchDensity, DatabaseWorkbenchState,
        DatabaseWorkbenchSurfaces,
    };
    use termrock::widgets::SchemaBrowserEntry;

    let mut state = DatabaseWorkbenchState::new();
    match kind {
        DatabaseWorkbenchStoryKind::Narrow => {
            state.density = Some(DatabaseWorkbenchDensity::Narrow);
        }
        DatabaseWorkbenchStoryKind::Disconnected => {
            state
                .connections
                .set_connections(example_disconnected_connections());
            state.sync_conn_gate_from_selection();
        }
        DatabaseWorkbenchStoryKind::Error => {
            state.conn_gate = DatabaseConnGate::Connected;
            state.finish_run_error("syntax error near FROM");
            state.set_tx_status(DatabaseTxStatus::Failed);
        }
        DatabaseWorkbenchStoryKind::Running => {
            state.conn_gate = DatabaseConnGate::Connected;
            state.begin_run("story-run-1");
            state.set_tx_status(DatabaseTxStatus::Active);
        }
        DatabaseWorkbenchStoryKind::Unicode => {
            state.tabs[0].title = "ユーザー".into();
            state.tabs[0].draft = "SELECT * FROM ユーザー LIMIT 10;".into();
            state.query.set_text(&state.tabs[0].draft);
            state.query.title = Some("ユーザー".into());
        }
        DatabaseWorkbenchStoryKind::Basic => {
            state.conn_gate = DatabaseConnGate::Connected;
            state.finish_run_success(3, 12);
        }
    }

    let schema = if matches!(kind, DatabaseWorkbenchStoryKind::Unicode) {
        vec![
            SchemaBrowserEntry::connection("c", "本番", "prod")
                .branch()
                .expanded(),
            SchemaBrowserEntry::table("t", "ユーザー", "prod/ユーザー", 1)
                .parent("c")
                .secondary("≈1万"),
        ]
    } else {
        example_schema_entries()
    };
    let cols = example_result_columns();
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
            system,
            state: &mut state,
            schema_entries: &schema,
            result_columns: &cols,
            result_rows: &rows,
            inspect_fields: &inspect,
            history: &history,
            commands: &commands,
        },
    );
}

fn database_workbench_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Basic);
}

fn database_workbench_disconnected(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Disconnected);
}

fn database_workbench_error(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Error);
}

fn database_workbench_running(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Running);
}

fn database_workbench_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Narrow);
}

fn database_workbench_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_database_workbench_story(frame, area, system, DatabaseWorkbenchStoryKind::Unicode);
}

#[derive(Clone, Copy)]
enum GitWorkbenchStoryKind {
    Basic,
    Conflict,
    Narrow,
    Fullscreen,
    Unicode,
    Clean,
    Empty,
}

fn paint_git_workbench_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: GitWorkbenchStoryKind,
) {
    use termrock::patterns::{
        example_clean_files, example_conflict_diagnostics, example_conflict_files,
        example_empty_files, example_git_commits, example_git_diff_files, example_git_diff_lines,
        example_git_files, example_git_help_entries, example_git_hunks, example_git_terminal_lines,
        example_git_terminal_meta, render_git_workbench, GitRepoStatus, GitWorkbenchDensity,
        GitWorkbenchState, GitWorkbenchSurfaces,
    };

    let mut state = GitWorkbenchState::new();
    match kind {
        GitWorkbenchStoryKind::Narrow => {
            state.density = Some(GitWorkbenchDensity::Narrow);
            state.repo_status = GitRepoStatus::Dirty;
        }
        GitWorkbenchStoryKind::Conflict => {
            state.repo_status = GitRepoStatus::Conflict;
            state.focus = "diagnostics";
        }
        GitWorkbenchStoryKind::Fullscreen => {
            state.repo_status = GitRepoStatus::Dirty;
            let _ = state.set_fullscreen_diff(true);
        }
        GitWorkbenchStoryKind::Clean | GitWorkbenchStoryKind::Empty => {
            state.repo_status = GitRepoStatus::Clean;
        }
        GitWorkbenchStoryKind::Unicode | GitWorkbenchStoryKind::Basic => {
            state.repo_status = GitRepoStatus::Dirty;
        }
    }

    let files = match kind {
        GitWorkbenchStoryKind::Conflict => example_conflict_files(),
        GitWorkbenchStoryKind::Clean => example_clean_files(),
        GitWorkbenchStoryKind::Empty => example_empty_files(),
        _ => example_git_files(),
    };
    let lines = if matches!(kind, GitWorkbenchStoryKind::Empty | GitWorkbenchStoryKind::Clean) {
        vec![]
    } else {
        example_git_diff_lines()
    };
    let hunks = if matches!(kind, GitWorkbenchStoryKind::Empty | GitWorkbenchStoryKind::Clean) {
        vec![]
    } else {
        example_git_hunks()
    };
    let dfiles = if matches!(kind, GitWorkbenchStoryKind::Empty | GitWorkbenchStoryKind::Clean) {
        vec![]
    } else {
        example_git_diff_files()
    };
    let commits = example_git_commits();
    let diags = if matches!(kind, GitWorkbenchStoryKind::Conflict) {
        example_conflict_diagnostics()
    } else {
        vec![]
    };
    let meta = example_git_terminal_meta();
    let tlines = example_git_terminal_lines();
    let help = example_git_help_entries(system);

    render_git_workbench(
        frame.buffer_mut(),
        area,
        GitWorkbenchSurfaces {
            system,
            state: &mut state,
            files: &files,
            diff_lines: &lines,
            hunks: &hunks,
            diff_files: &dfiles,
            commits: &commits,
            diagnostics: &diags,
            terminal_meta: &meta,
            terminal_lines: &tlines,
            help_entries: &help,
        },
    );
}

fn git_workbench_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Basic);
}

fn git_workbench_conflict(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Conflict);
}

fn git_workbench_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Narrow);
}

fn git_workbench_fullscreen(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Fullscreen);
}

fn git_workbench_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Unicode);
}

fn git_workbench_clean(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Clean);
}

fn git_workbench_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_git_workbench_story(frame, area, system, GitWorkbenchStoryKind::Empty);
}

#[derive(Clone, Copy)]
enum ObservabilityStoryKind {
    Basic,
    Failure,
    Narrow,
    Unicode,
}

fn paint_observability_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: ObservabilityStoryKind,
) {
    use termrock::patterns::{
        example_log_inspect_fields, example_observability_alerts, example_observability_events,
        example_observability_logs, example_observability_tiles, render_observability_dashboard,
        seed_failure_state, ObservabilityDashboardState, ObservabilityDashboardSurfaces,
        ObservabilityDensity, ObservabilityLiveState,
    };

    let mut state = ObservabilityDashboardState::new();
    match kind {
        ObservabilityStoryKind::Narrow => {
            state.density = Some(ObservabilityDensity::Narrow);
        }
        ObservabilityStoryKind::Failure => {
            seed_failure_state(&mut state);
        }
        ObservabilityStoryKind::Unicode | ObservabilityStoryKind::Basic => {
            state.live = ObservabilityLiveState::Live;
        }
    }

    let logs = example_observability_logs();
    let events = example_observability_events();
    let tiles = example_observability_tiles();
    let alerts = example_observability_alerts();
    let inspect = example_log_inspect_fields();

    render_observability_dashboard(
        frame.buffer_mut(),
        area,
        ObservabilityDashboardSurfaces {
            system,
            state: &mut state,
            logs: &logs,
            events: &events,
            tiles: &tiles,
            alerts: &alerts,
            inspect_fields: &inspect,
        },
    );
}

fn observability_dashboard_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_observability_story(frame, area, system, ObservabilityStoryKind::Basic);
}

fn observability_dashboard_failure(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_observability_story(frame, area, system, ObservabilityStoryKind::Failure);
}

fn observability_dashboard_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_observability_story(frame, area, system, ObservabilityStoryKind::Narrow);
}

fn observability_dashboard_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_observability_story(frame, area, system, ObservabilityStoryKind::Unicode);
}

#[derive(Clone, Copy)]
enum FileManagerStoryKind {
    Basic,
    Conflict,
    Narrow,
    Unicode,
}

fn paint_file_manager_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: FileManagerStoryKind,
) {
    use termrock::patterns::{
        example_file_entries, example_file_ops, example_file_preview,
        example_quick_open_from_entries, render_file_manager, seed_conflict_state,
        FileManagerDensity, FileManagerState, FileManagerSurfaces,
    };

    let mut state = FileManagerState::new();
    state.cwd = "/project".into();
    match kind {
        FileManagerStoryKind::Narrow => {
            state.density = Some(FileManagerDensity::Narrow);
            state.drawer_open = true;
        }
        FileManagerStoryKind::Conflict => {
            seed_conflict_state(&mut state);
        }
        FileManagerStoryKind::Unicode | FileManagerStoryKind::Basic => {}
    }

    let entries = example_file_entries();
    let ops = example_file_ops();
    let (preview, _, _) = example_file_preview();
    let qo = example_quick_open_from_entries(&entries);

    render_file_manager(
        frame.buffer_mut(),
        area,
        FileManagerSurfaces {
            system,
            state: &mut state,
            entries: &entries,
            ops: &ops,
            preview: Some(preview),
            quick_open_items: &qo,
        },
    );
}

fn file_manager_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_file_manager_story(frame, area, system, FileManagerStoryKind::Basic);
}

fn file_manager_conflict(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_file_manager_story(frame, area, system, FileManagerStoryKind::Conflict);
}

fn file_manager_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_file_manager_story(frame, area, system, FileManagerStoryKind::Narrow);
}

fn file_manager_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_file_manager_story(frame, area, system, FileManagerStoryKind::Unicode);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectLauncherStoryKind {
    Basic,
    Stale,
    Narrow,
    Inline,
    Unicode,
}

fn paint_project_launcher_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: ProjectLauncherStoryKind,
) {
    use termrock::patterns::{
        example_project_preview, example_project_quick_open, example_projects,
        render_project_launcher, seed_onboarding_state, seed_stale_state, ProjectLauncherDensity,
        ProjectLauncherMode, ProjectLauncherState, ProjectLauncherSurfaces,
    };
    use termrock::widgets::example_sessions;

    let mut state = match kind {
        ProjectLauncherStoryKind::Inline => ProjectLauncherState::inline(),
        _ => ProjectLauncherState::new(),
    };
    match kind {
        ProjectLauncherStoryKind::Narrow => {
            state.density = Some(ProjectLauncherDensity::Narrow);
        }
        ProjectLauncherStoryKind::Stale => {
            seed_stale_state(&mut state);
        }
        ProjectLauncherStoryKind::Inline => {
            state.mode = ProjectLauncherMode::Inline;
        }
        ProjectLauncherStoryKind::Unicode | ProjectLauncherStoryKind::Basic => {}
    }
    // empty catalog shows onboarding only when host enables it
    if kind == ProjectLauncherStoryKind::Basic {
        seed_onboarding_state(&mut state);
        state.show_onboarding = false; // keep basic clean; stale uses problems
    }

    let projects = example_projects();
    let sessions = example_sessions();
    let (preview, _, _) = example_project_preview();
    let qo = example_project_quick_open(&projects);

    render_project_launcher(
        frame.buffer_mut(),
        area,
        ProjectLauncherSurfaces {
            system,
            state: &mut state,
            projects: &projects,
            sessions: &sessions,
            preview: Some(preview),
            quick_open_items: &qo,
        },
    );
}

fn project_launcher_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_project_launcher_story(frame, area, system, ProjectLauncherStoryKind::Basic);
}

fn project_launcher_stale(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_project_launcher_story(frame, area, system, ProjectLauncherStoryKind::Stale);
}

fn project_launcher_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_project_launcher_story(frame, area, system, ProjectLauncherStoryKind::Narrow);
}

fn project_launcher_inline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_project_launcher_story(frame, area, system, ProjectLauncherStoryKind::Inline);
}

fn project_launcher_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_project_launcher_story(frame, area, system, ProjectLauncherStoryKind::Unicode);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HelpCenterStoryKind {
    Basic,
    Compact,
    Narrow,
    Doctor,
    Unicode,
}

fn paint_help_center_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: HelpCenterStoryKind,
) {
    use termrock::patterns::{
        command_entries_from_help, example_help_center_entries, example_help_doctor_report,
        example_help_topics, render_help_center, seed_compact_mode, seed_diagnostics_state,
        HelpCenterDensity, HelpCenterState, HelpCenterSurfaces,
    };

    let mut state = match kind {
        HelpCenterStoryKind::Compact => HelpCenterState::compact(),
        _ => HelpCenterState::new(),
    };
    match kind {
        HelpCenterStoryKind::Narrow => {
            state.density = Some(HelpCenterDensity::Narrow);
        }
        HelpCenterStoryKind::Compact => {
            seed_compact_mode(&mut state);
        }
        HelpCenterStoryKind::Doctor => {
            seed_diagnostics_state(&mut state);
        }
        HelpCenterStoryKind::Basic | HelpCenterStoryKind::Unicode => {}
    }
    if kind == HelpCenterStoryKind::Unicode {
        state.selected_topic = Some("unicode".into());
    } else {
        state.selected_topic = Some("getting-started".into());
    }

    let topics = example_help_topics();
    let help = example_help_center_entries(system);
    let cmds = command_entries_from_help(&help);
    let doctor = example_help_doctor_report();
    let components = vec!["keyboard-help".into(), "command-palette".into()];

    render_help_center(
        frame.buffer_mut(),
        area,
        HelpCenterSurfaces {
            system,
            state: &mut state,
            topics: &topics,
            help_entries: &help,
            commands: &cmds,
            doctor: Some(&doctor),
            component_ids: &components,
        },
    );
}

fn help_center_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_help_center_story(frame, area, system, HelpCenterStoryKind::Basic);
}

fn help_center_compact(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_help_center_story(frame, area, system, HelpCenterStoryKind::Compact);
}

fn help_center_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_help_center_story(frame, area, system, HelpCenterStoryKind::Narrow);
}

fn help_center_doctor(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_help_center_story(frame, area, system, HelpCenterStoryKind::Doctor);
}

fn help_center_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_help_center_story(frame, area, system, HelpCenterStoryKind::Unicode);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ErrorRecoveryStoryKind {
    Basic,
    Redacted,
    Inline,
    Unicode,
}

fn paint_error_recovery_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: ErrorRecoveryStoryKind,
) {
    use termrock::patterns::{
        example_crash_snapshot_with_secrets, example_recovery_snapshot, render_error_recovery,
        seed_inline_fallback, seed_terminal_restore_failed, CrashReportSnapshot,
        ErrorRecoveryMode, ErrorRecoveryState, ErrorRecoverySurfaces, FailureClass,
    };

    let mut state = ErrorRecoveryState::new();
    let mut snap = match kind {
        ErrorRecoveryStoryKind::Redacted => example_crash_snapshot_with_secrets(),
        ErrorRecoveryStoryKind::Basic => example_recovery_snapshot(),
        ErrorRecoveryStoryKind::Inline => example_crash_snapshot_with_secrets(),
        ErrorRecoveryStoryKind::Unicode => CrashReportSnapshot {
            summary: "予期しないエラー · unexpected".into(),
            technical: "panic at 日本語 path".into(),
            source: "termrock".into(),
            preserved_note: "ドラフト保持".into(),
            work_preserved: true,
            env_lines: vec!["TERM=xterm-256color".into()],
            log_lines: vec!["INFO 日本語 log".into()],
            capabilities_text: String::new(),
            class: FailureClass::Crash,
        },
    };
    match kind {
        ErrorRecoveryStoryKind::Inline => {
            seed_inline_fallback(&mut state);
        }
        ErrorRecoveryStoryKind::Redacted => {
            seed_terminal_restore_failed(&mut state);
        }
        ErrorRecoveryStoryKind::Basic | ErrorRecoveryStoryKind::Unicode => {
            state.mode = ErrorRecoveryMode::Full;
        }
    }
    let _ = &mut snap;

    render_error_recovery(
        frame.buffer_mut(),
        area,
        ErrorRecoverySurfaces {
            system,
            state: &mut state,
            snapshot: &snap,
            doctor: None,
        },
    );
}

fn error_recovery_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_error_recovery_story(frame, area, system, ErrorRecoveryStoryKind::Basic);
}

fn error_recovery_redacted(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_error_recovery_story(frame, area, system, ErrorRecoveryStoryKind::Redacted);
}

fn error_recovery_inline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_error_recovery_story(frame, area, system, ErrorRecoveryStoryKind::Inline);
}

fn error_recovery_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_error_recovery_story(frame, area, system, ErrorRecoveryStoryKind::Unicode);
}

fn input_otp_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{InputOtp, InputOtpState};
    let mut st = InputOtpState::new(6);
    let _ = st.set_value("42");
    InputOtp::new(system)
        .label("One-time code")
        .paint(area, frame.buffer_mut(), &st);
}

fn carousel_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_carousel_slides, Carousel, CarouselState};
    let slides = example_carousel_slides();
    let st = CarouselState::new();
    Carousel::new(&slides, system).paint(area, frame.buffer_mut(), &st);
}

fn input_group_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{example_url_input_addons, InputGroup, InputGroupState};
    let addons = example_url_input_addons();
    let mut st = InputGroupState::new();
    st.set_focused(true);
    st.set_value("example.com");
    InputGroup::new(&addons, system)
        .placeholder("host")
        .paint(area, frame.buffer_mut(), &mut st);
}

#[derive(Clone, Copy)]
enum SettingsScreenStoryKind {
    Basic,
    Search,
    Validation,
    Conflicts,
    Theme,
    Keybinding,
    Narrow,
    Tiny,
    NoResults,
    Help,
}

fn paint_settings_screen_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: SettingsScreenStoryKind,
) {
    use termrock::patterns::{
        example_settings_appearance_fields, example_settings_categories,
        example_settings_keys_fields, example_settings_profile_fields, filter_settings_nav,
        render_settings_screen, SettingsBodyMode, SettingsDensity, SettingsRegion,
        SettingsScreenState, SettingsScreenSurfaces,
    };
    use termrock::widgets::{
        Fieldset, KeybindingRecorderState, StatusBarState, BUILTIN_THEME_PRESETS,
    };

    let mut state = SettingsScreenState::<&str>::new();
    let mut nav = example_settings_categories();
    let appearance = example_settings_appearance_fields();
    let profile = example_settings_profile_fields();
    let keys = example_settings_keys_fields();
    let mut sstate = StatusBarState::default();

    let (fieldsets, title, body_mode): (Vec<Fieldset<'_, &str>>, &str, SettingsBodyMode) =
        match kind {
            SettingsScreenStoryKind::Basic | SettingsScreenStoryKind::Search => {
                state.region = SettingsRegion::Body;
                let _ = state.select_section("appearance");
                (
                    vec![Fieldset::new("Appearance", &appearance)],
                    "Appearance",
                    SettingsBodyMode::Form,
                )
            }
            SettingsScreenStoryKind::Validation => {
                let _ = state.select_section("profile");
                (
                    vec![Fieldset::new("Profile", &profile)],
                    "Profile",
                    SettingsBodyMode::Form,
                )
            }
            SettingsScreenStoryKind::Conflicts => {
                let _ = state.select_section("tools");
                state.has_conflicts = true;
                state.restart_required = true;
                state.dirty = true;
                (
                    vec![
                        Fieldset::new("Keys", &keys),
                        Fieldset::new("Appearance", &appearance),
                    ],
                    "Keys & chrome",
                    SettingsBodyMode::Form,
                )
            }
            SettingsScreenStoryKind::Theme => {
                let _ = state.select_section("appearance");
                (vec![], "Theme", SettingsBodyMode::Theme)
            }
            SettingsScreenStoryKind::Keybinding => {
                let _ = state.select_section("tools");
                state.keybinding = KeybindingRecorderState::new("submit", "Submit chord");
                (vec![], "Keybindings", SettingsBodyMode::Keybinding)
            }
            SettingsScreenStoryKind::Narrow => {
                state.density = Some(SettingsDensity::Narrow);
                let _ = state.select_section("appearance");
                state.drawer_open = true;
                (
                    vec![Fieldset::new("Appearance", &appearance)],
                    "Appearance",
                    SettingsBodyMode::Form,
                )
            }
            SettingsScreenStoryKind::Tiny => {
                state.density = Some(SettingsDensity::Tiny);
                let _ = state.select_section("appearance");
                (
                    vec![Fieldset::new("Appearance", &appearance)],
                    "Appearance",
                    SettingsBodyMode::Form,
                )
            }
            SettingsScreenStoryKind::NoResults => {
                state.body_mode = SettingsBodyMode::NoResults;
                state.search.set_query("zzzz-nope");
                (vec![], "Search", SettingsBodyMode::NoResults)
            }
            SettingsScreenStoryKind::Help => {
                state.help_open = true;
                let _ = state.select_section("appearance");
                (
                    vec![Fieldset::new("Appearance", &appearance)],
                    "Appearance",
                    SettingsBodyMode::Form,
                )
            }
        };

    state.body_mode = body_mode;
    if matches!(kind, SettingsScreenStoryKind::Search) {
        state.region = SettingsRegion::Search;
        state.search.set_query("appear");
        state.search.set_focused(true);
        nav = filter_settings_nav(&nav, "appear");
    }

    render_settings_screen(
        frame.buffer_mut(),
        area,
        SettingsScreenSurfaces {
            system,
            state: &mut state,
            nav: &nav,
            fieldsets: &fieldsets,
            theme_presets: BUILTIN_THEME_PRESETS,
            theme_paint: Some(system),
            status_slots: &[],
            status_state: &mut sstate,
            section_title: title,
        },
    );
}

fn settings_screen_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Basic);
}
fn settings_screen_search(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Search);
}
fn settings_screen_validation(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Validation);
}
fn settings_screen_conflicts(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Conflicts);
}
fn settings_screen_theme(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Theme);
}
fn settings_screen_keybinding(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Keybinding);
}
fn settings_screen_narrow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Narrow);
}
fn settings_screen_tiny(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Tiny);
}
fn settings_screen_no_results(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::NoResults);
}
fn settings_screen_help(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_settings_screen_story(frame, area, system, SettingsScreenStoryKind::Help);
}

#[derive(Clone, Copy)]
enum SetupWizardStoryKind {
    Welcome,
    Capability,
    Account,
    Permission,
    Theme,
    Summary,
    Recovery,
    Inline,
    Resume,
    CancelConfirm,
}

fn paint_setup_wizard_story(
    frame: &mut Frame<'_>,
    area: Rect,
    system: &DesignSystem,
    kind: SetupWizardStoryKind,
) {
    use termrock::patterns::{
        example_capability_lines, example_setup_account_fields, example_setup_steps,
        example_setup_summary_lines, render_setup_wizard, SetupStepKind, SetupWizardMode,
        SetupWizardState, SetupWizardSurfaces,
    };
    use termrock::widgets::{Fieldset, WizardGate, BUILTIN_THEME_PRESETS};

    let mut state = SetupWizardState::from_steps(example_setup_steps()).with_title("First run");
    if matches!(kind, SetupWizardStoryKind::Inline) {
        state = state.with_mode(SetupWizardMode::Inline);
    }

    let caps = example_capability_lines();
    let summary = example_setup_summary_lines();
    let account = example_setup_account_fields();
    let fieldsets: Vec<Fieldset<'_, &str>> = match kind {
        SetupWizardStoryKind::Account => vec![Fieldset::new("Account", &account)],
        _ => vec![],
    };

    // Position wizard on the desired step kind
    let target = match kind {
        SetupWizardStoryKind::Welcome | SetupWizardStoryKind::Inline => SetupStepKind::Welcome,
        SetupWizardStoryKind::Capability => SetupStepKind::Capability,
        SetupWizardStoryKind::Account => SetupStepKind::Account,
        SetupWizardStoryKind::Permission => SetupStepKind::Permission,
        SetupWizardStoryKind::Theme => SetupStepKind::Theme,
        SetupWizardStoryKind::Summary | SetupWizardStoryKind::Resume => SetupStepKind::Summary,
        SetupWizardStoryKind::Recovery => SetupStepKind::Recovery,
        SetupWizardStoryKind::CancelConfirm => SetupStepKind::Welcome,
    };

    match kind {
        SetupWizardStoryKind::Recovery => {
            let _ = state.wizard.fail("Could not reach API — retry when ready");
        }
        SetupWizardStoryKind::Resume => {
            // Advance then restore
            while state.current_kind() != SetupStepKind::Theme && state.wizard.step() < 20 {
                state.set_gate(WizardGate::Valid);
                let _ = state.wizard.next();
            }
            let snap = state.progress();
            state = SetupWizardState::from_steps(example_setup_steps()).with_title("Resume setup");
            let _ = state.resume(&snap);
        }
        SetupWizardStoryKind::CancelConfirm => {
            let _ = state.request_cancel();
        }
        SetupWizardStoryKind::Summary => {
            // Open review phase
            while !matches!(
                state.wizard.phase(),
                termrock::widgets::WizardPhase::Review
            ) && state.wizard.step() < 30
            {
                state.set_gate(WizardGate::Valid);
                let out = state.wizard.next();
                if matches!(out, termrock::widgets::FormWizardOutcome::ReviewOpened) {
                    break;
                }
                if matches!(out, termrock::widgets::FormWizardOutcome::SubmitRequested) {
                    break;
                }
            }
        }
        _ => {
            while state.current_kind() != target && state.wizard.step() < 20 {
                state.set_gate(WizardGate::Valid);
                let _ = state.wizard.next();
                if matches!(
                    state.wizard.phase(),
                    termrock::widgets::WizardPhase::Review | termrock::widgets::WizardPhase::Failed
                ) {
                    break;
                }
            }
            if matches!(target, SetupStepKind::Account) {
                state.set_gate(WizardGate::Invalid);
            } else {
                state.set_gate(WizardGate::Valid);
            }
        }
    }

    render_setup_wizard(
        frame.buffer_mut(),
        area,
        SetupWizardSurfaces {
            system,
            state: &mut state,
            fieldsets: &fieldsets,
            capabilities: &caps,
            summary_lines: &summary,
            welcome_title: "TermRock setup",
            welcome_detail: "Configure once. Keyboard-first.",
            theme_presets: BUILTIN_THEME_PRESETS,
            theme_paint: Some(system),
            permission: None,
        },
    );
}

fn setup_wizard_welcome(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Welcome);
}
fn setup_wizard_capability(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Capability);
}
fn setup_wizard_account(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Account);
}
fn setup_wizard_permission(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Permission);
}
fn setup_wizard_theme(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Theme);
}
fn setup_wizard_summary(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Summary);
}
fn setup_wizard_recovery(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Recovery);
}
fn setup_wizard_inline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Inline);
}
fn setup_wizard_resume(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::Resume);
}
fn setup_wizard_cancel_confirm(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    paint_setup_wizard_story(frame, area, system, SetupWizardStoryKind::CancelConfirm);
}

fn transcript_empty(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let blocks: [TranscriptBlock<'_, &str>; 0] = [];
    let mut state = TranscriptState::new();
    frame.render_stateful_widget(
        &Transcript::new(&blocks, system).empty_label("(no messages)"),
        area,
        &mut state,
    );
}

fn transcript_folded_follow(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let user = ["ship dual chrome kill"];
    let tool = ["cargo test", "ok"];
    let asst = ["done"];
    let blocks = [
        TranscriptBlock::new("u1", TranscriptKind::User, &user),
        TranscriptBlock::new("t1", TranscriptKind::Tool, &tool)
            .folded(true)
            .summary("cargo test — ok"),
        TranscriptBlock::new("a1", TranscriptKind::Assistant, &asst),
    ];
    let mut state = TranscriptState::new();
    state.set_follow(true);
    state.select(Some("t1"));
    frame.render_stateful_widget(
        &Transcript::new(&blocks, system).focused(true),
        area,
        &mut state,
    );
}

fn transcript_ascii_colorless(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let user = ["hello"];
    let think = ["…reasoning…"];
    let blocks = [
        TranscriptBlock::new("u1", TranscriptKind::User, &user),
        TranscriptBlock::new("th", TranscriptKind::Thinking, &think),
    ];
    let mut state = TranscriptState::new();
    frame.render_stateful_widget(
        &Transcript::new(&blocks, system)
            .ascii(true)
            .colorless(true)
            .focused(true),
        area,
        &mut state,
    );
}
