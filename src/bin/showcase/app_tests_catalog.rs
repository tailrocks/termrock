//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::core::data::{DataError, ShapeError, validate_rectangular};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::Axis;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keyed::{KeyedRecord, KeyedSourceError, KeyedState, KeyedVec, Reconcile};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::core::response::{
    Actions, ActionsIntoIter, ActionsIter, Flow, Invalidate, Response,
};
use junie_tui::core::selection::RowSelection;
use junie_tui::core::text::TextBuffer;
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::edit::{EditField, Validator};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list::{ListAction, ListBox, ListItem, SelectMode};
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::table::{Cell, Column, DataTable};
use junie_tui::widgets::tree::{KeyedTree, KeyedTreeNode, TreeNodeRef, TreeSource, TreeView};
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
