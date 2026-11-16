//! What the Showcase proves about the library's declared API (#14).
//!
//! `PROOFS` lists the APIs a callable scenario in `app_tests_catalog.rs`
//! exercises. `EXCEPTIONS` lists APIs with no page demonstration (`Nonvisual`,
//! or `Deferred` with a tracking issue) and the exact test that pins them.
//! `coverage_debt/<lane>.txt` lists what still owes one of the two; it may only
//! shrink. `src/catalog_contract.rs` checks all three against the exported API.
//! One line per record keeps a restack mechanical.
//!
//! The #14 burn-down is split into lanes so parallel work never edits the same
//! file. A lane owns one `coverage/lane_<lane>.rs` (its `PROOFS` and
//! `EXCEPTIONS`), one `app_tests_catalog/<lane>.rs` (its `CASES` and scenario
//! functions), one `coverage_debt/<lane>.txt`, and its own marked section of
//! `ScenarioId` below. `proofs()` and `exceptions()` chain the lane arrays with
//! the older records in this file, so nothing else here changes per lane.

use crate::app::PageId;

#[path = "coverage/lane_collections.rs"]
mod lane_collections;
#[path = "coverage/lane_dialogs.rs"]
mod lane_dialogs;
#[path = "coverage/lane_fields.rs"]
mod lane_fields;
#[path = "coverage/lane_gridtable.rs"]
mod lane_gridtable;
#[path = "coverage/lane_layout.rs"]
mod lane_layout;
#[path = "coverage/lane_menus.rs"]
mod lane_menus;
#[path = "coverage/lane_pickers.rs"]
mod lane_pickers;
#[path = "coverage/lane_routing.rs"]
mod lane_routing;
#[path = "coverage/lane_scroll.rs"]
mod lane_scroll;
#[path = "coverage/lane_shell.rs"]
mod lane_shell;
#[path = "coverage/lane_state.rs"]
mod lane_state;
#[path = "coverage/lane_theme.rs"]
mod lane_theme;

/// Every proof: the records below, then each lane's.
pub fn proofs() -> impl Iterator<Item = &'static CapabilityProof> {
    PROOFS
        .iter()
        .chain(lane_shell::PROOFS)
        .chain(lane_theme::PROOFS)
        .chain(lane_routing::PROOFS)
        .chain(lane_state::PROOFS)
        .chain(lane_scroll::PROOFS)
        .chain(lane_gridtable::PROOFS)
        .chain(lane_fields::PROOFS)
        .chain(lane_dialogs::PROOFS)
        .chain(lane_menus::PROOFS)
        .chain(lane_pickers::PROOFS)
        .chain(lane_collections::PROOFS)
        .chain(lane_layout::PROOFS)
}

/// Every exception: the records below, then each lane's.
pub fn exceptions() -> impl Iterator<Item = &'static CapabilityException> {
    EXCEPTIONS
        .iter()
        .chain(lane_shell::EXCEPTIONS)
        .chain(lane_theme::EXCEPTIONS)
        .chain(lane_routing::EXCEPTIONS)
        .chain(lane_state::EXCEPTIONS)
        .chain(lane_scroll::EXCEPTIONS)
        .chain(lane_gridtable::EXCEPTIONS)
        .chain(lane_fields::EXCEPTIONS)
        .chain(lane_dialogs::EXCEPTIONS)
        .chain(lane_menus::EXCEPTIONS)
        .chain(lane_pickers::EXCEPTIONS)
        .chain(lane_collections::EXCEPTIONS)
        .chain(lane_layout::EXCEPTIONS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScenarioId {
    PickerStableSelection,
    RequiredCustomValidation,
    DisabledInputRejection,
    OverlayDismissPolicy,
    CollectionReplacement,
    NarrowLayout,
    TerminalViewportLifecycle,
    FixedControlsPolicy,
    FixedDialogsPolicy,
    ListDetailNavigation,
    StatusNotice,
    PickerLiveControl,
    TextInputSetValue,
    ChoiceEditorOptions,
    SplitterCancelDrag,
    CodeEditorProviders,
    CodeEditorDiagnosticsFind,
    GridEditSession,
    GridChangeCapture,
    GridSubmissionLock,
    GridSourceIdentity,
    GridUndoKinds,
    ExternalFeedBounds,
    ScrollWindowWork,
    ModalFocusPolicy,
    DocumentWorkspaceLifecycle,
    CompletionPopupSnapshot,
    SessionAttachReplay,
    SessionInputAndPaste,
    SessionEndingsAndPolicy,
    WorkCounterContract,
    SearchDetailQuery,
    SearchDetailSelection,
    SearchDetailPaint,
    KeyedListSource,
    NavItemReplacement,
    TableModelUpdates,
    KeyedTableStore,
    KeyedTableSource,
    KeyedTableEdit,
    KeyedTreeStore,
    KeyedTreeView,
    ListDetailKeyedSource,
    ListDetailHostedPanes,
    ChipBarEditing,
    ToolbarEditing,
    TabsEditing,
    ScrollPanelContent,
    KeyedStoreIdentity,
    KeyedSourceRejection,
    KeyedPointerPress,
    ResponseBatching,
    DataShapeErrors,
    TextBufferGeometry,
    // lane:shell begin: this lane adds its ScenarioId variants between these two lines only
    ShellStatusLine,
    ShellCxRequests,
    ShellRouterStack,
    ShellChromeMenus,
    ShellFooterHints,
    ShellKeyRouting,
    ShellHostLoop,
    ShellPageHelpers,
    ShellClipboardCopy,
    ShellMotionPolicy,
    RuntimeInputPump,
    RuntimeExternalChannel,
    // lane:shell end
    // lane:theme begin: this lane adds its ScenarioId variants between these two lines only
    ThemeColorLevels,
    ThemeCharsetGlyphs,
    ThemeInteractiveStates,
    ThemeInactiveSelection,
    ThemeTonesAndStatus,
    ThemeLegibility,
    ThemeStyleTokens,
    SpecimenStateGrid,
    TextUtilityFunctions,
    RowPainting,
    ContentStates,
    MeasureTiles,
    FadeScrollEdges,
    StatusSegments,
    BrandLockup,
    KeyReferenceLayout,
    // lane:theme end
    // lane:routing begin: this lane adds its ScenarioId variants between these two lines only
    InputDecoding,
    KeyboardNegotiation,
    RefusalReasons,
    HitRegistryRouting,
    KeymapResolution,
    NavKeyProfiles,
    HintBindings,
    IntentVocabulary,
    PointerClickRouting,
    PointerScrollGestures,
    PointerStatePolicies,
    // lane:routing end
    // lane:state begin: this lane adds its ScenarioId variants between these two lines only
    StateIdentity,
    StateFocusRing,
    StateFocusBlur,
    StateResponseFlow,
    StatePointerShapes,
    StateListCursor,
    StateTypeAhead,
    StateNavProfile,
    StateRowSelection,
    StateClickPolicy,
    StateTextBuffer,
    StateTextCommands,
    StateUndoHistory,
    StateEditEntry,
    StatePartRegions,
    // lane:state end
    // lane:scroll begin: this lane adds its ScenarioId variants between these two lines only
    ScrollStateGeometry,
    ScrollPaneAxes,
    ScrollbarRendering,
    ScrollViewHorizontal,
    ScrollViewLayout,
    ScrollViewFollow,
    ScrollViewCommands,
    RevealInline,
    RevealTooltip,
    RevealHelpers,
    PanelChrome,
    ScrollPanelInput,
    TextViewportContent,
    TextViewportInput,
    TextViewportLayout,
    TerminalGridState,
    TerminalGridView,
    TerminalViewportPointer,
    TerminalKeyEncoding,
    // lane:scroll end
    // lane:gridtable begin: this lane adds its ScenarioId variants between these two lines only
    DiffModelAndView,
    InspectChangesModes,
    GridValuesAndDraft,
    GridKeysAndCommands,
    GridPointerAndLabels,
    GridEditSessions,
    TableModelAndSelection,
    TableKeysAndEdits,
    TablePointerAndChrome,
    // lane:gridtable end
    // lane:fields begin: this lane adds its ScenarioId variants between these two lines only
    FieldsButtonControls,
    FieldsButtonRowLayout,
    FieldsActionRow,
    FieldsChoiceControls,
    FieldsIconButton,
    FieldsEditFieldCore,
    FieldsValidators,
    FieldsFieldCommon,
    FieldsPointerRouting,
    FieldsTextInputBuilders,
    FieldsTextInputEditing,
    FieldsTextInputPointer,
    FieldsTextAreaBuilders,
    FieldsTextAreaEditing,
    FieldsCodeEditorKeys,
    FieldsCodeEditorFind,
    FieldsCodeEditorConfig,
    // lane:fields end
    // lane:dialogs begin: this lane adds its ScenarioId variants between these two lines only
    DialogsLeaveGuardFlow,
    DialogsModalStackLifecycle,
    DialogsModalFrameChrome,
    DialogsRenderCtxRegistration,
    DialogsInteractionVisualState,
    DialogsPromptEditing,
    DialogsFactsAck,
    DialogsActionModel,
    DialogsPointerInput,
    DialogsFormModel,
    DialogsFormInput,
    DialogsInfoFlow,
    DialogsChoiceFlow,
    DialogsLayerResults,
    DialogsLayerSpecs,
    // lane:dialogs end
    // lane:menus begin: this lane adds its ScenarioId variants between these two lines only
    MenusPopupDismissPolicy,
    MenusPopupPlacement,
    MenusContextMenuKeys,
    MenusContextMenuPointer,
    MenusContextMenuLayout,
    MenusSelectKeys,
    MenusSelectPointer,
    MenusSelectPopup,
    MenusBarKeys,
    MenusBarPointer,
    MenusBarOverflow,
    MenusRowMenu,
    MenusContextRequests,
    MenusContextCommands,
    MenusEditMenuCaps,
    MenusEditMenuHost,
    MenusTooltips,
    MenusKeyHintLabels,
    MenusKeyHintRendering,
    MenusHintBar,
    MenusStatusBar,
    MenusHelpOverlay,
    // lane:menus end
    // lane:pickers begin: this lane adds its ScenarioId variants between these two lines only
    MeterToneLevels,
    MeterStaleAndVisuals,
    ProgressBarStatuses,
    MeterListColumns,
    CompletionLifecycle,
    OptionListItems,
    OptionListNavigation,
    PickerSingleActions,
    PickerMultiSelection,
    PickerStatesAndChrome,
    ColumnEditorModel,
    ChoiceEditorKeys,
    ChoiceEditorPointerAndRender,
    EditorAffordance,
    PropsSheetLayout,
    PropsListInteraction,
    OptionListSelectArrows,
    OptionListQueryPointer,
    // lane:pickers end
    // lane:collections begin: this lane adds its ScenarioId variants between these two lines only
    CollectionsTabsLook,
    CollectionsTabsInput,
    CollectionsTabsOverflow,
    CollectionsNavList,
    CollectionsListSingle,
    CollectionsListMulti,
    CollectionsListOptions,
    CollectionsTreeStructure,
    CollectionsTreeInput,
    CollectionsTreeOptions,
    // lane:collections end
    // lane:layout begin: this lane adds its ScenarioId variants between these two lines only
    LayoutSplitGeometry,
    LayoutSplitKeys,
    LayoutStackFlow,
    LayoutStackScroll,
    LayoutSplitterSeam,
    LayoutPaneNodes,
    LayoutPaneTreeFocus,
    LayoutPaneTreeSeams,
    LayoutColumnSolve,
    LayoutColumnSizing,
    LayoutColumnHeaderMenu,
    LayoutChipBarActions,
    LayoutChipBarOverflow,
    LayoutToolbarActions,
    LayoutToolbarOverflow,
    LayoutStepRail,
    LayoutStepRailScroll,
    LayoutListCore,
    LayoutNoticeKinds,
    LayoutNoticeWrapping,
    LayoutEmptyState,
    LayoutPageHeader,
    LayoutDetailCard,
    LayoutSidebarRail,
    LayoutTooSmall,
    LayoutBreadcrumb,
    // lane:layout end
}

#[allow(dead_code)]
pub struct CapabilityProof {
    pub api: &'static str,
    pub route: PageId,
    pub scenarios: &'static [ScenarioId],
    pub feature: Option<&'static str>,
}

/// Why an API has an exception and no proof. The kind is the only signal the
/// gate reads: reason text is prose and never decides whether a demo is owed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionKind {
    /// No page can show it: a helper, a host-facing hook or test plumbing.
    /// `tracking_issue` is optional.
    Nonvisual,
    /// A demo is owed and not yet built. `tracking_issue` is required.
    /// None is left once every area of #14 has its demo, and the gate still
    /// accepts the kind for the next API that arrives without one.
    #[allow(dead_code)]
    Deferred,
}

/// `regression_test` is `path/to/file.rs::test_name` of a live `#[test]` whose
/// body uses the API (its member, and for a member of a type, that type in the
/// body or the file). `reason` is a sentence, never a page name.
#[allow(dead_code)]
pub struct CapabilityException {
    pub api: &'static str,
    pub kind: ExceptionKind,
    pub reason: &'static str,
    pub regression_test: &'static str,
    pub tracking_issue: Option<u32>,
}

/// The session controller capabilities the Terminal pane page shows: its
/// "session controller" source runs a scripted transport through every one.
pub const TERMINAL_SESSION_COVERAGE: &[&str] = &[
    "terminal::SessionFault",
    "terminal::SessionSize",
    "terminal::SessionLease",
    "terminal::SessionLease::Owner",
    "terminal::SessionLease::ReadOnly",
    "terminal::SessionSent",
    "terminal::SessionSent::Queued",
    "terminal::SessionCommand",
    "terminal::SessionCommand::Input",
    "terminal::SessionCommand::Reply",
    "terminal::SessionCommand::Redraw",
    "terminal::SessionEnd",
    "terminal::SessionEnd::Detached",
    "terminal::SessionEvent",
    "terminal::SessionEvent::Attached",
    "terminal::SessionEvent::Lease",
    "terminal::SessionEvent::Output",
    "terminal::SessionEvent::Notice",
    "terminal::SessionEvent::Exited",
    "terminal::SessionEvent::Ended",
    "terminal::SessionLink",
    "terminal::SessionLink::send",
    "terminal::SessionLink::drain",
    "terminal::SessionSource",
    "terminal::SessionSource::open",
    "terminal::SessionSource::restart",
    "terminal::SessionDetach",
    "terminal::SessionDetach::Operator",
    "terminal::SessionStatus",
    "terminal::SessionStatus::Pending",
    "terminal::SessionStatus::Attaching",
    "terminal::SessionStatus::Attached",
    "terminal::SessionStatus::Detached",
    "terminal::SessionStatus::Exited",
    "terminal::SessionStatus::Failed",
    "terminal::SessionConfig",
    "terminal::SessionClipboardContext",
    "terminal::SessionClipboardRefusal",
    "terminal::SessionClipboardRefusal::Write",
    "terminal::SessionClipboardDecision",
    "terminal::SessionClipboardDecision::Refuse",
    "terminal::SessionHostPolicy",
    "terminal::SessionHostPolicy::title",
    "terminal::SessionHostPolicy::notice",
    "terminal::SessionHostPolicy::clipboard",
    "terminal::SessionEffect",
    "terminal::SessionEffect::Detached",
    "terminal::SessionEffect::Failed",
    "terminal::SessionEffect::Notice",
    "terminal::SessionEffect::ClipboardRefused",
    "terminal::SessionKey",
    "terminal::SessionKey::Consumed",
    "terminal::SessionKey::Forwarded",
    "terminal::SessionPaste",
    "terminal::SessionPaste::Sent",
    "terminal::SessionPaste::NeedsConfirm",
    "terminal::SessionPaste::Dropped",
    "terminal::TerminalSessionController",
    "terminal::SessionConfig::MINIMUM",
    "terminal::SessionConfig::new",
    "terminal::TerminalSessionController::new",
    "terminal::TerminalSessionController::status",
    "terminal::TerminalSessionController::viewport",
    "terminal::TerminalSessionController::title",
    "terminal::TerminalSessionController::attention",
    "terminal::TerminalSessionController::has_output",
    "terminal::TerminalSessionController::dropped_replies",
    "terminal::TerminalSessionController::live",
    "terminal::TerminalSessionController::too_small",
    "terminal::TerminalSessionController::pump",
    "terminal::TerminalSessionController::next_replay_deadline",
    "terminal::TerminalSessionController::take_effects",
    "terminal::TerminalSessionController::key",
    "terminal::TerminalSessionController::gesture",
    "terminal::TerminalSessionController::paste",
    "terminal::TerminalSessionController::paste_confirmed",
    "terminal::TerminalSessionController::sync_size",
    "terminal::TerminalSessionController::focus_changed",
    "terminal::TerminalSessionController::close_needs_confirm",
    "terminal::TerminalSessionController::set_config",
    "terminal::TerminalSessionController::wheel",
    "terminal::TerminalSessionController::redraw",
    "terminal::TerminalSessionController::clear",
    "terminal::TerminalSessionController::viewport_mut",
];

pub const PICKER_COVERAGE: &[(&str, &str)] = &[
    ("widgets::picker::Picker::report_item_error", "pickers"),
    ("widgets::picker::Picker::list", "pickers"),
    ("widgets::picker::PickerStatus", "pickers"),
    ("widgets::picker::PickerStatus::Ready", "pickers"),
    ("widgets::picker::PickerStatus::Loading", "pickers"),
    ("widgets::picker::PickerStatus::Error", "pickers"),
    ("widgets::picker::Picker", "pickers"),
    ("widgets::picker::PickerAction", "pickers"),
    ("widgets::picker::PickerAction::QueryChanged", "pickers"),
    ("widgets::picker::PickerAction::Chosen", "pickers"),
    ("widgets::picker::PickerAction::ChosenMany", "pickers"),
    ("widgets::picker::PickerAction::Toggled", "pickers"),
    (
        "widgets::picker::PickerAction::SelectionConflict",
        "pickers",
    ),
    ("widgets::picker::PickerAction::ChosenAlt", "pickers"),
    ("widgets::picker::PickerAction::Submit", "pickers"),
    ("widgets::picker::PickerAction::Secondary", "pickers"),
    ("widgets::picker::PickerAction::NextScope", "pickers"),
    ("widgets::picker::PickerAction::PrevScope", "pickers"),
    ("widgets::picker::PickerAction::Cancelled", "pickers"),
    ("widgets::picker::PickerAction::Back", "pickers"),
    ("widgets::picker::PickerKeyProfile", "pickers"),
    ("widgets::picker::PickerKeyProfile::Desktop", "pickers"),
    ("widgets::picker::PickerKeyProfile::Vim", "pickers"),
    ("widgets::picker::Picker::new", "pickers"),
    ("widgets::picker::Picker::multi_select", "pickers"),
    ("widgets::picker::Picker::key_profile", "pickers"),
    ("widgets::picker::Picker::nav", "pickers"),
    ("widgets::picker::Picker::delete_is_secondary", "pickers"),
    ("widgets::picker::Picker::set_searchable", "pickers"),
    ("widgets::picker::Picker::query", "pickers"),
    ("widgets::picker::Picker::width", "pickers"),
    ("widgets::picker::Picker::placement", "pickers"),
    ("widgets::picker::Picker::anchor", "pickers"),
    ("widgets::picker::Picker::fit", "pickers"),
    ("widgets::picker::Picker::backdrop", "pickers"),
    ("widgets::picker::Picker::hints", "pickers"),
    ("widgets::picker::Picker::max_rows", "pickers"),
    ("widgets::picker::Picker::placeholder", "pickers"),
    ("widgets::picker::Picker::empty", "pickers"),
    ("widgets::picker::Picker::empty_text", "pickers"),
    ("widgets::picker::Picker::dismiss", "pickers"),
    ("widgets::picker::Picker::cursor_style", "pickers"),
    ("widgets::picker::Picker::set_items", "pickers"),
    ("widgets::picker::Picker::replace_items", "pickers"),
    ("widgets::picker::Picker::visible_item", "pickers"),
    ("widgets::picker::Picker::set_visible_keys", "pickers"),
    ("widgets::picker::VisibleKey", "pickers"),
    ("widgets::picker::VisibleKey::new", "pickers"),
    ("widgets::picker::VisibleKey::matched", "pickers"),
    ("widgets::picker::Picker::set_cursor", "pickers"),
    ("widgets::picker::Picker::set_max_rows", "pickers"),
    ("widgets::picker::Picker::toggle_checked", "pickers"),
];

pub const TEXT_INPUT_COVERAGE: &[(&str, &str)] =
    &[("widgets::input::TextInput::set_value", "selectable")];
pub const GRID_EDIT_COVERAGE: &[(&str, &str)] = &[
    ("widgets::grid::DataGrid::try_edit_text", "grid"),
    ("widgets::grid::DataGrid::paste_into_cell", "grid"),
    ("widgets::grid::DataGrid::finish_edit_session", "grid"),
    ("widgets::grid::CellEditResult", "grid"),
    ("widgets::grid::CellEditResult::Changed", "grid"),
    ("widgets::grid::CellEditResult::Unchanged", "grid"),
    ("widgets::grid::GridEditError", "grid"),
    ("widgets::grid::GridEditError::Busy", "grid"),
    ("widgets::grid::GridEditError::InvalidTarget", "grid"),
    ("widgets::grid::GridEditError::NoChoice", "grid"),
    ("widgets::grid::GridEditError::Validation", "grid"),
    ("widgets::grid::FinishEdit", "grid"),
    ("widgets::grid::FinishEdit::NoSession", "grid"),
    ("widgets::grid::FinishEdit::Accepted", "grid"),
    ("widgets::grid::GridChangeSet", "grid"),
    ("widgets::grid::InsertedRow", "grid"),
    ("widgets::grid::UpdatedRow", "grid"),
    ("widgets::grid::DeletedRow", "grid"),
    ("widgets::grid::DataGrid::snapshot_changes", "grid"),
    ("widgets::grid::InsertedRow::key", "grid"),
    ("widgets::grid::InsertedRow::values", "grid"),
    ("widgets::grid::UpdatedRow::key", "grid"),
    ("widgets::grid::UpdatedRow::before", "grid"),
    ("widgets::grid::UpdatedRow::cells", "grid"),
    ("widgets::grid::DeletedRow::before", "grid"),
    ("widgets::grid::CellChange::column", "grid"),
    ("widgets::grid::CellChange::after", "grid"),
    ("widgets::grid::GridChangeSet::inserted", "grid"),
    ("widgets::grid::GridChangeSet::updated", "grid"),
    ("widgets::grid::GridChangeSet::deleted", "grid"),
    ("widgets::grid::GridChangeSet::counts", "grid"),
    ("widgets::grid::GridChangeSet::identity_receipt", "grid"),
    ("widgets::grid::GridCommitToken", "grid"),
    ("widgets::grid::GridCommitFailure", "grid"),
    ("widgets::grid::DataGrid::begin_submission", "grid"),
    ("widgets::grid::DataGrid::cancel_submission", "grid"),
    ("widgets::grid::DataGrid::complete_submission", "grid"),
    ("widgets::grid::DataGrid::submission_pending", "grid"),
];

pub const DOCUMENT_WORKSPACE_COVERAGE: &[(&str, &str)] = &[
    (
        "widgets::document_workspace::DocumentWorkspace",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceDocument",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceDocument::title",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceDocument::dirty",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceDocument::focus_target",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceDocument::view",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::OperationToken",
        "queryworkbench",
    ),
    ("widgets::document_workspace::CloseToken", "queryworkbench"),
    ("widgets::document_workspace::OpenKind", "queryworkbench"),
    (
        "widgets::document_workspace::OpenKind::Pinned",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::OpenKind::Preview",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::CloseDecision",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::CloseDecision::Cancel",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::CloseDecision::Discard",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction::Activated",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction::PreviewPromoted",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction::CloseRequested",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction::CancelOperation",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::WorkspaceAction::Closed",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::new",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::open_with",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::activate",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::promote",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::key_at",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::index_of",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::get",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::iter",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::len",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::active_key",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::ordered_keys",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::update",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::view_mut",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::begin_operation",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::end_operation",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::cancel_operation",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::running",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::request_close",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::resolve_close",
        "queryworkbench",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::sync_tabs_with",
        "queryworkbench",
    ),
];

pub const DOCUMENT_WORKSPACE_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::document_workspace::DocumentWorkspace::finish_operation",
        "Only an operation whose result changes content ends here, such as a TablePro grid commit; Showcase runs fill a result pane and end through end_operation. tests/regressions/document_workspace.rs asserts it.",
    ),
    (
        "widgets::document_workspace::WorkspaceError",
        "The page reports a refusal through its Display text and never matches a variant, so each variant is asserted by tests/regressions/document_workspace.rs instead.",
    ),
    (
        "widgets::document_workspace::WorkspaceError::MissingDocument",
        "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.",
    ),
    (
        "widgets::document_workspace::WorkspaceError::StaleClose",
        "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.",
    ),
    (
        "widgets::document_workspace::WorkspaceError::StaleOperation",
        "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.",
    ),
    (
        "widgets::document_workspace::WorkspaceError::SaveInProgress",
        "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.",
    ),
    (
        "widgets::document_workspace::WorkspaceError::StillDirty",
        "Only the save protocol returns it, and Showcase documents are never saved; tests/regressions/document_workspace.rs asserts this variant.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::id",
        "The page keeps its own widget id constant and never asks the controller for it.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::scoped_id",
        "The page derives its editor and result ids with its own key-scoped helpers; TablePro's tab ids and the conformance suite use the controller's.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::is_empty",
        "The page counts tabs with len, so the emptiness predicate has no call site.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::contains",
        "The page finds documents by title through iter, so the membership predicate has no call site; the conformance suite asserts it.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::preview_key",
        "The page opens and promotes previews through the controller's actions and never reads the slot; TablePro's workbench and the conformance suite do.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::open",
        "The page opens through open_with because each document builds its ids from its own key.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::reorder",
        "Tabs emits no move gesture, so no page can drive a reorder; the conformance suite and the example do.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::revision",
        "The page never captures a content revision; the conformance suite asserts that view work leaves it unchanged.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::pending_close",
        "The page keeps the token it was handed and never asks for the stored one; TablePro's save-then-close and the conformance suite do.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::save_locked",
        "Showcase documents are never saved, so no page holds a close-save lock; TablePro's key and pointer paths read it.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::sync_tabs",
        "The page decorates its tab items, so it calls sync_tabs_with; the conformance suite and the example call the plain projection.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::begin_close_save",
        "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::validate_close_save",
        "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::complete_close_save",
        "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::fail_close_save",
        "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.",
    ),
    (
        "widgets::document_workspace::DocumentWorkspace::cancel_close_save",
        "Showcase documents are never saved; TablePro's connection reset and the conformance suite drive the save protocol.",
    ),
];

pub const GRID_CHANGE_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::grid::DataGrid::instance_id",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridInstanceId",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridChangeSet::origin",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridChangeSet::revision",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridChangeSet::is_empty",
        "The Showcase page keeps its own pending-count guard before it opens the dialog, and the dialog already shows the zero counts.",
    ),
    (
        "widgets::grid::GridChangeSet::same_capture",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::CommittedRow",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridCommitReceipt",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridCommitReceipt::new",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridCommitReceipt::origin",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridCommitReceipt::revision",
        "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::DeletedRow::key",
        "The DELETE preview matches on the base row from DeletedRow::before, so the row key is not shown.",
    ),
    (
        "widgets::grid::CellChange",
        "The page reads a changed cell through its accessors and never names the type.",
    ),
    (
        "widgets::grid::GridCommitError",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::Busy",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::Editing",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::Empty",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::WrongOrigin",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::StaleRevision",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::StaleToken",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::InvalidReceipt",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridCommitError::Exhausted",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::Missing",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::Unexpected",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::Duplicate",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::Width",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::UpdatedKeyChanged",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::ReceiptFault::KeyCollision",
        "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.",
    ),
    (
        "widgets::grid::GridDataError",
        "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridDataError::Busy",
        "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridDataError::Key",
        "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridDataError::Shape",
        "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridDataError::ColumnWidth",
        "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.",
    ),
    (
        "widgets::grid::GridCommitToken::origin",
        "A host that holds a token may compare its grid; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.",
    ),
    (
        "widgets::grid::GridCommitToken::revision",
        "A host that holds a token may compare its capture revision; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.",
    ),
    (
        "widgets::grid::DataGrid::validate_submission_receipt",
        "Used by a host that lets an external store mutate before it completes; the Grid page has no store, and complete_submission runs the same check, which src/widgets/grid_submission_tests.rs asserts.",
    ),
    (
        "widgets::grid::DataGrid::commit_failure",
        "The Grid page marks the failed row; the grid-level message for a failure that names no row is asserted in src/widgets/grid_submission_tests.rs.",
    ),
    (
        "widgets::grid::CellChange::before",
        "The UPDATE preview matches on the row's base values from UpdatedRow::before, so the per-cell before value is not shown.",
    ),
];

pub const GRID_PARSER_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::edit::parsers::json_container",
        "The grid page demonstrates this grammar through default_validator on its notes (Json) column; no page calls the helper directly.",
    ),
    (
        "widgets::edit::parsers::canonical_uuid",
        "The grid page demonstrates this grammar through default_validator on its Id columns; no page calls the helper directly.",
    ),
    (
        "widgets::edit::parsers::date_or_timestamp",
        "The grid page demonstrates this grammar through default_validator on its renewed_at (Timestamp) column; no page calls the helper directly.",
    ),
];

pub const SCROLL_WINDOW_COVERAGE: &[(&str, &str)] = &[
    ("ui::scroll_view::ScrollView::render_window", "scrolling"),
    ("ui::scroll_view::ScrollView::last_work", "scrolling"),
    ("ui::scroll_view::ScrollWork", "scrolling"),
    ("ui::scroll_view::VisibleWindow", "scrolling"),
    (
        "ui::scroll_view::VisibleWindow::project_content_rect",
        "scrolling",
    ),
    ("ui::scroll_view::VisibleWindow::content_rows", "scrolling"),
];

pub const SCROLL_WINDOW_ALLOWLIST: &[(&str, &str)] = &[
    (
        "ui::scroll_view::VisibleWindow::project_rect",
        "The Virtual rows demo projects model rows with project_content_rect, which wraps it; a padded-canvas rectangle is only needed by hosts that position a popup anchor.",
    ),
    (
        "ui::scroll_view::VisibleWindow::project_point",
        "The Virtual rows demo has no point-anchored overlay; cursor and caret hosts project a point from the same window.",
    ),
    (
        "ui::scroll_view::VisibleWindow::rows",
        "The demo iterates content_rows, which is rows without the padding; rows is the padded-canvas range for painting padding decorations.",
    ),
];

pub const EXTERNAL_COVERAGE: &[(&str, &str)] = &[
    ("runtime::bounded_external_channel", "activity"),
    ("runtime::ExternalPolicy", "activity"),
    ("runtime::ExternalSender::try_send", "activity"),
    ("runtime::ExternalSendError", "activity"),
    ("runtime::ExternalSendError::Full", "activity"),
    ("runtime::ExternalSendError::Closed", "activity"),
];

pub const EXTERNAL_ALLOWLIST: &[(&str, &str)] = &[
    (
        "runtime::ExternalReceiver::try_recv",
        "The runtime calls it between handlers; no page reads the queue itself. The runtime unit tests external_turn_ and bounded_external_ pin its order and bound.",
    ),
    (
        "runtime::ExternalReceiver::policy",
        "A read accessor the runtime uses to copy the bound before each turn; the Activity page shows the bound through the pages it limits, not as a value.",
    ),
    (
        "runtime::ExternalReceiver::set_policy",
        "The Activity page passes its policy to bounded_external_channel; the setter is covered by the bounded_external_policy_ unit test.",
    ),
    (
        "runtime::ExternalReceiver::with_policy",
        "The Activity page passes its policy to bounded_external_channel; the builder is covered by the bounded_external_policy_ unit test.",
    ),
    (
        "runtime::ExternalReceiver::drain",
        "Deprecated and unbounded; the runtime and every app read with try_recv, so no page demonstrates it.",
    ),
];

pub const RUNTIME_ALLOWLIST: &[(&str, &str)] = &[
    (
        "runtime::RawInputPolicy",
        "Raw terminal event budgets act below the widget layer and draw nothing; pinned by the runtime::tests raw_input_ and startup_discard_ unit tests, and the showcase runs on the default policy.",
    ),
    (
        "runtime::Application::raw_input_policy",
        "A host-level scheduling override with no visible state; the showcase keeps the default and the runtime unit tests drive an application with an explicit policy.",
    ),
    (
        "runtime::Application::take_render_invalidated",
        "Event-loop plumbing with no visible state: the shell sets it when a frame is still unsettled after its last pass and the runtime draws again; the runtime unit test render_pending_survives_all_draw_sites pins it.",
    ),
    (
        "runtime::drain_pending_input_bounded",
        "Called once by shell::run before the terminal session starts, so no page can show it; the startup_discard_ unit tests pin its bound and raw-mode restoration.",
    ),
];

pub const INSPECT_CHANGES_COVERAGE: &[(&str, &str)] = &[
    ("widgets::inspect_changes::ChangeSet", "diff"),
    ("widgets::inspect_changes::ChangeWords", "diff"),
    ("widgets::inspect_changes::InspectOutcome", "diff"),
    ("widgets::inspect_changes::InspectOutcome::Back", "diff"),
    ("widgets::inspect_changes::InspectMode", "diff"),
    ("widgets::inspect_changes::InspectMode::Compact", "diff"),
    ("widgets::inspect_changes::InspectMode::Advanced", "diff"),
    ("widgets::inspect_changes::InspectChanges", "diff"),
    ("widgets::inspect_changes::InspectChanges::new", "diff"),
    (
        "widgets::inspect_changes::InspectChanges::returns_to_exit",
        "diff",
    ),
    ("widgets::inspect_changes::InspectChanges::words", "diff"),
];

pub const INSPECT_CHANGES_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::inspect_changes::InspectOutcome::Close",
        "The Diff page configures returns_to_exit(true), so its inspector always resolves to Back; Close is what an inspector with the default returns_to_exit(false) reports, pinned by a_dismissal_resolves_like_leaving_with_escape.",
    ),
    (
        "widgets::inspect_changes::ChangeSet::additions",
        "The inspector draws the added-line total in its summary line; the page does not read it separately.",
    ),
    (
        "widgets::inspect_changes::ChangeSet::deletions",
        "The inspector draws the removed-line total in its summary line; the page does not read it separately.",
    ),
    (
        "widgets::inspect_changes::ChangeSet::is_empty",
        "The inspector draws its empty state itself; the page always supplies a change set that has files.",
    ),
    (
        "widgets::inspect_changes::ChangeSet::summary",
        "The inspector draws this summary in its header; the page shows it through the widget rather than calling it.",
    ),
    (
        "widgets::inspect_changes::InspectMode::label",
        "The inspector draws the mode label in its header; the page shows it through the widget rather than calling it.",
    ),
    (
        "widgets::inspect_changes::InspectChanges::wheel_at",
        "Wheel events reach the inspector through its layer pointer route, which the page forwards pointer events to.",
    ),
    (
        "widgets::inspect_changes::InspectChanges::initial_focus",
        "The Diff page renders the inspector inline and never pushes its layer, so the layer spec's initial focus does not apply there; the user reaches the inspector with Tab twice, as the PTY tests do.",
    ),
];

/// The pages that show the shared Finder composition, each with the source
/// file `search_detail_coverage_ids_are_used_by_their_pages` reads. A page
/// that adopts the composition adds a row here and its name to the ids it uses.
pub const SEARCH_DETAIL_PAGES: &[(&str, &str)] = &[
    ("finder", "src/bin/showcase/pages/finder.rs"),
    ("finder_hosted", "src/bin/showcase/pages/finder/hosted.rs"),
    ("holla", "src/bin/holla/screens/finder.rs"),
];

/// Which pages use each shared Finder id. An id is listed once, with every page
/// that uses it, and is not in [`SEARCH_SESSION_ALLOWLIST`].
pub const SEARCH_DETAIL_COVERAGE: &[(&str, &[&str])] = &[
    (
        "widgets::search_detail::ResultRowKind",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::ResultRowKind::Heading",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::ResultRowKind::Item",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::ResultRowKind::Cells",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::ResultRowKind::Spacer",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::SearchSource", &["finder", "holla"]),
    (
        "widgets::search_detail::SearchSource::row_kind",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchSource::cell_count",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchSource::cell_key",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchSource::cell_label",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchSource::cell_enabled",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchSource::cell_click_policy",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::QueryToken::result_stamp",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::QueryRequest", &["finder", "holla"]),
    ("widgets::search_detail::SearchAction", &["finder", "holla"]),
    (
        "widgets::search_detail::SearchAction::QueryRequested",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchAction::Activate",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchAction::ContextRequested",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchAction::DetailAction",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::MissingSelection",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::MissingSelection::NearestPosition",
        &["finder"],
    ),
    (
        "widgets::search_detail::MissingSelection::FirstEnabled",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::QueryPasteMode",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::QueryPasteMode::StripLineBreaks",
        &["finder"],
    ),
    (
        "widgets::search_detail::QueryPasteMode::JoinLinesWithSpaceTrim",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::SearchConfig", &["finder", "holla"]),
    (
        "widgets::search_detail::SearchConfig::click",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::select_first_on_query",
        &["finder"],
    ),
    (
        "widgets::search_detail::SearchConfig::missing_selection",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::paste_mode",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::navigate_while_loading",
        &["finder"],
    ),
    ("widgets::search_detail::SearchPhase", &["finder"]),
    ("widgets::search_detail::SearchPhase::Loading", &["finder"]),
    ("widgets::search_detail::SearchPhase::Ready", &["finder"]),
    ("widgets::search_detail::SearchPhase::Failed", &["finder"]),
    ("widgets::search_detail::SearchSession::error", &["finder"]),
    ("widgets::search_detail::SearchDetail", &["finder", "holla"]),
    (
        "widgets::search_detail::SearchDetail::new",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::config",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::collapse_below",
        &["finder"],
    ),
    (
        "widgets::search_detail::SearchDetail::set_query_readout",
        &["finder"],
    ),
    ("widgets::search_detail::SearchDetail::editing", &["finder"]),
    ("widgets::search_detail::SearchDetail::on_blur", &["finder"]),
    ("widgets::search_detail::SearchDetail::id", &["finder"]),
    (
        "widgets::search_detail::SearchDetail::detail_id",
        &["finder"],
    ),
    ("widgets::search_detail::SearchDetail::session", &["finder"]),
    (
        "widgets::search_detail::SearchDetail::query",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::selected_key",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::SearchDetail::phase", &["finder"]),
    (
        "widgets::search_detail::SearchDetail::on_detail",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::set_query",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::refresh_source",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::SearchDetail::retry", &["finder"]),
    (
        "widgets::search_detail::SearchDetail::submit",
        &["finder", "holla"],
    ),
    ("widgets::search_detail::SearchDetail::fail", &["finder"]),
    (
        "widgets::search_detail::SearchDetail::on_key",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::on_paste",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::pointer",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::emit_pending_context",
        &["finder"],
    ),
    (
        "widgets::search_detail::SearchDetail::render_source",
        &["finder"],
    ),
    (
        "widgets::search_detail::SearchAction::SelectionChanged",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchAction::QueryCommand",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchAction::DetailItem",
        &["holla"],
    ),
    ("widgets::search_detail::SearchCmd", &["finder", "holla"]),
    (
        "widgets::search_detail::SearchCmd::Activate",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::Up",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::Down",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::Left",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::Right",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::Home",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::End",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::PageUp",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchCmd::PageDown",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchFocusMode",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchFocusMode::Combined",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::keymap",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::focus_mode",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::type_to_query_from_detail",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::reset_preview_on_query",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::configure_query",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::render_hosted",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::config_mut",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchPaint",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::query",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::rows_begin",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::row",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::empty",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::detail",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::summary",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchPaint::is_lead",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::QueryView",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::RowPaint",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::RowPaint::place_cell",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::RowPaint::cell_hit",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::list_detail::DetailPane",
        &["finder_hosted", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::detail_share",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::summary_rows",
        &["finder", "holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::preserve_preview_on_same_key",
        &["finder"],
    ),
];

const SEARCH_PINNED_REASON: &str = "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.";

const SEARCH_HEADLESS_REASON: &str = "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.";

const SEARCH_INTERNAL_REASON: &str = "Reached through the Finder's SearchDetail and its SearchSource without being named in page code: the standard painter and the blanket SearchDetailSource impl use it.";

const HOSTED_PANES_REASON: &str = "SearchDetail::render_hosted and the list-detail hosted layer serve an app that paints every pane itself; apps compose through SearchPaint, so the Finder pages do not call the layer below it directly. The Showcase Finder's Hosted painter and the Holla Finder run through it end to end, and the List & detail page's hosted painter calls it directly.";

const SEARCH_HOLLA_REASON: &str = "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.";

pub const SEARCH_SESSION_ALLOWLIST: &[(&str, &str)] = &[
    ("widgets::search_detail::QueryToken", SEARCH_INTERNAL_REASON),
    ("widgets::search_detail::SearchError", SEARCH_PINNED_REASON),
    (
        "widgets::search_detail::SearchError::StaleQuery",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchError::AlreadyCompleted",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchError::StampMismatch",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchError::InconsistentRow",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchError::InvalidSource",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchError::UnsubmittedProjection",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::FocusQuery",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::FocusResults",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::OpenDetail",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::CloseDetail",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::Context",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchCmd::is_host_owned",
        SEARCH_PINNED_REASON,
    ),
    (
        "widgets::search_detail::SearchFocusMode::Separate",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchConfig::context",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchPhase::Uninitialized",
        SEARCH_INTERNAL_REASON,
    ),
    (
        "widgets::search_detail::SearchSession",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::new",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::config",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::config_mut",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::query",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::query_input",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::phase",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::is_interactive",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::accepted_stamp",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::verify_display",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::selected_key",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::selected_cell",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::selectable_len",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::set_query",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::configure_query",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::refresh_source",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::retry",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::submit",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::fail",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::command_for",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::on_key",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::on_paste",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::run",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::select",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::detail_action",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::press",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::release",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::click",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::invalidate_layout",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::begin_context",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::context_intent",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::emit_context",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchDetail::open_detail",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchDetail::close_detail",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchDetail::render_source_with",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchDetailSource",
        SEARCH_INTERNAL_REASON,
    ),
    (
        "widgets::search_detail::paint_query_default",
        SEARCH_HOLLA_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::accepted_query",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::is_selectable",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::detail_item",
        SEARCH_HEADLESS_REASON,
    ),
    (
        "widgets::search_detail::SearchSession::cancel_press",
        SEARCH_HEADLESS_REASON,
    ),
    ("widgets::list_detail::MasterRow", HOSTED_PANES_REASON),
    ("widgets::list_detail::PaneHost", HOSTED_PANES_REASON),
    (
        "widgets::list_detail::PaneHost::rows_begin",
        HOSTED_PANES_REASON,
    ),
    ("widgets::list_detail::PaneHost::row", HOSTED_PANES_REASON),
    ("widgets::list_detail::PaneHost::empty", HOSTED_PANES_REASON),
    (
        "widgets::list_detail::PaneHost::detail",
        HOSTED_PANES_REASON,
    ),
    (
        "widgets::list_detail::PaneHost::summary",
        HOSTED_PANES_REASON,
    ),
    (
        "widgets::list_detail::PaneHost::is_lead",
        HOSTED_PANES_REASON,
    ),
    (
        "widgets::list_detail::ListDetail::render_hosted",
        HOSTED_PANES_REASON,
    ),
];

#[rustfmt::skip]
pub const PROOFS: &[CapabilityProof] = &[
    CapabilityProof { api: "widgets::picker::Picker", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::PickerAction", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::PickerAction::ChosenMany", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::new", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::multi_select", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::visible_item", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::set_cursor", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::set_visible_keys", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::VisibleKey", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::VisibleKey::new", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::VisibleKey::matched", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::set_items", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection, ScenarioId::CollectionReplacement], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::replace_items", route: PageId::Pickers, scenarios: &[ScenarioId::PickerStableSelection, ScenarioId::CollectionReplacement], feature: None },
    CapabilityProof { api: "widgets::input::TextInput", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::new", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::required", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::validator", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::validate", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::error", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::value", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField::single", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField::validator", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField::validate", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField::error_message", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::EditField::set_text", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::edit::Validator::check", route: PageId::Inputs, scenarios: &[ScenarioId::RequiredCustomValidation], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::disabled", route: PageId::Inputs, scenarios: &[ScenarioId::DisabledInputRejection], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::on_key", route: PageId::Inputs, scenarios: &[ScenarioId::DisabledInputRejection], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::on_paste", route: PageId::Inputs, scenarios: &[ScenarioId::DisabledInputRejection], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::text", route: PageId::Inputs, scenarios: &[ScenarioId::DisabledInputRejection], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::begin_edit", route: PageId::Inputs, scenarios: &[ScenarioId::DisabledInputRejection], feature: None },
    CapabilityProof { api: "ui::popup::Dismiss", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "ui::popup::Dismiss::LIGHT", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "ui::popup::Dismiss::ESCAPE", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "ui::popup::Dismiss::NEVER", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::dismiss", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::Dialog::dismiss", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::Dialog::on_click_outside", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::Dialog::confirm", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::Dialog::on_key", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::Dialog", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::dialog::DialogResult", route: PageId::Dialogs, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::width", route: PageId::Pickers, scenarios: &[ScenarioId::NarrowLayout], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::max_rows", route: PageId::Pickers, scenarios: &[ScenarioId::NarrowLayout], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::placement", route: PageId::Pickers, scenarios: &[ScenarioId::NarrowLayout], feature: None },
    CapabilityProof { api: "terminal::TerminalViewport", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalViewport::new", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalViewport::feed", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalViewport::offset", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalViewport::position_label", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalViewport::reset", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalState", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalState::new", route: PageId::TerminalViewport, scenarios: &[ScenarioId::TerminalViewportLifecycle], feature: Some("terminal") },
    CapabilityProof { api: "widgets::choice::Checkbox", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "widgets::choice::Checkbox::new", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "widgets::choice::Checkbox::fixed", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "widgets::choice::Checkbox::on_key", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "widgets::choice::Checkbox::on_click", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "core::refusal::Reason", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "core::refusal::Reason::new", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "core::refusal::Reason::short", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "core::refusal::take_refusal", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "core::refusal::clear_refusal", route: PageId::FixedControls, scenarios: &[ScenarioId::FixedControlsPolicy], feature: None },
    CapabilityProof { api: "widgets::button::Button::fixed", route: PageId::FixedDialogs, scenarios: &[ScenarioId::FixedDialogsPolicy], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::new", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::select", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::clamp", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "core::text::TextBuffer::line_width", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::diagnostics", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::find_state", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::highlighter_fn", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::invalidate_highlighter", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::invalidate_segmenter", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::push_diagnostic", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::segmenter_fn", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::set_diagnostics", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::set_find_current", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::set_find_query", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorDiagnosticsFind], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::set_highlighter", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::code::CodeEditor::set_segmenter", route: PageId::Editor, scenarios: &[ScenarioId::CodeEditorProviders], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::open_detail", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::close_detail", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::on_key", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::render", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListRow", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListRow::new", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Action::new", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Frame", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Event", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Event::Item", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Line", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "widgets::list_detail::Line::Item", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailNavigation], feature: None },
    CapabilityProof { api: "shell::Cx::notice", route: PageId::StatusHints, scenarios: &[ScenarioId::StatusNotice], feature: None },
    CapabilityProof { api: "shell::Request::Notice", route: PageId::StatusHints, scenarios: &[ScenarioId::StatusNotice], feature: None },
    CapabilityProof { api: "shell::StatusLine::notice", route: PageId::StatusHints, scenarios: &[ScenarioId::StatusNotice], feature: None },
    CapabilityProof { api: "shell::StatusLine::notice_at", route: PageId::StatusHints, scenarios: &[ScenarioId::StatusNotice], feature: None },
    CapabilityProof { api: "widgets::choice_dialog::ChoiceDialog::dismiss", route: PageId::DialogKinds, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::form_dialog::FormDialog::dismiss", route: PageId::DialogKinds, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::info_dialog::InfoDialog::dismiss", route: PageId::DialogKinds, scenarios: &[ScenarioId::OverlayDismissPolicy], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::set_max_rows", route: PageId::Pickers, scenarios: &[ScenarioId::PickerLiveControl], feature: None },
    CapabilityProof { api: "widgets::picker::Picker::toggle_checked", route: PageId::Pickers, scenarios: &[ScenarioId::PickerLiveControl], feature: None },
    CapabilityProof { api: "widgets::input::TextInput::set_value", route: PageId::QuitLeave, scenarios: &[ScenarioId::TextInputSetValue], feature: None },
    CapabilityProof { api: "widgets::option_editor::ChoiceEditor::options", route: PageId::CellEditors, scenarios: &[ScenarioId::ChoiceEditorOptions], feature: None },
    CapabilityProof { api: "widgets::splitter::Splitter::cancel_drag", route: PageId::SplitsStacks, scenarios: &[ScenarioId::SplitterCancelDrag], feature: None },
    CapabilityProof { api: "widgets::grid::CellChange::after", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::CellChange::column", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::CellEditResult", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::CellEditResult::Changed", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::CellEditResult::Unchanged", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::begin_submission", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::cancel_submission", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::columns", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::complete_submission", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::finish_edit_session", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::paste_into_cell", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::pending", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::revision", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::row_key", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::snapshot_changes", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::source_index", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::submission_pending", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_append_rows", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_edit_text", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_set_model", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_set_model_keyed", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_set_rows", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::try_set_rows_keyed", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DataGrid::update_column", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::DeletedRow", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::DeletedRow::before", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::FinishEdit", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::FinishEdit::Accepted", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::FinishEdit::NoSession", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::GridAction::Undone", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet::counts", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet::deleted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet::identity_receipt", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet::inserted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::GridChangeSet::updated", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::GridCommitFailure", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::GridCommitToken", route: PageId::Grid, scenarios: &[ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::GridEditError", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::GridEditError::Busy", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession, ScenarioId::GridSubmissionLock], feature: None },
    CapabilityProof { api: "widgets::grid::GridEditError::InvalidTarget", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::GridEditError::NoChoice", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::GridEditError::Validation", route: PageId::Grid, scenarios: &[ScenarioId::GridEditSession], feature: None },
    CapabilityProof { api: "widgets::grid::GridKeyError", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::GridKeyError::DuplicateKey", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::GridKeyError::KeyCount", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::GridRevision", route: PageId::Grid, scenarios: &[ScenarioId::GridSourceIdentity], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndo", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndoKind", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndoKind::Cell", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndoKind::DeleteMark", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndoKind::Insert", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::GridUndoKind::RemovedInsert", route: PageId::Grid, scenarios: &[ScenarioId::GridUndoKinds], feature: None },
    CapabilityProof { api: "widgets::grid::InsertedRow", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::InsertedRow::key", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::InsertedRow::values", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::PendingChanges::cells", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::PendingChanges::deleted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::PendingChanges::inserted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::PendingChanges::is_deleted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::PendingChanges::is_inserted", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::UpdatedRow", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::UpdatedRow::before", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::UpdatedRow::cells", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
    CapabilityProof { api: "widgets::grid::UpdatedRow::key", route: PageId::Grid, scenarios: &[ScenarioId::GridChangeCapture], feature: None },
CapabilityProof { api: "runtime::ExternalPolicy", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalPolicy::DEFAULT_MAX_DURATION", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalPolicy::DEFAULT_MAX_MESSAGES", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalSendError", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalSendError::Closed", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalSendError::Full", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::ExternalSender::try_send", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "runtime::bounded_external_channel", route: PageId::Activity, scenarios: &[ScenarioId::ExternalFeedBounds], feature: None },
CapabilityProof { api: "ui::modal::FocusOnOpen", route: PageId::FocusNavigation, scenarios: &[ScenarioId::ModalFocusPolicy], feature: None },
CapabilityProof { api: "ui::modal::FocusOnOpen::Preserve", route: PageId::FocusNavigation, scenarios: &[ScenarioId::ModalFocusPolicy], feature: None },
CapabilityProof { api: "ui::modal::FocusOnOpen::Set", route: PageId::FocusNavigation, scenarios: &[ScenarioId::ModalFocusPolicy], feature: None },
CapabilityProof { api: "ui::modal::ModalStack::push_with_focus_policy", route: PageId::FocusNavigation, scenarios: &[ScenarioId::ModalFocusPolicy], feature: None },
CapabilityProof { api: "ui::modal::ModalStack::settle", route: PageId::FocusNavigation, scenarios: &[ScenarioId::ModalFocusPolicy], feature: None },
CapabilityProof { api: "ui::scroll_view::ScrollView::last_work", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
CapabilityProof { api: "ui::scroll_view::ScrollView::render_window", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
CapabilityProof { api: "ui::scroll_view::ScrollWork", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
CapabilityProof { api: "ui::scroll_view::VisibleWindow", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
CapabilityProof { api: "ui::scroll_view::VisibleWindow::content_rows", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
CapabilityProof { api: "ui::scroll_view::VisibleWindow::project_content_rect", route: PageId::Scrolling, scenarios: &[ScenarioId::ScrollWindowWork], feature: None },
    CapabilityProof { api: "widgets::document_workspace::CloseDecision", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::CloseDecision::Cancel", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::CloseDecision::Discard", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::CloseToken", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::activate", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::active_key", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::begin_operation", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::cancel_operation", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::end_operation", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::get", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::index_of", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::iter", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::key_at", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::len", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::new", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::open_with", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::ordered_keys", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::promote", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::request_close", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::resolve_close", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::running", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::sync_tabs_with", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::update", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::DocumentWorkspace::view_mut", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::OpenKind", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::OpenKind::Pinned", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::OpenKind::Preview", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::OperationToken", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction::Activated", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction::CancelOperation", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction::CloseRequested", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction::Closed", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceAction::PreviewPromoted", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::document_workspace::WorkspaceDocument", route: PageId::QueryWorkbench, scenarios: &[ScenarioId::DocumentWorkspaceLifecycle], feature: None },
    CapabilityProof { api: "widgets::completion::Completion::has_eligible", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::completion::Completion::items", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::completion::Completion::set_items", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::OptionList::snapshot", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupRow", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot::more_above", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot::more_below", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot::paint_rows", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot::row_id", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "widgets::option_list::PopupSnapshot::rows", route: PageId::Editor, scenarios: &[ScenarioId::CompletionPopupSnapshot], feature: None },
    CapabilityProof { api: "terminal::SessionClipboardContext", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionClipboardDecision", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionClipboardDecision::Copy", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionClipboardDecision::Refuse", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionClipboardRefusal", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionClipboardRefusal::Write", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionCommand", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionCommand::Input", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionCommand::Redraw", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionCommand::Reply", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionConfig", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionConfig::MINIMUM", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionConfig::new", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionDetach", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionDetach::Operator", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect::ClipboardRefused", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect::Copy", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect::Detached", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect::Failed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEffect::Notice", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEnd", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEnd::Detached", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEnd::Failed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEnd::Refused", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Attached", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Ended", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Exited", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Lease", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Notice", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionEvent::Output", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionFault", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionHostPolicy", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionKey", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionKey::Consumed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionKey::Forwarded", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionLease", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionLease::Owner", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionLease::ReadOnly", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionLink", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionPaste", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionPaste::Dropped", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionPaste::NeedsConfirm", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionPaste::Sent", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionSent", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionSent::Closed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionSent::Queued", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionSize", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionSource", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Attached", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Attaching", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Detached", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Exited", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Failed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::SessionStatus::Pending", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::attention", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::clear", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::close_needs_confirm", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::dropped_replies", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::focus_changed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::gesture", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::has_output", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::key", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::live", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::new", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::next_replay_deadline", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::paste", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::paste_confirmed", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::pump", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::redraw", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::set_config", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::set_host_policy", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionEndingsAndPolicy], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::status", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::sync_size", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::take_effects", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::title", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::too_small", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::viewport", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionAttachReplay], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::viewport_mut", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "terminal::TerminalSessionController::wheel", route: PageId::TerminalPane, scenarios: &[ScenarioId::SessionInputAndPaste], feature: Some("terminal") },
    CapabilityProof { api: "metrics::ENABLED", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::METRIC_COUNT", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::ALL", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::AllocatedBufferCells", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::AnnotationQueries", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::BufferCellsCopied", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::BufferCellsFilled", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::DiffCellsVisited", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::FitCompares", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::FitVisits", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::IdentityComparisons", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::IndexExtends", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::IndexRebuilds", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::IndexShifts", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::InputDispatches", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::SourceRowsEnumerated", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::KeyIndexInserts", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::KeyLookups", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::WidthRowsMeasured", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::OverlayRowsSnapshotted", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::LabelsRead", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::IntrinsicWidthCalls", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::LabelBytesNormalized", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::LabelsNormalized", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::LineCharsCounted", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::NulBytesProbed", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::PayloadBytesCloned", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::PrefixRollbackProbes", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::ProjectionRebuilds", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::RankRowsVisited", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::RawEventsFiltered", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::RawEventsRead", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::ReflowedLines", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::SchedulingPolls", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::SchedulingRecordsVisited", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::SegmentedLines", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::SourceRowsRead", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::TerminalBytesFed", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::TerminalBytesWritten", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::TextBytesMeasured", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::TextBytesSanitized", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::Utf8BytesValidated", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::VisibleRowsPainted", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::Metric::name", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::MetricsError", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::MetricsError::CounterWentBackwards", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::MetricsError::DifferentEpoch", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::MetricsError::DifferentThread", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::MetricsError::Overflow", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::ZERO", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::check_budget", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::check_exact", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::get", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::merged", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkDelta::nonzero", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkSnapshot", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "metrics::WorkSnapshot::delta", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "record_work!", route: PageId::Meters, scenarios: &[ScenarioId::WorkCounterContract], feature: None },
    CapabilityProof { api: "widgets::search_detail::MissingSelection", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::MissingSelection::FirstEnabled", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::MissingSelection::NearestPosition", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryPasteMode", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryPasteMode::JoinLinesWithSpaceTrim", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryPasteMode::StripLineBreaks", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryRequest", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryToken", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryToken::result_stamp", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::QueryView", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::ResultRowKind", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::ResultRowKind::Cells", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::ResultRowKind::Heading", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::ResultRowKind::Item", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::ResultRowKind::Spacer", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::RowPaint", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::RowPaint::cell_hit", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::RowPaint::place_cell", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::Activate", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::ContextRequested", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::DetailAction", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::DetailItem", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::QueryCommand", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::QueryRequested", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchAction::SelectionChanged", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Activate", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::CloseDetail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Down", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::End", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Home", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Left", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::PageDown", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::PageUp", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Right", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::Up", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchCmd::is_host_owned", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::click", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::focus_mode", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::keymap", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::missing_selection", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::navigate_while_loading", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::paste_mode", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::preserve_preview_on_same_key", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::reset_preview_on_query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::select_first_on_query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchConfig::type_to_query_from_detail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::close_detail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::collapse_below", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::config", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::config_mut", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::configure_query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::detail_id", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::detail_share", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::editing", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::emit_pending_context", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::fail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::id", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::new", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::on_blur", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::on_detail", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::on_key", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::on_paste", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::phase", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::pointer", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::refresh_source", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::render_hosted", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::render_source", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::retry", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::selected_key", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::session", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::set_query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::set_query_readout", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::submit", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetail::summary_rows", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchDetailSource", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchFocusMode", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchFocusMode::Combined", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchFocusMode::Separate", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchPaint", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchPhase", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchPhase::Failed", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchPhase::Loading", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchPhase::Ready", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::accepted_query", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::cancel_press", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::config_mut", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::detail_item", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailSelection], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::error", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::is_interactive", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::is_selectable", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSession::query_input", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery], feature: None },
    CapabilityProof { api: "widgets::search_detail::SearchSource", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailQuery, ScenarioId::SearchDetailSelection, ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::search_detail::paint_query_default", route: PageId::Finder, scenarios: &[ScenarioId::SearchDetailPaint], feature: None },
    CapabilityProof { api: "widgets::list::ListAction", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListAction::Activated", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListAction::SelectionChanged", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListBox::keyed", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListBox::on_key_source", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListBox::on_pointer_source", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListBox::render_source", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListItem::borrowed", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListItemRef", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::list::ListSource", route: PageId::Lists, scenarios: &[ScenarioId::KeyedListSource], feature: None },
    CapabilityProof { api: "widgets::nav_list::NavList::set_items", route: PageId::Sidebars, scenarios: &[ScenarioId::NavItemReplacement], feature: None },
    CapabilityProof { api: "widgets::table::Cell::borrowed", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::CellRef", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::columns", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::keyed", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableSource], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::on_key_edit_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableEdit], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::on_blur_edit_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableEdit], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::on_key_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableSource], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::on_pointer_edit_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableEdit], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::on_pointer_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableSource], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::render_edit_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableEdit], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::render_source", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableSource], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::rows", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::try_new", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::try_set_model", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::try_set_row", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::try_set_rows", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::update_cell", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::DataTable::update_column", route: PageId::Tables, scenarios: &[ScenarioId::TableModelUpdates], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::insert", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::is_empty", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::len", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::move_row", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::remove", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::replace_rows", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::row", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::row_keys", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::try_new", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTable::update_cell", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTableError", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTableError::Key", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::KeyedTableError::Shape", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::table::TableEditSource", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableEdit], feature: None },
    CapabilityProof { api: "widgets::table::TableSource", route: PageId::Tables, scenarios: &[ScenarioId::KeyedTableStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::get", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::insert", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::is_empty", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::len", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::new", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::remove", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTree::update", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode::children", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode::glyph", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode::lazy", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode::meta", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::KeyedTreeNode::new", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::TreeNodeRef", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::TreeSource", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeStore], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::expand_all_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::keyed", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::on_key_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::on_pointer_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::render_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::select_follows_cursor", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::select_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::set_cursor_source", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::tree::TreeView::show_selection", route: PageId::Trees, scenarios: &[ScenarioId::KeyedTreeView], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::bind_source", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::click_source", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::keyed", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::render_source", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::selected_key", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetailSource", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListRow::borrowed", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListRowRef", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::SourceFrame", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailKeyedSource], feature: None },
    CapabilityProof { api: "widgets::list_detail::DetailPane", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::drawer_open", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::master_offset", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::master_page", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::on_detail_key", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::render_hosted", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::render_source_selected_with", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::reset_detail", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::ListDetail::wheel_event", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::MasterRow", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::list_detail::PaneHost", route: PageId::ListDetail, scenarios: &[ScenarioId::ListDetailHostedPanes], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::add_label", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::chips", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::insert_chip", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::lead", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::remove_chip", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::set_add_label", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::set_chips", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::set_lead", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::chips::ChipBar::update_chip", route: PageId::Chips, scenarios: &[ScenarioId::ChipBarEditing], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::clear", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::extend_lines", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::lines", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::replace_line", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::set_lines", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::panel::ScrollPanel::source_selection", route: PageId::Panels, scenarios: &[ScenarioId::ScrollPanelContent], feature: None },
    CapabilityProof { api: "widgets::tabs::Tabs::items", route: PageId::Tabs, scenarios: &[ScenarioId::TabsEditing], feature: None },
    CapabilityProof { api: "widgets::tabs::Tabs::push", route: PageId::Tabs, scenarios: &[ScenarioId::TabsEditing], feature: None },
    CapabilityProof { api: "widgets::tabs::Tabs::reorder", route: PageId::Tabs, scenarios: &[ScenarioId::TabsEditing], feature: None },
    CapabilityProof { api: "widgets::tabs::Tabs::update_item", route: PageId::Tabs, scenarios: &[ScenarioId::TabsEditing], feature: None },
    CapabilityProof { api: "widgets::toolbar::Toolbar::insert_item", route: PageId::Toolbars, scenarios: &[ScenarioId::ToolbarEditing], feature: None },
    CapabilityProof { api: "widgets::toolbar::Toolbar::items", route: PageId::Toolbars, scenarios: &[ScenarioId::ToolbarEditing], feature: None },
    CapabilityProof { api: "widgets::toolbar::Toolbar::remove_item", route: PageId::Toolbars, scenarios: &[ScenarioId::ToolbarEditing], feature: None },
    CapabilityProof { api: "widgets::toolbar::Toolbar::set_items", route: PageId::Toolbars, scenarios: &[ScenarioId::ToolbarEditing], feature: None },
    CapabilityProof { api: "widgets::toolbar::Toolbar::update_item", route: PageId::Toolbars, scenarios: &[ScenarioId::ToolbarEditing], feature: None },
    CapabilityProof { api: "core::data::DataError", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::data::DataError::EmptySchemaWithRows", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::data::DataError::MissingRow", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::data::DataError::Shape", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::data::ShapeError", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::data::validate_rectangular", route: PageId::Tables, scenarios: &[ScenarioId::DataShapeErrors], feature: None },
    CapabilityProof { api: "core::keyed::KeyedRecord", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError", route: PageId::Lists, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::Cycle", route: PageId::Trees, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::DuplicateAuthoritative", route: PageId::Lists, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::DuplicateVisible", route: PageId::Lists, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::MissingNode", route: PageId::Trees, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::PositionalKey", route: PageId::Lists, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedSourceError::UnknownVisible", route: PageId::Lists, scenarios: &[ScenarioId::KeyedSourceRejection], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::adopt_cursor_view", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::adopt_selection_view", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::anchor", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::anchor_view", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::begin_edit", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::cancel_edit", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::cancel_press", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::capture_press", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::clear_cursor", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::cursor", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::cursor_view", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::edit_key", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::expanded", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::expanded_revision", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::finish_edit", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::invalidate_layout", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::is_authoritative", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::is_enabled", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::is_expanded", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::is_selected", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::new", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::pressed", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::reconcile", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::release_press", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::replace_selection", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::selected", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::selected_len", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::selected_view", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::set_anchor", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::set_cursor", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::set_expanded", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::set_selected", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::stamp", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::visible_index", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::visible_key", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedState::visible_len", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::get", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::insert", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::is_empty", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::iter", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::key_at", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::len", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::move_to", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::new", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::position", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::push", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::remove", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::replace", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::replace_if_changed", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::stamp", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::try_from_rows", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::update", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::KeyedVec::value_at", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::keyed::Reconcile", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::list::KeyedRowSource", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::list::SourceStamp", route: PageId::Lists, scenarios: &[ScenarioId::KeyedStoreIdentity], feature: None },
    CapabilityProof { api: "core::pointer::PointerEvent::key_of", route: PageId::Lists, scenarios: &[ScenarioId::KeyedPointerPress], feature: None },
    CapabilityProof { api: "core::response::Actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::Many", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::None", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::One", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::append", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::is_empty", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::iter", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::len", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Actions::push", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIntoIter", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIntoIter::Many", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIntoIter::Small", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIter", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIter::Many", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::ActionsIter::Small", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::has_actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::into_actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::into_raw_parts", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::map_actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::merge", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::on_actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::push_action", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::take_actions", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::response::Response::try_into_single", route: PageId::Lists, scenarios: &[ScenarioId::ResponseBatching], feature: None },
    CapabilityProof { api: "core::selection::RowSelection::replace", route: PageId::Lists, scenarios: &[ScenarioId::KeyedPointerPress], feature: None },
    CapabilityProof { api: "core::text::TextBuffer::line_range", route: PageId::TextUtilities, scenarios: &[ScenarioId::TextBufferGeometry], feature: None },
    CapabilityProof { api: "core::text::TextBuffer::max_line_width", route: PageId::TextUtilities, scenarios: &[ScenarioId::TextBufferGeometry], feature: None },
    CapabilityProof { api: "core::text::TextBuffer::position_at", route: PageId::TextUtilities, scenarios: &[ScenarioId::TextBufferGeometry], feature: None },
    CapabilityProof { api: "core::text::TextBuffer::revision", route: PageId::TextUtilities, scenarios: &[ScenarioId::TextBufferGeometry], feature: None },
];

#[rustfmt::skip]
pub const EXCEPTIONS: &[CapabilityException] = &[
    CapabilityException { api: "widgets::picker::Picker::render", kind: ExceptionKind::Nonvisual, reason: "The page composes pickers through layers and does not call the widget's direct render entry point.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "core::hints::HintCx::current", kind: ExceptionKind::Nonvisual, reason: "Pages draw widgets through the showcase harness, which supplies the hint context; no page builds one from ambient state.", regression_test: "src/widgets/picker.rs::searchable_multi_profile_binding_is_visible_exact_and_preserves_space_input", tracking_issue: None },
    CapabilityException { api: "core::keymap::KeyboardCapabilities", kind: ExceptionKind::Nonvisual, reason: "Pages render with the ambient terminal capabilities and never construct a capability set; every hint bar already reflects them.", regression_test: "src/widgets/picker.rs::custom_multi_hints_follow_the_effective_binding_and_capabilities", tracking_issue: None },
    CapabilityException { api: "core::keymap::KeyboardCapabilities::current", kind: ExceptionKind::Nonvisual, reason: "Showcase pages ask widgets for their hints instead of probing the thread-local capability state themselves.", regression_test: "src/widgets/picker.rs::custom_multi_hints_follow_the_effective_binding_and_capabilities", tracking_issue: None },
    CapabilityException { api: "terminal::ForwardPolicy", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::ctrl_d_and_ctrl_q_never_reach_the_program", tracking_issue: None },
    CapabilityException { api: "terminal::ForwardPolicy::confirmed", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::a_confirmed_paste_is_sent_as_it_is_without_brackets", tracking_issue: None },
    CapabilityException { api: "terminal::ForwardPolicy::key", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::ctrl_d_and_ctrl_q_never_reach_the_program", tracking_issue: None },
    CapabilityException { api: "terminal::ForwardPolicy::new", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::a_paste_over_the_limit_is_refused_by_its_byte_count", tracking_issue: None },
    CapabilityException { api: "terminal::ForwardPolicy::paste", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::a_paste_drops_control_bytes_but_keeps_tab_and_newline", tracking_issue: None },
    CapabilityException { api: "terminal::Paste", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::one_line_is_sent_and_many_lines_ask_first", tracking_issue: None },
    CapabilityException { api: "terminal::Paste::NeedsConfirm", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::one_line_is_sent_and_many_lines_ask_first", tracking_issue: None },
    CapabilityException { api: "terminal::Paste::Send", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::a_paste_drops_control_bytes_but_keeps_tab_and_newline", tracking_issue: None },
    CapabilityException { api: "terminal::Paste::TooLarge", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/forward_policy.rs::a_paste_over_the_limit_is_refused_by_its_byte_count", tracking_issue: None },
    CapabilityException { api: "terminal::RowWrap", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::view_reports_soft_wraps", tracking_issue: None },
    CapabilityException { api: "terminal::RowWrap::Hard", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::view_reports_soft_wraps", tracking_issue: None },
    CapabilityException { api: "terminal::RowWrap::Soft", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::view_reports_soft_wraps", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::Bell", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::ClipboardWrite", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::Cwd", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::Notification", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::Reply", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::replies_for_queries", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::ScrollbackCleared", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal_viewport.rs::scrollback_cleared_resets", tracking_issue: None },
    CapabilityException { api: "terminal::TermEvent::Title", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::passthrough_events_map", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::clear_scrollback", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::clear_scrollback_drops_the_history_and_keeps_the_screen", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::focus_events", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::the_program_turns_focus_events_and_mouse_reporting_on_and_off", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::mouse_reporting", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::the_program_turns_focus_events_and_mouse_reporting_on_and_off", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::scrolled_off", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::scrolled_off_is_exact_after_many_feeds", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::view", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::scrolled_view_reads_scrollback", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalState::with_scrollback", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal.rs::with_scrollback_limit_is_honoured", tracking_issue: None },
    CapabilityException { api: "terminal::TerminalViewport::clear_scrollback", kind: ExceptionKind::Nonvisual, reason: "used by the host that feeds the terminal state (the jackin Capsule) and pinned by tests/terminal.rs, tests/terminal_viewport.rs and tests/forward_policy.rs; the terminalpane page keeps its approved baseline", regression_test: "tests/terminal_viewport.rs::clear_scrollback_returns_to_the_tail_and_drops_the_selection", tracking_issue: None },
    CapabilityException { api: "widgets::picker::DEFAULT_GLYPH", kind: ExceptionKind::Nonvisual, reason: "Every showcase row sets its glyph to show the list style explicitly, so the default glyph is not visible in this page.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::MAX_ROWS", kind: ExceptionKind::Nonvisual, reason: "The page exposes row limits through the configured builder and does not refer to the widget's default constant directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::WIDTH", kind: ExceptionKind::Nonvisual, reason: "The page exposes widths through the configured builder and does not refer to the widget's default constant directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::caret", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates filtering through terminal input and does not inspect the edit caret directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::checked", kind: ExceptionKind::Nonvisual, reason: "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading the checked-key buffer directly.", regression_test: "src/widgets/picker.rs::searchable_multi_keeps_hidden_identities_across_filter_and_reorder", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::choose_hint", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates choice labels through its configured hint lines rather than this per-picker label builder.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::current", kind: ExceptionKind::Nonvisual, reason: "The page presents row selection through the picker layer and does not inspect the selected item directly.", regression_test: "src/widgets/picker.rs::searchable_multi_profile_binding_is_visible_exact_and_preserves_space_input", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::current_key", kind: ExceptionKind::Nonvisual, reason: "The page presents row selection through the picker layer and does not inspect the selected key directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::draw", kind: ExceptionKind::Nonvisual, reason: "The page composes pickers through layers and does not draw the widget directly into a buffer.", regression_test: "src/widgets/picker.rs::searchable_multi_keeps_hidden_identities_across_filter_and_reorder", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::eligible", kind: ExceptionKind::Nonvisual, reason: "The page presents disabled-row behavior through visible rows and does not query eligible indices directly.", regression_test: "src/widgets/picker.rs::corrected_items_restore_readiness_after_report_item_error", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::height", kind: ExceptionKind::Nonvisual, reason: "The page delegates sizing to its layer and layout system rather than calling the widget height helper directly.", regression_test: "src/widgets/picker.rs::zero_row_limit_is_one_row_in_every_status", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::is_multi", kind: ExceptionKind::Nonvisual, reason: "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading widget mode directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::is_searchable", kind: ExceptionKind::Nonvisual, reason: "The page presents search behavior through visible picker interaction and does not inspect the internal mode flag.", regression_test: "src/widgets/picker.rs::enabling_searchable_multi_rejects_unstable_items", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::item_for_key", kind: ExceptionKind::Nonvisual, reason: "The page presents item behavior through visible rows and does not query the backing item map directly.", regression_test: "src/widgets/picker.rs::explicit_index_picker_keys_survive_reorder", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::keymap", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates the supported Desktop and Vim profiles; custom keymaps are not a page configuration option.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::on_click", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates pointer behavior through terminal events and does not call the widget event handler directly.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::on_click_outside", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates dismissal through the layer interaction and does not invoke its low-level outside-click handler.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::on_key", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates keyboard behavior through terminal events and does not call the widget event handler directly.", regression_test: "src/widgets/picker.rs::searchable_multi_keeps_hidden_identities_across_filter_and_reorder", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::on_paste", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates filtering through ordinary terminal input and has no paste-specific picker scenario.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::reset_selection", kind: ExceptionKind::Nonvisual, reason: "Picker layers in the showcase are ephemeral and have no user-facing reset action; the widget state test exercises the reset operation directly.", regression_test: "src/widgets/picker.rs::hidden_deleted_and_disabled_choices_are_pruned_and_restoration_does_not_reselect", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::row_id", kind: ExceptionKind::Nonvisual, reason: "The page delegates row identity and pointer routing to the picker layer rather than calling the row-id helper directly.", regression_test: "src/widgets/picker.rs::picker_layer_pointer_toggle_repaints_stays_open_and_skips_disabled_rows", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::searchable", kind: ExceptionKind::Nonvisual, reason: "The page uses the explicit searchable state setter for its nonsearchable examples and does not need the builder form.", regression_test: "src/widgets/picker.rs::refreshing_a_nonsearch_multi_picker_clears_position_keys", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_chosen", kind: ExceptionKind::Nonvisual, reason: "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_placement", kind: ExceptionKind::Nonvisual, reason: "The showcase configures placement through its builder; no concrete app uses this setter.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_query", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates filtering through terminal input and does not set the query programmatically.", regression_test: "src/widgets/picker.rs::searchable_layer_routes_edit_menu_enter_before_multi_confirm_and_insert", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_width", kind: ExceptionKind::Nonvisual, reason: "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::toggle_binding", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates the supported Desktop and Vim profiles; custom toggle bindings are not a page configuration option.", regression_test: "src/widgets/picker.rs::custom_multi_hints_follow_the_effective_binding_and_capabilities", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::wheel", kind: ExceptionKind::Nonvisual, reason: "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_visible_items", kind: ExceptionKind::Nonvisual, reason: "Compatibility ingress that keeps only each item's key and match; the showcase uses set_visible_keys, and tests/regressions/keyed_costs pins the old ingress.", regression_test: "src/widgets/picker.rs::picker_rejects_ambiguous_and_unknown_keys_atomically", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::visible_items", kind: ExceptionKind::Nonvisual, reason: "Read-only enumeration of the projection for application code; the showcase reads one row through visible_item.", regression_test: "src/widgets/picker.rs::picker_visible_projection_reports_rows_in_display_order", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::visible_len", kind: ExceptionKind::Nonvisual, reason: "Read-only count of the projection for application code; the showcase has no counter of visible rows.", regression_test: "src/widgets/picker.rs::picker_visible_projection_reports_rows_in_display_order", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerItemView", kind: ExceptionKind::Nonvisual, reason: "Returned by visible_item and visible_items; pages read its item and matched fields without naming the type.", regression_test: "src/widgets/picker.rs::picker_visible_projection_reports_rows_in_display_order", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerCmd", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command type as a page action.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerCmd::Toggle", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command variant as a page action.", regression_test: "src/widgets/picker.rs::a_custom_toggle_command_binding_checks_the_cursor_item", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerItemsError", kind: ExceptionKind::Nonvisual, reason: "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.", regression_test: "src/widgets/picker.rs::corrected_items_restore_readiness_after_report_item_error", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerItemsError::DuplicateKey", kind: ExceptionKind::Nonvisual, reason: "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.", regression_test: "src/widgets/picker.rs::duplicate_picker_keys_are_rejected_without_mutation_or_panic", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerItemsError::MissingStableKey", kind: ExceptionKind::Nonvisual, reason: "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.", regression_test: "src/widgets/picker.rs::enabling_searchable_multi_rejects_unstable_items", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerItemsError::UnknownVisibleKey", kind: ExceptionKind::Nonvisual, reason: "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.", regression_test: "src/widgets/picker.rs::picker_rejects_ambiguous_and_unknown_keys_atomically", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerKeyProfile::current", kind: ExceptionKind::Nonvisual, reason: "The page configures each picker through its builder and intentionally does not read the thread-local default profile.", regression_test: "src/widgets/picker.rs::searchable_multi_profile_binding_is_visible_exact_and_preserves_space_input", tracking_issue: None },
    CapabilityException { api: "widgets::picker::PickerKeyProfile::install", kind: ExceptionKind::Nonvisual, reason: "The page configures each picker through its builder and intentionally does not change the thread-local default profile.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "catalog::ALLOWLIST", kind: ExceptionKind::Nonvisual, reason: "The catalog's own table of APIs allowed to lack a Showcase page, and its record type; they are data read by the coverage tests and no page can show a table of ids.", regression_test: "src/catalog_contract.rs::catalog_allowlist_records_are_unique_exact_ids_with_a_reason", tracking_issue: None },
    CapabilityException { api: "catalog::Allowed", kind: ExceptionKind::Nonvisual, reason: "The catalog's own table of APIs allowed to lack a Showcase page, and its record type; they are data read by the coverage tests and no page can show a table of ids.", regression_test: "src/catalog_contract.rs::catalog_allowlist_records_are_unique_exact_ids_with_a_reason", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::Environment", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::MeasurementReport", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::MeasurementReport::file_name", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::MeasurementReport::to_json", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_required_fields_are_all_present", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::MeasurementReport::write_in", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::REQUIRED_FIELDS", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_required_fields_are_all_present", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::SAMPLES", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::SCHEMA_VERSION", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_required_fields_are_all_present", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::WARMUPS", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_report_is_written_whole_under_its_file_name", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::median_ns", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_percentiles_follow_the_specified_indices", tracking_issue: None },
    CapabilityException { api: "metrics::measurement::p95_ns", kind: ExceptionKind::Nonvisual, reason: "The elapsed-time report of the performance measurements (#56): a benchmark artifact written by ignored release tests, with no screen a Showcase page could draw.", regression_test: "src/metrics/measurement.rs::measurement_percentiles_follow_the_specified_indices", tracking_issue: None },
    CapabilityException { api: "runtime::SessionTerminal", kind: ExceptionKind::Nonvisual, reason: "The terminal-side session handle the runtime tests open to drive input, resize and resume; it draws nothing a page could show.", regression_test: "src/runtime/tests.rs::session_terminal_is_metered_exactly_when_work_metrics_is_on", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::policy", kind: ExceptionKind::Nonvisual, reason: "A read accessor the runtime uses to copy the bound before each turn; the Activity page shows the bound through the pages it limits, not as a value.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::set_policy", kind: ExceptionKind::Nonvisual, reason: "The Activity page passes its policy to bounded_external_channel; the setter is covered by the bounded_external_policy_ unit test.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::try_recv", kind: ExceptionKind::Nonvisual, reason: "The runtime calls it between handlers; no page reads the queue itself. The runtime unit tests external_turn_ and bounded_external_ pin its order and bound.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::with_policy", kind: ExceptionKind::Nonvisual, reason: "The Activity page passes its policy to bounded_external_channel; the builder is covered by the bounded_external_policy_ unit test.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::RawInputPolicy", kind: ExceptionKind::Nonvisual, reason: "Raw terminal event budgets act below the widget layer and draw nothing; pinned by the runtime::tests raw_input_ and startup_discard_ unit tests, and the showcase runs on the default policy.", regression_test: "src/runtime.rs::raw_input_empty_and_read_error_are_distinct", tracking_issue: None },
    CapabilityException { api: "runtime::RawInputPolicy::DEFAULT_MAX_DURATION", kind: ExceptionKind::Nonvisual, reason: "The default raw-input time budget is read only by the runtime between terminal reads and draws nothing, so no page can show it; the unit test pins its 2 ms value.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: None },
    CapabilityException { api: "runtime::RawInputPolicy::DEFAULT_MAX_EVENTS", kind: ExceptionKind::Nonvisual, reason: "The default raw-input event budget is read only by the runtime between terminal reads and draws nothing, so no page can show it; the unit test pins its 256 events.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: None },
    CapabilityException { api: "runtime::drain_pending_input_bounded", kind: ExceptionKind::Nonvisual, reason: "It runs once in shell::run before the terminal session starts, discarding stale input bytes with no frame drawn, so no page can show it; the startup_discard unit tests pin its bound.", regression_test: "src/runtime.rs::the_public_bounded_drain_with_a_zero_bound_reads_nothing", tracking_issue: None },
    CapabilityException { api: "terminal::PENDING_EFFECTS", kind: ExceptionKind::Nonvisual, reason: "the cap of 16 queued host effects; the page keeps three on screen, so the cap is not visible here. session_controller_effect_queue_keeps_the_newest_sixteen pins it.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: None },
    CapabilityException { api: "terminal::ReplayTiming", kind: ExceptionKind::Nonvisual, reason: "the three numbers that set the replay window; the page keeps the defaults of SessionConfig::new and sets none of its own. session_controller_replay_deadline_follows_output pins the deadlines they produce.", regression_test: "tests/terminal/session.rs::session_replay_timing_is_the_default_window_a_config_carries", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardRefusal::Read", kind: ExceptionKind::Nonvisual, reason: "the page's scripts only write to the clipboard and never send a read request, so DemoPolicy never reports Read. session_controller_clipboard_decisions_and_refusal_throttle pins the refusal.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardRefusal::TooLarge", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy reports only Write and Read refusals, never TooLarge. session_controller_clipboard_decisions_and_refusal_throttle pins it with a 'big' payload.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionCommand::Detach", kind: ExceptionKind::Nonvisual, reason: "the controller never sends Detach. apps/jackin maps it to the pane's own command, and no page control or test sends it.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: None },
    CapabilityException { api: "terminal::SessionDetach::TransportLost", kind: ExceptionKind::Nonvisual, reason: "no page script ends the stream after the attach, so the page cannot reach this status. session_controller_resize_retry_restart_transcript pins it.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: None },
    CapabilityException { api: "terminal::SessionEnd::StreamClosed", kind: ExceptionKind::Nonvisual, reason: "no page script closes the stream. session_controller_resize_retry_restart_transcript pins a close before the attach and a close after it.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: None },
    CapabilityException { api: "terminal::SessionPaste::Full", kind: ExceptionKind::Nonvisual, reason: "the page's scripted link always queues, so a paste never meets a full queue. session_controller_reply_and_paste_limits pins it.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "terminal::SessionPaste::TooLarge", kind: ExceptionKind::Nonvisual, reason: "the page pastes only a one-line and a three-line text, both under SessionConfig::paste_max_bytes. session_controller_reply_and_paste_limits pins the refusal with a nine-byte paste against a limit of eight.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "terminal::SessionSent::Full", kind: ExceptionKind::Nonvisual, reason: "the page's scripted link always queues. session_controller_reply_and_paste_limits pins a full queue.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::project_point", kind: ExceptionKind::Nonvisual, reason: "The Virtual rows demo has no point-anchored overlay; cursor and caret hosts project a point from the same window.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: None },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::project_rect", kind: ExceptionKind::Nonvisual, reason: "The Virtual rows demo projects model rows with project_content_rect, which wraps it; a padded-canvas rectangle is only needed by hosts that position a popup anchor.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: None },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::rows", kind: ExceptionKind::Nonvisual, reason: "The demo iterates content_rows, which is rows without the padding; rows is the padded-canvas range for painting padding decorations.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_padding_maps_source_row_zero", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::begin_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_paint_and_navigation_preserve_close_token", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::cancel_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's connection reset and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_cancel_close_save_is_targeted", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::complete_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_invalid_preflight_never_invokes_apply_saved", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::contains", kind: ExceptionKind::Nonvisual, reason: "The page finds documents by title through iter, so the membership predicate has no call site; the conformance suite asserts it.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::fail_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::finish_operation", kind: ExceptionKind::Nonvisual, reason: "Only an operation whose result changes content ends here, such as a TablePro grid commit; Showcase runs fill a result pane and end through end_operation. tests/regressions/document_workspace.rs asserts it.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::id", kind: ExceptionKind::Nonvisual, reason: "The page keeps its own widget id constant and never asks the controller for it.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::is_empty", kind: ExceptionKind::Nonvisual, reason: "The page counts tabs with len, so the emptiness predicate has no call site.", regression_test: "tests/regressions/document_workspace.rs::workspace_first_update_that_dirties_a_preview_promotes_once", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::open", kind: ExceptionKind::Nonvisual, reason: "The page opens through open_with because each document builds its ids from its own key.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_pinned_activates_and_leaves_others_alone", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::pending_close", kind: ExceptionKind::Nonvisual, reason: "The page keeps the token it was handed and never asks for the stored one; TablePro's save-then-close and the conformance suite do.", regression_test: "tests/regressions/document_workspace.rs::workspace_resolve_cancel_clears_only_that_request", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::preview_key", kind: ExceptionKind::Nonvisual, reason: "The page opens and promotes previews through the controller's actions and never reads the slot; TablePro's workbench and the conformance suite do.", regression_test: "tests/regressions/document_workspace.rs::workspace_preview_promotes_before_replacement", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::reorder", kind: ExceptionKind::Nonvisual, reason: "Tabs emits no move gesture, so no page can drive a reorder; the conformance suite and the example do.", regression_test: "tests/regressions/document_workspace.rs::workspace_identity_survives_reorder_and_close", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::revision", kind: ExceptionKind::Nonvisual, reason: "The page never captures a content revision; the conformance suite asserts that view work leaves it unchanged.", regression_test: "tests/regressions/document_workspace.rs::workspace_operation_progress_keeps_the_close_token_valid", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::save_locked", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved, so no page holds a close-save lock; TablePro's key and pointer paths read it.", regression_test: "tests/regressions/document_workspace.rs::workspace_cancel_close_save_is_targeted", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::scoped_id", kind: ExceptionKind::Nonvisual, reason: "The page derives its editor and result ids with its own key-scoped helpers; TablePro's tab ids and the conformance suite use the controller's.", regression_test: "tests/regressions/document_workspace.rs::workspace_scoped_ids_follow_the_key_not_the_position", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::sync_tabs", kind: ExceptionKind::Nonvisual, reason: "The page decorates its tab items, so it calls sync_tabs_with; the conformance suite and the example call the plain projection.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_translates_events_by_key", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::validate_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text and never matches a variant, so each variant is asserted by tests/regressions/document_workspace.rs instead.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::MissingDocument", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_completion_uses_originating_token", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::SaveInProgress", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StaleClose", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StaleOperation", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StillDirty", kind: ExceptionKind::Nonvisual, reason: "Only the save protocol returns it, and Showcase documents are never saved; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::canonical_uuid", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its Id columns; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::date_or_timestamp", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its renewed_at (Timestamp) column; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::json_container", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its notes (Json) column; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::grid::CellChange", kind: ExceptionKind::Nonvisual, reason: "The page reads a changed cell through its accessors and never names the type.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_semantic_noop_overrides_produce_no_update", tracking_issue: None },
    CapabilityException { api: "widgets::grid::CellChange::before", kind: ExceptionKind::Nonvisual, reason: "The UPDATE preview matches on the row's base values from UpdatedRow::before, so the per-cell before value is not shown.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: None },
    CapabilityException { api: "widgets::grid::CommittedRow", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::commit_failure", kind: ExceptionKind::Nonvisual, reason: "The Grid page marks the failed row; the grid-level message for a failure that names no row is asserted in src/widgets/grid_submission_tests.rs.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_backend_failure_unlocks_and_keeps_the_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::instance_id", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::validate_submission_receipt", kind: ExceptionKind::Nonvisual, reason: "Used by a host that lets an external store mutate before it completes; the Grid page has no store, and complete_submission runs the same check, which src/widgets/grid_submission_tests.rs asserts.", regression_test: "src/bin/tablepro/store_tests.rs::issue51_receipt_matches_the_grid_it_answers", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DeletedRow::key", kind: ExceptionKind::Nonvisual, reason: "The DELETE preview matches on the base row from DeletedRow::before, so the row key is not shown.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_receipt_records_the_snapshot_it_answers", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::is_empty", kind: ExceptionKind::Nonvisual, reason: "The Showcase page keeps its own pending-count guard before it opens the dialog, and the dialog already shows the zero counts.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::origin", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::revision", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/bin/showcase/pages/grid.rs::issue12_showcase_reload_asks_before_dropping_a_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::same_capture", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_same_capture_is_bit_exact_where_eq_is_not", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Busy", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Editing", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_save_finishes_live_editor_once", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Empty", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_sort_tests.rs::issue46_empty_commit_and_stale_sort_column_are_safe", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Exhausted", kind: ExceptionKind::Nonvisual, reason: "Only a submission nonce or revision counter already at u64::MAX returns it, about 1.8e19 saves, so no session reaches it; the named test sets the nonce there and pins the refusal.", regression_test: "src/widgets/grid_submission_tests.rs::a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::InvalidReceipt", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::StaleRevision", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_begin_submission_checks_the_capture", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::StaleToken", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_completed_token_is_stale", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::WrongOrigin", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::new", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::origin", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::revision", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitToken::origin", kind: ExceptionKind::Nonvisual, reason: "A host that holds a token may compare its grid; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitToken::revision", kind: ExceptionKind::Nonvisual, reason: "A host that holds a token may compare its capture revision; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.", regression_test: "src/bin/showcase/pages/grid.rs::issue12_showcase_reload_asks_before_dropping_a_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Busy", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::ColumnWidth", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "tests/regressions/data_shape.rs::issue42_grid_model_rejects_unusable_width_ranges", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Key", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Shape", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "tests/regressions/data_shape.rs::issue42_empty_schema_is_inert", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridInstanceId", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Duplicate", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::KeyCollision", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Missing", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Unexpected", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::UpdatedKeyChanged", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Width", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Context", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::FocusQuery", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::FocusResults", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::OpenDetail", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchConfig::context", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_secondary_click_selects_then_requests_through_context_config", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchDetail::open_detail", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchDetail::render_source_with", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_unsubmitted_source_is_refused_before_any_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::AlreadyCompleted", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::InconsistentRow", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_item_key_alias_validates_once", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::InvalidSource", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::StaleQuery", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_rejects_stale_submission", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::StampMismatch", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::UnsubmittedProjection", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_pending_renders_only_last_accepted_source", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchPhase::Uninitialized", kind: ExceptionKind::Nonvisual, reason: "Reached through the Finder's SearchDetail and its SearchSource without being named in page code: the standard painter and the blanket SearchDetailSource impl use it.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::accepted_stamp", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_rejects_stale_submission", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::begin_context", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::click", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_click_policy_is_per_cell", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::command_for", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::config", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::configure_query", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_configure_query_requests_on_text_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::context_intent", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::detail_action", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_detail_action_captures_selected_key", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::emit_context", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::fail", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_failed_requires_retry", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::invalidate_layout", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_layout_change_cancels_press", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::new", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::on_key", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::on_paste", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_paste_respects_detail_focus", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::phase", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::press", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_layout_change_cancels_press", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::query", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::refresh_source", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_requests_once_per_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::release", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_layout_change_cancels_press", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::retry", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_failed_requires_retry", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::run", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::select", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_change_selects_first", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::selectable_len", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_item_key_alias_validates_once", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::selected_cell", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_grouped_cells_preserve_identity", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::selected_key", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::set_query", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_requests_once_per_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::submit", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::verify_display", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_pending_renders_only_last_accepted_source", tracking_issue: None },
    CapabilityException { api: "clock::TestRequest::FENCE_ENV", kind: ExceptionKind::Nonvisual, reason: "names the environment variable a functional harness sets to ask a program to answer the input fence; it draws nothing, and tests/functional/support.rs sets it for every Showcase and Holla launch", regression_test: "tests/shell.rs::the_fence_opt_in_follows_its_own_variable", tracking_issue: None },
    CapabilityException { api: "clock::TestRequest::fence_opted_in", kind: ExceptionKind::Nonvisual, reason: "read once by the shell at start to decide whether it answers the input fence; it draws nothing and has no visible state to demo", regression_test: "tests/shell.rs::the_fence_opt_in_follows_its_own_variable", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/metrics/tests.rs::harness_send_draws_after_every_input_and_warm_runs_that_many_ticks", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::draw", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "apps/jackin/tests/regressions/work_budgets.rs::work_contract_jackin_capsule_frame_after_typing", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::new", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/metrics/tests.rs::harness_send_draws_after_every_input_and_warm_runs_that_many_ticks", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::press", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/bin/tablepro/work_metrics_tests.rs::work_contract_tablepro_idle_and_cell_edit", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::report", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/bin/showcase/work_metrics_tests.rs::work_contract_showcase_idle_and_switch", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::screen", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/bin/showcase/work_metrics_tests.rs::work_contract_showcase_idle_and_switch", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::screen_without_spinner", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/bin/holla/work_metrics_tests.rs::work_contract_holla_idle_and_source_update", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::send", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/metrics/tests.rs::harness_send_draws_after_every_input_and_warm_runs_that_many_ticks", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::tick", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/metrics/tests.rs::harness_tick_is_one_tick_input_and_one_frame", tracking_issue: None },
    CapabilityException { api: "metrics::harness::Harness::warm", kind: ExceptionKind::Nonvisual, reason: "The off-screen harness that drives an Application through its real input and render path for the work-budget tests; it draws nothing a page could show and exists only with the `work-metrics` feature.", regression_test: "src/metrics/tests.rs::harness_send_draws_after_every_input_and_warm_runs_that_many_ticks", tracking_issue: None },
    CapabilityException { api: "metrics::harness::assert_at_most", kind: ExceptionKind::Nonvisual, reason: "An assertion helper of the work-budget tests, compiled only with the `work-metrics` feature; it checks counts and has no visual form.", regression_test: "src/metrics/tests.rs::harness_assert_at_most_accepts_a_ceiling_and_names_the_metric_over_it", tracking_issue: None },
    CapabilityException { api: "metrics::harness::assert_exact", kind: ExceptionKind::Nonvisual, reason: "An assertion helper of the work-budget tests, compiled only with the `work-metrics` feature; it checks counts and has no visual form.", regression_test: "apps/jackin/tests/regressions/work_budgets.rs::work_contract_jackin_capsule_frame_after_typing", tracking_issue: None },
    CapabilityException { api: "metrics::measure", kind: ExceptionKind::Nonvisual, reason: "Thread-local counter plumbing behind record_work!, compiled only with the `work-metrics` feature; it serves the work-budget tests and no Showcase page can show a counter.", regression_test: "src/metrics/tests.rs::metrics_measure_returns_the_value_and_its_work", tracking_issue: None },
    CapabilityException { api: "metrics::record", kind: ExceptionKind::Nonvisual, reason: "Thread-local counter plumbing behind record_work!, compiled only with the `work-metrics` feature; it serves the work-budget tests and no Showcase page can show a counter.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: None },
    CapabilityException { api: "metrics::report::SCHEMA_VERSION", kind: ExceptionKind::Nonvisual, reason: "The count-only JSON report of a work-budget scenario, compiled only with the `work-metrics` feature; it is written for CI and has no visual form.", regression_test: "tests/regressions/work_budgets.rs::work_report_is_count_only_json_with_every_metric", tracking_issue: None },
    CapabilityException { api: "metrics::report::ScenarioReport", kind: ExceptionKind::Nonvisual, reason: "The count-only JSON report of a work-budget scenario, compiled only with the `work-metrics` feature; it is written for CI and has no visual form.", regression_test: "tests/regressions/work_budgets.rs::work_report_is_count_only_json_with_every_metric", tracking_issue: None },
    CapabilityException { api: "metrics::report::ScenarioReport::to_json", kind: ExceptionKind::Nonvisual, reason: "The count-only JSON report of a work-budget scenario, compiled only with the `work-metrics` feature; it is written for CI and has no visual form.", regression_test: "tests/regressions/work_budgets.rs::work_report_is_count_only_json_with_every_metric", tracking_issue: None },
    CapabilityException { api: "metrics::report::ScenarioReport::write", kind: ExceptionKind::Nonvisual, reason: "The count-only JSON report of a work-budget scenario, compiled only with the `work-metrics` feature; it is written for CI and has no visual form.", regression_test: "tests/regressions/work_budgets.rs::work_report_write_publishes_under_the_performance_directory", tracking_issue: None },
    CapabilityException { api: "metrics::report::ScenarioReport::write_in", kind: ExceptionKind::Nonvisual, reason: "The count-only JSON report of a work-budget scenario, compiled only with the `work-metrics` feature; it is written for CI and has no visual form.", regression_test: "tests/regressions/work_budgets.rs::work_report_concurrent_writers_leave_one_whole_report", tracking_issue: None },
    CapabilityException { api: "metrics::reset", kind: ExceptionKind::Nonvisual, reason: "Thread-local counter plumbing behind record_work!, compiled only with the `work-metrics` feature; it serves the work-budget tests and no Showcase page can show a counter.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_reset", tracking_issue: None },
    CapabilityException { api: "metrics::snapshot", kind: ExceptionKind::Nonvisual, reason: "Thread-local counter plumbing behind record_work!, compiled only with the `work-metrics` feature; it serves the work-budget tests and no Showcase page can show a counter.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: None },
    CapabilityException { api: "metrics::suspended", kind: ExceptionKind::Nonvisual, reason: "A scoped switch that drops record_work! counts inside a reference computation of the work-budget tests; it is compiled only with the `work-metrics` feature and has no visual form.", regression_test: "src/metrics/tests.rs::metrics_suspended_drops_work_and_resumes_after_a_panic", tracking_issue: None },
    CapabilityException { api: "shell::StatusLine::notice_with", kind: ExceptionKind::Nonvisual, reason: "Differs from notice only by a caller-chosen payload type P that the shell never draws; the Showcase shell carries P = (), so the Notice buttons on Status and hints show exactly this behavior and the payload has no visual form.", regression_test: "tests/fixed.rs::a_notice_with_a_payload_waits_for_the_clock_and_yields_to_a_message", tracking_issue: None },
    CapabilityException { api: "shell::StatusLine::notice_with_at", kind: ExceptionKind::Nonvisual, reason: "Differs from notice only by a caller-chosen payload type P that the shell never draws; the Showcase shell carries P = (), so the Notice buttons on Status and hints show exactly this behavior and the payload has no visual form.", regression_test: "tests/fixed.rs::a_notice_with_a_payload_waits_for_the_clock_and_yields_to_a_message", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardDecision::Drop", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy refuses a clipboard write or copies it, and never drops one. session_controller_clipboard_decisions_and_refusal_throttle pins the drop with a 'drop' payload.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: None },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_source() -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(root.join("src/catalog.rs")).expect("catalog source reads")
    }

    #[test]
    fn text_input_coverage_ids_are_catalog_capabilities() {
        let catalog = catalog_source();
        for (id, page) in TEXT_INPUT_COVERAGE {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(!page.is_empty(), "coverage entry names no page: {id}");
            let method = id.rsplit("::").next().expect("a capability id has a name");
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let source =
                std::fs::read_to_string(root.join(format!("src/bin/showcase/pages/{page}.rs")))
                    .unwrap_or_else(|e| panic!("coverage page {page} has no source: {e}"));
            assert!(
                source.contains(&format!(".{method}(")),
                "page {page} never calls {method}: {id}"
            );
        }
    }

    #[test]
    fn grid_edit_coverage_ids_are_catalog_capabilities_used_by_their_page() {
        let catalog = catalog_source();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut seen = std::collections::BTreeSet::new();
        for (id, page) in GRID_EDIT_COVERAGE {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(seen.insert(*id), "duplicate coverage entry: {id}");
            let source =
                std::fs::read_to_string(root.join(format!("src/bin/showcase/pages/{page}.rs")))
                    .unwrap_or_else(|e| panic!("coverage page {page} has no source: {e}"));
            let mut parts = id.rsplit("::");
            let last = parts.next().expect("id has a segment");
            let prev = parts.next().unwrap_or("");
            let needle = if last.starts_with(|c: char| c.is_ascii_lowercase()) {
                format!("{last}(")
            } else if prev.starts_with(|c: char| c.is_ascii_uppercase()) {
                format!("{prev}::{last}")
            } else {
                last.to_owned()
            };
            assert!(
                source.contains(&needle),
                "page {page} never uses {id} (looked for {needle})"
            );
        }
    }

    #[test]
    fn grid_change_apis_are_covered_or_allowlisted_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, reason) in GRID_CHANGE_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "duplicate allowlist entry: {id}");
            assert!(
                !GRID_EDIT_COVERAGE.iter().any(|(covered, _)| covered == id),
                "{id} is both covered and allowlisted"
            );
        }
        let declared =
            |id: &str| seen.contains(id) || GRID_EDIT_COVERAGE.iter().any(|(c, _)| *c == id);
        for line in catalog.lines() {
            let Some(id) = line.split('"').nth(1) else {
                continue;
            };
            let change_type = [
                "GridInstanceId",
                "CellChange",
                "InsertedRow",
                "UpdatedRow",
                "DeletedRow",
                "GridChangeSet",
                "CommittedRow",
                "GridCommitReceipt",
                "GridCommitToken",
                "GridCommitError",
                "GridCommitFailure",
                "ReceiptFault",
                "GridDataError",
            ]
            .iter()
            .any(|t| id.starts_with(&format!("widgets::grid::{t}")));
            if change_type
                || id == "widgets::grid::DataGrid::snapshot_changes"
                || id == "widgets::grid::DataGrid::instance_id"
                || id.starts_with("widgets::grid::DataGrid::begin_submission")
                || id.starts_with("widgets::grid::DataGrid::cancel_submission")
                || id.starts_with("widgets::grid::DataGrid::complete_submission")
                || id.starts_with("widgets::grid::DataGrid::validate_submission")
                || id.starts_with("widgets::grid::DataGrid::submission_pending")
                || id.starts_with("widgets::grid::DataGrid::commit_failure")
            {
                assert!(
                    declared(id),
                    "grid change capability {id} has no coverage or allowlist line"
                );
            }
        }
    }

    #[test]
    fn grid_parser_helpers_are_declared_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, reason) in GRID_PARSER_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "duplicate allowlist entry: {id}");
        }
        for id in [
            "widgets::edit::parsers::json_container",
            "widgets::edit::parsers::canonical_uuid",
            "widgets::edit::parsers::date_or_timestamp",
        ] {
            assert!(seen.contains(id), "{id} is neither covered nor allowlisted");
        }
    }

    #[test]
    fn document_workspace_apis_are_covered_or_allowlisted_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, reason) in DOCUMENT_WORKSPACE_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "duplicate allowlist entry: {id}");
            assert!(
                !DOCUMENT_WORKSPACE_COVERAGE
                    .iter()
                    .any(|(covered, _)| covered == id),
                "{id} is both covered and allowlisted"
            );
        }
        let mut covered = std::collections::BTreeSet::new();
        for (id, page) in DOCUMENT_WORKSPACE_COVERAGE {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert_eq!(*page, "queryworkbench", "{id}");
            assert!(covered.insert(*id), "duplicate coverage entry: {id}");
        }
        for line in catalog.lines() {
            let Some(id) = line.split('"').nth(1) else {
                continue;
            };
            if id.starts_with("widgets::document_workspace::") {
                assert!(
                    seen.contains(id) || covered.contains(id),
                    "document workspace capability {id} has no coverage or allowlist line"
                );
            }
        }
    }

    #[test]
    fn scroll_window_apis_are_covered_or_allowlisted_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, why) in SCROLL_WINDOW_COVERAGE.iter().chain(SCROLL_WINDOW_ALLOWLIST) {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(!why.trim().is_empty(), "no reason: {id}");
            assert!(seen.insert(*id), "duplicate declaration: {id}");
        }
        for line in catalog.lines().filter(|l| {
            l.contains("ui::scroll_view::VisibleWindow")
                || l.contains("ScrollView::render_window")
                || l.contains("ScrollView::last_work")
                || l.contains("ui::scroll_view::ScrollWork")
        }) {
            let id = line.split('"').nth(1).expect("catalog line has an id");
            assert!(
                seen.contains(id),
                "scroll window API lacks a declaration: {id}"
            );
        }
    }

    #[test]
    fn external_message_apis_are_declared() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, owner) in EXTERNAL_COVERAGE {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(!owner.trim().is_empty(), "coverage entry has no page: {id}");
            assert!(seen.insert(*id), "duplicate declaration: {id}");
        }
        for (id, reason) in EXTERNAL_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "duplicate declaration: {id}");
        }
        for id in [
            "runtime::ExternalSender::try_send",
            "runtime::ExternalSendError",
            "runtime::ExternalSendError::Full",
            "runtime::ExternalSendError::Closed",
            "runtime::ExternalPolicy",
            "runtime::ExternalReceiver::with_policy",
            "runtime::ExternalReceiver::policy",
            "runtime::ExternalReceiver::set_policy",
            "runtime::ExternalReceiver::try_recv",
            "runtime::bounded_external_channel",
        ] {
            assert!(seen.contains(id), "external API lacks a declaration: {id}");
        }
    }

    #[test]
    fn runtime_raw_input_apis_are_declared_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, reason) in RUNTIME_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "duplicate allowlist entry: {id}");
        }
        for id in [
            "runtime::RawInputPolicy",
            "runtime::Application::raw_input_policy",
            "runtime::Application::take_render_invalidated",
            "runtime::drain_pending_input_bounded",
        ] {
            assert!(
                seen.contains(id),
                "raw input API lacks a coverage declaration: {id}"
            );
        }
    }

    #[test]
    fn search_detail_api_is_covered_or_allowed_exactly_once() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, pages) in SEARCH_DETAIL_COVERAGE {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(seen.insert(*id), "declared twice: {id}");
            assert!(!pages.is_empty(), "no page uses {id}");
            let mut named = std::collections::BTreeSet::new();
            for page in *pages {
                assert!(
                    SEARCH_DETAIL_PAGES.iter().any(|(name, _)| name == page),
                    "{id} names an unknown page {page}"
                );
                assert!(named.insert(*page), "{id} names {page} twice");
            }
        }
        for (id, reason) in SEARCH_SESSION_ALLOWLIST {
            assert!(
                catalog.contains(&format!("\"{id}\"")),
                "declared id is not a catalog capability: {id}"
            );
            assert!(
                !reason.trim().is_empty(),
                "allowlist entry has no reason: {id}"
            );
            assert!(seen.insert(*id), "declared twice: {id}");
        }
        for line in catalog.lines() {
            let Some(rest) = line.split('"').nth(1) else {
                continue;
            };
            if rest.starts_with("widgets::search_detail::") {
                assert!(
                    seen.contains(rest),
                    "search_detail capability lacks a coverage declaration: {rest}"
                );
            }
        }
    }

    #[test]
    fn search_detail_coverage_ids_are_used_by_their_pages() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for (id, pages) in SEARCH_DETAIL_COVERAGE {
            let mut parts = id.rsplit("::");
            let last = parts.next().expect("id has a segment");
            let prev = parts.next().unwrap_or("");
            let needle = if last.starts_with(|c: char| c.is_ascii_lowercase()) {
                format!("{last}(")
            } else if prev.starts_with(|c: char| c.is_ascii_uppercase()) {
                format!("{prev}::{last}")
            } else {
                last.to_owned()
            };
            for page in *pages {
                let (_, path) = SEARCH_DETAIL_PAGES
                    .iter()
                    .find(|(name, _)| name == page)
                    .expect("page names are checked by the exactly-once test");
                let source = std::fs::read_to_string(root.join(path))
                    .unwrap_or_else(|e| panic!("the {page} page source {path}: {e}"));
                assert!(
                    source.contains(&needle),
                    "the {page} page never uses {id} (looked for {needle})"
                );
            }
        }
    }
}
