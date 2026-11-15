//! Holla Why, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h01_why/` and each checkpoint's dots become underscores.
//! `--frame 40` is only the pre-tick loop in `App::launch`. The test does not
//! advance the clock.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, cell_bg, find_all, row_text, text, try_launch};

const ROW_A: &str = "Analyze disk usage";
const ROW_B: &str = "Open btm";
const WHY_ITEM: &str = "Why is this here?";
const TITLE: &str = "Why is “Open btm” here?";
const LIVE_KEY: &str = "live state";
const LIVE: &str = "sustained pressure · continuous view fits";
const RISK: &str = "read-only · runs directly · never changes the rank";
const ORDER: &str = "Ranking order:";
const GATE: &str = "changes the gate, never the rank.";
const FOCUS: &str = "▎";
const WIDE: &str = "H01.why.why.120x40";
const NARROW: &str = "H01.why.why.narrow";
const WIDE_AGE_BEFORE: &str = "· 4 s ago";
const WIDE_AGE: &str = "· 5 s ago";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h01_why/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec![
        "--scenario".to_string(),
        "first-use".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(ROW_A);
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn selected(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let Some(at) = line.find(label) else {
            return false;
        };
        at > mark && at - mark <= 4
    })
}

fn on_row(screen: &TerminalSnapshot, label: &str, other: &str) -> bool {
    selected(screen, label) && !selected(screen, other)
}

fn menu_titled(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(inside) = line.split('│').nth(1) else {
            return false;
        };
        inside.split_whitespace().collect::<Vec<_>>().join(" ") == label
    })
}

fn menu_point(screen: &TerminalSnapshot, label: &str) -> crate::support::Point {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| row_text(screen, u32::from(hit.y)).contains('│'))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "menu hit {label:?}: {hits:?}\n{}",
        text(screen)
    );
    hits[0]
}

/// The alternatives row paints a fill and no `▎`, so the focused item is the
/// one whose background differs from the other rows.
fn why_item_focused(screen: &TerminalSnapshot) -> bool {
    let why = cell_bg(screen, menu_point(screen, WHY_ITEM));
    let run = cell_bg(screen, menu_point(screen, "Run now"));
    let pin = cell_bg(screen, menu_point(screen, "Pin here"));
    why != run && why != pin && run == pin
}

fn title_line(screen: &TerminalSnapshot) -> Option<String> {
    text(screen)
        .lines()
        .find(|line| line.contains("Why is “"))
        .map(str::to_owned)
}

fn why_open(screen: &TerminalSnapshot) -> bool {
    let Some(title) = title_line(screen) else {
        return false;
    };
    let visible = text(screen);
    title.contains(ROW_B)
        && !title.contains(ROW_A)
        && visible.contains(LIVE_KEY)
        && visible.contains(LIVE)
        && visible.contains(RISK)
        && visible.contains(ORDER)
        && visible.contains(GATE)
        && !visible.contains(WHY_ITEM)
}

fn back_on_b(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The title clears before the ranking paragraph leaves the grid.
    !visible.contains("Why is")
        && !visible.contains(ORDER)
        && !visible.contains(WHY_ITEM)
        && !visible.contains("never changes the rank")
        && on_row(screen, ROW_B, ROW_A)
        && visible.contains(ROW_A)
        && visible
            .lines()
            .any(|line| line.contains(FOCUS) && line.contains("Search actions"))
}

fn select_b(app: &App) {
    app.key("home");
    let on_a = app.wait_screen("row A selected", |screen| on_row(screen, ROW_A, ROW_B));
    assert_eq!((on_a.cols(), on_a.rows()), (120, 40));
    see(&on_a, &[ROW_A, ROW_B], &["Why is"]);
    app.key("down");
    let on_b = app.wait_screen("row B selected", |screen| on_row(screen, ROW_B, ROW_A));
    see(&on_b, &[ROW_A, ROW_B], &["Why is"]);
}

fn open_why_for_b(app: &App) {
    select_b(app);
    app.key("alt+enter");
    let menu = app.wait_screen("alternatives for row B", |screen| {
        text(screen).contains(WHY_ITEM) && menu_titled(screen, ROW_B) && !menu_titled(screen, ROW_A)
    });
    see(&menu, &[WHY_ITEM, ROW_B], &[TITLE]);
    app.key("end");
    app.wait_screen("Why item focused", why_item_focused);
    app.key("enter");
    let opened = app.wait_screen("why modal for row B", why_open);
    let title = title_line(&opened).expect("why title");
    assert!(
        title.contains(ROW_B) && !title.contains(ROW_A),
        "title names the wrong item: {title}"
    );
    see(
        &opened,
        &[TITLE, LIVE_KEY, LIVE, RISK, ORDER, GATE],
        &[WHY_ITEM],
    );
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. The size chip is not on screen at 80 columns, so that wait uses the
/// terminal size rather than the text `80×24`.
fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && why_open(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && why_open(screen)
    });
}

/// The preview age beside the modal is in the wide shot. Waiting for the
/// previous second, then this one, starts the capture at the beginning of it.
fn wait_wide_age(app: &App) {
    for age in [WIDE_AGE_BEFORE, WIDE_AGE] {
        app.wait_screen(age, |screen| {
            screen.cols() == 120
                && screen.rows() == 40
                && why_open(screen)
                && text(screen).contains(age)
        });
    }
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn close_why(app: &App) {
    app.key("escape");
    let back = app.wait_screen("why closed on row B", back_on_b);
    see(
        &back,
        &[ROW_A, ROW_B],
        &["Why is", WHY_ITEM, ORDER, "never changes the rank"],
    );
    assert!(
        on_row(&back, ROW_B, ROW_A),
        "focus left row B\n{}",
        text(&back)
    );
}

#[test]
fn holla_why_route_returns_to_the_finder_row() {
    let app = open("h01-why-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[ROW_A, ROW_B, "Search actions and resources"],
        &["Why is"],
    );
    open_why_for_b(&app);
    close_why(&app);
    app.finish();
}

#[test]
fn holla_why_explains_the_selected_item() {
    let app = open("h01-why-why");
    open_why_for_b(&app);
    nudge(&app);
    wait_wide_age(&app);

    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(
        why_open(&wide) && text(&wide).contains(WIDE_AGE),
        "why closed before the wide shot\n{}",
        text(&wide)
    );
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("why at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && why_open(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    nudge(&app);
    // The size-restored wait can return, and the next read can still be the
    // cleared 80x24 grid. Wait until the modal is on that grid again.
    let narrow = app.wait_screen("why still open at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && why_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(
        why_open(&narrow),
        "why closed before the narrow shot\n{}",
        text(&narrow)
    );
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("why back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && why_open(screen)
    });
    assert!(
        why_open(&restored),
        "why closed after restore\n{}",
        text(&restored)
    );
    close_why(&app);
    app.finish();
}
