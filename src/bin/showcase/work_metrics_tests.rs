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
            (Metric::IntrinsicWidthCalls, 229),
            (Metric::TextBytesMeasured, 1_493),
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
            (Metric::IntrinsicWidthCalls, 396),
            (Metric::TextBytesMeasured, 2_623),
            (Metric::AnnotationQueries, 1),
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
    // The Finder page paints a detail card that sizes itself in a scratch
    // buffer on every frame: 1440 cells allocated per frame today.
    assert_exact(
        &finder,
        "finder settled tick",
        &[
            (Metric::AllocatedBufferCells, 1_440),
            (Metric::VisibleRowsPainted, 24),
            (Metric::IntrinsicWidthCalls, 223),
            (Metric::TextBytesMeasured, 1_225),
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
            (Metric::AllocatedBufferCells, 1_440),
            (Metric::VisibleRowsPainted, 24),
            (Metric::IntrinsicWidthCalls, 223),
            (Metric::TextBytesMeasured, 1_225),
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
