// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Public visual-component → catalog page ownership.
//!
//! Every [`termrock::registry::PublicUiId`] maps to exactly one catalog page.
//! Source-prefix pages own the widgets they already demonstrate. Remaining
//! identities live on TermRock-only pages after the frozen prefix.

use termrock::registry::PublicUiId;

use crate::catalog::PageId;

/// Components presented on the BUTTONS page.
pub const BUTTONS_UI: &[PublicUiId] = &[
    PublicUiId::Button,
    PublicUiId::IconButton,
    PublicUiId::ButtonGroup,
    PublicUiId::ActionBar,
    PublicUiId::ActionLink,
    PublicUiId::Toolbar,
    PublicUiId::ToggleGroup,
    PublicUiId::Icon,
];

/// Components presented on the INPUTS page.
pub const INPUTS_UI: &[PublicUiId] = &[
    PublicUiId::TextInput,
    PublicUiId::PasswordInput,
    PublicUiId::NumberInput,
    PublicUiId::SearchInput,
    PublicUiId::PathInput,
    PublicUiId::InputGroup,
    PublicUiId::InputOtp,
    PublicUiId::Slider,
    PublicUiId::RangeSlider,
    PublicUiId::DateTimePicker,
    PublicUiId::KeybindingRecorder,
];

/// Components presented on the TEXT_AREAS page.
pub const TEXT_AREAS_UI: &[PublicUiId] = &[PublicUiId::TextArea];

/// Components presented on the FORMS page.
pub const FORMS_UI: &[PublicUiId] = &[
    PublicUiId::Form,
    PublicUiId::Checkbox,
    PublicUiId::RadioGroup,
    PublicUiId::Switch,
    PublicUiId::FieldRow,
    PublicUiId::FieldCaption,
    PublicUiId::Label,
    PublicUiId::Description,
    PublicUiId::FormWizard,
    PublicUiId::Stepper,
];

/// Components presented on the LISTS page.
pub const LISTS_UI: &[PublicUiId] = &[
    PublicUiId::List,
    PublicUiId::ComposedRowParts,
    PublicUiId::SearchResults,
];

/// Components presented on the TREES page.
pub const TREES_UI: &[PublicUiId] = &[
    PublicUiId::Tree,
    PublicUiId::FileTree,
    PublicUiId::TreeNavigation,
    PublicUiId::TreeTable,
];

/// Components presented on the TABLES page.
pub const TABLES_UI: &[PublicUiId] = &[
    PublicUiId::Table,
    PublicUiId::DetailTable,
    PublicUiId::Pagination,
];

/// Components presented on the EDITABLE page.
pub const EDITABLE_UI: &[PublicUiId] = &[PublicUiId::DataTable];

/// Components presented on the PANELS page.
pub const PANELS_UI: &[PublicUiId] = &[
    PublicUiId::Panel,
    PublicUiId::Card,
    PublicUiId::Collapsible,
    PublicUiId::Surface,
    PublicUiId::Section,
];

/// Components presented on the SIDEBARS page.
pub const SIDEBARS_UI: &[PublicUiId] = &[
    PublicUiId::Sidebar,
    PublicUiId::NavigationList,
    PublicUiId::AccentRail,
];

/// Components presented on the DIALOGS page.
pub const DIALOGS_UI: &[PublicUiId] = &[
    PublicUiId::Dialog,
    PublicUiId::AlertDialog,
    PublicUiId::ConfirmPrompt,
    PublicUiId::MessageDialog,
    PublicUiId::ChoiceDialog,
    PublicUiId::PermissionPrompt,
];

/// Components presented on the PROGRESS page.
pub const PROGRESS_UI: &[PublicUiId] = &[
    PublicUiId::ProgressBar,
    PublicUiId::Spinner,
    PublicUiId::ActivityIndicator,
    PublicUiId::ProgressSteps,
    PublicUiId::Gauge,
    PublicUiId::ContextMeter,
    PublicUiId::TokenMeter,
    PublicUiId::SegmentedMeter,
];

/// Components presented on the SCROLLING page.
pub const SCROLLING_UI: &[PublicUiId] = &[
    PublicUiId::ScrollArea,
    PublicUiId::Viewport,
    PublicUiId::VirtualList,
    PublicUiId::DialogScroll,
];

/// Components presented on the EDITOR page.
pub const EDITOR_UI: &[PublicUiId] = &[
    PublicUiId::CodeBlock,
    PublicUiId::CodeFrame,
    PublicUiId::CompletionMenu,
    PublicUiId::HighlightedText,
    PublicUiId::MarkdownView,
    PublicUiId::StreamingMarkdown,
    PublicUiId::AnsiText,
    PublicUiId::HexViewer,
];

/// Components presented on the GRID page.
pub const GRID_UI: &[PublicUiId] = &[PublicUiId::VirtualGrid];

/// Components presented on the CHIPS page.
pub const CHIPS_UI: &[PublicUiId] = &[
    PublicUiId::Chip,
    PublicUiId::Tag,
    PublicUiId::Select,
    PublicUiId::TokenStrip,
    PublicUiId::TokenField,
    PublicUiId::StatusStrip,
    PublicUiId::EmptyState,
    PublicUiId::KeyValueList,
    PublicUiId::KeyValueTable,
    PublicUiId::PasteChip,
    PublicUiId::AttachmentChip,
];

/// Components presented on the PICKERS page.
pub const PICKERS_UI: &[PublicUiId] = &[
    PublicUiId::Picker,
    PublicUiId::FilePicker,
    PublicUiId::HistoryPicker,
    PublicUiId::Combobox,
    PublicUiId::QuickOpen,
    PublicUiId::CommandPalette,
    PublicUiId::MultiSelect,
    PublicUiId::ThemePicker,
    PublicUiId::JumpOverlay,
];

/// Components presented on the SETTINGS page.
pub const SETTINGS_UI: &[PublicUiId] = &[
    PublicUiId::Tabs,
    PublicUiId::Toggle,
    PublicUiId::SegmentedControl,
];

/// Components presented on the TASK_RUNNER page.
pub const TASK_RUNNER_UI: &[PublicUiId] = &[
    PublicUiId::LogPane,
    PublicUiId::LogStream,
    PublicUiId::EventStream,
    PublicUiId::TerminalOutput,
    PublicUiId::TerminalRunCard,
    PublicUiId::TerminalCellGrid,
];

/// Components presented on the TABLEPRO page.
pub const TABLEPRO_UI: &[PublicUiId] = &[
    PublicUiId::QueryEditor,
    PublicUiId::ResultGrid,
    PublicUiId::SchemaBrowser,
    PublicUiId::SessionPicker,
    PublicUiId::ConnectionManager,
    PublicUiId::ProcessTable,
];

/// Components presented on the FEEDBACK page.
pub const FEEDBACK_UI: &[PublicUiId] = &[
    PublicUiId::Alert,
    PublicUiId::Banner,
    PublicUiId::Callout,
    PublicUiId::Toast,
    PublicUiId::ToastStack,
    PublicUiId::Skeleton,
    PublicUiId::ErrorState,
    PublicUiId::OfflineBanner,
    PublicUiId::OfflineChrome,
    PublicUiId::OfflineSurface,
    PublicUiId::StatusBar,
    PublicUiId::StatusIndicator,
    PublicUiId::NotificationCenter,
    PublicUiId::LoadingView,
    PublicUiId::LoadingOverlay,
    PublicUiId::BusyBoundary,
    PublicUiId::HintBar,
    PublicUiId::ShortcutHint,
    PublicUiId::Tooltip,
    PublicUiId::SemanticZoomBadge,
    PublicUiId::ActivityShelf,
    PublicUiId::BackgroundTaskPanel,
    PublicUiId::WorkingStateCard,
    PublicUiId::IntegrationStatus,
    PublicUiId::Badge,
];

/// Components presented on the OVERLAYS page.
pub const OVERLAYS_UI: &[PublicUiId] = &[
    PublicUiId::Drawer,
    PublicUiId::Popover,
    PublicUiId::Backdrop,
    PublicUiId::OverlayStack,
    PublicUiId::DismissableLayer,
    PublicUiId::DropdownMenu,
    PublicUiId::MenuBar,
    PublicUiId::SlashCommandMenu,
    PublicUiId::FullscreenViewer,
    PublicUiId::PreviewCard,
];

/// Components presented on the CHARTS page.
pub const CHARTS_UI: &[PublicUiId] = &[
    PublicUiId::Chart,
    PublicUiId::Sparkline,
    PublicUiId::Histogram,
    PublicUiId::BarSeries,
    PublicUiId::MetricRadar,
    PublicUiId::MetricTileView,
    PublicUiId::Timeline,
    PublicUiId::CheckpointTimeline,
    PublicUiId::TraceWaterfall,
    PublicUiId::DependencyGraph,
];

/// Components presented on the STRUCTURE page.
pub const STRUCTURE_UI: &[PublicUiId] = &[
    PublicUiId::Accordion,
    PublicUiId::SplitPane,
    PublicUiId::Stack,
    PublicUiId::Center,
    PublicUiId::Grid,
    PublicUiId::WorkSurface,
    PublicUiId::Workspace,
    PublicUiId::ResizablePanelGroup,
    PublicUiId::Separator,
    PublicUiId::ChromeRow,
    PublicUiId::Breadcrumbs,
    PublicUiId::Carousel,
    PublicUiId::Inline,
    PublicUiId::InlineMention,
    PublicUiId::Heading,
    PublicUiId::Paragraph,
    PublicUiId::Text,
    PublicUiId::FocusGraph,
    PublicUiId::FocusLens,
    PublicUiId::InteractionScene,
    PublicUiId::SemanticScene,
    PublicUiId::RovingFocusGroup,
    PublicUiId::UiContext,
    PublicUiId::ImageSurface,
    PublicUiId::Identity,
    PublicUiId::AvatarGlyph,
    PublicUiId::Kbd,
    PublicUiId::KeyboardHelp,
    PublicUiId::Link,
    PublicUiId::CitationList,
    PublicUiId::SourceCitation,
    PublicUiId::DesignInspector,
    PublicUiId::ObjectInspector,
    PublicUiId::DiagnosticView,
    PublicUiId::DiffView,
    PublicUiId::DiffReview,
    PublicUiId::MessageThread,
    PublicUiId::Transcript,
    PublicUiId::ThinkingBlock,
    PublicUiId::ToolCallCard,
    PublicUiId::ToolCard,
    PublicUiId::SubagentCard,
    PublicUiId::ModeRibbon,
    PublicUiId::ModelSelector,
    PublicUiId::AgentModeSelector,
    PublicUiId::ComposerSelectors,
    PublicUiId::AgentStatusHeader,
    PublicUiId::TaskRail,
    PublicUiId::PromptComposer,
    PublicUiId::PromptQueue,
    PublicUiId::QuestionFlow,
    PublicUiId::ApprovalQueue,
    PublicUiId::MetricsDashboard,
    PublicUiId::PlanReview,
];

/// Every ownership bucket. Each public UI id appears in exactly one slice.
pub const BUCKETS: &[(PageId, &[PublicUiId])] = &[
    (PageId::BUTTONS, BUTTONS_UI),
    (PageId::INPUTS, INPUTS_UI),
    (PageId::TEXT_AREAS, TEXT_AREAS_UI),
    (PageId::FORMS, FORMS_UI),
    (PageId::LISTS, LISTS_UI),
    (PageId::TREES, TREES_UI),
    (PageId::TABLES, TABLES_UI),
    (PageId::EDITABLE, EDITABLE_UI),
    (PageId::PANELS, PANELS_UI),
    (PageId::SIDEBARS, SIDEBARS_UI),
    (PageId::DIALOGS, DIALOGS_UI),
    (PageId::PROGRESS, PROGRESS_UI),
    (PageId::SCROLLING, SCROLLING_UI),
    (PageId::EDITOR, EDITOR_UI),
    (PageId::GRID, GRID_UI),
    (PageId::CHIPS, CHIPS_UI),
    (PageId::PICKERS, PICKERS_UI),
    (PageId::SETTINGS, SETTINGS_UI),
    (PageId::TASK_RUNNER, TASK_RUNNER_UI),
    (PageId::TABLEPRO, TABLEPRO_UI),
    (PageId::FEEDBACK, FEEDBACK_UI),
    (PageId::OVERLAYS, OVERLAYS_UI),
    (PageId::CHARTS, CHARTS_UI),
    (PageId::STRUCTURE, STRUCTURE_UI),
];

/// Catalog page that owns this public visual component.
#[must_use]
pub fn catalog_page_for(id: PublicUiId) -> PageId {
    for &(page, ids) in BUCKETS {
        if ids.contains(&id) {
            return page;
        }
    }
    unreachable!("unmapped public UI {id}");
}

/// Public UI identities presented on a TermRock-only extras page.
#[must_use]
pub fn extras_on(page: PageId) -> &'static [PublicUiId] {
    match page {
        PageId::FEEDBACK => FEEDBACK_UI,
        PageId::OVERLAYS => OVERLAYS_UI,
        PageId::CHARTS => CHARTS_UI,
        PageId::STRUCTURE => STRUCTURE_UI,
        _ => &[],
    }
}
