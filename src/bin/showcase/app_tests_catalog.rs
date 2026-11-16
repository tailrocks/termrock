//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::clock::Moment;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::Axis;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::core::text::TextBuffer;
use junie_tui::shell::{Cx, Request, StatusLine, background};
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, SyntaxTone, Theme, Tone};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::choice_dialog::ChoiceDialog;
use junie_tui::widgets::code::{CodeEditor, Diagnostic, Highlighter, Segmenter, Severity};
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::edit::{EditField, Validator};
use junie_tui::widgets::form_dialog::{FormAction, FormDialog, FormField};
use junie_tui::widgets::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, DataGrid, FinishEdit, GridAction,
    GridCommitFailure, GridDataError, GridEditError, GridKeyError, GridRows, GridUndoKind,
    RowTotal,
};
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};
use junie_tui::widgets::option_editor::{
    ChoiceEditor, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use super::super::coverage::ScenarioId;

pub struct ScenarioCase {
    pub id: ScenarioId,
    pub run: fn(),
}

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::PickerStableSelection, run: picker_stable_selection },
    ScenarioCase { id: ScenarioId::RequiredCustomValidation, run: required_custom_validation },
    ScenarioCase { id: ScenarioId::DisabledInputRejection, run: disabled_input_rejection },
    ScenarioCase { id: ScenarioId::OverlayDismissPolicy, run: overlay_dismiss_policy },
    ScenarioCase { id: ScenarioId::CollectionReplacement, run: collection_replacement },
    ScenarioCase { id: ScenarioId::NarrowLayout, run: narrow_layout },
    ScenarioCase { id: ScenarioId::TerminalViewportLifecycle, run: terminal_viewport_lifecycle },
    ScenarioCase { id: ScenarioId::FixedControlsPolicy, run: fixed_controls_policy },
    ScenarioCase { id: ScenarioId::FixedDialogsPolicy, run: fixed_dialogs_policy },
    ScenarioCase { id: ScenarioId::ListDetailNavigation, run: list_detail_navigation },
    ScenarioCase { id: ScenarioId::StatusNotice, run: status_notice },
    ScenarioCase { id: ScenarioId::PickerLiveControl, run: picker_live_control },
    ScenarioCase { id: ScenarioId::TextInputSetValue, run: text_input_set_value },
    ScenarioCase { id: ScenarioId::ChoiceEditorOptions, run: choice_editor_options },
    ScenarioCase { id: ScenarioId::SplitterCancelDrag, run: splitter_cancel_drag },
    ScenarioCase { id: ScenarioId::CodeEditorProviders, run: code_editor_providers },
    ScenarioCase { id: ScenarioId::CodeEditorDiagnosticsFind, run: code_editor_diagnostics_find },
    ScenarioCase { id: ScenarioId::GridEditSession, run: grid_edit_session },
    ScenarioCase { id: ScenarioId::GridChangeCapture, run: grid_change_capture },
    ScenarioCase { id: ScenarioId::GridSubmissionLock, run: grid_submission_lock },
    ScenarioCase { id: ScenarioId::GridSourceIdentity, run: grid_source_identity },
    ScenarioCase { id: ScenarioId::GridUndoKinds, run: grid_undo_kinds },
    ScenarioCase { id: ScenarioId::ExternalFeedBounds, run: external_feed_bounds },
    ScenarioCase { id: ScenarioId::ScrollWindowWork, run: scroll_window_work },
    ScenarioCase { id: ScenarioId::ModalFocusPolicy, run: modal_focus_policy },
    ScenarioCase { id: ScenarioId::DocumentWorkspaceLifecycle, run: document_workspace_lifecycle },
    ScenarioCase { id: ScenarioId::CompletionPopupSnapshot, run: completion_popup_snapshot },
    ScenarioCase { id: ScenarioId::SessionAttachReplay, run: session_attach_replay },
    ScenarioCase { id: ScenarioId::SessionInputAndPaste, run: session_input_and_paste },
    ScenarioCase { id: ScenarioId::SessionEndingsAndPolicy, run: session_endings_and_policy },
];

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect()
}

fn text_of(buf: &Buffer) -> String {
    rows(buf).join("\n")
}

fn paint(area: Rect, draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    draw(area, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    buf
}

fn lettered(labels: &[&str]) -> Vec<Item> {
    labels
        .iter()
        .map(|label| Item::new(*label).key(ItemKey::text(&label.to_lowercase())))
        .collect()
}

fn picker_stable_selection() {
    let mut picker = Picker::new(WidgetId::of("scenario.picker"), "Choose")
        .multi_select(true)
        .expect("an empty picker accepts multi-selection");
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    assert!(picker.visible_item(2).is_some() && picker.visible_item(3).is_none());
    picker.set_cursor(0);
    let (outcome, _) = picker.on_key(&key(KeyCode::Insert));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

    picker
        .set_visible_keys(vec![
            VisibleKey::new(ItemKey::text("beta")).matched(vec![0]),
        ])
        .expect("a filtered view keeps hidden choices");
    assert_eq!(
        picker.visible_item(0).map(|row| row.matched.to_vec()),
        Some(vec![0])
    );
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
    picker
        .replace_items(lettered(&["Gamma", "Beta", "Alpha"]))
        .expect("reordering keeps identity");
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

    let (_, action) = picker.on_key(&key(KeyCode::Enter));
    assert_eq!(
        action,
        Some(PickerAction::ChosenMany(vec![ItemKey::text("alpha")]))
    );
    let buf = paint(Rect::new(0, 0, 80, 24), |area, buf, ctx| {
        picker.draw(area, buf, ctx)
    });
    assert!(text_of(&buf).contains("Alpha"));
}

fn required_custom_validation() {
    let mut required = TextInput::new(WidgetId::of("scenario.required"), "Name").required(true);
    assert!(!required.validate(), "an empty required field is invalid");
    assert!(required.error().is_some());
    required = required.value("filled");
    assert!(required.validate());
    assert_eq!(required.error(), None);

    let mut custom = TextInput::new(WidgetId::of("scenario.custom"), "Name")
        .validator(|text| (text == "bad").then(|| "not allowed".to_owned()));
    custom = custom.value("bad");
    assert!(!custom.validate());
    assert_eq!(custom.error(), Some("not allowed"));
    custom = custom.value("good");
    assert!(custom.validate());
    assert_eq!(custom.error(), None);

    let mut field = EditField::single("bad").validator(Validator::check(|text| {
        (text == "bad").then(|| "not allowed".to_owned())
    }));
    assert!(!field.validate());
    assert_eq!(field.error_message(), Some("not allowed"));
    field.set_text("good");
    assert!(field.validate());
    assert_eq!(field.error_message(), None);
}

fn disabled_input_rejection() {
    let mut input = TextInput::new(WidgetId::of("scenario.disabled"), "Name")
        .value("fixed")
        .disabled(true);
    let (outcome, action) = input.on_key(&key(KeyCode::Char('x')));
    assert_eq!(outcome, Outcome::Ignored);
    assert_eq!(action, None);
    assert_eq!(input.on_paste("pasted"), Outcome::Ignored);
    assert_eq!(input.text(), "fixed");

    let mut live = TextInput::new(WidgetId::of("scenario.live"), "Name").value("ok");
    live.begin_edit();
    let (outcome, _) = live.on_key(&key(KeyCode::Char('!')));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(live.text(), "ok!");
}

fn overlay_dismiss_policy() {
    for (policy, outside, escape) in [
        (Dismiss::LIGHT, true, true),
        (Dismiss::ESCAPE, false, true),
        (Dismiss::NEVER, false, false),
    ] {
        assert_eq!(policy.outside_click, outside);
        assert_eq!(policy.escape, escape);
        let mut picker = Picker::new(WidgetId::of("scenario.dismiss"), "Choose").dismiss(policy);
        assert_eq!(
            picker.on_click_outside(),
            outside.then_some(PickerAction::Cancelled)
        );

        let mut dialog = Dialog::confirm(WidgetId::of("scenario.dialog"), "Close", "Sure?", "Yes")
            .dismiss(policy);
        dialog.on_click_outside();
        assert_eq!(
            dialog.result,
            outside.then_some(DialogResult::Action(0)),
            "an outside click resolves to the cancel action only when the policy allows it"
        );
        let mut dialog = Dialog::confirm(WidgetId::of("scenario.dialog"), "Close", "Sure?", "Yes")
            .dismiss(policy);
        let mut focus = Focus::default();
        dialog.on_key(&key(KeyCode::Esc), &mut focus, &FocusRing::default());
        assert_eq!(
            dialog.result.is_some(),
            escape,
            "Esc follows the escape policy"
        );

        let ring = FocusRing::default();
        let mut choice = ChoiceDialog::new(
            WidgetId::of("scenario.choice"),
            "Channel",
            "Updates come from",
            &["Stable", "Beta"],
            0,
        )
        .dismiss(policy);
        choice.on_key(&key(KeyCode::Esc), &mut Focus::default(), &ring);
        assert_eq!(
            choice.result,
            escape.then_some(None),
            "a dismissed choice dialog resolves to cancelled, never to a choice"
        );

        let mut info = InfoDialog::new(
            WidgetId::of("scenario.info"),
            "Server",
            vec![Prop::new("Host", "db.internal")],
        )
        .dismiss(policy);
        info.on_key(&key(KeyCode::Esc), &mut Focus::default(), &ring);
        assert_eq!(info.result.is_some(), escape, "info Esc follows the policy");

        let mut form = FormDialog::new(
            WidgetId::of("scenario.form"),
            "Connection",
            vec![FormField::input(
                "name",
                TextInput::new(WidgetId::of("scenario.form.name"), "Name"),
            )],
        )
        .dismiss(policy);
        form.on_key(&key(KeyCode::Esc), &mut Focus::default(), &ring);
        assert_eq!(
            form.events,
            if escape {
                vec![FormAction::Cancel]
            } else {
                vec![]
            },
            "a dismissed form resolves to Cancel and keeps its values"
        );
    }
}

fn collection_replacement() {
    let mut picker = Picker::new(WidgetId::of("scenario.replace"), "Choose");
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    picker.set_cursor(1);
    assert_eq!(
        picker.current().map(|item| item.label.as_str()),
        Some("Beta")
    );
    picker
        .replace_items(lettered(&["Gamma", "Beta", "Alpha"]))
        .expect("reordering keeps identity");
    assert_eq!(
        picker.current().map(|item| item.label.as_str()),
        Some("Beta"),
        "the cursor follows the item, not the index"
    );
    let duplicate = lettered(&["Alpha", "Alpha"]);
    assert_eq!(
        picker.replace_items(duplicate),
        Err(PickerItemsError::DuplicateKey(ItemKey::text("alpha")))
    );
    assert!(
        picker.visible_item(2).is_some() && picker.visible_item(3).is_none(),
        "a rejected replacement changes nothing"
    );
}

fn narrow_layout() {
    let mut picker = Picker::new(WidgetId::of("scenario.narrow"), "Choose")
        .width(64)
        .max_rows(3)
        .placement(Placement::Center);
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]))
        .expect("keyed items are accepted");
    let area = Rect::new(0, 0, 28, 12);
    let buf = paint(area, |area, buf, ctx| picker.draw(area, buf, ctx));
    let text = text_of(&buf);
    assert!(
        text.contains("Choose"),
        "the title survives a 28 column screen:\n{text}"
    );
    assert!(text.contains("Alpha"));
    assert!(
        !text.contains("Epsilon"),
        "max_rows bounds the visible rows"
    );
}

fn terminal_viewport_lifecycle() {
    let id = WidgetId::of("scenario.terminal");
    let mut viewport = TerminalViewport::new(id, TerminalState::new(10, 10));
    viewport.set_area(Rect::new(0, 0, 11, 10));
    let lines = (1..=100)
        .map(|n| format!("line {n:03}"))
        .collect::<Vec<_>>()
        .join("\r\n");
    viewport.feed(lines.as_bytes());
    assert!(viewport.follow());
    assert_eq!(
        viewport.position_label(),
        "",
        "the live tail shows no position"
    );
    assert_eq!(viewport.wheel(Axis::Vertical, -3), Outcome::Changed);
    assert!(!viewport.follow());
    assert_eq!(viewport.offset(), 3);
    assert_eq!(viewport.position_label(), "88–97 of 100");
    viewport.feed(b"\r\nline 101");
    assert_eq!(
        viewport.offset(),
        4,
        "new output does not move what is read"
    );
    viewport.reset();
    assert_eq!(viewport.offset(), 0);
}

fn fixed_controls_policy() {
    clear_refusal();
    let reason = || Reason::new("Always off, the host decides").short("managed");
    let mut fixed =
        Checkbox::new(WidgetId::of("scenario.fixed"), "Keep awake", false).fixed(reason());
    assert_eq!(fixed.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(
        take_refusal().map(|r| r.text).as_deref(),
        Some("Always off, the host decides")
    );
    assert_eq!(fixed.on_click(), Outcome::Consumed);
    assert!(take_refusal().is_some());
    assert!(!fixed.checked);
    assert_eq!(
        take_refusal().map(|r| r.text),
        None,
        "taking the refusal clears it"
    );

    let mut live = Checkbox::new(WidgetId::of("scenario.live"), "Keep awake", false);
    assert_eq!(live.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(live.checked);
    assert_eq!(take_refusal().map(|r| r.text), None);
}

fn fixed_dialogs_policy() {
    clear_refusal();
    let route = |fixed: bool| {
        let id = WidgetId::of("scenario.fixed.dialog");
        let mut dialog = Dialog::confirm(id, "Disable all", "Every item stays on.", "Disable");
        if fixed {
            dialog.actions[1] = dialog.actions[1]
                .clone()
                .fixed(Reason::new("Always on, the host decides"));
        }
        let mut focus = Focus::default();
        focus.set(Some(id.sub("ok")));
        let outcome = dialog.on_key(&key(KeyCode::Enter), &mut focus, &FocusRing::default());
        (outcome, dialog.result)
    };
    let (outcome, result) = route(false);
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(result, Some(DialogResult::Action(1)));
    assert!(take_refusal().is_none());
    let (outcome, result) = route(true);
    assert!(outcome.consumed());
    assert_eq!(result, None, "a fixed action never resolves the dialog");
    assert_eq!(
        take_refusal().map(|r| r.text).as_deref(),
        Some("Always on, the host decides")
    );
}

fn list_detail_navigation() {
    const MASTER: WidgetId = WidgetId::of("scenario.ld.master");
    const DETAIL: WidgetId = WidgetId::of("scenario.ld.detail");
    let area = Rect::new(0, 0, 120, 20);
    let rows = vec![ListRow::new("first"), ListRow::new("second")];
    let mut list = ListDetail::new(MASTER, DETAIL, Split::new(42, 36, 44));
    let mut focus = Focus::default();
    focus.focus(DETAIL);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let render = |list: &mut ListDetail, focus: &Focus| {
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        let items = (0..4).map(|n| Line::Item {
            mark: None,
            cols: [format!("item{n:02}"), "agent".into(), "meta".into()],
            tone: junie_tui::theme::Tone::Secondary,
        });
        let frame = Frame {
            title: "List",
            meta: String::new(),
            head: vec![],
            rows: &rows,
            under: None,
            detail_title: "Detail".into(),
            detail_meta: String::new(),
            lines: items.collect(),
            actions: vec![Action::new(
                "go",
                "go the whole selection",
                junie_tui::theme::ButtonKind::Secondary,
            )],
        };
        list.render(area, &mut buf, &mut ctx, &frame);
        buf
    };
    let buf = render(&mut list, &focus);
    assert!(text_of(&buf).contains("second"), "both rows are listed");
    for _ in 0..3 {
        list.on_key(&key(KeyCode::Down), &mut focus, rows.len());
    }
    let (_, event) = list.on_key(&key(KeyCode::Enter), &mut focus, rows.len());
    assert_eq!(
        event,
        Some(Event::Item(3)),
        "Enter reports the item under the cursor"
    );
    list.select(1);
    assert_eq!(
        (list.cursor, list.item_cursor),
        (1, 0),
        "selecting a row resets the item cursor"
    );
    list.clamp(1);
    assert_eq!(
        list.cursor, 0,
        "clamp keeps the cursor inside a shorter list"
    );
    list.open_detail(&mut focus);
    list.close_detail(&mut focus);
    let _ = Position::new(0, 0);
}

const GRID_NAME: usize = 0;
const GRID_SEATS: usize = 1;

fn grid_columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new("name", CellKind::Text),
        ColumnSpec::new("seats", CellKind::Number),
    ]
}

fn grid_rows(n: i64) -> GridRows {
    GridRows {
        rows: (0..n)
            .map(|i| vec![CellValue::Text(format!("row{i}")), CellValue::Int(3 + i)])
            .collect(),
        total: RowTotal::Exact(n as usize),
        more: false,
    }
}

fn seeded_grid(name: &'static str, n: i64) -> DataGrid {
    let mut grid = DataGrid::new(WidgetId::of(name), grid_columns());
    grid.try_set_rows(grid_rows(n)).expect("rectangular rows");
    grid
}

fn grid_action(grid: &mut DataGrid, code: KeyCode) -> Option<GridAction> {
    grid.on_key(&key(code))
        .try_into_single()
        .expect("at most one action")
        .1
}

fn grid_edit_session() {
    let mut grid = seeded_grid("scenario.grid.edit", 3);
    assert_eq!(
        grid.try_edit_text(0, GRID_SEATS, "40"),
        Ok(CellEditResult::Changed)
    );
    assert_eq!(
        grid.try_edit_text(0, GRID_SEATS, "40"),
        Ok(CellEditResult::Unchanged),
        "the same value records nothing twice"
    );
    assert!(matches!(
        grid.try_edit_text(0, GRID_SEATS, "lots"),
        Err(GridEditError::Validation(_))
    ));
    assert_eq!(
        grid.try_edit_text(9, GRID_SEATS, "1"),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(
        grid.pending().total(),
        1,
        "a refusal leaves the draft as it was"
    );

    grid.cursor = (1, GRID_SEATS);
    let pasted = grid
        .paste_into_cell("7")
        .try_into_single()
        .expect("one action");
    assert_eq!(
        pasted.1,
        Some(GridAction::CellChanged {
            row: 1,
            col: GRID_SEATS
        })
    );
    assert_eq!(grid.value(1, GRID_SEATS), &CellValue::Int(7));

    assert_eq!(grid.finish_edit_session(), Ok(FinishEdit::NoSession));
    grid.cursor = (2, GRID_NAME);
    grid_action(&mut grid, KeyCode::Enter);
    for c in "ab".chars() {
        grid_action(&mut grid, KeyCode::Char(c));
    }
    assert_eq!(
        grid.finish_edit_session(),
        Ok(FinishEdit::Accepted {
            row: 2,
            col: GRID_NAME,
            result: CellEditResult::Changed
        })
    );

    let mut combo = DataGrid::new(
        WidgetId::of("scenario.grid.combo"),
        vec![
            ColumnSpec::new("pick", CellKind::Text).editor(ColumnEditor::Combo(Options::new(
                ["a", "b"].map(OptionItem::new),
            ))),
        ],
    );
    combo
        .try_set_rows(GridRows {
            rows: vec![vec![CellValue::Text("a".into())]],
            total: RowTotal::Exact(1),
            more: false,
        })
        .expect("rectangular rows");
    grid_action(&mut combo, KeyCode::Enter);
    for c in "zz".chars() {
        grid_action(&mut combo, KeyCode::Char(c));
    }
    assert_eq!(
        combo.finish_edit_session(),
        Err(GridEditError::NoChoice),
        "text that matches no option commits nothing"
    );

    let snapshot = grid.snapshot_changes();
    let token = grid
        .begin_submission(snapshot)
        .expect("a fresh capture locks");
    assert_eq!(
        grid.try_edit_text(0, GRID_SEATS, "5"),
        Err(GridEditError::Busy)
    );
    grid.cancel_submission(token).expect("the lock's own token");
}

fn grid_change_capture() {
    let mut grid = seeded_grid("scenario.grid.capture", 3);
    grid.record_cell(0, GRID_SEATS, CellValue::Int(40))
        .expect("an unlocked grid");
    grid.toggle_delete(1).expect("an unlocked grid");
    grid.insert_row().expect("an unlocked grid");
    let deleted = grid.row_key(1).expect("row 1 has a key");
    let inserted = grid.row_key(3).expect("the new row has a key");

    let pending = grid.pending();
    assert_eq!(pending.cells().count(), 1);
    assert_eq!(pending.deleted().collect::<Vec<_>>(), vec![deleted]);
    assert_eq!(pending.inserted().collect::<Vec<_>>(), vec![inserted]);
    assert!(pending.is_deleted(deleted) && !pending.is_inserted(deleted));
    assert!(pending.is_inserted(inserted) && !pending.is_deleted(inserted));

    let snapshot = grid.snapshot_changes();
    assert_eq!(snapshot.counts(), (1, 1, 1));
    let updated = &snapshot.updated()[0];
    assert_eq!(Some(updated.key()), grid.row_key(0));
    assert_eq!(updated.before()[GRID_SEATS], CellValue::Int(3));
    let change = &updated.cells()[0];
    assert_eq!(grid.columns()[change.column()].name, "seats");
    assert_eq!(change.after(), &CellValue::Int(40));
    let insert = &snapshot.inserted()[0];
    assert_eq!(insert.key(), inserted);
    assert_eq!(insert.values().len(), grid.columns().len());
    let delete = &snapshot.deleted()[0];
    assert_eq!(delete.before()[GRID_NAME], CellValue::Text("row1".into()));

    grid.record_cell(2, GRID_SEATS, CellValue::Int(9))
        .expect("an unlocked grid");
    assert_eq!(
        snapshot.counts(),
        (1, 1, 1),
        "a capture never sees a later edit"
    );
}

fn grid_submission_lock() {
    let mut grid = seeded_grid("scenario.grid.submit", 3);
    grid.record_cell(0, GRID_SEATS, CellValue::Int(40))
        .expect("an unlocked grid");
    assert!(!grid.submission_pending());

    let snapshot = grid.snapshot_changes();
    let token = grid.begin_submission(snapshot.clone()).expect("locks");
    assert!(grid.submission_pending());
    assert_eq!(
        grid.record_cell(1, GRID_SEATS, CellValue::Int(5)),
        Err(GridEditError::Busy),
        "a locked draft refuses edits"
    );
    grid.cancel_submission(token).expect("the lock's own token");
    assert!(!grid.submission_pending());
    assert_eq!(grid.pending().total(), 1, "a cancel keeps the draft");

    let token = grid
        .begin_submission(snapshot.clone())
        .expect("locks again");
    let refused = GridCommitFailure {
        key: grid.row_key(0),
        message: "seats above the plan limit".into(),
    };
    grid.complete_submission(token, Err(refused))
        .expect("the lock's own token");
    assert!(!grid.submission_pending(), "a failure unlocks");
    assert_eq!(grid.pending().total(), 1, "and keeps the draft");
    assert!(grid.row_errors.contains_key(&0), "and marks the row");

    let token = grid
        .begin_submission(snapshot.clone())
        .expect("locks again");
    grid.complete_submission(token, Ok(snapshot.identity_receipt()))
        .expect("the receipt names the captured rows");
    assert!(grid.pending().is_empty(), "a saved draft is gone");
    assert_eq!(grid.value(0, GRID_SEATS), &CellValue::Int(40));
}

fn grid_source_identity() {
    let key = ItemKey::num;
    let mut grid = DataGrid::new(WidgetId::of("scenario.grid.source"), grid_columns());
    grid.try_set_rows_keyed(grid_rows(3), vec![key(10), key(20), key(30)])
        .expect("three distinct keys");
    assert_eq!(grid.row_key(1), Some(key(20)));
    assert_eq!(grid.source_index(key(30)), Some(2));
    assert_eq!(grid.source_index(key(99)), None);

    let revision = grid.revision();
    assert_eq!(
        grid.try_set_rows_keyed(grid_rows(3), vec![key(1), key(1), key(2)]),
        Err(GridDataError::Key(GridKeyError::DuplicateKey(key(1))))
    );
    assert_eq!(
        grid.try_set_rows_keyed(grid_rows(3), vec![key(1)]),
        Err(GridDataError::Key(GridKeyError::KeyCount {
            expected: 3,
            actual: 1
        }))
    );
    assert_eq!(
        grid.revision(),
        revision,
        "a refused source changes nothing"
    );
    assert_eq!(grid.row_key(0), Some(key(10)));

    grid.try_set_rows(grid_rows(2)).expect("local keys");
    assert_eq!(grid.len(), 2);
    assert!(
        grid.revision().source > revision.source,
        "a new source bumps the revision"
    );
    grid.try_append_rows(grid_rows(2)).expect("a second page");
    assert_eq!(grid.len(), 4);

    let wide = || {
        let mut cols = grid_columns();
        cols.push(ColumnSpec::new("tier", CellKind::Text));
        cols
    };
    let wide_rows = |n: i64| GridRows {
        rows: (0..n)
            .map(|i| {
                vec![
                    CellValue::Text(format!("row{i}")),
                    CellValue::Int(i),
                    CellValue::Null,
                ]
            })
            .collect(),
        total: RowTotal::Exact(n as usize),
        more: false,
    };
    grid.try_set_model(wide(), wide_rows(2))
        .expect("a wider model");
    assert_eq!(grid.columns().len(), 3);
    grid.try_set_model_keyed(wide(), wide_rows(2), vec![key(7), key(8)])
        .expect("a keyed model");
    assert_eq!(grid.row_key(1), Some(key(8)));

    let before = grid.revision();
    assert_eq!(grid.update_column(0, |c| c.read_only = true), Ok(true));
    assert_eq!(grid.update_column(0, |c| c.read_only = true), Ok(false));
    assert_eq!(grid.revision().source, before.source + 1);
    assert_eq!(
        grid.update_column(9, |c| c.read_only = true),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(grid.update_column(1, |c| c.max_width = 12), Ok(true));
    assert_eq!(
        grid.revision().source,
        before.source + 1,
        "a width change is not a new schema"
    );
}

fn grid_undo_kinds() {
    let mut grid = seeded_grid("scenario.grid.undo", 3);
    grid.record_cell(0, GRID_SEATS, CellValue::Int(40))
        .expect("an unlocked grid");
    grid.toggle_delete(1).expect("an unlocked grid");
    grid.insert_row().expect("an unlocked grid");
    grid.toggle_delete(3).expect("removing a draft row");
    let marked = grid.row_key(1).expect("row 1 has a key");
    let first = grid.row_key(0).expect("row 0 has a key");

    let mut seen = Vec::new();
    while let Some(GridAction::Undone(undone)) = grid_action(&mut grid, KeyCode::Char('u')) {
        seen.push(undone.kind);
        match undone.kind {
            GridUndoKind::DeleteMark => assert_eq!(undone.key, marked),
            GridUndoKind::Cell => assert_eq!(undone.key, first),
            GridUndoKind::Insert | GridUndoKind::RemovedInsert => {}
        }
    }
    assert_eq!(
        seen,
        [
            GridUndoKind::RemovedInsert,
            GridUndoKind::Insert,
            GridUndoKind::DeleteMark,
            GridUndoKind::Cell
        ],
        "undo walks the draft back newest first"
    );
    assert!(grid.pending().is_empty());
}

fn completion_popup_snapshot() {
    const ID: WidgetId = WidgetId::of("scenario.completion");
    let labelled = |n: usize| -> Vec<Item> {
        (0..n)
            .map(|i| Item::new(format!("cand{i:02}")).key(ItemKey::index(i)))
            .collect()
    };
    let mut completion = Completion::new(ID).max_rows(4);
    completion.open(
        vec![Item::new("blocked").disabled(true)],
        Rect::new(2, 1, 1, 1),
        0,
    );
    assert!(
        !completion.has_eligible(),
        "a disabled row cannot be accepted"
    );
    completion.set_items(labelled(10));
    assert!(completion.has_eligible());
    assert_eq!(completion.items().len(), 10, "set_items replaced the rows");

    let area = Rect::new(0, 0, 60, 16);
    let seen = std::cell::RefCell::new(Vec::new());
    let buf = paint(area, |area, buf, ctx| {
        completion.render(area, buf, ctx);
        let inner = Rect::new(30, 8, 20, 4);
        let snapshot = completion
            .list
            .snapshot(inner, completion.items(), |i| format!("row{i}"));
        let rows = snapshot.rows();
        assert_eq!(rows.len(), 4, "the snapshot holds only the visible rows");
        assert!(rows.iter().all(|row| row.enabled && !row.checked));
        assert_eq!(rows[0].payload, "row0");
        assert!(!snapshot.more_above() && snapshot.more_below());
        assert_eq!(snapshot.row_id(2), completion.row_id(2));
        snapshot.paint_rows(inner, ctx.theme.canvas, buf, ctx, |row, _, buf, _| {
            seen.borrow_mut().push(row.index);
            buf.set_string(
                inner.x,
                inner.y + row.line,
                &row.payload,
                ratatui::style::Style::new(),
            );
        });
    });
    assert_eq!(*seen.borrow(), [0, 1, 2, 3]);
    let text = text_of(&buf);
    assert!(text.contains("cand00") && text.contains("row3"), "{text}");

    for _ in 0..6 {
        completion.on_key(&key(KeyCode::Down));
    }
    paint(area, |area, buf, ctx| {
        completion.render(area, buf, ctx);
        let inner = Rect::new(30, 8, 20, 4);
        let snapshot = completion.list.snapshot(inner, completion.items(), |i| i);
        assert!(
            snapshot.more_above(),
            "scrolling hides the first rows above"
        );
        assert!(snapshot.rows().iter().any(|row| row.index == 6));
    });
    assert_eq!(
        completion.current().map(|item| item.label.as_str()),
        Some("cand06")
    );
    completion.set_items(labelled(12));
    assert_eq!(
        completion.current().map(|item| item.label.as_str()),
        Some("cand06"),
        "set_items keeps the highlighted row; open would reset it"
    );
}

#[test]
fn catalog_scenarios_execute() {
    for case in CASES {
        (case.run)();
    }
}

#[test]
fn catalog_scenario_ids_are_registered_once() {
    let mut seen = std::collections::BTreeSet::new();
    for case in CASES {
        assert!(
            seen.insert(case.id),
            "scenario {:?} is registered twice",
            case.id
        );
    }
}

#[test]
fn catalog_proofs_have_callable_cases() {
    use super::super::coverage::PROOFS;
    let registered: std::collections::BTreeSet<_> = CASES.iter().map(|case| case.id).collect();
    let routes: std::collections::BTreeSet<_> =
        super::NAV_ENTRIES.iter().map(|e| e.id as u8).collect();
    let mut apis = std::collections::BTreeSet::new();
    for proof in PROOFS {
        assert!(apis.insert(proof.api), "duplicate proof: {}", proof.api);
        assert!(
            !proof.scenarios.is_empty(),
            "{} names no scenario",
            proof.api
        );
        assert!(
            routes.contains(&(proof.route as u8)),
            "{} names a route that is not in the navigation",
            proof.api
        );
        for scenario in proof.scenarios {
            assert!(
                registered.contains(scenario),
                "{} names scenario {scenario:?} with no callable case",
                proof.api
            );
        }
    }
    for case in CASES {
        assert!(
            PROOFS
                .iter()
                .any(|proof| proof.scenarios.contains(&case.id)),
            "scenario {:?} proves no API",
            case.id
        );
    }
}

#[test]
fn catalog_all_routes_visible_once() {
    use super::{NAV_ENTRIES, NAV_GROUPS, PageId, entries, nav_roots};
    let file = syn::parse_file(include_str!("app.rs")).expect("app.rs parses");
    let declared = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == "PageId" => Some(e.variants.len()),
            _ => None,
        })
        .expect("PageId is declared");
    assert_eq!(
        NAV_ENTRIES.len(),
        declared,
        "every PageId has one navigation entry"
    );
    assert_eq!(
        entries().count(),
        NAV_ENTRIES.len(),
        "entries() returns each entry once"
    );
    let mut seen = std::collections::BTreeSet::new();
    for entry in NAV_ENTRIES {
        assert!(
            seen.insert(entry.id as u8),
            "{} is listed twice",
            entry.label
        );
        assert!(
            entry.group.is_empty() || NAV_GROUPS.contains(&entry.group),
            "{} names an unknown group",
            entry.label
        );
    }
    for (name, id) in [
        ("terminalpane", PageId::TerminalPane),
        ("fixedcontrols", PageId::FixedControls),
        ("terminalviewport", PageId::TerminalViewport),
        ("listdetail", PageId::ListDetail),
        ("fixeddialogs", PageId::FixedDialogs),
    ] {
        assert_eq!(
            PageId::from_name(name),
            Some(id),
            "the CLI route {name} stays"
        );
        assert_eq!(
            id.index(),
            NAV_ENTRIES.iter().position(|e| e.id == id).expect("listed")
        );
        assert_eq!(
            PageId::from_nav_path(&id.nav_path()),
            Some(id),
            "{name} is reachable from the sidebar"
        );
        assert!(
            !id.entry().group.is_empty(),
            "{name} sits in a sidebar group like every other component page"
        );
    }
    for entry in NAV_ENTRIES {
        assert_eq!(
            PageId::from_nav_path(&entry.id.nav_path()),
            Some(entry.id),
            "{}",
            entry.label
        );
    }
    assert_eq!(
        nav_roots()
            .iter()
            .filter(|r| matches!(r, super::NavRoot::Group(_)))
            .count(),
        NAV_GROUPS.len()
    );
}

#[test]
fn catalog_picker_setters_and_accessors_behave() {
    use junie_tui::core::hints::{HasHints, HintCx};
    use junie_tui::core::hit::Axis;
    use junie_tui::core::keymap::KeyBinding;
    use junie_tui::widgets::picker::{DEFAULT_GLYPH, PickerCmd, PickerKeyProfile};

    assert_eq!(
        (Picker::MAX_ROWS, Picker::WIDTH, DEFAULT_GLYPH),
        (12, 64, "·")
    );
    let id = WidgetId::of("scenario.accessors");
    let mut picker = Picker::new(id, "Choose")
        .choose_hint("Pick")
        .keymap(vec![(KeyBinding::key(KeyCode::Insert), PickerCmd::Toggle)]);
    assert!(!picker.is_multi());
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    picker.set_cursor(1);
    assert_eq!(picker.current_key(), Some(ItemKey::text("beta")));
    picker.set_chosen(Some(2));
    picker.set_chosen(Some(99));
    picker.set_width(40);
    picker.set_placement(Placement::Below);
    assert!(
        HasHints::hints(&picker, &HintCx::new(None))
            .iter()
            .any(|hint| hint.action == "Pick"),
        "choose_hint labels the choose key"
    );

    #[allow(deprecated)]
    let rendered = paint(Rect::new(0, 0, 80, 24), |area, buf, ctx| {
        picker.render(area, buf, ctx, "")
    });
    assert!(text_of(&rendered).contains("Choose"));
    assert_eq!(
        picker.on_click(id.sub("missing")),
        None,
        "an unknown id is no row"
    );
    assert_eq!(
        picker.on_click(picker.row_id(0)),
        Some(PickerAction::Chosen(0))
    );
    assert_eq!(
        picker.wheel(Axis::Vertical, 1),
        Outcome::Ignored,
        "three rows fit, so the wheel has nothing to scroll"
    );
    assert_eq!(picker.on_click_outside(), Some(PickerAction::Cancelled));

    let (outcome, action) = picker.on_paste("al");
    assert_eq!(
        (outcome, action),
        (Outcome::Changed, Some(PickerAction::QueryChanged))
    );
    assert_eq!(picker.caret(), 2, "the caret follows the pasted text");

    PickerKeyProfile::Vim.install();
    assert_eq!(PickerKeyProfile::current(), PickerKeyProfile::Vim);
    PickerKeyProfile::Desktop.install();
    assert_eq!(PickerKeyProfile::current(), PickerKeyProfile::Desktop);
}

fn status_notice() {
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: Cx<'_, ()> = Cx::new(&mut focus, &ring);
    cx.notice("Refresh failed", Tone::Warning);
    cx.status_in("Saved 3 rows", Tone::Secondary);
    let requests = cx.take();
    assert!(
        requests
            .iter()
            .any(|r| matches!(r, Request::Notice(text, Tone::Warning) if text == "Refresh failed")),
        "a page posts a notice through its Cx"
    );
    assert_eq!(
        background(requests).count(),
        2,
        "a notice is allowed from a screen that is not shown"
    );

    let mut line = StatusLine::<()>::new();
    line.notice_at("Sync paused", Tone::Secondary, Moment::ZERO);
    line.notice_at("Refresh failed", Tone::Warning, Moment::from_millis(10));
    assert_eq!(line.get(), Some(("Refresh failed", Tone::Warning)));
    line.set_at("Saved 3 rows", Tone::Secondary, Moment::from_millis(20));
    line.notice("Sync paused", Tone::Secondary);
    assert_eq!(
        line.text(),
        Some("Saved 3 rows"),
        "a notice never hides a post"
    );
    assert!(line.expire(Moment::from_millis(5_100)));
    assert_eq!(line.text(), None);
    line.notice("Sync paused", Tone::Secondary);
    assert_eq!(
        line.text(),
        Some("Sync paused"),
        "a free line takes a notice"
    );
}

fn picker_live_control() {
    let mut picker = Picker::new(WidgetId::of("scenario.live_rows"), "Choose")
        .width(40)
        .max_rows(2)
        .multi_select(true)
        .expect("an empty picker accepts multi-selection");
    let group = ItemKey::text("pair");
    picker
        .set_items(vec![
            Item::new("Alpha").key(ItemKey::text("alpha")),
            Item::new("Beta")
                .key(ItemKey::text("beta"))
                .selection_group(group),
            Item::new("Gamma")
                .key(ItemKey::text("gamma"))
                .selection_group(group),
            Item::new("Delta").key(ItemKey::text("delta")),
            Item::new("Epsilon").key(ItemKey::text("epsilon")),
        ])
        .expect("keyed items are accepted");
    let area = Rect::new(0, 0, 60, 16);
    let shown = |picker: &mut Picker| text_of(&paint(area, |a, b, c| picker.draw(a, b, c)));
    let two = shown(&mut picker);
    assert!(two.contains("Alpha") && !two.contains("Delta"), "{two}");
    picker.set_max_rows(4);
    let four = shown(&mut picker);
    assert!(
        four.contains("Delta") && !four.contains("Epsilon"),
        "{four}"
    );
    picker.set_max_rows(0);
    let one = shown(&mut picker);
    assert!(
        one.contains("Alpha") && !one.contains("Beta"),
        "zero rows means one row:\n{one}"
    );

    assert_eq!(picker.toggle_checked(0), None);
    assert_eq!(picker.toggle_checked(1), None);
    assert_eq!(
        picker.toggle_checked(2),
        Some(PickerAction::SelectionConflict {
            attempted: ItemKey::text("gamma"),
            selected: ItemKey::text("beta"),
        }),
        "a row of an occupied selection group is refused"
    );
    assert_eq!(
        picker.checked(),
        &[ItemKey::text("alpha"), ItemKey::text("beta")]
    );
    assert_eq!(picker.toggle_checked(0), None);
    assert_eq!(
        picker.checked(),
        &[ItemKey::text("beta")],
        "a second toggle unchecks"
    );
}

fn text_input_set_value() {
    let mut title = TextInput::new(WidgetId::of("scenario.title"), "Title")
        .required(true)
        .value("Q3 settlement report");
    title.begin_edit();
    title.on_key(&key(KeyCode::Char('X')));
    assert!(title.is_dirty(), "typing marks the field edited");
    title.set_error(Some("stale".to_owned()));

    title.set_value("Q3 settlement report");
    assert_eq!(title.text(), "Q3 settlement report");
    assert_eq!(title.error(), None, "a programmatic value clears the error");
    assert!(!title.is_dirty(), "the new value is the cancel baseline");
    assert!(!title.editing, "the field leaves edit mode");
    let buf = paint(Rect::new(0, 0, 60, 3), |area, buf, ctx| {
        title.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("Q3 settlement report"));
}

fn choice_editor_options() {
    let options = Options::new(["eu-west-1", "us-east-1", "ap-south-1"].map(|value| {
        let item = OptionItem::new(value);
        if value == "ap-south-1" {
            item.disabled("Region is not enabled")
        } else {
            item
        }
    }));
    let editor = ColumnEditor::Select(options.clone());
    let chooser = ChoiceEditor::open(
        WidgetId::of("scenario.chooser"),
        &editor,
        "us-east-1",
        &EditorConfig::default(),
    )
    .expect("a select column opens a choice editor");
    assert_eq!(
        chooser.options(),
        &options,
        "the editor reports the option set it was opened with"
    );
    assert!(chooser.options().0[2].is_disabled());
    assert!(
        ChoiceEditor::open(
            WidgetId::of("scenario.text"),
            &ColumnEditor::Text,
            "",
            &EditorConfig::default()
        )
        .is_none(),
        "a text column has no options to open"
    );
}

fn splitter_cancel_drag() {
    let mut seam = Splitter::new(WidgetId::of("scenario.seam"), SplitDir::Horizontal);
    let mut split = Split::new(40, 1, 1);
    let container = Rect::new(0, 0, 40, 5);
    seam.on_press(&split, container, 1, Position::new(20, 2));
    assert_eq!(
        seam.on_drag(&mut split, container, 1, Position::new(25, 2)),
        Outcome::Changed
    );
    let dragged = split;
    assert!(seam.cancel_drag(), "the held grab is dropped");
    assert!(!seam.cancel_drag(), "a second cancel finds nothing to drop");
    assert_eq!(split, dragged, "cancelling never undoes the drag");
}

static TONE_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static BLOCK_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn counted_tones(src: &str) -> Vec<(std::ops::Range<usize>, SyntaxTone)> {
    TONE_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    src.find("let")
        .map(|at| vec![(at..at + 3, SyntaxTone::Keyword)])
        .unwrap_or_default()
}

fn counted_blocks(src: &str) -> Vec<std::ops::Range<usize>> {
    BLOCK_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::iter::once(0..src.len()).collect()
}

fn runs(counter: &std::sync::atomic::AtomicUsize) -> usize {
    counter.load(std::sync::atomic::Ordering::Relaxed)
}

fn editor_frame(editor: &mut CodeEditor) {
    paint(Rect::new(0, 0, 60, 8), |area, buf, ctx| {
        editor.render(area, buf, ctx, ratatui::style::Color::Reset)
    });
}

fn code_editor_providers() {
    let mut editor = CodeEditor::new(WidgetId::of("scenario.code"), "let delay = 1;");
    assert!(editor.highlighter_fn().is_none() && editor.segmenter_fn().is_none());

    editor.set_highlighter(Some(counted_tones as Highlighter));
    editor.set_segmenter(Some(counted_blocks as Segmenter));
    assert!(editor.highlighter_fn().is_some() && editor.segmenter_fn().is_some());
    let (tones, blocks) = (runs(&TONE_RUNS), runs(&BLOCK_RUNS));
    editor_frame(&mut editor);
    assert_eq!(
        runs(&TONE_RUNS),
        tones + 1,
        "the first paint asks for tones"
    );
    assert_eq!(editor.blocks().len(), 1);
    assert_eq!(editor.blocks()[0], 0..14);
    assert_eq!(
        runs(&BLOCK_RUNS),
        blocks + 1,
        "the first read asks for blocks"
    );

    editor_frame(&mut editor);
    editor.blocks();
    assert_eq!(
        runs(&TONE_RUNS),
        tones + 1,
        "unchanged text reuses the tones"
    );
    assert_eq!(
        runs(&BLOCK_RUNS),
        blocks + 1,
        "unchanged text reuses the blocks"
    );

    editor.invalidate_highlighter();
    editor_frame(&mut editor);
    assert_eq!(
        runs(&TONE_RUNS),
        tones + 2,
        "an invalidated highlighter runs again"
    );
    assert_eq!(runs(&BLOCK_RUNS), blocks + 1, "and leaves the blocks alone");

    editor.invalidate_segmenter();
    editor.blocks();
    assert_eq!(
        runs(&BLOCK_RUNS),
        blocks + 2,
        "an invalidated segmenter runs again"
    );
    assert_eq!(runs(&TONE_RUNS), tones + 2, "and leaves the tones alone");

    editor.set_highlighter(None);
    editor.set_segmenter(None);
    assert!(editor.highlighter_fn().is_none() && editor.segmenter_fn().is_none());
    editor_frame(&mut editor);
    assert_eq!(runs(&TONE_RUNS), tones + 2, "no highlighter, no scan");
}

fn code_editor_diagnostics_find() {
    let src = "let delay = 1;\nsleep(delay);\nlet wide = \"日本\";\ndelay *= 2;";
    let mut editor = CodeEditor::new(WidgetId::of("scenario.find"), src);

    let warning = Diagnostic {
        range: 4..9,
        severity: Severity::Warning,
        message: "unused".into(),
    };
    let error = Diagnostic {
        range: 15..20,
        severity: Severity::Error,
        message: "unknown fn".into(),
    };
    editor.set_diagnostics(vec![warning.clone()]);
    editor.push_diagnostic(error.clone());
    assert_eq!(editor.diagnostics(), &[warning, error.clone()]);
    editor.set_diagnostics(vec![error.clone()]);
    assert_eq!(editor.diagnostics(), &[error]);
    editor.set_diagnostics(Vec::new());
    assert!(editor.diagnostics().is_empty());

    assert!(editor.find_state().is_none(), "find starts closed");
    editor.set_find_query("delay");
    let find = editor.find_state().expect("a query opens the find bar");
    assert_eq!(find.needle.text(), "delay");
    assert_eq!(find.matches.len(), 3);
    assert_eq!(find.current, 0);
    assert!(!find.editing, "the query is set by the host, not typed");
    editor.set_find_current(2);
    assert_eq!(editor.find_state().map(|f| f.current), Some(2));
    editor.set_find_current(99);
    assert_eq!(
        editor.find_state().map(|f| f.current),
        Some(2),
        "the current match clamps to the matches found"
    );
    editor.set_find_query("missing");
    let find = editor.find_state().expect("find stays open");
    assert!(find.matches.is_empty() && find.current == 0);
    editor.set_find_current(5);
    assert_eq!(editor.find_state().map(|f| f.current), Some(0));
    editor_frame(&mut editor);

    let buffer = TextBuffer::multi(src);
    assert_eq!(buffer.line_width(0), Some(14));
    assert_eq!(
        buffer.line_width(2),
        Some(18),
        "wide glyphs count two cells"
    );
    assert_eq!(
        buffer.line_width(9),
        None,
        "a line past the end has no width"
    );
}

/// The Activity page's channel feed: a real producer thread bursts past a
/// small bounded channel, retries on `Full` and loses nothing, while the
/// receiver carries the policy the page's "Turn budget" option shows.
fn external_feed_bounds() {
    use junie_tui::core::event::External;
    use junie_tui::runtime::{ExternalPolicy, ExternalSendError, bounded_external_channel};
    use std::num::NonZeroUsize;
    use std::time::{Duration, Instant};

    assert_eq!(
        (
            ExternalPolicy::DEFAULT_MAX_MESSAGES,
            ExternalPolicy::DEFAULT_MAX_DURATION
        ),
        (64, Duration::from_millis(2))
    );
    assert_eq!(
        ExternalPolicy::default(),
        ExternalPolicy {
            max_messages: ExternalPolicy::DEFAULT_MAX_MESSAGES,
            max_duration: ExternalPolicy::DEFAULT_MAX_DURATION,
        }
    );

    let policy = ExternalPolicy {
        max_messages: 16,
        max_duration: ExternalPolicy::DEFAULT_MAX_DURATION,
    };
    let capacity = NonZeroUsize::new(2).expect("nonzero");
    let (tx, rx) = bounded_external_channel(capacity, policy);
    assert_eq!(rx.policy(), policy, "the channel carries the page's budget");

    let feed = crate::feed::Feed::start(tx.clone(), 10, Duration::from_secs(3600));
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut seen = Vec::new();
    while seen.len() < 10 {
        assert!(Instant::now() < deadline, "feed stalled at {seen:?}");
        match rx.try_recv() {
            Some(message) => seen.push(
                message
                    .downcast_ref::<crate::feed::FeedLine>()
                    .expect("a feed line")
                    .seq,
            ),
            None => std::thread::yield_now(),
        }
    }
    assert_eq!(seen, (0..10).collect::<Vec<_>>(), "in order, none dropped");
    assert!(feed.full() > 0, "a burst of 10 overflows a capacity of 2");
    feed.stop();

    let mut fill = 0_u32;
    while tx.try_send(External::new(fill)).is_ok() {
        fill += 1;
    }
    let rejected = External::new(u32::MAX);
    match tx.try_send(rejected.clone()) {
        Err(ExternalSendError::Full(back)) => assert_eq!(back, rejected),
        other => panic!("a full queue answers Full, got {other:?}"),
    }
    while rx.try_recv().is_some() {}
    drop(rx);
    match tx.try_send(rejected.clone()) {
        Err(ExternalSendError::Closed(back)) => assert_eq!(back, rejected),
        other => panic!("a dropped receiver answers Closed, got {other:?}"),
    }
}

/// The Scrolling page's Virtual rows: a 10,000-row source paints only the
/// window, and the frame's scratch work stays bounded by the viewport.
fn scroll_window_work() {
    use junie_tui::ui::scroll_view::{Axes, ScrollView, ScrollWork, VisibleWindow};
    use ratatui::layout::Size;

    let screen = Rect::new(0, 0, 60, 14);
    let area = Rect::new(2, 2, 40, 8);
    let rows: u16 = 10_000;
    let id = WidgetId::of("scenario.scroll_window");
    let mut view = ScrollView::new(id).axes(Axes::VERTICAL);
    assert_eq!(view.last_work(), ScrollWork::default(), "no frame, no work");

    let mut painted = Vec::new();
    let mut window: Option<VisibleWindow> = None;
    let buf = paint(screen, |_, buf, ctx| {
        window = Some(view.render_window(
            screen,
            area,
            Size::new(40, rows),
            None,
            buf,
            ctx,
            |window, buf, _| {
                for index in window.content_rows() {
                    painted.push(index);
                    let rect = window.project_content_rect(Rect::new(0, index as u16, 40, 1));
                    buf.set_string(
                        rect.x,
                        rect.y,
                        format!("Row {index:05}"),
                        ratatui::style::Style::default(),
                    );
                }
            },
        ));
    });
    let window = window.expect("the window is returned");
    assert!(
        !painted.is_empty() && painted.len() <= usize::from(area.height),
        "only the window is painted: {painted:?}"
    );
    assert_eq!(painted.first(), Some(&0));
    assert!(text_of(&buf).contains("Row 00000") && !text_of(&buf).contains("Row 09999"));
    assert_eq!(
        window.project_content_rect(Rect::new(0, 3, 40, 1)).y,
        area.y + 3,
        "a model row maps onto the viewport"
    );

    let work = view.last_work();
    let viewport_cells = usize::from(area.width) * usize::from(area.height);
    assert!(work.allocated > 0, "a windowed frame allocates scratch");
    assert!(
        work.allocated <= viewport_cells && work.filled <= viewport_cells,
        "scratch follows the viewport, not the 10,000 rows: {work:?}"
    );
}

/// Layer focus policy on a real `ModalStack`: opening can preserve or move
/// focus, closing can restore or keep it, and `settle` drains every queued
/// layer result in order.
fn modal_focus_policy() {
    use junie_tui::core::event::Key as KeyEvent;
    use junie_tui::core::intent::DismissReason;
    use junie_tui::ui::layer::{Layer, LayerCx, LayerOut, LayerSpec, LayerTag, ShellLayers, Taken};
    use junie_tui::ui::modal::{FocusOnOpen, ModalStack};

    let (a, b, c, d) = (
        WidgetId::of("scenario.modal.a"),
        WidgetId::of("scenario.modal.b"),
        WidgetId::of("scenario.modal.c"),
        WidgetId::of("scenario.modal.d"),
    );
    let mut focus = Focus::default();
    focus.set(Some(a));
    let mut stack: ModalStack<u8> = ModalStack::new();

    stack.push_with_focus_policy(1, &mut focus, FocusOnOpen::Preserve, true);
    assert_eq!(focus.current(), Some(a), "Preserve leaves focus alone");
    focus.set(Some(b));
    stack.pop(&mut focus);
    assert_eq!(focus.current(), Some(a), "restore returns to the opener");

    stack.push_with_focus_policy(2, &mut focus, FocusOnOpen::Set(Some(c)), false);
    assert_eq!(focus.current(), Some(c), "Set moves focus on open");
    focus.set(Some(d));
    stack.pop(&mut focus);
    assert_eq!(focus.current(), Some(d), "no restore keeps the moved focus");

    stack.push_with_focus_policy(3, &mut focus, FocusOnOpen::Set(None), true);
    assert_eq!(focus.current(), None, "Set(None) clears focus on open");
    stack.pop(&mut focus);
    assert_eq!(focus.current(), Some(d));

    struct Queued(Vec<LayerOut<&'static str>>);

    impl Layer<&'static str> for Queued {
        fn spec(&self) -> LayerSpec {
            LayerSpec::tooltip()
        }
        fn render(&mut self, _: Rect, _: &mut Buffer, _: &mut RenderCtx, _: &()) {}
        fn on_key(&mut self, _: &KeyEvent, _: &mut LayerCx<'_, ()>) -> Outcome {
            Outcome::Ignored
        }
        fn on_dismiss(&mut self, _: DismissReason) {}
        fn take_result(&mut self) -> Option<LayerOut<&'static str>> {
            (!self.0.is_empty()).then(|| self.0.remove(0))
        }
    }

    struct Host {
        layers: ShellLayers<&'static str>,
        focus: Focus,
        seen: Vec<String>,
    }
    let mut host = Host {
        layers: ShellLayers::new(),
        focus: Focus::default(),
        seen: Vec::new(),
    };
    host.focus.set(Some(a));
    host.layers.open(
        Box::new(Queued(vec![
            LayerOut::Event("first"),
            LayerOut::Event("second"),
            LayerOut::Close("last"),
        ])),
        LayerTag::new("queued"),
        (),
        &mut host.focus,
    );
    assert_eq!(
        host.focus.current(),
        Some(a),
        "a passive layer opens with FocusOnOpen::Preserve"
    );
    let settle = |host: &mut Host| {
        ShellLayers::settle(
            host,
            |h| (&mut h.layers, &mut h.focus),
            Outcome::Ignored,
            |h, taken| {
                let (closed, text) = match taken {
                    Taken::Event(r) => (false, r),
                    Taken::Closed(_, r) => (true, r),
                };
                h.seen
                    .push(format!("{}{text}", if closed { "close " } else { "" }));
                Outcome::Changed
            },
        )
    };
    assert_eq!(settle(&mut host), Outcome::Changed);
    assert_eq!(host.seen, ["first", "second", "close last"]);
    assert!(host.layers.is_empty(), "the close popped the layer");
    assert_eq!(settle(&mut host), Outcome::Ignored, "nothing left to drain");
}

/// The Query workbench's document workspace, on a minimal document: pinned
/// and preview opens, activation, operations that only the live generation
/// may end, and close requests that only the live token may answer.
fn document_workspace_lifecycle() {
    use junie_tui::core::response::Response;
    use junie_tui::widgets::document_workspace::{
        CloseDecision, CloseToken, DocumentWorkspace, OpenKind, OperationToken, WorkspaceAction,
        WorkspaceDocument,
    };
    use junie_tui::widgets::tabs::Tabs;
    use ratatui::style::Color;
    use std::borrow::Cow;

    struct Note {
        title: String,
        text: String,
        opened: String,
        result: Option<String>,
    }

    /// The workspace's restricted facet: results, never the text.
    struct NoteView<'a> {
        note: &'a mut Note,
    }

    impl NoteView<'_> {
        fn show(&mut self, result: &str) {
            self.note.result = Some(result.to_owned());
        }
    }

    impl WorkspaceDocument for Note {
        type View<'a> = NoteView<'a>;

        fn title(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.title)
        }
        fn dirty(&self) -> bool {
            self.text != self.opened
        }
        fn focus_target(&self) -> Option<WidgetId> {
            None
        }
        fn view(&mut self) -> NoteView<'_> {
            NoteView { note: self }
        }
    }

    fn note(title: &str) -> Note {
        Note {
            title: title.to_owned(),
            text: String::new(),
            opened: String::new(),
            result: None,
        }
    }

    /// The transition's actions in order, as short words.
    fn acts(resp: Response<WorkspaceAction<Note>>) -> Vec<String> {
        resp.into_actions()
            .into_iter()
            .map(|action| match action {
                WorkspaceAction::Activated(key) => format!("activated {key:?}"),
                WorkspaceAction::PreviewPromoted(key) => format!("promoted {key:?}"),
                WorkspaceAction::CloseRequested(token) => format!("asked {:?}", token.document),
                WorkspaceAction::CancelOperation(token) => format!("cancel {:?}", token.document),
                WorkspaceAction::Closed { key, document } => {
                    format!("closed {key:?} {}", document.title)
                }
            })
            .collect()
    }

    fn edit(workspace: &mut DocumentWorkspace<Note>, key: ItemKey, text: &str) -> Vec<String> {
        let (_, resp) = workspace
            .update(key, |note| note.text = text.to_owned())
            .expect("an open document updates");
        acts(resp)
    }

    let mut workspace = DocumentWorkspace::new(WidgetId::of("scenario.workspace"));
    assert_eq!(workspace.len(), 0);
    assert_eq!(
        workspace.active_key(),
        None,
        "an empty workspace has no active document"
    );

    // Pinned opens append and activate; the first key is the strip's first tab.
    let (a, resp) = workspace.open_with(|_| note("a"), OpenKind::Pinned);
    assert_eq!(acts(resp), [format!("activated {a:?}")]);
    let (b, _) = workspace.open_with(|_| note("b"), OpenKind::Pinned);
    assert_eq!(workspace.ordered_keys(), [a, b]);
    assert_eq!((workspace.key_at(0), workspace.key_at(2)), (Some(a), None));
    assert_eq!((workspace.index_of(b), workspace.len()), (Some(1), 2));
    assert_eq!(workspace.get(b).map(|n| n.title.as_str()), Some("b"));
    assert_eq!(workspace.active_key(), Some(b));
    let titles: Vec<_> = workspace.iter().map(|(_, n)| n.title.as_str()).collect();
    assert_eq!(titles, ["a", "b"], "iter follows strip order");

    // Activation moves the active key; repeating it changes nothing.
    assert_eq!(acts(workspace.activate(a)), [format!("activated {a:?}")]);
    assert!(acts(workspace.activate(a)).is_empty());
    assert_eq!(workspace.active_key(), Some(a));

    // A clean preview is replaced by the next one: Closed comes before Activated.
    let (p, _) = workspace.open_with(|_| note("p"), OpenKind::Preview);
    let (q, resp) = workspace.open_with(|_| note("q"), OpenKind::Preview);
    assert_eq!(
        acts(resp),
        [format!("closed {p:?} p"), format!("activated {q:?}")]
    );
    assert_eq!(workspace.len(), 3, "the preview slot never grows the strip");
    assert_eq!(workspace.get(p).map(|n| n.title.as_str()), None);

    // Promote pins the preview once; anything else is a no-op.
    assert_eq!(acts(workspace.promote(q)), [format!("promoted {q:?}")]);
    assert!(acts(workspace.promote(q)).is_empty());
    assert!(acts(workspace.promote(a)).is_empty());
    let (r, _) = workspace.open_with(|_| note("r"), OpenKind::Preview);
    assert_eq!(
        edit(&mut workspace, r, "x"),
        [format!("promoted {r:?}")],
        "the first change pins a preview"
    );
    let (_, resp) = workspace.open_with(|_| note("s"), OpenKind::Preview);
    assert_eq!(
        workspace.len(),
        5,
        "a pinned r stays beside the new preview"
    );
    assert!(!acts(resp).iter().any(|action| action.starts_with("closed")));

    // An operation is ended only by its own generation.
    let (first, _): (OperationToken, _) =
        workspace.begin_operation(a).expect("an open document runs");
    assert_eq!(workspace.running(a), Some(first));
    let (second, resp) = workspace.begin_operation(a).expect("a rerun supersedes");
    assert_eq!(
        acts(resp),
        [format!("cancel {a:?}")],
        "the superseded run is reported"
    );
    assert_ne!(first, second);
    assert_eq!(
        workspace
            .end_operation(first, |mut view| view.show("stale"))
            .err(),
        Some(junie_tui::widgets::document_workspace::WorkspaceError::StaleOperation)
    );
    let (_, _) = workspace
        .end_operation(second, |mut view| view.show("25 rows"))
        .expect("the live generation ends");
    assert_eq!(workspace.running(a), None);
    assert_eq!(
        workspace.get(a).and_then(|n| n.result.as_deref()),
        Some("25 rows")
    );
    let mut view = workspace.view_mut(a).expect("an open document has a view");
    view.show("viewed");
    assert_eq!(
        workspace.get(a).and_then(|n| n.result.as_deref()),
        Some("viewed")
    );

    // Cancelling reports the token once, and its late result is refused.
    let (token, _) = workspace.begin_operation(b).expect("b runs");
    assert_eq!(
        acts(workspace.cancel_operation(b)),
        [format!("cancel {b:?}")]
    );
    assert!(acts(workspace.cancel_operation(b)).is_empty());
    assert!(
        workspace
            .end_operation(token, |mut view| view.show("late"))
            .is_err()
    );

    // The strip is rebuilt from the workspace, with the host's own marks.
    let (_, _) = workspace.begin_operation(b).expect("b runs again");
    let mut tabs = Tabs::with_items(WidgetId::of("scenario.workspace.tabs"), vec![]);
    workspace.sync_tabs_with(&mut tabs, |note, item| {
        item.prefix(&note.title.to_uppercase())
    });
    assert_eq!(tabs.items().len(), workspace.len());
    assert!(
        tabs.items()[1].busy && !tabs.items()[0].busy,
        "only b is busy"
    );
    let strip = paint(Rect::new(0, 0, 60, 3), |area, buf, ctx| {
        tabs.render(area, buf, ctx, Color::Reset)
    });
    assert!(
        text_of(&strip).contains("B b"),
        "the prefix decorates the tab"
    );
    let _ = workspace.cancel_operation(b);

    // A dirty document asks before it closes; a clean one closes at once.
    let clean = acts(workspace.request_close(q));
    assert_eq!(
        clean,
        [format!("closed {q:?} q")],
        "a clean document closes at once"
    );
    let ask = |workspace: &mut DocumentWorkspace<Note>, key| -> CloseToken {
        match workspace
            .request_close(key)
            .into_actions()
            .into_iter()
            .next()
        {
            Some(WorkspaceAction::CloseRequested(token)) => token,
            _ => panic!("the document should ask"),
        }
    };
    edit(&mut workspace, a, "draft");
    let token = ask(&mut workspace, a);
    assert_eq!(token.document, a);
    assert!(workspace.get(a).is_some(), "asking keeps the document");
    let _ = workspace
        .resolve_close(token, CloseDecision::Cancel)
        .expect("the live token answers");
    assert!(workspace.get(a).is_some(), "Cancel keeps the document");
    assert!(
        workspace
            .resolve_close(token, CloseDecision::Discard)
            .is_err(),
        "a token is answered once"
    );

    // A new request is a new token, and an edit makes it stale.
    let again = ask(&mut workspace, a);
    assert_ne!(again, token);
    edit(&mut workspace, a, "draft 2");
    assert!(
        workspace
            .resolve_close(again, CloseDecision::Discard)
            .is_err(),
        "an edit made the request stale"
    );

    // Discard stops the running work, then closes only its own document.
    let (_, _) = workspace.begin_operation(a).expect("a runs");
    let running = ask(&mut workspace, a);
    let closed = acts(
        workspace
            .resolve_close(running, CloseDecision::Discard)
            .expect("the live token discards"),
    );
    assert_eq!(
        closed.first(),
        Some(&format!("cancel {a:?}")),
        "its running work stops first"
    );
    assert!(
        closed
            .iter()
            .any(|action| action.starts_with(&format!("closed {a:?}")))
    );
    assert!(workspace.get(a).is_none() && workspace.get(b).is_some());
}

/// The Terminal pane page's session controller, driven directly: the area
/// the program needs, the attach, the replay window that follows it, the
/// program's title, bell and clipboard request, and what the host reads back.
fn session_attach_replay() {
    use crate::pages::terminal_session::{DemoFault, Script, controller};
    use junie_tui::terminal::{
        SessionClipboardRefusal, SessionConfig, SessionEffect, SessionLease, SessionStatus,
    };

    let (mut session, commands) = controller(Script::Owner, 64 * 1024);
    assert_eq!(session.status(), &SessionStatus::<DemoFault>::Pending);
    assert!(!session.live() && !session.has_output() && session.title().is_none());

    let min = SessionConfig::MINIMUM;
    session.sync_size(Rect::new(0, 0, min.cols - 3, min.rows - 1));
    assert!(session.too_small(), "below the minimum the session waits");
    assert_eq!(session.status(), &SessionStatus::Pending);
    assert!(!session.live());

    session.sync_size(Rect::new(0, 0, 41, 12));
    assert!(!session.too_small() && session.live());
    assert_eq!(session.status(), &SessionStatus::Attaching);
    assert_eq!(session.next_replay_deadline(), None, "no attach yet");

    assert!(session.pump(0), "the attach and the replay arrive");
    for now in [100, 200, 300, 400, 500] {
        session.pump(now);
    }
    assert_eq!(session.status(), &SessionStatus::Attaching);
    assert_eq!(session.next_replay_deadline(), Some(600));
    assert!(session.has_output(), "the replay painted the screen");
    session.pump(600);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner)
    );
    assert_eq!(session.next_replay_deadline(), None);
    assert_eq!(session.title(), Some("demo session"));
    assert_eq!(session_row(&session, 0), "demo$ ls");
    assert!(session.close_needs_confirm(), "an attached session asks");
    assert_eq!(session.dropped_replies(), 0);

    let mut now = 600;
    while now < 1_200 {
        now += 100;
        session.pump(now);
    }
    assert!(
        logged(&commands).contains(&"reply ESC [ 3 ; 5 R".to_owned()),
        "the owner answers the program's query: {:?}",
        logged(&commands)
    );
    while now < 1_500 {
        now += 100;
        session.pump(now);
    }
    assert!(
        session.attention(),
        "a bell while unfocused asks for a look"
    );
    session.focus_changed(true);
    assert!(!session.attention(), "focus clears the attention");
    while now < 2_000 {
        now += 100;
        session.pump(now);
    }
    assert_eq!(
        session.take_effects(),
        [SessionEffect::ClipboardRefused(
            SessionClipboardRefusal::Write
        )],
        "deny by default"
    );
    assert!(session.take_effects().is_empty(), "effects are read once");

    let (mut small, _) = controller(Script::Owner, 64 * 1024);
    small.set_config(SessionConfig {
        feed_budget_bytes: 8,
        ..SessionConfig::new(200)
    });
    small.sync_size(Rect::new(0, 0, 41, 12));
    for now in (0..=700).step_by(100) {
        small.pump(now);
    }
    assert_eq!(
        small.status(),
        &SessionStatus::Attaching,
        "a small drain budget keeps the replay window open"
    );
    assert_eq!(small.next_replay_deadline(), Some(800));
}

/// Typing, clicking, pasting and the wheel through the controller, and what
/// the link receives for each.
fn session_input_and_paste() {
    use crate::pages::terminal_session::{Script, controller};
    use junie_tui::terminal::{SessionKey, SessionPaste};

    let (mut session, commands) = controller(Script::Owner, 64 * 1024);
    let a = key(KeyCode::Char('a'));
    assert_eq!(session.key(&a, 0), SessionKey::Consumed, "no link yet");
    assert_eq!(session.paste("echo hello", 0), SessionPaste::Dropped);

    session.sync_size(Rect::new(0, 0, 41, 12));
    for now in (0..=700).step_by(100) {
        session.pump(now);
    }
    assert!(
        logged(&commands)
            .iter()
            .all(|line| !line.starts_with("input"))
    );
    assert_eq!(session.key(&a, 700), SessionKey::Forwarded);
    assert_eq!(
        logged(&commands).last().map(String::as_str),
        Some("input a")
    );

    session.gesture(700);
    assert_eq!(session.paste("echo hello", 700), SessionPaste::Sent);
    assert_eq!(
        session.paste("one\ntwo\nthree", 700),
        SessionPaste::NeedsConfirm { lines: 3 },
        "a multi-line paste waits for the operator"
    );
    let held = logged(&commands).len();
    assert_eq!(
        session.paste_confirmed("one\ntwo\nthree", 700),
        SessionPaste::Sent
    );
    assert_eq!(
        logged(&commands).len(),
        held + 1,
        "the confirmed paste went out"
    );

    assert_eq!(
        session.wheel(Axis::Vertical, -1),
        None,
        "off the alternate screen the viewport scrolls itself"
    );
    session.redraw();
    assert_eq!(logged(&commands).last().map(String::as_str), Some("redraw"));

    let sent = logged(&commands).len();
    session.clear();
    session.viewport_mut().reset();
    assert_eq!(session.viewport().offset(), 0);
    assert_eq!(session_row(&session, 0), "", "the host's screen is blank");
    assert_eq!(logged(&commands).len(), sent, "no byte reached the program");
}

/// How a session ends and how the host policy answers a program: detach,
/// exit, a failed or refused attach, a dying link, a viewer lease, and a
/// policy swapped while the program runs.
fn session_endings_and_policy() {
    use crate::pages::terminal_session::{DemoFault, Script, controller, host_policy};
    use junie_tui::terminal::{
        SessionDetach, SessionEffect, SessionKey, SessionLease, SessionPaste, SessionStatus,
    };

    let ready = |script: Script, until: i64| {
        let (mut session, commands) = controller(script, 64 * 1024);
        session.sync_size(Rect::new(0, 0, 41, 12));
        for now in (0..=until).step_by(100) {
            session.pump(now);
        }
        (session, commands)
    };
    let enter = key(KeyCode::Enter);

    let (mut session, _) = ready(Script::Detaches, 1_500);
    assert_eq!(
        session.status(),
        &SessionStatus::Detached(SessionDetach::Operator)
    );
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Detached(SessionDetach::Operator)]
    );
    assert_eq!(session.key(&enter, 1_600), SessionKey::Consumed);
    assert_eq!(
        session.status(),
        &SessionStatus::Attaching,
        "Enter reattaches"
    );

    let (mut session, _) = ready(Script::Exits, 1_500);
    assert_eq!(session.status(), &SessionStatus::Exited(Some(0)));
    assert_eq!(session.key(&enter, 1_600), SessionKey::Consumed);
    assert_eq!(
        session.status(),
        &SessionStatus::Attaching,
        "Enter restarts"
    );
    for now in (1_600..=2_300).step_by(100) {
        session.pump(now);
    }
    assert_eq!(session_row(&session, 0), "restarted");

    let (mut session, _) = ready(Script::Fails, 0);
    let busy = DemoFault("no capacity, the host is busy");
    assert_eq!(session.status(), &SessionStatus::Failed(busy.clone()));
    assert_eq!(session.take_effects(), [SessionEffect::Failed(busy)]);
    assert_eq!(session.key(&enter, 100), SessionKey::Consumed);
    assert_eq!(session.status(), &SessionStatus::Attaching, "Enter retries");

    let (mut session, _) = ready(Script::Refuses, 0);
    let refused = DemoFault("the other side refused the attach");
    assert_eq!(session.status(), &SessionStatus::Failed(refused.clone()));
    assert_eq!(session.take_effects(), [SessionEffect::Failed(refused)]);
    assert!(!session.has_output(), "a refused attach shows no screen");

    let (mut session, commands) = ready(Script::Severs, 2_000);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner),
        "the end has not been reported yet"
    );
    let sent = logged(&commands).len();
    assert_eq!(
        session.key(&key(KeyCode::Char('a')), 2_000),
        SessionKey::Consumed,
        "a closed link takes no key"
    );
    assert_eq!(session.paste("echo hello", 2_000), SessionPaste::Dropped);
    assert_eq!(logged(&commands).len(), sent, "nothing reached the link");
    for now in (2_100..=2_500).step_by(100) {
        session.pump(now);
    }
    assert_eq!(
        session.status(),
        &SessionStatus::Detached(SessionDetach::TransportLost)
    );
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Detached(SessionDetach::TransportLost)]
    );

    let (mut session, _) = ready(Script::Viewer, 700);
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::ReadOnly)
    );
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Notice(
            "another client controls this session".to_owned()
        )]
    );
    for now in (800..=2_500).step_by(100) {
        session.pump(now);
    }
    assert_eq!(
        session.status(),
        &SessionStatus::Attached(SessionLease::Owner),
        "the lease was handed over"
    );

    let (mut session, _) = ready(Script::Owner, 1_000);
    session.set_host_policy(host_policy(true));
    for now in (1_100..=2_000).step_by(100) {
        session.pump(now);
    }
    assert_eq!(
        session.take_effects(),
        [SessionEffect::Copy("SGVsbG8=".to_owned())],
        "the swapped policy lets the write through"
    );
}

fn logged(commands: &crate::pages::terminal_session::CommandLog) -> Vec<String> {
    commands.lock().expect("commands").clone()
}

fn session_row(
    session: &junie_tui::terminal::TerminalSessionController<
        crate::pages::terminal_session::DemoFault,
    >,
    row: u16,
) -> String {
    let view = session.viewport().state().view(0);
    (0..view.cols())
        .filter_map(|col| view.cell(row, col))
        .filter(|cell| !cell.is_wide_continuation())
        .map(|cell| match cell.text() {
            "" => " ".to_owned(),
            text => text.to_owned(),
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}
