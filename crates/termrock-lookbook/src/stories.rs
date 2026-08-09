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
        Action, ActionBar, ActionBarState, Anchor, BUILTIN_THEME_PRESETS, Backdrop, Badge, Banner,
        BarDatum, BarSeries, Button, ButtonState, Callout, CalloutTone, CellAlignment, Checkbox,
        CheckboxState, ChoiceDialog, ChoiceDialogState, CodeBlock, Column, ColumnWidth,
        CommandPalette, CommandPaletteState, CompletionCandidate, CompletionMenu,
        CompletionMenuSize, CompletionMenuState, DataTable, DataTableState, DataTableToolbar,
        DesignInspector, DesignInspectorFrame, DetailCapability, DetailRow, DetailTable,
        DetailTableState, Dialog, DiffKind, DiffLine, DiffState, DiffView, Drawer, EmptyState,
        ErrorView, Form, FormField, FormSection, FormState, FormWizardState, GridCell, GridColumn,
        GridRow, Heading, HeadingLevel, Hint, HintBar, ImageMeta, ImageProtocol, ImageSurface,
        JumpOverlay, JumpTarget, Kbd, List, ListRow, ListState, LoadingView, LogPane, LogPaneState,
        MarkdownBlock, MarkdownBlockKind, MarkdownView, Menu, MenuItem, MenuState, MessageDialog,
        MeterSegment, ModeRibbon, Panel, PanelChrome, PermissionActionKind, PermissionPrompt,
        PermissionPromptState, PermissionProvenance, PermissionRequest, PermissionRisk, Picker,
        PickerState, PlanReview, PlanReviewState, PlanStep, Popover, Progress, ProgressKind,
        PromptComposer, PromptComposerState, QuestionFlow, QuestionFlowState, QuestionOption,
        QuestionStep, RowRole, SegmentedMeter, SeparatorLine, SessionItem, SessionPicker, Severity,
        Skeleton, SortDirection, Sparkline, SplitDirection, SplitPane, SplitPaneState, SplitRatio,
        StatusBar, StatusBarState, StatusSlot, Surface, SurfaceElevation, Switch, SwitchState, Tab,
        Table, TableRow, TableState, Tabs, TabsState, TaskRail, TextArea, TextAreaState,
        TextCursor, TextInput, TextInputState, ThemePicker, ThemePickerState, ThinkingBlock,
        Timeline, TimelineEvent, Toast, TokenMeter, ToolCard, ToolStatus, Transcript,
        TranscriptBlock, TranscriptKind, TranscriptState, Tree, TreeNode, TreeNodeStatus,
        TreeState, Validation, Viewport, VirtualGrid, VirtualGridState, WorkbenchMode,
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
            "panel/focused",
            "Focused panel",
            "Panel",
            "A semantically focused bordered panel.",
            48,
            7,
            panel,
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
            "progress/determinate",
            "Progress",
            "Progress",
            "Caller-ticked determinate and indeterminate progress.",
            42,
            2,
            progress,
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
            "completion-menu/basic",
            "Completion menu",
            "CompletionMenu",
            "Popup candidates with stable IDs, kind annotations, and anchor clamp.",
            48,
            12,
            completion_menu_basic,
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
            "Unicode candidates remain bounded in a narrow popup.",
            22,
            8,
            completion_menu_basic,
        ),
        Story::new(
            "completion-menu/unicode",
            "Completion menu Unicode",
            "CompletionMenu",
            "Display-width clipping preserves complete Unicode candidates.",
            32,
            8,
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
            "Header-only rendering with no domain empty-state wording.",
            42,
            3,
            table_empty,
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
            "status-bar/basic",
            "Status bar",
            "StatusBar",
            "Caller-owned left and right status slots.",
            60,
            1,
            status_bar,
        ),
        Story::new(
            "design-inspector/basic",
            "Design inspector",
            "DesignInspector",
            "Studio focus/layer/capability strip.",
            48,
            4,
            design_inspector,
        )
        .with_interactor(design_inspector_interactor),
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
            "overlay/nested-escape",
            "Nested overlays",
            "OverlayStack",
            "Parent dialog + child menu; Esc peels one layer.",
            48,
            14,
            overlay_nested,
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
            "Multiple dialogs stacked; top owns Esc.",
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
            "Responsive neutral dialog shell.",
            48,
            7,
            dialog,
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
            "Diff view",
            "DiffView",
            "Borrowed projected diff lines.",
            54,
            6,
            diff,
        ),
        Story::new(
            "toast/success",
            "Toast",
            "Toast",
            "Caller-owned transient message.",
            34,
            4,
            toast,
        )
        .with_interactor(toast_interactor),
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
            "Centered empty surface with non-color glyph.",
            36,
            5,
            empty_state,
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
            "error-view/basic",
            "Error view",
            "ErrorView",
            "Centered failure surface with danger glyph.",
            36,
            5,
            error_view,
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
            "Skeleton",
            "Skeleton",
            "Placeholder loading lines.",
            32,
            4,
            skeleton,
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
            "command-palette/basic",
            "Command palette",
            "CommandPalette",
            "Filterable command list in focused chrome.",
            42,
            10,
            command_palette,
        )
        .with_interactor(command_palette_interactor),
        Story::new(
            "code-block/basic",
            "Code block",
            "CodeBlock",
            "Source listing with line numbers.",
            40,
            5,
            code_block,
        ),
        Story::new(
            "markdown-view/basic",
            "Markdown view",
            "MarkdownView",
            "Projected markdown blocks.",
            40,
            6,
            markdown_view,
        ),
        Story::new(
            "sparkline/basic",
            "Sparkline",
            "Sparkline",
            "One-row density chart.",
            32,
            1,
            sparkline,
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
            "Activity timeline events.",
            40,
            4,
            timeline,
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
            "Button",
            "Button",
            "Focused primary button activation chrome.",
            32,
            3,
            button_story,
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
            "badge/basic",
            "Badge",
            "Badge",
            "Non-interactive status badge.",
            20,
            3,
            badge_story,
        ),
        Story::new(
            "callout/basic",
            "Callout",
            "Callout",
            "Semantic callout with non-color glyph.",
            40,
            4,
            callout_story,
        ),
        Story::new(
            "drawer/basic",
            "Drawer",
            "Drawer",
            "Edge drawer chrome.",
            24,
            10,
            drawer_story,
        ),
        Story::new(
            "heading/basic",
            "Heading",
            "Heading",
            "Terminal typography heading.",
            40,
            3,
            heading_story,
        ),
        Story::new(
            "kbd/basic",
            "Kbd",
            "Kbd",
            "Key chord chrome for hints.",
            16,
            3,
            kbd_story,
        ),
        Story::new(
            "paragraph/basic",
            "Paragraph",
            "Paragraph",
            "Body paragraph wrap.",
            40,
            4,
            paragraph_story,
        ),
        Story::new(
            "surface/basic",
            "Surface",
            "Surface",
            "Elevated surface fill.",
            20,
            5,
            surface_story,
        ),
        Story::new(
            "separator/basic",
            "Separator",
            "SeparatorLine",
            "Horizontal separator rule.",
            30,
            3,
            separator_story,
        ),
        Story::new(
            "popover/basic",
            "Popover",
            "Popover",
            "Anchored non-modal popover chrome.",
            28,
            4,
            popover_story,
        ),
        Story::new(
            "agent-workbench/basic",
            "Agent workbench",
            "AgentWorkbench",
            "Canonical agent shell: task rail, transcript, PromptComposer, status.",
            80,
            24,
            agent_workbench_basic,
        ),
        Story::new(
            "agent-workbench/permission",
            "Agent workbench permission",
            "AgentWorkbench",
            "PermissionPrompt overlay on workbench with default-deny focus.",
            80,
            24,
            agent_workbench_permission,
        ),
        Story::new(
            "agent-workbench/narrow",
            "Agent workbench narrow",
            "AgentWorkbench",
            "Narrow-terminal geometry for the flagship workbench pattern.",
            40,
            16,
            agent_workbench_basic,
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
            "Plan steps review list.",
            48,
            8,
            plan_review_story,
        ),
        Story::new(
            "question-flow/basic",
            "Question flow",
            "QuestionFlow",
            "Multi-step question chrome.",
            48,
            8,
            question_flow_story,
        ),
        Story::new(
            "session-picker/basic",
            "Session picker",
            "SessionPicker",
            "Session list picker.",
            40,
            8,
            session_picker_story,
        ),
        Story::new(
            "task-rail/basic",
            "Task rail",
            "TaskRail",
            "Titled task list rail.",
            28,
            10,
            task_rail_story,
        ),
        Story::new(
            "blocks/form-wizard",
            "FormWizard",
            "FormWizard",
            "Wizard step projection (navigation state only).",
            40,
            4,
            form_wizard_story,
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
            "Narrow-terminal geometry for DiffView (22 cols).",
            22,
            6,
            diff,
        ),
        Story::new(
            "diff/unicode",
            "Unicode DiffView",
            "DiffView",
            "Unicode-safe paint path for DiffView (CJK/emoji-capable layout).",
            54,
            6,
            diff_unicode_story,
        ),
        Story::new(
            "drawer/narrow",
            "Narrow Drawer",
            "Drawer",
            "Narrow-terminal geometry for Drawer (16 cols).",
            16,
            10,
            drawer_story,
        ),
        Story::new(
            "drawer/unicode",
            "Unicode Drawer",
            "Drawer",
            "Unicode-safe paint path for Drawer (CJK/emoji-capable layout).",
            28,
            10,
            drawer_unicode_story,
        ),
        Story::new(
            "empty-state/narrow",
            "Narrow EmptyState",
            "EmptyState",
            "Narrow-terminal geometry for EmptyState (18 cols).",
            18,
            5,
            empty_state,
        ),
        Story::new(
            "empty-state/unicode",
            "Unicode EmptyState",
            "EmptyState",
            "Unicode-safe paint path for EmptyState (CJK/emoji-capable layout).",
            36,
            5,
            empty_state_unicode_story,
        ),
        Story::new(
            "error-view/narrow",
            "Narrow ErrorView",
            "ErrorView",
            "Narrow-terminal geometry for ErrorView (18 cols).",
            18,
            5,
            error_view,
        ),
        Story::new(
            "error-view/unicode",
            "Unicode ErrorView",
            "ErrorView",
            "Unicode-safe paint path for ErrorView (CJK/emoji-capable layout).",
            36,
            5,
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
            "Narrow-terminal geometry for FormWizard (20 cols).",
            20,
            4,
            form_wizard_story,
        ),
        Story::new(
            "blocks/unicode",
            "Unicode FormWizard",
            "FormWizard",
            "Unicode-safe paint path for FormWizard (CJK/emoji-capable layout).",
            40,
            4,
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
            "Narrow-terminal geometry for Panel (22 cols).",
            22,
            7,
            panel,
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
            8,
            plan_review_story,
        ),
        Story::new(
            "plan-review/unicode",
            "Unicode PlanReview",
            "PlanReview",
            "Unicode-safe paint path for PlanReview (CJK/emoji-capable layout).",
            48,
            8,
            plan_review_unicode_story,
        ),
        Story::new(
            "popover/narrow",
            "Narrow Popover",
            "Popover",
            "Narrow-terminal geometry for Popover (14 cols).",
            14,
            4,
            popover_story,
        ),
        Story::new(
            "popover/unicode",
            "Unicode Popover",
            "Popover",
            "Unicode-safe paint path for Popover (CJK/emoji-capable layout).",
            28,
            4,
            popover_unicode_story,
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
            8,
            session_picker_story,
        ),
        Story::new(
            "session-picker/unicode",
            "Unicode SessionPicker",
            "SessionPicker",
            "Unicode-safe paint path for SessionPicker (CJK/emoji-capable layout).",
            40,
            8,
            session_picker_unicode_story,
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
            "text-input/basic",
            "Text input",
            "TextInput",
            "Default focused text input.",
            32,
            1,
            text_input_basic_story,
        ),
    ]
}

/// Interactive-gallery entries, including compile-proven design prototypes.
/// Catalog generation deliberately uses [`stories`] instead.
pub(crate) fn gallery_stories() -> Vec<Story> {
    stories()
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
        focused: Some("accept"),
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
            trailing: Some(Line::from("4 items")),
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "documents",
            label: Line::from("Documents"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("2 items")),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Remote items"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
        },
        TreeNode {
            id: "notes",
            label: Line::from("Wide 🧪 notes"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("12 KiB")),
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
    ]
}

pub(crate) fn form_fields() -> Vec<FormField<'static, &'static str>> {
    vec![
        FormField::new("name", Line::from("Name"), Line::from("Example profile"))
            .help(Line::from("A recognizable display name"))
            .required(true),
        FormField::new("endpoint", Line::from("Endpoint"), Line::from("localhost"))
            .error(Line::from("Enter a reachable address"))
            .required(true),
        FormField::new(
            "mode",
            Line::from("Managed mode"),
            Line::from("Unavailable"),
        )
        .enabled(false),
    ]
}

fn form(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let fields = form_fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new();
    frame.render_stateful_widget(
        &Form::new(&sections, system).focused_field(Some(&"name")),
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

fn tree(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let nodes = tree_nodes();
    let mut state = TreeState::new(Some("workspace"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"notes");
    frame.render_stateful_widget(&Tree::new(&nodes, &tokens), area, &mut state);
}

fn tabs(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab {
            id: "overview",
            label: "Overview",
            glyph: Some(Span::styled("●", system.style(Role::Success))),
            active: true,
            enabled: true,
        },
        Tab {
            id: "details",
            label: "Details",
            glyph: None,
            active: false,
            enabled: true,
        },
    ];
    let mut state = TabsState {
        selected: Some("overview"),
        focused: true,
        ..TabsState::default()
    };
    frame.render_stateful_widget(&Tabs::new(&items, system).gap(1), area, &mut state);
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

fn list_unicode(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let rows = [
        ListRow {
            id: "cjk",
            label: Line::from("東京 設定"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("日本語")),
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "emoji",
            label: Line::from("🧪 Laboratory"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("✅")),
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "combining",
            label: Line::from("Cafe\u{301} profile"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("e\u{301}")),
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
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("3 entries")),
            role: RowRole::Separator,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "alpha",
            label: Line::from("Alpha"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("12 ms")),
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "beta",
            label: Line::from("Beta"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("28 ms")),
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "gamma",
            label: Line::from("Gamma"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
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
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("操作")),
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "cafe",
            label: Line::from("Cafe\u{301} logs"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("表示")),
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

#[derive(Clone, Copy)]
enum TableVariant {
    Basic,
    Sorted,
    Narrow,
    Unicode,
    Disabled,
    Empty,
}

fn completion_menu_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(Panel::new(&panel_tokens).title("Editor"), area);
    let candidates = [
        CompletionCandidate::new("select", "SELECT").kind("keyword"),
        CompletionCandidate::new("from", "FROM").kind("keyword"),
        CompletionCandidate::new("users", "users").kind("table"),
        CompletionCandidate::new("orders", "orders").kind("table"),
        CompletionCandidate::new("where", "WHERE").kind("keyword"),
    ];
    let anchor = Rect::new(area.x.saturating_add(4), area.y.saturating_add(2), 1, 1);
    let mut state = CompletionMenuState::new(Some("select"));
    frame.render_stateful_widget(
        &CompletionMenu::new(&candidates, system, area, anchor).preferred_size(
            CompletionMenuSize {
                width: 28,
                height: 6,
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

fn render_table(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem, variant: TableVariant) {
    let tokens = system.clone().density(termrock::style::Density::default());
    let sorted = matches!(variant, TableVariant::Sorted);
    let columns = [
        Column {
            id: "pid",
            title: Line::from("PID"),
            width: ColumnWidth::Fixed(7),
            alignment: CellAlignment::Right,
            sortable: true,
            sort: None,
        },
        Column {
            id: "process",
            title: Line::from("Process"),
            width: ColumnWidth::Fill(NonZeroU16::new(2).unwrap()),
            alignment: CellAlignment::Left,
            sortable: true,
            sort: None,
        },
        Column {
            id: "region",
            title: Line::from("Region"),
            width: ColumnWidth::Min(10),
            alignment: CellAlignment::Center,
            sortable: false,
            sort: None,
        },
        Column {
            id: "cpu",
            title: Line::from("CPU"),
            width: ColumnWidth::Fixed(8),
            alignment: CellAlignment::Right,
            sortable: true,
            sort: sorted.then_some(SortDirection::Descending),
        },
        Column {
            id: "state",
            title: Line::from("State"),
            width: ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
            alignment: CellAlignment::Center,
            sortable: false,
            sort: None,
        },
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
        .map(|(index, cells)| TableRow {
            id: index,
            cells,
            leading: None,
            badge: None,
            enabled: !(matches!(variant, TableVariant::Disabled) && index == 2),
            emphasis: index == 0 && matches!(variant, TableVariant::Unicode),
            style: None,
        })
        .collect::<Vec<_>>();
    let visible = if matches!(variant, TableVariant::Empty) {
        &rows[..0]
    } else {
        &rows
    };
    let mut state = TableState::new((!visible.is_empty()).then_some(
        if matches!(variant, TableVariant::Disabled) {
            1
        } else {
            3
        },
    ));
    frame.render_stateful_widget(&Table::new(&columns, visible, &tokens), area, &mut state);
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
    let left = [StatusSlot {
        id: "state",
        content: " Ready ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().reversed(),
        hover_style: Some(Style::new().bold().reversed()),
    }];
    let right = [StatusSlot {
        id: "position",
        content: " 3/12 ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().dim(),
        hover_style: Some(Style::new().bold()),
    }];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::new(&left, &right, system).alpha(1.0),
        area,
        &mut state,
    );
}

fn design_inspector(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let layers = ["root"];
    let recipes = ["list_row", "panel"];
    let snap = DesignInspectorFrame {
        focused: Some("list"),
        layer: Some("root"),
        capability: ColorCapability::Truecolor,
        density: "comfortable",
        layers: &layers,
        recipes: &recipes,
        selection_chrome: "gutter",
    };
    frame.render_widget(DesignInspector::new(snap, system), area);
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
    };
    frame.render_widget(DesignInspector::new(snap, &mono), area);
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
    let tokens = system.clone().density(Density::default());
    let mut stack = OverlayStack::<()>::new();
    let _ = stack.open(
        area,
        OverlaySpec::dialog("d1", OverlaySize::dialog(30, 6), None),
    );
    let _ = stack.open(
        area,
        OverlaySpec::dialog("d2", OverlaySize::dialog(28, 6), None),
    );
    paint_stack_rects(frame, area, &stack, &tokens, system);
    frame.render_widget(
        Paragraph::new(Line::from(format!(
            "depth={} top owns Esc",
            stack.entries().len()
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
        .style(Style::new())
        .emphasis(termrock::widgets::PanelChrome::Focused)
        .footer_hint("esc dismiss"),
        area,
    );
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

fn diff(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let system = if system.palette() == &RolePalette::tailrocks_phosphor() {
        DesignSystem::from_palette(
            system
                .palette()
                .clone()
                .with_role(Role::DiffAdded, Style::new().bold())
                .with_role(Role::DiffRemoved, Style::new().dim()),
        )
    } else {
        system.clone()
    };
    let lines = [
        DiffLine {
            text: " context",
            kind: DiffKind::Context,
        },
        DiffLine {
            text: "-before",
            kind: DiffKind::Removed,
        },
        DiffLine {
            text: "+after",
            kind: DiffKind::Added,
        },
    ];
    frame.render_stateful_widget(
        &DiffView::new(&lines, &system),
        area,
        &mut DiffState::default(),
    );
}

fn toast(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        Toast::new(system, "Updated", Severity::Success).anchor(Anchor::TopRight),
        area,
    );
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
    frame.render_widget(
        EmptyState::new("No results", system).detail("Try another query"),
        area,
    );
}

fn loading_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(LoadingView::new("Loading…", "⠋", system), area);
}

fn error_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        ErrorView::new("Request failed", system).detail("Timed out"),
        area,
    );
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

fn jump_overlay(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let panel_tokens = system.clone().density(Density::default());
    frame.render_widget(
        Panel::new(&panel_tokens)
            .title("Jump targets")
            .emphasis(PanelChrome::Normal),
        area,
    );
    let targets = [
        JumpTarget {
            id: "files",
            area: Rect::new(area.x.saturating_add(2), area.y.saturating_add(1), 12, 1),
            badge: 'f',
        },
        JumpTarget {
            id: "main",
            area: Rect::new(area.x.saturating_add(2), area.y.saturating_add(3), 12, 1),
            badge: 'm',
        },
    ];
    frame.render_widget(JumpOverlay::new(&targets, system), area);
}

fn command_palette(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let rows = [
        ListRow::item("system", Line::from("Toggle system")),
        ListRow::item("quit", Line::from("Quit")),
    ];
    let mut state = CommandPaletteState::new(Some("system"));
    frame.render_stateful_widget(
        &CommandPalette::new("Commands", &rows, &tokens),
        area,
        &mut state,
    );
}

fn code_block(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let lines = ["fn main() {", "    println!(\"hi\");", "}"];
    frame.render_widget(
        CodeBlock::new(&lines, system)
            .language("rust")
            .line_numbers(true),
        area,
    );
}

fn markdown_view(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let blocks = [
        MarkdownBlock {
            kind: MarkdownBlockKind::Heading,
            text: "Plan",
        },
        MarkdownBlock {
            kind: MarkdownBlockKind::ListItem,
            text: "Implement widgets",
        },
        MarkdownBlock {
            kind: MarkdownBlockKind::Paragraph,
            text: "Ship the PR.",
        },
        MarkdownBlock {
            kind: MarkdownBlockKind::Code,
            text: "cargo test -p termrock",
        },
    ];
    frame.render_widget(MarkdownView::new(&blocks, system), area);
}

fn sparkline(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let samples = [0.1, 0.3, 0.2, 0.7, 0.9, 0.5, 0.8, 0.4];
    frame.render_widget(Sparkline::new(&samples, system), area);
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
    frame.render_widget(BarSeries::new(&bars, system), area);
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
    frame.render_widget(SegmentedMeter::new(&segments, system), area);
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
        TimelineEvent {
            when: "12:01",
            text: "Started",
            active: false,
        },
        TimelineEvent {
            when: "12:02",
            text: "Running tests",
            active: true,
        },
        TimelineEvent {
            when: "12:03",
            text: "Open PR",
            active: false,
        },
    ];
    frame.render_widget(Timeline::new(&events, system), area);
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
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_focused(true);
    Button::new("Save", &tokens)
        .primary(true)
        .render(area, frame.buffer_mut(), &mut state);
}

fn checkbox_switch_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut cb = CheckboxState::new(true);
    Checkbox::new("enable", "Enable", &tokens).render(
        Rect::new(area.x, area.y, area.width, 1),
        frame.buffer_mut(),
        &mut cb,
    );
    let mut sw = SwitchState::new(false);
    Switch::new("dark", "Dark mode", &tokens).render(
        Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        frame.buffer_mut(),
        &mut sw,
    );
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

fn menu_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let items = [
        MenuItem::new("a", "Open"),
        MenuItem::new("b", "Disabled").enabled(false),
        MenuItem::new("c", "Save"),
    ];
    let state = MenuState::new();
    Menu::new(&items, &tokens).render(area, frame.buffer_mut(), &state);
}

fn form_wizard_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = FormWizardState::new(3);
    frame.render_stateful_widget(
        &termrock::widgets::FormWizard::new(&tokens, "Wizard"),
        area,
        &mut state,
    );
}

fn badge_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Badge::new("NEW", &tokens), area, frame.buffer_mut());
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

fn drawer_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Drawer::new("Drawer", &tokens), area, frame.buffer_mut());
}

fn heading_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Heading::new("Section title", &tokens).level(HeadingLevel::H1),
        area,
        frame.buffer_mut(),
    );
}

fn kbd_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Kbd::new("C-k", &tokens), area, frame.buffer_mut());
}

fn paragraph_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &termrock::widgets::Paragraph::new(
            "Body text wraps by display columns when height allows.",
            &tokens,
        ),
        area,
        frame.buffer_mut(),
    );
}

fn surface_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Surface::new(&tokens).elevation(SurfaceElevation::Elevated),
        area,
        frame.buffer_mut(),
    );
}

fn separator_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &SeparatorLine::horizontal(&tokens),
        area,
        frame.buffer_mut(),
    );
}

fn popover_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Popover::new("Popover tip", &tokens),
        area,
        frame.buffer_mut(),
    );
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
    let tokens = system.clone().density(Density::default());
    let steps = [
        PlanStep {
            id: "s1",
            title: "Inspect",
            detail: Some("Read files"),
            accepted: true,
        },
        PlanStep {
            id: "s2",
            title: "Edit",
            detail: None,
            accepted: false,
        },
    ];
    let mut state = PlanReviewState::new(Some("s1"));
    frame.render_stateful_widget(&PlanReview::new(&steps, &tokens), area, &mut state);
}

fn question_flow_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let opts = [
        QuestionOption {
            id: "y",
            label: "Yes",
        },
        QuestionOption {
            id: "n",
            label: "No",
        },
    ];
    let steps = [QuestionStep {
        id: "q1",
        prompt: "Continue?",
        options: &opts,
        required: true,
    }];
    let mut state = QuestionFlowState::new(1);
    frame.render_stateful_widget(&QuestionFlow::new(&steps, &tokens), area, &mut state);
}

fn session_picker_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let sessions = [
        SessionItem {
            id: "s1",
            title: "Session A",
            meta: Some("2m ago"),
        },
        SessionItem {
            id: "s2",
            title: "Session B",
            meta: None,
        },
    ];
    let mut state = ListState::new(Some("s1"));
    frame.render_stateful_widget(&SessionPicker::new(&sessions, &tokens), area, &mut state);
}

fn task_rail_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let rows = [ListRow {
        id: "t1",
        label: Line::from("Task one"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: None,
        role: RowRole::Item,
        enabled: true,
        loading: false,
    }];
    let mut state = ListState::new(Some("t1"));
    frame.render_stateful_widget(&TaskRail::new(&rows, &tokens, "Tasks"), area, &mut state);
}

// ── State-axis story helpers ────────────────────────────────────────────────

fn button_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_focused(true);
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
    state.activation.set_focused(true);
    state.activation.set_loading(true);
    frame.render_stateful_widget(&Button::new("Save", &tokens), area, &mut state);
}

fn button_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = ButtonState::new();
    state.activation.set_focused(true);
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
    frame.render_stateful_widget(
        &Checkbox::new("enable", "Enable", &tokens),
        area,
        &mut state,
    );
}

fn checkbox_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut cb = CheckboxState::new(true);
    Checkbox::new("jp", "有効化 🇯🇵", &tokens).render(
        Rect::new(area.x, area.y, area.width, 1),
        frame.buffer_mut(),
        &mut cb,
    );
    let mut sw = SwitchState::new(false);
    Switch::new("dark", "暗色モード", &tokens).render(
        Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
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
    let state = MenuState::new();
    Menu::new(&items, &tokens).render(area, frame.buffer_mut(), &state);
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
        focused: Some("accept"),
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
            trailing: None,
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "src",
            label: Line::from("ソース 📦"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
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
            active: true,
            enabled: true,
        },
        Tab {
            id: "two",
            label: "詳細 📋",
            glyph: None,
            active: false,
            enabled: true,
        },
    ];
    let mut state = TabsState {
        selected: Some("one"),
        focused: true,
        ..TabsState::default()
    };
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
    diff(frame, area, system);
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
    frame.render_widget(
        EmptyState::new("結果なし 🌀", system).detail("クエリを変更してください"),
        area,
    );
}

fn error_view_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame.render_widget(
        ErrorView::new("失敗しました 💥", system).detail("再試行してください"),
        area,
    );
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
    let tokens = system.clone().density(Density::default());
    let steps = [
        PlanStep {
            id: "s1",
            title: "検査 🔍",
            detail: Some("ファイルを読む"),
            accepted: true,
        },
        PlanStep {
            id: "s2",
            title: "編集 ✏️",
            detail: None,
            accepted: false,
        },
    ];
    let mut state = PlanReviewState::new(Some("s1"));
    frame.render_stateful_widget(&PlanReview::new(&steps, &tokens), area, &mut state);
}

fn question_flow_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let opts = [
        QuestionOption {
            id: "y",
            label: "はい ✅",
        },
        QuestionOption {
            id: "n",
            label: "いいえ ❌",
        },
    ];
    let steps = [QuestionStep {
        id: "q1",
        prompt: "続行しますか？",
        options: &opts,
        required: true,
    }];
    let mut state = QuestionFlowState::new(1);
    frame.render_stateful_widget(&QuestionFlow::new(&steps, &tokens), area, &mut state);
}

fn session_picker_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let sessions = [
        SessionItem {
            id: "s1",
            title: "セッション甲 🅰️",
            meta: Some("2分前"),
        },
        SessionItem {
            id: "s2",
            title: "セッション乙 🅱️",
            meta: None,
        },
    ];
    let mut state = ListState::new(Some("s1"));
    frame.render_stateful_widget(&SessionPicker::new(&sessions, &tokens), area, &mut state);
}

fn task_rail_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let rows = [ListRow {
        id: "t1",
        label: Line::from("タスク一 📌"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: None,
        role: RowRole::Item,
        enabled: true,
        loading: false,
    }];
    let mut state = ListState::new(Some("t1"));
    frame.render_stateful_widget(&TaskRail::new(&rows, &tokens, "任務"), area, &mut state);
}

fn drawer_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Drawer::new("設定 ⚙️", &tokens), area, frame.buffer_mut());
}

fn popover_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Popover::new("ヒント 💡", &tokens),
        area,
        frame.buffer_mut(),
    );
}

fn separator_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &SeparatorLine::horizontal(&tokens),
        area,
        frame.buffer_mut(),
    );
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
    Widget::render(
        &Surface::new(&tokens).elevation(SurfaceElevation::Elevated),
        area,
        frame.buffer_mut(),
    );
    if area.width > 2 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(area.height / 2),
            "面 🎴",
            usize::from(area.width.saturating_sub(2)),
            system.style(Role::Text),
        );
    }
}

fn kbd_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Kbd::new("⌘K", &tokens), area, frame.buffer_mut());
}

fn form_wizard_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    let mut state = FormWizardState::new(3);
    frame.render_stateful_widget(
        &termrock::widgets::FormWizard::new(&tokens, "ウィザード 🪄"),
        area,
        &mut state,
    );
}

fn badge_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(&Badge::new("新规 ✨", &tokens), area, frame.buffer_mut());
}

fn heading_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &Heading::new("見出し ✨", &tokens).level(HeadingLevel::H1),
        area,
        frame.buffer_mut(),
    );
}

fn paragraph_unicode_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let tokens = system.clone().density(Density::default());
    Widget::render(
        &termrock::widgets::Paragraph::new("日本語と絵文字 🚀 を含む本文。", &tokens),
        area,
        frame.buffer_mut(),
    );
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
    text_input_unicode(frame, area, system);
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

fn agent_workbench_basic(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{
        AgentWorkbenchState, WorkbenchSurfaces, default_modes, render_agent_workbench,
    };
    use termrock::widgets::{
        ListRow, PromptComposer, PromptComposerState, StatusBarState, StatusSlot, Transcript,
        TranscriptBlock, TranscriptKind, TranscriptState,
    };

    let mut workbench = AgentWorkbenchState::new();
    let lines = ["Plan the cutover", "Implement PermissionPrompt path"];
    let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
    let transcript = Transcript::new(&blocks, system);
    let mut tstate = TranscriptState::new();
    let prompt = PromptComposer::new(system);
    let mut pstate = PromptComposerState::new();
    pstate.set_text("ship the dual-chrome kill");
    let tasks = [
        ListRow::item("t1", Line::from("Plan review")),
        ListRow::item("t2", Line::from("Tool: cargo test")),
    ];
    workbench.task_list.select(Some("t1"));
    let modes = default_modes("plan");
    let slots = [StatusSlot {
        id: "s",
        content: "ready",
        priority: 0,
        min_width: 0,
        enabled: true,
        style: Style::default(),
        hover_style: None,
    }];
    let mut sstate = StatusBarState::default();
    render_agent_workbench(
        frame.buffer_mut(),
        area,
        WorkbenchSurfaces {
            system,
            state: &mut workbench,
            tasks: &tasks,
            modes: &modes,
            transcript: &transcript,
            transcript_state: &mut tstate,
            prompt: &prompt,
            prompt_state: &mut pstate,
            status_slots: &slots,
            status_state: &mut sstate,
            permission: None,
            question: None,
        },
    );
}

fn agent_workbench_permission(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::patterns::{
        AgentWorkbenchState, WorkbenchSurfaces, default_modes, render_agent_workbench,
    };
    use termrock::widgets::{
        ListRow, PermissionPrompt, PermissionPromptState, PermissionRequest, PermissionRisk,
        PromptComposer, PromptComposerState, StatusBarState, Transcript, TranscriptBlock,
        TranscriptKind, TranscriptState,
    };

    let mut workbench = AgentWorkbenchState::new();
    let lines = ["Need shell access"];
    let blocks = [TranscriptBlock::new(
        "b1",
        TranscriptKind::Assistant,
        &lines,
    )];
    let transcript = Transcript::new(&blocks, system);
    let mut tstate = TranscriptState::new();
    let prompt = PromptComposer::new(system);
    let mut pstate = PromptComposerState::new();
    let tasks = [ListRow::item("t1", Line::from("Awaiting permission"))];
    let modes = default_modes("build");
    let slots = [];
    let mut sstate = StatusBarState::default();
    let perm_w = PermissionPrompt::new(system);
    let mut perm = PermissionPromptState::new();
    let _ = perm.enqueue(
        PermissionRequest::new("r1", "bash", "workspace")
            .risk(PermissionRisk::High)
            .command("cargo test --all-features"),
    );
    render_agent_workbench(
        frame.buffer_mut(),
        area,
        WorkbenchSurfaces {
            system,
            state: &mut workbench,
            tasks: &tasks,
            modes: &modes,
            transcript: &transcript,
            transcript_state: &mut tstate,
            prompt: &prompt,
            prompt_state: &mut pstate,
            status_slots: &slots,
            status_state: &mut sstate,
            permission: Some((&perm_w, &mut perm)),
            question: None,
        },
    );
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
