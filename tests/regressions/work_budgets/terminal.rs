//! `TerminalView` and `TerminalViewport` work contracts (#56).
//!
//! The emulated grid is fixed at 30 rows by 99 columns (the 100x30 viewport
//! less the scroll track) while the scrollback behind it holds 1,000, 10,000
//! or 100,000 lines. Feeding output, painting a prepared frame and selecting
//! or copying are measured as separate operations. `SourceRowsRead` counts the
//! grid or scrollback rows a paint, selection or copy reads from the emulator;
//! `TerminalBytesFed` counts the bytes handed to its `feed`.

use super::*;
use junie_tui::terminal::{TerminalState, TerminalView, TerminalViewport};
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Position;
use ratatui::style::Color;
use ratatui::widgets::Widget;

const ROWS: u64 = 30;
const CHUNK_LINES: usize = 100;

fn output(from: usize, count: usize) -> Vec<u8> {
    (from..from + count)
        .flat_map(|i| format!("{}\r\n", line_text(i)).into_bytes())
        .collect()
}

fn viewport(n: usize) -> TerminalViewport {
    let mut v = TerminalViewport::new(ID, TerminalState::with_scrollback(30, 99, n));
    v.feed(&output(0, n));
    v.set_area(VIEWPORT);
    v
}

fn draw(v: &TerminalViewport) -> Buffer {
    frame(|buf, ctx| {
        v.draw(VIEWPORT, buf, ctx, Color::Reset, true);
    })
    .0
}

fn publish_terminal(operation: &'static str, n: usize, work: WorkDelta, oracle: &str) {
    publish("terminal", operation, n, work, oracle);
}

#[test]
fn work_contract_terminal_prepared_paint() {
    for n in SIZES {
        let mut v = viewport(n);
        let history = v.state().scrollback_len();
        assert_eq!(history, n - 29, "the scrollback behind the grid, n={n}");
        let only_rows = [(Metric::SourceRowsRead, ROWS)];

        let (buf, live) = measure(|| draw(&v));
        assert_shows(&buf, 0, &line_text(n - 29));
        assert_exact(&live, "viewport draw at the live tail", &only_rows);
        publish_terminal(
            "viewport_draw_live_tail",
            n,
            live,
            "row 0 shows the oldest line still on the grid",
        );

        let (_, again) = measure(|| draw(&v));
        assert_exact(&again, "second viewport draw", &only_rows);

        let (buf, widget) = measure(|| {
            let mut buf = Buffer::empty(VIEWPORT);
            TerminalView::new(v.state()).render(VIEWPORT, &mut buf);
            buf
        });
        assert_shows(&buf, 0, &line_text(n - 29));
        assert_exact(&widget, "TerminalView render", &only_rows);
        publish_terminal(
            "terminal_view_render",
            n,
            widget,
            "row 0 shows the oldest line still on the grid",
        );

        v.jump_start();
        assert_eq!(v.offset(), history);
        let (buf, deep) = measure(|| draw(&v));
        assert_shows(&buf, 0, &line_text(0));
        assert_exact(&deep, "viewport draw at the oldest history", &only_rows);
        publish_terminal(
            "viewport_draw_oldest_history",
            n,
            deep,
            "row 0 shows the first line ever fed",
        );
    }
}

#[test]
fn work_contract_terminal_feed_and_copy_units() {
    for n in SIZES {
        let mut v = viewport(n);
        let chunk = output(n, CHUNK_LINES);
        assert_eq!(chunk.len(), 3_500);

        let (_, fed) = measure(|| v.feed(&chunk));
        assert_exact(
            &fed,
            "feed 100 lines",
            &[(Metric::TerminalBytesFed, chunk.len() as u64)],
        );
        publish_terminal(
            "feed_100_lines",
            n,
            fed,
            "every byte is fed once and no row is read",
        );

        // The 100 new lines pushed history; select rows 1..=10 of the grid.
        let first = n + CHUNK_LINES - 29;
        let (_, pressed) = measure(|| v.on_press_with(Position::new(1, 1), 1, KeyModifiers::NONE));
        assert_exact(
            &pressed,
            "press to start a selection",
            &[(Metric::SourceRowsRead, 5)],
        );
        let (_, dragged) = measure(|| v.on_drag(Position::new(30, 10)));
        assert_exact(
            &dragged,
            "drag to extend it",
            &[(Metric::SourceRowsRead, 1)],
        );

        let (text, copied) = measure(|| v.selected_text());
        let text = text.expect("ten selected rows");
        assert_eq!(text.lines().count(), 10);
        assert!(
            text.starts_with(&line_text(first + 1)[1..]),
            "the copy starts at column 1 of row 1: {text:?}"
        );
        // A row is read once to cut its text and once more to ask whether the
        // next row continues it: 10 rows, 9 row boundaries.
        assert_exact(
            &copied,
            "copy ten selected rows",
            &[(Metric::SourceRowsRead, 19)],
        );
        publish_terminal(
            "copy_10_selected_rows",
            n,
            copied,
            "ten rows are copied; reads do not grow with the scrollback",
        );
    }
}
