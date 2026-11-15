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
    "terminal::SessionLink::set_size",
    "terminal::SessionLink::drain",
    "terminal::SessionSource",
    "terminal::SessionSource::open",
    "terminal::SessionSource::restart",
    "terminal::SessionSource::closed_before_attach",
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
    "terminal::SessionClipboardRefusal::Read",
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
    "terminal::SessionEffect::Copy",
    "terminal::SessionKey",
    "terminal::SessionKey::Consumed",
    "terminal::SessionKey::Forwarded",
    "terminal::SessionPaste",
    "terminal::SessionPaste::Sent",
    "terminal::SessionPaste::NeedsConfirm",
    "terminal::SessionPaste::TooLarge",
    "terminal::SessionPaste::Full",
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

pub const GRID_EDIT_COVERAGE: &[(&str, &str)] = &[
    ("widgets::grid::DataGrid::try_edit_text", "grid"),
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
