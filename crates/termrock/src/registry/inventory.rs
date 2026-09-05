// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Typed authority for the public widget inventory.
use std::collections::BTreeSet;

macro_rules! define_public_ui_ids {
    ($($id:ident),+ $(,)?) => {
        /// Stable public visual-owner identity shared by Rust, stories, and docs data.
        #[allow(missing_docs)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum PublicUiId {
            $($id),+
        }

        impl PublicUiId {
            /// Every registered identity, in stable lexical order.
            pub const ALL: &'static [Self] = &[$(Self::$id),+];

            /// Stable PascalCase Rust component name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$id => stringify!($id)),+
                }
            }

            /// Resolve only registered public component identities.
            #[must_use]
            pub fn parse(id: &str) -> Option<Self> {
                public_ui_by_id(id).map(|component| component.id)
            }
        }

        impl AsRef<str> for PublicUiId {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

define_public_ui_ids![
    AccentRail,
    Accordion,
    ActionBar,
    ActionLink,
    ActivityIndicator,
    ActivityShelf,
    AgentModeSelector,
    AgentStatusHeader,
    Alert,
    AlertDialog,
    AnsiText,
    ApprovalQueue,
    AttachmentChip,
    AvatarGlyph,
    Backdrop,
    BackgroundTaskPanel,
    Badge,
    Banner,
    BarSeries,
    Breadcrumbs,
    BusyBoundary,
    Button,
    ButtonGroup,
    Callout,
    Card,
    Carousel,
    Center,
    Chart,
    Checkbox,
    CheckpointTimeline,
    Chip,
    ChoiceDialog,
    ChromeRow,
    CitationList,
    CodeBlock,
    CodeFrame,
    Collapsible,
    Combobox,
    CommandPalette,
    CompletionMenu,
    ComposedRowParts,
    ComposerSelectors,
    ConfirmPrompt,
    ConnectionManager,
    ContextMeter,
    DataTable,
    DateTimePicker,
    DependencyGraph,
    Description,
    DesignInspector,
    DetailTable,
    DiagnosticView,
    Dialog,
    DialogScroll,
    DiffReview,
    DiffView,
    DismissableLayer,
    Drawer,
    DropdownMenu,
    EmptyState,
    ErrorState,
    EventStream,
    FieldCaption,
    FieldRow,
    FilePicker,
    FileTree,
    FocusGraph,
    FocusLens,
    Form,
    FormWizard,
    FullscreenViewer,
    Gauge,
    Grid,
    Heading,
    HexViewer,
    HighlightedText,
    HintBar,
    Histogram,
    HistoryPicker,
    Icon,
    IconButton,
    Identity,
    ImageSurface,
    Inline,
    InlineMention,
    InputGroup,
    InputOtp,
    IntegrationStatus,
    InteractionScene,
    JumpOverlay,
    Kbd,
    KeyValueList,
    KeyValueTable,
    KeybindingRecorder,
    KeyboardHelp,
    Label,
    Link,
    List,
    LoadingOverlay,
    LoadingView,
    LogPane,
    LogStream,
    MarkdownView,
    MenuBar,
    MessageDialog,
    MessageThread,
    MetricRadar,
    MetricTileView,
    MetricsDashboard,
    ModeRibbon,
    ModelSelector,
    MultiSelect,
    NavigationList,
    NotificationCenter,
    NumberInput,
    ObjectInspector,
    OfflineBanner,
    OfflineChrome,
    OfflineSurface,
    OverlayStack,
    Pagination,
    Panel,
    Paragraph,
    PasswordInput,
    PasteChip,
    PathInput,
    PermissionPrompt,
    Picker,
    PlanReview,
    Popover,
    PreviewCard,
    ProcessTable,
    ProgressBar,
    ProgressSteps,
    PromptComposer,
    PromptQueue,
    QueryEditor,
    QuestionFlow,
    QuickOpen,
    RadioGroup,
    RangeSlider,
    ResizablePanelGroup,
    ResultGrid,
    RovingFocusGroup,
    SchemaBrowser,
    ScrollArea,
    SearchInput,
    SearchResults,
    Section,
    SegmentedControl,
    SegmentedMeter,
    Select,
    SemanticScene,
    SemanticZoomBadge,
    Separator,
    SessionPicker,
    ShortcutHint,
    Sidebar,
    Skeleton,
    SlashCommandMenu,
    Slider,
    SourceCitation,
    Sparkline,
    Spinner,
    SplitPane,
    Stack,
    StatusBar,
    StatusIndicator,
    StatusStrip,
    Stepper,
    StreamingMarkdown,
    SubagentCard,
    Surface,
    Switch,
    Table,
    Tabs,
    Tag,
    TaskRail,
    TerminalCellGrid,
    TerminalOutput,
    TerminalRunCard,
    Text,
    TextArea,
    TextInput,
    ThemePicker,
    ThinkingBlock,
    Timeline,
    Toast,
    ToastStack,
    Toggle,
    ToggleGroup,
    TokenField,
    TokenMeter,
    TokenStrip,
    ToolCallCard,
    ToolCard,
    Toolbar,
    Tooltip,
    TraceWaterfall,
    Transcript,
    Tree,
    TreeNavigation,
    TreeTable,
    UiContext,
    Viewport,
    VirtualGrid,
    VirtualList,
    WorkSurface,
    WorkingStateCard,
    Workspace,
];

impl std::fmt::Display for PublicUiId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Product role of a public terminal component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ComponentFamily {
    /// Commands and direct activation controls.
    Action,
    /// Editable values and form controls.
    Input,
    /// Selection, wayfinding, and disclosure.
    Navigation,
    /// Structured or streaming data readers.
    Data,
    /// Status, progress, errors, and transient feedback.
    Feedback,
    /// Layered, modal, or anchored surfaces.
    Overlay,
    /// Spatial structure and text hierarchy.
    Layout,
    /// Charts, measures, and timelines.
    Visualization,
    /// Media and semantic content fragments.
    Content,
}

impl ComponentFamily {
    /// Stable docs/schema identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Input => "input",
            Self::Navigation => "navigation",
            Self::Data => "data",
            Self::Feedback => "feedback",
            Self::Overlay => "overlay",
            Self::Layout => "layout",
            Self::Visualization => "visualization",
            Self::Content => "content",
        }
    }
}

/// Public rendering contract exposed by a catalogued terminal symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ComponentKind {
    /// Implements ratatui's `Widget` or `StatefulWidget` contract.
    Widget,
    /// Paints directly into a terminal buffer through a public API.
    Paint,
    /// Computes terminal geometry without owning paint.
    Layout,
    /// Public behavior or composition model rendered by another component.
    Behavior,
}

impl ComponentKind {
    /// Stable docs/schema identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Widget => "widget",
            Self::Paint => "paint",
            Self::Layout => "layout",
            Self::Behavior => "behavior",
        }
    }
}

/// Canonical docs collection for one exact public visual owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DocumentationKind {
    /// Atomic component, layout, paint, or behavior page.
    Component,
    /// Exact public `termrock::patterns` type documented on a pattern page.
    Pattern,
}

impl DocumentationKind {
    /// Stable schema identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Component => "component",
            Self::Pattern => "pattern",
        }
    }

    /// Stable docs path prefix.
    #[must_use]
    pub const fn path_prefix(self) -> &'static str {
        match self {
            Self::Component => "/docs/components/",
            Self::Pattern => "/docs/patterns/",
        }
    }
}

/// One public component joined to docs routing and representative story data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicUiInventoryEntry {
    /// Stable typed component identity.
    pub id: PublicUiId,
    /// Public rendering contract of this exact symbol.
    pub kind: ComponentKind,
    /// Product family, independent of source-file layout.
    pub family: ComponentFamily,
    /// Canonical documentation collection.
    pub documentation: DocumentationKind,
    /// Stable docs route slug.
    pub docs_slug: &'static str,
    /// Representative story used by docs and gallery entry points.
    pub representative_story: &'static str,
}

impl PublicUiInventoryEntry {
    /// Unique canonical documentation path.
    #[must_use]
    pub fn docs_path(self) -> String {
        format!("{}{}", self.documentation.path_prefix(), self.docs_slug)
    }
}

/// Structural inventory failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PublicUiInventoryError {
    /// No components were supplied.
    Empty,
    /// Component id is not PascalCase ASCII.
    InvalidId(PublicUiId),
    /// Docs slug is not lowercase kebab-case ASCII.
    InvalidDocsSlug(PublicUiId),
    /// Representative story is not a stable `family/variant` id.
    InvalidRepresentativeStory(PublicUiId),
    /// Component id appears more than once.
    DuplicateId(PublicUiId),
    /// Docs slug appears more than once.
    DuplicateDocsSlug(PublicUiId),
    /// Component ids are not in stable lexical order.
    UnsortedId(PublicUiId),
}

impl std::fmt::Display for PublicUiInventoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("component inventory is empty"),
            Self::InvalidId(id) => write!(formatter, "invalid component id {id}"),
            Self::InvalidDocsSlug(id) => write!(formatter, "invalid docs slug for {id}"),
            Self::InvalidRepresentativeStory(id) => {
                write!(formatter, "invalid representative story for {id}")
            }
            Self::DuplicateId(id) => write!(formatter, "duplicate component id {id}"),
            Self::DuplicateDocsSlug(id) => write!(formatter, "duplicate docs slug for {id}"),
            Self::UnsortedId(id) => write!(formatter, "component id is out of order at {id}"),
        }
    }
}

impl std::error::Error for PublicUiInventoryError {}

macro_rules! public_ui {
    ($id:ident, $kind:ident, $family:ident, $slug:literal, $story:literal) => {
        PublicUiInventoryEntry {
            id: PublicUiId::$id,
            kind: ComponentKind::$kind,
            family: ComponentFamily::$family,
            documentation: DocumentationKind::Component,
            docs_slug: $slug,
            representative_story: $story,
        }
    };
}

macro_rules! public_pattern {
    ($id:ident, $kind:ident, $family:ident, $slug:literal, $story:literal) => {
        PublicUiInventoryEntry {
            id: PublicUiId::$id,
            kind: ComponentKind::$kind,
            family: ComponentFamily::$family,
            documentation: DocumentationKind::Pattern,
            docs_slug: $slug,
            representative_story: $story,
        }
    };
}

/// Canonical public component inventory.
pub static PUBLIC_UI_INVENTORY: &[PublicUiInventoryEntry] = &[
    public_ui!(
        AccentRail,
        Widget,
        Layout,
        "accent-rail",
        "accent-rail/actors"
    ),
    public_ui!(
        Accordion,
        Widget,
        Navigation,
        "accordion",
        "accordion/section"
    ),
    public_ui!(ActionBar, Widget, Action, "action-bar", "action-bar/basic"),
    public_ui!(
        ActionLink,
        Widget,
        Action,
        "action-link",
        "action-link/basic"
    ),
    public_ui!(
        ActivityIndicator,
        Paint,
        Feedback,
        "activity-indicator",
        "activity-indicator/basic"
    ),
    public_pattern!(
        ActivityShelf,
        Paint,
        Feedback,
        "activity-shelf",
        "activity-shelf/statuses"
    ),
    public_ui!(
        AgentModeSelector,
        Paint,
        Navigation,
        "agent-mode-selector",
        "agent-mode-selector/ribbon"
    ),
    public_pattern!(
        AgentStatusHeader,
        Widget,
        Feedback,
        "agent-status-header",
        "agent-status-header/basic"
    ),
    public_ui!(Alert, Widget, Feedback, "alert", "alert/danger"),
    public_ui!(
        AlertDialog,
        Widget,
        Overlay,
        "alert-dialog",
        "alert-dialog/delete"
    ),
    public_ui!(AnsiText, Widget, Content, "ansi-text", "ansi-text/basic"),
    public_pattern!(
        ApprovalQueue,
        Widget,
        Data,
        "approval-queue",
        "approval-queue/basic"
    ),
    public_ui!(
        AttachmentChip,
        Paint,
        Content,
        "attachment-chip",
        "attachment-chip/file"
    ),
    public_ui!(
        AvatarGlyph,
        Widget,
        Content,
        "avatar-glyph",
        "avatar-glyph/basic"
    ),
    public_ui!(Backdrop, Widget, Overlay, "backdrop", "backdrop/basic"),
    public_pattern!(
        BackgroundTaskPanel,
        Widget,
        Feedback,
        "background-task-panel",
        "background-tasks/mixed-statuses"
    ),
    public_ui!(Badge, Widget, Feedback, "badge", "badge/basic"),
    public_ui!(Banner, Widget, Feedback, "banner", "banner/basic"),
    public_ui!(
        BarSeries,
        Widget,
        Visualization,
        "bar-series",
        "bar-series/basic"
    ),
    public_ui!(
        Breadcrumbs,
        Widget,
        Navigation,
        "breadcrumbs",
        "breadcrumbs/path"
    ),
    public_ui!(
        BusyBoundary,
        Paint,
        Feedback,
        "busy-boundary",
        "loading-overlay/nested"
    ),
    public_ui!(Button, Widget, Action, "button", "button/activation"),
    public_ui!(
        ButtonGroup,
        Widget,
        Action,
        "button-group",
        "button-group/dialog"
    ),
    public_ui!(Callout, Widget, Feedback, "callout", "callout/basic"),
    public_ui!(Card, Widget, Layout, "card", "card/basic"),
    public_ui!(Carousel, Paint, Navigation, "carousel", "carousel/basic"),
    public_ui!(Center, Layout, Layout, "center", "center/both"),
    public_ui!(Chart, Widget, Visualization, "chart", "chart/basic"),
    public_ui!(Checkbox, Widget, Action, "checkbox", "checkbox/switch"),
    public_ui!(
        CheckpointTimeline,
        Widget,
        Visualization,
        "checkpoint-timeline",
        "checkpoint-timeline/basic"
    ),
    public_ui!(Chip, Paint, Input, "chip", "chip/filter"),
    public_ui!(
        ChoiceDialog,
        Widget,
        Overlay,
        "choice-dialog",
        "choice-dialog/basic"
    ),
    public_ui!(ChromeRow, Paint, Layout, "chrome-row", "chrome-row/basic"),
    public_ui!(
        CitationList,
        Paint,
        Content,
        "citation-list",
        "citation-list/expanded"
    ),
    public_ui!(CodeBlock, Widget, Data, "code-block", "code-block/basic"),
    public_ui!(
        CodeFrame,
        Paint,
        Data,
        "code-frame",
        "diagnostic/code-frame"
    ),
    public_ui!(
        Collapsible,
        Widget,
        Navigation,
        "collapsible",
        "collapsible/inline"
    ),
    public_ui!(Combobox, Paint, Input, "combobox", "combobox/basic"),
    public_ui!(
        CommandPalette,
        Widget,
        Navigation,
        "command-palette",
        "command-palette/basic"
    ),
    public_ui!(
        CompletionMenu,
        Widget,
        Navigation,
        "completion-menu",
        "completion-menu/basic"
    ),
    public_ui!(
        ComposedRowParts,
        Paint,
        Layout,
        "composed-row-parts",
        "composed-row-parts/basic"
    ),
    public_ui!(
        ComposerSelectors,
        Paint,
        Input,
        "composer-selectors",
        "composer-selectors/strip"
    ),
    public_ui!(
        ConfirmPrompt,
        Paint,
        Action,
        "confirm-prompt",
        "confirm-prompt/basic"
    ),
    public_pattern!(
        ConnectionManager,
        Widget,
        Navigation,
        "connection-manager",
        "connection-manager/full"
    ),
    public_ui!(
        ContextMeter,
        Paint,
        Feedback,
        "context-meter",
        "context-meter/low-mid-high"
    ),
    public_ui!(DataTable, Widget, Data, "data-table", "data-table/toolbar"),
    public_ui!(
        DateTimePicker,
        Widget,
        Input,
        "date-time-picker",
        "date-time-picker/date"
    ),
    public_ui!(
        DependencyGraph,
        Paint,
        Data,
        "dependency-graph",
        "dependency-graph/basic"
    ),
    public_ui!(
        Description,
        Widget,
        Layout,
        "description",
        "description/kinds"
    ),
    public_ui!(
        DesignInspector,
        Widget,
        Content,
        "design-inspector",
        "design-inspector/basic"
    ),
    public_ui!(
        DetailTable,
        Widget,
        Data,
        "detail-table",
        "detail-table/basic"
    ),
    public_ui!(
        DiagnosticView,
        Widget,
        Data,
        "diagnostic-view",
        "diagnostic/list"
    ),
    public_ui!(Dialog, Widget, Overlay, "dialog", "dialog/message"),
    public_ui!(
        DialogScroll,
        Behavior,
        Overlay,
        "dialog-scroll",
        "dialog-scroll/basic"
    ),
    public_ui!(DiffReview, Widget, Data, "diff-review", "diff-review/hunks"),
    public_ui!(DiffView, Widget, Data, "diff-view", "diff/basic"),
    public_ui!(
        DismissableLayer,
        Behavior,
        Overlay,
        "dismissable-layer",
        "dismissable/gestures"
    ),
    public_ui!(Drawer, Widget, Overlay, "drawer", "drawer/basic"),
    public_ui!(
        DropdownMenu,
        Widget,
        Navigation,
        "dropdown-menu",
        "dropdown-menu/basic"
    ),
    public_ui!(
        EmptyState,
        Widget,
        Feedback,
        "empty-state",
        "empty-state/basic"
    ),
    public_ui!(
        ErrorState,
        Widget,
        Feedback,
        "error-state",
        "error-state/network"
    ),
    public_ui!(
        EventStream,
        Widget,
        Data,
        "event-stream",
        "event-stream/basic"
    ),
    public_ui!(
        FieldCaption,
        Widget,
        Input,
        "field-caption",
        "field-caption/basic"
    ),
    public_ui!(FieldRow, Widget, Input, "field-row", "field-row/states"),
    public_ui!(FilePicker, Widget, Input, "file-picker", "file-picker/unix"),
    public_ui!(FileTree, Paint, Data, "file-tree", "file-tree/basic"),
    public_ui!(
        FocusGraph,
        Behavior,
        Navigation,
        "focus-graph",
        "focus-graph/workbench"
    ),
    public_ui!(
        FocusLens,
        Widget,
        Feedback,
        "focus-lens",
        "focus-lens/combined"
    ),
    public_ui!(Form, Widget, Input, "form", "form/responsive"),
    public_ui!(
        FormWizard,
        Widget,
        Input,
        "form-wizard",
        "blocks/form-wizard"
    ),
    public_ui!(
        FullscreenViewer,
        Widget,
        Overlay,
        "fullscreen-viewer",
        "fullscreen-viewer/basic"
    ),
    public_ui!(Gauge, Widget, Visualization, "gauge", "gauge/basic"),
    public_ui!(Grid, Layout, Layout, "grid", "grid/columns"),
    public_ui!(Heading, Widget, Layout, "heading", "heading/basic"),
    public_ui!(HexViewer, Widget, Data, "hex-viewer", "hex-viewer/basic"),
    public_ui!(
        HighlightedText,
        Widget,
        Content,
        "highlighted-text",
        "highlighted-text/basic"
    ),
    public_ui!(HintBar, Widget, Layout, "hint-bar", "hint-bar/wrapped"),
    public_ui!(
        Histogram,
        Widget,
        Visualization,
        "histogram",
        "histogram/basic"
    ),
    public_ui!(
        HistoryPicker,
        Widget,
        Navigation,
        "history-picker",
        "history-picker/basic"
    ),
    public_ui!(Icon, Widget, Content, "icon", "icon/browser"),
    public_ui!(IconButton, Paint, Action, "icon-button", "button/icon"),
    public_ui!(Identity, Widget, Content, "identity", "identity/basic"),
    public_ui!(
        ImageSurface,
        Widget,
        Content,
        "image-surface",
        "image-surface/basic"
    ),
    public_ui!(Inline, Layout, Layout, "inline", "stack/inline"),
    public_ui!(
        InlineMention,
        Paint,
        Content,
        "inline-mention",
        "inline-mention/basic"
    ),
    public_ui!(InputGroup, Paint, Input, "input-group", "input-group/basic"),
    public_ui!(InputOtp, Paint, Input, "input-otp", "input-otp/basic"),
    public_pattern!(
        IntegrationStatus,
        Widget,
        Feedback,
        "integration-status",
        "integration-status/list"
    ),
    public_ui!(
        InteractionScene,
        Behavior,
        Navigation,
        "interaction-scene",
        "interaction-scene/basic"
    ),
    public_ui!(
        JumpOverlay,
        Widget,
        Navigation,
        "jump-overlay",
        "jump-overlay/basic"
    ),
    public_ui!(Kbd, Widget, Content, "kbd", "kbd/basic"),
    public_ui!(
        KeyValueList,
        Paint,
        Data,
        "key-value-list",
        "key-value-list/basic"
    ),
    public_ui!(
        KeyValueTable,
        Widget,
        Data,
        "key-value-table",
        "key-value-table/http"
    ),
    public_ui!(
        KeybindingRecorder,
        Widget,
        Input,
        "keybinding-recorder",
        "keybinding-recorder/idle"
    ),
    public_ui!(
        KeyboardHelp,
        Widget,
        Navigation,
        "keyboard-help",
        "keyboard-help/footer"
    ),
    public_ui!(Label, Widget, Layout, "label", "label/basic"),
    public_ui!(Link, Widget, Action, "link", "link/basic"),
    public_ui!(List, Widget, Navigation, "list", "list/selection"),
    public_ui!(
        LoadingOverlay,
        Paint,
        Overlay,
        "loading-overlay",
        "loading-overlay/blocking"
    ),
    public_ui!(
        LoadingView,
        Widget,
        Feedback,
        "loading-view",
        "loading-view/basic"
    ),
    public_ui!(LogPane, Widget, Data, "log-pane", "log-pane/follow"),
    public_ui!(LogStream, Widget, Data, "log-stream", "log-stream/follow"),
    public_ui!(
        MarkdownView,
        Widget,
        Data,
        "markdown-view",
        "markdown-view/basic"
    ),
    public_ui!(MenuBar, Widget, Navigation, "menu-bar", "menu-bar/basic"),
    public_ui!(
        MessageDialog,
        Widget,
        Overlay,
        "message-dialog",
        "message-dialog/details"
    ),
    public_ui!(
        MessageThread,
        Paint,
        Data,
        "message-thread",
        "message-thread/basic"
    ),
    public_ui!(
        MetricRadar,
        Widget,
        Visualization,
        "metric-radar",
        "metric-radar/basic"
    ),
    public_ui!(
        MetricTileView,
        Widget,
        Visualization,
        "metric-tile",
        "metric-tile/basic"
    ),
    public_pattern!(
        MetricsDashboard,
        Paint,
        Visualization,
        "metrics-dashboard",
        "metrics-dashboard/basic"
    ),
    public_ui!(
        ModeRibbon,
        Widget,
        Navigation,
        "mode-ribbon",
        "mode-ribbon/basic"
    ),
    public_ui!(
        ModelSelector,
        Paint,
        Input,
        "model-selector",
        "model-selector/compact"
    ),
    public_ui!(
        MultiSelect,
        Widget,
        Input,
        "multi-select",
        "multi-select/basic"
    ),
    public_ui!(
        NavigationList,
        Widget,
        Navigation,
        "navigation-list",
        "navigation-list/basic"
    ),
    public_ui!(
        NotificationCenter,
        Widget,
        Overlay,
        "notification-center",
        "notification-center/drawer"
    ),
    public_ui!(
        NumberInput,
        Widget,
        Input,
        "number-input",
        "number-input/basic"
    ),
    public_ui!(
        ObjectInspector,
        Widget,
        Data,
        "object-inspector",
        "object-inspector/flat"
    ),
    public_ui!(
        OfflineBanner,
        Widget,
        Feedback,
        "offline-banner",
        "connectivity/banner"
    ),
    public_ui!(
        OfflineChrome,
        Paint,
        Feedback,
        "offline-chrome",
        "offline-chrome/basic"
    ),
    public_ui!(
        OfflineSurface,
        Paint,
        Feedback,
        "offline-surface",
        "connectivity/reconnecting"
    ),
    public_ui!(
        OverlayStack,
        Behavior,
        Overlay,
        "overlay-stack",
        "overlay/nested-escape"
    ),
    public_ui!(
        Pagination,
        Widget,
        Navigation,
        "pagination",
        "pagination/full"
    ),
    public_ui!(Panel, Widget, Layout, "panel", "panel/focused"),
    public_ui!(Paragraph, Widget, Layout, "paragraph", "paragraph/basic"),
    public_ui!(
        PasswordInput,
        Widget,
        Input,
        "password-input",
        "password-input/basic"
    ),
    public_ui!(PasteChip, Paint, Content, "paste-chip", "paste-chip/large"),
    public_ui!(PathInput, Widget, Input, "path-input", "path-input/basic"),
    public_ui!(
        PermissionPrompt,
        Widget,
        Overlay,
        "permission-prompt",
        "permission-prompt/basic"
    ),
    public_ui!(Picker, Widget, Input, "picker", "picker/basic"),
    public_pattern!(PlanReview, Widget, Data, "plan-review", "plan-review/basic"),
    public_ui!(Popover, Widget, Overlay, "popover", "popover/basic"),
    public_ui!(
        PreviewCard,
        Widget,
        Overlay,
        "preview-card",
        "preview-card/file"
    ),
    public_pattern!(
        ProcessTable,
        Paint,
        Data,
        "process-table",
        "process-table/basic"
    ),
    public_ui!(
        ProgressBar,
        Widget,
        Feedback,
        "progress-bar",
        "progress-bar/basic"
    ),
    public_ui!(
        ProgressSteps,
        Widget,
        Feedback,
        "progress-steps",
        "progress-steps/pipeline"
    ),
    public_ui!(
        PromptComposer,
        Widget,
        Input,
        "prompt-composer",
        "prompt-composer/basic"
    ),
    public_pattern!(
        PromptQueue,
        Widget,
        Data,
        "prompt-queue",
        "prompt-queue/compact"
    ),
    public_pattern!(
        QueryEditor,
        Paint,
        Input,
        "query-editor",
        "query-editor/basic"
    ),
    public_ui!(
        QuestionFlow,
        Widget,
        Input,
        "question-flow",
        "question-flow/basic"
    ),
    public_ui!(
        QuickOpen,
        Widget,
        Navigation,
        "quick-open",
        "quick-open/basic"
    ),
    public_ui!(RadioGroup, Paint, Input, "radio-group", "radio-group/basic"),
    public_ui!(
        RangeSlider,
        Widget,
        Input,
        "range-slider",
        "range-slider/basic"
    ),
    public_ui!(
        ResizablePanelGroup,
        Widget,
        Layout,
        "resizable-panel-group",
        "resizable-panel-group/workbench"
    ),
    public_pattern!(ResultGrid, Paint, Data, "result-grid", "result-grid/basic"),
    public_ui!(
        RovingFocusGroup,
        Behavior,
        Navigation,
        "roving-focus-group",
        "roving-focus/group"
    ),
    public_pattern!(
        SchemaBrowser,
        Paint,
        Navigation,
        "schema-browser",
        "schema-browser/basic"
    ),
    public_ui!(
        ScrollArea,
        Paint,
        Data,
        "scroll-area",
        "scroll-area/follow-paused"
    ),
    public_ui!(
        SearchInput,
        Widget,
        Input,
        "search-input",
        "search-input/basic"
    ),
    public_ui!(
        SearchResults,
        Paint,
        Data,
        "search-results",
        "search-results/basic"
    ),
    public_ui!(Section, Widget, Layout, "section", "section/quiet"),
    public_ui!(
        SegmentedControl,
        Widget,
        Navigation,
        "segmented-control",
        "segmented-control/basic"
    ),
    public_ui!(
        SegmentedMeter,
        Widget,
        Visualization,
        "segmented-meter",
        "segmented-meter/basic"
    ),
    public_ui!(Select, Widget, Input, "select", "select/basic"),
    public_ui!(
        SemanticScene,
        Behavior,
        Content,
        "semantic-scene",
        "semantic-scene/tree"
    ),
    public_ui!(
        SemanticZoomBadge,
        Paint,
        Feedback,
        "semantic-zoom-badge",
        "semantic-zoom-badge/basic"
    ),
    public_ui!(Separator, Widget, Layout, "separator", "separator/basic"),
    public_pattern!(
        SessionPicker,
        Widget,
        Navigation,
        "session-picker",
        "session-picker/basic"
    ),
    public_ui!(
        ShortcutHint,
        Widget,
        Layout,
        "shortcut-hint",
        "shortcut-hint/footer"
    ),
    public_ui!(Sidebar, Widget, Navigation, "sidebar", "sidebar/settings"),
    public_ui!(Skeleton, Widget, Feedback, "skeleton", "skeleton/basic"),
    public_ui!(
        SlashCommandMenu,
        Paint,
        Navigation,
        "slash-command-menu",
        "slash-command-menu/filter"
    ),
    public_ui!(Slider, Widget, Input, "slider", "slider/basic"),
    public_ui!(
        SourceCitation,
        Paint,
        Content,
        "source-citation",
        "source-citation/inline"
    ),
    public_ui!(
        Sparkline,
        Widget,
        Visualization,
        "sparkline",
        "sparkline/basic"
    ),
    public_ui!(Spinner, Widget, Feedback, "spinner", "spinner/labeled"),
    public_ui!(
        SplitPane,
        Widget,
        Layout,
        "split-pane",
        "split-pane/horizontal"
    ),
    public_ui!(Stack, Layout, Layout, "stack", "stack/vertical"),
    public_ui!(
        StatusBar,
        Widget,
        Feedback,
        "status-bar",
        "status-bar/basic"
    ),
    public_ui!(
        StatusIndicator,
        Widget,
        Feedback,
        "status-indicator",
        "status-indicator/catalog"
    ),
    public_ui!(
        StatusStrip,
        Paint,
        Feedback,
        "status-strip",
        "status-strip/basic"
    ),
    public_ui!(Stepper, Widget, Navigation, "stepper", "stepper/horizontal"),
    public_ui!(
        StreamingMarkdown,
        Paint,
        Data,
        "streaming-markdown",
        "streaming-markdown/mid-fence"
    ),
    public_pattern!(
        SubagentCard,
        Paint,
        Content,
        "subagent-card",
        "subagent-card/running"
    ),
    public_ui!(Surface, Widget, Layout, "surface", "surface/ladder"),
    public_ui!(Switch, Widget, Action, "switch", "switch/basic"),
    public_ui!(Table, Widget, Data, "table", "table/basic"),
    public_ui!(Tabs, Widget, Navigation, "tabs", "tabs/status"),
    public_ui!(Tag, Paint, Content, "tag", "tag/removable"),
    public_pattern!(TaskRail, Widget, Navigation, "task-rail", "task-rail/basic"),
    public_ui!(
        TerminalCellGrid,
        Widget,
        Data,
        "terminal-cell-grid",
        "terminal-cell-grid/basic"
    ),
    public_ui!(
        TerminalOutput,
        Widget,
        Data,
        "terminal-output",
        "terminal-output/running"
    ),
    public_pattern!(
        TerminalRunCard,
        Paint,
        Content,
        "terminal-run-card",
        "terminal-run-card/running"
    ),
    public_ui!(Text, Widget, Layout, "text", "text/basic"),
    public_ui!(TextArea, Widget, Input, "text-area", "text-area/basic"),
    public_ui!(TextInput, Widget, Input, "text-input", "text-input/basic"),
    public_ui!(
        ThemePicker,
        Widget,
        Input,
        "theme-picker",
        "system-picker/basic"
    ),
    public_ui!(
        ThinkingBlock,
        Widget,
        Feedback,
        "thinking-block",
        "thinking-block/basic"
    ),
    public_ui!(
        Timeline,
        Widget,
        Visualization,
        "timeline",
        "timeline/basic"
    ),
    public_ui!(Toast, Widget, Feedback, "toast", "toast/success"),
    public_ui!(
        ToastStack,
        Paint,
        Feedback,
        "toast-stack",
        "toast-stack/basic"
    ),
    public_ui!(Toggle, Widget, Action, "toggle", "toggle/pressed"),
    public_ui!(
        ToggleGroup,
        Widget,
        Action,
        "toggle-group",
        "toggle-group/format"
    ),
    public_ui!(
        TokenField,
        Widget,
        Input,
        "token-field",
        "token-field/basic"
    ),
    public_ui!(
        TokenMeter,
        Widget,
        Visualization,
        "token-meter",
        "token-meter/basic"
    ),
    public_ui!(TokenStrip, Paint, Input, "token-strip", "token-strip/wrap"),
    public_ui!(
        ToolCallCard,
        Paint,
        Content,
        "tool-call-card",
        "tool-call-card/running"
    ),
    public_ui!(ToolCard, Widget, Content, "tool-card", "tool-card/basic"),
    public_ui!(Toolbar, Widget, Action, "toolbar", "toolbar/basic"),
    public_ui!(Tooltip, Widget, Feedback, "tooltip", "tooltip/plain"),
    public_ui!(
        TraceWaterfall,
        Paint,
        Visualization,
        "trace-waterfall",
        "trace-waterfall/basic"
    ),
    public_ui!(Transcript, Widget, Data, "transcript", "transcript/basic"),
    public_ui!(Tree, Widget, Navigation, "tree", "tree/navigation"),
    public_ui!(
        TreeNavigation,
        Widget,
        Navigation,
        "tree-navigation",
        "tree-navigation/project"
    ),
    public_ui!(TreeTable, Widget, Data, "tree-table", "tree-table/process"),
    public_ui!(
        UiContext,
        Behavior,
        Layout,
        "ui-context",
        "ui-context/frame"
    ),
    public_ui!(Viewport, Widget, Layout, "viewport", "viewport/both-axes"),
    public_ui!(
        VirtualGrid,
        Widget,
        Data,
        "virtual-grid",
        "virtual-grid/basic"
    ),
    public_ui!(
        VirtualList,
        Widget,
        Data,
        "virtual-list",
        "virtual-list/million"
    ),
    public_ui!(
        WorkSurface,
        Layout,
        Layout,
        "work-surface",
        "work-surface/basic"
    ),
    public_pattern!(
        WorkingStateCard,
        Widget,
        Feedback,
        "working-state-card",
        "working-state-card/basic"
    ),
    public_ui!(Workspace, Layout, Layout, "workspace", "workspace/basic"),
];

/// Borrow the canonical inventory in stable component-id order.
#[must_use]
pub const fn public_ui_inventory() -> &'static [PublicUiInventoryEntry] {
    PUBLIC_UI_INVENTORY
}

/// Resolve one public component by stable identity.
#[must_use]
pub fn public_ui_by_id(id: &str) -> Option<&'static PublicUiInventoryEntry> {
    PUBLIC_UI_INVENTORY
        .iter()
        .find(|component| component.id.as_str() == id)
}

/// Validate identity, docs routing, representative stories, and uniqueness.
pub fn validate_public_ui_inventory(
    inventory: &[PublicUiInventoryEntry],
) -> Result<(), PublicUiInventoryError> {
    if inventory.is_empty() {
        return Err(PublicUiInventoryError::Empty);
    }
    let mut ids = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    let mut previous_id = None;
    for component in inventory {
        if !valid_public_ui_id(component.id.as_str()) {
            return Err(PublicUiInventoryError::InvalidId(component.id));
        }
        if !valid_slug(component.docs_slug) {
            return Err(PublicUiInventoryError::InvalidDocsSlug(component.id));
        }
        if !valid_story_id(component.representative_story) {
            return Err(PublicUiInventoryError::InvalidRepresentativeStory(
                component.id,
            ));
        }
        if !ids.insert(component.id.as_str()) {
            return Err(PublicUiInventoryError::DuplicateId(component.id));
        }
        if !slugs.insert(component.docs_slug) {
            return Err(PublicUiInventoryError::DuplicateDocsSlug(component.id));
        }
        if previous_id.is_some_and(|previous| previous >= component.id.as_str()) {
            return Err(PublicUiInventoryError::UnsortedId(component.id));
        }
        previous_id = Some(component.id.as_str());
    }
    Ok(())
}

fn valid_public_ui_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_uppercase())
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_story_id(story: &str) -> bool {
    let Some((family, variant)) = story.split_once('/') else {
        return false;
    };
    valid_slug(family) && valid_slug(variant)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
    };

    use syn::{
        FnArg, ImplItem, Item, ItemImpl, ReturnType, Type, UseTree, Visibility,
        visit::{self, Visit},
    };

    use super::*;

    const EXCLUDED_PUBLIC_SUPPORT_TYPES: &[(&str, &str)] = &[
        ("CollectionState", "headless collection state"),
        ("ComponentContract", "registry metadata model"),
        (
            "DatabaseWorkbenchState",
            "pattern state that delegates workspace geometry",
        ),
        (
            "DesignSystem",
            "shared style authority, not one visual surface",
        ),
        ("EntityMention", "inline-mention data model"),
        ("EventResult", "interaction result protocol"),
        ("Field", "form field data model"),
        ("Fieldset", "form grouping data model"),
        ("FileMention", "inline-mention data model"),
        ("FrameClock", "runtime time source"),
        (
            "GitWorkbenchState",
            "pattern state that delegates workspace geometry",
        ),
        ("MentionDraft", "mention editor data model"),
        (
            "ObservabilityDashboardState",
            "pattern state that delegates workspace geometry",
        ),
        ("ReconnectingState", "connectivity state model"),
        ("ResponsiveSnapshot", "responsive diagnostic model"),
        ("SelectionModel", "headless selection state"),
        ("TerminalCapabilities", "host capability model"),
        ("TailScroll", "headless follow-scroll state"),
        ("Virtualizer", "headless viewport projection"),
    ];

    #[derive(Debug)]
    struct PublicMethod {
        name: String,
        type_names: BTreeSet<String>,
    }

    #[derive(Default)]
    struct SourceApi {
        declarations: BTreeMap<String, PathBuf>,
        exported_types: BTreeSet<String>,
        widget_owners: BTreeSet<String>,
        methods: BTreeMap<String, Vec<PublicMethod>>,
    }

    fn rust_files(path: &Path, output: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
            .map(|entry| entry.expect("source directory entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                rust_files(&entry, output);
            } else if entry.extension().is_some_and(|extension| extension == "rs")
                && !matches!(
                    entry.file_name().and_then(|name| name.to_str()),
                    Some("inventory.rs" | "pattern_inventory.rs")
                )
            {
                output.push(entry);
            }
        }
    }

    fn public_use_names(tree: &UseTree, names: &mut BTreeSet<String>) {
        match tree {
            UseTree::Path(path) => public_use_names(&path.tree, names),
            UseTree::Name(name) if name.ident != "self" => {
                names.insert(name.ident.to_string());
            }
            UseTree::Rename(rename) => {
                names.insert(rename.rename.to_string());
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    public_use_names(item, names);
                }
            }
            UseTree::Glob(_) | UseTree::Name(_) => (),
        }
    }

    fn collect_exported_types(root: &Path, api: &mut SourceApi) {
        let lib_path = root.join("lib.rs");
        let lib = syn::parse_file(&fs::read_to_string(&lib_path).expect("crate root source"))
            .expect("parse crate root");
        for item in lib.items {
            let Item::Mod(module) = item else { continue };
            if !matches!(module.vis, Visibility::Public(_)) {
                continue;
            }
            let module_name = module.ident.to_string();
            let file = root.join(format!("{module_name}.rs"));
            let module_path = if file.is_file() {
                file
            } else {
                root.join(module_name).join("mod.rs")
            };
            let module_source = fs::read_to_string(&module_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", module_path.display()));
            let module_file = syn::parse_file(&module_source)
                .unwrap_or_else(|error| panic!("parse {}: {error}", module_path.display()));
            for module_item in module_file.items {
                match module_item {
                    Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.exported_types.insert(item.ident.to_string());
                    }
                    Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.exported_types.insert(item.ident.to_string());
                    }
                    Item::Type(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.exported_types.insert(item.ident.to_string());
                    }
                    Item::Use(item) if matches!(item.vis, Visibility::Public(_)) => {
                        public_use_names(&item.tree, &mut api.exported_types);
                    }
                    _ => (),
                }
            }
        }
    }

    fn type_name(type_: &Type) -> Option<String> {
        match type_ {
            Type::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            Type::Reference(reference) => type_name(&reference.elem),
            Type::Paren(paren) => type_name(&paren.elem),
            Type::Group(group) => type_name(&group.elem),
            _ => None,
        }
    }

    #[derive(Default)]
    struct TypeNameVisitor {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for TypeNameVisitor {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            self.names.extend(
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string()),
            );
            visit::visit_type_path(self, path);
        }
    }

    fn method_type_names(method: &syn::ImplItemFn) -> BTreeSet<String> {
        let mut visitor = TypeNameVisitor::default();
        for input in &method.sig.inputs {
            if let FnArg::Typed(argument) = input {
                visitor.visit_type(&argument.ty);
            }
        }
        if let ReturnType::Type(_, output) = &method.sig.output {
            visitor.visit_type(output);
        }
        visitor.names
    }

    fn collect_impl(api: &mut SourceApi, implementation: &ItemImpl) {
        let Some(owner) = type_name(&implementation.self_ty) else {
            return;
        };
        if let Some((_, trait_path, _)) = &implementation.trait_
            && trait_path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "Widget" | "StatefulWidget"
                )
            })
        {
            api.widget_owners.insert(owner.clone());
        }
        if implementation.trait_.is_some() {
            return;
        }
        for item in &implementation.items {
            let ImplItem::Fn(method) = item else { continue };
            if matches!(method.vis, Visibility::Public(_)) {
                api.methods
                    .entry(owner.clone())
                    .or_default()
                    .push(PublicMethod {
                        name: method.sig.ident.to_string(),
                        type_names: method_type_names(method),
                    });
            }
        }
    }

    fn source_api() -> SourceApi {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&root, &mut files);
        let mut api = SourceApi::default();
        collect_exported_types(&root, &mut api);
        for file in files {
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
            let parsed = syn::parse_file(&source)
                .unwrap_or_else(|error| panic!("parse {}: {error}", file.display()));
            for item in parsed.items {
                match item {
                    Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.declarations
                            .insert(item.ident.to_string(), file.clone());
                    }
                    Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.declarations
                            .insert(item.ident.to_string(), file.clone());
                    }
                    Item::Type(item) if matches!(item.vis, Visibility::Public(_)) => {
                        api.declarations
                            .insert(item.ident.to_string(), file.clone());
                    }
                    Item::Impl(implementation) => collect_impl(&mut api, &implementation),
                    _ => (),
                }
            }
        }
        api.widget_owners
            .retain(|owner| api.exported_types.contains(owner));
        api.methods
            .retain(|owner, _| api.exported_types.contains(owner));
        api
    }

    fn is_terminal_paint(method: &PublicMethod) -> bool {
        (method.name == "paint"
            || method.name.starts_with("paint_")
            || method.name == "render"
            || method.name.starts_with("render_"))
            && (method.type_names.contains("Frame") || method.type_names.contains("Buffer"))
    }

    fn is_terminal_layout(method: &PublicMethod) -> bool {
        method.name == "layout" && method.type_names.contains("Rect")
    }

    fn behavior_contract(name: &str, declaration: &Path, methods: &[PublicMethod]) -> bool {
        let behavior_namespace = declaration.to_string_lossy().contains("/src/interaction/")
            || declaration.ends_with("src/context.rs")
            || declaration.ends_with("src/scroll/mod.rs");
        behavior_namespace
            && !methods.is_empty()
            && [
                "Layer", "Graph", "Scene", "Group", "Stack", "Context", "Scroll",
            ]
            .iter()
            .any(|suffix| name.ends_with(suffix))
    }

    fn discovered_visual_owners(api: &SourceApi) -> BTreeMap<String, ComponentKind> {
        let mut owners = BTreeMap::new();
        for name in &api.exported_types {
            let methods = api.methods.get(name).map(Vec::as_slice).unwrap_or_default();
            let kind = if api.widget_owners.contains(name) {
                Some(ComponentKind::Widget)
            } else if api
                .declarations
                .get(name)
                .is_some_and(|path| behavior_contract(name, path, methods))
            {
                Some(ComponentKind::Behavior)
            } else if methods.iter().any(is_terminal_paint) {
                Some(ComponentKind::Paint)
            } else if methods.iter().any(is_terminal_layout) {
                Some(ComponentKind::Layout)
            } else {
                None
            };
            if let Some(kind) = kind {
                owners.insert(name.clone(), kind);
            }
        }
        owners
    }

    #[test]
    fn source_public_api_exactly_matches_typed_inventory() {
        let api = source_api();
        let mut discovered = discovered_visual_owners(&api);
        for (id, reason) in EXCLUDED_PUBLIC_SUPPORT_TYPES {
            assert!(
                !reason.is_empty(),
                "excluded support type {id} needs a reason"
            );
            assert!(
                api.exported_types.contains(*id),
                "excluded support type {id} is no longer in the public API"
            );
            assert_eq!(
                PublicUiId::parse(id),
                None,
                "support/model type {id} must not enter the visual-owner inventory"
            );
            discovered.remove(*id);
        }
        let inventory: BTreeMap<_, _> = public_ui_inventory()
            .iter()
            .map(|entry| (entry.id.as_str().to_owned(), entry.kind))
            .collect();
        assert_eq!(discovered, inventory);
    }

    #[test]
    fn public_inventory_is_complete_unique_and_joinable() {
        assert_eq!(public_ui_inventory().len(), 210);
        assert_eq!(public_ui_inventory().len(), PublicUiId::ALL.len());
        assert_eq!(
            PublicUiId::ALL.iter().copied().collect::<BTreeSet<_>>(),
            public_ui_inventory()
                .iter()
                .map(|component| component.id)
                .collect()
        );
        assert_eq!(validate_public_ui_inventory(public_ui_inventory()), Ok(()));
        let kind_counts =
            public_ui_inventory()
                .iter()
                .fold([0usize; 4], |mut counts, component| {
                    let index = match component.kind {
                        ComponentKind::Widget => 0,
                        ComponentKind::Paint => 1,
                        ComponentKind::Layout => 2,
                        ComponentKind::Behavior => 3,
                    };
                    counts[index] += 1;
                    counts
                });
        assert_eq!(kind_counts, [148, 48, 6, 8]);
        assert_eq!(
            public_ui_inventory()
                .iter()
                .filter(|entry| entry.documentation == DocumentationKind::Component)
                .count(),
            192
        );
        assert_eq!(
            public_ui_inventory()
                .iter()
                .filter(|entry| entry.documentation == DocumentationKind::Pattern)
                .count(),
            18
        );
        for component in public_ui_inventory() {
            assert_eq!(PublicUiId::parse(component.id.as_str()), Some(component.id));
        }
    }

    #[test]
    fn duplicate_identity_is_rejected() {
        let component = public_ui_inventory()[0];
        assert_eq!(
            validate_public_ui_inventory(&[component, component]),
            Err(PublicUiInventoryError::DuplicateId(component.id))
        );
    }

    #[test]
    fn exact_symbols_own_distinct_docs_pages_and_stories() {
        let loading = public_ui_by_id("LoadingOverlay").expect("loading overlay");
        let boundary = public_ui_by_id("BusyBoundary").expect("busy boundary");
        let stack = public_ui_by_id("Stack").expect("stack");
        let inline = public_ui_by_id("Inline").expect("inline");
        assert_ne!(loading.docs_path(), boundary.docs_path());
        assert_ne!(stack.docs_path(), inline.docs_path());
        assert_ne!(loading.id, boundary.id);
        assert_ne!(loading.representative_story, boundary.representative_story);
        assert_ne!(stack.representative_story, inline.representative_story);
    }
}
