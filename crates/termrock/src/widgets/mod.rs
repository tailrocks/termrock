//! Product-neutral terminal widgets with borrowed render data and stable IDs.

pub use crate::interaction::Outcome;

mod action_bar;
mod agent;
mod agent_blocks;
mod blocks;
mod charts;
mod code_block;
mod command_palette;
mod completion_menu;
mod composed_row;
mod content;
mod controls;
mod data_table;
mod data_view;
mod design_inspector;
mod detail_table;
mod dialog;
mod diff;
mod edit_core;
mod form;
mod hint_bar;
mod image_surface;
mod jump_overlay;
mod list;
mod log_pane;
mod markdown;
mod menu_nav;
mod panel;
mod permission;
mod picker;
mod primitives;
mod progress;
mod prompt_composer;
mod review;
mod scroll_area;
mod selection;
mod split_pane;
mod status_bar;
mod table;
mod tabs;
mod text_area;
mod text_input;
mod theme_picker;
mod toast;
mod transcript;
mod tree;
mod view_state;
mod viewport;
mod virtual_grid;

pub use crate::style::PanelChrome;
pub use action_bar::{Action, ActionBar, ActionBarState};
pub use agent::{
    StreamItem, StreamItemKind, StreamView, ThinkingBlock, Timeline, TimelineEvent, TokenMeter,
    ToolCard, ToolStatus,
};
pub use agent_blocks::{
    ModeRibbon, ModeRibbonOutcome, ModeRibbonState, PlanReview, PlanReviewOutcome, PlanReviewState,
    PlanStep, QuestionFlow, QuestionFlowOutcome, QuestionFlowState, QuestionOption, QuestionStep,
    SessionItem, SessionPicker, SessionPickerOutcome, TaskRail, WorkbenchMode,
    session_picker_handle_key,
};
pub use blocks::{
    BlockChrome, FormWizard, FormWizardOutcome, FormWizardState, OpsDashboardOutcome,
    OpsDashboardState, OpsRegion, ResourceBrowserOutcome, ResourceBrowserState,
    SettingsShellOutcome, SettingsShellState,
};
pub use charts::{BarDatum, BarSeries, MeterSegment, SegmentedMeter, Sparkline};
pub use code_block::{CodeBlock, PlainSyntax, SyntaxHighlighter};
pub use command_palette::{
    COMMAND_PALETTE_OVERLAY_ID, CommandPalette, CommandPaletteOutcome, CommandPaletteSize,
    CommandPaletteState, dismiss_command_palette_overlay, open_command_palette_overlay,
    place_command_palette,
};
pub use completion_menu::{
    COMPLETION_OVERLAY_ID, CompletionCandidate, CompletionMenu, CompletionMenuOutcome,
    CompletionMenuSize, CompletionMenuState, dismiss_completion_overlay, open_completion_overlay,
    place_completion_menu,
};
pub use composed_row::{ComposedRow, ComposedRowParts};
pub use content::{
    Alert, AlertOutcome, AlertState, AlertTone, Callout, CalloutTone, Heading, HeadingLevel,
    Paragraph, Section, SectionOutcome, SectionState, Surface, SurfaceElevation,
};
pub use controls::{
    Checkbox, CheckboxOutcome, CheckboxState, Combobox, ComboboxOutcome, ComboboxState,
    MultiSelect, MultiSelectOutcome, MultiSelectState, RadioGroup, RadioOutcome, RadioState,
    Select, SelectOutcome, SelectState, Switch, SwitchOutcome, SwitchState,
};
pub use data_table::{DataTable, DataTableOutcome, DataTableState, DataTableToolbar};
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
    Backdrop, ChoiceDialog, ChoiceDialogState, DIALOG_OVERLAY_ID, Dialog, DialogSize,
    DialogVariant, MessageDialog, dismiss_dialog_overlay, open_alert_dialog_overlay,
    open_dialog_overlay, place_dialog,
};
pub use diff::{DiffKind, DiffLine, DiffState, DiffView};
pub use form::{Form, FormField, FormFieldRegion, FormOutcome, FormSection, FormState};
pub use hint_bar::{
    Hint, HintBar, HintSpan, hint_row_cols, render_hint_bar, styled_hint_spans, wrapped_hint_lines,
};
pub use image_surface::{ImageMeta, ImageProtocol, ImageSurface, protocol_emission_hint};
pub use jump_overlay::{
    JUMP_OVERLAY_ID, JumpOutcome, JumpOverlay, JumpOverlayState, JumpTarget, assign_jump_badges,
    dismiss_jump_overlay, open_jump_overlay,
};
pub use list::{List, ListClickPolicy, ListRow, ListState, RowRole};
pub use log_pane::{LogPane, LogPaneState};
pub use markdown::{MarkdownBlock, MarkdownBlockKind, MarkdownView, project_plain_lines};
pub use menu_nav::{
    BreadcrumbItem, Breadcrumbs, BreadcrumbsOutcome, BreadcrumbsState, ContextMenu,
    DRAWER_OVERLAY_ID, Drawer, DrawerOutcome, DrawerState, Menu, MenuItem, MenuOutcome, MenuState,
    POPOVER_OVERLAY_ID, Popover, Sidebar, SidebarItem, SidebarOutcome, SidebarState,
    TOOLTIP_OVERLAY_ID, Tooltip, TooltipState, dismiss_drawer_overlay, open_drawer_overlay,
    open_popover_overlay, open_tooltip_overlay, place_drawer, place_popover, place_tooltip,
};
pub use panel::{Panel, PanelSlots};
pub use permission::{
    DataMovement, EditField, ExecutionLocation, InitiatorKind, PERMISSION_OVERLAY_ID,
    PermissionAction, PermissionActionKind, PermissionActionRegion, PermissionAuditEntry,
    PermissionOutcome, PermissionPrompt, PermissionPromptState, PermissionProvenance,
    PermissionQueue, PermissionRequest, PermissionRisk, PermissionScope, PermissionTarget,
    PriorGrant, ProvenanceHop, StalePermission, StaleReason,
};
pub use picker::{Picker, PickerOutcome, PickerState};
pub use primitives::{
    ActivationOutcome, ActivationState, Badge, Button, ButtonState, Chip, ChipOutcome, ChipState,
    IconButton, IconButtonState, Kbd, Separator, SeparatorLine, Spinner, Tag, TagOutcome, TagState,
    button_hit,
};
pub use progress::{Progress, ProgressKind};
pub use prompt_composer::{
    ChipKind, CompletionKind, CompletionQuery, ComposerChip, ComposerConnection,
    ComposerPresentation, ContextEstimate, LARGE_PASTE_THRESHOLD, ModeIndicator, ModelIndicator,
    PROMPT_COMPLETION_OVERLAY_ID, PROMPT_FULLSCREEN_OVERLAY_ID, PromptComposer,
    PromptComposerLayout, PromptComposerOutcome, PromptComposerState, QueuedPrompt, SubmitPolicy,
};
pub use review::{
    DiffHunk, DiffReview, DiffReviewOutcome, DiffReviewState, InspectorField, LogLevel, LogLine,
    LogStream, LogStreamOutcome, LogStreamState, ObjectInspector, ObjectInspectorOutcome,
    ObjectInspectorState,
};
pub use scroll_area::{ScrollArea, ScrollAreaState, ScrollBarVisibility};
pub use selection::Selection;
pub use split_pane::{
    SplitDirection, SplitPane, SplitPaneLayout, SplitPaneOutcome, SplitPaneState, SplitRatio,
    SplitSide,
};
pub use status_bar::{StatusBar, StatusBarState, StatusSlot};
pub use table::{
    CellAlignment, Column, ColumnWidth, SortDirection, Table, TableHeaderRegion, TableOutcome,
    TableRow, TableRowRegion, TableState, resolve_widths,
};
pub use tabs::{TAB_GAP, Tab, TabCell, Tabs, TabsState, lay_out_tabs, tab_at_column};
pub use text_area::{TextArea, TextAreaOutcome, TextAreaState, TextCursor};
pub use text_input::{
    EditAction, TextInput, TextInputOutcome, TextInputState, TextInputValidity, Validation,
};
pub use theme_picker::{
    BUILTIN_THEME_PRESETS, ThemePicker, ThemePickerOutcome, ThemePickerState, ThemePreset,
    theme_from_preset_id,
};
pub use toast::{Anchor, Severity, Toast, ToastLifetime, ToastState};
pub use transcript::{
    Transcript, TranscriptAnchor, TranscriptBlock, TranscriptKind, TranscriptOutcome,
    TranscriptState,
};
pub use tree::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState};
pub use view_state::{Banner, EmptyState, ErrorView, LoadingView, Skeleton};
pub use viewport::Viewport;
pub use virtual_grid::{
    GridCell, GridCellRegion, GridColumn, GridColumnWidth, GridHeaderRegion, GridRow, VirtualGrid,
    VirtualGridOutcome, VirtualGridState,
};

#[cfg(test)]
mod tests;
