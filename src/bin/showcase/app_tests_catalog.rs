//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::clock::Moment;
use junie_tui::core::data::{DataError, ShapeError, validate_rectangular};
use junie_tui::core::edit::{FieldKeys, TextCmd};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusChange, FocusRing};
use junie_tui::core::hit::{Axis, HitRegistry};
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keyed::{KeyedRecord, KeyedSourceError, KeyedState, KeyedVec, Reconcile};
use junie_tui::core::keymap::{Actions as _, KeyBinding};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::core::response::{
    Actions, ActionsIntoIter, ActionsIter, Flow, Invalidate, Response,
};
use junie_tui::core::selection::{ClickPolicy, RowSelection};
use junie_tui::core::text::TextBuffer;
use junie_tui::shell::{Cx, Request, StatusLine, background};
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, SyntaxTone, Theme, Tone};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::chips::{Chip, ChipAction, ChipBar};
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
use junie_tui::widgets::input::{TextInput, TextInput as FinderQuery};
use junie_tui::widgets::list::{
    ListAction, ListBox, ListItem, ListItemRef, ListSource, SelectMode,
};
use junie_tui::widgets::list_detail::{
    Action, DetailPane, Event, Frame, Line, ListDetail, ListDetailSource, ListRow, ListRowRef,
    MasterRow, PaneHost, SourceFrame,
};
use junie_tui::widgets::nav_list::{NavItem, NavList};
use junie_tui::widgets::option_editor::{
    ChoiceEditor, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::ScrollPanel;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::search_detail::{
    MissingSelection, QueryPasteMode, QueryRequest, QueryView, ResultRowKind, RowPaint,
    SearchAction, SearchCmd, SearchConfig, SearchDetail, SearchDetailSource, SearchError,
    SearchFocusMode, SearchPaint, SearchPhase, SearchSession, SearchSource, paint_query_default,
};
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::table::{
    Cell, CellRef, Column, DataTable, KeyedTable, KeyedTableError, TableAction, TableEditSource,
    TableSource,
};
use junie_tui::widgets::tabs::{TabItem, Tabs};
use junie_tui::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};
use junie_tui::widgets::tree::{
    KeyedTree, KeyedTreeNode, TreeAction, TreeNodeRef, TreeSource, TreeView,
};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Position, Rect};

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
    ScenarioCase { id: ScenarioId::ListDetailKeyedSource, run: list_detail_keyed_source },
    ScenarioCase { id: ScenarioId::ListDetailHostedPanes, run: list_detail_hosted_panes },
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
    ScenarioCase { id: ScenarioId::WorkCounterContract, run: work_counter_contract },
    ScenarioCase { id: ScenarioId::SearchDetailQuery, run: search_detail_query },
    ScenarioCase { id: ScenarioId::SearchDetailSelection, run: search_detail_selection },
    ScenarioCase { id: ScenarioId::SearchDetailPaint, run: search_detail_paint },
    ScenarioCase { id: ScenarioId::KeyedListSource, run: keyed_list_source },
    ScenarioCase { id: ScenarioId::NavItemReplacement, run: nav_item_replacement },
    ScenarioCase { id: ScenarioId::TableModelUpdates, run: table_model_updates },
    ScenarioCase { id: ScenarioId::KeyedTableStore, run: keyed_table_store },
    ScenarioCase { id: ScenarioId::KeyedTableSource, run: keyed_table_source },
    ScenarioCase { id: ScenarioId::KeyedTableEdit, run: keyed_table_edit },
    ScenarioCase { id: ScenarioId::KeyedTreeStore, run: keyed_tree_store },
    ScenarioCase { id: ScenarioId::KeyedTreeView, run: keyed_tree_view },
    ScenarioCase { id: ScenarioId::ChipBarEditing, run: chip_bar_editing },
    ScenarioCase { id: ScenarioId::ToolbarEditing, run: toolbar_editing },
    ScenarioCase { id: ScenarioId::TabsEditing, run: tabs_editing },
    ScenarioCase { id: ScenarioId::ScrollPanelContent, run: scroll_panel_content },
    ScenarioCase { id: ScenarioId::KeyedStoreIdentity, run: keyed_store_identity },
    ScenarioCase { id: ScenarioId::KeyedSourceRejection, run: keyed_source_rejection },
    ScenarioCase { id: ScenarioId::KeyedPointerPress, run: keyed_pointer_press },
    ScenarioCase { id: ScenarioId::ResponseBatching, run: response_batching },
    ScenarioCase { id: ScenarioId::DataShapeErrors, run: data_shape_errors },
    ScenarioCase { id: ScenarioId::TextBufferGeometry, run: text_buffer_geometry },
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
    paint_hits(area, draw).buf
}

/// One painted frame with the hit regions it registered, so a scenario can
/// aim a pointer event at what is on screen.
struct Painted {
    buf: Buffer,
    hits: HitRegistry,
}

impl Painted {
    fn event(&self, phase: Phase, owner: WidgetId, part: Part, at: Position) -> PointerEvent {
        let id = self
            .hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == owner && r.part.part == part && r.area.contains(at))
            .map(|r| r.id)
            .expect("a hit region at the position");
        PointerEvent::new(&self.hits, phase, id, at, 1, KeyModifiers::NONE)
    }
}

fn paint_hits(area: Rect, draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> Painted {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Painted { buf, hits }
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

// ---- Finder (SearchDetail) ------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Head,
    Item,
    Off,
    Cells,
    Gap,
}

type FinderRow = (u64, &'static str, Shape);

/// A heading, items (one disabled), a spacer and a row of two chips.
const FINDER_ROWS: &[FinderRow] = &[
    (1, "Group", Shape::Head),
    (10, "alpha", Shape::Item),
    (11, "beta", Shape::Off),
    (12, "gamma", Shape::Item),
    (2, "", Shape::Gap),
    (3, "chips", Shape::Cells),
    (13, "delta", Shape::Item),
    (14, "echo", Shape::Item),
];

const CHIPS: [&str; 2] = ["one", "two"];

fn chip_key(row: u64, cell: usize) -> ItemKey {
    ItemKey::num(row * 10 + cell as u64)
}

/// The answer a host builds for one query request.
struct FinderCat {
    stamp: SourceStamp,
    rows: Vec<FinderRow>,
}

impl FinderCat {
    fn new(request: &QueryRequest, rows: &[FinderRow]) -> Self {
        Self {
            stamp: request.token.result_stamp(),
            rows: rows.to_vec(),
        }
    }

    fn all_keys(&self) -> Vec<ItemKey> {
        let mut keys = Vec::new();
        for (key, _, shape) in &self.rows {
            keys.push(ItemKey::num(*key));
            if *shape == Shape::Cells {
                keys.extend((0..CHIPS.len()).map(|cell| chip_key(*key, cell)));
            }
        }
        keys
    }
}

impl RowSource for FinderCat {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn enabled(&self, i: usize) -> bool {
        self.rows[i].2 != Shape::Off
    }

    fn label(&self, i: usize) -> &str {
        self.rows[i].1
    }

    fn key(&self, i: usize) -> ItemKey {
        ItemKey::num(self.rows[i].0)
    }
}

impl KeyedRowSource for FinderCat {
    fn stamp(&self) -> SourceStamp {
        self.stamp
    }

    fn source_len(&self) -> usize {
        self.all_keys().len()
    }

    fn source_key(&self, i: usize) -> ItemKey {
        self.all_keys()[i]
    }
}

impl SearchSource for FinderCat {
    fn row_kind(&self, row: usize) -> ResultRowKind {
        match self.rows[row].2 {
            Shape::Head => ResultRowKind::Heading,
            Shape::Item | Shape::Off => ResultRowKind::Item,
            Shape::Cells => ResultRowKind::Cells,
            Shape::Gap => ResultRowKind::Spacer,
        }
    }

    fn cell_count(&self, row: usize) -> usize {
        match self.rows[row].2 {
            Shape::Head | Shape::Gap => 0,
            Shape::Item | Shape::Off => 1,
            Shape::Cells => CHIPS.len(),
        }
    }

    fn cell_key(&self, row: usize, cell: usize) -> ItemKey {
        match self.rows[row].2 {
            Shape::Cells => chip_key(self.rows[row].0, cell),
            _ => ItemKey::num(self.rows[row].0),
        }
    }

    fn cell_label(&self, row: usize, cell: usize) -> &str {
        match self.rows[row].2 {
            Shape::Cells => CHIPS[cell],
            _ => self.rows[row].1,
        }
    }

    fn cell_enabled(&self, row: usize, _cell: usize) -> bool {
        self.rows[row].2 != Shape::Off
    }
}

impl ListDetailSource for FinderCat {
    fn row(&self, visible: usize) -> ListRowRef<'_> {
        ListRowRef {
            mark: None,
            label: self.rows[visible].1,
            detail: "via-list-detail",
            meta: "",
            meta_tone: junie_tui::theme::Tone::Muted,
            faint: false,
        }
    }
}

fn finder_stamp(data: u64) -> SourceStamp {
    SourceStamp {
        instance: ItemKey::num(7_000),
        data,
        view: 0,
    }
}

fn finder_search(config: SearchConfig) -> SearchDetail {
    SearchDetail::new(
        WidgetId::of("scenario.fd"),
        FinderQuery::new(WidgetId::of("scenario.fd.query"), "Search"),
        Split::new(60, 28, 34),
    )
    .config(config)
}

fn actions_of(reply: Response<SearchAction>) -> Vec<SearchAction> {
    reply.into_actions().into_iter().collect()
}

fn requested(reply: Response<SearchAction>) -> QueryRequest {
    let mut found = actions_of(reply)
        .into_iter()
        .filter_map(|action| match action {
            SearchAction::QueryRequested(request) => Some(request),
            _ => None,
        });
    let request = found.next().expect("the input asked for one query request");
    assert!(found.next().is_none(), "exactly one request per change");
    request
}

/// Starts a search and answers its first request with `rows`.
fn finder_started(search: &mut SearchDetail, rows: &[FinderRow]) -> FinderCat {
    let request = requested(search.refresh_source(finder_stamp(1)));
    let cat = FinderCat::new(&request, rows);
    let _ = search.submit(request.token, &cat).expect("fresh results");
    cat
}

fn finder_key(
    search: &mut SearchDetail,
    cat: &FinderCat,
    focus: &mut Focus,
    code: KeyCode,
) -> Vec<SearchAction> {
    let reply = search
        .on_key(&key(code), cat, focus)
        .expect("the displayed results are the accepted ones");
    actions_of(reply)
}

fn finder_chord(
    search: &mut SearchDetail,
    cat: &FinderCat,
    focus: &mut Focus,
    code: KeyCode,
    mods: KeyModifiers,
) -> Vec<SearchAction> {
    let reply = search
        .on_key(&Key { code, mods }, cat, focus)
        .expect("the displayed results are the accepted ones");
    actions_of(reply)
}

fn num(n: u64) -> Option<ItemKey> {
    Some(ItemKey::num(n))
}

fn search_detail_query() {
    let mut search = finder_search(SearchConfig::default());
    let query_id = search.session().query_input().id;
    let _ = search.configure_query(|field| field.keys = FieldKeys::Desktop);
    search.set_query_readout("scope ‹ here ›");
    assert!(!search.editing());

    // The first request is for the empty query and nothing is accepted yet.
    let first = requested(search.refresh_source(finder_stamp(1)));
    assert_eq!(first.text, "");
    assert_eq!(search.phase(), SearchPhase::Loading);
    assert!(!search.session().is_interactive());
    assert_eq!(search.session().accepted_query(), None);

    // The answer carries the stamp the token asks for, and selects the first
    // enabled item.
    let cat = FinderCat::new(&first, FINDER_ROWS);
    assert_eq!(cat.stamp, first.token.result_stamp());
    let accepted = actions_of(search.submit(first.token, &cat).expect("fresh results"));
    assert!(accepted.contains(&SearchAction::SelectionChanged(num(10))));
    assert_eq!(search.phase(), SearchPhase::Ready);
    assert!(search.session().is_interactive());
    assert_eq!(search.session().accepted_query(), Some(""));
    assert_eq!(search.selected_key(), num(10));
    let session = search.session();
    assert!(session.is_selectable(ItemKey::num(10)));
    assert!(!session.is_selectable(ItemKey::num(11)), "a disabled item");
    assert!(!session.is_selectable(ItemKey::num(1)), "a heading");

    // A newer request makes the older token stale.
    let second = requested(search.set_query("al"));
    assert_eq!(second.text, "al");
    assert!(second.token.generation > first.token.generation);
    assert_eq!(
        search.submit(first.token, &cat).err(),
        Some(SearchError::StaleQuery)
    );

    // A failure keeps the old results on screen and Retry asks again.
    let _ = search
        .fail(
            second.token,
            "the ranking service did not answer".to_owned(),
        )
        .expect("the latest request can fail");
    assert_eq!(search.phase(), SearchPhase::Failed);
    assert_eq!(
        search.session().error(),
        Some("the ranking service did not answer")
    );
    let retry = requested(search.retry());
    assert_eq!(retry.text, "al");
    let again = FinderCat::new(&retry, FINDER_ROWS);
    let _ = search
        .submit(retry.token, &again)
        .expect("the retry answers");
    assert_eq!(search.phase(), SearchPhase::Ready);
    assert_eq!(search.session().error(), None);
    assert_eq!(search.session().accepted_query(), Some("al"));

    // Paste mode decides what a line break becomes.
    let mut focus = Focus::default();
    focus.focus(query_id);
    let _ = search.set_query("");
    let _ = search.on_paste("a\nb\n", &mut focus);
    assert_eq!(search.query(), "ab", "line breaks are stripped by default");
    search.config_mut().paste_mode = QueryPasteMode::JoinLinesWithSpaceTrim;
    let _ = search.set_query("");
    let _ = search.on_paste("a\nb\n", &mut focus);
    assert_eq!(search.query(), "a b", "line breaks join with a space");
    assert_eq!(search.session().query_input().field.text(), "a b");
    let mut joined =
        finder_search(SearchConfig::default().paste_mode(QueryPasteMode::JoinLinesWithSpaceTrim));
    let mut at = Focus::default();
    at.focus(joined.session().query_input().id);
    let _ = joined.on_paste("x\ny", &mut at);
    assert_eq!(joined.query(), "x y", "the builder sets the same policy");
    let mut stripped =
        finder_search(SearchConfig::default().paste_mode(QueryPasteMode::StripLineBreaks));
    let mut at = Focus::default();
    at.focus(stripped.session().query_input().id);
    let _ = stripped.on_paste("x\ny", &mut at);
    assert_eq!(stripped.query(), "xy");

    // Leaving the query ends editing.
    let _ = search.on_blur(FocusChange {
        from: query_id,
        to: None,
    });
    assert!(!search.editing());

    // The headless session answers an edit command that changed nothing.
    let mut session = SearchSession::new(
        FinderQuery::new(WidgetId::of("scenario.fd.session"), "Search"),
        SearchConfig::default(),
    );
    session.config_mut().select_first_on_query = false;
    let _ = session.configure_query(|field| field.keys = FieldKeys::Desktop);
    let request = requested(session.refresh_source(finder_stamp(1)));
    let _ = session
        .submit(request.token, &FinderCat::new(&request, FINDER_ROWS))
        .expect("the session accepts its own request");
    let undo = Key {
        code: KeyCode::Char('z'),
        mods: KeyModifiers::CONTROL,
    };
    let undone = actions_of(session.on_key(&undo, true, 5));
    assert!(
        undone.contains(&SearchAction::QueryCommand {
            cmd: TextCmd::Undo,
            effective: false
        }),
        "undo with nothing to undo reports that it did nothing: {undone:?}"
    );
    assert_eq!(session.query_input().field.text(), "");
    assert_eq!(session.accepted_query(), Some(""));
    assert!(session.is_selectable(ItemKey::num(12)));
    assert_eq!(session.error(), None);
}

fn search_detail_selection() {
    // Every command has a label, and the host owns the focus and detail ones.
    let all = [
        SearchCmd::FocusQuery,
        SearchCmd::FocusResults,
        SearchCmd::OpenDetail,
        SearchCmd::CloseDetail,
        SearchCmd::Activate,
        SearchCmd::Context,
        SearchCmd::Up,
        SearchCmd::Down,
        SearchCmd::Left,
        SearchCmd::Right,
        SearchCmd::Home,
        SearchCmd::End,
        SearchCmd::PageUp,
        SearchCmd::PageDown,
    ];
    assert!(all.iter().all(|cmd| !cmd.label().is_empty()));
    assert_eq!(all.iter().filter(|cmd| cmd.is_host_owned()).count(), 5);

    // Keys move over items and chips, skipping headings, spacers and the
    // disabled row, and Enter activates the selection.
    let mut search = finder_search(SearchConfig::default());
    let cat = finder_started(&mut search, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(search.id());
    let selected = |search: &mut SearchDetail, focus: &mut Focus, code| {
        finder_key(search, &cat, focus, code);
        search.selected_key()
    };
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Down), num(12));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Down), num(30));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Right), num(31));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Left), num(30));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Down), num(13));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::End), num(14));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Home), num(10));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Down), num(12));
    assert_eq!(selected(&mut search, &mut focus, KeyCode::Up), num(10));
    for code in [KeyCode::PageDown, KeyCode::PageUp] {
        let landed = selected(&mut search, &mut focus, code).expect("a selection");
        assert!(search.session().is_selectable(landed));
    }
    selected(&mut search, &mut focus, KeyCode::Home);
    let ran = finder_key(&mut search, &cat, &mut focus, KeyCode::Enter);
    assert!(ran.contains(&SearchAction::Activate(ItemKey::num(10))));

    // Ctrl+O opens the preview, which the host closes again; Ctrl+F returns
    // to the query.
    finder_chord(
        &mut search,
        &cat,
        &mut focus,
        KeyCode::Char('o'),
        KeyModifiers::CONTROL,
    );
    assert!(search.on_detail(focus.current()));
    assert_eq!(focus.current(), Some(search.detail_id()));
    search.close_detail(&mut focus);
    assert!(!search.on_detail(focus.current()));
    finder_chord(
        &mut search,
        &cat,
        &mut focus,
        KeyCode::Char('f'),
        KeyModifiers::CONTROL,
    );
    assert_eq!(focus.current(), Some(search.session().query_input().id));

    // The session reports preview actions and items against the selection.
    let mut session = SearchSession::new(
        FinderQuery::new(WidgetId::of("scenario.fd.keys"), "Search"),
        SearchConfig::default(),
    );
    let request = requested(session.refresh_source(finder_stamp(1)));
    let _ = session
        .submit(request.token, &FinderCat::new(&request, FINDER_ROWS))
        .expect("accepted");
    assert_eq!(
        actions_of(session.detail_item(2)),
        vec![SearchAction::DetailItem {
            key: ItemKey::num(10),
            index: 2
        }]
    );
    assert_eq!(
        actions_of(session.detail_action("why")),
        vec![SearchAction::DetailAction {
            key: ItemKey::num(10),
            name: "why"
        }]
    );
    session.cancel_press();

    // The lost-selection policy.
    let nearest = lost_selection(MissingSelection::NearestPosition);
    let first = lost_selection(MissingSelection::FirstEnabled);
    assert_eq!(first, num(10), "the first enabled key");
    assert_eq!(
        nearest,
        num(12),
        "the nearest enabled key to where the selection was"
    );

    // Focus owner: in Separate the query and the results own focus apart, so
    // a letter typed over the results edits nothing.
    let mut apart = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Separate));
    let cat = finder_started(&mut apart, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(apart.id());
    let _ = finder_key(&mut apart, &cat, &mut focus, KeyCode::Char('a'));
    assert_eq!(apart.query(), "", "the results do not take typing");
    focus.focus(apart.session().query_input().id);
    let _ = finder_key(&mut apart, &cat, &mut focus, KeyCode::Char('a'));
    assert_eq!(apart.query(), "a", "the query does");

    // Focus owner: in Combined one owner takes typing and navigation.
    let mut combined = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let cat = finder_started(&mut combined, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(combined.id());
    let typed = finder_key(&mut combined, &cat, &mut focus, KeyCode::Char('a'));
    assert_eq!(combined.query(), "a");
    let request = typed
        .into_iter()
        .find_map(|action| match action {
            SearchAction::QueryRequested(request) => Some(request),
            _ => None,
        })
        .expect("typing asked for results");
    let answer = FinderCat::new(&request, FINDER_ROWS);
    let _ = combined.submit(request.token, &answer).expect("accepted");
    let _ = finder_key(&mut combined, &answer, &mut focus, KeyCode::Down);
    assert_eq!(combined.selected_key(), num(12), "Down still navigates");

    // Typing from the preview returns to the query only when it is allowed.
    for (allowed, text) in [(false, ""), (true, "x")] {
        let mut search = finder_search(SearchConfig::default().type_to_query_from_detail(allowed));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.detail_id());
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Char('x'));
        assert_eq!(
            search.query(),
            text,
            "type_to_query_from_detail = {allowed}"
        );
        assert_eq!(
            focus.current() == Some(search.session().query_input().id),
            allowed
        );
    }

    // A keymap override replaces the default bindings.
    let mut keymap = SearchCmd::default_keymap();
    keymap.push((KeyBinding::ctrl('n'), SearchCmd::Down));
    keymap.push((KeyBinding::ctrl('w'), SearchCmd::CloseDetail));
    let mut emacs = finder_search(SearchConfig::default().keymap(keymap));
    let cat = finder_started(&mut emacs, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(emacs.id());
    finder_chord(
        &mut emacs,
        &cat,
        &mut focus,
        KeyCode::Char('n'),
        KeyModifiers::CONTROL,
    );
    assert_eq!(emacs.selected_key(), num(12));
    let closed = Key {
        code: KeyCode::Char('w'),
        mods: KeyModifiers::CONTROL,
    };
    let closed = emacs
        .on_key(&closed, &cat, &mut focus)
        .expect("the displayed results are the accepted ones");
    assert!(closed.is_consumed(), "a bound CloseDetail is handled");

    // Moving while a request is pending, and the first item of a new query.
    for (moves, expected) in [(false, num(10)), (true, num(12))] {
        let mut search = finder_search(SearchConfig::default().navigate_while_loading(moves));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.id());
        let _ = search.set_query("z");
        assert_eq!(search.phase(), SearchPhase::Loading);
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Down);
        assert_eq!(
            search.selected_key(),
            expected,
            "navigate_while_loading = {moves}"
        );
    }
    for (first, expected) in [(true, num(10)), (false, num(12))] {
        let mut search = finder_search(SearchConfig::default().select_first_on_query(first));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.id());
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Down);
        let request = requested(search.set_query("g"));
        let answer = FinderCat::new(&request, FINDER_ROWS);
        let _ = search.submit(request.token, &answer).expect("accepted");
        assert_eq!(
            search.selected_key(),
            expected,
            "select_first_on_query = {first}"
        );
    }
}

/// Where the selection lands when the last row of the results disappears.
fn lost_selection(policy: MissingSelection) -> Option<ItemKey> {
    let mut search = finder_search(SearchConfig::default().missing_selection(policy));
    let cat = finder_started(&mut search, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(search.id());
    let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::End);
    assert_eq!(search.selected_key(), num(14));
    let request = requested(search.refresh_source(finder_stamp(2)));
    let fewer = &FINDER_ROWS[..4];
    let _ = search
        .submit(request.token, &FinderCat::new(&request, fewer))
        .expect("accepted");
    search.selected_key()
}

/// Draws the host's own look for every pane.
#[derive(Default)]
struct HostPaint {
    query: bool,
    rows: Vec<usize>,
    chips: usize,
    empty: bool,
    detail: bool,
    summary: bool,
}

impl SearchPaint<FinderCat> for HostPaint {
    fn query(&mut self, area: Rect, view: QueryView<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.query = true;
        paint_query_default(area, view, buf, ctx);
    }

    fn row(&mut self, mut row: RowPaint<'_, FinderCat>, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.rows.push(row.row);
        let rect = row.rect;
        if row.kind == ResultRowKind::Cells {
            for cell in 0..row.source.cell_count(row.row) {
                assert_ne!(
                    row.cell_hit(cell),
                    row.hit,
                    "a chip has a region of its own"
                );
                let at = Rect::new(rect.x + 3 + cell as u16 * 8, rect.y, 6, 1);
                buf.set_string(
                    at.x,
                    at.y,
                    format!("[{}]", row.source.cell_label(row.row, cell)),
                    ratatui::style::Style::default(),
                );
                row.place_cell(cell, at);
                self.chips += 1;
            }
        } else {
            buf.set_string(
                rect.x + 3,
                rect.y,
                row.source.label(row.row),
                ratatui::style::Style::default(),
            );
        }
    }

    fn empty(&mut self, area: Rect, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.empty = true;
        buf.set_string(
            area.x + 1,
            area.y,
            "host empty",
            ratatui::style::Style::default(),
        );
    }

    fn detail(&mut self, pane: DetailPane<'_>, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.detail = true;
        buf.set_string(
            pane.area.x + 1,
            pane.area.y,
            "host detail",
            ratatui::style::Style::default(),
        );
    }

    fn summary(&mut self, area: Rect, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.summary = true;
        buf.set_string(
            area.x + 1,
            area.y,
            "host summary",
            ratatui::style::Style::default(),
        );
    }
}

fn preview_frame(lines: &[Line]) -> SourceFrame<'_> {
    SourceFrame {
        title: "Results",
        meta: "5",
        head: &[],
        under: None,
        detail_title: "Preview",
        detail_meta: "",
        lines,
        actions: &[],
    }
}

fn preview_lines(count: usize) -> Vec<Line> {
    (1..=count)
        .map(|n| Line::Text(format!("entry {n:02}"), junie_tui::theme::Tone::Muted))
        .collect()
}

/// Paints the standard look and returns the screen with its hit regions.
fn draw_source<S: SearchDetailSource + ?Sized>(
    search: &mut SearchDetail,
    cat: &S,
    area: Rect,
    lines: &[Line],
    focus: &Focus,
) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        search
            .render_source(area, cat, &preview_frame(lines), &mut buf, &mut ctx)
            .expect("the accepted results are drawn");
    }
    (buf, hits)
}

/// A press, then a click, on the row of `key`.
fn click_row(
    search: &mut SearchDetail,
    cat: &FinderCat,
    hits: &HitRegistry,
    focus: &mut Focus,
    key: u64,
    count: u8,
) -> Vec<SearchAction> {
    let target = *hits
        .regions()
        .iter()
        .rev()
        .find(|r| {
            r.owner == search.id()
                && r.part.part == Part::ROW
                && r.part.item == Some(ItemKey::num(key))
        })
        .expect("the row has a hit region");
    let at = Position::new(target.area.x + 4, target.area.y);
    let mut actions = Vec::new();
    for phase in [Phase::Press, Phase::Click] {
        let event = PointerEvent::new(hits, phase, target.id, at, count, KeyModifiers::NONE);
        actions.extend(actions_of(
            search.pointer(&event, cat, focus).expect("accepted"),
        ));
    }
    actions
}

/// Scrolls the preview down, changes the results the way `change` says, and
/// reports whether the preview is back at its top.
fn preview_back_at_top(config: SearchConfig, new_query: bool) -> bool {
    let area = Rect::new(0, 0, 120, 16);
    let lines = preview_lines(40);
    let mut search = finder_search(config).detail_share(50, 40, 70);
    let cat = finder_started(&mut search, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(search.id());
    let _ = draw_source(&mut search, &cat, area, &lines, &focus);
    finder_chord(
        &mut search,
        &cat,
        &mut focus,
        KeyCode::Char('o'),
        KeyModifiers::CONTROL,
    );
    assert!(search.on_detail(focus.current()));
    let _ = draw_source(&mut search, &cat, area, &lines, &focus);
    for _ in 0..30 {
        finder_key(&mut search, &cat, &mut focus, KeyCode::Down);
    }
    let scrolled = text_of(&draw_source(&mut search, &cat, area, &lines, &focus).0);
    assert!(
        scrolled.contains("entry 39") && !scrolled.contains("entry 01"),
        "the preview scrolled to its end:\n{scrolled}"
    );
    let request = if new_query {
        requested(search.set_query("g"))
    } else {
        requested(search.refresh_source(finder_stamp(2)))
    };
    let answer = FinderCat::new(&request, FINDER_ROWS);
    let _ = search.submit(request.token, &answer).expect("accepted");
    let after = text_of(&draw_source(&mut search, &answer, area, &lines, &focus).0);
    after.contains("entry 01")
}

fn search_detail_paint() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let wide = Rect::new(0, 0, 120, 24);

    // The standard look through the shared list rows, then a pointer press and
    // a double click on a row.
    let mut search = finder_search(SearchConfig::default())
        .collapse_below(90)
        .detail_share(50, 40, 70)
        .summary_rows(3);
    let cat = finder_started(&mut search, FINDER_ROWS);
    let lines = preview_lines(30);
    let mut focus = Focus::default();
    focus.focus(search.id());
    let (buf, hits) = draw_source(&mut search, &cat, wide, &lines, &focus);
    let shown = text_of(&buf);
    assert!(shown.contains("alpha") && shown.contains("gamma") && shown.contains("Preview"));
    // `render_source` accepts only a `SearchDetailSource`. FinderCat gets that
    // through the blanket impl from its `SearchSource` and `ListDetailSource`
    // impls, and the rows come from the latter: the detail text exists nowhere
    // else, so seeing it proves the controller drew a source it was handed.
    assert!(
        shown.contains("via-list-detail"),
        "the standard rows read ListDetailSource::row: {shown}"
    );
    let clicked = click_row(&mut search, &cat, &hits, &mut focus, 12, 2);
    assert!(clicked.contains(&SearchAction::SelectionChanged(num(12))));
    assert!(clicked.contains(&SearchAction::Activate(ItemKey::num(12))));

    // A single click runs the row when the click policy says so, and only
    // selects it under the default policy.
    let mut single = finder_search(SearchConfig::default().click(ClickPolicy::NAVIGATION));
    let cat = finder_started(&mut single, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(single.id());
    let (_, hits) = draw_source(&mut single, &cat, wide, &lines, &focus);
    let ran = click_row(&mut single, &cat, &hits, &mut focus, 12, 1);
    assert!(
        ran.contains(&SearchAction::Activate(ItemKey::num(12))),
        "{ran:?}"
    );
    let mut double = finder_search(SearchConfig::default());
    let cat = finder_started(&mut double, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(double.id());
    let (_, hits) = draw_source(&mut double, &cat, wide, &lines, &focus);
    let selected = click_row(&mut double, &cat, &hits, &mut focus, 12, 1);
    assert!(
        !selected.contains(&SearchAction::Activate(ItemKey::num(12))),
        "{selected:?}"
    );
    assert_eq!(double.selected_key(), num(12));

    // The preview keeps its scroll while the same item answers again, and
    // returns to its top only when the config says so.
    assert!(!preview_back_at_top(SearchConfig::default(), true));
    assert!(!preview_back_at_top(SearchConfig::default(), false));
    assert!(preview_back_at_top(
        SearchConfig::default().reset_preview_on_query(true),
        true
    ));
    assert!(preview_back_at_top(
        SearchConfig::default().preserve_preview_on_same_key(false),
        false
    ));

    // A keyboard context request waits for a paint, then anchors to the row.
    let mut menu = finder_search(SearchConfig::default());
    let cat = finder_started(&mut menu, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(menu.id());
    let asked = finder_chord(
        &mut menu,
        &cat,
        &mut focus,
        KeyCode::F(10),
        KeyModifiers::SHIFT,
    );
    assert!(
        asked.is_empty(),
        "no row is placed yet, so nothing opens: {asked:?}"
    );
    let _ = draw_source(&mut menu, &cat, wide, &lines, &focus);
    let anchored = actions_of(menu.emit_pending_context());
    assert!(
        anchored.iter().any(
            |a| matches!(a, SearchAction::ContextRequested(r) if r.target == ItemKey::num(10))
        ),
        "the paint released the pending request: {anchored:?}"
    );

    // The host's own row, chip and query painters.
    let mut combined = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let cat = finder_started(&mut combined, FINDER_ROWS);
    let mut buf = Buffer::empty(wide);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut chips = Vec::new();
    let mut queried = false;
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        combined
            .render_source_with(
                wide,
                &cat,
                &preview_frame(&lines),
                &mut buf,
                &mut ctx,
                |mut row: RowPaint<'_, FinderCat>, buf, _ctx| {
                    let rect = row.rect;
                    if row.kind == ResultRowKind::Cells {
                        for cell in 0..row.source.cell_count(row.row) {
                            let at = Rect::new(rect.x + 3 + cell as u16 * 8, rect.y, 6, 1);
                            buf.set_string(
                                at.x,
                                at.y,
                                row.source.cell_label(row.row, cell),
                                ratatui::style::Style::default(),
                            );
                            row.place_cell(cell, at);
                            chips.push(row.cell_hit(cell));
                        }
                    } else if row.kind == ResultRowKind::Item {
                        buf.set_string(
                            rect.x + 3,
                            rect.y,
                            row.source.label(row.row).to_uppercase(),
                            ratatui::style::Style::default(),
                        );
                    }
                },
                |area, view, buf, ctx| {
                    queried = true;
                    paint_query_default(area, view, buf, ctx);
                },
            )
            .expect("the accepted results are drawn");
    }
    let shown = text_of(&buf);
    assert!(queried && shown.contains("ALPHA") && shown.contains("one") && shown.contains("two"));
    assert_eq!(chips.len(), 2);
    assert!(
        chips
            .iter()
            .all(|id| hits.regions().iter().any(|r| r.id == *id)),
        "each placed chip registered its region"
    );

    // Every pane in the host's hands, with no answer yet, then answered, then
    // too narrow for the preview.
    let mut hosted = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let mut paint = HostPaint::default();
    let render =
        |search: &mut SearchDetail, source: &FinderCat, area: Rect, paint: &mut HostPaint| {
            let mut buf = Buffer::empty(area);
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            search
                .render_hosted(area, source, &mut buf, &mut ctx, paint)
                .expect("the accepted results are drawn");
            text_of(&buf)
        };
    let request = requested(hosted.refresh_source(finder_stamp(1)));
    let answer = FinderCat::new(&request, FINDER_ROWS);
    let blank = render(&mut hosted, &answer, wide, &mut paint);
    assert!(
        paint.empty && blank.contains("host empty"),
        "nothing is listed before an answer"
    );
    let _ = hosted.submit(request.token, &answer).expect("accepted");
    let drawn = render(&mut hosted, &answer, wide, &mut paint);
    assert!(paint.query && paint.detail && drawn.contains("host detail"));
    assert_eq!(paint.rows.len(), FINDER_ROWS.len());
    assert_eq!(paint.chips, 2);
    assert!(drawn.contains("[one]") && drawn.contains("gamma"));
    let narrow = render(&mut hosted, &answer, Rect::new(0, 0, 60, 24), &mut paint);
    assert!(paint.summary && narrow.contains("host summary"));
}

/// Records the list reads in place and identifies by label, never by position.
struct Records {
    items: Vec<ListItem>,
    reversed: bool,
}

impl Records {
    fn new(labels: &[&str]) -> Self {
        Self {
            items: labels.iter().map(|label| ListItem::new(label)).collect(),
            reversed: false,
        }
    }

    fn at(&self, visible: usize) -> usize {
        if self.reversed {
            self.items.len() - 1 - visible
        } else {
            visible
        }
    }
}

impl RowSource for Records {
    fn len(&self) -> usize {
        self.items.len()
    }

    fn enabled(&self, i: usize) -> bool {
        !self.items[self.at(i)].disabled
    }

    fn label(&self, i: usize) -> &str {
        &self.items[self.at(i)].label
    }

    fn key(&self, i: usize) -> ItemKey {
        ItemKey::text(self.label(i))
    }
}

impl KeyedRowSource for Records {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: ItemKey::text("scenario.records"),
            data: 0,
            view: u64::from(self.reversed),
        }
    }

    fn source_len(&self) -> usize {
        self.items.len()
    }

    fn source_key(&self, source_index: usize) -> ItemKey {
        ItemKey::text(&self.items[source_index].label)
    }
}

impl ListSource for Records {
    fn item(&self, visible: usize) -> ListItemRef<'_> {
        self.items[self.at(visible)].borrowed()
    }
}

fn keyed_list_source() {
    const AREA: Rect = Rect::new(0, 0, 40, 10);
    let beta = ItemKey::text("Beta");
    let mut source = Records::new(&["Alpha", "Beta", "Gamma", "Delta"]);
    let mut list = ListBox::new(
        WidgetId::of("scenario.keyed.list"),
        vec![],
        SelectMode::Single,
    )
    .click(ClickPolicy::NAVIGATION);
    let draw = |list: &mut ListBox, source: &Records| {
        paint_hits(AREA, |area, buf, ctx| {
            list.render_source(area, source, buf, ctx, ctx.theme.canvas)
                .expect("unique keys are a valid source");
        })
    };
    assert!(text_of(&draw(&mut list, &source).buf).contains("Alpha"));

    let _ = list
        .on_key_source(&key(KeyCode::Down), &source)
        .expect("valid source");
    let out = list
        .on_key_source(&key(KeyCode::Enter), &source)
        .expect("valid source");
    assert_eq!(
        out.actions().copied().collect::<Vec<_>>(),
        vec![ListAction::Activated(beta), ListAction::SelectionChanged],
        "Enter reports the chosen row by key, then the changed selection"
    );
    assert_eq!(list.keyed().selected().collect::<Vec<_>>(), vec![beta]);

    source.reversed = true;
    let frame = draw(&mut list, &source);
    assert_eq!(list.keyed().cursor(), Some(beta));
    assert_eq!(
        list.chosen,
        Some(2),
        "the choice follows its key to the new row"
    );
    assert!(text_of(&frame.buf).contains("Delta"));

    let at = Position::new(6, 0);
    let press = frame.event(Phase::Press, list.id, Part::ROW, at);
    let _ = list
        .on_pointer_source(&press, &source)
        .expect("valid source");
    let click = frame.event(Phase::Click, list.id, Part::ROW, at);
    let out = list
        .on_pointer_source(&click, &source)
        .expect("valid source");
    assert!(
        out.actions()
            .any(|a| *a == ListAction::Activated(ItemKey::text("Delta"))),
        "a click activates the row under the pointer, by key"
    );

    let mut multi = ListBox::new(
        WidgetId::of("scenario.keyed.multi"),
        vec![],
        SelectMode::Multi,
    );
    draw(&mut multi, &source);
    let out = multi
        .on_key_source(&key(KeyCode::Char(' ')), &source)
        .expect("valid source");
    assert!(out.actions().any(|a| *a == ListAction::SelectionChanged));
    assert_eq!(multi.keyed().selected().count(), 1);
}

fn nav_item_replacement() {
    let items = |labels: &[&str]| labels.iter().map(|l| NavItem::new(l)).collect::<Vec<_>>();
    let mut nav = NavList::new(
        WidgetId::of("scenario.nav"),
        items(&["Tasks", "Runs", "Logs"]),
    );
    nav.on_key(&key(KeyCode::End));
    assert_eq!(nav.cursor, 2);
    assert!(
        nav.set_items(items(&["Tasks", "Runs"])),
        "a new set replaces the old"
    );
    assert_eq!(nav.cursor, 1, "the cursor is repaired to the last row left");
    assert!(
        !nav.set_items(items(&["Tasks", "Runs"])),
        "an equal set leaves all state untouched"
    );
    assert!(nav.set_items(vec![]));
    assert!(nav.items.is_empty());
    assert!(nav.set_items(items(&["Home", "Search"])));
    assert_eq!(
        (nav.cursor, nav.current),
        (0, 0),
        "an empty list starts on its first row"
    );
    let buf = paint(Rect::new(0, 0, 30, 8), |area, buf, ctx| {
        nav.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("Search"));
}

fn cells(texts: &[&str]) -> Vec<Cell> {
    texts.iter().map(|text| Cell::new(*text)).collect()
}

fn two_columns() -> Vec<Column> {
    vec![
        Column::new("Task", Constraint::Min(12)),
        Column::new("Owner", Constraint::Length(8)),
    ]
}

fn table_model_updates() {
    use junie_tui::core::data::DataError;
    let mut table = DataTable::try_new(
        WidgetId::of("scenario.table"),
        two_columns(),
        vec![cells(&["lint", "ana"]), cells(&["build", "kai"])],
    )
    .expect("rectangular rows");
    assert_eq!((table.columns().len(), table.rows().len()), (2, 2));
    assert!(
        DataTable::try_new(
            WidgetId::of("scenario.bad"),
            two_columns(),
            vec![cells(&["x"])]
        )
        .is_err(),
        "a short row is refused at construction"
    );

    table
        .try_set_rows(vec![
            cells(&["lint", "ana"]),
            cells(&["build", "kai"]),
            cells(&["ship", "mira"]),
        ])
        .expect("rectangular rows");
    assert_eq!(table.rows().len(), 3);
    assert!(table.try_set_rows(vec![cells(&["x"])]).is_err());
    assert_eq!(
        table.rows().len(),
        3,
        "a refused replacement changes nothing"
    );

    table
        .try_set_row(1, cells(&["build", "sofia"]))
        .expect("a full row replaces row 1");
    assert_eq!(table.rows()[1][1].text, "sofia");
    assert_eq!(
        table.try_set_row(7, cells(&["a", "b"])),
        Err(DataError::MissingRow(7))
    );
    assert!(table.try_set_row(0, cells(&["a"])).is_err());

    assert!(table.update_cell(0, 1, |cell| cell.text = "jonas".into()));
    assert_eq!(table.rows()[0][1].text, "jonas");
    assert!(
        !table.update_cell(9, 0, |_| {}),
        "a missing cell is refused"
    );

    assert_eq!(
        table.update_column(0, |column| column.title = "Summary".into()),
        Ok(true)
    );
    assert_eq!(
        table.update_column(0, |_| {}),
        Ok(false),
        "no change reports false"
    );
    assert_eq!(table.update_column(7, |_| {}), Err(7));
    let frame = paint(Rect::new(0, 0, 40, 8), |area, buf, ctx| {
        table.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&frame).contains("Summary"));

    table
        .try_set_model(
            vec![Column::new("Name", Constraint::Min(10))],
            vec![cells(&["lint"]), cells(&["build"])],
        )
        .expect("the new model is rectangular");
    assert_eq!((table.columns().len(), table.rows().len()), (1, 2));
    assert!(
        table
            .try_set_model(
                vec![Column::new("Name", Constraint::Min(10))],
                vec![cells(&["a", "b"])]
            )
            .is_err()
    );
    assert_eq!(table.columns().len(), 1, "a refused model changes nothing");
}

fn tasks_store() -> KeyedTable {
    let k = ItemKey::num;
    KeyedTable::try_new(
        vec![
            Column::new("Task", Constraint::Length(12)).editable(),
            Column::new("Owner", Constraint::Length(10)).editable(),
        ],
        [
            (k(1), cells(&["alpha", "ann"])),
            (k(2), cells(&["bravo", "bob"])),
            (k(3), cells(&["charlie", "cy"])),
        ],
    )
    .expect("rectangular, unique rows")
}

fn keyed_table_store() {
    let k = ItemKey::num;
    let mut store = tasks_store();
    assert!(!store.is_empty());
    assert_eq!(store.len(), 3);
    assert_eq!(store.row_keys().collect::<Vec<_>>(), vec![k(1), k(2), k(3)]);
    assert_eq!(
        store.row(k(2)).map(|row| row[0].text.as_str()),
        Some("bravo")
    );

    store
        .insert(1, k(9), cells(&["delta", "dee"]))
        .expect("a new key in a full row");
    assert_eq!(
        store.row_keys().collect::<Vec<_>>(),
        vec![k(1), k(9), k(2), k(3)]
    );
    assert_eq!(
        store.insert(0, k(1), cells(&["dup", "x"])),
        Err(KeyedTableError::Key(
            KeyedSourceError::DuplicateAuthoritative { key: k(1) }
        ))
    );
    assert!(matches!(
        store.insert(0, k(8), cells(&["short"])),
        Err(KeyedTableError::Shape(_))
    ));
    assert_eq!(store.len(), 4, "a refused insert changes nothing");

    assert!(store.move_row(k(3), 0));
    assert_eq!(store.row_keys().next(), Some(k(3)));
    assert!(store.update_cell(k(2), 0, |cell| cell.text = "bravo!".into()));
    assert!(
        !store.update_cell(k(2), 5, |_| {}),
        "a missing column is refused"
    );
    assert_eq!(
        store.remove(k(9)).map(|row| row[0].text.clone()),
        Some("delta".to_owned())
    );
    assert!(store.remove(k(9)).is_none());

    store
        .replace_rows([
            (k(7), cells(&["one", "uno"])),
            (k(8), cells(&["two", "dos"])),
        ])
        .expect("rectangular, unique rows");
    assert_eq!(store.row_keys().collect::<Vec<_>>(), vec![k(7), k(8)]);
    assert!(store.replace_rows([(k(1), cells(&["x"]))]).is_err());

    let borrowed: CellRef<'_> = TableSource::cell(&store, 1, 0);
    assert_eq!(borrowed.text, "two");
    let owned = Cell::new("note");
    assert_eq!(owned.borrowed().text, "note");
}

fn keyed_table_source() {
    const AREA: Rect = Rect::new(0, 0, 40, 10);
    let k = ItemKey::num;
    let mut store = tasks_store();
    let mut table = DataTable::try_new(WidgetId::of("scenario.keyed.table"), vec![], vec![])
        .expect("an empty table");
    let draw = |table: &mut DataTable, store: &KeyedTable| {
        paint_hits(AREA, |area, buf, ctx| {
            table
                .render_source(area, store, buf, ctx, ctx.theme.canvas)
                .expect("unique keys are a valid source");
        })
    };
    assert!(text_of(&draw(&mut table, &store).buf).contains("bravo"));

    let _ = table
        .on_key_source(&key(KeyCode::Down), &store)
        .expect("valid source");
    assert_eq!(table.keyed().cursor(), Some(k(2)));
    let out = table
        .on_key_source(&key(KeyCode::Enter), &store)
        .expect("valid source");
    assert!(
        out.actions().any(|a| *a == TableAction::Activated(k(2))),
        "Enter activates the row under the cursor, by key"
    );

    store.move_row(k(2), 0);
    let frame = draw(&mut table, &store);
    assert_eq!(
        table.keyed().cursor(),
        Some(k(2)),
        "the cursor follows its row"
    );

    let at = Position::new(8, 2);
    let press = frame.event(Phase::Press, table.id, Part::CELL, at);
    let _ = table
        .on_pointer_source(&press, &store)
        .expect("valid source");
    assert_eq!(
        table.keyed().pressed(),
        Some(k(1)),
        "the press is held by key"
    );
    let click = frame.event(Phase::Click, table.id, Part::CELL, at);
    let _ = table
        .on_pointer_source(&click, &store)
        .expect("valid source");
    assert_eq!(table.keyed().cursor(), Some(k(1)));
}

fn keyed_table_edit() {
    const AREA: Rect = Rect::new(0, 0, 40, 10);
    let k = ItemKey::num;
    let mut store = tasks_store();
    let mut table = DataTable::try_new(WidgetId::of("scenario.keyed.edit"), vec![], vec![])
        .expect("an empty table")
        .cell_nav(true);
    let draw = |table: &mut DataTable, store: &mut KeyedTable| {
        paint_hits(AREA, |area, buf, ctx| {
            table
                .render_edit_source(area, store, buf, ctx, ctx.theme.canvas)
                .expect("unique keys are a valid source");
        })
    };
    draw(&mut table, &mut store);
    let _ = table
        .on_key_edit_source(&key(KeyCode::Down), &mut store)
        .expect("valid source");
    let _ = table
        .on_key_edit_source(&key(KeyCode::Enter), &mut store)
        .expect("valid source");
    assert_eq!(
        table.keyed().edit_key(),
        Some(k(2)),
        "the edit starts on the row's key"
    );

    store.move_row(k(2), 0);
    draw(&mut table, &mut store);
    assert_eq!(
        table.keyed().edit_key(),
        Some(k(2)),
        "a reorder keeps the edit target"
    );
    let _ = table
        .on_key_edit_source(&key(KeyCode::Char('!')), &mut store)
        .expect("valid source");
    let out = table
        .on_key_edit_source(&key(KeyCode::Enter), &mut store)
        .expect("valid source");
    assert_eq!(
        out.actions().copied().collect::<Vec<_>>(),
        vec![TableAction::Committed { row: k(2), col: 0 }],
        "the commit names the edited row by key"
    );
    assert_eq!(
        store.row(k(2)).map(|row| row[0].text.as_str()),
        Some("bravo!")
    );
    assert_eq!(
        store.row(k(1)).map(|row| row[0].text.as_str()),
        Some("alpha")
    );

    assert_eq!(store.set_cell(k(1), 1, "zed".into()), Ok(()));
    assert!(
        store.set_cell(k(99), 0, "x".into()).is_err(),
        "a removed row is refused"
    );

    let frame = draw(&mut table, &mut store);
    let at = Position::new(8, 2);
    let press = frame.event(Phase::Press, table.id, Part::CELL, at);
    let _ = table
        .on_pointer_edit_source(&press, &mut store)
        .expect("valid source");
    let click = frame.event(Phase::Click, table.id, Part::CELL, at);
    let _ = table
        .on_pointer_edit_source(&click, &mut store)
        .expect("valid source");
    assert_eq!(
        table.keyed().pressed(),
        None,
        "the click consumed the press"
    );

    // Focus leaving mid-edit: the blur policy commits the typed text to the
    // row captured when the edit began, even after a reorder.
    let _ = table
        .on_key_edit_source(&key(KeyCode::Down), &mut store)
        .expect("valid source");
    let _ = table
        .on_key_edit_source(&key(KeyCode::Enter), &mut store)
        .expect("valid source");
    let target = table.keyed().edit_key().expect("an edit is open");
    let _ = table
        .on_key_edit_source(&key(KeyCode::Char('?')), &mut store)
        .expect("valid source");
    store.move_row(target, 0);
    draw(&mut table, &mut store);
    let out = table.on_blur_edit_source(&mut store).expect("valid source");
    assert_eq!(
        out.actions().copied().collect::<Vec<_>>(),
        vec![TableAction::Committed {
            row: target,
            col: 0
        }],
        "the blur commit names the edited row by key"
    );
    assert!(!table.is_editing(), "the blur closed the editor");
    assert_eq!(table.keyed().edit_key(), None, "no stale edit target");
}

fn project_store() -> KeyedTree {
    let k = ItemKey::num;
    let mut tree = KeyedTree::new();
    assert!(tree.is_empty());
    tree.insert(None, 0, k(1), KeyedTreeNode::new("src"))
        .expect("a root");
    tree.insert(
        Some(k(1)),
        0,
        k(11),
        KeyedTreeNode::new("main.rs").meta("1.2 KB").glyph("◆"),
    )
    .expect("a child");
    tree.insert(Some(k(1)), 1, k(12), KeyedTreeNode::new("lib.rs"))
        .expect("a child");
    tree.insert(None, 1, k(2), KeyedTreeNode::lazy("target"))
        .expect("a lazy root");
    tree
}

fn keyed_tree_store() {
    let k = ItemKey::num;
    let mut tree = project_store();
    assert_eq!((tree.len(), tree.is_empty()), (4, false));
    assert_eq!(
        tree.get(k(1)).map(|node| node.children().to_vec()),
        Some(vec![k(11), k(12)])
    );
    assert!(tree.get(k(2)).is_some_and(|node| node.lazy));

    assert_eq!(
        tree.insert(None, 0, k(11), KeyedTreeNode::new("dup")),
        Err(KeyedSourceError::DuplicateAuthoritative { key: k(11) })
    );
    assert_eq!(
        tree.insert(Some(k(99)), 0, k(5), KeyedTreeNode::new("orphan")),
        Err(KeyedSourceError::MissingNode { key: k(99) })
    );
    assert!(tree.update(k(11), |node| node.label = "app.rs".into()));
    assert!(!tree.update(k(99), |_| {}));
    let src: TreeNodeRef<'_> = TreeSource::node(&tree, k(1)).expect("a served node");
    assert_eq!((src.label, src.children.len()), ("src", 2));
    let app = TreeSource::node(&tree, k(11)).expect("a served node");
    assert_eq!(
        (app.label, app.meta, app.glyph),
        ("app.rs", Some("1.2 KB"), Some("◆"))
    );
    assert_eq!(tree.root_count(), 2);

    assert!(tree.remove(k(1)), "removing a folder removes its subtree");
    assert_eq!(tree.len(), 1);
    assert!(tree.get(k(11)).is_none());
    assert!(!tree.remove(k(1)));
}

fn keyed_tree_view() {
    const AREA: Rect = Rect::new(0, 0, 40, 10);
    let k = ItemKey::num;
    let mut store = project_store();
    let mut view = TreeView::new(WidgetId::of("scenario.keyed.tree"), vec![])
        .select_follows_cursor(true)
        .show_selection(false);
    let draw = |view: &mut TreeView, store: &KeyedTree| {
        paint_hits(AREA, |area, buf, ctx| {
            view.render_source(area, store, buf, ctx, ctx.theme.canvas)
                .expect("a valid keyed tree");
        })
    };
    assert!(text_of(&draw(&mut view, &store).buf).contains("main.rs"));
    assert!(view.keyed().is_expanded(k(1)), "roots start open");

    let _ = view
        .on_key_source(&key(KeyCode::Down), &store)
        .expect("valid source");
    assert_eq!(view.keyed().cursor(), Some(k(11)));
    assert!(
        view.keyed().is_selected(k(11)),
        "select_follows_cursor selects the leaf the cursor lands on"
    );
    let out = view
        .on_key_source(&key(KeyCode::Enter), &store)
        .expect("valid source");
    assert!(out.actions().any(|a| *a == TreeAction::Activate(k(11))));

    assert!(view.set_cursor_source(k(12), &store).expect("valid source"));
    assert!(!view.set_cursor_source(k(99), &store).expect("valid source"));
    assert!(view.select_source(k(12), &store).expect("valid source"));
    assert!(!view.select_source(k(99), &store).expect("valid source"));
    assert_eq!(view.keyed().selected().collect::<Vec<_>>(), vec![k(12)]);

    store
        .insert(None, 0, k(9), KeyedTreeNode::new("aaa"))
        .expect("an earlier sibling");
    draw(&mut view, &store);
    assert_eq!(
        view.keyed().cursor(),
        Some(k(12)),
        "an earlier sibling moves no cursor"
    );
    assert!(view.keyed().is_expanded(k(1)));

    view.expand_all_source(&store).expect("valid source");
    let frame = draw(&mut view, &store);
    let at = Position::new(6, 1);
    let press = frame.event(Phase::Press, view.id, Part::ROW, at);
    let _ = view
        .on_pointer_source(&press, &store)
        .expect("valid source");
    let click = frame.event(Phase::Click, view.id, Part::ROW, at);
    let _ = view
        .on_pointer_source(&click, &store)
        .expect("valid source");
    assert_eq!(view.keyed().pressed(), None, "the click consumed the press");
}

/// A keyed list-detail over a `KeyedVec` of rows: the selection is a key, so it
/// follows its job through a reorder, a click reaches the key that was drawn,
/// and the list reports its own paging without a frame.
fn list_detail_keyed_source() {
    const MASTER: WidgetId = WidgetId::of("scenario.keyed.master");
    const DETAIL: WidgetId = WidgetId::of("scenario.keyed.detail");
    let k = ItemKey::num;
    let mut store = KeyedVec::try_from_rows(
        [(1, "alpha"), (2, "bravo"), (3, "charlie")]
            .map(|(n, label)| (k(n), ListRow::new(label).meta("ok", Tone::Success))),
    )
    .expect("unique keys");
    let owned = ListRow::new("alpha").meta("ok", Tone::Success);
    let shown: ListRowRef<'_> = owned.borrowed();
    assert_eq!((shown.label, shown.meta), ("alpha", "ok"));
    assert_eq!(
        ListDetailSource::row(&store, 1).label,
        "bravo",
        "a keyed store lends its rows in place"
    );

    let area = Rect::new(0, 0, 120, 10);
    let frame = SourceFrame {
        title: "Jobs",
        meta: "3 listed",
        head: &[],
        under: None,
        detail_title: "Detail",
        detail_meta: "",
        lines: &[],
        actions: &[],
    };
    let draw = |ld: &mut ListDetail, store: &KeyedVec<ListRow>| {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        ld.render_source(area, store, &frame, &mut buf, &mut ctx)
            .expect("a valid source renders");
        (buf, hits)
    };
    let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
    let (buf, hits) = draw(&mut ld, &store);
    let first = text_of(&buf);
    assert!(
        first.contains("alpha") && first.contains("charlie"),
        "{first}"
    );
    assert_eq!(ld.selected_key(), Some(k(1)));
    assert_eq!(ld.keyed().cursor(), Some(k(1)));
    assert_eq!(ld.master_offset(), 0, "a short list is not scrolled");
    assert!(ld.master_page() >= 3, "page keys move by the visible rows");
    assert!(!ld.drawer_open(), "a wide area shows the detail beside it");

    ld.select(1);
    store.move_to(k(2), 0);
    draw(&mut ld, &store);
    assert_eq!(
        (ld.cursor, ld.selected_key()),
        (0, Some(k(2))),
        "the view index follows the key through a reorder"
    );

    ld.select(2);
    store.move_to(k(3), 0);
    ld.bind_source(&store).expect("a valid source binds");
    assert_eq!(
        ld.selected_key(),
        Some(k(3)),
        "binding adopts a moved cursor by key without painting"
    );

    let row = hits
        .regions()
        .iter()
        .rev()
        .find(|r| r.area.contains(Position::new(4, 2)) && r.id != MASTER)
        .map(|r| r.id)
        .expect("a master row hit");
    store.move_to(k(2), 2);
    draw(&mut ld, &store);
    let (_, event) = ld.click_source(row, &mut Focus::default());
    assert_eq!(event, Some(Event::Row(2)));
    assert_eq!(
        ld.selected_key(),
        Some(k(2)),
        "a click reaches the key that was drawn, not the row now at that place"
    );
}

/// A list-detail whose panes the host draws, selection owned by the host: the
/// list hands over keyed rows and the detail pane, and still routes the detail
/// keys, the wheel and the drawer.
fn list_detail_hosted_panes() {
    const MASTER: WidgetId = WidgetId::of("scenario.hosted.master");
    const DETAIL: WidgetId = WidgetId::of("scenario.hosted.detail");
    let k = ItemKey::num;

    #[derive(Default)]
    struct Host {
        rows: Vec<(usize, ItemKey, bool)>,
        details: Vec<(bool, usize)>,
        summaries: usize,
    }
    impl PaneHost for Host {
        fn row(&mut self, row: MasterRow, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
            self.rows.push((row.index, row.key, row.selected));
        }
        fn empty(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}
        fn detail(&mut self, pane: DetailPane<'_>, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
            self.details.push((pane.drawer, pane.scroll.offset));
            pane.scroll.set_content(100);
            pane.scroll.set_viewport(5);
        }
        fn summary(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
            self.summaries += 1;
        }
    }

    let wide = Rect::new(0, 0, 120, 10);
    let narrow = Rect::new(0, 0, 60, 10);
    let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
    let hosted = |ld: &mut ListDetail, area: Rect, host: &mut Host| {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        ld.render_hosted(
            area,
            3,
            |i| k(i as u64 + 1),
            Some(1),
            &mut buf,
            &mut ctx,
            host,
        );
        hits
    };

    let mut host = Host::default();
    let hits = hosted(&mut ld, wide, &mut host);
    assert_eq!(
        host.rows,
        [(0, k(1), false), (1, k(2), true), (2, k(3), false)],
        "every visible row arrives with its key and the host's selection"
    );
    assert_eq!(host.details, [(false, 0)], "a wide area splits beside");
    assert_eq!(ld.master_offset(), 0);
    assert!(ld.master_page() >= 1 && !ld.drawer_open());

    let mut focus = Focus::default();
    focus.focus(DETAIL);
    assert_eq!(
        ld.on_detail_key(&key(KeyCode::PageDown), &mut focus)
            .outcome(),
        Outcome::Changed,
        "a detail key scrolls the host's pane"
    );
    let wheel = PointerEvent::new(
        &hits,
        Phase::Move,
        DETAIL,
        Position::new(100, 3),
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(ld.wheel_event(&wheel, 2), Outcome::Changed);
    let mut scrolled = Host::default();
    hosted(&mut ld, wide, &mut scrolled);
    assert!(scrolled.details[0].1 > 0, "{:?}", scrolled.details);
    ld.reset_detail();
    let mut reset = Host::default();
    hosted(&mut ld, wide, &mut reset);
    assert_eq!(reset.details[0].1, 0, "a new selection starts at the top");

    let mut summary = Host::default();
    hosted(&mut ld, narrow, &mut summary);
    assert!(summary.details.is_empty() && summary.summaries == 1);
    ld.open_detail(&mut focus);
    let mut drawer = Host::default();
    hosted(&mut ld, narrow, &mut drawer);
    assert!(ld.drawer_open());
    assert!(drawer.details[0].0, "the pane reports it is a drawer");

    let rows = KeyedVec::try_from_rows(
        [(1, "alpha"), (2, "bravo"), (3, "charlie")].map(|(n, l)| (k(n), ListRow::new(l))),
    )
    .expect("unique keys");
    let frame = SourceFrame {
        title: "Rows",
        meta: "3",
        head: &[],
        under: None,
        detail_title: "Detail",
        detail_meta: "",
        lines: &[],
        actions: &[],
    };
    let mut painted = Vec::new();
    let mut plain = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(wide);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    plain.render_source_selected_with(
        wide,
        &rows,
        &frame,
        Some(2),
        &mut buf,
        &mut ctx,
        |source, row: MasterRow, _buf, _ctx| {
            painted.push((
                ListDetailSource::row(source, row.index).label.to_owned(),
                row.selected,
            ));
        },
    );
    assert_eq!(
        painted,
        [
            ("alpha".to_owned(), false),
            ("bravo".to_owned(), false),
            ("charlie".to_owned(), true)
        ],
        "the caller paints each row, and the selection is the one supplied"
    );
    assert_eq!(
        plain.selected_key(),
        None,
        "the list's own state is untouched"
    );
}

fn chip_bar_editing() {
    let mut bar = ChipBar::new(WidgetId::of("scenario.chips"));
    assert!(bar.chips().is_empty());
    assert!(bar.set_chips(vec![Chip::new("alpha"), Chip::new("beta")]));
    assert!(bar.insert_chip(2, Chip::new("gamma")));
    assert!(!bar.insert_chip(9, Chip::new("past the end")));
    assert!(bar.update_chip(1, |chip| chip.enabled = false));
    assert!(!bar.update_chip(1, |chip| chip.enabled = false));
    assert_eq!(
        bar.remove_chip(0).map(|chip| chip.label),
        Some("alpha".to_owned())
    );
    assert!(bar.remove_chip(9).is_none());
    let labels: Vec<&str> = bar.chips().iter().map(|chip| chip.label.as_str()).collect();
    assert_eq!(labels, ["beta", "gamma"]);
    assert!(!bar.chips()[0].enabled);

    assert_eq!(bar.lead(), None);
    assert!(bar.set_lead(Some("match all".to_owned())));
    assert_eq!(bar.lead(), Some("match all"));
    assert_eq!(bar.add_label(), Some("+ Add filter"));
    assert!(bar.set_add_label(Some("+ New".to_owned())));
    assert!(!bar.set_add_label(Some("+ New".to_owned())));
    let draw = |bar: &mut ChipBar| {
        paint(Rect::new(0, 0, 70, 1), |area, buf, ctx| {
            bar.render(area, buf, ctx, ctx.theme.canvas)
        })
    };
    let text = text_of(&draw(&mut bar));
    for shown in ["match all", "beta", "gamma", "+ New"] {
        assert!(text.contains(shown), "{shown} is drawn:\n{text}");
    }
    assert!(!text.contains("+ Add filter"));
    let (_, action) = bar.on_key(&key(KeyCode::Char('+')));
    assert_eq!(action, Some(ChipAction::Add));

    assert!(bar.set_add_label(None));
    let text = text_of(&draw(&mut bar));
    assert!(!text.contains("+ New"), "the add action is gone:\n{text}");
    let (_, action) = bar.on_key(&key(KeyCode::Char('+')));
    assert_eq!(action, None, "a bar without an add label cannot add");
}

fn toolbar_editing() {
    let items = || {
        vec![
            ToolItem::new("Run"),
            ToolItem::new("Stop").reason("No query is running"),
        ]
    };
    let mut bar = Toolbar::new(WidgetId::of("scenario.toolbar"), items());
    assert_eq!(bar.items().len(), 2);
    assert!(bar.insert_item(1, ToolItem::new("Run all")));
    assert!(!bar.insert_item(9, ToolItem::new("past the end")));
    assert!(bar.update_item(2, |item| {
        item.disabled = false;
        item.reason = None;
    }));
    assert!(!bar.update_item(2, |item| item.disabled = false));
    assert_eq!(
        bar.remove_item(0).map(|item| item.label),
        Some("Run".to_owned())
    );
    assert!(bar.remove_item(9).is_none());
    let labels: Vec<&str> = bar.items().iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["Run all", "Stop"]);
    assert!(!bar.items()[1].disabled, "Stop was enabled in place");

    assert!(bar.set_items(items()));
    assert!(
        !bar.set_items(items()),
        "an equal collection changes nothing"
    );
    let text = text_of(&paint(Rect::new(0, 0, 40, 1), |area, buf, ctx| {
        bar.render(area, buf, ctx, ctx.theme.canvas)
    }));
    assert!(text.contains("Run") && text.contains("Stop"), "{text}");
    let (_, action) = bar.on_key(&key(KeyCode::Enter));
    assert_eq!(action, Some(ToolbarAction::Activated(0)));
}

fn tabs_editing() {
    let mut tabs = Tabs::new(WidgetId::of("scenario.tabs"), &["one", "two"]);
    tabs.push(TabItem::new("three").closable());
    let labels = |tabs: &Tabs| -> Vec<String> {
        tabs.items().iter().map(|item| item.label.clone()).collect()
    };
    assert_eq!(labels(&tabs), ["one", "two", "three"]);
    tabs.set_active(2);
    assert!(tabs.reorder(2, 0), "the last tab moves to the front");
    assert!(!tabs.reorder(1, 1) && !tabs.reorder(0, 9));
    assert_eq!(labels(&tabs), ["three", "one", "two"]);
    assert_eq!(tabs.active, 0, "the active mark follows the moved tab");
    assert!(tabs.update_item(1, |item| item.dirty = true));
    assert!(!tabs.update_item(1, |item| item.dirty = true));
    assert!(!tabs.update_item(9, |item| item.dirty = true));
    assert!(tabs.items()[1].dirty);
    let text = text_of(&paint(Rect::new(0, 0, 60, 3), |area, buf, ctx| {
        tabs.render(area, buf, ctx, ctx.theme.canvas)
    }));
    let at = |label: &str| {
        text.find(label)
            .unwrap_or_else(|| panic!("{label}:\n{text}"))
    };
    assert!(at("three") < at("one") && at("one") < at("two"), "{text}");
}

fn scroll_panel_content() {
    fn plain(theme: &Theme, _line: &str) -> ratatui::style::Style {
        theme.secondary()
    }
    let source = |n: usize| (1..=n).map(|i| format!("line {i}")).collect::<Vec<_>>();
    let mut panel = ScrollPanel::new(WidgetId::of("scenario.panel"), source(5));
    let draw = |panel: &mut ScrollPanel| {
        text_of(&paint(Rect::new(0, 0, 30, 8), |area, buf, ctx| {
            panel.render(area, buf, ctx, ctx.theme.canvas, plain)
        }))
    };
    assert!(draw(&mut panel).contains("line 5"));
    assert_eq!(panel.lines().len(), 5);
    assert_eq!(panel.replace_line(1, "edited".to_owned()), Ok(true));
    assert_eq!(panel.replace_line(1, "edited".to_owned()), Ok(false));
    assert_eq!(panel.replace_line(9, "x".to_owned()), Err(9));
    assert!(panel.extend_lines(["appended".to_owned()]));
    assert!(!panel.extend_lines(Vec::new()));
    let text = draw(&mut panel);
    assert!(
        text.contains("edited") && text.contains("appended"),
        "{text}"
    );
    assert!(!text.contains("line 2"), "the old text is gone:\n{text}");

    panel.select_all();
    assert_eq!(
        panel.source_selection(),
        Some(((0, 0), (5, "appended".len()))),
        "the selection is in source lines, not wrapped rows"
    );
    assert!(panel.extend_lines(["more".to_owned()]));
    assert!(panel.source_selection().is_some(), "an append keeps it");
    assert!(panel.set_lines(source(2)));
    assert_eq!(panel.source_selection(), None, "a replacement clears it");
    assert_eq!(panel.lines(), ["line 1", "line 2"]);
    assert!(panel.clear());
    assert!(!panel.clear());
    assert!(panel.lines().is_empty());
    assert!(!draw(&mut panel).contains("line 1"));
}

fn k(name: &str) -> ItemKey {
    ItemKey::text(name)
}

#[derive(Clone, Debug, PartialEq)]
struct Doc {
    title: String,
    locked: bool,
}

impl Doc {
    fn new(title: &str, locked: bool) -> Self {
        Self {
            title: title.to_owned(),
            locked,
        }
    }
}

impl KeyedRecord for Doc {
    fn label(&self) -> &str {
        &self.title
    }

    fn enabled(&self) -> bool {
        !self.locked
    }
}

fn keyed_store_identity() {
    let (a, b, c) = (k("a"), k("b"), k("c"));
    let mut store: KeyedVec<Doc> = KeyedVec::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
    store.push(a, Doc::new("alpha", false)).expect("a is new");
    store.push(b, Doc::new("bravo", true)).expect("b is new");
    store
        .insert(0, c, Doc::new("charlie", false))
        .expect("c is new");
    assert_eq!(store.len(), 3);
    assert_eq!(
        store.iter().map(|(key, _)| key).collect::<Vec<_>>(),
        vec![c, a, b],
        "insert places a record before the position"
    );
    assert_eq!(store.key_at(1), Some(a));
    assert_eq!(store.value_at(2).map(|d| d.title.as_str()), Some("bravo"));
    assert_eq!(store.position(b), Some(2));
    assert_eq!(store.get(c).map(|d| d.title.as_str()), Some("charlie"));
    assert!(store.get(k("zzz")).is_none() && store.key_at(9).is_none());
    assert_eq!(
        store.push(a, Doc::new("again", false)),
        Err(KeyedSourceError::DuplicateAuthoritative { key: a })
    );
    assert_eq!(
        store.push(ItemKey::index(0), Doc::new("positional", false)),
        Err(KeyedSourceError::PositionalKey {
            index: 3,
            authoritative: true
        })
    );
    assert_eq!(store.len(), 3, "a refused record changes nothing");

    let start: SourceStamp = store.stamp();
    store
        .update(a, |doc| doc.title.push('!'))
        .expect("a is held");
    let after_update = store.stamp();
    assert_eq!(
        (after_update.data, after_update.view),
        (start.data + 1, start.view),
        "an edit moves the data revision only"
    );
    assert!(store.move_to(a, 0) && !store.move_to(a, 0));
    let after_move = store.stamp();
    assert_eq!(
        (after_move.data, after_move.view),
        (after_update.data, after_update.view + 1),
        "a move changes the view revision only"
    );
    let same: Vec<(ItemKey, Doc)> = store.iter().map(|(key, doc)| (key, doc.clone())).collect();
    assert_eq!(store.replace_if_changed(same.clone()), Ok(false));
    assert_eq!(store.stamp(), after_move, "equal rows keep the stamp");
    let mut reversed = same;
    reversed.reverse();
    assert_eq!(store.replace_if_changed(reversed), Ok(true));
    assert_eq!(
        store.stamp().instance,
        start.instance,
        "a replace keeps the instance"
    );
    assert_eq!(store.key_at(0), Some(b));
    let clone = store.clone();
    assert_ne!(clone.stamp().instance, store.stamp().instance);
    assert!(
        KeyedVec::<Doc>::try_from_rows([(a, Doc::new("x", false)), (a, Doc::new("y", false))])
            .is_err()
    );
    store
        .replace([
            (a, Doc::new("alpha", false)),
            (b, Doc::new("bravo", true)),
            (c, Doc::new("charlie", false)),
        ])
        .expect("distinct keys");

    let mut state = KeyedState::new();
    assert!(state.stamp().is_none());
    let first: Reconcile = state.reconcile(&store).expect("a valid source");
    assert!(first.data_changed && first.view_changed && first.cursor_changed);
    assert_eq!(state.stamp(), Some(store.stamp()));
    assert_eq!(state.reconcile(&store), Ok(Reconcile::default()));
    assert_eq!(state.visible_len(), 3);
    assert_eq!(state.visible_key(1), Some(b));
    assert_eq!(state.visible_index(c), Some(2));
    assert!(state.is_authoritative(a) && !state.is_authoritative(k("zzz")));
    assert!(
        state.is_enabled(a) && !state.is_enabled(b),
        "a locked record is disabled"
    );
    assert_eq!(
        state.cursor(),
        Some(a),
        "the cursor lands on the first enabled row"
    );
    assert!(!state.set_cursor(b) && state.set_cursor(c));
    assert_eq!(state.cursor_view(), Some(2));
    assert!(
        state.adopt_cursor_view(0) && !state.adopt_cursor_view(1) && !state.adopt_cursor_view(9)
    );
    assert!(state.clear_cursor() && state.cursor().is_none());

    assert!(
        state.set_anchor(Some(a))
            && !state.set_anchor(Some(a))
            && !state.set_anchor(Some(k("zzz")))
    );
    assert_eq!((state.anchor(), state.anchor_view()), (Some(a), Some(0)));
    assert!(
        state.set_selected(a, true) && !state.set_selected(b, true),
        "a disabled row is not newly selected"
    );
    assert!(state.replace_selection([a, c, k("zzz")]));
    assert_eq!(
        state.selected().collect::<Vec<_>>().len(),
        state.selected_len()
    );
    assert_eq!(state.selected_len(), 2);
    assert!(state.is_selected(c) && !state.is_selected(b));
    assert_eq!(state.selected_view(), vec![0, 2]);
    assert!(
        state.adopt_selection_view([1, 2], Some(2)),
        "a disabled view row is skipped"
    );
    assert_eq!(
        (state.selected_view(), state.anchor_view()),
        (vec![2], Some(2))
    );

    let before = state.expanded_revision();
    assert!(
        state.set_expanded(a, true)
            && !state.set_expanded(a, true)
            && !state.set_expanded(k("zzz"), true)
    );
    assert!(state.is_expanded(a));
    assert_eq!(state.expanded().collect::<Vec<_>>(), vec![a]);
    assert_eq!(state.expanded_revision(), before + 1);

    assert!(!state.begin_edit(b), "a disabled row cannot be edited");
    assert!(state.begin_edit(a) && !state.begin_edit(a));
    assert_eq!(state.edit_key(), Some(a));
    assert!(state.cancel_edit() && !state.cancel_edit());
    assert!(state.begin_edit(a));
    store
        .update(c, |doc| doc.title.push('?'))
        .expect("c is held");
    let cancelled = state.reconcile(&store).expect("still valid");
    assert!(
        cancelled.edit_cancelled && state.edit_key().is_none(),
        "any data change cancels an edit"
    );
    assert!(state.begin_edit(a));
    store
        .update(a, |doc| doc.title.push('#'))
        .expect("a is held");
    assert_eq!(
        state.finish_edit(),
        Some(a),
        "a committed edit is finished before reconciling"
    );
    assert!(!state.reconcile(&store).expect("valid").edit_cancelled);

    assert!(state.capture_press(a, 1) && !state.capture_press(b, 1));
    assert_eq!(
        state.pressed(),
        Some(a),
        "a refused capture keeps the held press"
    );
    assert!(state.capture_press(a, 1));
    assert!(state.release_press(a, 1) && !state.release_press(a, 1));
    assert!(state.capture_press(a, 1));
    state.cancel_press();
    assert_eq!(state.pressed(), None);
    assert!(state.capture_press(a, 1));
    assert!(!state.invalidate_layout(1) && state.invalidate_layout(2));
    assert_eq!(state.pressed(), None, "a layout change drops the capture");
    assert!(state.capture_press(a, 2));
    store.move_to(a, 2);
    let moved = state.reconcile(&store).expect("valid");
    assert!(
        moved.pointer_cancelled && moved.view_changed,
        "a reorder drops the press"
    );
    assert!(!RowSource::enabled(&store, 0) && KeyedRowSource::source_len(&store) == 3);
    assert_eq!(
        KeyedRowSource::source_key(&store, 0),
        store.key_at(0).expect("row 0")
    );
    assert_eq!(RowSource::label(&store, 0), "bravo");
}

struct Shown {
    data: u64,
    authoritative: Vec<ItemKey>,
    visible: Vec<ItemKey>,
}

impl RowSource for Shown {
    fn len(&self) -> usize {
        self.visible.len()
    }

    fn label(&self, _: usize) -> &str {
        "row"
    }

    fn key(&self, i: usize) -> ItemKey {
        self.visible[i]
    }
}

impl KeyedRowSource for Shown {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: k("scenario.shown"),
            data: self.data,
            view: 1,
        }
    }

    fn source_len(&self) -> usize {
        self.authoritative.len()
    }

    fn source_key(&self, i: usize) -> ItemKey {
        self.authoritative[i]
    }
}

struct Graph {
    roots: Vec<ItemKey>,
    nodes: Vec<(ItemKey, Vec<ItemKey>)>,
}

impl TreeSource for Graph {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: k("scenario.graph"),
            data: 1,
            view: 1,
        }
    }

    fn root_count(&self) -> usize {
        self.roots.len()
    }

    fn root_key(&self, index: usize) -> ItemKey {
        self.roots[index]
    }

    fn node(&self, key: ItemKey) -> Option<TreeNodeRef<'_>> {
        let (_, children) = self.nodes.iter().find(|(k, _)| *k == key)?;
        Some(TreeNodeRef {
            label: "node",
            meta: None,
            glyph: None,
            glyph_tone: junie_tui::theme::Tone::Muted,
            label_tone: None,
            children,
            lazy: false,
            busy: false,
            note: false,
        })
    }
}

fn keyed_source_rejection() {
    let (a, b) = (k("a"), k("b"));
    let good = Shown {
        data: 1,
        authoritative: vec![a, b],
        visible: vec![a, b],
    };
    let mut state = KeyedState::new();
    state.reconcile(&good).expect("a valid source");
    let held = state.stamp();
    // Each broken source keeps the instance and moves the data revision, so
    // the state checks every key again; the error leaves the state untouched.
    let mut reject = |authoritative: Vec<ItemKey>, visible: Vec<ItemKey>| {
        let bad = Shown {
            data: 2,
            authoritative,
            visible,
        };
        let error = state.reconcile(&bad).expect_err("a broken source");
        assert_eq!(
            state.stamp(),
            held,
            "a rejected source leaves state as it was"
        );
        error
    };
    assert_eq!(
        reject(vec![ItemKey::index(0), b], vec![a, b]),
        KeyedSourceError::PositionalKey {
            index: 0,
            authoritative: true
        }
    );
    assert_eq!(
        reject(vec![a, a], vec![a]),
        KeyedSourceError::DuplicateAuthoritative { key: a }
    );
    assert_eq!(
        reject(vec![a, b], vec![a, a]),
        KeyedSourceError::DuplicateVisible { key: a }
    );
    assert_eq!(
        reject(vec![a], vec![a, b]),
        KeyedSourceError::UnknownVisible { key: b }
    );

    let (root, kid, ghost) = (k("root"), k("kid"), k("ghost"));
    let mut view = TreeView::new(WidgetId::of("scenario.tree"), Vec::new());
    let dangling = Graph {
        roots: vec![root],
        nodes: vec![(root, vec![kid, ghost]), (kid, vec![])],
    };
    assert_eq!(
        view.expand_all_source(&dangling),
        Err(KeyedSourceError::MissingNode { key: ghost })
    );
    let cyclic = Graph {
        roots: vec![root],
        nodes: vec![(root, vec![kid]), (kid, vec![root])],
    };
    assert_eq!(
        view.expand_all_source(&cyclic),
        Err(KeyedSourceError::Cycle { key: root })
    );
    let sound = Graph {
        roots: vec![root],
        nodes: vec![(root, vec![kid]), (kid, vec![])],
    };
    assert_eq!(view.expand_all_source(&sound), Ok(()));

    let mut tree = KeyedTree::new();
    assert_eq!(
        tree.insert(Some(ghost), 0, kid, KeyedTreeNode::new("kid")),
        Err(KeyedSourceError::MissingNode { key: ghost }),
        "the store refuses a child of a node it does not hold"
    );
}

fn keyed_pointer_press() {
    let id = WidgetId::of("scenario.press");
    let store = KeyedVec::try_from_rows([
        (k("one"), ListItem::new("one")),
        (k("two"), ListItem::new("two")),
        (k("three"), ListItem::new("three")),
    ])
    .expect("distinct keys");
    let mut list = ListBox::new(id, Vec::new(), SelectMode::Multi);
    let area = Rect::new(0, 0, 30, 6);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        list.render_source(area, &store, &mut buf, &mut ctx, theme.canvas)
            .expect("a valid store");
    }
    let row_at = |y: u16| {
        let pos = Position::new(2, y);
        let region = hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == id && r.part.part == Part::ROW && r.area.contains(pos))
            .expect("a row region");
        (region.id, pos)
    };
    let event = |phase: Phase, y: u16| {
        let (region, pos) = row_at(y);
        PointerEvent::new(&hits, phase, region, pos, 1, KeyModifiers::NONE)
    };
    let press = event(Phase::Press, 1);
    assert_eq!(
        press.key_of(Part::ROW),
        Some(k("two")),
        "the event names the record, not the row number"
    );
    assert_eq!(
        press.key_of(Part::MARKER),
        None,
        "another part carries no key"
    );
    let held = list.on_pointer_source(&press, &store).expect("valid");
    assert!(!held.is_consumed(), "a press only captures");
    let clicked = list
        .on_pointer_source(&event(Phase::Click, 1), &store)
        .expect("valid");
    assert!(
        clicked
            .actions()
            .any(|a| *a == ListAction::SelectionChanged)
    );
    assert_eq!(list.keyed().selected().collect::<Vec<_>>(), vec![k("two")]);

    let stray = list
        .on_pointer_source(&event(Phase::Press, 0), &store)
        .expect("valid");
    assert!(!stray.has_actions());
    let off = list
        .on_pointer_source(&event(Phase::Click, 2), &store)
        .expect("valid");
    assert!(
        off.is_consumed() && !off.has_actions(),
        "a release on another row than the press is refused"
    );
    assert_eq!(list.keyed().selected().collect::<Vec<_>>(), vec![k("two")]);

    let mut rows = RowSelection::default();
    rows.replace([2, 0], Some(2));
    assert_eq!(rows.iter().collect::<Vec<_>>().len(), 2);
    assert_eq!(rows.anchor(), Some(2));
    rows.replace([], None);
    assert_eq!(rows.iter().count(), 0);
}

fn response_batching() {
    let mut none: Actions<&str> = Actions::default();
    assert!(matches!(none, Actions::None) && none.is_empty());
    assert_eq!(none.len(), 0);
    assert!(matches!(none.iter(), ActionsIter::Small(_)));
    none.push("first");
    assert!(matches!(none, Actions::One("first")));
    assert!(matches!(none.iter(), ActionsIter::Small(_)));
    none.push("second");
    assert!(matches!(&none, Actions::Many(all) if all.len() == 2));
    assert!(matches!(none.iter(), ActionsIter::Many(_)));
    assert_eq!(
        none.iter().len(),
        2,
        "the borrowed iterator knows its length"
    );
    let mut other = Actions::One("third");
    none.append(other.clone());
    other.push("fourth");
    none.append(other);
    assert_eq!(
        none.iter().copied().collect::<Vec<_>>(),
        vec!["first", "second", "third", "third", "fourth"]
    );
    assert!(matches!(none.clone().into_iter(), ActionsIntoIter::Many(_)));
    assert!(matches!(
        Actions::One(1).into_iter(),
        ActionsIntoIter::Small(_)
    ));
    assert_eq!(Actions::One(1).into_iter().len(), 1);
    assert_eq!(Actions::<u8>::None.into_iter().count(), 0);

    let mut response: Response<&str> = Response::ignored();
    assert!(!response.has_actions() && response.actions().count() == 0);
    response.push_action("open");
    assert_eq!(
        (
            response.flow(),
            response.invalidate(),
            response.has_actions()
        ),
        (Flow::Consumed, Invalidate::Paint, true)
    );
    let single = response.clone();
    assert_eq!(
        single.try_into_single(),
        Ok((Outcome::Changed, Some("open")))
    );
    response.push_action("close");
    assert_eq!(
        response.clone().try_into_single().map(|_| ()),
        Err(response.clone()),
        "a batch of two comes back whole"
    );
    assert_eq!(
        response.actions().copied().collect::<Vec<_>>(),
        vec!["open", "close"]
    );

    let shown = response.clone().map_actions(|action| action.len());
    assert_eq!(
        shown.into_actions().iter().copied().collect::<Vec<_>>(),
        vec![4, 5]
    );
    let mut seen = Vec::new();
    let drained = response.clone().on_actions(|action| seen.push(action));
    assert_eq!(seen, vec!["open", "close"]);
    assert!(!drained.has_actions() && drained.outcome() == Outcome::Changed);

    let mut taken = response.clone();
    let batch = taken.take_actions();
    assert!(matches!(batch, Actions::Many(_)) && !taken.has_actions());
    let (flow, invalidate, actions) = response.clone().into_raw_parts();
    assert_eq!(
        (flow, invalidate, actions.len()),
        (Flow::Consumed, Invalidate::Paint, 2)
    );

    let relayout = Response::<&str>::consumed().relayout();
    let merged = response.merge(relayout).merge(Response::emit("last"));
    assert_eq!(
        merged.invalidate(),
        Invalidate::Layout,
        "the stronger invalidation wins"
    );
    assert_eq!(
        merged.actions().copied().collect::<Vec<_>>(),
        vec!["open", "close", "last"],
        "actions keep their order"
    );
}

fn data_shape_errors() {
    let ragged = vec![vec!["a", "b"], vec!["c"], vec!["d", "e", "f"]];
    let error = validate_rectangular(2, &ragged).expect_err("row 1 is short");
    assert_eq!(
        error,
        DataError::Shape(ShapeError {
            row: 1,
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(error.to_string(), "row 1 has 1 cells, expected 2");
    assert_eq!(validate_rectangular(2, &ragged[..1]), Ok(()));
    assert_eq!(
        validate_rectangular(0, &[vec!["x"]]),
        Err(DataError::EmptySchemaWithRows)
    );

    let mut table = DataTable::try_new(
        WidgetId::of("scenario.table"),
        vec![
            Column::new("name", Constraint::Length(8)),
            Column::new("state", Constraint::Length(8)),
        ],
        vec![vec![Cell::new("lint"), Cell::new("ok")]],
    )
    .expect("a rectangular table");
    assert_eq!(
        table.try_set_row(5, vec![Cell::new("x"), Cell::new("y")]),
        Err(DataError::MissingRow(5))
    );
    assert_eq!(
        table.try_set_row(0, vec![Cell::new("x")]),
        Err(DataError::Shape(ShapeError {
            row: 0,
            expected: 2,
            actual: 1
        }))
    );
    assert_eq!(table.rows().len(), 1, "a refused row changes nothing");
    assert!(matches!(
        DataTable::try_new(
            WidgetId::of("scenario.none"),
            Vec::new(),
            vec![vec![Cell::new("x")]]
        ),
        Err(DataError::EmptySchemaWithRows)
    ));
}

fn text_buffer_geometry() {
    let mut buffer = TextBuffer::multi("SELECT id, name\nFROM users\nWHERE active");
    assert_eq!(buffer.line_range(0), Some(0..15));
    assert_eq!(buffer.line_range(1), Some(16..26));
    assert_eq!(
        buffer.line_range(3),
        None,
        "past the last line there is no range"
    );
    assert_eq!(buffer.max_line_width(), 15);
    let at = buffer.text().find("users").expect("users is in the text");
    let pos = buffer.position_at(at);
    assert_eq!((pos.line, pos.col), (1, 5));
    let before = buffer.revision();
    buffer.insert_at(0, "-- ");
    assert_ne!(buffer.revision(), before, "an edit moves the revision");
    assert_eq!(buffer.max_line_width(), 18);
    assert_eq!(buffer.line_range(0), Some(0..18));
    let after = buffer.revision();
    buffer.insert_at(0, "");
    assert_eq!(buffer.revision(), after, "an empty edit keeps the revision");
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

/// The #56 work counters, on the Meters page's Work counters card. The same
/// calls run in a build that records and in one that compiles the counters
/// away: the catalog and the snapshot contract are always real, and
/// `record_work!` counts exactly when `ENABLED` says it does.
fn work_counter_contract() {
    use crate::pages::meters_work;
    use junie_tui::metrics::{
        ENABLED, METRIC_COUNT, Metric, MetricsError, WorkDelta, WorkSnapshot,
    };

    /// `SegmentedLines` becomes `segmented_lines`: the name every metric must carry,
    /// derived from the variant so the scenario does not repeat the catalog.
    fn snake_case(variant: &str) -> String {
        let mut out = String::new();
        for (at, ch) in variant.chars().enumerate() {
            if ch.is_ascii_uppercase() && at > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        }
        out
    }

    /// Counts `amount` of every metric through the recorder behind
    /// `record_work!`, and returns each metric's count.
    #[cfg(feature = "work-metrics")]
    fn record_each(amount: u64) -> WorkDelta {
        junie_tui::metrics::measure(|| {
            for metric in Metric::ALL {
                junie_tui::metrics::record(metric, amount);
            }
        })
        .1
    }

    assert_eq!(ENABLED, cfg!(feature = "work-metrics"));
    assert_eq!(Metric::ALL.len(), METRIC_COUNT);
    for (index, metric) in Metric::ALL.into_iter().enumerate() {
        assert_eq!(metric as usize, index, "ALL follows index order");
        assert_eq!(metric.name(), snake_case(&format!("{metric:?}")));
    }

    let thread = std::thread::current().id();
    let at = |epoch: u64, rows: u64| {
        let mut counts = [0; METRIC_COUNT];
        counts[Metric::VisibleRowsPainted as usize] = rows;
        WorkSnapshot {
            thread,
            epoch,
            counts,
        }
    };
    let delta = at(0, 9).delta(at(0, 4)).expect("comparable snapshots");
    assert_eq!(delta.get(Metric::VisibleRowsPainted), 5);
    assert_eq!(
        delta.nonzero().collect::<Vec<_>>(),
        [(Metric::VisibleRowsPainted, 5)]
    );
    assert_eq!(WorkDelta::ZERO.nonzero().count(), 0);
    assert_eq!(
        delta.check_exact(&[(Metric::VisibleRowsPainted, 5)]),
        Ok(())
    );
    assert_eq!(
        delta.check_budget(&[(Metric::VisibleRowsPainted, 5)]),
        Ok(())
    );
    assert_eq!(
        delta.check_budget(&[(Metric::VisibleRowsPainted, 4)]),
        Err("visible_rows_painted was 5, budget 4".to_owned())
    );
    assert_eq!(
        delta.check_exact(&[]),
        Err("visible_rows_painted was 5, expected 0".to_owned())
    );
    assert_eq!(at(1, 9).delta(at(0, 4)), Err(MetricsError::DifferentEpoch));
    assert_eq!(
        at(0, 1).delta(at(0, 4)),
        Err(MetricsError::CounterWentBackwards {
            index: Metric::VisibleRowsPainted as usize
        })
    );
    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("thread id");
    let mut foreign = at(0, 9);
    foreign.thread = other;
    assert_eq!(foreign.delta(at(0, 4)), Err(MetricsError::DifferentThread));
    let merged = delta.merged(delta).expect("a small merge");
    assert_eq!(merged.get(Metric::VisibleRowsPainted), 10);
    assert_eq!(
        WorkDelta([u64::MAX; METRIC_COUNT]).merged(delta),
        Err(MetricsError::Overflow {
            index: Metric::VisibleRowsPainted as usize
        })
    );

    #[cfg(feature = "work-metrics")]
    {
        let counted = record_each(2);
        assert_eq!(
            counted.nonzero().count(),
            METRIC_COUNT,
            "the recorder accepts every metric variant"
        );
        assert_eq!(
            counted.check_exact(&Metric::ALL.map(|metric| (metric, 2))),
            Ok(())
        );
        let (_, delta) = junie_tui::metrics::measure(|| {
            junie_tui::record_work!(SegmentedLines, 3usize);
        });
        assert_eq!(delta.get(Metric::SegmentedLines), 3);
        assert_eq!(meters_work::probe(), Some(3));
    }
    #[cfg(not(feature = "work-metrics"))]
    {
        let evaluated = std::cell::Cell::new(false);
        junie_tui::record_work!(InputDispatches, {
            evaluated.set(true);
            1usize
        });
        assert!(!evaluated.get(), "a compiled-out site never evaluates");
        assert_eq!(meters_work::probe(), None);
    }

    let mut page = crate::pages::meters::page();
    page.set_option("meters.work", 1);
    let text = crate::pages::page_text(page.as_mut(), 120, 90);
    let catalog = format!(
        "{METRIC_COUNT} metrics, {} to {}",
        Metric::ALL[0].name(),
        Metric::ALL[METRIC_COUNT - 1].name()
    );
    for needle in [
        "Work counters",
        catalog.as_str(),
        "visible_rows_painted +6, intrinsic_width_calls +40",
        "intrinsic_width_calls was 40, budget 30",
        "snapshots are from different threads",
        "a reset ran between the snapshots",
        "counter visible_rows_painted went backwards",
        "counter intrinsic_width_calls overflowed",
    ] {
        assert!(text.contains(needle), "{needle:?} missing:\n{text}");
    }
    let build = if ENABLED { "recording" } else { "compiled out" };
    assert!(text.contains(build), "the card names its build: {build}");
}
