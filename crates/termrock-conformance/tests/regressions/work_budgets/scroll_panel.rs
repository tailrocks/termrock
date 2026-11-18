//! `ScrollPanel` work contracts (#56, #39).
//!
//! The panel sits in a 100x30 viewport and shows 30 rows. Every count is exact
//! and is measured per operation, each ending in a frame. `SourceRowsRead` is
//! the number of source lines the panel read to wrap or measure; the painted
//! rows are `VisibleRowsPainted`; the width counters come from the shared width
//! helper. A flat panel with horizontal scrolling caches its widest line, a
//! wrapping panel caches its wrapped rows.

use super::*;
use ratatui::style::{Color, Style};
use termrock_core::scroll::HScroll;
use termrock_viewport::panel::ScrollPanel;

const ROWS: u64 = 30;
/// Bytes of the one line the append operations add.
const ADDED: &str = "one more line";
/// Text bytes the width helper measures while the 30 painted rows are fitted:
/// 30 seeded lines of 33 bytes.
const PAINTED_BYTES: u64 = 990;

fn lines(n: usize) -> Vec<String> {
    (0..n).map(line_text).collect()
}

fn flat(n: usize) -> ScrollPanel {
    ScrollPanel::new(ID, lines(n))
        .wrap(false)
        .horizontal(HScroll::default())
}

fn wrapped(n: usize) -> ScrollPanel {
    ScrollPanel::new(ID, lines(n)).wrap(true)
}

fn paint(panel: &mut ScrollPanel) -> Buffer {
    frame(|buf, ctx| {
        panel.render(VIEWPORT, buf, ctx, Color::Reset, |_, _| Style::new());
    })
    .0
}

fn publish_all(operation: &'static str, n: usize, work: WorkDelta, oracle: &str) {
    publish("scroll_panel", operation, n, work, oracle);
}

#[test]
fn work_contract_scroll_panel_flat_measures_once_and_appends_incrementally() {
    for n in SIZES {
        let nn = n as u64;
        let mut panel = flat(n);
        let (buf, first) = measure(|| paint(&mut panel));
        assert_shows(&buf, 0, &line_text(0));
        assert_exact(
            &first,
            "first frame",
            &[
                (Metric::SourceRowsRead, nn),
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, nn + ROWS),
                (Metric::TextBytesMeasured, 33 * nn + PAINTED_BYTES),
            ],
        );
        publish_all("first_frame", n, first, "row 0 shows the first seeded line");

        let (buf, settled) = measure(|| paint(&mut panel));
        assert_shows(&buf, 0, &line_text(0));
        assert_exact(
            &settled,
            "settled frame",
            &[
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, ROWS),
                (Metric::TextBytesMeasured, PAINTED_BYTES),
            ],
        );
        publish_all(
            "settled_frame",
            n,
            settled,
            "the widest-line cache is reused: no source row is read",
        );

        let (buf, appended) = measure(|| {
            panel.push(ADDED.to_owned());
            paint(&mut panel)
        });
        assert_shows(&buf, 0, &line_text(0));
        assert_eq!(panel.lines().len(), n + 1);
        assert_exact(
            &appended,
            "push then frame",
            &[
                (Metric::SourceRowsRead, 1),
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, 1 + ROWS),
                (
                    Metric::TextBytesMeasured,
                    PAINTED_BYTES + ADDED.len() as u64,
                ),
            ],
        );
        publish_all(
            "push_then_frame",
            n,
            appended,
            "one pushed line is measured once; the cached widest line is extended",
        );

        let (_, scrolled) = measure(|| {
            panel.scroll.scroll_by(1);
            paint(&mut panel)
        });
        assert_exact(
            &scrolled,
            "scroll one row then frame",
            &[
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, ROWS),
                (Metric::TextBytesMeasured, PAINTED_BYTES),
            ],
        );
    }
}

/// One pushed line, then a frame, on a settled wrapping panel: the added line
/// is the only source row read, whatever the history length.
const APPEND_THEN_FRAME: [(Metric, u64); 4] = [
    (Metric::SourceRowsRead, 1),
    (Metric::VisibleRowsPainted, ROWS),
    (Metric::IntrinsicWidthCalls, 46),
    (Metric::TextBytesMeasured, PAINTED_BYTES + 24),
];

#[test]
fn work_contract_scroll_panel_wrapped_builds_once_per_content_change() {
    // An append keeps the wrapped prefix and wraps only the added line (#291),
    // so the push row does not depend on n. Any other mutation rebuilds.
    for n in SIZES {
        let nn = n as u64;
        let mut panel = wrapped(n);
        let (buf, first) = measure(|| paint(&mut panel));
        assert_shows(&buf, 0, &line_text(0));
        assert_exact(
            &first,
            "first frame",
            &[
                (Metric::SourceRowsRead, nn),
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, 39 * nn + ROWS),
                (Metric::TextBytesMeasured, 61 * nn + PAINTED_BYTES),
            ],
        );
        publish_all(
            "wrapped_first_frame",
            n,
            first,
            "every source line is wrapped once",
        );

        let (_, settled) = measure(|| paint(&mut panel));
        assert_exact(
            &settled,
            "settled frame",
            &[
                (Metric::VisibleRowsPainted, ROWS),
                (Metric::IntrinsicWidthCalls, ROWS),
                (Metric::TextBytesMeasured, PAINTED_BYTES),
            ],
        );
        publish_all(
            "wrapped_settled_frame",
            n,
            settled,
            "the wrapped rows are reused: no source row is read",
        );

        let (buf, appended) = measure(|| {
            panel.push(ADDED.to_owned());
            paint(&mut panel)
        });
        assert_shows(&buf, 0, &line_text(0));
        assert_exact(&appended, "push then frame", &APPEND_THEN_FRAME);
        publish_all(
            "wrapped_push_then_frame",
            n,
            appended,
            "only the pushed line is wrapped; the cached prefix is kept",
        );

        // Mutation proof: a full rewrap is exactly what `set_lines` does, so
        // the same one-line change made that way reads n + 1 source rows and
        // must fail the append contract above.
        let mut rewrapped = wrapped(n);
        paint(&mut rewrapped);
        let (_, full) = measure(|| {
            let mut all = lines(n);
            all.push(ADDED.to_owned());
            rewrapped.set_lines(all);
            paint(&mut rewrapped)
        });
        assert_eq!(full.get(Metric::SourceRowsRead), nn + 1);
        let failed = std::panic::catch_unwind(|| {
            assert_exact(&full, "full rewrap", &APPEND_THEN_FRAME);
        });
        assert!(
            failed.is_err(),
            "a full rewrap must fail the incremental append contract at n = {n}"
        );
    }
}
