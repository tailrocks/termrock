//! `SearchDetail`: the composed query, results and preview, driven through
//! rendered hit regions, focus and the public input paths.

use crate::common::{chord, key, row};
use crate::search_detail::{R, Results, acts, input, k, list, one_request};
use junie_tui::core::edit::FieldKeys;
use junie_tui::core::event::Key;
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::list::{KeyedRowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::response::Response;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::theme::{ColorLevel, Theme, Tone};
use junie_tui::ui::context::{ContextAnchor, ContextConfig, ContextSource};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list_detail::{
    DetailPane, Line, ListDetailSource, ListRowRef, SourceFrame,
};
use junie_tui::widgets::search_detail::{
    QueryRequest, QueryView, ResultRowKind, RowPaint, SearchAction, SearchConfig, SearchDetail,
    SearchError, SearchFocusMode, SearchPaint, SearchPhase, paint_query_default,
};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("sd");
const QUERY: WidgetId = WidgetId::of("sd.q");
const WIDE: Rect = Rect::new(0, 0, 120, 14);
const NARROW: Rect = Rect::new(0, 0, 60, 14);

impl ListDetailSource for Results {
    fn row(&self, visible: usize) -> ListRowRef<'_> {
        use junie_tui::core::list::RowSource;
        ListRowRef {
            mark: None,
            label: self.label(visible),
            detail: "",
            meta: "",
            meta_tone: Tone::Muted,
            faint: false,
        }
    }
}

trait Quiet {
    fn quiet(self);
}

impl<A> Quiet for Response<A> {
    fn quiet(self) {
        let _ = self;
    }
}

struct Frame {
    buf: Buffer,
    hits: HitRegistry,
}

impl Frame {
    fn text(&self) -> String {
        (0..self.buf.area.height)
            .map(|y| row(&self.buf, y).trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn hit(&self, part: Part, key: ItemKey) -> (WidgetId, Rect) {
        self.hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == ID && r.part.part == part && r.part.item == Some(key))
            .map(|r| (r.id, r.area))
            .unwrap_or_else(|| panic!("no {part:?} hit for {key:?}"))
    }

    fn event(&self, phase: Phase, key: ItemKey, part: Part, count: u8) -> PointerEvent {
        let (id, area) = self.hit(part, key);
        PointerEvent::new(
            &self.hits,
            phase,
            id,
            Position::new(area.x + 1, area.y),
            count,
            KeyModifiers::NONE,
        )
    }

    fn field_event(&self, phase: Phase) -> PointerEvent {
        let region = self
            .hits
            .regions()
            .iter()
            .find(|r| r.owner == ID && r.part.part == Part::FIELD)
            .expect("a query hit");
        PointerEvent::new(
            &self.hits,
            phase,
            region.id,
            region.area.as_position(),
            1,
            KeyModifiers::NONE,
        )
    }
}

fn frame_lines(n: usize) -> Vec<Line> {
    (0..n)
        .map(|i| Line::Text(format!("preview line {i:02}"), Tone::Normal))
        .collect()
}

fn detail_frame(lines: &[Line]) -> SourceFrame<'_> {
    SourceFrame {
        title: "Results",
        meta: "",
        head: &[],
        under: None,
        detail_title: "Preview",
        detail_meta: "",
        lines,
        actions: &[],
    }
}

#[derive(Default)]
struct Seen {
    stale: bool,
    selected_cells: Vec<(usize, Option<usize>)>,
    query: Option<(String, bool, Option<std::ops::Range<usize>>, usize)>,
}

fn draw(
    search: &mut SearchDetail,
    source: &Results,
    area: Rect,
    focus: Option<WidgetId>,
    lines: &[Line],
    seen: &mut Seen,
) -> Frame {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        search
            .render_source_with(
                area,
                source,
                &detail_frame(lines),
                &mut buf,
                &mut ctx,
                |mut row: RowPaint<'_, Results>, buf: &mut Buffer, _ctx: &mut RenderCtx| {
                    seen.stale = row.stale;
                    seen.selected_cells.push((row.row, row.selected_cell));
                    let marker = if row.selected { ">" } else { " " };
                    match row.kind {
                        ResultRowKind::Cells => {
                            let mut x = row.rect.x + 2;
                            for cell in 0
                                ..junie_tui::widgets::search_detail::SearchSource::cell_count(
                                    row.source, row.row,
                                )
                            {
                                let label =
                                    junie_tui::widgets::search_detail::SearchSource::cell_label(
                                        row.source, row.row, cell,
                                    );
                                let on = row.selected_cell == Some(cell);
                                let text = if on {
                                    format!("[{label}]")
                                } else {
                                    format!(" {label} ")
                                };
                                let w = text.len() as u16;
                                buf.set_string(x, row.rect.y, &text, ratatui::style::Style::new());
                                row.place_cell(cell, Rect::new(x, row.rect.y, w, 1));
                                x += w + 1;
                            }
                        }
                        _ => {
                            use junie_tui::core::list::RowSource;
                            let label = row.source.label(row.row);
                            buf.set_string(
                                row.rect.x,
                                row.rect.y,
                                format!("{marker} {label}"),
                                ratatui::style::Style::new(),
                            );
                        }
                    }
                },
                |area, view: QueryView<'_>, buf: &mut Buffer, ctx: &mut RenderCtx| {
                    seen.query = Some((
                        view.text.to_owned(),
                        view.focused,
                        view.selection.clone(),
                        view.caret_byte,
                    ));
                    paint_query_default(area, view, buf, ctx);
                },
            )
            .expect("the accepted source");
    }
    Frame { buf, hits }
}

fn new_search(config: SearchConfig) -> SearchDetail {
    let mut search = SearchDetail::new(ID, TextInput::new(QUERY, "Search"), Split::new(50, 36, 40))
        .config(config);
    search
        .configure_query(|field| field.keys = FieldKeys::Desktop)
        .quiet();
    search
}

fn mixed() -> Vec<R> {
    vec![
        R::Head(1, "Group"),
        R::Item(10, "alpha"),
        R::Cells(
            3,
            vec![(30, "one", true), (31, "two", true), (32, "three", true)],
        ),
        R::Item(12, "charlie"),
    ]
}

fn accept(search: &mut SearchDetail, stamp: u64, rows: Vec<R>) -> QueryRequest {
    let req = one_request(search.refresh_source(input(stamp)));
    let source = Results::new(&req, rows);
    search.submit(req.token, &source).expect("accepted").quiet();
    req
}

fn accept_with_policy(
    search: &mut SearchDetail,
    rows: Vec<R>,
    item: Option<ClickPolicy>,
    cell: Option<ClickPolicy>,
) -> Results {
    let req = one_request(search.refresh_source(input(1)));
    let mut source = Results::new(&req, rows);
    source.item_policy = item;
    source.cell_policy = cell;
    search.submit(req.token, &source).expect("accepted").quiet();
    source
}

fn source_for(req: &QueryRequest, rows: Vec<R>) -> Results {
    Results::new(req, rows)
}

fn on_key(
    search: &mut SearchDetail,
    source: &Results,
    focus: &mut Focus,
    key: &Key,
) -> Vec<SearchAction> {
    acts(
        search
            .on_key(key, source, focus)
            .expect("the accepted source"),
    )
}

fn press_click(
    search: &mut SearchDetail,
    source: &Results,
    focus: &mut Focus,
    frame: &Frame,
    key: ItemKey,
    part: Part,
) -> Vec<SearchAction> {
    let mut out = acts(
        search
            .pointer(&frame.event(Phase::Press, key, part, 1), source, focus)
            .expect("source"),
    );
    out.extend(acts(
        search
            .pointer(&frame.event(Phase::Click, key, part, 1), source, focus)
            .expect("source"),
    ));
    out
}

fn ctrl(c: char) -> Key {
    chord(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn scrolled_top(frame: &Frame) -> Option<String> {
    frame
        .text()
        .lines()
        .find_map(|l| l.find("preview line ").map(|i| l[i..i + 15].to_owned()))
}

#[test]
fn search_detail_registers_row_and_cell_hits_by_stable_key() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut seen);
    frame.hit(Part::ROW, k(10));
    frame.hit(Part::ROW, k(12));
    for cell in [30, 31, 32] {
        frame.hit(Part::CELL, k(cell));
    }
    assert_eq!(
        seen.selected_cells.iter().find(|(r, _)| *r == 1),
        Some(&(1, Some(0)))
    );
    assert_eq!(
        seen.selected_cells.iter().find(|(r, _)| *r == 2),
        Some(&(2, None))
    );
    assert!(frame.text().contains(" one "), "{}", frame.text());
}

#[test]
fn search_detail_click_policy_decides_activation_by_cell_kind() {
    let mut search = new_search(SearchConfig::default());
    let source = accept_with_policy(
        &mut search,
        mixed(),
        Some(ClickPolicy::DATA),
        Some(ClickPolicy::NAVIGATION),
    );
    let mut focus = Focus::default();
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut seen);

    let out = press_click(&mut search, &source, &mut focus, &frame, k(32), Part::CELL);
    assert_eq!(
        out,
        [
            SearchAction::SelectionChanged(Some(k(32))),
            SearchAction::Activate(k(32))
        ],
        "a cell activates once, after its selection"
    );
    let frame = draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    let out = press_click(&mut search, &source, &mut focus, &frame, k(12), Part::ROW);
    assert_eq!(
        out,
        [SearchAction::SelectionChanged(Some(k(12)))],
        "an item needs a second click under the data policy"
    );
    assert_eq!(focus.current(), Some(ID), "a row click focuses the results");
}

#[test]
fn search_detail_resize_between_press_and_release_cancels_the_click() {
    let mut search = new_search(SearchConfig::default());
    let source = accept_with_policy(&mut search, mixed(), None, Some(ClickPolicy::NAVIGATION));
    let mut focus = Focus::default();
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut seen);
    search
        .pointer(
            &frame.event(Phase::Press, k(31), Part::CELL, 1),
            &source,
            &mut focus,
        )
        .expect("source")
        .quiet();
    let moved = draw(
        &mut search,
        &source,
        Rect::new(0, 0, 100, 14),
        None,
        &[],
        &mut seen,
    );
    let out = acts(
        search
            .pointer(
                &moved.event(Phase::Click, k(31), Part::CELL, 1),
                &source,
                &mut focus,
            )
            .expect("source"),
    );
    assert!(
        out.is_empty(),
        "no activation after the layout moved: {out:?}"
    );
}

#[test]
fn search_detail_cancelled_press_does_not_activate_later() {
    let mut search = new_search(SearchConfig::default());
    let source = accept_with_policy(&mut search, mixed(), None, Some(ClickPolicy::NAVIGATION));
    let mut focus = Focus::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut Seen::default());
    search
        .pointer(
            &frame.event(Phase::Press, k(31), Part::CELL, 1),
            &source,
            &mut focus,
        )
        .expect("source")
        .quiet();
    search
        .pointer(
            &frame.event(Phase::Release, k(31), Part::CELL, 1),
            &source,
            &mut focus,
        )
        .expect("source")
        .quiet();
    let out = acts(
        search
            .pointer(
                &frame.event(Phase::Click, k(31), Part::CELL, 1),
                &source,
                &mut focus,
            )
            .expect("source"),
    );
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn search_detail_navigation_paints_the_session_selection() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    assert_eq!(search.selected_key(), Some(k(30)));
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Right));
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    assert!(frame.text().contains("[two]"), "{}", frame.text());
    assert!(seen.selected_cells.contains(&(2, Some(1))));
}

#[test]
fn search_detail_same_key_preserves_preview_and_new_key_resets_it() {
    let lines = frame_lines(40);
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &lines, &mut seen);

    search.open_detail(&mut focus);
    let frame = draw(
        &mut search,
        &source,
        WIDE,
        focus.current(),
        &lines,
        &mut seen,
    );
    assert_eq!(scrolled_top(&frame).as_deref(), Some("preview line 00"));
    for _ in 0..5 {
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    }
    let frame = draw(
        &mut search,
        &source,
        WIDE,
        focus.current(),
        &lines,
        &mut seen,
    );
    assert_eq!(scrolled_top(&frame).as_deref(), Some("preview line 05"));
    assert_eq!(
        search.selected_key(),
        Some(k(10)),
        "the preview keys never move results"
    );

    let req = one_request(search.refresh_source(input(2)));
    let next = source_for(&req, mixed());
    let reply = search.submit(req.token, &next).expect("accepted");
    assert!(acts(reply).is_empty(), "the same key stays selected");
    let frame = draw(&mut search, &next, WIDE, focus.current(), &lines, &mut seen);
    assert_eq!(
        scrolled_top(&frame).as_deref(),
        Some("preview line 05"),
        "a payload refresh under the same key keeps the scroll"
    );

    focus.focus(ID);
    on_key(&mut search, &next, &mut focus, &key(KeyCode::Down));
    let frame = draw(&mut search, &next, WIDE, Some(ID), &lines, &mut seen);
    assert_eq!(
        scrolled_top(&frame).as_deref(),
        Some("preview line 00"),
        "a new selected key returns the preview to its top"
    );
}

#[test]
fn search_detail_preview_reset_policy_follows_the_query() {
    let lines = frame_lines(40);
    for (reset, expected) in [(false, "preview line 05"), (true, "preview line 00")] {
        let mut search = new_search(SearchConfig::default().reset_preview_on_query(reset));
        let req = accept(&mut search, 1, mixed());
        let source = source_for(&req, mixed());
        let mut focus = Focus::default();
        focus.focus(ID);
        let mut seen = Seen::default();
        draw(&mut search, &source, WIDE, Some(ID), &lines, &mut seen);
        search.open_detail(&mut focus);
        draw(
            &mut search,
            &source,
            WIDE,
            focus.current(),
            &lines,
            &mut seen,
        );
        for _ in 0..5 {
            on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
        }
        let req = one_request(search.set_query("a"));
        let next = source_for(&req, mixed());
        search.submit(req.token, &next).expect("accepted").quiet();
        let frame = draw(&mut search, &next, WIDE, focus.current(), &lines, &mut seen);
        assert_eq!(
            scrolled_top(&frame).as_deref(),
            Some(expected),
            "reset={reset}"
        );
    }
}

#[test]
fn search_detail_context_anchors_to_the_painted_key() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    assert_eq!(search.selected_key(), Some(k(30)));

    let menu = chord(KeyCode::F(10), KeyModifiers::SHIFT);
    assert!(
        on_key(&mut search, &source, &mut focus, &menu).is_empty(),
        "no painted anchor yet: nothing is invented from a stale position"
    );
    assert!(acts(search.emit_pending_context()).is_empty());

    let req = one_request(search.refresh_source(input(2)));
    let reordered = source_for(
        &req,
        vec![
            R::Item(12, "charlie"),
            R::Cells(
                3,
                vec![(32, "three", true), (31, "two", true), (30, "one", true)],
            ),
            R::Item(10, "alpha"),
        ],
    );
    search
        .submit(req.token, &reordered)
        .expect("accepted")
        .quiet();
    let frame = draw(
        &mut search,
        &reordered,
        WIDE,
        Some(ID),
        &[],
        &mut Seen::default(),
    );
    let (id, rect) = frame.hit(Part::CELL, k(30));
    let out = acts(search.emit_pending_context());
    assert_eq!(out.len(), 1);
    let SearchAction::ContextRequested(request) = &out[0] else {
        panic!("{out:?}")
    };
    assert_eq!(request.target, k(30), "the original key, wherever it moved");
    assert_eq!(
        request.anchor,
        ContextAnchor::Control { id, fallback: rect }
    );

    let drawn = on_key(&mut search, &reordered, &mut focus, &menu);
    assert!(
        matches!(&drawn[..], [SearchAction::ContextRequested(r)] if r.target == k(30)),
        "{drawn:?}"
    );
}

#[test]
fn search_detail_context_intent_is_cancelled_when_its_key_is_deleted() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    on_key(
        &mut search,
        &source,
        &mut focus,
        &chord(KeyCode::F(10), KeyModifiers::SHIFT),
    );
    let req = one_request(search.refresh_source(input(2)));
    let fewer = source_for(&req, vec![R::Item(12, "charlie")]);
    search.submit(req.token, &fewer).expect("accepted").quiet();
    draw(
        &mut search,
        &fewer,
        WIDE,
        Some(ID),
        &[],
        &mut Seen::default(),
    );
    assert!(acts(search.emit_pending_context()).is_empty());
}

#[test]
fn search_detail_secondary_click_selects_then_requests_through_context_config() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut Seen::default());
    let (_, rect) = frame.hit(Part::CELL, k(32));
    let out = acts(
        search
            .pointer(
                &frame.event(Phase::Secondary, k(32), Part::CELL, 1),
                &source,
                &mut focus,
            )
            .expect("source"),
    );
    assert_eq!(
        out,
        [
            SearchAction::SelectionChanged(Some(k(32))),
            SearchAction::ContextRequested(junie_tui::ui::context::ContextRequest {
                target: k(32),
                anchor: ContextAnchor::Pointer(Position::new(rect.x + 1, rect.y)),
            }),
        ]
    );

    let mut still =
        new_search(SearchConfig::default().context(ContextConfig::DEFAULT.moves_cursor(false)));
    let req = accept(&mut still, 1, mixed());
    let source = source_for(&req, mixed());
    let frame = draw(&mut still, &source, WIDE, None, &[], &mut Seen::default());
    let out = acts(
        still
            .pointer(
                &frame.event(Phase::Secondary, k(32), Part::CELL, 1),
                &source,
                &mut focus,
            )
            .expect("source"),
    );
    assert!(
        matches!(&out[..], [SearchAction::ContextRequested(r)] if r.target == k(32)),
        "moves_cursor(false) leaves the selection: {out:?}"
    );
    assert_eq!(still.selected_key(), Some(k(10)));
}

#[test]
fn search_detail_is_a_context_source_for_generic_hosts() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut Seen::default());
    let ev = frame.event(Phase::Secondary, k(12), Part::ROW, 1);
    assert_eq!(search.context_target(&ev), Some(k(12)));
    let heading = k(1);
    let head_ev = PointerEvent::new(
        &frame.hits,
        Phase::Secondary,
        ID.part(junie_tui::core::id::PartRef::item(Part::ROW, heading)),
        Position::new(1, 3),
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(
        search.context_target(&head_ev),
        None,
        "a heading is not a target"
    );
    let request = ContextConfig::DEFAULT
        .on_secondary(&mut search, &ev)
        .expect("a request");
    assert_eq!(request.target, k(12));
    assert_eq!(search.selected_key(), Some(k(12)));
}

#[test]
fn search_detail_loading_paints_stale_and_cannot_act() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let pending = one_request(search.set_query("x"));
    assert_eq!(search.phase(), SearchPhase::Loading);
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    assert!(seen.stale, "the retained results paint dimmed");
    assert!(on_key(&mut search, &source, &mut focus, &key(KeyCode::Enter)).is_empty());
    let menu = chord(KeyCode::F(10), KeyModifiers::SHIFT);
    assert!(on_key(&mut search, &source, &mut focus, &menu).is_empty());
    assert!(acts(search.emit_pending_context()).is_empty());

    search
        .fail(pending.token, "down".into())
        .expect("current")
        .quiet();
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    assert!(seen.stale);
    assert!(on_key(&mut search, &source, &mut focus, &key(KeyCode::Enter)).is_empty());
}

#[test]
fn search_detail_unsubmitted_source_is_refused_before_any_change() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let mut other = source_for(&req, mixed());
    other.stamp.data += 1;
    assert!(matches!(
        search.on_key(&key(KeyCode::Down), &other, &mut focus),
        Err(SearchError::UnsubmittedProjection)
    ));
    assert_eq!(search.selected_key(), Some(k(10)));

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(WIDE);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let err = search
        .render_source_with(
            WIDE,
            &other,
            &detail_frame(&[]),
            &mut buf,
            &mut ctx,
            |_: RowPaint<'_, Results>, _: &mut Buffer, _: &mut RenderCtx| {},
            |_, _: QueryView<'_>, _: &mut Buffer, _: &mut RenderCtx| {},
        )
        .expect_err("not the accepted projection");
    assert_eq!(err, SearchError::UnsubmittedProjection);
}

#[test]
fn search_detail_empty_before_first_result_ignores_the_source() {
    let mut search = new_search(SearchConfig::default());
    let req = one_request(search.refresh_source(input(1)));
    let never = source_for(&req, vec![R::Item(1, "must not paint")]);
    let frame = draw(&mut search, &never, WIDE, None, &[], &mut Seen::default());
    assert!(!frame.text().contains("must not paint"));
    assert_eq!(search.phase(), SearchPhase::Loading);
}

#[test]
fn search_detail_combined_query_hook_and_field_hit() {
    let config = SearchConfig::default().focus_mode(SearchFocusMode::Combined);
    let mut search = new_search(config);
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, WIDE, None, &[], &mut seen);
    assert_eq!(seen.query, Some((String::new(), false, None, 0)));

    let click = search
        .pointer(&frame.field_event(Phase::Click), &source, &mut focus)
        .expect("source");
    assert!(click.is_consumed());
    assert_eq!(focus.current(), Some(ID), "one owner for query and results");

    let mut requested = 0;
    for c in "ls".chars() {
        let out = on_key(&mut search, &source, &mut focus, &key(KeyCode::Char(c)));
        requested += out
            .iter()
            .filter(|a| matches!(a, SearchAction::QueryRequested(_)))
            .count();
    }
    assert_eq!(requested, 2);
    assert_eq!(search.query(), "ls");
    on_key(&mut search, &source, &mut focus, &ctrl('a'));
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &[], &mut seen);
    assert_eq!(seen.query, Some(("ls".into(), true, Some(0..2), 2)));
    let hits_before = on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    assert!(hits_before.is_empty(), "pending results stay inert");
}

#[test]
fn search_detail_combined_query_click_precedes_hooks_own_hits() {
    let config = SearchConfig::default().focus_mode(SearchFocusMode::Combined);
    let mut search = new_search(config);
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(WIDE);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let scope = WidgetId::of("sd.scope");
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        search
            .render_source_with(
                WIDE,
                &source,
                &detail_frame(&[]),
                &mut buf,
                &mut ctx,
                |_: RowPaint<'_, Results>, _: &mut Buffer, _: &mut RenderCtx| {},
                |area, _: QueryView<'_>, _: &mut Buffer, ctx: &mut RenderCtx| {
                    ctx.clickable(scope, Rect::new(area.right() - 8, area.y, 8, 1));
                },
            )
            .expect("source");
    }
    let hit = hits
        .resolve(Position::new(WIDE.width - 3, 0))
        .expect("a hit");
    assert_eq!(
        hit.owner, scope,
        "the hook's own region wins inside its rect"
    );
    let hit = hits.resolve(Position::new(3, 0)).expect("a hit");
    assert_eq!(hit.owner, ID);
    assert_eq!(hit.part.part, Part::FIELD);
}

#[test]
fn search_detail_separate_query_edits_request_and_results_keys_navigate() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(QUERY);
    let out = on_key(&mut search, &source, &mut focus, &key(KeyCode::Char('z')));
    assert!(matches!(&out[..], [SearchAction::QueryRequested(r)] if r.text == "z"));
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Tab));
    assert_eq!(focus.current(), Some(ID), "Tab moves to the results");
    on_key(&mut search, &source, &mut focus, &ctrl('f'));
    assert_eq!(focus.current(), Some(QUERY), "Ctrl-F returns to the query");
}

#[test]
fn search_detail_tab_on_the_results_is_left_to_the_shell() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let reply = search
        .on_key(&key(KeyCode::Tab), &source, &mut focus)
        .expect("the accepted source");
    assert!(
        !reply.is_consumed(),
        "Tab on the results must reach the shell's focus ring, not re-focus the results"
    );
    assert_eq!(focus.current(), Some(ID));
}

#[test]
fn search_detail_narrow_drawer_restores_focus() {
    let lines = frame_lines(40);
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let mut seen = Seen::default();
    let frame = draw(&mut search, &source, NARROW, Some(ID), &lines, &mut seen);
    assert!(frame.text().contains("alpha"), "master shows when narrow");

    on_key(&mut search, &source, &mut focus, &ctrl('o'));
    assert!(search.on_detail(focus.current()), "Ctrl-O opens the drawer");
    let frame = draw(
        &mut search,
        &source,
        NARROW,
        focus.current(),
        &lines,
        &mut seen,
    );
    assert!(frame.text().contains("preview line 00"), "{}", frame.text());
    assert!(
        !frame.text().contains("alpha"),
        "the drawer covers the list"
    );
    for _ in 0..3 {
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    }

    on_key(&mut search, &source, &mut focus, &key(KeyCode::Esc));
    assert_eq!(
        focus.current(),
        Some(ID),
        "Esc returns focus to the results"
    );
    let frame = draw(
        &mut search,
        &source,
        NARROW,
        focus.current(),
        &lines,
        &mut seen,
    );
    assert!(frame.text().contains("alpha"));
    assert_eq!(
        search.selected_key(),
        Some(k(10)),
        "selection untouched by the drawer"
    );

    on_key(&mut search, &source, &mut focus, &ctrl('o'));
    let frame = draw(
        &mut search,
        &source,
        NARROW,
        focus.current(),
        &lines,
        &mut seen,
    );
    assert_eq!(
        scrolled_top(&frame).as_deref(),
        Some("preview line 03"),
        "the preview scroll survives closing and reopening"
    );
}

#[test]
fn search_detail_typing_returns_from_the_preview_only_when_configured() {
    for (on, expected) in [(false, ""), (true, "x")] {
        let config = SearchConfig::default()
            .focus_mode(SearchFocusMode::Combined)
            .type_to_query_from_detail(on);
        let mut search = new_search(config);
        let req = accept(&mut search, 1, mixed());
        let source = source_for(&req, mixed());
        let mut focus = Focus::default();
        focus.focus(ID);
        draw(
            &mut search,
            &source,
            WIDE,
            Some(ID),
            &frame_lines(30),
            &mut Seen::default(),
        );
        search.open_detail(&mut focus);
        draw(
            &mut search,
            &source,
            WIDE,
            focus.current(),
            &frame_lines(30),
            &mut Seen::default(),
        );
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Char('j')));
        assert_eq!(
            search.query(),
            "",
            "j scrolls the preview, it is not typing"
        );
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Char('x')));
        assert_eq!(search.query(), expected, "type_to_query_from_detail={on}");
        assert_eq!(focus.current() == Some(ID), on);
        let pasted = acts(search.on_paste("p", &mut focus));
        assert_eq!(!pasted.is_empty(), on, "paste follows the same switch");
    }
}

#[test]
fn search_detail_standard_paint_draws_rows_and_registers_hits() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, list());
    let source = source_for(&req, list());
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(WIDE);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        search
            .render_source(WIDE, &source, &detail_frame(&[]), &mut buf, &mut ctx)
            .expect("source");
    }
    let text = (0..WIDE.height).map(|y| row(&buf, y)).collect::<String>();
    assert!(text.contains('a') && text.contains("More"));
    assert!(
        hits.regions()
            .iter()
            .any(|r| r.owner == ID && r.part.item == Some(k(13)))
    );
    assert!(source.source_len() > 0);
    let _: SourceStamp = source.stamp();
}

#[derive(Default)]
struct Hosted {
    lead: bool,
    rows: Vec<(usize, Rect, bool)>,
    cell_hits: Vec<(usize, WidgetId)>,
    begin: Option<(Rect, u16)>,
    empty: Option<Rect>,
    detail: Option<(Rect, bool, usize)>,
    summary: Option<Rect>,
}

impl SearchPaint<Results> for Hosted {
    fn query(&mut self, area: Rect, view: QueryView<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        paint_query_default(area, view, buf, ctx);
    }

    fn rows_begin(&mut self, area: Rect, row_w: u16) {
        self.begin = Some((area, row_w));
    }

    fn row(&mut self, mut row: RowPaint<'_, Results>, buf: &mut Buffer, _ctx: &mut RenderCtx) {
        use junie_tui::core::list::RowSource;
        use junie_tui::widgets::search_detail::SearchSource;
        self.rows.push((row.row, row.rect, row.selected));
        buf.set_string(
            row.rect.x,
            row.rect.y,
            row.source.label(row.row),
            ratatui::style::Style::new(),
        );
        if row.kind == ResultRowKind::Cells {
            let mut x = row.rect.x + 2;
            for cell in 0..row.source.cell_count(row.row) {
                row.place_cell(cell, Rect::new(x, row.rect.y, 4, 1));
                self.cell_hits.push((cell, row.cell_hit(cell)));
                x += 5;
            }
        }
    }

    fn empty(&mut self, area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.empty = Some(area);
    }

    fn detail(&mut self, pane: DetailPane<'_>, _buf: &mut Buffer, ctx: &mut RenderCtx) {
        ctx.control(pane.id, pane.area, false);
        ctx.scrollable(pane.id, pane.area);
        pane.scroll.set_content(60);
        pane.scroll.set_viewport(pane.area.height as usize);
        self.detail = Some((pane.area, pane.drawer, pane.scroll.offset));
    }

    fn summary(&mut self, area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
        self.summary = Some(area);
    }

    fn is_lead(&self, source: &Results, row: usize) -> bool {
        use junie_tui::widgets::search_detail::SearchSource;
        self.lead && source.row_kind(row) == ResultRowKind::Heading
    }
}

fn hosted_search() -> SearchDetail {
    new_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined))
        .detail_share(34, 34, 48)
        .summary_rows(1)
        .collapse_below(108)
}

fn draw_hosted(
    search: &mut SearchDetail,
    source: &Results,
    area: Rect,
    focus: Option<WidgetId>,
    host: &mut Hosted,
) -> Frame {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    *host = Hosted {
        lead: host.lead,
        ..Hosted::default()
    };
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        search
            .render_hosted(area, source, &mut buf, &mut ctx, host)
            .expect("the accepted source");
    }
    Frame { buf, hits }
}

#[test]
fn search_detail_hosted_gives_every_pane_to_the_host_and_keeps_the_layout() {
    let mut search = hosted_search();
    let mut host = Hosted::default();
    let none = Results::new(&one_request(search.refresh_source(input(1))), Vec::new());
    draw_hosted(&mut search, &none, WIDE, Some(ID), &mut host);
    assert!(
        host.empty.is_some() && host.rows.is_empty(),
        "no accepted results is an empty list, and the source is not read"
    );

    let req = accept(&mut search, 2, mixed());
    let source = source_for(&req, mixed());
    let frame = draw_hosted(&mut search, &source, WIDE, Some(ID), &mut host);
    let (area, row_w) = host.begin.expect("rows begin");
    assert_eq!(
        (area.x, area.y),
        (0, 2),
        "the query line and its gap come first, and the list has no frame"
    );
    assert_eq!(row_w, area.width, "a short list has no scrollbar column");
    let (first, rect, _) = host.rows[0];
    assert_eq!((first, rect.x, rect.y, rect.width), (0, 0, 2, row_w));
    let (detail, drawer, _) = host.detail.expect("the preview paints beside the list");
    assert!(!drawer && detail.x >= area.right(), "{detail:?} {area:?}");
    assert_eq!(detail.width, 40, "{detail:?}");
    assert!(host.summary.is_none());
    frame.hit(Part::ROW, k(10));

    let narrow = draw_hosted(&mut search, &source, NARROW, Some(ID), &mut host);
    assert!(host.detail.is_none(), "below the collapse width");
    let summary = host.summary.expect("the summary row");
    assert_eq!(summary.height, 1);
    drop(narrow);
}

#[test]
fn search_detail_hosted_cells_register_under_their_stable_keys() {
    let mut search = hosted_search();
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut host = Hosted::default();
    let frame = draw_hosted(&mut search, &source, WIDE, Some(ID), &mut host);
    for (cell, key) in [(0, 30), (1, 31), (2, 32)] {
        let (id, _) = frame.hit(Part::CELL, k(key));
        assert!(
            host.cell_hits.contains(&(cell, id)),
            "cell_hit names the region registered for cell {cell}"
        );
    }
}

#[test]
fn search_detail_hosted_wheel_scrolls_the_list_and_the_preview() {
    let mut search = hosted_search();
    let rows: Vec<R> = (0..30).map(|i| R::Item(100 + i, "row")).collect();
    let req = accept(&mut search, 1, rows.clone());
    let source = source_for(&req, rows);
    let mut host = Hosted::default();
    let mut focus = Focus::default();
    let frame = draw_hosted(
        &mut search,
        &source,
        Rect::new(0, 0, 120, 12),
        Some(ID),
        &mut host,
    );
    assert_eq!(host.rows[0].0, 0);
    let wheel = |id: WidgetId, delta: i32| {
        PointerEvent::wheel_on(
            &frame.hits,
            id,
            junie_tui::core::hit::Axis::Vertical,
            delta,
            Position::new(2, 4),
            KeyModifiers::NONE,
        )
    };
    search
        .pointer(&wheel(ID, 3), &source, &mut focus)
        .expect("source")
        .quiet();
    let _ = draw_hosted(
        &mut search,
        &source,
        Rect::new(0, 0, 120, 12),
        Some(ID),
        &mut host,
    );
    assert_eq!(host.rows[0].0, 3, "the list scrolled three rows");
    search
        .pointer(&wheel(search.detail_id(), 4), &source, &mut focus)
        .expect("source")
        .quiet();
    let _ = draw_hosted(
        &mut search,
        &source,
        Rect::new(0, 0, 120, 12),
        Some(ID),
        &mut host,
    );
    assert_eq!(host.detail.expect("preview").2, 4, "the preview scrolled");
}

#[test]
fn search_detail_hosted_heading_stays_above_an_item_at_the_top_edge() {
    let rows = || {
        let mut rows = vec![R::Head(1, "H1")];
        rows.extend((0..5).map(|i| R::Item(10 + i, "a")));
        rows.push(R::Head(2, "H2"));
        rows.extend((0..5).map(|i| R::Item(20 + i, "b")));
        rows
    };
    let small = Rect::new(0, 0, 120, 4);
    for (lead, expected_top) in [(true, 6), (false, 7)] {
        let mut search = hosted_search();
        let req = accept(&mut search, 1, rows());
        let source = source_for(&req, rows());
        let mut host = Hosted {
            lead,
            ..Hosted::default()
        };
        let mut focus = Focus::default();
        focus.focus(ID);
        draw_hosted(&mut search, &source, small, Some(ID), &mut host);
        on_key(&mut search, &source, &mut focus, &key(KeyCode::End));
        draw_hosted(&mut search, &source, small, Some(ID), &mut host);
        for _ in 0..4 {
            on_key(&mut search, &source, &mut focus, &key(KeyCode::Up));
            draw_hosted(&mut search, &source, small, Some(ID), &mut host);
        }
        assert_eq!(search.selected_key(), Some(k(20)));
        assert_eq!(
            host.rows[0].0, expected_top,
            "lead {lead}: the top row of the list"
        );
    }
}

#[test]
fn review283_hosted_wheel_over_a_row_scrolls_the_list() {
    let mut search = hosted_search();
    let rows: Vec<R> = (0..30).map(|i| R::Item(100 + i, "row")).collect();
    let req = accept(&mut search, 1, rows.clone());
    let source = source_for(&req, rows);
    let mut host = Hosted::default();
    let mut focus = Focus::default();
    let area = Rect::new(0, 0, 120, 12);
    let frame = draw_hosted(&mut search, &source, area, Some(ID), &mut host);
    let (row_id, rect) = frame.hit(Part::ROW, k(100));
    let ev = PointerEvent::wheel_on(
        &frame.hits,
        row_id,
        junie_tui::core::hit::Axis::Vertical,
        3,
        Position::new(rect.x + 1, rect.y),
        KeyModifiers::NONE,
    );
    search
        .pointer(&ev, &source, &mut focus)
        .expect("source")
        .quiet();
    let _ = draw_hosted(&mut search, &source, area, Some(ID), &mut host);
    assert_eq!(host.rows[0].0, 3, "a wheel over a row scrolls the list");
}

#[test]
fn review283_same_key_selection_paths_keep_the_preview_scroll() {
    let lines = frame_lines(40);
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let mut focus = Focus::default();
    focus.focus(ID);
    let mut seen = Seen::default();
    draw(&mut search, &source, WIDE, Some(ID), &lines, &mut seen);
    search.open_detail(&mut focus);
    draw(
        &mut search,
        &source,
        WIDE,
        focus.current(),
        &lines,
        &mut seen,
    );
    for _ in 0..5 {
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    }
    focus.focus(ID);
    let top = |search: &mut SearchDetail, focus: &Focus, seen: &mut Seen| {
        let f = draw(search, &source, WIDE, focus.current(), &lines, seen);
        scrolled_top(&f)
    };
    assert_eq!(
        top(&mut search, &focus, &mut seen).as_deref(),
        Some("preview line 05")
    );
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Home));
    assert_eq!(search.selected_key(), Some(k(10)));
    assert_eq!(
        top(&mut search, &focus, &mut seen).as_deref(),
        Some("preview line 05"),
        "Home on the selected row"
    );
    let frame = draw(&mut search, &source, WIDE, Some(ID), &lines, &mut seen);
    for (phase, count) in [(Phase::Press, 1), (Phase::Click, 1), (Phase::Secondary, 1)] {
        let _ = search.pointer(
            &frame.event(phase, k(10), Part::ROW, count),
            &source,
            &mut focus,
        );
        assert_eq!(
            top(&mut search, &focus, &mut seen).as_deref(),
            Some("preview line 05"),
            "{phase:?} on the selected row"
        );
    }
    // Explore cell re-click on the selected cell.
    on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    let frame = draw(&mut search, &source, WIDE, Some(ID), &lines, &mut seen);
    assert_eq!(search.selected_key(), Some(k(30)));
    assert_eq!(scrolled_top(&frame).as_deref(), Some("preview line 00"));
}

#[test]
fn search_detail_standard_paint_places_selectable_cells() {
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(WIDE);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        search
            .render_source(WIDE, &source, &detail_frame(&[]), &mut buf, &mut ctx)
            .expect("source");
    }
    let text = (0..WIDE.height)
        .map(|y| row(&buf, y).trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    for label in ["one", "two", "three"] {
        assert!(text.contains(label), "chip {label} is painted\n{text}");
    }
    for cell in [30, 31, 32] {
        assert!(
            hits.regions().iter().any(|r| r.owner == ID
                && r.part.part == Part::CELL
                && r.part.item == Some(k(cell))),
            "cell {cell} registers a hit region under its own key"
        );
    }
}

#[test]
fn search_detail_chip_that_does_not_fit_is_not_selectable_or_runnable() {
    const TIGHT: Rect = Rect::new(0, 0, 24, 14);
    let mut search = new_search(SearchConfig::default());
    let req = accept(&mut search, 1, mixed());
    let source = source_for(&req, mixed());
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(TIGHT);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        search
            .render_source(TIGHT, &source, &detail_frame(&[]), &mut buf, &mut ctx)
            .expect("source");
    }
    let painted: Vec<ItemKey> = [30, 31, 32]
        .into_iter()
        .map(k)
        .filter(|&cell| {
            hits.regions()
                .iter()
                .any(|r| r.owner == ID && r.part.part == Part::CELL && r.part.item == Some(cell))
        })
        .collect();
    assert!(
        !painted.is_empty() && painted.len() < 3,
        "premise: at this width some chips fit and some do not, painted {painted:?}"
    );
    let mut focus = Focus::default();
    focus.focus(ID);
    for _ in 0..3 {
        if search
            .selected_key()
            .is_some_and(|sel| [k(30), k(31), k(32)].contains(&sel))
        {
            break;
        }
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Down));
    }
    assert!(
        search
            .selected_key()
            .is_some_and(|sel| [k(30), k(31), k(32)].contains(&sel)),
        "premise: the cursor is on the chip row, not {:?}",
        search.selected_key()
    );
    let mut reached = Vec::new();
    let mut ran = Vec::new();
    for _ in 0..4 {
        on_key(&mut search, &source, &mut focus, &key(KeyCode::Right));
        reached.extend(search.selected_key());
        ran.extend(
            on_key(&mut search, &source, &mut focus, &key(KeyCode::Enter))
                .into_iter()
                .filter_map(|action| match action {
                    SearchAction::Activate(item) => Some(item),
                    _ => None,
                }),
        );
    }
    let hidden =
        |item: &&ItemKey| [k(30), k(31), k(32)].contains(*item) && !painted.contains(*item);
    assert_eq!(
        reached.iter().filter(hidden).count(),
        0,
        "Right landed on a chip that is not drawn: {reached:?}"
    );
    assert_eq!(
        ran.iter().filter(hidden).count(),
        0,
        "Enter ran a chip that is not drawn: {ran:?}"
    );
    assert!(
        ran.iter().any(|item| painted.contains(item)),
        "the chips that are drawn still run: {ran:?}"
    );
}
