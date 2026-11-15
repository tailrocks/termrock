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

use std::ops::Range;

use junie_tui::core::event::Key;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::metrics::harness::assert_exact;
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::theme::SyntaxTone;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::scroll_view::ScrollView;
use junie_tui::widgets::code::CodeEditor;
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::table::{Cell, Column, DataTable};
use junie_tui::widgets::textarea::TextArea;
use junie_tui::widgets::tree::{TreeNode, TreeView};
use junie_tui::widgets::viewport::{Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Rect, Size};

const VIEWPORT: Rect = Rect::new(0, 0, 100, 30);
const SIZES: [usize; 3] = [1_000, 10_000, 100_000];
const ID: WidgetId = WidgetId::of("work.budget");

fn frame<T>(paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> T) -> (Buffer, T) {
    frame_with(Interaction::default(), paint)
}

fn frame_with<T>(
    interaction: Interaction,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> T,
) -> (Buffer, T) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(VIEWPORT);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let out = {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
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
        let mut table = DataTable::try_new(ID, columns, rows).expect("rectangular rows");
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

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn focused() -> Interaction {
    Interaction {
        focus: Some(ID),
        ..Interaction::default()
    }
}

fn screen_has(buf: &Buffer, text: &str) -> bool {
    (0..buf.area.height).any(|y| row_text(buf, y).contains(text))
}

fn per_size<T: Copy>(n: usize, values: [T; 3]) -> T {
    values[SIZES
        .iter()
        .position(|&size| size == n)
        .expect("a fixture size")]
}

fn picker_items(n: usize) -> Vec<Item> {
    (0..n)
        .map(|i| {
            Item::new(format!("Item {i:06}"))
                .key(ItemKey::Index(i))
                .detail("detail")
        })
        .collect()
}

fn draw_picker(picker: &mut Picker) -> Buffer {
    frame(|buf, ctx| {
        picker.draw(VIEWPORT, buf, ctx);
        ctx.flush_overlays(buf);
    })
    .0
}

#[test]
fn work_contract_picker_linear_projection() {
    for n in SIZES {
        let items = picker_items(n);
        let mut picker = Picker::new(ID, "Pick");
        let (set, set_work) = measure(|| picker.set_items(items.clone()));
        set.expect("valid items");
        // Measured: set_items moves the items in and indexes them by key, so
        // it clones nothing.
        assert_exact(&set_work, &format!("picker set_items, n={n}"), &[]);

        draw_picker(&mut picker);
        let (buf, full) = measure(|| draw_picker(&mut picker));
        assert!(screen_has(&buf, "Item 000000"));
        // Measured: a settled frame paints 12 rows (the painted list is capped
        // at max_rows) but measures the width of every item's label, detail and
        // group (3 calls per item) to size the columns, so its cost grows with
        // the hidden list.
        assert_exact(
            &full,
            &format!("picker settled frame, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 12),
                (
                    Metric::IntrinsicWidthCalls,
                    per_size(n, [3_264, 30_264, 300_264]),
                ),
                (
                    Metric::TextBytesMeasured,
                    per_size(n, [17_264, 170_264, 1_700_264]),
                ),
            ],
        );

        let stride = n / 20;
        let visible: Vec<Item> = (0..20).map(|k| items[k * stride].clone()).collect();
        let (res, project) = measure(|| picker.set_visible_items(visible.clone()));
        res.expect("known keys");
        // Measured: each visible key resolves through the key index and the
        // projection stores source positions, so the projection compares no
        // identity and clones no item at any list size.
        assert_exact(&project, &format!("picker set_visible_items, n={n}"), &[]);

        let (buf, filtered) = measure(|| draw_picker(&mut picker));
        assert!(screen_has(&buf, "Item 000000"));
        // Measured: a frame over the 20 filtered items costs the same at every
        // hidden size.
        assert_exact(
            &filtered,
            &format!("picker filtered frame, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 12),
                (Metric::IntrinsicWidthCalls, 324),
                (Metric::TextBytesMeasured, 604),
            ],
        );

        let (res, replace) = measure(|| picker.replace_items(items.clone()));
        res.expect("valid items");
        // Measured: a full replacement moves the items in, rebuilds the key
        // index and restores the cursor by key, so it clones and compares
        // nothing.
        assert_exact(&replace, &format!("picker replace_items, n={n}"), &[]);

        publish(
            "picker",
            "settled_frame",
            n,
            full,
            "the frame shows the first item",
        );
        publish(
            "picker",
            "set_visible_items",
            n,
            project,
            "the picker shows the 20 projected items",
        );
    }
}

/// A cursor that is not on the first item must be re-found by key, not by a
/// scan: no identity comparison at any cursor position or list size.
#[test]
fn work_contract_picker_cursor_restore() {
    for n in SIZES {
        let items = picker_items(n);
        let build = |cursor: usize| {
            let mut picker = Picker::new(ID, "Pick");
            picker.set_items(items.clone()).expect("valid items");
            picker.set_cursor(cursor);
            assert_eq!(picker.current_key(), Some(ItemKey::Index(cursor)));
            picker
        };

        for cursor in [0, 19, n / 2] {
            // A 20 item window that contains the cursor item.
            let start = cursor.saturating_sub(10).min(n - 20);
            let visible: Vec<Item> = items[start..start + 20].to_vec();
            let mut picker = build(cursor);
            let (res, work) = measure(|| picker.set_visible_items(visible.clone()));
            res.expect("known keys");
            // Measured: the cursor item keeps the cursor, found through the
            // projection's key map, with no comparison and no clone.
            assert_exact(
                &work,
                &format!("picker set_visible_items, cursor {cursor}, n={n}"),
                &[],
            );
            assert_eq!(picker.current_key(), Some(ItemKey::Index(cursor)));

            let mut picker = build(cursor);
            let (res, work) = measure(|| picker.replace_items(items.clone()));
            res.expect("valid items");
            // Measured: the identity projection puts the cursor item at its
            // source position, found through the key index.
            assert_exact(
                &work,
                &format!("picker replace_items, cursor {cursor}, n={n}"),
                &[],
            );
            assert_eq!(picker.current_key(), Some(ItemKey::Index(cursor)));
        }

        // The cursor deep in a sparse projection, then a full replacement:
        // the source position is far from the visible position.
        let stride = n / 20;
        let sparse: Vec<Item> = (0..20).map(|k| items[k * stride].clone()).collect();
        let mut picker = build(0);
        picker.set_visible_items(sparse).expect("known keys");
        picker.set_cursor(19);
        assert_eq!(picker.current_key(), Some(ItemKey::Index(19 * stride)));
        let (res, work) = measure(|| picker.replace_items(items.clone()));
        res.expect("valid items");
        assert_exact(
            &work,
            &format!("picker replace_items from a sparse projection, n={n}"),
            &[],
        );
        assert_eq!(picker.current_key(), Some(ItemKey::Index(19 * stride)));
    }
}

#[test]
fn work_contract_select_open_frame() {
    for n in SIZES {
        let labels: Vec<String> = (0..n).map(|i| format!("option {i:06}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut select = Select::new(ID, "Choice", &refs, 0);
        let area = Rect::new(2, 2, 40, 3);
        let draw = |select: &mut Select| {
            frame(|buf, ctx| {
                select.render(area, buf, ctx, ctx.theme.canvas);
                ctx.flush_overlays(buf);
            })
            .0
        };

        let (buf, closed) = measure(|| draw(&mut select));
        assert!(screen_has(&buf, "option 000000"));
        // Measured: a closed select measures its label and clones nothing.
        assert_exact(
            &closed,
            &format!("select closed frame, n={n}"),
            &[
                (Metric::IntrinsicWidthCalls, 1),
                (Metric::TextBytesMeasured, 6),
            ],
        );

        let _ = select.on_key(&key(KeyCode::Enter));
        assert!(select.open, "Enter opens the list");
        draw(&mut select);
        let (buf, open) = measure(|| draw(&mut select));
        assert!(screen_has(&buf, "option 000007"));
        // Measured: every open frame paints 8 rows and clones only those 8
        // option labels into the overlay snapshot, so the clone is the same at
        // every list size.
        assert_exact(
            &open,
            &format!("select open frame, n={n}"),
            &[
                (Metric::PayloadBytesCloned, 104),
                (Metric::VisibleRowsPainted, 8),
                (Metric::IntrinsicWidthCalls, 1),
                (Metric::TextBytesMeasured, 6),
            ],
        );

        publish(
            "select",
            "open_frame",
            n,
            open,
            "the open list shows the first 8 options",
        );
    }
}

#[test]
fn work_contract_completion_open_frame() {
    for n in SIZES {
        let items: Vec<Item> = (0..n)
            .map(|i| Item::new(format!("symbol_{i:06}")).detail("fn"))
            .collect();
        let mut completion = Completion::new(ID);
        let draw = |completion: &mut Completion| {
            frame(|buf, ctx| {
                completion.render(VIEWPORT, buf, ctx);
                ctx.flush_overlays(buf);
            })
            .0
        };

        let (_, closed) = measure(|| draw(&mut completion));
        assert_exact(&closed, &format!("completion closed frame, n={n}"), &[]);

        completion.open(items, Rect::new(10, 5, 1, 1), 0);
        draw(&mut completion);
        let (buf, open) = measure(|| draw(&mut completion));
        assert!(screen_has(&buf, "symbol_000000"));
        // Measured: every open frame paints 8 rows, sizes the popup from
        // widths cached until the items change, and clones only the 8 painted
        // items into the overlay snapshot. The cost is the same at every
        // list size.
        assert_exact(
            &open,
            &format!("completion open frame, n={n}"),
            &[
                (Metric::PayloadBytesCloned, 120),
                (Metric::VisibleRowsPainted, 8),
                (Metric::IntrinsicWidthCalls, 240),
                (Metric::TextBytesMeasured, 360),
            ],
        );

        publish(
            "completion",
            "open_frame",
            n,
            open,
            "the popup shows the first 8 candidates",
        );
    }
}

fn editor_text(n: usize) -> String {
    (0..n)
        .map(|i| format!("let value_{i:06} = {i};"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn highlight_first_keyword(_: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    vec![(0..3, SyntaxTone::Keyword)]
}

fn one_block(text: &str) -> Vec<Range<usize>> {
    std::iter::once(0..text.len()).collect()
}

#[test]
fn work_contract_editor_prepared_frame() {
    for n in SIZES {
        let text = editor_text(n);
        let len = text.len() as u64;
        let mut editor = CodeEditor::new(ID, &text)
            .highlighter(highlight_first_keyword)
            .segmenter(one_block);
        let draw = |editor: &mut CodeEditor| {
            frame_with(focused(), |buf, ctx| {
                editor.render(VIEWPORT, buf, ctx, ctx.theme.canvas);
            })
            .0
        };
        let widths = |calls: [u64; 3], bytes: [u64; 3]| {
            [
                (Metric::IntrinsicWidthCalls, per_size(n, calls)),
                (Metric::TextBytesMeasured, per_size(n, bytes)),
            ]
        };

        let (buf, first) = measure(|| draw(&mut editor));
        assert!(screen_has(&buf, "let value_000000 = 0;"));
        // Measured: the first focused frame paints 29 rows from the cached
        // line view, copies and hashes no text, and calls both annotation
        // providers once.
        let [w, b] = widths([30, 30, 30], [67, 68, 69]);
        assert_exact(
            &first,
            &format!("editor first frame, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 29),
                w,
                b,
                (Metric::AnnotationQueries, 2),
            ],
        );

        let (_, settled) = measure(|| draw(&mut editor));
        // Measured: a settled frame paints the same rows and calls no
        // annotation provider: both results are cached by the text revision.
        assert_exact(
            &settled,
            &format!("editor settled frame, n={n}"),
            &[(Metric::VisibleRowsPainted, 29), w, b],
        );

        let (_, first_key) = measure(|| editor.on_key(&key(KeyCode::Char('x'))));
        // Measured: the first keystroke of a run snapshots the whole text for
        // undo.
        assert_exact(
            &first_key,
            &format!("editor first keystroke, n={n}"),
            &[(Metric::PayloadBytesCloned, len)],
        );
        let (_, second_key) = measure(|| editor.on_key(&key(KeyCode::Char('y'))));
        assert_exact(
            &second_key,
            &format!("editor second keystroke of the run, n={n}"),
            &[],
        );

        let (buf, after) = measure(|| draw(&mut editor));
        assert!(screen_has(&buf, "xylet value_000000 = 0;"));
        // Measured: the frame after an edit repeats the settled frame's work
        // and re-runs both annotation providers on the new text revision.
        assert_exact(
            &after,
            &format!("editor frame after edit, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 29),
                w,
                b,
                (Metric::AnnotationQueries, 2),
            ],
        );

        publish(
            "code_editor",
            "settled_frame",
            n,
            settled,
            "the frame shows the first line",
        );
        publish(
            "code_editor",
            "frame_after_edit",
            n,
            after,
            "the first row starts with the two typed characters",
        );
    }
}

fn notes_text(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {i:06} of the seeded notes"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn work_contract_textarea_prepared_frame() {
    for n in SIZES {
        let text = notes_text(n);
        let len = text.len() as u64;
        let mut area = TextArea::new(ID, "Notes", 10).value(&text);
        let draw = |area: &mut TextArea| {
            frame_with(focused(), |buf, ctx| {
                area.render(VIEWPORT, buf, ctx, ctx.theme.canvas);
            })
            .0
        };

        let (buf, first) = measure(|| draw(&mut area));
        let last = format!("line {:06} of the seeded notes", n - 1);
        assert!(
            screen_has(&buf, &last),
            "a focused area follows its cursor to the end"
        );
        // Measured: the first focused frame snapshots the text for edit entry
        // and paints the 10 visible lines. Only those lines are measured.
        assert_exact(
            &first,
            &format!("textarea first frame, n={n}"),
            &[
                (Metric::PayloadBytesCloned, len),
                (Metric::VisibleRowsPainted, 10),
                (Metric::IntrinsicWidthCalls, 3),
                (Metric::TextBytesMeasured, per_size(n, [29, 33, 37])),
            ],
        );

        let (_, settled) = measure(|| draw(&mut area));
        // Measured: a settled frame clones nothing and measures only the
        // visible lines.
        assert_exact(
            &settled,
            &format!("textarea settled frame, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 10),
                (Metric::IntrinsicWidthCalls, 3),
                (Metric::TextBytesMeasured, per_size(n, [29, 33, 37])),
            ],
        );

        let (_, first_key) = measure(|| area.on_key(&key(KeyCode::Char('x'))));
        assert_exact(
            &first_key,
            &format!("textarea first keystroke, n={n}"),
            &[(Metric::PayloadBytesCloned, len)],
        );
        let (_, second_key) = measure(|| area.on_key(&key(KeyCode::Char('y'))));
        assert_exact(
            &second_key,
            &format!("textarea second keystroke of the run, n={n}"),
            &[],
        );

        let (buf, after) = measure(|| draw(&mut area));
        assert!(screen_has(&buf, &format!("{last}xy")));
        assert_exact(
            &after,
            &format!("textarea frame after edit, n={n}"),
            &[
                (Metric::VisibleRowsPainted, 10),
                (Metric::IntrinsicWidthCalls, 3),
                (Metric::TextBytesMeasured, per_size(n, [29, 33, 37])),
            ],
        );

        publish(
            "textarea",
            "settled_frame",
            n,
            settled,
            "the frame shows the last 10 lines, following the cursor",
        );
        publish(
            "textarea",
            "frame_after_edit",
            n,
            after,
            "the last row ends with the two typed characters",
        );
    }
}

#[test]
fn work_report_is_count_only_json_with_every_metric() {
    let work = measure(|| junie_tui::record_work!(SegmentedLines, 3usize)).1;
    let report = ScenarioReport {
        scenario: "report_shape",
        operation: "one_count",
        record_count: 3,
        viewport: (100, 30),
        work,
        correctness_oracle: "quoted \"text\" with a \\ and a newline\n".to_owned(),
    };
    let parsed: serde_json::Value = serde_json::from_str(&report.to_json()).expect("valid JSON");

    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(
        parsed["schema_version"],
        junie_tui::metrics::report::SCHEMA_VERSION,
        "the JSON states the schema version the crate exports"
    );
    assert_eq!(parsed["instrumentation_enabled"], true);
    assert_eq!(
        parsed["work_delta"][Metric::IdentityComparisons.name()],
        0,
        "a metric nothing recorded is listed as a zero"
    );
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

#[test]
fn work_report_concurrent_writers_leave_one_whole_report() {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("work-report-race");
    let _ = std::fs::remove_dir_all(&dir);
    let reports: Vec<ScenarioReport> = (0..8u64)
        .map(|n| ScenarioReport {
            scenario: "race",
            operation: "same_path",
            record_count: 1,
            viewport: (100, 30),
            work: WorkDelta([n; Metric::ALL.len()]),
            correctness_oracle: format!("writer {n} {}", "x".repeat(4_096)),
        })
        .collect();
    let whole: Vec<String> = reports.iter().map(ScenarioReport::to_json).collect();
    let writers_left = AtomicUsize::new(reports.len());

    let (paths, torn_reads) = std::thread::scope(|scope| {
        let writers: Vec<_> = reports
            .iter()
            .map(|report| {
                let (dir, writers_left) = (&dir, &writers_left);
                scope.spawn(move || {
                    let mut path = PathBuf::new();
                    for _ in 0..200 {
                        path = report.write_in(dir).expect("write");
                    }
                    writers_left.fetch_sub(1, Ordering::SeqCst);
                    path
                })
            })
            .collect();
        let reader = scope.spawn(|| {
            let path = dir.join("race-same_path-1.json");
            let mut torn = 0usize;
            while writers_left.load(Ordering::SeqCst) > 0 {
                if let Ok(text) = std::fs::read_to_string(&path)
                    && !whole.contains(&text)
                {
                    torn += 1;
                }
            }
            torn
        });
        let paths: Vec<_> = writers
            .into_iter()
            .map(|h| h.join().expect("writer"))
            .collect();
        (paths, reader.join().expect("reader"))
    });

    assert_eq!(torn_reads, 0, "a reader saw a truncated or mixed report");
    let path = &paths[0];
    assert!(
        paths.iter().all(|p| p == path),
        "every writer targets one path"
    );
    let text = std::fs::read_to_string(path).expect("read");
    assert!(
        whole.contains(&text),
        "the file is exactly one writer's whole report"
    );
    let names: Vec<_> = std::fs::read_dir(&dir)
        .expect("list")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names.len(), 1, "no temp file is left behind: {names:?}");
}

#[test]
fn work_report_write_publishes_under_the_performance_directory() {
    let report = ScenarioReport {
        scenario: "write_probe",
        operation: "one",
        record_count: 1,
        viewport: (10, 2),
        work: WorkDelta::ZERO,
        correctness_oracle: "the file holds exactly to_json".to_owned(),
    };
    let path = report.write().expect("write the count report");
    assert!(
        path.ends_with("performance/write_probe-one-1.json"),
        "{path:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read it back"),
        report.to_json()
    );
    let _ = std::fs::remove_file(&path);
}
