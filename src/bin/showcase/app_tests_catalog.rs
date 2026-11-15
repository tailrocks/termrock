//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::Axis;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keyed::KeyedSourceError;
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::core::selection::ClickPolicy;
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::edit::{EditField, Validator};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list::{
    ListAction, ListBox, ListItem, ListItemRef, ListSource, SelectMode,
};
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};
use junie_tui::widgets::nav_list::{NavItem, NavList};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::table::{
    Cell, CellRef, Column, DataTable, KeyedTable, KeyedTableError, TableAction, TableEditSource,
    TableSource,
};
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
    ScenarioCase { id: ScenarioId::TerminalViewportLifecycle, run: terminal_viewport_lifecycle },
    ScenarioCase { id: ScenarioId::FixedControlsPolicy, run: fixed_controls_policy },
    ScenarioCase { id: ScenarioId::FixedDialogsPolicy, run: fixed_dialogs_policy },
    ScenarioCase { id: ScenarioId::ListDetailNavigation, run: list_detail_navigation },
    ScenarioCase { id: ScenarioId::KeyedListSource, run: keyed_list_source },
    ScenarioCase { id: ScenarioId::NavItemReplacement, run: nav_item_replacement },
    ScenarioCase { id: ScenarioId::TableModelUpdates, run: table_model_updates },
    ScenarioCase { id: ScenarioId::KeyedTableStore, run: keyed_table_store },
    ScenarioCase { id: ScenarioId::KeyedTableSource, run: keyed_table_source },
    ScenarioCase { id: ScenarioId::KeyedTableEdit, run: keyed_table_edit },
    ScenarioCase { id: ScenarioId::KeyedTreeStore, run: keyed_tree_store },
    ScenarioCase { id: ScenarioId::KeyedTreeView, run: keyed_tree_view },
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
