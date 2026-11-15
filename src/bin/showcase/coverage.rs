pub const PICKER_COVERAGE: &[(&str, &str)] = &[
    ("widgets::picker::Picker::report_item_error", "pickers"),
    ("widgets::picker::Picker::items", "pickers"),
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
    ("widgets::picker::Picker::set_visible_items", "pickers"),
    ("widgets::picker::Picker::set_cursor", "pickers"),
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
        "widgets::grid::GridDataError::Keys",
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

pub const PICKER_ALLOWLIST: &[(&str, &str)] = &[
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

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_source() -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(root.join("src/catalog.rs")).expect("catalog source reads")
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
            assert!(seen.insert(*id), "duplicate allowlist entry: {id}");
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
}
