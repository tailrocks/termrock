//! `SearchSession`: query requests, stale-result rejection, selection by key
//! and the keyboard, pointer and context paths, driven through the public API.

use crate::common::{chord, key};
use junie_tui::core::edit::{FieldKeys, TextCmd};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::keyed::KeyedSourceError;
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::response::Response;
use junie_tui::core::selection::{Activate, ClickPolicy};
use junie_tui::ui::context::{ContextAnchor, ContextRequest};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::search_detail::{
    MissingSelection, QueryPasteMode, QueryRequest, ResultRowKind, SearchAction, SearchCmd,
    SearchConfig, SearchError, SearchFocusMode, SearchPhase, SearchSession, SearchSource,
};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;

type Reply = Response<SearchAction>;

trait Quiet {
    fn quiet(self);
}

impl<A> Quiet for Response<A> {
    fn quiet(self) {
        let _ = self;
    }
}

fn k(n: u64) -> ItemKey {
    ItemKey::num(n)
}

#[derive(Clone)]
enum R {
    Head(u64, &'static str),
    Item(u64, &'static str),
    Off(u64, &'static str),
    Cells(u64, Vec<(u64, &'static str, bool)>),
    Gap(u64),
    BadItem { key: u64, cell: u64, count: usize },
}

struct Results {
    stamp: SourceStamp,
    rows: Vec<R>,
    hidden: Vec<ItemKey>,
    item_policy: Option<ClickPolicy>,
    cell_policy: Option<ClickPolicy>,
}

impl Results {
    fn new(req: &QueryRequest, rows: Vec<R>) -> Self {
        Self {
            stamp: req.token.result_stamp(),
            rows,
            hidden: Vec::new(),
            item_policy: None,
            cell_policy: None,
        }
    }

    fn row_key(&self, row: usize) -> ItemKey {
        match &self.rows[row] {
            R::Head(n, _) | R::Item(n, _) | R::Off(n, _) | R::Cells(n, _) | R::Gap(n) => k(*n),
            R::BadItem { key, .. } => k(*key),
        }
    }
}

impl RowSource for Results {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn enabled(&self, row: usize) -> bool {
        !matches!(self.rows[row], R::Off(..))
    }

    fn label(&self, row: usize) -> &str {
        match &self.rows[row] {
            R::Head(_, l) | R::Item(_, l) | R::Off(_, l) => l,
            _ => "",
        }
    }

    fn key(&self, row: usize) -> ItemKey {
        self.row_key(row)
    }
}

impl KeyedRowSource for Results {
    fn stamp(&self) -> SourceStamp {
        self.stamp
    }

    fn source_len(&self) -> usize {
        self.source_keys().len()
    }

    fn source_key(&self, index: usize) -> ItemKey {
        self.source_keys()[index]
    }
}

impl Results {
    fn source_keys(&self) -> Vec<ItemKey> {
        let mut keys = Vec::new();
        for (row, r) in self.rows.iter().enumerate() {
            keys.push(self.row_key(row));
            if let R::Cells(_, cells) = r {
                keys.extend(cells.iter().map(|(n, ..)| k(*n)));
            }
        }
        keys.extend(self.hidden.iter().copied());
        keys
    }
}

impl SearchSource for Results {
    fn row_kind(&self, row: usize) -> ResultRowKind {
        match self.rows[row] {
            R::Head(..) => ResultRowKind::Heading,
            R::Item(..) | R::Off(..) | R::BadItem { .. } => ResultRowKind::Item,
            R::Cells(..) => ResultRowKind::Cells,
            R::Gap(_) => ResultRowKind::Spacer,
        }
    }

    fn cell_count(&self, row: usize) -> usize {
        match &self.rows[row] {
            R::Head(..) | R::Gap(_) => 0,
            R::Item(..) | R::Off(..) => 1,
            R::BadItem { count, .. } => *count,
            R::Cells(_, cells) => cells.len(),
        }
    }

    fn cell_key(&self, row: usize, cell: usize) -> ItemKey {
        match &self.rows[row] {
            R::BadItem { cell, .. } => k(*cell),
            R::Cells(_, cells) => k(cells[cell].0),
            _ => self.row_key(row),
        }
    }

    fn cell_label(&self, row: usize, cell: usize) -> &str {
        match &self.rows[row] {
            R::Cells(_, cells) => cells[cell].1,
            _ => self.label(row),
        }
    }

    fn cell_enabled(&self, row: usize, cell: usize) -> bool {
        match &self.rows[row] {
            R::Cells(_, cells) => cells[cell].2,
            _ => self.enabled(row),
        }
    }

    fn cell_click_policy(&self, row: usize, _cell: usize) -> Option<ClickPolicy> {
        match self.rows[row] {
            R::Cells(..) => self.cell_policy,
            _ => self.item_policy,
        }
    }
}

fn session(config: SearchConfig) -> SearchSession {
    let mut s = SearchSession::new(TextInput::new(WidgetId::of("t.search"), "Search"), config);
    s.configure_query(|field| field.keys = FieldKeys::Desktop)
        .quiet();
    s
}

fn combined() -> SearchSession {
    session(SearchConfig::default().focus_mode(SearchFocusMode::Combined))
}

fn input(data: u64) -> SourceStamp {
    SourceStamp {
        instance: k(7_000),
        data,
        view: 0,
    }
}

fn acts(reply: Reply) -> Vec<SearchAction> {
    reply.into_actions().into_iter().collect()
}

fn requests(reply: Reply) -> Vec<QueryRequest> {
    acts(reply)
        .into_iter()
        .filter_map(|a| match a {
            SearchAction::QueryRequested(r) => Some(r),
            _ => None,
        })
        .collect()
}

fn one_request(reply: Reply) -> QueryRequest {
    let mut all = requests(reply);
    assert_eq!(all.len(), 1, "exactly one request");
    all.remove(0)
}

fn answer(s: &mut SearchSession, req: &QueryRequest, rows: Vec<R>) -> Vec<SearchAction> {
    let source = Results::new(req, rows);
    acts(s.submit(req.token, &source).expect("results accepted"))
}

fn type_text(s: &mut SearchSession, text: &str) -> Vec<Reply> {
    text.chars()
        .map(|c| s.on_key(&key(KeyCode::Char(c)), true, 5))
        .collect()
}

fn started(s: &mut SearchSession, rows: Vec<R>) -> QueryRequest {
    let req = one_request(s.refresh_source(input(1)));
    answer(s, &req, rows);
    req
}

fn list() -> Vec<R> {
    vec![
        R::Head(1, "Group"),
        R::Item(10, "a"),
        R::Off(11, "b"),
        R::Item(12, "c"),
        R::Gap(2),
        R::Head(3, "More"),
        R::Item(13, "d"),
        R::Item(14, "e"),
    ]
}

fn ctrl(c: char) -> junie_tui::core::event::Key {
    chord(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn search_detail_query_requests_once_per_change() {
    let mut s = session(SearchConfig::default());
    let initial = one_request(s.refresh_source(input(1)));
    assert_eq!(initial.text, "");
    assert!(
        acts(s.refresh_source(input(1))).is_empty(),
        "an equal stamp requests nothing"
    );
    assert!(
        acts(s.set_query("")).is_empty(),
        "an equal query requests nothing"
    );

    let typed = type_text(&mut s, "ab");
    let all: Vec<_> = typed.into_iter().flat_map(requests).collect();
    assert_eq!(
        all.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a", "ab"]
    );
    assert!(all[1].token.generation > all[0].token.generation);

    assert_eq!(
        one_request(s.on_key(&ctrl('z'), true, 5)).text,
        "",
        "typing folds into one undo step"
    );
    assert_eq!(one_request(s.on_key(&ctrl('y'), true, 5)).text, "ab");
    assert_eq!(one_request(s.set_query("xyz")).text, "xyz");
    assert_eq!(one_request(s.on_paste("!", true)).text, "xyz!");
    assert_eq!(one_request(s.refresh_source(input(2))).text, "xyz!");
}

#[test]
fn search_detail_initial_source_and_terminal_completion() {
    let mut s = session(SearchConfig::default());
    assert_eq!(s.phase(), SearchPhase::Uninitialized);
    assert!(requests(s.set_query("docker clean")).is_empty());
    let req = one_request(s.refresh_source(input(1)));
    assert_eq!(req.text, "docker clean");
    assert_eq!(req.token.source, input(1));
    assert_eq!(s.phase(), SearchPhase::Loading);

    let source = Results::new(&req, list());
    s.submit(req.token, &source).expect("accepted").quiet();
    assert_eq!(s.phase(), SearchPhase::Ready);

    assert_eq!(
        s.fail(req.token, "late".into()).err(),
        Some(SearchError::AlreadyCompleted)
    );
    assert_eq!(s.phase(), SearchPhase::Ready, "late fail changes nothing");
    assert_eq!(s.error(), None);
    let again = s.submit(req.token, &source).expect("exact duplicate");
    assert!(
        !again.is_consumed() && !again.has_actions(),
        "duplicate is a no-op"
    );
    assert_eq!(s.selected_key(), Some(k(10)));
}

#[test]
fn search_detail_failed_requires_retry() {
    let mut s = session(SearchConfig::default());
    let req = one_request(s.refresh_source(input(1)));
    s.fail(req.token, "boom".into()).expect("current").quiet();
    assert_eq!(s.phase(), SearchPhase::Failed);
    assert_eq!(s.error(), Some("boom"));
    let source = Results::new(&req, list());
    assert_eq!(
        s.submit(req.token, &source).err(),
        Some(SearchError::AlreadyCompleted)
    );
    let retry = one_request(s.retry());
    assert_eq!(s.error(), None);
    assert_ne!(retry.token, req.token);
    answer(&mut s, &retry, list());
    assert_eq!(s.phase(), SearchPhase::Ready);
}

#[test]
fn search_detail_rejects_stale_submission() {
    let mut s = session(SearchConfig::default());
    let first = started(&mut s, list());
    s.select(k(12)).quiet();
    let a = one_request(s.on_paste("a", true));
    let b = one_request(s.on_paste("b", true));
    let stale = Results::new(&a, vec![R::Item(13, "stale")]);
    assert_eq!(
        s.submit(a.token, &stale).err(),
        Some(SearchError::StaleQuery)
    );
    assert_eq!(
        s.fail(a.token, "x".into()).err(),
        Some(SearchError::StaleQuery)
    );
    assert_eq!(s.phase(), SearchPhase::Loading);
    assert_eq!(
        s.selected_key(),
        Some(k(12)),
        "stale result did not move selection"
    );
    assert_eq!(
        s.accepted_stamp(),
        Some(first.token.result_stamp()),
        "the retained projection is untouched"
    );
    answer(&mut s, &b, vec![R::Item(14, "fresh")]);
    assert_eq!(s.selected_key(), Some(k(14)));
}

#[test]
fn search_detail_rejects_other_controller_token() {
    let mut one = session(SearchConfig::default());
    let mut two = session(SearchConfig::default());
    let a = one_request(one.refresh_source(input(1)));
    let b = one_request(two.refresh_source(input(1)));
    assert_eq!(a.token.generation, b.token.generation);
    assert_eq!(a.token.source, b.token.source);
    assert_ne!(a.token, b.token, "owner differs");
    let source = Results::new(&a, list());
    assert_eq!(
        one.submit(b.token, &source).err(),
        Some(SearchError::StaleQuery)
    );
    assert_eq!(
        two.submit(a.token, &source).err(),
        Some(SearchError::StaleQuery)
    );
    assert_eq!(one.phase(), SearchPhase::Loading);
    assert_eq!(one.selected_key(), None);
}

#[test]
fn search_detail_invalid_source_leaves_request_pending() {
    let mut s = session(SearchConfig::default());
    let req = one_request(s.refresh_source(input(1)));

    let mut wrong = Results::new(&req, list());
    wrong.stamp.view += 1;
    assert!(matches!(
        s.submit(req.token, &wrong),
        Err(SearchError::StampMismatch { .. })
    ));

    let dup = Results::new(&req, vec![R::Item(1, "x"), R::Item(1, "y")]);
    assert!(matches!(
        s.submit(req.token, &dup),
        Err(SearchError::InvalidSource(
            KeyedSourceError::DuplicateAuthoritative { .. }
        ))
    ));
    let positional = Results::new(&req, vec![R::Item(1, "x")]);
    let mut positional_keys = positional;
    positional_keys.hidden.push(ItemKey::index(0));
    assert!(matches!(
        s.submit(req.token, &positional_keys),
        Err(SearchError::InvalidSource(
            KeyedSourceError::PositionalKey { .. }
        ))
    ));
    assert_eq!(s.phase(), SearchPhase::Loading);
    assert_eq!(s.selected_key(), None);
    answer(&mut s, &req, list());
    assert_eq!(s.phase(), SearchPhase::Ready);
}

#[test]
fn search_detail_item_key_alias_validates_once() {
    let mut s = session(SearchConfig::default());
    let req = one_request(s.refresh_source(input(1)));
    let alias = Results::new(&req, vec![R::Item(1, "x")]);
    assert_eq!(alias.source_len(), 1, "item and its cell are one key");
    s.submit(req.token, &alias).expect("alias accepted").quiet();
    assert_eq!(s.selectable_len(), 1);

    let mut s = session(SearchConfig::default());
    let req = one_request(s.refresh_source(input(1)));
    for bad in [
        R::BadItem {
            key: 1,
            cell: 2,
            count: 1,
        },
        R::BadItem {
            key: 1,
            cell: 1,
            count: 2,
        },
        R::BadItem {
            key: 1,
            cell: 1,
            count: 0,
        },
    ] {
        let source = Results::new(&req, vec![R::Item(9, "ok"), bad]);
        assert_eq!(
            s.submit(req.token, &source).err(),
            Some(SearchError::InconsistentRow { row: 1 })
        );
    }
    assert_eq!(s.phase(), SearchPhase::Loading);
}

#[test]
fn search_detail_pending_renders_only_last_accepted_source() {
    let mut s = session(SearchConfig::default());
    let first = started(&mut s, list());
    let shown = first.token.result_stamp();
    assert_eq!(s.verify_display(shown), Ok(()));
    let next = one_request(s.on_paste("q", true));
    assert_eq!(
        s.accepted_stamp(),
        Some(shown),
        "old results stay on display"
    );
    assert_eq!(
        s.verify_display(next.token.result_stamp()),
        Err(SearchError::UnsubmittedProjection),
        "the pending source cannot render before submit"
    );
    answer(&mut s, &next, list());
    assert_eq!(s.verify_display(next.token.result_stamp()), Ok(()));
    assert_eq!(
        s.verify_display(shown),
        Err(SearchError::UnsubmittedProjection)
    );
}

#[test]
fn search_detail_grouped_cells_preserve_identity() {
    let cells = |order: &[u64]| {
        R::Cells(
            50,
            order.iter().map(|n| (*n, "cell", true)).collect::<Vec<_>>(),
        )
    };
    let mut s = session(SearchConfig::default());
    started(
        &mut s,
        vec![R::Item(1, "x"), cells(&[60, 61, 62]), R::Item(2, "y")],
    );
    assert_eq!(s.select(k(62)).into_actions().len(), 1);
    assert_eq!(s.selected_cell(), Some((1, 2)));

    let req = one_request(s.refresh_source(input(2)));
    let out = answer(
        &mut s,
        &req,
        vec![R::Item(3, "new"), R::Item(2, "y"), cells(&[62, 60, 61])],
    );
    assert_eq!(s.selected_key(), Some(k(62)), "same cell key after reorder");
    assert_eq!(s.selected_cell(), Some((2, 0)));
    assert!(
        !out.iter()
            .any(|a| matches!(a, SearchAction::SelectionChanged(_))),
        "the key did not change, so no selection event"
    );
}

#[test]
fn search_detail_skips_headings_and_disabled_cells() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    assert_eq!(s.selected_key(), Some(k(10)), "first enabled item");
    let mut seen = vec![];
    for _ in 0..5 {
        s.on_key(&key(KeyCode::Down), false, 3).quiet();
        seen.push(s.selected_key().expect("selected"));
    }
    assert_eq!(seen, [k(12), k(13), k(14), k(14), k(14)]);
    for _ in 0..4 {
        s.on_key(&key(KeyCode::Up), false, 3).quiet();
    }
    assert_eq!(s.selected_key(), Some(k(10)));
    s.on_key(&key(KeyCode::End), false, 3).quiet();
    assert_eq!(s.selected_key(), Some(k(14)));
    s.on_key(&key(KeyCode::Home), false, 3).quiet();
    assert_eq!(s.selected_key(), Some(k(10)));
    s.on_key(&key(KeyCode::PageDown), false, 3).quiet();
    assert_eq!(
        s.selected_key(),
        Some(k(13)),
        "row 1 plus 3 rows is a gap, so the next eligible row"
    );
    s.on_key(&key(KeyCode::PageDown), false, 3).quiet();
    assert_eq!(
        s.selected_key(),
        Some(k(14)),
        "past the end lands on the last"
    );
    s.on_key(&key(KeyCode::PageUp), false, 3).quiet();
    assert_eq!(s.selected_key(), Some(k(12)));
}

#[test]
fn search_detail_cells_navigate_horizontally_without_wrapping() {
    let rows = || {
        vec![
            R::Cells(50, vec![(60, "a", true), (61, "b", false), (62, "c", true)]),
            R::Item(1, "x"),
            R::Cells(51, vec![(70, "a", true), (71, "b", true)]),
        ]
    };
    let mut s = combined();
    started(&mut s, rows());
    assert_eq!(s.selected_key(), Some(k(60)));
    s.on_key(&key(KeyCode::Left), true, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(60)), "no wrap at the left edge");
    s.on_key(&key(KeyCode::Right), true, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(62)), "disabled cell skipped");
    s.on_key(&key(KeyCode::Right), true, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(62)), "no wrap at the right edge");
    s.on_key(&key(KeyCode::Down), true, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(1)));
    let r = s.on_key(&key(KeyCode::Right), true, 5);
    assert!(!r.is_consumed(), "Left and Right ignore a non-cell row");
    s.on_key(&key(KeyCode::Down), true, 5).quiet();
    assert_eq!(
        s.selected_key(),
        Some(k(71)),
        "the column chosen on the first row is kept across an item row"
    );
}

#[test]
fn search_detail_query_change_selects_first() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    s.select(k(14)).quiet();
    let req = one_request(s.on_paste("q", true));
    let out = answer(&mut s, &req, list());
    assert_eq!(out, [SearchAction::SelectionChanged(Some(k(10)))]);

    let mut keep = session(SearchConfig::default().select_first_on_query(false));
    started(&mut keep, list());
    keep.select(k(14)).quiet();
    let req = one_request(keep.on_paste("q", true));
    let out = answer(&mut keep, &req, list());
    assert!(out.is_empty());
    assert_eq!(keep.selected_key(), Some(k(14)));
}

#[test]
fn search_detail_source_refresh_keeps_selection_by_key() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    s.select(k(13)).quiet();
    let req = one_request(s.refresh_source(input(2)));
    let reordered = vec![
        R::Item(14, "e"),
        R::Item(99, "n"),
        R::Item(13, "d"),
        R::Item(10, "a"),
    ];
    let out = answer(&mut s, &req, reordered);
    assert_eq!(s.selected_key(), Some(k(13)));
    assert!(out.is_empty());
}

#[test]
fn search_detail_missing_selection_policy() {
    for (policy, expected) in [
        (MissingSelection::NearestPosition, k(14)),
        (MissingSelection::FirstEnabled, k(10)),
    ] {
        let mut s = session(SearchConfig::default().missing_selection(policy));
        started(&mut s, list());
        s.select(k(13)).quiet();
        let req = one_request(s.refresh_source(input(2)));
        let out = answer(
            &mut s,
            &req,
            vec![R::Item(10, "a"), R::Item(12, "c"), R::Item(14, "e")],
        );
        assert_eq!(s.selected_key(), Some(expected), "{policy:?}");
        assert_eq!(out, [SearchAction::SelectionChanged(Some(expected))]);
    }
}

#[test]
fn search_detail_loading_cannot_activate_stale_result() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    assert_eq!(
        acts(s.on_key(&key(KeyCode::Enter), false, 5)),
        [SearchAction::Activate(k(10))]
    );
    let pending = one_request(s.on_paste("q", true));
    let _ = pending;
    for cmd in [SearchCmd::Activate, SearchCmd::Context, SearchCmd::Down] {
        assert!(acts(s.run(cmd, 5)).is_empty(), "{cmd:?} while pending");
    }
    assert!(acts(s.detail_action("open")).is_empty());
    assert!(acts(s.context_at(k(10), Position::new(1, 1))).is_empty());
    assert!(acts(s.click(k(12), 1, 2)).is_empty());
    assert_eq!(s.selected_key(), Some(k(10)));

    let req = s.retry();
    let req = one_request(req);
    s.fail(req.token, "down".into()).expect("current").quiet();
    assert!(acts(s.run(SearchCmd::Activate, 5)).is_empty(), "failed");
    assert!(acts(s.detail_action("open")).is_empty());
}

#[test]
fn search_detail_loading_navigation_is_configurable() {
    let mut s = session(SearchConfig::default().navigate_while_loading(true));
    started(&mut s, list());
    one_request(s.on_paste("q", true));
    assert_eq!(
        acts(s.run(SearchCmd::Down, 5)),
        [SearchAction::SelectionChanged(Some(k(12)))]
    );
    assert!(acts(s.run(SearchCmd::Activate, 5)).is_empty());
}

#[test]
fn search_detail_context_captures_key_and_anchor() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    s.select(k(13)).quiet();
    s.begin_context().quiet();
    assert_eq!(s.context_intent(), Some(k(13)));

    let req = one_request(s.refresh_source(input(2)));
    answer(
        &mut s,
        &req,
        vec![R::Item(14, "e"), R::Item(13, "d"), R::Item(10, "a")],
    );
    let anchor = ContextAnchor::Pointer(Position::new(4, 2));
    assert_eq!(
        acts(s.emit_context(anchor)),
        [SearchAction::ContextRequested(ContextRequest {
            target: k(13),
            anchor
        })],
        "the original key, not the row now at the old position"
    );

    s.begin_context().quiet();
    let req = one_request(s.refresh_source(input(3)));
    answer(&mut s, &req, vec![R::Item(14, "e"), R::Item(10, "a")]);
    assert!(
        acts(s.emit_context(anchor)).is_empty(),
        "deleted key cancels the intent"
    );
    assert_eq!(s.context_intent(), None);

    s.select(k(10)).quiet();
    let out = acts(s.context_at(k(14), Position::new(9, 9)));
    assert_eq!(
        out,
        [
            SearchAction::SelectionChanged(Some(k(14))),
            SearchAction::ContextRequested(ContextRequest {
                target: k(14),
                anchor: ContextAnchor::Pointer(Position::new(9, 9)),
            }),
        ]
    );
}

#[test]
fn search_detail_detail_action_captures_selected_key() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    s.select(k(13)).quiet();
    assert_eq!(
        acts(s.detail_action("copy")),
        [SearchAction::DetailAction {
            key: k(13),
            name: "copy"
        }]
    );
}

#[test]
fn search_detail_click_policy_is_per_cell() {
    let rows = || {
        vec![
            R::Item(1, "item"),
            R::Cells(50, vec![(60, "a", true), (61, "b", true)]),
        ]
    };
    let mut s = session(SearchConfig::default());
    let req = one_request(s.refresh_source(input(1)));
    let mut source = Results::new(&req, rows());
    source.item_policy = Some(ClickPolicy::DATA);
    source.cell_policy = Some(ClickPolicy::NAVIGATION);
    s.submit(req.token, &source).expect("accepted").quiet();

    assert!(
        acts(s.click(k(1), 1, 1)).is_empty(),
        "a single click on a data item only selects an already selected row"
    );
    assert_eq!(
        acts(s.click(k(1), 1, 2)),
        [SearchAction::Activate(k(1))],
        "double click activates a data item"
    );
    assert_eq!(
        acts(s.click(k(61), 1, 1)),
        [
            SearchAction::SelectionChanged(Some(k(61))),
            SearchAction::Activate(k(61))
        ],
        "a navigation cell selects then activates, once"
    );
    assert_eq!(s.selected_cell(), Some((1, 1)));

    let mut again =
        session(SearchConfig::default().click(ClickPolicy::DATA.activate(Activate::ClickAgain)));
    started(&mut again, rows());
    assert!(
        acts(again.click(k(61), 1, 1)).len() == 1,
        "first click only selects"
    );
    assert_eq!(
        acts(again.click(k(61), 1, 1)),
        [SearchAction::Activate(k(61))],
        "click on the selected row again activates"
    );
}

#[test]
fn search_detail_layout_change_cancels_press() {
    let mut s = session(SearchConfig::default().click(ClickPolicy::NAVIGATION));
    started(&mut s, list());
    s.press(k(12), 1).quiet();
    assert!(s.invalidate_layout(2));
    assert!(
        acts(s.release(k(12), 2, 1)).is_empty(),
        "moved layout never activates"
    );
    assert!(
        acts(s.release(k(12), 2, 1)).is_empty(),
        "every release consumes the capture"
    );
    s.press(k(12), 2).quiet();
    assert_eq!(
        acts(s.release(k(12), 2, 1)),
        [SearchAction::Activate(k(12))]
    );

    s.press(k(13), 2).quiet();
    let req = one_request(s.refresh_source(input(2)));
    answer(&mut s, &req, vec![R::Item(10, "a"), R::Item(13, "d")]);
    assert!(
        acts(s.release(k(13), 2, 1)).is_empty(),
        "a row that moved under the press is not activated"
    );
}

#[test]
fn search_detail_headings_and_disabled_rows_are_not_hit_targets() {
    let mut s = session(SearchConfig::default().click(ClickPolicy::NAVIGATION));
    started(&mut s, list());
    for dead in [k(1), k(11), k(2), k(3), k(404)] {
        assert!(acts(s.click(dead, 1, 1)).is_empty(), "{dead:?}");
        s.select(dead).quiet();
        assert_eq!(s.selected_key(), Some(k(10)), "{dead:?} is not selectable");
    }
    assert_eq!(s.selected_key(), Some(k(10)));
}

#[test]
fn search_detail_query_command_reports_effectiveness() {
    let mut s = combined();
    s.refresh_source(input(1)).quiet();
    let command = |reply: Reply| {
        acts(reply).into_iter().find_map(|a| match a {
            SearchAction::QueryCommand { cmd, effective } => Some((cmd, effective)),
            _ => None,
        })
    };
    assert_eq!(
        command(s.on_key(&ctrl('a'), true, 5)),
        Some((TextCmd::SelectAll, false)),
        "select all on an empty query"
    );
    assert_eq!(
        command(s.on_key(&ctrl('z'), true, 5)),
        Some((TextCmd::Undo, false)),
        "nothing to undo"
    );
    assert_eq!(
        command(s.on_key(&ctrl('y'), true, 5)),
        Some((TextCmd::Redo, false)),
        "nothing to redo"
    );
    type_text(&mut s, "ab");
    assert_eq!(
        command(s.on_key(&ctrl('a'), true, 5)),
        Some((TextCmd::SelectAll, true))
    );
    let undo = s.on_key(&ctrl('z'), true, 5);
    let sequence = acts(undo);
    assert!(matches!(
        sequence.as_slice(),
        [
            SearchAction::QueryCommand {
                cmd: TextCmd::Undo,
                effective: true
            },
            SearchAction::QueryRequested(_)
        ]
    ));
    assert_eq!(
        command(s.on_key(&ctrl('y'), true, 5)),
        Some((TextCmd::Redo, true))
    );
    assert_eq!(s.query(), "ab");
}

#[test]
fn search_detail_combined_focus_preserves_holla_query_routing() {
    let mut s = session(
        SearchConfig::default()
            .focus_mode(SearchFocusMode::Combined)
            .paste_mode(QueryPasteMode::JoinLinesWithSpaceTrim),
    );
    started(&mut s, list());
    type_text(&mut s, "ls");
    assert_eq!(s.query(), "ls");
    s.on_key(&key(KeyCode::Down), true, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(10)), "pending results stay inert");

    let left = s.on_key(&key(KeyCode::Left), true, 5);
    assert!(!left.is_consumed(), "Left on a non-cell row is the host's");
    assert_eq!(s.query(), "ls", "Left never edits in Combined");

    let esc = key(KeyCode::Esc);
    s.on_key(&ctrl('a'), true, 5).quiet();
    let r = s.on_key(&esc, true, 5);
    assert!(r.is_consumed() && !r.has_actions());
    assert_eq!(s.query(), "ls", "first Esc only clears the selection");
    let cleared = s.on_key(&esc, true, 5);
    assert_eq!(one_request(cleared).text, "", "second Esc clears the query");
    let last = s.on_key(&esc, true, 5);
    assert!(!last.is_consumed(), "third Esc is left to the host");

    assert!(
        s.on_key(&key(KeyCode::Backspace), true, 5).is_consumed(),
        "Backspace on an empty query is consumed"
    );
    assert!(requests(s.on_key(&key(KeyCode::Backspace), true, 5)).is_empty());

    let paste = one_request(s.on_paste("  one\r\ntwo\nthree  ", true));
    assert_eq!(paste.text, "one  two three");
    assert!(
        acts(s.on_paste("\n\r  \n", true)).is_empty(),
        "blank paste does nothing"
    );
}

#[test]
fn search_detail_separate_focus_gives_text_first_refusal() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    type_text(&mut s, "j");
    assert_eq!(s.query(), "j");
    s.on_key(&key(KeyCode::Down), true, 5).quiet();
    assert_eq!(s.query(), "j", "Down is not text");
    s.on_key(&key(KeyCode::Left), true, 5).quiet();
    assert_eq!(
        s.selected_key(),
        Some(k(10)),
        "Left edits the query, not cells"
    );

    let enter = s.on_key(&key(KeyCode::Enter), true, 5);
    assert!(
        acts(enter).is_empty(),
        "Enter on a loading query activates nothing"
    );

    for cmd in [
        SearchCmd::FocusResults,
        SearchCmd::FocusQuery,
        SearchCmd::OpenDetail,
    ] {
        let binding = s
            .config()
            .keymap
            .iter()
            .find(|(_, c)| *c == cmd)
            .map(|(b, _)| *b)
            .expect("bound by default");
        let host = junie_tui::core::event::Key {
            code: binding.code,
            mods: binding.mods,
        };
        assert_eq!(s.command_for(&host), Some(cmd));
        assert!(
            !s.on_key(&host, true, 5).is_consumed(),
            "{cmd:?} is the host's"
        );
        assert_eq!(s.query(), "j");
    }
}

#[test]
fn search_detail_paste_respects_detail_focus() {
    let mut s = session(SearchConfig::default());
    started(&mut s, list());
    assert!(acts(s.on_paste("abc", false)).is_empty());
    assert_eq!(s.query(), "", "detail paste does not edit the query");

    let mut back = session(SearchConfig::default().type_to_query_from_detail(true));
    started(&mut back, list());
    assert_eq!(one_request(back.on_paste("abc", false)).text, "abc");

    let mut strip = session(SearchConfig::default());
    started(&mut strip, list());
    assert_eq!(one_request(strip.on_paste("a\nb\r\nc", true)).text, "abc");
}

#[test]
fn search_detail_keymap_is_overridable() {
    let mut s = session(SearchConfig::default().keymap(vec![
        (
            junie_tui::core::keymap::KeyBinding::char('j'),
            SearchCmd::Down,
        ),
        (
            junie_tui::core::keymap::KeyBinding::char('k'),
            SearchCmd::Up,
        ),
    ]));
    started(&mut s, list());
    s.on_key(&key(KeyCode::Char('j')), false, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(12)));
    s.on_key(&key(KeyCode::Char('k')), false, 5).quiet();
    assert_eq!(s.selected_key(), Some(k(10)));
    assert!(
        !s.on_key(&key(KeyCode::Down), false, 5).is_consumed(),
        "unbound now"
    );
}

#[test]
fn search_detail_configure_query_requests_on_text_change() {
    let mut s = session(SearchConfig::default());
    s.refresh_source(input(1)).quiet();
    let unchanged = s.configure_query(|field| field.undo_runs = false);
    assert!(requests(unchanged).is_empty());
    let changed = s.configure_query(|field| field.set_text("seeded"));
    assert_eq!(one_request(changed).text, "seeded");
}
