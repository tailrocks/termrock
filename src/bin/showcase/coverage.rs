//! What the Showcase proves about the library's declared API (#14).
//!
//! `PROOFS` lists the APIs a callable scenario in `app_tests_catalog.rs`
//! exercises. `EXCEPTIONS` lists APIs with no page demonstration (`Nonvisual`,
//! or `Deferred` with a tracking issue) and the exact test that pins them.
//! `coverage_debt.txt` lists what still owes one of the two; it may only shrink. `src/catalog_contract.rs` checks all three against
//! the exported API. One line per record keeps a restack mechanical.

use crate::app::PageId;

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
    CompletionPopupSnapshot,
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
    ("widgets::search_detail::SearchCmd", &["holla"]),
    ("widgets::search_detail::SearchCmd::Activate", &["holla"]),
    ("widgets::search_detail::SearchCmd::Up", &["holla"]),
    ("widgets::search_detail::SearchCmd::Down", &["holla"]),
    ("widgets::search_detail::SearchCmd::Left", &["holla"]),
    ("widgets::search_detail::SearchCmd::Right", &["holla"]),
    ("widgets::search_detail::SearchCmd::Home", &["holla"]),
    ("widgets::search_detail::SearchCmd::End", &["holla"]),
    ("widgets::search_detail::SearchCmd::PageUp", &["holla"]),
    ("widgets::search_detail::SearchCmd::PageDown", &["holla"]),
    ("widgets::search_detail::SearchFocusMode", &["holla"]),
    (
        "widgets::search_detail::SearchFocusMode::Combined",
        &["holla"],
    ),
    ("widgets::search_detail::SearchConfig::keymap", &["holla"]),
    (
        "widgets::search_detail::SearchConfig::focus_mode",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::type_to_query_from_detail",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchConfig::reset_preview_on_query",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::configure_query",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::render_hosted",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::config_mut",
        &["holla"],
    ),
    ("widgets::search_detail::SearchPaint", &["holla"]),
    ("widgets::search_detail::SearchPaint::query", &["holla"]),
    (
        "widgets::search_detail::SearchPaint::rows_begin",
        &["holla"],
    ),
    ("widgets::search_detail::SearchPaint::row", &["holla"]),
    ("widgets::search_detail::SearchPaint::empty", &["holla"]),
    ("widgets::search_detail::SearchPaint::detail", &["holla"]),
    ("widgets::search_detail::SearchPaint::summary", &["holla"]),
    ("widgets::search_detail::SearchPaint::is_lead", &["holla"]),
    ("widgets::search_detail::QueryView", &["holla"]),
    ("widgets::search_detail::RowPaint", &["holla"]),
    ("widgets::search_detail::RowPaint::place_cell", &["holla"]),
    ("widgets::search_detail::RowPaint::cell_hit", &["holla"]),
    ("widgets::list_detail::DetailPane", &["holla"]),
    (
        "widgets::search_detail::SearchDetail::detail_share",
        &["holla"],
    ),
    (
        "widgets::search_detail::SearchDetail::summary_rows",
        &["holla"],
    ),
];

const SEARCH_PINNED_REASON: &str = "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.";

const SEARCH_HEADLESS_REASON: &str = "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.";

const SEARCH_INTERNAL_REASON: &str = "Reached through the Finder's SearchDetail and its SearchSource without being named in page code: the standard painter and the blanket SearchDetailSource impl use it.";

const HOSTED_PANES_REASON: &str = "SearchDetail::render_hosted and the list-detail hosted layer serve an app that paints every pane itself; apps compose through SearchPaint, so no showcase page calls the layer below it directly. The Holla Finder exercises it end to end.";

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
        "widgets::search_detail::SearchConfig::preserve_preview_on_same_key",
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
    CapabilityException { api: "widgets::picker::Picker::set_max_rows", kind: ExceptionKind::Deferred, reason: "Picker API added after this gate was written: no Showcase page or callable scenario drives the setter yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/picker.rs::zero_row_limit_is_one_row_in_every_status", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::picker::Picker::set_placement", kind: ExceptionKind::Nonvisual, reason: "The showcase configures placement through its builder; no concrete app uses this setter.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_query", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates filtering through terminal input and does not set the query programmatically.", regression_test: "src/widgets/picker.rs::searchable_layer_routes_edit_menu_enter_before_multi_confirm_and_insert", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::set_width", kind: ExceptionKind::Nonvisual, reason: "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.", regression_test: "src/bin/showcase/app_tests_catalog.rs::catalog_picker_setters_and_accessors_behave", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::toggle_binding", kind: ExceptionKind::Nonvisual, reason: "The page demonstrates the supported Desktop and Vim profiles; custom toggle bindings are not a page configuration option.", regression_test: "src/widgets/picker.rs::custom_multi_hints_follow_the_effective_binding_and_capabilities", tracking_issue: None },
    CapabilityException { api: "widgets::picker::Picker::toggle_checked", kind: ExceptionKind::Deferred, reason: "Picker API added after this gate was written: no Showcase page or callable scenario toggles a checked row directly yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/picker.rs::hidden_deleted_and_disabled_choices_are_pruned_and_restoration_does_not_reselect", tracking_issue: Some(306) },
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
    CapabilityException { api: "core::data::DataError", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_empty_schema_is_inert", tracking_issue: Some(301) },
    CapabilityException { api: "core::data::DataError::EmptySchemaWithRows", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_empty_schema_is_inert", tracking_issue: Some(301) },
    CapabilityException { api: "core::data::DataError::MissingRow", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_table_rejects_short_and_long_rows", tracking_issue: Some(301) },
    CapabilityException { api: "core::data::DataError::Shape", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_grid_sparse_rows_normalize_to_null", tracking_issue: Some(301) },
    CapabilityException { api: "core::data::ShapeError", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_edit_lifecycle.rs::issue32_failed_shape_change_keeps_editor", tracking_issue: Some(301) },
    CapabilityException { api: "core::data::validate_rectangular", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_validate_rectangular_accepts_full_width_rows", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedRecord", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::Cycle", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_rejects_cycles_duplicates_and_missing_nodes_atomically", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::DuplicateAuthoritative", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::DuplicateVisible", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::MissingNode", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_rejects_cycles_duplicates_and_missing_nodes_atomically", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::PositionalKey", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedSourceError::UnknownVisible", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::keyed_unchanged_stamp_does_not_enumerate_the_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::adopt_cursor_view", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::adopt_selection_view", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::anchor", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::anchor_view", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::begin_edit", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_edit_cancels_on_changed_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::cancel_edit", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::cancel_press", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::capture_press", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_failed_release_consumes_capture", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::clear_cursor", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::cursor", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_cursor_fallback_is_deterministic", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::cursor_view", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::edit_key", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_edit_cancels_on_changed_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::expanded", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::expanded_revision", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::finish_edit", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::invalidate_layout", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::keyed_resize_cancels_capture_without_source_scan", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::is_authoritative", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::is_enabled", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::keyed_unchanged_stamp_does_not_enumerate_the_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::is_expanded", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::is_selected", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::new", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::pressed", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_press_cannot_retarget_after_reorder", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::reconcile", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::release_press", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_press_cannot_retarget_after_reorder", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::replace_selection", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::selected", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_selection_follows_keys_across_reorder", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::selected_len", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::selected_view", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::set_anchor", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::set_cursor", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_cursor_fallback_is_deterministic", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::set_expanded", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::set_selected", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_filter_preserves_hidden_selection", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::stamp", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::visible_index", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_reconcile_is_atomic_on_invalid_source", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::visible_key", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_state_reports_view_positions_and_ends_edits_and_presses", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedState::visible_len", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::get", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::insert", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::is_empty", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_lists_its_row_keys_and_edits_one_cell_by_key", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::iter", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::key_at", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::len", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::move_to", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::new", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::position", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::push", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::remove", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_cancels_the_edit_when_its_row_is_removed", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::replace", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::row_selection_replace_swaps_the_rows_and_the_anchor_in_one_step", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::replace_if_changed", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_owns_its_revisions", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::stamp", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::try_from_rows", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::update", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::KeyedVec::value_at", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: Some(301) },
    CapabilityException { api: "core::keyed::Reconcile", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_vec_serves_keyed_records_and_a_state_reconciles_them", tracking_issue: Some(301) },
    CapabilityException { api: "core::list::KeyedRowSource", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::list::SourceStamp", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(301) },
    CapabilityException { api: "core::pointer::PointerEvent::key_of", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/picker.rs::remapped_toggle_hint_clicks_deliver_the_bound_key", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::Many", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::None", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::One", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::append", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::is_empty", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::iter", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::len", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Actions::push", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIntoIter", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_into_iter_small_and_many_keep_count_and_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIntoIter::Many", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIntoIter::Small", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_into_iter_small_and_many_keep_count_and_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIter", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_iter_many_yields_every_action_in_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIter::Many", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_report_emptiness_and_grow_one_push_at_a_time", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::ActionsIter::Small", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::actions_into_iter_small_and_many_keep_count_and_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_actions_and_into_actions_report_zero_one_and_many_in_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::has_actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::grid_traversal_does_not_activate_checkbox_or_viewer", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::into_actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_actions_and_into_actions_report_zero_one_and_many_in_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::into_raw_parts", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_layout_survives_nested_action_dispatch", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::map_actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_layout_survives_nested_action_dispatch", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::merge", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_merge_preserves_order_and_flags", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::on_actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_layout_survives_nested_action_dispatch", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::push_action", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_actions_and_into_actions_report_zero_one_and_many_in_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::take_actions", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_actions_and_into_actions_report_zero_one_and_many_in_order", tracking_issue: Some(301) },
    CapabilityException { api: "core::response::Response::try_into_single", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/response_batches.rs::response_single_conversion_preserves_batch_on_error", tracking_issue: Some(301) },
    CapabilityException { api: "core::selection::RowSelection::replace", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::row_selection_replace_swaps_the_rows_and_the_anchor_in_one_step", tracking_issue: Some(301) },
    CapabilityException { api: "core::text::TextBuffer::line_range", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/core/text/tests.rs::text_index_preserves_unicode_and_trailing_line", tracking_issue: Some(301) },
    CapabilityException { api: "core::text::TextBuffer::line_width", kind: ExceptionKind::Deferred, reason: "Code editor text API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/core/text/tests.rs::text_index_preserves_unicode_and_trailing_line", tracking_issue: Some(297) },
    CapabilityException { api: "core::text::TextBuffer::max_line_width", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/core/text/tests.rs::text_index_preserves_unicode_and_trailing_line", tracking_issue: Some(301) },
    CapabilityException { api: "core::text::TextBuffer::position_at", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/core/text/tests.rs::text_index_preserves_unicode_and_trailing_line", tracking_issue: Some(301) },
    CapabilityException { api: "core::text::TextBuffer::revision", kind: ExceptionKind::Deferred, reason: "Keyed collections and responses API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/core/text/tests.rs::text_revision_tracks_content_not_cursor", tracking_issue: Some(301) },
    CapabilityException { api: "metrics::ENABLED", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/functional/cases/j04_accounts.rs::j04_accounts_accounts", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::METRIC_COUNT", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_names_are_unique_and_follow_index_order", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_viewport_first_use_and_prepared_draw", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::ALL", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_names_are_unique_and_follow_index_order", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::AllocatedBufferCells", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/showcase/work_metrics_tests.rs::work_contract_showcase_idle_and_switch", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::AnnotationQueries", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_editor_prepared_frame", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::BufferCellsCopied", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_scroll_view_content_sized_canvas", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::BufferCellsFilled", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::FitCompares", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/showcase/work_metrics_tests.rs::work_contract_stress_chrome_items", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::FitVisits", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/tablepro/work_metrics_tests.rs::work_contract_tablepro_idle_and_cell_edit", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::IdentityComparisons", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_report_is_count_only_json_with_every_metric", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::IndexExtends", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_viewport_one_append_is_incremental", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::IndexRebuilds", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_reset", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::IndexShifts", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_a_counter_that_went_backwards", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::IntrinsicWidthCalls", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_settled_frame_paints_only_visible_rows", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::LabelBytesNormalized", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_tree_filtered_rebuild", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::LabelsNormalized", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_tree_filtered_rebuild", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::PayloadBytesCloned", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_viewport_adapter_unprepared_draw", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::ProjectionRebuilds", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/holla/work_metrics_tests.rs::work_contract_holla_idle_and_source_update", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::RankRowsVisited", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/holla/work_metrics_tests.rs::work_contract_holla_idle_and_source_update", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::ReflowedLines", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_viewport_first_use_and_prepared_draw", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::SchedulingPolls", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "apps/jackin/tests/regressions/work_budgets.rs::work_contract_jackin_idle_and_roster_update", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::SchedulingRecordsVisited", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/holla/work_metrics_tests.rs::work_contract_holla_idle_and_source_update", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::SegmentedLines", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::TextBytesMeasured", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_textarea_prepared_frame", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::VisibleRowsPainted", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/work_budgets.rs::work_contract_settled_frame_paints_only_visible_rows", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::Metric::name", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_names_are_unique_and_follow_index_order", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::MetricsError", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_other_thread", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::MetricsError::CounterWentBackwards", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_a_counter_that_went_backwards", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::MetricsError::DifferentEpoch", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_reset", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::MetricsError::DifferentThread", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_rejects_other_thread", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::MetricsError::Overflow", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_merge_adds_worker_deltas_and_reports_overflow", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_merge_adds_worker_deltas_and_reports_overflow", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::ZERO", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_merge_adds_worker_deltas_and_reports_overflow", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::check_budget", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_check_budget_flags_overruns_and_unlisted_work", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::check_exact", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_check_exact_flags_undercounts_overcounts_and_unlisted_work", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::get", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_merge_adds_worker_deltas_and_reports_overflow", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::merged", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_merge_adds_worker_deltas_and_reports_overflow", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkDelta::nonzero", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkSnapshot", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "metrics::WorkSnapshot::delta", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "record_work!", kind: ExceptionKind::Deferred, reason: "Work metrics API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/metrics/tests.rs::metrics_delta_counts_units_between_snapshots", tracking_issue: Some(308) },
    CapabilityException { api: "runtime::ExternalPolicy", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalPolicy::DEFAULT_MAX_DURATION", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalPolicy::DEFAULT_MAX_MESSAGES", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalReceiver::policy", kind: ExceptionKind::Nonvisual, reason: "A read accessor the runtime uses to copy the bound before each turn; the Activity page shows the bound through the pages it limits, not as a value.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::set_policy", kind: ExceptionKind::Nonvisual, reason: "The Activity page passes its policy to bounded_external_channel; the setter is covered by the bounded_external_policy_ unit test.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::try_recv", kind: ExceptionKind::Nonvisual, reason: "The runtime calls it between handlers; no page reads the queue itself. The runtime unit tests external_turn_ and bounded_external_ pin its order and bound.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalReceiver::with_policy", kind: ExceptionKind::Nonvisual, reason: "The Activity page passes its policy to bounded_external_channel; the builder is covered by the bounded_external_policy_ unit test.", regression_test: "src/runtime.rs::bounded_external_policy_is_receiver_local_and_normalized", tracking_issue: None },
    CapabilityException { api: "runtime::ExternalSendError", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalSendError::Closed", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalSendError::Full", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::ExternalSender::try_send", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::RawInputPolicy", kind: ExceptionKind::Nonvisual, reason: "Raw terminal event budgets act below the widget layer and draw nothing; pinned by the runtime::tests raw_input_ and startup_discard_ unit tests, and the showcase runs on the default policy.", regression_test: "src/runtime.rs::raw_input_empty_and_read_error_are_distinct", tracking_issue: None },
    CapabilityException { api: "runtime::RawInputPolicy::DEFAULT_MAX_DURATION", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::RawInputPolicy::DEFAULT_MAX_EVENTS", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/runtime.rs::default_policies_carry_the_documented_default_budgets", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::bounded_external_channel", kind: ExceptionKind::Deferred, reason: "The Activity page names it (recorded in EXTERNAL_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/runtime.rs::bounded_external_full_and_closed_preserve_payload", tracking_issue: Some(305) },
    CapabilityException { api: "runtime::drain_pending_input_bounded", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/runtime.rs::the_public_bounded_drain_with_a_zero_bound_reads_nothing", tracking_issue: Some(305) },
    CapabilityException { api: "shell::Cx::notice", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/status.rs::a_notice_request_is_allowed_from_a_screen_that_is_not_shown", tracking_issue: Some(306) },
    CapabilityException { api: "shell::Request::Notice", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/status.rs::a_notice_request_is_allowed_from_a_screen_that_is_not_shown", tracking_issue: Some(306) },
    CapabilityException { api: "shell::StatusLine::notice", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/status.rs::a_notice_replaces_an_earlier_notice", tracking_issue: Some(306) },
    CapabilityException { api: "shell::StatusLine::notice_at", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/status.rs::a_dropped_notice_does_not_come_back_when_the_message_expires", tracking_issue: Some(306) },
    CapabilityException { api: "shell::StatusLine::notice_with", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/fixed.rs::a_notice_with_a_payload_waits_for_the_clock_and_yields_to_a_message", tracking_issue: Some(306) },
    CapabilityException { api: "shell::StatusLine::notice_with_at", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/fixed.rs::a_notice_with_a_payload_waits_for_the_clock_and_yields_to_a_message", tracking_issue: Some(306) },
    CapabilityException { api: "terminal::PENDING_EFFECTS", kind: ExceptionKind::Nonvisual, reason: "the cap of 16 queued host effects; the page keeps three on screen, so the cap is not visible here. session_controller_effect_queue_keeps_the_newest_sixteen pins it.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: None },
    CapabilityException { api: "terminal::ReplayTiming", kind: ExceptionKind::Nonvisual, reason: "the three numbers that set the replay window; the page keeps the defaults of SessionConfig::new and sets none of its own. session_controller_replay_deadline_follows_output pins the deadlines they produce.", regression_test: "tests/terminal/session.rs::session_replay_timing_is_the_default_window_a_config_carries", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardContext", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionClipboardDecision", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionClipboardDecision::Copy", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy refuses every clipboard write, so it never returns Copy. session_controller_clipboard_decisions_and_refusal_throttle pins the copy path.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardDecision::Drop", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy refuses every clipboard write and never drops one. session_controller_clipboard_decisions_and_refusal_throttle pins the drop with a 'drop' payload.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardDecision::Refuse", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionClipboardRefusal", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionClipboardRefusal::Read", kind: ExceptionKind::Nonvisual, reason: "the page's scripts only write to the clipboard and never send a read request, so DemoPolicy never reports Read. session_controller_clipboard_decisions_and_refusal_throttle pins the refusal.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardRefusal::TooLarge", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy reports only Write and Read refusals, never TooLarge. session_controller_clipboard_decisions_and_refusal_throttle pins it with a 'big' payload.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionClipboardRefusal::Write", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionCommand", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionCommand::Detach", kind: ExceptionKind::Nonvisual, reason: "the controller never sends Detach. apps/jackin maps it to the pane's own command, and no page control or test sends it.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: None },
    CapabilityException { api: "terminal::SessionCommand::Input", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionCommand::Redraw", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_redraw_and_clear", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionCommand::Reply", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionConfig", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_asks_the_link_for_its_configured_drain_budget", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionConfig::MINIMUM", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionConfig::new", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionDetach", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionDetach::Operator", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionDetach::TransportLost", kind: ExceptionKind::Nonvisual, reason: "no page script ends the stream after the attach, so the page cannot reach this status. session_controller_resize_retry_restart_transcript pins it.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: None },
    CapabilityException { api: "terminal::SessionEffect", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEffect::ClipboardRefused", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEffect::Copy", kind: ExceptionKind::Nonvisual, reason: "the page's DemoPolicy refuses every clipboard write, so the controller never emits Copy. session_controller_clipboard_decisions_and_refusal_throttle pins the effect.", regression_test: "tests/terminal/session.rs::session_controller_clipboard_decisions_and_refusal_throttle", tracking_issue: None },
    CapabilityException { api: "terminal::SessionEffect::Detached", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEffect::Failed", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEffect::Notice", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEnd", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEnd::Detached", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEnd::Failed", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: the page's fails script fails inside SessionSource::open and never sends this event, so its demo is tracked in the area issue and the named test pins the event meanwhile.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEnd::Refused", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no page script sends a refusal, so its demo is tracked in the area issue and the named test pins the event meanwhile.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEnd::StreamClosed", kind: ExceptionKind::Nonvisual, reason: "no page script closes the stream. session_controller_resize_retry_restart_transcript pins a close before the attach and a close after it.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: None },
    CapabilityException { api: "terminal::SessionEvent", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Attached", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Ended", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Exited", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Lease", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Notice", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionEvent::Output", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionFault", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionHostPolicy", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_host_policy_decides_each_clipboard_request_from_its_context", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionKey", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionKey::Consumed", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionKey::Forwarded", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionLease", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_deadline_follows_output", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionLease::Owner", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_deadline_follows_output", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionLease::ReadOnly", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionLink", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionPaste", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionPaste::Dropped", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionPaste::Full", kind: ExceptionKind::Nonvisual, reason: "the page's scripted link always queues, so a paste never meets a full queue. session_controller_reply_and_paste_limits pins it.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "terminal::SessionPaste::NeedsConfirm", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionPaste::Sent", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionPaste::TooLarge", kind: ExceptionKind::Nonvisual, reason: "the page pastes only a one-line and a three-line text, both under SessionConfig::paste_max_bytes. session_controller_reply_and_paste_limits pins the refusal with a nine-byte paste against a limit of eight.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "terminal::SessionSent", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionSent::Closed", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: the page's scripted link always queues and never reports a closed queue, so its demo is tracked in the area issue and the named test pins the link meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionSent::Full", kind: ExceptionKind::Nonvisual, reason: "the page's scripted link always queues. session_controller_reply_and_paste_limits pins a full queue.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: None },
    CapabilityException { api: "terminal::SessionSent::Queued", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionSize", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionSource", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_source_opens_a_link_that_reports_what_it_did_with_each_command", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Attached", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_deadline_follows_output", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Attaching", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Detached", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_failed_attach_can_be_refused_by_the_other_side", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Exited", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Failed", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::SessionStatus::Pending", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_asks_the_link_for_its_configured_drain_budget", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::attention", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_suppresses_side_effects", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::clear", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_redraw_and_clear", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::close_needs_confirm", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_without_a_link_has_nothing_to_pump", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::dropped_replies", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::focus_changed", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_focus_reports_ignore_the_replay_window", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::gesture", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::has_output", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::key", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::live", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::new", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_effect_queue_keeps_the_newest_sixteen", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::next_replay_deadline", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_replay_deadline_follows_output", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::paste", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::paste_confirmed", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_reply_and_paste_limits", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::pump", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_without_a_link_has_nothing_to_pump", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::redraw", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/bin/showcase/pages/terminal_session.rs::redraw_is_sent_to_a_live_link_and_not_without_one", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::set_config", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_asks_the_link_for_its_configured_drain_budget", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::set_host_policy", kind: ExceptionKind::Deferred, reason: "Terminal session API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/terminal/session.rs::session_controller_set_host_policy_replaces_the_sanitiser_for_later_events", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::status", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_fault_from_a_refused_open_reaches_status_and_effect", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::sync_size", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::take_effects", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::title", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::too_small", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_resize_retry_restart_transcript", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::viewport", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_viewport_handle_cannot_leave_the_grid_off_the_forwarded_size", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::viewport_mut", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_viewport_handle_cannot_leave_the_grid_off_the_forwarded_size", tracking_issue: Some(307) },
    CapabilityException { api: "terminal::TerminalSessionController::wheel", kind: ExceptionKind::Deferred, reason: "The terminal session page names it (recorded in TERMINAL_SESSION_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/terminal/session.rs::session_controller_readonly_input_and_owner_automation", tracking_issue: Some(307) },
    CapabilityException { api: "ui::modal::FocusOnOpen", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/shell.rs::push_with_focus_policy_separates_open_from_close", tracking_issue: Some(305) },
    CapabilityException { api: "ui::modal::FocusOnOpen::Preserve", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/shell.rs::push_with_focus_policy_separates_open_from_close", tracking_issue: Some(305) },
    CapabilityException { api: "ui::modal::FocusOnOpen::Set", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/shell.rs::push_with_focus_policy_separates_open_from_close", tracking_issue: Some(305) },
    CapabilityException { api: "ui::modal::ModalStack::push_with_focus_policy", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/shell.rs::push_with_focus_policy_separates_open_from_close", tracking_issue: Some(305) },
    CapabilityException { api: "ui::modal::ModalStack::settle", kind: ExceptionKind::Deferred, reason: "Runtime, scroll view and modal stack API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/ui/layer.rs::settle_drains_every_queued_result_in_order", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::ScrollView::last_work", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_scratch_cells_ignore_content_height", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::ScrollView::render_window", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_nested_popup_preserves_root_screen", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::ScrollWork", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_work_starts_at_zero_and_reports_the_latest_frames_scratch_cells", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::VisibleWindow", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::content_rows", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::project_content_rect", kind: ExceptionKind::Deferred, reason: "The Scrolling page names it (recorded in SCROLL_WINDOW_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: Some(305) },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::project_point", kind: ExceptionKind::Nonvisual, reason: "The Virtual rows demo has no point-anchored overlay; cursor and caret hosts project a point from the same window.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: None },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::project_rect", kind: ExceptionKind::Nonvisual, reason: "The Virtual rows demo projects model rows with project_content_rect, which wraps it; a padded-canvas rectangle is only needed by hosts that position a popup anchor.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_projection_helpers_map_at_nonzero_origins", tracking_issue: None },
    CapabilityException { api: "ui::scroll_view::VisibleWindow::rows", kind: ExceptionKind::Nonvisual, reason: "The demo iterates content_rows, which is rows without the padding; rows is the padded-canvas range for painting padding decorations.", regression_test: "src/ui/scroll_view/window_tests.rs::scroll_window_padding_maps_source_row_zero", tracking_issue: None },
    CapabilityException { api: "widgets::chips::ChipBar::add_label", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/chip_add_hotkey.rs::plus_adds_when_the_bar_has_an_add_action", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::chips", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::collection_checked_inserts_and_removes_reject_out_of_range", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::insert_chip", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::collection_checked_inserts_and_removes_reject_out_of_range", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::lead", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/chip_add_hotkey.rs::the_lead_label_is_read_back_and_set_lead_reports_a_change", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::remove_chip", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::collection_checked_inserts_and_removes_reject_out_of_range", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::set_add_label", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::chip_add_label_change_invalidates_old_add_hit", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::set_chips", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::chip_cursor_clamps_to_current_stops_on_change", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::set_lead", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::chip_equal_updates_preserve_menu", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::chips::ChipBar::update_chip", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::chip_context_remove_rechecks_current_eligibility", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::choice_dialog::ChoiceDialog::dismiss", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/ui/layer.rs::dismiss_resolves_to_cancelled_over_queued_results", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::code::CodeEditor::diagnostics", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_diagnostics_reads_back_the_diagnostics_in_the_order_they_were_set", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::find_state", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_find_scans_once_per_text_and_query", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::highlighter_fn", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::highlighter_fn_returns_the_installed_provider", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::invalidate_highlighter", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::invalidate_highlighter_makes_the_next_paint_recompute_unchanged_text", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::invalidate_segmenter", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_shared_block_cache_before_render_and_replacement", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::push_diagnostic", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_edit_rebuilds_each_projection_once", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::segmenter_fn", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_shared_block_cache_before_render_and_replacement", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::set_diagnostics", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_diagnostics_merge_exact_and_range_ordinals", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::set_find_current", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_find_scans_once_per_text_and_query", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::set_find_query", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_find_scans_once_per_text_and_query", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::set_highlighter", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_provider_change_invalidates_unchanged_text", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::code::CodeEditor::set_segmenter", kind: ExceptionKind::Deferred, reason: "Code editor API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/code/tests.rs::editor_shared_block_cache_before_render_and_replacement", tracking_issue: Some(297) },
    CapabilityException { api: "widgets::document_workspace::CloseDecision", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::CloseDecision::Cancel", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_cancel_during_save_cancels_the_save", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::CloseDecision::Discard", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::CloseToken", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::activate", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_switch_preserves_editor_history", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::active_key", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/bin/showcase/pages/query_workbench.rs::issue52_close_inactive_preserves_active_key", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::begin_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_paint_and_navigation_preserve_close_token", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::begin_operation", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::cancel_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's connection reset and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_cancel_close_save_is_targeted", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::cancel_operation", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_completion_uses_originating_token", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::complete_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_invalid_preflight_never_invokes_apply_saved", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::contains", kind: ExceptionKind::Nonvisual, reason: "The page finds documents by title through iter, so the membership predicate has no call site; the conformance suite asserts it.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::end_operation", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_end_operation_checks_before_it_applies", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::fail_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::finish_operation", kind: ExceptionKind::Nonvisual, reason: "Only an operation whose result changes content ends here, such as a TablePro grid commit; Showcase runs fill a result pane and end through end_operation. tests/regressions/document_workspace.rs asserts it.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::get", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::id", kind: ExceptionKind::Nonvisual, reason: "The page keeps its own widget id constant and never asks the controller for it.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::index_of", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::is_empty", kind: ExceptionKind::Nonvisual, reason: "The page counts tabs with len, so the emptiness predicate has no call site.", regression_test: "tests/regressions/document_workspace.rs::workspace_first_update_that_dirties_a_preview_promotes_once", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::iter", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_translates_events_by_key", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::key_at", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_translates_events_by_key", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::len", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_closing_an_inactive_document_keeps_the_active_one", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::new", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::open", kind: ExceptionKind::Nonvisual, reason: "The page opens through open_with because each document builds its ids from its own key.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_pinned_activates_and_leaves_others_alone", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::open_with", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::ordered_keys", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_identity_survives_reorder_and_close", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::pending_close", kind: ExceptionKind::Nonvisual, reason: "The page keeps the token it was handed and never asks for the stored one; TablePro's save-then-close and the conformance suite do.", regression_test: "tests/regressions/document_workspace.rs::workspace_resolve_cancel_clears_only_that_request", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::preview_key", kind: ExceptionKind::Nonvisual, reason: "The page opens and promotes previews through the controller's actions and never reads the slot; TablePro's workbench and the conformance suite do.", regression_test: "tests/regressions/document_workspace.rs::workspace_preview_promotes_before_replacement", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::promote", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_promote_pins_only_the_preview_document", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::reorder", kind: ExceptionKind::Nonvisual, reason: "Tabs emits no move gesture, so no page can drive a reorder; the conformance suite and the example do.", regression_test: "tests/regressions/document_workspace.rs::workspace_identity_survives_reorder_and_close", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::request_close", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_close_picks_the_survivor_at_the_old_position_then_the_previous", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::resolve_close", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::revision", kind: ExceptionKind::Nonvisual, reason: "The page never captures a content revision; the conformance suite asserts that view work leaves it unchanged.", regression_test: "tests/regressions/document_workspace.rs::workspace_operation_progress_keeps_the_close_token_valid", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::running", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_running_reports_the_operation_in_flight_for_its_document_only", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::save_locked", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved, so no page holds a close-save lock; TablePro's key and pointer paths read it.", regression_test: "tests/regressions/document_workspace.rs::workspace_cancel_close_save_is_targeted", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::scoped_id", kind: ExceptionKind::Nonvisual, reason: "The page derives its editor and result ids with its own key-scoped helpers; TablePro's tab ids and the conformance suite use the controller's.", regression_test: "tests/regressions/document_workspace.rs::workspace_scoped_ids_follow_the_key_not_the_position", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::sync_tabs", kind: ExceptionKind::Nonvisual, reason: "The page decorates its tab items, so it calls sync_tabs_with; the conformance suite and the example call the plain projection.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_translates_events_by_key", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::sync_tabs_with", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_marks_dirty_and_busy", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::update", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_first_update_that_dirties_a_preview_promotes_once", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::validate_close_save", kind: ExceptionKind::Nonvisual, reason: "Showcase documents are never saved; TablePro's table close and the conformance suite drive the save protocol.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::DocumentWorkspace::view_mut", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_switch_preserves_editor_history", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::OpenKind", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_preview_promotes_before_replacement", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::OpenKind::Pinned", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_pinned_activates_and_leaves_others_alone", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::OpenKind::Preview", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_preview_promotes_before_replacement", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::OperationToken", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction::Activated", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_translates_events_by_key", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction::CancelOperation", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction::CloseRequested", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction::Closed", kind: ExceptionKind::Deferred, reason: "The Query workbench page names it (recorded in DOCUMENT_WORKSPACE_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceAction::PreviewPromoted", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceDocument", kind: ExceptionKind::Deferred, reason: "Document workspace API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_open_with_keys_the_document_and_reports_every_transition_as_an_action", tracking_issue: Some(299) },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text and never matches a variant, so each variant is asserted by tests/regressions/document_workspace.rs instead.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::MissingDocument", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_completion_uses_originating_token", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::SaveInProgress", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_save_lock_allows_view_but_rejects_mutation", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StaleClose", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_stale_close_never_targets_new_document", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StaleOperation", kind: ExceptionKind::Nonvisual, reason: "The page reports a refusal through its Display text; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_superseded_operation_is_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::document_workspace::WorkspaceError::StillDirty", kind: ExceptionKind::Nonvisual, reason: "Only the save protocol returns it, and Showcase documents are never saved; tests/regressions/document_workspace.rs asserts this variant.", regression_test: "tests/regressions/document_workspace.rs::workspace_failed_save_keeps_document_and_intent", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::canonical_uuid", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its Id columns; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::date_or_timestamp", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its renewed_at (Timestamp) column; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::edit::parsers::json_container", kind: ExceptionKind::Nonvisual, reason: "The grid page demonstrates this grammar through default_validator on its notes (Json) column; no page calls the helper directly.", regression_test: "tests/regressions/grid_parsers.rs::issue47_public_helpers_validate_the_exact_text", tracking_issue: None },
    CapabilityException { api: "widgets::form_dialog::FormDialog::dismiss", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/ui/layer.rs::dismiss_resolves_to_cancelled_over_queued_results", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::grid::CellChange", kind: ExceptionKind::Nonvisual, reason: "The page reads a changed cell through its accessors and never names the type.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_semantic_noop_overrides_produce_no_update", tracking_issue: None },
    CapabilityException { api: "widgets::grid::CellChange::after", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::CellChange::before", kind: ExceptionKind::Nonvisual, reason: "The UPDATE preview matches on the row's base values from UpdatedRow::before, so the per-cell before value is not shown.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: None },
    CapabilityException { api: "widgets::grid::CellChange::column", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::CellEditResult", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::CellEditResult::Changed", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::CellEditResult::Unchanged", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::CommittedRow", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::begin_submission", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_begin_submission_checks_the_capture", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::cancel_submission", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::columns", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::commit_failure", kind: ExceptionKind::Nonvisual, reason: "The Grid page marks the failed row; the grid-level message for a failure that names no row is asserted in src/widgets/grid_submission_tests.rs.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_backend_failure_unlocks_and_keeps_the_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::complete_submission", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::finish_edit_session", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::instance_id", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DataGrid::pending", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_sort_tests.rs::issue46_append_sorts_by_saved_values_not_pending", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::revision", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_key_revision_lifecycle", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::row_key", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::snapshot_changes", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::source_index", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_remove_inserted_before_edited_row", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::submission_pending", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_append_rows", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_sort_tests.rs::issue46_append_preserves_active_local_sort", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_edit_text", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_try_edit_text_is_the_same_gateway", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_set_model", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_submission_tests.rs::issue278_try_set_model_refuses_while_locked", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_set_model_keyed", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_set_rows", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/grid_validation.rs::issue43_null_intent_is_not_custom_text", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::try_set_rows_keyed", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::update_column", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_table_update_column_is_checked", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DataGrid::validate_submission_receipt", kind: ExceptionKind::Nonvisual, reason: "Used by a host that lets an external store mutate before it completes; the Grid page has no store, and complete_submission runs the same check, which src/widgets/grid_submission_tests.rs asserts.", regression_test: "src/bin/tablepro/store_tests.rs::issue51_receipt_matches_the_grid_it_answers", tracking_issue: None },
    CapabilityException { api: "widgets::grid::DeletedRow", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DeletedRow::before", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::DeletedRow::key", kind: ExceptionKind::Nonvisual, reason: "The DELETE preview matches on the base row from DeletedRow::before, so the row key is not shown.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_receipt_records_the_snapshot_it_answers", tracking_issue: None },
    CapabilityException { api: "widgets::grid::FinishEdit", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::FinishEdit::Accepted", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::FinishEdit::NoSession", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridAction::Undone", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/showcase/pages/grid.rs::undo_status_names_the_row_by_its_key", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet::counts", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet::deleted", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet::identity_receipt", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet::inserted", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_duplicate_snapshot_contains_complete_row", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridChangeSet::is_empty", kind: ExceptionKind::Nonvisual, reason: "The Showcase page keeps its own pending-count guard before it opens the dialog, and the dialog already shows the zero counts.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::origin", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::revision", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/bin/showcase/pages/grid.rs::issue12_showcase_reload_asks_before_dropping_a_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::same_capture", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_same_capture_is_bit_exact_where_eq_is_not", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridChangeSet::updated", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_save_finishes_live_editor_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridCommitError", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Busy", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Editing", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_save_finishes_live_editor_once", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Empty", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_sort_tests.rs::issue46_empty_commit_and_stale_sort_column_are_safe", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::Exhausted", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_submission_tests.rs::a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridCommitError::InvalidReceipt", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::StaleRevision", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_begin_submission_checks_the_capture", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::StaleToken", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_completed_token_is_stale", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitError::WrongOrigin", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitFailure", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_backend_failure_unlocks_and_keeps_the_draft", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridCommitReceipt", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::new", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::origin", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_clone_gets_a_fresh_origin_with_the_same_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitReceipt::revision", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitToken", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridCommitToken::origin", kind: ExceptionKind::Nonvisual, reason: "A host that holds a token may compare its grid; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_cross_grid_snapshot_token_and_clone_rejected", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridCommitToken::revision", kind: ExceptionKind::Nonvisual, reason: "A host that holds a token may compare its capture revision; the page keeps the token private and the tests in src/widgets/grid_submission_tests.rs read it.", regression_test: "src/bin/showcase/pages/grid.rs::issue12_showcase_reload_asks_before_dropping_a_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Busy", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::ColumnWidth", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "tests/regressions/data_shape.rs::issue42_grid_model_rejects_unusable_width_ranges", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Key", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridDataError::Shape", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the refusal through Display in its status line and never branches on the variant; src/widgets/grid_undo_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "tests/regressions/data_shape.rs::issue42_empty_schema_is_inert", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridEditError", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridEditError::Busy", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_review_locks_exact_draft", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridEditError::InvalidTarget", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_try_edit_text_is_the_same_gateway", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridEditError::NoChoice", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridEditError::Validation", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/grid_validation.rs::issue43_finish_session_resolves_text_choice_once", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridInstanceId", kind: ExceptionKind::Nonvisual, reason: "Admission and acknowledgement plumbing. The Grid page's save drives begin_submission and complete_submission, which use it internally, and never names it; src/widgets/grid_changeset_tests.rs and src/widgets/grid_submission_tests.rs assert it.", regression_test: "src/widgets/grid_submission_tests.rs::a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock", tracking_issue: None },
    CapabilityException { api: "widgets::grid::GridKeyError", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridKeyError::DuplicateKey", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridKeyError::KeyCount", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_keyed_setters_validate_before_install", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridRevision", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_key_revision_lifecycle", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndo", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_remove_inserted_before_edited_row", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndoKind", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_delete_undelete_restores_staged_cells", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndoKind::Cell", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/showcase/pages/grid.rs::undo_status_names_the_row_by_its_key", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndoKind::DeleteMark", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_delete_undelete_restores_staged_cells", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndoKind::Insert", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::GridUndoKind::RemovedInsert", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_bulk_delete_of_marked_rows_follows_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::InsertedRow", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::InsertedRow::key", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_receipt_records_the_snapshot_it_answers", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::InsertedRow::values", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_duplicate_override_and_dirty_source", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::PendingChanges::cells", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::PendingChanges::deleted", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::PendingChanges::inserted", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::PendingChanges::is_deleted", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_undo_tests.rs::issue33_bulk_delete_of_marked_rows_follows_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::PendingChanges::is_inserted", kind: ExceptionKind::Deferred, reason: "Data grid API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::ReceiptFault", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Duplicate", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::KeyCollision", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Missing", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Unexpected", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::UpdatedKeyChanged", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::ReceiptFault::Width", kind: ExceptionKind::Nonvisual, reason: "The Grid page prints the error through Display in its status line and never branches on the variant; src/widgets/grid_submission_tests.rs asserts each one.", regression_test: "src/widgets/grid_submission_tests.rs::issue11_receipt_validation_is_atomic", tracking_issue: None },
    CapabilityException { api: "widgets::grid::UpdatedRow", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::a_snapshot_lists_typed_rows_and_the_pending_draft_names_the_same_keys", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::UpdatedRow::before", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_deleted_draft_never_emits_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::UpdatedRow::cells", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_semantic_noop_overrides_produce_no_update", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::grid::UpdatedRow::key", kind: ExceptionKind::Deferred, reason: "The Grid page names it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_changeset_tests.rs::issue50_receipt_records_the_snapshot_it_answers", tracking_issue: Some(298) },
    CapabilityException { api: "widgets::info_dialog::InfoDialog::dismiss", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/info_dialog.rs::form_clean_always_on_edit_escape_respects_policy", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::input::TextInput::set_value", kind: ExceptionKind::Deferred, reason: "The Selectable page names it (recorded in TEXT_INPUT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "tests/regressions/form_value.rs::form_value_set_value_builder_and_setter_share_one_path", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::list::ListAction", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_reports_activation_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListAction::Activated", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_reports_activation_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListAction::SelectionChanged", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_selection_follows_keys_across_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListBox::keyed", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_selection_follows_keys_across_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListBox::on_key_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_owned_and_borrowed_paths_conform", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListBox::on_pointer_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_press_cannot_retarget_after_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListBox::render_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::data_table_and_list_box_paint_borrowed_rows_through_render_source", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListItem::borrowed", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListItemRef", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list::ListSource", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::list_detail::DetailPane", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::bind_source", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::bind_source_adopts_the_moved_cursor_without_painting", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::click_source", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_detail_click_reaches_the_clicked_key", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::drawer_open", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::keyed", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_selection_follows_keys_across_reorder", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::master_offset", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::master_page", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::on_detail_key", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::detail_keys_wheel_and_reset_drive_the_hosted_detail_scroll", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::render_hosted", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::render_source", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::list_detail_unchanged_widths_are_cached", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::render_source_selected_with", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::list_detail_source_rows_are_painted_by_a_caller_closure_without_touching_the_list", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::reset_detail", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::detail_keys_wheel_and_reset_drive_the_hosted_detail_scroll", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::selected_key", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_detail_selection_follows_the_key_across_reorder", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetail::wheel_event", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::detail_keys_wheel_and_reset_drive_the_hosted_detail_scroll", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListDetailSource", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::list_detail_source_rows_are_painted_by_a_caller_closure_without_touching_the_list", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListRow::borrowed", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::ListRowRef", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::list_detail_source_rows_are_painted_by_a_caller_closure_without_touching_the_list", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::MasterRow", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::list_detail_source_rows_are_painted_by_a_caller_closure_without_touching_the_list", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::PaneHost", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::list_detail::SourceFrame", kind: ExceptionKind::Deferred, reason: "List-detail API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::list_detail_unchanged_widths_are_cached", tracking_issue: Some(302) },
    CapabilityException { api: "widgets::nav_list::NavList::set_items", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/nav_list_state.rs::navlist_set_items_repairs_indices_and_ignores_equal_replacement", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::option_editor::ChoiceEditor::options", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/keyed_cost_tests.rs::overlay_snapshot_choice_editor_copies_visible_rows_only", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::panel::ScrollPanel::clear", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue39_clear_and_zero_width_rebuild", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::panel::ScrollPanel::extend_lines", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue39_same_width_replace_updates_wrapping", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::panel::ScrollPanel::lines", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue45_copy_preserves_source_whitespace", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::panel::ScrollPanel::replace_line", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue39_replace_line_rejects_a_missing_index", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::panel::ScrollPanel::set_lines", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue39_shrink_clamps_follow_and_scroll", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::panel::ScrollPanel::source_selection", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/panel/tests.rs::issue45_selection_survives_resize_and_append", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::search_detail::MissingSelection", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_missing_selection_policy", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::MissingSelection::FirstEnabled", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_missing_selection_policy", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::MissingSelection::NearestPosition", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_missing_selection_policy", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryPasteMode", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryPasteMode::JoinLinesWithSpaceTrim", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryPasteMode::StripLineBreaks", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_config_mut_changes_policy_in_place", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryRequest", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryToken", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryToken::result_stamp", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_pending_renders_only_last_accepted_source", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::QueryView", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_unsubmitted_source_is_refused_before_any_change", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::ResultRowKind", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_source_row_kinds_cover_headings_and_spacers", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::ResultRowKind::Cells", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_cells_navigate_horizontally_without_wrapping", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::ResultRowKind::Heading", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_source_row_kinds_cover_headings_and_spacers", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::ResultRowKind::Item", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::ResultRowKind::Spacer", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_source_row_kinds_cover_headings_and_spacers", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::RowPaint", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_unsubmitted_source_is_refused_before_any_change", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::RowPaint::cell_hit", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::row_paint_places_cells_and_names_their_hit_regions", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::RowPaint::place_cell", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::row_paint_places_cells_and_names_their_hit_regions", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_click_policy_is_per_cell", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::Activate", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::ContextRequested", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_context_anchors_to_the_painted_key", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::DetailAction", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_detail_action_captures_selected_key", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::DetailItem", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_detail_item_and_cancel_press_and_host_owned_commands", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::QueryCommand", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_command_reports_effectiveness", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::QueryRequested", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_command_reports_effectiveness", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchAction::SelectionChanged", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_click_policy_decides_activation_by_cell_kind", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Activate", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::CloseDetail", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_detail_item_and_cancel_press_and_host_owned_commands", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Context", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_cannot_activate_stale_result", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Down", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::End", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::FocusQuery", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::FocusResults", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Home", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Left", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::OpenDetail", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchCmd::PageDown", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::PageUp", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Right", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_cells_navigate_horizontally_without_wrapping", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::Up", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_keymap_is_overridable", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchCmd::is_host_owned", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_detail_item_and_cancel_press_and_host_owned_commands", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_paste_respects_detail_focus", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::click", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_click_policy_is_per_cell", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::context", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_secondary_click_selects_then_requests_through_context_config", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchConfig::focus_mode", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::keymap", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_keymap_is_overridable", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::missing_selection", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_missing_selection_policy", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::navigate_while_loading", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_loading_navigation_is_configurable", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::paste_mode", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::preserve_preview_on_same_key", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_config_mut_changes_policy_in_place", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::reset_preview_on_query", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_preview_reset_policy_follows_the_query", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::select_first_on_query", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_query_change_selects_first", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchConfig::type_to_query_from_detail", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_typing_returns_from_the_preview_only_when_configured", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::close_detail", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_retry_after_a_failure_and_close_detail_return_focus", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::collapse_below", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_layout_builders_place_the_preview_and_size_the_summary", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::config", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::config_mut", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_editing_blur_readout_and_the_session_it_exposes", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::configure_query", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_editing_blur_readout_and_the_session_it_exposes", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::detail_id", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_hosted_wheel_scrolls_the_list_and_the_preview", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::detail_share", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_layout_builders_place_the_preview_and_size_the_summary", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::editing", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_editing_blur_readout_and_the_session_it_exposes", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::emit_pending_context", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_context_anchors_to_the_painted_key", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::fail", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_loading_paints_stale_and_cannot_act", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::id", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::row_paint_places_cells_and_names_their_hit_regions", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::new", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::on_blur", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_editing_blur_readout_and_the_session_it_exposes", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::on_detail", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_narrow_drawer_restores_focus", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::on_key", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::on_paste", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_typing_returns_from_the_preview_only_when_configured", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::open_detail", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchDetail::phase", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::pointer", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_cancelled_press_does_not_activate_later", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::query", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_combined_query_hook_and_field_hit", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::refresh_source", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::render_hosted", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_layout_builders_place_the_preview_and_size_the_summary", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::render_source", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_standard_paint_draws_rows_and_registers_hits", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::render_source_with", kind: ExceptionKind::Nonvisual, reason: "Neither the Showcase Finder nor the Holla Finder calls it by name: they run on the defaults or on other API here; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin it.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_unsubmitted_source_is_refused_before_any_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchDetail::retry", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_retry_after_a_failure_and_close_detail_return_focus", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::selected_key", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::review283_same_key_selection_paths_keep_the_preview_scroll", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::session", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_retry_after_a_failure_and_close_detail_return_focus", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::set_query", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::set_query_readout", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_editing_blur_readout_and_the_session_it_exposes", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::submit", kind: ExceptionKind::Deferred, reason: "The finder and holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "src/widgets/search_detail/cost_tests.rs::search_detail_noop_source_does_not_rebuild", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetail::summary_rows", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_layout_builders_place_the_preview_and_size_the_summary", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchDetailSource", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::row_paint_places_cells_and_names_their_hit_regions", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchError", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::AlreadyCompleted", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::InconsistentRow", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_item_key_alias_validates_once", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::InvalidSource", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::StaleQuery", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_rejects_stale_submission", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::StampMismatch", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchError::UnsubmittedProjection", kind: ExceptionKind::Nonvisual, reason: "The Finder keeps the default key map and treats a refused call as ignored input; tests/regressions/search_detail.rs and tests/regressions/search_detail_view.rs pin every variant.", regression_test: "tests/regressions/search_detail.rs::search_detail_pending_renders_only_last_accepted_source", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchFocusMode", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchFocusMode::Combined", kind: ExceptionKind::Deferred, reason: "The holla page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchFocusMode::Separate", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_config_mut_changes_policy_in_place", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPaint", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::search_detail_layout_builders_place_the_preview_and_size_the_summary", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPhase", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPhase::Failed", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_failed_requires_retry", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPhase::Loading", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPhase::Ready", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchPhase::Uninitialized", kind: ExceptionKind::Nonvisual, reason: "Reached through the Finder's SearchDetail and its SearchSource without being named in page code: the standard painter and the blanket SearchDetailSource impl use it.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::accepted_query", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::accepted_stamp", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_rejects_stale_submission", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::begin_context", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::cancel_press", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_detail_item_and_cancel_press_and_host_owned_commands", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::click", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_click_policy_is_per_cell", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::command_for", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::config", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_separate_focus_gives_text_first_refusal", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::config_mut", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_config_mut_changes_policy_in_place", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::configure_query", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_configure_query_requests_on_text_change", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::context_intent", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::detail_action", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_detail_action_captures_selected_key", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::detail_item", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_detail_item_and_cancel_press_and_host_owned_commands", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::emit_context", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_context_captures_key_and_anchor", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::error", kind: ExceptionKind::Deferred, reason: "The finder page names it (recorded in SEARCH_DETAIL_COVERAGE), but no callable scenario drives it yet; the test below pins its behavior.", regression_test: "tests/regressions/search_detail.rs::search_detail_failed_requires_retry", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::fail", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_failed_requires_retry", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::invalidate_layout", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_layout_change_cancels_press", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::is_interactive", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::is_selectable", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::SearchSession::new", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_invalid_source_leaves_request_pending", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::on_key", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_skips_headings_and_disabled_cells", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::on_paste", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_paste_respects_detail_focus", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::phase", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_initial_source_and_terminal_completion", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::press", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_layout_change_cancels_press", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::query", kind: ExceptionKind::Nonvisual, reason: "SearchSession is the headless half and runs inside SearchDetail, which the Finder page drives; no page uses the session on its own.", regression_test: "tests/regressions/search_detail.rs::search_detail_combined_focus_preserves_holla_query_routing", tracking_issue: None },
    CapabilityException { api: "widgets::search_detail::SearchSession::query_input", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_session_exposes_its_query_acceptance_and_selectable_keys", tracking_issue: Some(300) },
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
    CapabilityException { api: "widgets::search_detail::SearchSource", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail.rs::search_source_row_kinds_cover_headings_and_spacers", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::search_detail::paint_query_default", kind: ExceptionKind::Deferred, reason: "Finder (SearchDetail) API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/search_detail_view.rs::row_paint_places_cells_and_names_their_hit_regions", tracking_issue: Some(300) },
    CapabilityException { api: "widgets::splitter::Splitter::cancel_drag", kind: ExceptionKind::Deferred, reason: "Shell, dialogs and input widgets API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/splitter.rs::splitter_cancel_drag_is_idempotent", tracking_issue: Some(306) },
    CapabilityException { api: "widgets::table::Cell::borrowed", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_rejects_invalid_sources_without_touching_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::CellRef", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::columns", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/deferred_api_calls.rs::data_table_columns_returns_the_columns_in_construction_order", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::keyed", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_selection_follows_keys_across_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::on_key_edit_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_commits_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::on_key_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_owned_and_borrowed_tables_conform", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::on_pointer_edit_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_press_cannot_retarget_after_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::on_pointer_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_list_press_cannot_retarget_after_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::render_edit_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::render_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::data_table_and_list_box_paint_borrowed_rows_through_render_source", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::rows", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_grid_sparse_rows_normalize_to_null", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::try_new", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_edit_lifecycle.rs::issue32_failed_shape_change_keeps_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::try_set_model", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_model_replacement_rebuilds_dependent_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::try_set_row", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_table_rejects_short_and_long_rows", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::try_set_rows", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_edit_lifecycle.rs::issue32_failed_shape_change_keeps_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::update_cell", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_update_cell_keeps_the_cursor_on_its_record_when_the_sort_moves_it", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::DataTable::update_column", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/data_shape.rs::issue42_table_update_column_is_checked", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_validates_shape_and_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::insert", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::is_empty", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_lists_its_row_keys_and_edits_one_cell_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::len", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::move_row", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_commits_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::remove", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_cancels_the_edit_when_its_row_is_removed", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::replace_rows", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_rejects_rows_under_an_empty_schema", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::row", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::row_keys", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_lists_its_row_keys_and_edits_one_cell_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::try_new", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_validates_shape_and_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTable::update_cell", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::data_table_and_list_box_paint_borrowed_rows_through_render_source", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTableError", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_validates_shape_and_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTableError::Key", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_validates_shape_and_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::KeyedTableError::Shape", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_validates_shape_and_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::TableEditSource", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::table::TableSource", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_keeps_a_rejected_commit_in_the_editor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tabs::Tabs::items", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/document_workspace.rs::workspace_tabs_projection_marks_dirty_and_busy", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::tabs::Tabs::push", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/deferred_api_calls.rs::tabs_push_appends_the_item_after_the_existing_ones", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::tabs::Tabs::reorder", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/tabs_overflow_mutation.rs::issue44_reorder_moves_active_with_its_tab", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::tabs::Tabs::update_item", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/tabs_overflow_mutation.rs::issue44_item_mutation_noop_keeps_menu", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::toolbar::Toolbar::insert_item", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::collection_checked_inserts_and_removes_reject_out_of_range", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::toolbar::Toolbar::items", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/bin/showcase/pages/toolbars.rs::stress_items_do_not_start_or_stop_a_run", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::toolbar::Toolbar::remove_item", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::toolbar_cursor_keeps_the_more_stop_and_clamps_past_it", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::toolbar::Toolbar::set_items", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::toolbar_mutation_invalidates_hidden_geometry", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::toolbar::Toolbar::update_item", kind: ExceptionKind::Deferred, reason: "Chips, toolbars, tabs and panels API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/collection_menu_lifecycle.rs::collection_equal_updates_preserve_menu", tracking_issue: Some(296) },
    CapabilityException { api: "widgets::tree::KeyedTree", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::get", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::insert", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::is_empty", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_table_lists_its_row_keys_and_edits_one_cell_by_key", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::len", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "src/widgets/tree.rs::unfiltered_refold_keeps_branch_local_work", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::new", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::remove", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::borrowed_table_cancels_the_edit_when_its_row_is_removed", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTree::update", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode::children", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode::glyph", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode::lazy", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_collapse_and_lazy_expand_report_keys", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode::meta", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::KeyedTreeNode::new", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_new_provider_with_equal_counters_resets_state", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeNodeRef", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeSource", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::expand_all_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_cursor_moves_by_key_and_expand_all_opens_every_branch", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::keyed", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::on_key_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_keeps_expansion_after_insert", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::on_pointer_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_press_cannot_retarget_after_reorder", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::render_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::data_table_and_list_box_paint_borrowed_rows_through_render_source", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::select_follows_cursor", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::select_follows_cursor_moves_the_selection_with_the_cursor", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::select_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_select_source_refuses_an_unknown_key_without_mutation", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::set_cursor_source", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_cursor_moves_by_key_and_expand_all_opens_every_branch", tracking_issue: Some(303) },
    CapabilityException { api: "widgets::tree::TreeView::show_selection", kind: ExceptionKind::Deferred, reason: "Lists, tables and trees API added after this gate was written: no Showcase page or callable scenario drives it yet, so its demo is tracked in the area issue and the named test pins its behavior meanwhile.", regression_test: "tests/regressions/keyed_collections.rs::keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view", tracking_issue: Some(303) },
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
    CapabilityException { api: "widgets::grid::DataGrid::paste_into_cell", kind: ExceptionKind::Deferred, reason: "The Grid page calls it (recorded in GRID_EDIT_COVERAGE), but no callable scenario drives it yet; the named test pins its behavior.", regression_test: "src/widgets/grid_undo_tests.rs::codex196_paste_into_cell_runs_the_edit_gateway", tracking_issue: Some(298) },
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
