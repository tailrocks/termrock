//! Release elapsed-time measurements for #56.
//!
//! Each `measurement_*` scenario times one real library operation at 1,000 /
//! 10,000 / 100,000 seeded records (60,000 rows at most for `ScrollView`,
//! whose extent is `u16`) in the fixed 100x30 viewport: 20 warm-ups, then 200
//! samples. The timed interval holds exactly the operation. Fixture building,
//! the correctness oracle before and after each operation, and the restore of
//! any state the operation consumed all sit outside it.
//!
//! These tests are `#[ignore]`d. They only mean something in an optimized
//! build with the count instrumentation off, and only when nothing else is
//! running, so run them through `scripts/measure-performance.sh`, which sets
//! that up and records the machine load. A report goes to
//! `<target dir>/performance/<scenario>-<operation>-<records>.timing.json`.
//!
//! No wall-clock gate exists: nothing here compares a time to a threshold.
//! The one test that is not ignored, `measurement_edit_samples_are_real_changes`,
//! checks the sampling logic itself at a tiny size, so a scenario whose
//! "edit" degenerates into a no-op after its first sample fails the profile.

use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use junie_tui::core::event::Key;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::metrics::ENABLED;
use junie_tui::metrics::measurement::{Environment, MeasurementReport, SAMPLES, WARMUPS};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::scroll_view::ScrollView;
use junie_tui::widgets::code::CodeEditor;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::textarea::TextArea;
use junie_tui::widgets::tree::{TreeNode, TreeView};
use junie_tui::widgets::viewport::{Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};

const VIEWPORT: Rect = Rect::new(0, 0, 100, 30);
const SIZES: [usize; 3] = [1_000, 10_000, 100_000];
const SCROLL_ROWS: [u16; 3] = [1_000, 10_000, 60_000];
const ID: WidgetId = WidgetId::of("work.measurement");
const BACKEND: &str = "in-memory ratatui Buffer (no terminal, no backend I/O)";

/// Samples of one scenario at one size, with what the report needs beside them.
struct Outcome {
    scenario: &'static str,
    operation: &'static str,
    record_count: usize,
    sample_ns: Vec<u64>,
    setup_ns: u64,
    oracle: &'static str,
}

/// Runs `op` for `warmups + samples` rounds over `state` and returns the
/// elapsed nanoseconds of the sampled rounds. `before` runs ahead of the timer
/// and `after` checks the result behind it, so neither is in an interval.
fn sample<S, T>(
    state: &mut S,
    warmups: usize,
    samples: usize,
    mut before: impl FnMut(&mut S, usize),
    mut op: impl FnMut(&mut S, usize) -> T,
    mut after: impl FnMut(&mut S, usize, &T),
) -> Vec<u64> {
    let mut out = Vec::with_capacity(samples);
    for round in 0..warmups + samples {
        before(state, round);
        let start = Instant::now();
        let result = black_box(op(state, round));
        let elapsed = start.elapsed();
        after(state, round, &result);
        if round >= warmups {
            out.push(u64::try_from(elapsed.as_nanos()).expect("a sample fits u64 nanoseconds"));
        }
    }
    out
}

fn paint<T>(paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> T) -> (Buffer, T) {
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

fn assert_row(buf: &Buffer, y: u16, text: &str) {
    let row = row_text(buf, y);
    assert!(row.starts_with(text), "row {y} is {row:?}, want {text:?}");
}

fn history(n: usize) -> Vec<Line> {
    (0..n).map(|i| vec![Span::plain(line_text(i))]).collect()
}

fn line_text(i: usize) -> String {
    format!("line {i:06} of the seeded history")
}

fn timed_setup<T>(build: impl FnOnce() -> T) -> (T, u64) {
    let start = Instant::now();
    let built = build();
    let ns = u64::try_from(start.elapsed().as_nanos()).expect("setup fits u64");
    (black_box(built), ns)
}

/// One pushed line, then `set_area` to take it into the layout. The history
/// grows by a line per round, so every sample appends different text and no
/// round is an equal write. The oracle draws the frame behind the timer.
fn viewport_append(n: usize, warmups: usize, samples: usize) -> Outcome {
    let (mut v, setup_ns) = timed_setup(|| {
        let mut v = TextViewport::with_lines(ID, history(n));
        v.set_area(VIEWPORT);
        v
    });
    let sample_ns = sample(
        &mut v,
        warmups,
        samples,
        |_, _| {},
        |v, round| {
            v.push(vec![Span::plain(format!("appended line {round}"))]);
            v.set_area(VIEWPORT);
        },
        |v, round, ()| {
            let (buf, _) = paint(|buf, ctx| v.draw(VIEWPORT, buf, ctx, ctx.theme.canvas, true));
            assert_row(&buf, 29, &format!("appended line {round}"));
            let above = if round == 0 {
                line_text(n - 1)
            } else {
                format!("appended line {}", round - 1)
            };
            assert_row(&buf, 28, &above);
        },
    );
    Outcome {
        scenario: "viewport_prepared",
        operation: "append_one_line",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "after every round the last frame row shows that round's appended line and the row above shows the previous one",
    }
}

/// A frame of an already prepared `TextViewport`: no state changes, the
/// oracle is the pixels of the last 30 lines.
fn viewport_prepared_draw(n: usize, warmups: usize, samples: usize) -> Outcome {
    let (mut v, setup_ns) = timed_setup(|| {
        let mut v = TextViewport::with_lines(ID, history(n));
        v.set_area(VIEWPORT);
        v
    });
    let sample_ns = sample(
        &mut v,
        warmups,
        samples,
        |_, _| {},
        |v, _| paint(|buf, ctx| v.draw(VIEWPORT, buf, ctx, ctx.theme.canvas, true)).0,
        |_, _, buf| {
            assert_row(buf, 0, &line_text(n - 30));
            assert_row(buf, 29, &line_text(n - 1));
        },
    );
    Outcome {
        scenario: "viewport_prepared",
        operation: "prepared_draw",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "every frame shows the last 30 lines, following the tail",
    }
}

/// A settled `ListBox` frame, checked cell-equal to the warm frame.
fn list_settled_frame(n: usize, warmups: usize, samples: usize) -> Outcome {
    let ((mut list, warm), setup_ns) = timed_setup(|| {
        let items = (0..n)
            .map(|i| ListItem::new(&format!("item {i:06}")))
            .collect();
        let mut list = ListBox::new(ID, items, SelectMode::Single);
        let warm = paint(|b, c| list.render(VIEWPORT, b, c, c.theme.canvas)).0;
        (list, warm)
    });
    assert!(
        row_text(&warm, 0).contains("item 000000"),
        "row 0 shows the first item"
    );
    let mut state = (&mut list, warm);
    let sample_ns = sample(
        &mut state,
        warmups,
        samples,
        |_, _| {},
        |(list, _), _| paint(|b, c| list.render(VIEWPORT, b, c, c.theme.canvas)).0,
        |(_, warm), _, buf| assert_eq!(buf, warm, "a settled list repaints the same cells"),
    );
    Outcome {
        scenario: "list_box",
        operation: "settled_frame",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "every frame is cell-equal to the warm frame, and row 0 shows item 0",
    }
}

/// Changes the `TreeView` filter between two queries that keep different
/// numbers of nodes. Alternating makes every round a real filter change; a
/// repeated query would be an equal write after the first round.
fn tree_filter_change(n: usize, warmups: usize, samples: usize) -> Outcome {
    const QUERIES: [&str; 2] = ["node 5", "node 55"];
    let labels: Vec<String> = (0..n).map(|i| format!("Node {i}")).collect();
    let expected: [usize; 2] = QUERIES.map(|q| {
        labels
            .iter()
            .filter(|l| l.to_lowercase().contains(q))
            .count()
    });
    assert_ne!(
        expected[0], expected[1],
        "the two queries must keep different node counts at n={n}"
    );
    let (mut tree, setup_ns) =
        timed_setup(|| TreeView::new(ID, labels.iter().map(|l| TreeNode::leaf(l)).collect()));
    let mut previous_rows = tree.rows().len();
    let sample_ns = sample(
        &mut tree,
        warmups,
        samples,
        |_, _| {},
        |tree, round| tree.set_filter(Some(QUERIES[round % 2])),
        |tree, round, ()| {
            let rows = tree.rows().len();
            assert_eq!(rows, expected[round % 2], "round {round} kept the matches");
            assert_ne!(
                rows, previous_rows,
                "round {round} changed the visible rows"
            );
            previous_rows = rows;
        },
    );
    Outcome {
        scenario: "tree_view",
        operation: "filter_change",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "after every round the visible row count equals the labels containing that round's query, and differs from the previous round's",
    }
}

/// `ScrollView::render` of overflowing content: allocates and fills a canvas
/// as large as the content (#20), so this is the cost of today's behaviour.
fn scroll_view_render(rows: u16, warmups: usize, samples: usize) -> Outcome {
    let (mut sv, setup_ns) = timed_setup(|| ScrollView::new(ID));
    let content = Size::new(100, rows);
    let sample_ns = sample(
        &mut sv,
        warmups,
        samples,
        |_, _| {},
        |sv, _| {
            paint(|buf, ctx| {
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
        },
        |_, _, buf| assert_row(buf, 0, "first content row"),
    );
    Outcome {
        scenario: "scroll_view",
        operation: "render_overflowing_content",
        record_count: usize::from(rows),
        sample_ns,
        setup_ns,
        oracle: "every frame's first row shows the first content row",
    }
}

/// A paint with the widget focused, as an editor is while it is being typed in.
fn paint_focused<T>(paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> T) -> (Buffer, T) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(VIEWPORT);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus: Some(ID),
        ..Interaction::default()
    };
    let out = {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        paint(&mut buf, &mut ctx)
    };
    (buf, out)
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn screen_has(buf: &Buffer, text: &str) -> bool {
    (0..buf.area.height).any(|y| row_text(buf, y).contains(text))
}

fn code_line(i: usize) -> String {
    format!("let value_{i:06} = {i};")
}

fn code_text(n: usize) -> String {
    (0..n).map(code_line).collect::<Vec<_>>().join("\n")
}

fn highlight_first_keyword(_: &str) -> Vec<(std::ops::Range<usize>, junie_tui::theme::SyntaxTone)> {
    vec![(0..3, junie_tui::theme::SyntaxTone::Keyword)]
}

fn one_block(text: &str) -> Vec<std::ops::Range<usize>> {
    std::iter::once(0..text.len()).collect()
}

/// One keystroke in a `CodeEditor`, then the frame it causes. Rounds alternate
/// typing `x` at the start of the first line and Backspace, so every round
/// changes the text between two genuinely distinct values and the highlighter
/// and block segmenter re-run on a new text revision each time. The oracle
/// checks the text and the first painted line behind the timer.
fn editor_edit_then_frame(n: usize, warmups: usize, samples: usize) -> Outcome {
    let base_len = code_text(n).len();
    let ((mut editor, ()), setup_ns) = timed_setup(|| {
        let mut editor = CodeEditor::new(ID, &code_text(n))
            .highlighter(highlight_first_keyword)
            .segmenter(one_block);
        paint_focused(|buf, ctx| editor.render(VIEWPORT, buf, ctx, ctx.theme.canvas));
        (editor, ())
    });
    let sample_ns = sample(
        &mut editor,
        warmups,
        samples,
        |_, _| {},
        |editor, round| {
            let code = if round % 2 == 0 {
                KeyCode::Char('x')
            } else {
                KeyCode::Backspace
            };
            editor.on_key(&key(code));
            paint_focused(|buf, ctx| editor.render(VIEWPORT, buf, ctx, ctx.theme.canvas)).0
        },
        |editor, round, buf| {
            let typed = round % 2 == 0;
            assert_eq!(
                editor.text().len(),
                base_len + usize::from(typed),
                "round {round}: the keystroke changed the text"
            );
            let first = format!("{}{}", if typed { "x" } else { "" }, code_line(0));
            assert!(
                editor.text().starts_with(&first) && screen_has(buf, &first),
                "round {round}: the frame shows {first:?}"
            );
        },
    );
    Outcome {
        scenario: "code_editor",
        operation: "keystroke_then_frame",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "after every round the text is one byte longer on a typed round and back to its original length on a Backspace round, and the frame shows the first line with or without the typed x",
    }
}

fn notes_line(i: usize) -> String {
    format!("line {i:06} of the seeded notes")
}

/// One keystroke in a `TextArea`, then the frame it causes: `x` appended at the
/// end of the last line, then Backspace, alternating. The first focused frame
/// is part of the setup, so the area already follows its cursor to the end and
/// has taken its edit-entry snapshot. The oracle reads the painted last line.
fn textarea_edit_then_frame(n: usize, warmups: usize, samples: usize) -> Outcome {
    let text = (0..n).map(notes_line).collect::<Vec<_>>().join("\n");
    let last = notes_line(n - 1);
    let ((mut area, ()), setup_ns) = timed_setup(|| {
        let mut area = TextArea::new(ID, "Notes", 10).value(&text);
        let (buf, ()) = paint_focused(|buf, ctx| area.render(VIEWPORT, buf, ctx, ctx.theme.canvas));
        assert!(screen_has(&buf, &last), "the warm frame follows the cursor");
        (area, ())
    });
    let sample_ns = sample(
        &mut area,
        warmups,
        samples,
        |_, _| {},
        |area, round| {
            let code = if round % 2 == 0 {
                KeyCode::Char('x')
            } else {
                KeyCode::Backspace
            };
            area.on_key(&key(code));
            paint_focused(|buf, ctx| area.render(VIEWPORT, buf, ctx, ctx.theme.canvas)).0
        },
        |_, round, buf| {
            let typed = round % 2 == 0;
            let shown = (0..buf.area.height)
                .map(|y| row_text(buf, y))
                .find(|row| row.contains(&last))
                .unwrap_or_default();
            let after = shown
                .split_once(&last)
                .map(|(_, rest)| rest.trim_start().chars().next());
            assert!(
                after.is_some(),
                "round {round}: the last line {last:?} is on screen, got {shown:?}"
            );
            assert_eq!(
                after.flatten() == Some('x'),
                typed,
                "round {round}: the typed x follows the last line exactly on a typed round, got {shown:?}"
            );
        },
    );
    Outcome {
        scenario: "text_area",
        operation: "keystroke_then_frame",
        record_count: n,
        sample_ns,
        setup_ns,
        oracle: "after every round the painted last line ends with the typed x on a typed round and without it on a Backspace round",
    }
}

/// `ScrollView::render_window` (#20): the viewport-sized path. Rounds scroll
/// the window between two offsets so each frame shows different rows, and the
/// oracle checks the first painted row behind the timer.
fn scroll_view_render_window(rows: u16, warmups: usize, samples: usize) -> Outcome {
    const OFFSETS: [usize; 2] = [0, 500];
    let (mut sv, setup_ns) = timed_setup(|| ScrollView::new(ID));
    let content = Size::new(20, rows);
    let sample_ns = sample(
        &mut sv,
        warmups,
        samples,
        |sv, round| {
            assert!(
                sv.scroll_to(0, OFFSETS[round % 2]) || round == 0,
                "round {round}: the window moves between the two offsets"
            );
        },
        |sv, _| {
            paint(|buf, ctx| {
                sv.render_window(
                    VIEWPORT,
                    VIEWPORT,
                    content,
                    None,
                    buf,
                    ctx,
                    |window, buf, _| {
                        for index in window.content_rows() {
                            let at = window.project_content_rect(Rect::new(0, index as u16, 20, 1));
                            buf.set_string(
                                at.x,
                                at.y,
                                format!("row {index:05}"),
                                ratatui::style::Style::new(),
                            );
                        }
                    },
                );
            })
            .0
        },
        |_, round, buf| assert_row(buf, 0, &format!("row {:05}", OFFSETS[round % 2])),
    );
    Outcome {
        scenario: "scroll_view",
        operation: "render_window_scroll",
        record_count: usize::from(rows),
        sample_ns,
        setup_ns,
        oracle: "every frame's first row shows the first row of the offset that round scrolled to, alternating row 0 and row 500",
    }
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn cpu() -> Option<String> {
    command("sysctl", &["-n", "machdep.cpu.brand_string"]).or_else(|| {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("model name"))
            .map(|l| l.trim_start_matches([' ', '\t', ':']).trim().to_owned())
    })
}

/// Reads the environment from the running machine. A value it cannot read is
/// `"unknown"`, never a guess.
fn environment() -> Environment {
    let unknown = || "unknown".to_owned();
    let rustc_full = command("rustc", &["-vV"]);
    let rustc = command("rustc", &["--version"]).unwrap_or_else(unknown);
    let target = rustc_full
        .as_deref()
        .and_then(|v| v.lines().find_map(|l| l.strip_prefix("host: ")))
        .map_or_else(unknown, str::to_owned);
    Environment {
        rustc,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .to_owned(),
        target,
        os: command("uname", &["-sr"]).unwrap_or_else(unknown),
        cpu: cpu().unwrap_or_else(unknown),
        backend: BACKEND.to_owned(),
        commit: command("git", &["rev-parse", "HEAD"]).unwrap_or_else(unknown),
        commit_dirty: dirty(),
        load_average: command("uptime", &[]),
    }
}

/// Whether tracked files differ from HEAD, or `None` if git cannot say.
fn dirty() -> Option<bool> {
    let out = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()?;
    out.status.success().then_some(!out.stdout.is_empty())
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target")))
}

/// Refuses a build whose timing means nothing, then runs every size of one
/// scenario and writes a report each.
fn measure_scenario(sizes: &[usize], run: impl Fn(usize, usize, usize) -> Outcome) {
    if cfg!(debug_assertions) {
        panic!("timing runs need an optimized build: use scripts/measure-performance.sh");
    }
    if ENABLED {
        panic!("timing runs need work-metrics off; counts are in the work_budgets reports");
    }
    let environment = environment();
    for &size in sizes {
        let outcome = run(size, WARMUPS, SAMPLES);
        let report = MeasurementReport {
            scenario: outcome.scenario,
            operation: outcome.operation,
            record_count: outcome.record_count,
            viewport: (VIEWPORT.width, VIEWPORT.height),
            warmups: WARMUPS,
            sample_ns: outcome.sample_ns,
            work_delta: None,
            setup_ns: Some(outcome.setup_ns),
            terminal_io_included: false,
            correctness_oracle: outcome.oracle.to_owned(),
            environment: environment.clone(),
        };
        let path = report
            .write_in(&target_dir().join("performance"))
            .expect("write the timing report");
        println!("wrote {}", path.display());
    }
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_viewport_append_one_line() {
    measure_scenario(&SIZES, viewport_append);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_viewport_prepared_draw() {
    measure_scenario(&SIZES, viewport_prepared_draw);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_list_settled_frame() {
    measure_scenario(&SIZES, list_settled_frame);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_tree_filter_change() {
    measure_scenario(&SIZES, tree_filter_change);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_editor_keystroke_then_frame() {
    measure_scenario(&SIZES, editor_edit_then_frame);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_textarea_keystroke_then_frame() {
    measure_scenario(&SIZES, textarea_edit_then_frame);
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_scroll_view_render_window() {
    let sizes = SCROLL_ROWS.map(usize::from);
    measure_scenario(&sizes, |rows, w, s| {
        scroll_view_render_window(u16::try_from(rows).expect("rows fit the u16 extent"), w, s)
    });
}

#[test]
#[ignore = "release measurement: run scripts/measure-performance.sh"]
fn measurement_scroll_view_render() {
    let sizes = SCROLL_ROWS.map(usize::from);
    measure_scenario(&sizes, |rows, w, s| {
        scroll_view_render(u16::try_from(rows).expect("rows fit the u16 extent"), w, s)
    });
}

/// The mutating scenarios must make a real change every round, including the
/// warm-ups. Each scenario asserts its own per-round oracle, so running them
/// at a tiny size in any build proves a sample cannot degenerate into a no-op:
/// a scenario that stopped changing its state fails its oracle here.
#[test]
fn measurement_edit_samples_are_real_changes() {
    let append = viewport_append(100, 2, 5);
    assert_eq!(append.sample_ns.len(), 5);
    let filter = tree_filter_change(100, 2, 5);
    assert_eq!(filter.sample_ns.len(), 5);
    // Non-mutating scenarios share the same sampler; they must run too.
    assert_eq!(viewport_prepared_draw(100, 1, 3).sample_ns.len(), 3);
    assert_eq!(list_settled_frame(100, 1, 3).sample_ns.len(), 3);
    assert_eq!(scroll_view_render(200, 1, 3).sample_ns.len(), 3);
    // The keystroke scenarios and the window scroll change state every round
    // too, warm-ups included; their oracles fail a round that did not.
    assert_eq!(editor_edit_then_frame(100, 2, 5).sample_ns.len(), 5);
    assert_eq!(textarea_edit_then_frame(100, 2, 5).sample_ns.len(), 5);
    assert_eq!(scroll_view_render_window(1_000, 2, 5).sample_ns.len(), 5);
}
