//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::core::edit::{FieldKeys, TextCmd};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusChange, FocusRing};
use junie_tui::core::hit::Axis;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::response::Response;
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::edit::{EditField, Validator};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::input::TextInput as FinderQuery;
use junie_tui::widgets::list_detail::{
    Action, DetailPane, Event, Frame, Line, ListDetail, ListDetailSource, ListRow, ListRowRef,
    SourceFrame,
};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::search_detail::{
    MissingSelection, QueryPasteMode, QueryRequest, QueryView, ResultRowKind, RowPaint,
    SearchAction, SearchCmd, SearchConfig, SearchDetail, SearchDetailSource, SearchError,
    SearchFocusMode, SearchPaint, SearchPhase, SearchSession, SearchSource, paint_query_default,
};
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
    ScenarioCase { id: ScenarioId::SearchDetailQuery, run: search_detail_query },
    ScenarioCase { id: ScenarioId::SearchDetailSelection, run: search_detail_selection },
    ScenarioCase { id: ScenarioId::SearchDetailPaint, run: search_detail_paint },
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
            detail: "",
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
    let mut found = actions_of(reply).into_iter().filter_map(|action| match action {
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
    search.submit(request.token, &cat).expect("fresh results");
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
    search
        .fail(second.token, "the ranking service did not answer".to_owned())
        .expect("the latest request can fail");
    assert_eq!(search.phase(), SearchPhase::Failed);
    assert_eq!(
        search.session().error(),
        Some("the ranking service did not answer")
    );
    let retry = requested(search.retry());
    assert_eq!(retry.text, "al");
    let again = FinderCat::new(&retry, FINDER_ROWS);
    search.submit(retry.token, &again).expect("the retry answers");
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
    session
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
    let mut selected = |search: &mut SearchDetail, focus: &mut Focus, code| {
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
    for code in [KeyCode::PageDown, KeyCode::PageUp] {
        let landed = selected(&mut search, &mut focus, code).expect("a selection");
        assert!(search.session().is_selectable(landed));
    }
    selected(&mut search, &mut focus, KeyCode::Home);
    let ran = finder_key(&mut search, &cat, &mut focus, KeyCode::Enter);
    assert!(ran.contains(&SearchAction::Activate(ItemKey::num(10))));

    // Ctrl+O opens the preview, which the host closes again; Ctrl+F returns
    // to the query.
    finder_chord(&mut search, &cat, &mut focus, KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(search.on_detail(focus.current()));
    assert_eq!(focus.current(), Some(search.detail_id()));
    search.close_detail(&mut focus);
    assert!(!search.on_detail(focus.current()));
    finder_chord(&mut search, &cat, &mut focus, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert_eq!(focus.current(), Some(search.session().query_input().id));

    // The session reports preview actions and items against the selection.
    let mut session = SearchSession::new(
        FinderQuery::new(WidgetId::of("scenario.fd.keys"), "Search"),
        SearchConfig::default(),
    );
    let request = requested(session.refresh_source(finder_stamp(1)));
    session
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
    assert_eq!(nearest, num(13), "the nearest enabled key to where it was");

    // Focus owner: in Combined one owner takes typing and navigation.
    let mut combined = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let cat = finder_started(&mut combined, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(combined.id());
    let typed = finder_key(&mut combined, &cat, &mut focus, KeyCode::Char('a'));
    assert_eq!(combined.query(), "a");
    assert!(typed.iter().any(|a| matches!(a, SearchAction::QueryRequested(_))));
    let _ = finder_key(&mut combined, &cat, &mut focus, KeyCode::Down);
    assert_eq!(combined.selected_key(), num(12), "Down still navigates");

    // Typing from the preview returns to the query only when it is allowed.
    for (allowed, text) in [(false, ""), (true, "x")] {
        let mut search =
            finder_search(SearchConfig::default().type_to_query_from_detail(allowed));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.detail_id());
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Char('x'));
        assert_eq!(search.query(), text, "type_to_query_from_detail = {allowed}");
        assert_eq!(
            focus.current() == Some(search.session().query_input().id),
            allowed
        );
    }

    // A keymap override replaces the default bindings.
    let mut keymap = SearchCmd::default_keymap();
    keymap.push((KeyBinding::ctrl('n'), SearchCmd::Down));
    let mut emacs = finder_search(SearchConfig::default().keymap(keymap));
    let cat = finder_started(&mut emacs, FINDER_ROWS);
    let mut focus = Focus::default();
    focus.focus(emacs.id());
    finder_chord(&mut emacs, &cat, &mut focus, KeyCode::Char('n'), KeyModifiers::CONTROL);
    assert_eq!(emacs.selected_key(), num(12));

    // Moving while a request is pending, and the first item of a new query.
    for (moves, expected) in [(false, num(10)), (true, num(12))] {
        let mut search = finder_search(SearchConfig::default().navigate_while_loading(moves));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.id());
        let _ = search.set_query("z");
        assert_eq!(search.phase(), SearchPhase::Loading);
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Down);
        assert_eq!(search.selected_key(), expected, "navigate_while_loading = {moves}");
    }
    for (first, expected) in [(true, num(10)), (false, num(12))] {
        let mut search = finder_search(SearchConfig::default().select_first_on_query(first));
        let cat = finder_started(&mut search, FINDER_ROWS);
        let mut focus = Focus::default();
        focus.focus(search.id());
        let _ = finder_key(&mut search, &cat, &mut focus, KeyCode::Down);
        let request = requested(search.set_query("g"));
        let answer = FinderCat::new(&request, FINDER_ROWS);
        search.submit(request.token, &answer).expect("accepted");
        assert_eq!(search.selected_key(), expected, "select_first_on_query = {first}");
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
    search
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
                assert_ne!(row.cell_hit(cell), row.hit, "a chip has a region of its own");
                let at = Rect::new(rect.x + 3 + cell as u16 * 8, rect.y, 6, 1);
                buf.set_string(at.x, at.y, format!("[{}]", row.source.cell_label(row.row, cell)), ratatui::style::Style::default());
                row.place_cell(cell, at);
                self.chips += 1;
            }
        } else {
            buf.set_string(rect.x + 3, rect.y, row.source.label(row.row), ratatui::style::Style::default());
        }
    }

    fn empty(&mut self, area: Rect, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.empty = true;
        buf.set_string(area.x + 1, area.y, "host empty", ratatui::style::Style::default());
    }

    fn detail(&mut self, pane: DetailPane<'_>, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.detail = true;
        buf.set_string(pane.area.x + 1, pane.area.y, "host detail", ratatui::style::Style::default());
    }

    fn summary(&mut self, area: Rect, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.summary = true;
        buf.set_string(area.x + 1, area.y, "host summary", ratatui::style::Style::default());
    }
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
    let lines: Vec<Line> = (1..=30)
        .map(|n| Line::Text(format!("line {n}"), junie_tui::theme::Tone::Muted))
        .collect();
    let frame = |lines: &[Line]| SourceFrame {
        title: "Results",
        meta: "5",
        head: &[],
        under: None,
        detail_title: "Preview",
        detail_meta: "",
        lines: Box::leak(lines.to_vec().into_boxed_slice()),
        actions: &[],
    };
    fn as_detail_source<S: SearchDetailSource + ?Sized>(_: &S) {}
    as_detail_source(&cat);
    let mut buf = Buffer::empty(wide);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut focus = Focus::default();
    focus.focus(search.id());
    {
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        search
            .render_source(wide, &cat, &frame(&lines), &mut buf, &mut ctx)
            .expect("the accepted results are drawn");
    }
    let shown = text_of(&buf);
    assert!(shown.contains("alpha") && shown.contains("gamma") && shown.contains("Preview"));
    let target = hits
        .regions()
        .iter()
        .rev()
        .find(|r| {
            r.owner == search.id()
                && r.part.part == Part::ROW
                && r.part.item == Some(ItemKey::num(12))
        })
        .expect("the row of gamma has a hit region")
        .clone();
    let at = Position::new(target.area.x + 4, target.area.y);
    let press = PointerEvent::new(&hits, Phase::Press, target.id, at, 1, KeyModifiers::NONE);
    let pressed = actions_of(search.pointer(&press, &cat, &mut focus).expect("accepted"));
    assert!(pressed.contains(&SearchAction::SelectionChanged(num(12))));
    let click = PointerEvent::new(&hits, Phase::Click, target.id, at, 2, KeyModifiers::NONE);
    let clicked = actions_of(search.pointer(&click, &cat, &mut focus).expect("accepted"));
    assert!(clicked.contains(&SearchAction::Activate(ItemKey::num(12))));

    // A keyboard context request waits for a paint, then anchors to the row.
    let _ = finder_chord(&mut search, &cat, &mut focus, KeyCode::F(10), KeyModifiers::SHIFT);
    let anchored = actions_of(search.emit_pending_context());
    assert!(
        anchored.iter().all(|a| matches!(a, SearchAction::ContextRequested(_))),
        "the pending request is the only thing a paint can release: {anchored:?}"
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
                &frame(&lines),
                &mut buf,
                &mut ctx,
                |mut row: RowPaint<'_, FinderCat>, buf, _ctx| {
                    let rect = row.rect;
                    if row.kind == ResultRowKind::Cells {
                        for cell in 0..row.source.cell_count(row.row) {
                            let at = Rect::new(rect.x + 3 + cell as u16 * 8, rect.y, 6, 1);
                            buf.set_string(at.x, at.y, row.source.cell_label(row.row, cell), ratatui::style::Style::default());
                            row.place_cell(cell, at);
                            chips.push(row.cell_hit(cell));
                        }
                    } else if row.kind == ResultRowKind::Item {
                        buf.set_string(rect.x + 3, rect.y, row.source.label(row.row).to_uppercase(), ratatui::style::Style::default());
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
        chips.iter().all(|id| hits.regions().iter().any(|r| r.id == *id)),
        "each placed chip registered its region"
    );

    // Every pane in the host's hands, with no answer yet, then answered, then
    // too narrow for the preview.
    let mut hosted = finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let mut paint = HostPaint::default();
    let mut render = |search: &mut SearchDetail, source: &FinderCat, area: Rect, paint: &mut HostPaint| {
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
    assert!(paint.empty && blank.contains("host empty"), "nothing is listed before an answer");
    hosted.submit(request.token, &answer).expect("accepted");
    let drawn = render(&mut hosted, &answer, wide, &mut paint);
    assert!(paint.query && paint.detail && drawn.contains("host detail"));
    assert_eq!(paint.rows.len(), FINDER_ROWS.len());
    assert_eq!(paint.chips, 2);
    assert!(drawn.contains("[one]") && drawn.contains("gamma"));
    let narrow = render(&mut hosted, &answer, Rect::new(0, 0, 60, 24), &mut paint);
    assert!(paint.summary && narrow.contains("host summary"));
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
