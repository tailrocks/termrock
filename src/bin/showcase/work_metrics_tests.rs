//! Count-only work scenarios for Showcase pages (#56).
//!
//! Each operation runs through the real app: `shell::handle` for the tick or
//! pointer event, then `shell::render` into a 140x44 test terminal. Counts are
//! for the test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p junie-tui --features work-metrics --bin showcase \
//!     --no-tests fail -E 'test(/^work_metrics_tests::/)'
//! ```

use junie_tui::core::event::{Input, Mouse, MouseKind};
use junie_tui::metrics::harness::{Harness, assert_exact};
use junie_tui::metrics::{Metric, measure};
use junie_tui::motion::Motion;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::layout::Position;

use crate::app::{App, PageId};

const VIEWPORT: (u16, u16) = (140, 44);

fn fixture(page: PageId) -> Harness<App> {
    let mut app = App::with_motion(Theme::for_level(ColorLevel::TrueColor), Motion::Paused, 0);
    app.open_page(page);
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(30);
    f
}

fn click_text(f: &mut Harness<App>, text: &str) {
    let screen = f.screen();
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{screen}"));
    let x = line[..line.find(text).expect("found")].chars().count();
    for kind in [MouseKind::Down, MouseKind::Up] {
        f.send(Input::Mouse(Mouse::new(
            kind,
            Position::new(x as u16 + 1, y as u16),
        )));
    }
}

#[test]
fn work_contract_showcase_idle_and_switch() {
    let mut f = fixture(PageId::QueryWorkbench);
    let before = f.screen();
    assert!(before.contains("Query 1") && before.contains("customers"));
    assert!(
        before.contains("FROM orders"),
        "Query 1 is the active document"
    );

    // Rows painted is summed over every list, table, grid, tree, option list
    // and editor widget in the frame. The page has two documents; each frame
    // also paints the Explorer tree. The editor reads cached text and
    // annotation results, so a settled frame copies and hashes nothing.
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    assert_exact(
        &idle,
        "query workbench settled tick",
        &[
            (Metric::VisibleRowsPainted, 54),
            (Metric::IntrinsicWidthCalls, 222),
            (Metric::TextBytesMeasured, 1_448),
            (Metric::FitVisits, 13),
        ],
    );
    f.report(
        "showcase_query_workbench",
        "settled_tick",
        2,
        idle,
        "two documents in the strip and the frame text is unchanged by the tick",
    );

    // Pointer down and up on the second tab, each followed by a frame.
    let ((), switch) = measure(|| click_text(&mut f, "customers"));
    let after = f.screen();
    assert!(
        after.contains("FROM customers"),
        "the customers document is active"
    );
    assert!(!after.contains("FROM orders"));
    assert_exact(
        &switch,
        "document switch (two frames)",
        &[
            (Metric::VisibleRowsPainted, 91),
            (Metric::IntrinsicWidthCalls, 382),
            (Metric::TextBytesMeasured, 2_533),
            (Metric::AnnotationQueries, 1),
            (Metric::FitVisits, 24),
        ],
    );
    f.report(
        "showcase_query_workbench",
        "switch_document",
        2,
        switch,
        "the editor shows the customers query after the click",
    );

    let mut g = fixture(PageId::Finder);
    let before = g.screen();
    let ((), finder) = measure(|| g.tick());
    assert_eq!(
        g.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    // The Finder runs on the shared SearchDetail (#257), whose detail card no
    // longer sizes itself in a scratch buffer, and its master rows paint
    // through the clipping grapheme painter, which measures each cell run.
    assert_exact(
        &finder,
        "finder settled tick",
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::VisibleRowsPainted, 21),
            (Metric::IntrinsicWidthCalls, 422),
            (Metric::TextBytesMeasured, 1_135),
            (Metric::FitVisits, 5),
        ],
    );
    f.report(
        "showcase_finder",
        "settled_tick",
        crate::pages::finder::ACTION_COUNT,
        finder,
        "the frame text is unchanged by the tick",
    );

    // An option of another page changes while the Finder is on screen.
    let set = crate::options::parse_opt("queryworkbench.read_only=on").expect("a valid option");
    g.app.apply_opt(set);
    let ((), unrelated) = measure(|| g.tick());
    assert_eq!(
        g.screen(),
        before,
        "the Query workbench option is not on the Finder page"
    );
    assert_exact(
        &unrelated,
        "unrelated page option then tick",
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::VisibleRowsPainted, 21),
            (Metric::IntrinsicWidthCalls, 422),
            (Metric::TextBytesMeasured, 1_135),
            (Metric::FitVisits, 5),
        ],
    );
    f.report(
        "showcase_finder",
        "irrelevant_option_change_then_tick",
        crate::pages::finder::ACTION_COUNT,
        unrelated,
        "the frame text is unchanged by another page's option",
    );
}

/// One `Stress data` value on a page.
struct Stress {
    page: PageId,
    key: &'static str,
    value: &'static str,
    operation: &'static str,
    records: usize,
    /// Text the frame must show once the data is set, so a count cannot pass
    /// on a page that never rendered it.
    shows: &'static str,
}

/// Checks the work of two frames. `change` is the frame that applies the
/// option (option set, then a tick) and `settled` is the next tick with
/// nothing changed. Fitting is stateless, so the settled frame repeats the
/// layout.
fn stress_case(case: &Stress, change: &[(Metric, u64)], settled: &[(Metric, u64)]) {
    let Stress {
        page,
        key,
        value,
        operation,
        records,
        shows,
    } = *case;
    let mut f = fixture(page);
    assert!(
        !f.screen().contains(shows),
        "{key}=off already shows {shows:?}"
    );
    let set = crate::options::parse_opt(&format!("{key}={value}")).expect("a valid option");
    let ((), applied) = measure(|| {
        f.app.apply_opt(set);
        f.tick();
    });
    let screen = f.screen();
    assert!(
        screen.contains(shows),
        "{key}={value} does not show {shows:?}:\n{screen}"
    );
    assert_exact(&applied, &format!("{key}={value} option frame"), change);
    let ((), repeat) = measure(|| f.tick());
    assert!(f.screen().contains(shows), "{key}={value} settled frame");
    assert_exact(&repeat, &format!("{key}={value} settled frame"), settled);
    f.report(
        "showcase_stress_data",
        operation,
        records,
        repeat,
        &format!("the frame shows {shows:?}"),
    );
}

// The four `Stress data` options at 20,000 items (N) and at one 65,536-cell
// label, through the real page. Each frame counts every fitter on the page, so
// the totals include the demo bars the page also draws. The per-frame split
// below is arithmetic on the exact counts, not a separate measurement.
//
// - Toolbars: 20,446 widths is one pass over the N items plus the page's own
//   bars. The 20 visits show the pack loop stops at the first item that does
//   not fit instead of walking all N.
// - Breadcrumbs: 60,502 widths is 3N plus 502 and 19,998 visits is N. The page
//   plans the trail three times a frame (measure, geometry, paint), one width
//   per label each time. The trail is cut to its card, so the page fits the
//   viewport and paints straight into the frame: no off-screen buffer cells.
// - Chrome: 120,895 widths is 6N plus 895, 240,083 visits is 12N plus 83 and
//   1,825,650 compares is 6 sorts of about 1.01 N ceil(log2 N). The status bar
//   is therefore planned six times a frame, each plan linear in N plus one sort
//   (inferred from the arithmetic).
// - Chips: 80,627 widths is 4N plus 627 and 40,025 visits is 2N plus 25. The
//   28 compares equal the stress-off frame: the chip fitter never sorts. The
//   bar is cut to its card, so the page fits the viewport width and
//   `ScrollView::fit` measures it a second time to reserve the scrollbar
//   column. That is a fourth pass over the bar (the page measures at three
//   widths and then renders), one more N than before the clamp left room for
//   the card insets. The card now holds four filter chips and `+19996`.
//   In the label scene the 65,536-cell chip comes last, so the bar shows the
//   four demo filters and `+1`: 643 widths, 266,497 bytes and 35 visits.
//
// A fitter that rescans survivors per removal would add a factor of N to the
// visits or the widths and fail these exact counts.

#[test]
fn work_contract_stress_toolbars_items() {
    stress_case(
        &Stress {
            page: PageId::Toolbars,
            key: "toolbars.stress",
            value: "20k items",
            operation: "toolbars_items_settled",
            records: crate::pages::stress::COUNT,
            shows: "and 19991 more",
        },
        &[
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 20_446),
            (Metric::TextBytesMeasured, 242_551),
            (Metric::FitVisits, 20),
        ],
        &[
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 20_540),
            (Metric::TextBytesMeasured, 242_797),
            (Metric::FitVisits, 20),
        ],
    );
}

#[test]
fn work_contract_stress_toolbars_label() {
    stress_case(
        &Stress {
            page: PageId::Toolbars,
            key: "toolbars.stress",
            value: "65k label",
            operation: "toolbars_label_settled",
            records: 1,
            shows: "long label long label long l…",
        },
        &[
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 498),
            (Metric::TextBytesMeasured, 68_385),
            (Metric::FitVisits, 19),
        ],
        &[
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 678),
            (Metric::TextBytesMeasured, 69_145),
            (Metric::FitVisits, 19),
        ],
    );
}

#[test]
fn work_contract_stress_breadcrumbs_items() {
    stress_case(
        &Stress {
            page: PageId::BreadcrumbsRails,
            key: "breadcrumbsrails.stress",
            value: "20k items",
            operation: "breadcrumbs_items_settled",
            records: crate::pages::stress::COUNT,
            shows: "Folder 20000",
        },
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::BufferCellsFilled, 0),
            (Metric::BufferCellsCopied, 0),
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 60_502),
            (Metric::TextBytesMeasured, 722_753),
            (Metric::FitVisits, 19_998),
        ],
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::BufferCellsFilled, 0),
            (Metric::BufferCellsCopied, 0),
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 60_502),
            (Metric::TextBytesMeasured, 722_753),
            (Metric::FitVisits, 19_998),
        ],
    );
}

#[test]
fn work_contract_stress_breadcrumbs_label() {
    stress_case(
        &Stress {
            page: PageId::BreadcrumbsRails,
            key: "breadcrumbsrails.stress",
            value: "65k label",
            operation: "breadcrumbs_label_settled",
            records: 1,
            shows: "… › long label long label",
        },
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::BufferCellsFilled, 0),
            (Metric::BufferCellsCopied, 0),
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 616),
            (Metric::TextBytesMeasured, 200_064),
            (Metric::FitVisits, 10),
        ],
        &[
            (Metric::AllocatedBufferCells, 0),
            (Metric::BufferCellsFilled, 0),
            (Metric::BufferCellsCopied, 0),
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 616),
            (Metric::TextBytesMeasured, 200_064),
            (Metric::FitVisits, 10),
        ],
    );
}

#[test]
fn work_contract_stress_chrome_items() {
    stress_case(
        &Stress {
            page: PageId::Chrome,
            key: "chrome.stress",
            value: "20k items",
            operation: "chrome_items_settled",
            records: crate::pages::stress::COUNT,
            shows: "job 00008",
        },
        &[
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 120_895),
            (Metric::TextBytesMeasured, 1_084_574),
            (Metric::FitVisits, 240_083),
            (Metric::FitCompares, 1_825_650),
        ],
        &[
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 120_895),
            (Metric::TextBytesMeasured, 1_084_574),
            (Metric::FitVisits, 240_083),
            (Metric::FitCompares, 1_825_650),
        ],
    );
}

#[test]
fn work_contract_stress_chrome_label() {
    stress_case(
        &Stress {
            page: PageId::Chrome,
            key: "chrome.stress",
            value: "65k label",
            operation: "chrome_label_settled",
            records: 1,
            shows: "long label long label long label",
        },
        &[
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 1_349),
            (Metric::TextBytesMeasured, 401_772),
            (Metric::FitVisits, 124),
            (Metric::FitCompares, 216),
        ],
        &[
            (Metric::VisibleRowsPainted, 20),
            (Metric::IntrinsicWidthCalls, 1_349),
            (Metric::TextBytesMeasured, 401_772),
            (Metric::FitVisits, 124),
            (Metric::FitCompares, 216),
        ],
    );
}

#[test]
fn work_contract_stress_chips_items() {
    stress_case(
        &Stress {
            page: PageId::Chips,
            key: "chipsselects.stress",
            value: "20k items",
            operation: "chips_items_settled",
            records: crate::pages::stress::COUNT,
            shows: "+19996",
        },
        &[
            (Metric::AllocatedBufferCells, 6_440),
            (Metric::BufferCellsFilled, 4_368),
            (Metric::BufferCellsCopied, 12_096),
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 80_627),
            (Metric::TextBytesMeasured, 964_127),
            (Metric::FitVisits, 40_025),
            (Metric::FitCompares, 28),
        ],
        &[
            (Metric::AllocatedBufferCells, 6_440),
            (Metric::BufferCellsFilled, 4_368),
            (Metric::BufferCellsCopied, 12_096),
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 80_627),
            (Metric::TextBytesMeasured, 964_127),
            (Metric::FitVisits, 40_025),
            (Metric::FitCompares, 28),
        ],
    );
}

#[test]
fn work_contract_stress_chips_label() {
    stress_case(
        &Stress {
            page: PageId::Chips,
            key: "chipsselects.stress",
            value: "65k label",
            operation: "chips_label_settled",
            records: 1,
            shows: "'abc' ×   +1",
        },
        &[
            (Metric::AllocatedBufferCells, 6_440),
            (Metric::BufferCellsFilled, 4_368),
            (Metric::BufferCellsCopied, 12_096),
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 643),
            (Metric::TextBytesMeasured, 266_497),
            (Metric::FitVisits, 35),
            (Metric::FitCompares, 28),
        ],
        &[
            (Metric::AllocatedBufferCells, 6_440),
            (Metric::BufferCellsFilled, 4_368),
            (Metric::BufferCellsCopied, 12_096),
            (Metric::VisibleRowsPainted, 17),
            (Metric::IntrinsicWidthCalls, 643),
            (Metric::TextBytesMeasured, 266_497),
            (Metric::FitVisits, 35),
            (Metric::FitCompares, 28),
        ],
    );
}
