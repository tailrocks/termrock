//! Deterministic work budgets for the shared widgets (#56).
//!
//! Each test builds a fixture at 1,000 / 10,000 / 100,000 seeded rows or lines
//! in a fixed 100x30 viewport, warms it, performs exactly one operation and
//! reads the work counters of that operation. Budgets are counts, never times.
//! Every number below was measured with
//!
//! ```text
//! cargo nextest run -p junie-tui --features work-metrics --test regressions \
//!     --no-tests fail -E 'test(/^work_budgets::/)'
//! ```
//!
//! A budget that is not zero says so next to its constant. An unmet target of
//! another issue is a ceiling on today's behaviour, not a claim that the work
//! is bounded.

#![cfg(feature = "work-metrics")]

use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::scroll_view::ScrollView;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::table::{Cell, Column, DataTable};
use junie_tui::widgets::tree::{TreeNode, TreeView};
use junie_tui::widgets::viewport::{Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect, Size};

const VIEWPORT: Rect = Rect::new(0, 0, 100, 30);
const SIZES: [usize; 3] = [1_000, 10_000, 100_000];
const ID: WidgetId = WidgetId::of("work.budget");

fn frame<T>(paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> T) -> (Buffer, T) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(VIEWPORT);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let out = {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        paint(&mut buf, &mut ctx)
    };
    (buf, out)
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn assert_budget(delta: &WorkDelta, metric: Metric, budget: u64, what: &str) {
    let got = delta.get(metric);
    assert!(
        got <= budget,
        "{what}: {} was {got}, budget {budget}; whole delta {:?}",
        metric.name(),
        delta.nonzero().collect::<Vec<_>>()
    );
}

fn assert_no_work(delta: &WorkDelta, what: &str) {
    assert_eq!(
        *delta,
        WorkDelta::ZERO,
        "{what}: expected no counted work, got {:?}",
        delta.nonzero().collect::<Vec<_>>()
    );
}

fn publish(
    scenario: &'static str,
    operation: &'static str,
    n: usize,
    work: WorkDelta,
    oracle: &str,
) {
    let report = ScenarioReport {
        scenario,
        operation,
        record_count: n,
        viewport: (VIEWPORT.width, VIEWPORT.height),
        work,
        correctness_oracle: oracle.to_owned(),
    };
    report.write().expect("write the count report");
}

fn history(n: usize) -> Vec<Line> {
    (0..n).map(|i| vec![Span::plain(line_text(i))]).collect()
}

fn line_text(i: usize) -> String {
    format!("line {i:06} of the seeded history")
}

fn assert_shows(buf: &Buffer, y: u16, text: &str) {
    let row = row_text(buf, y);
    assert!(row.starts_with(text), "row {y} is {row:?}, want {text:?}");
}

fn history_bytes(n: usize) -> u64 {
    history(n)
        .iter()
        .flatten()
        .map(|span| span.text.len() as u64)
        .sum()
}

fn prepared_viewport(n: usize) -> TextViewport {
    let mut v = TextViewport::with_lines(ID, history(n));
    v.set_area(VIEWPORT);
    v
}

fn draw_viewport(v: &TextViewport, area: Rect) -> Buffer {
    frame(|buf, ctx| v.draw(area, buf, ctx, ctx.theme.canvas, true)).0
}

#[test]
fn work_contract_viewport_first_use_and_prepared_draw() {
    for n in SIZES {
        let mut v = TextViewport::with_lines(ID, history(n));
        let before = v.work_counters();
        let ((), first) = measure(|| v.set_area(VIEWPORT));
        let local = v.work_counters();

        // Measured: n segmented lines, 2n reflowed lines (two layout passes)
        // and 2 index rebuilds on the first set_area of a fresh view.
        assert_eq!(first.get(Metric::SegmentedLines), n as u64, "n={n}");
        assert_budget(&first, Metric::ReflowedLines, 2 * n as u64, "first use");
        assert_budget(&first, Metric::IndexRebuilds, 2, "first use");
        assert_eq!(
            first.get(Metric::SegmentedLines),
            (local.segmented_lines - before.segmented_lines) as u64,
            "the common counters equal the view's own, so nothing is counted twice"
        );
        assert_eq!(
            first.get(Metric::ReflowedLines),
            (local.reflowed_lines - before.reflowed_lines) as u64
        );
        assert_eq!(
            first.get(Metric::IndexRebuilds),
            (local.index_rebuilds - before.index_rebuilds) as u64
        );

        let (buf, prepared) = measure(|| draw_viewport(&v, VIEWPORT));
        assert_no_work(&prepared, &format!("prepared draw of {n} lines"));
        assert_shows(&buf, 0, &line_text(n - 30));
        assert_shows(&buf, 29, &line_text(n - 1));

        let ((), equal) = measure(|| v.set_area(VIEWPORT));
        assert_no_work(&equal, &format!("equal set_area on {n} lines"));

        publish(
            "viewport_prepared",
            "prepared_draw",
            n,
            prepared,
            "the frame shows the last 30 lines, following the tail",
        );
    }
}

#[test]
fn work_contract_viewport_one_append_is_incremental() {
    for n in SIZES {
        let mut v = prepared_viewport(n);
        let ((), push) = measure(|| v.push(vec![Span::plain("appended line")]));
        assert_no_work(&push, "push only queues the line");

        let ((), prepare) = measure(|| v.set_area(VIEWPORT));
        // Measured: one new line is segmented, reflowed and appended to the
        // index; the width helper sees only its 13 bytes. All independent of n.
        assert_eq!(prepare.get(Metric::SegmentedLines), 1, "n={n}");
        assert_eq!(prepare.get(Metric::ReflowedLines), 1);
        assert_eq!(prepare.get(Metric::IndexExtends), 1);
        assert_eq!(prepare.get(Metric::IndexRebuilds), 0);
        assert_eq!(prepare.get(Metric::TextBytesMeasured), 13);
        assert_eq!(prepare.get(Metric::PayloadBytesCloned), 0);

        let (buf, draw) = measure(|| draw_viewport(&v, VIEWPORT));
        assert_no_work(&draw, "the draw after re-preparing");
        assert_shows(&buf, 0, &line_text(n - 29));
        assert_shows(&buf, 29, "appended line");

        publish(
            "viewport_prepared",
            "append_one_line",
            n,
            prepare,
            "the appended line is the last frame row",
        );
    }
}

#[test]
fn work_contract_viewport_adapter_unprepared_draw() {
    for n in SIZES {
        let mut v = prepared_viewport(n);
        v.push(vec![Span::plain("appended line")]);
        let total = history_bytes(n) + "appended line".len() as u64;

        // Drawing through the shared reference after a push, without a
        // set_area, deep-clones the whole history. This is the cost #74
        // intends hosts to avoid by preparing first; the ceiling is the
        // exact payload size, so the clone cannot grow unnoticed.
        let before = v.work_counters();
        let (buf, draw) = measure(|| draw_viewport(&v, VIEWPORT));
        let after = v.work_counters();
        assert_eq!(draw.get(Metric::PayloadBytesCloned), total, "n={n}");
        assert_eq!(
            draw.get(Metric::PayloadBytesCloned),
            (after.draw_cloned_bytes - before.draw_cloned_bytes) as u64,
            "the common counter equals the view's own draw-clone counter"
        );
        assert_eq!(after.draw_fallbacks - before.draw_fallbacks, 1);
        assert_shows(&buf, 0, &line_text(n - 29));
        assert_shows(&buf, 29, "appended line");

        publish(
            "viewport_adapter",
            "unprepared_draw_after_push",
            n,
            draw,
            "the fallback draw shows the appended line as the last row",
        );
    }
}

#[test]
fn work_contract_scroll_view_content_sized_canvas() {
    // Known gap, issue #20: ScrollView::render still allocates and fills a
    // canvas as large as the content. These are the measured counts of today's
    // behaviour with the vertical extent below u16::MAX; #20 must lower the
    // allocation and fill ceilings to the visible window, and this test then
    // gets the lower numbers. Copies back to the screen are already
    // window-bound: they do not grow with the content.
    let mut copied_at_smallest = None;
    for rows in [1_000u16, 10_000, 60_000] {
        let mut sv = ScrollView::new(ID);
        let content = Size::new(100, rows);
        let (buf, delta) = measure(|| {
            frame(|buf, ctx| {
                sv.render(VIEWPORT, content, buf, ctx, |area, buf, _| {
                    buf.set_string(
                        area.x,
                        area.y,
                        "first content row",
                        ratatui::style::Style::new(),
                    );
                });
            })
            .0
        });
        let cells = 100 * u64::from(rows);
        assert_eq!(
            delta.get(Metric::AllocatedBufferCells),
            cells,
            "rows={rows}"
        );
        assert_eq!(delta.get(Metric::BufferCellsFilled), cells);
        assert_budget(&delta, Metric::BufferCellsCopied, 5_842, "screen copies");
        let copied = *copied_at_smallest.get_or_insert(delta.get(Metric::BufferCellsCopied));
        assert_eq!(
            delta.get(Metric::BufferCellsCopied),
            copied,
            "copies do not depend on content height"
        );
        assert!(row_text(&buf, 0).starts_with("first content row"));

        publish(
            "scroll_view",
            "render_overflowing_content",
            rows as usize,
            delta,
            "the first screen row shows the first content row",
        );
    }
}

#[test]
fn work_contract_settled_frame_paints_only_visible_rows() {
    let mut list_first = None;
    let mut table_first = None;
    let mut tree_first = None;
    for n in SIZES {
        let items = (0..n)
            .map(|i| ListItem::new(&format!("item {i:06}")))
            .collect();
        let mut list = ListBox::new(ID, items, SelectMode::Single);
        let (warm, _) = frame(|buf, ctx| list.render(VIEWPORT, buf, ctx, ctx.theme.canvas));
        let (buf, list_work) =
            measure(|| frame(|b, c| list.render(VIEWPORT, b, c, c.theme.canvas)).0);
        assert_eq!(warm, buf, "a settled list repaints the same cells");
        assert!(row_text(&buf, 0).contains("item 000000"));
        // Measured: 30 rows visited and no width call for the list.
        assert_eq!(list_work.get(Metric::VisibleRowsPainted), 30, "list n={n}");
        assert_budget(&list_work, Metric::IntrinsicWidthCalls, 0, "list");
        assert_eq!(
            *list_first.get_or_insert(list_work),
            list_work,
            "list n={n}"
        );

        let rows = (0..n)
            .map(|i| vec![Cell::new(format!("r{i}")), Cell::new("x")])
            .collect();
        let columns = vec![
            Column::new("a", Constraint::Length(20)),
            Column::new("b", Constraint::Length(20)),
        ];
        let mut table = DataTable::new(ID, columns, rows);
        frame(|b, c| table.render(VIEWPORT, b, c, c.theme.canvas));
        let (buf, table_work) =
            measure(|| frame(|b, c| table.render(VIEWPORT, b, c, c.theme.canvas)).0);
        assert!(row_text(&buf, 1).contains("r0"));
        // Measured: 29 body rows, 62 width calls over 108 bytes.
        assert_eq!(
            table_work.get(Metric::VisibleRowsPainted),
            29,
            "table n={n}"
        );
        assert_budget(&table_work, Metric::IntrinsicWidthCalls, 62, "table");
        assert_budget(&table_work, Metric::TextBytesMeasured, 108, "table");
        assert_eq!(
            *table_first.get_or_insert(table_work),
            table_work,
            "table n={n}"
        );

        let nodes = (0..n)
            .map(|i| TreeNode::leaf(&format!("Node {i}")))
            .collect();
        let mut tree = TreeView::new(ID, nodes);
        frame(|b, c| tree.render(VIEWPORT, b, c, c.theme.canvas));
        let (buf, tree_work) =
            measure(|| frame(|b, c| tree.render(VIEWPORT, b, c, c.theme.canvas)).0);
        assert!(row_text(&buf, 0).contains("Node 0"));
        // Measured: 30 rows, 30 width calls over 200 bytes, no normalization.
        assert_eq!(tree_work.get(Metric::VisibleRowsPainted), 30, "tree n={n}");
        assert_budget(&tree_work, Metric::IntrinsicWidthCalls, 30, "tree");
        assert_budget(&tree_work, Metric::TextBytesMeasured, 200, "tree");
        assert_eq!(tree_work.get(Metric::LabelsNormalized), 0);
        assert_eq!(
            *tree_first.get_or_insert(tree_work),
            tree_work,
            "tree n={n}"
        );

        publish(
            "list_box",
            "settled_frame",
            n,
            list_work,
            "the frame is cell-equal to the warm frame and row 0 shows item 0",
        );
        publish(
            "data_table",
            "settled_frame",
            n,
            table_work,
            "body row 1 shows the first record",
        );
        publish(
            "tree_view",
            "settled_frame",
            n,
            tree_work,
            "row 0 shows the first node",
        );
    }
}

#[test]
fn work_contract_tree_filtered_rebuild() {
    for n in SIZES {
        let labels: Vec<String> = (0..n).map(|i| format!("Node {i}")).collect();
        let nodes = labels.iter().map(|l| TreeNode::leaf(l)).collect();
        let mut tree = TreeView::new(ID, nodes);
        let ((), filter) = measure(|| tree.set_filter(Some("node 5")));

        // One pass over the source: each label is lowercased exactly once and
        // the bytes counted are the lowercased labels' bytes. A filtered
        // rebuild is O(N) by design (#55), so this is an equality, not zero.
        let bytes: u64 = labels.iter().map(|l| l.to_lowercase().len() as u64).sum();
        assert_eq!(filter.get(Metric::LabelsNormalized), n as u64, "n={n}");
        assert_eq!(filter.get(Metric::LabelBytesNormalized), bytes);
        let expected = labels
            .iter()
            .filter(|l| l.to_lowercase().contains("node 5"))
            .count();
        assert_eq!(tree.rows().len(), expected, "the filter kept the matches");

        let (_, settled) = measure(|| frame(|b, c| tree.render(VIEWPORT, b, c, c.theme.canvas)));
        assert_eq!(
            settled.get(Metric::LabelsNormalized),
            0,
            "rendering does not refilter"
        );

        publish(
            "tree_view",
            "filter_change",
            n,
            filter,
            "the visible row count equals the labels containing the query",
        );
    }
}

#[test]
fn work_report_is_count_only_json_with_every_metric() {
    let work = measure(|| junie_tui::record_work!(SegmentedLines, 3usize)).1;
    let path = ScenarioReport {
        scenario: "report_shape",
        operation: "one_count",
        record_count: 3,
        viewport: (100, 30),
        work,
        correctness_oracle: "quoted \"text\" with a \\ and a newline\n".to_owned(),
    }
    .write()
    .expect("write");
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("valid JSON");

    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["instrumentation_enabled"], true);
    assert_eq!(parsed["timing"], "not measured");
    assert_eq!(parsed["work_delta"]["segmented_lines"], 3);
    assert_eq!(
        parsed["work_delta"].as_object().map(|o| o.len()),
        Some(Metric::ALL.len()),
        "every metric is listed, zeros included"
    );
    assert_eq!(
        parsed["correctness_oracle"],
        "quoted \"text\" with a \\ and a newline\n"
    );
    for key in ["median_ns", "p95_ns", "samples"] {
        assert!(parsed.get(key).is_none(), "{key} would be a timing claim");
    }
}
