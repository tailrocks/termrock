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

pub const PICKER_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::picker::Picker::set_visible_items",
        "Compatibility ingress that keeps only each item's key and match; the showcase uses set_visible_keys, and tests/regressions/keyed_costs pins the old ingress.",
    ),
    (
        "widgets::picker::Picker::visible_items",
        "Read-only enumeration of the projection for application code; the showcase reads one row through visible_item.",
    ),
    (
        "widgets::picker::Picker::visible_len",
        "Read-only count of the projection for application code; the showcase has no counter of visible rows.",
    ),
    (
        "widgets::picker::PickerItemView",
        "Returned by visible_item and visible_items; pages read its item and matched fields without naming the type.",
    ),
    (
        "widgets::picker::Picker::wheel",
        "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.",
    ),
    (
        "widgets::picker::Picker::set_chosen",
        "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.",
    ),
    (
        "widgets::picker::Picker::set_placement",
        "The showcase configures placement through its builder; no concrete app uses this setter.",
    ),
    (
        "widgets::picker::Picker::set_max_rows",
        "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.",
    ),
    (
        "widgets::picker::Picker::set_width",
        "The showcase uses builder configuration and layer-driven interaction; this method serves application or layer plumbing.",
    ),
    (
        "widgets::picker::PickerItemsError::UnknownVisibleKey",
        "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.",
    ),
    (
        "widgets::picker::PickerItemsError::MissingStableKey",
        "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.",
    ),
    (
        "widgets::picker::PickerItemsError::DuplicateKey",
        "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.",
    ),
    (
        "widgets::picker::PickerItemsError",
        "Invalid item identities are exercised by picker validation tests; the showcase uses valid picker inputs.",
    ),
    (
        "widgets::picker::DEFAULT_GLYPH",
        "Every showcase row sets its glyph to show the list style explicitly, so the default glyph is not visible in this page.",
    ),
    (
        "widgets::picker::PickerCmd",
        "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command type as a page action.",
    ),
    (
        "widgets::picker::PickerCmd::Toggle",
        "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command variant as a page action.",
    ),
    (
        "widgets::picker::PickerKeyProfile::install",
        "The page configures each picker through its builder and intentionally does not change the thread-local default profile.",
    ),
    (
        "widgets::picker::PickerKeyProfile::current",
        "The page configures each picker through its builder and intentionally does not read the thread-local default profile.",
    ),
    (
        "widgets::picker::Picker::MAX_ROWS",
        "The page exposes row limits through the configured builder and does not refer to the widget's default constant directly.",
    ),
    (
        "widgets::picker::Picker::WIDTH",
        "The page exposes widths through the configured builder and does not refer to the widget's default constant directly.",
    ),
    (
        "widgets::picker::Picker::choose_hint",
        "The page demonstrates choice labels through its configured hint lines rather than this per-picker label builder.",
    ),
    (
        "widgets::picker::Picker::keymap",
        "The page demonstrates the supported Desktop and Vim profiles; custom keymaps are not a page configuration option.",
    ),
    (
        "widgets::picker::Picker::toggle_binding",
        "The page demonstrates the supported Desktop and Vim profiles; custom toggle bindings are not a page configuration option.",
    ),
    (
        "widgets::picker::Picker::searchable",
        "The page uses the explicit searchable state setter for its nonsearchable examples and does not need the builder form.",
    ),
    (
        "widgets::picker::Picker::is_multi",
        "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading widget mode directly.",
    ),
    (
        "widgets::picker::Picker::checked",
        "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading the checked-key buffer directly.",
    ),
    (
        "widgets::option_list::OptionList::toggle_checked",
        "The page drives check state through delivered input events on the picker layer rather than mutating the shared list directly.",
    ),
    (
        "widgets::option_list::OptionList::is_checked",
        "The page observes selection through rendered check glyphs and picker actions rather than polling the shared list directly.",
    ),
    (
        "widgets::picker::Picker::toggle_checked",
        "The page presents multi-select behavior through delivered input events rather than manipulating selection state directly.",
    ),
    (
        "widgets::picker::Picker::reset_selection",
        "Picker layers in the showcase are ephemeral and have no user-facing reset action; the widget state test exercises the reset operation directly.",
    ),
    (
        "widgets::picker::Picker::caret",
        "The page demonstrates filtering through terminal input and does not inspect the edit caret directly.",
    ),
    (
        "widgets::picker::Picker::set_query",
        "The page demonstrates filtering through terminal input and does not set the query programmatically.",
    ),
    (
        "widgets::picker::Picker::is_searchable",
        "The page presents search behavior through visible picker interaction and does not inspect the internal mode flag.",
    ),
    (
        "widgets::picker::Picker::current",
        "The page presents row selection through the picker layer and does not inspect the selected item directly.",
    ),
    (
        "widgets::picker::Picker::current_key",
        "The page presents row selection through the picker layer and does not inspect the selected key directly.",
    ),
    (
        "widgets::picker::Picker::eligible",
        "The page presents disabled-row behavior through visible rows and does not query eligible indices directly.",
    ),
    (
        "widgets::picker::Picker::item_for_key",
        "The page presents item behavior through visible rows and does not query the backing item map directly.",
    ),
    (
        "widgets::picker::Picker::height",
        "The page delegates sizing to its layer and layout system rather than calling the widget height helper directly.",
    ),
    (
        "widgets::picker::Picker::on_click_outside",
        "The page demonstrates dismissal through the layer interaction and does not invoke its low-level outside-click handler.",
    ),
    (
        "widgets::picker::Picker::on_key",
        "The page demonstrates keyboard behavior through terminal events and does not call the widget event handler directly.",
    ),
    (
        "widgets::picker::Picker::on_click",
        "The page demonstrates pointer behavior through terminal events and does not call the widget event handler directly.",
    ),
    (
        "widgets::picker::Picker::on_paste",
        "The page demonstrates filtering through ordinary terminal input and has no paste-specific picker scenario.",
    ),
    (
        "widgets::picker::Picker::row_id",
        "The page delegates row identity and pointer routing to the picker layer rather than calling the row-id helper directly.",
    ),
    (
        "widgets::picker::Picker::draw",
        "The page composes pickers through layers and does not draw the widget directly into a buffer.",
    ),
    (
        "widgets::picker::Picker::render",
        "The page composes pickers through layers and does not call the widget's direct render entry point.",
    ),
];

pub const KEYMAP_ALLOWLIST: &[(&str, &str)] = &[
    (
        "core::keymap::KeyboardCapabilities",
        "Pages render with the ambient terminal capabilities and never construct a capability set; every hint bar already reflects them.",
    ),
    (
        "core::keymap::KeyboardCapabilities::current",
        "Showcase pages ask widgets for their hints instead of probing the thread-local capability state themselves.",
    ),
    (
        "core::keymap::KeyBinding::deliverable_with",
        "Pages observe deliverability through the hint keys that survive rendering, not by calling the capability-aware predicate.",
    ),
    (
        "core::keymap::KeyBinding::matches_with",
        "Page key handling goes through widget keymaps; no page matches a binding against explicit capabilities.",
    ),
    (
        "core::keymap::lookup_with",
        "Pages look bindings up through the ambient `lookup` helper, so the capability-carrying overload has no page call site.",
    ),
    (
        "core::keymap::deliverable_binding_for_with",
        "Pages read the deliverable binding from rendered hint bars, so the capability-aware lookup has no page call site.",
    ),
    (
        "core::keymap::normalized_key_mods",
        "The widgets own the key normalization; no showcase page asks for the normalized modifiers of a key on its own.",
    ),
    (
        "core::keymap::KeyBinding::normalized_mods",
        "The widgets own the key normalization; no showcase page asks a binding for its normalized modifiers.",
    ),
    (
        "core::hints::HintCx::current",
        "Pages draw widgets through the showcase harness, which supplies the hint context; no page builds one from ambient state.",
    ),
    (
        "core::hints::HintCx::with_environment",
        "No showcase page overrides the hint environment; the harness-rendered hint bars are the demonstration.",
    ),
    (
        "widgets::keyhint::Hint::click_key",
        "Footer hint clicks are demonstrated through delivered pointer events on rendered hint bars; the stable click-key override is filled in internally by widget hint generators and is not a page configuration option.",
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
    fn capability_aware_keymap_apis_are_declared_with_reasons() {
        let catalog = catalog_source();
        let mut seen = std::collections::BTreeSet::new();
        for (id, reason) in KEYMAP_ALLOWLIST {
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
        for id in [
            "core::keymap::KeyboardCapabilities",
            "core::keymap::KeyboardCapabilities::current",
            "core::keymap::KeyBinding::deliverable_with",
            "core::keymap::KeyBinding::matches_with",
            "core::keymap::lookup_with",
            "core::keymap::deliverable_binding_for_with",
            "core::keymap::normalized_key_mods",
            "core::keymap::KeyBinding::normalized_mods",
            "core::hints::HintCx::current",
            "core::hints::HintCx::with_environment",
        ] {
            assert!(
                seen.contains(id),
                "capability-aware keymap API lacks a coverage declaration: {id}"
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
