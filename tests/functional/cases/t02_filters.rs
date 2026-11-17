//! TablePro organization filters, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_filters/` and the dotted checkpoint ids use underscores.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{App, Launch, find_all, flash_settle, text, try_launch};

const WIDE_ID: &str = "pre_refactor/t02_filters/t02_filters_filters_120x40";
const NARROW_ID: &str = "pre_refactor/t02_filters/t02_filters_filters_narrow";

const ORGS: &[&str] = &[
    "Northwind Labs",
    "Lumen Retail",
    "Halden & Co",
    "Bluefin Logistics",
    "Orbit Studio",
    "Kestrel Health",
    "Fjord Analytics",
    "Tessellate",
    "Juniper Home",
    "Meridian Foods",
];

const CHIP_CONTAINS: &str = "name contains 'Labs'";
const CHIP_RETAIL: &str = "name contains 'Retail'";
const CHIP_NULL: &str = "name is NULL";
const CHIP_BETWEEN: &str = "name between Lumen and Orbit";
const MATCH_ALL: &str = "match all ▾";
const MATCH_ANY: &str = "match any ▾";
const EMPTY_ROWS: &str = "Loading rows…";
const NO_VALUE: &str = "No value needed for this operator";
const CLEARED: &str = "Filters cleared";
const REMOVED: &str = "Filter removed";

const NORTHWIND: &str = "Northwind Labs";
const TESSELLATE: &str = "Tessellate";
const JUNIPER: &str = "Juniper Home";
const LUMEN: &str = "Lumen Retail";
const BLUEFIN: &str = "Bluefin Logistics";
const MERIDIAN: &str = "Meridian Foods";
const ORBIT: &str = "Orbit Studio";

#[derive(Clone, Copy)]
enum Rows {
    Empty,
    Same,
    Kept {
        allow: &'static [&'static str],
        show: &'static [&'static str],
    },
}

struct Op {
    downs: usize,
    values: &'static [&'static str],
    preview: &'static str,
    chip: &'static str,
    rows: Rows,
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, tablepro());
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn click_list(app: &App, name: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, name)
        .into_iter()
        .filter(|point| point.x < 40)
        .collect();
    assert_eq!(hits.len(), 1, "list {name}\n{}", text(&screen));
    app.click(hits[0]);
}

fn shot(app: &App, id: &str, precondition: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let outcome = app.check_visual_with_precondition(&root, id, precondition);
    assert!(outcome.is_pass(), "visual mismatch for {id}: {outcome:?}");
}

fn uuid_row(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes
        .windows(9)
        .any(|window| window[..8].iter().all(|byte| byte.is_ascii_hexdigit()) && window[8] == b'-')
}

fn org_on(line: &str) -> Option<&'static str> {
    ORGS.iter().copied().find(|name| line.contains(name))
}

fn org_rows(screen: &TerminalSnapshot) -> Vec<&'static str> {
    text(screen)
        .lines()
        .filter(|line| uuid_row(line))
        .map(|line| org_on(line).unwrap_or_else(|| panic!("row without an organization\n{line}")))
        .collect()
}

fn focused_org(screen: &TerminalSnapshot) -> &'static str {
    let hits: Vec<_> = text(screen)
        .lines()
        .filter(|line| line.contains('▎') && uuid_row(line))
        .map(|line| {
            org_on(line).unwrap_or_else(|| panic!("focused row has no organization\n{line}"))
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "focused organization {hits:?}\n{}",
        text(screen)
    );
    hits[0]
}

fn chip_line_focused(screen: &TerminalSnapshot) -> bool {
    text(screen).lines().any(|line| {
        line.contains('▎')
            && (line.contains(MATCH_ALL) || line.contains(MATCH_ANY) || line.contains("Add filter"))
    })
}

fn grid_focused(screen: &TerminalSnapshot) -> bool {
    text(screen)
        .lines()
        .any(|line| line.contains('▎') && uuid_row(line))
}

fn focus_grid(app: &App) {
    for _ in 0..8 {
        if grid_focused(&app.screen()) {
            return;
        }
        app.key("tab");
        app.settle();
    }
    panic!("grid did not take focus\n{}", text(&app.screen()));
}

fn focus_chips(app: &App) {
    for _ in 0..8 {
        if chip_line_focused(&app.screen()) {
            return;
        }
        app.key("shift+tab");
        app.settle();
    }
    panic!("chip bar did not take focus\n{}", text(&app.screen()));
}

fn open_organizations(app: &App) {
    click_list(app, "Development");
    flash_settle(app);
    app.key("enter");
    // Query 1 is drawn during "Loading schema…", before the connected status.
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains("Connected to Development")
            && visible.contains("Query 1")
            && !visible.contains("Loading schema")
    });
    app.key("down");
    app.key("down");
    app.key("enter");
    let grid = app.wait_text(NORTHWIND);
    assert!(
        text(&grid).contains("public › organizations"),
        "organizations grid\n{}",
        text(&grid)
    );
    app.key("right");
}

fn choose_op(app: &App, downs: usize) {
    app.key("shift+tab");
    app.key("enter");
    for _ in 0..downs {
        app.key("down");
    }
    app.key("enter");
}

fn apply_editor(app: &App, downs: usize, values: &[&str], preview: &str) {
    app.key("/");
    app.wait_text("WHERE ");
    choose_op(app, downs);
    if values.is_empty() {
        let editor = app.wait_text(NO_VALUE);
        let visible = text(&editor);
        assert!(
            visible.contains(preview),
            "missing WHERE {preview}\n{visible}"
        );
        app.key("tab");
        app.key("tab");
        app.key("enter");
        return;
    }
    app.key("tab");
    app.type_text(values[0]);
    if let Some(second) = values.get(1) {
        app.key("tab");
        app.type_text(second);
    }
    let editor = app.wait_text(preview);
    assert!(
        text(&editor).contains("WHERE "),
        "WHERE preview closed\n{}",
        text(&editor)
    );
    app.key("enter");
}

fn wait_chip(app: &App, chip: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("chip {chip}"), |screen| {
        let visible = text(screen);
        visible.contains(chip) && !visible.contains("WHERE ") && !visible.contains(NO_VALUE)
    })
}

fn assert_kept(screen: &TerminalSnapshot, allow: &[&str], show: &[&str]) {
    let rows = org_rows(screen);
    let visible = text(screen);
    assert!(!rows.is_empty(), "filter removed every row\n{visible}");
    for name in &rows {
        assert!(
            allow.contains(name),
            "{name} is outside {allow:?}\n{rows:?}\n{visible}"
        );
    }
    for name in show {
        assert!(
            rows.contains(name),
            "missing {name} from {rows:?}\n{visible}"
        );
    }
}

fn same_rows(screen: &TerminalSnapshot, baseline: &[&str]) -> bool {
    let rows = org_rows(screen);
    !rows.is_empty() && rows.len() <= baseline.len() && &baseline[..rows.len()] == rows.as_slice()
}

fn rows_ready(screen: &TerminalSnapshot, rows: Rows, baseline: &[&str]) -> bool {
    let got = org_rows(screen);
    let visible = text(screen);
    match rows {
        Rows::Empty => {
            got.is_empty()
                && visible.contains(EMPTY_ROWS)
                && !visible.contains(NORTHWIND)
                && !visible.contains(TESSELLATE)
        }
        Rows::Same => same_rows(screen, baseline),
        Rows::Kept { allow, show } => {
            !got.is_empty()
                && got.iter().all(|name| allow.contains(name))
                && show.iter().all(|name| got.contains(name))
        }
    }
}

/// The chip paints before the grid finishes the operator. The last rows of
/// `name is not NULL` were still the previous order on that first frame.
fn wait_op(app: &App, op: &Op, baseline: &[&str], before_same: &str) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let screen = app.screen();
        let visible = text(&screen);
        let chip_up = visible.contains(op.chip)
            && !visible.contains("WHERE ")
            && !visible.contains(NO_VALUE)
            && (op.chip != CHIP_NULL || !visible.contains("is not NULL"));
        let focused = text(&screen)
            .lines()
            .filter(|line| line.contains('▎') && uuid_row(line))
            .filter_map(org_on)
            .collect::<Vec<_>>();
        let focus_ok = !matches!(op.rows, Rows::Same) || focused.first() == Some(&before_same);
        if chip_up && focus_ok && focused.len() <= 1 && rows_ready(&screen, op.rows, baseline) {
            return screen;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "filter {} did not settle\n{visible}",
            op.chip
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_same(screen: &TerminalSnapshot, baseline: &[&str]) {
    let rows = org_rows(screen);
    let visible = text(screen);
    assert!(!rows.is_empty(), "unchanged filter drew no rows\n{visible}");
    assert!(
        rows.len() <= baseline.len(),
        "filtered view is taller than the unfiltered grid\n{rows:?}"
    );
    assert_eq!(
        &baseline[..rows.len()],
        rows.as_slice(),
        "identities moved\n{visible}"
    );
}

fn assert_empty(screen: &TerminalSnapshot) {
    let visible = text(screen);
    let rows = org_rows(screen);
    assert!(
        rows.is_empty(),
        "empty filter still drew {rows:?}\n{visible}"
    );
    assert!(
        !visible.contains(NORTHWIND),
        "empty filter still shows {NORTHWIND}\n{visible}"
    );
    assert!(
        !visible.contains(TESSELLATE),
        "empty filter still shows {TESSELLATE}\n{visible}"
    );
    assert!(
        visible.contains(EMPTY_ROWS),
        "empty grid sentence\n{visible}"
    );
}

fn assert_rows(screen: &TerminalSnapshot, rows: Rows, baseline: &[&str]) {
    match rows {
        Rows::Empty => assert_empty(screen),
        Rows::Same => assert_same(screen, baseline),
        Rows::Kept { allow, show } => assert_kept(screen, allow, show),
    }
}

fn clear_filters(app: &App) {
    let on_chips = chip_line_focused(&app.screen());
    if on_chips {
        app.key("shift+x");
    } else {
        app.key("shift+f");
    }
    let cleared = app.wait_screen("filters cleared", |screen| {
        !text(screen).contains("filtered (") && org_rows(screen).first() == Some(&TESSELLATE)
    });
    if !on_chips {
        assert!(
            text(&cleared).contains(CLEARED),
            "clear status\n{}",
            text(&cleared)
        );
    }
}

fn apply_filter(app: &App, op: &Op) -> TerminalSnapshot {
    focus_grid(app);
    apply_editor(app, op.downs, op.values, op.preview);
    wait_chip(app, op.chip)
}

const OPS: &[Op] = &[
    Op {
        downs: 0,
        values: &["Tessellate"],
        preview: "name = 'Tessellate'",
        chip: "name = 'Tessellate'",
        rows: Rows::Kept {
            allow: &[TESSELLATE],
            show: &[TESSELLATE],
        },
    },
    Op {
        downs: 1,
        values: &["Tessellate"],
        preview: "name != 'Tessellate'",
        chip: "name != 'Tessellate'",
        rows: Rows::Kept {
            allow: &[
                NORTHWIND,
                LUMEN,
                "Halden & Co",
                BLUEFIN,
                ORBIT,
                "Kestrel Health",
                "Fjord Analytics",
                JUNIPER,
                MERIDIAN,
            ],
            show: &[NORTHWIND],
        },
    },
    Op {
        downs: 2,
        values: &["Labs"],
        preview: "name ILIKE '%Labs%' ESCAPE '\\'",
        chip: CHIP_CONTAINS,
        rows: Rows::Kept {
            allow: &[NORTHWIND],
            show: &[NORTHWIND],
        },
    },
    Op {
        downs: 3,
        values: &["Jun"],
        preview: "name ILIKE 'Jun%' ESCAPE '\\'",
        chip: "name starts with 'Jun'",
        rows: Rows::Kept {
            allow: &[JUNIPER],
            show: &[JUNIPER],
        },
    },
    Op {
        downs: 4,
        values: &["Labs"],
        preview: "name ILIKE '%Labs' ESCAPE '\\'",
        chip: "name ends with 'Labs'",
        rows: Rows::Kept {
            allow: &[NORTHWIND],
            show: &[NORTHWIND],
        },
    },
    Op {
        downs: 5,
        values: &[],
        preview: "name IS NULL",
        chip: CHIP_NULL,
        rows: Rows::Empty,
    },
    Op {
        downs: 6,
        values: &[],
        preview: "name IS NOT NULL",
        chip: "name is not NULL",
        rows: Rows::Same,
    },
    Op {
        downs: 7,
        values: &[],
        preview: "name = ''",
        chip: "name is empty",
        rows: Rows::Empty,
    },
    Op {
        downs: 8,
        values: &["Labs"],
        preview: "name NOT ILIKE '%Labs%' ESCAPE '\\'",
        chip: "name not contains 'Labs'",
        rows: Rows::Kept {
            allow: &[
                LUMEN,
                "Halden & Co",
                BLUEFIN,
                ORBIT,
                "Kestrel Health",
                "Fjord Analytics",
                TESSELLATE,
                JUNIPER,
                MERIDIAN,
            ],
            show: &[TESSELLATE, JUNIPER],
        },
    },
    Op {
        downs: 9,
        values: &["Meridian"],
        preview: "name > 'Meridian'",
        chip: "name > 'Meridian'",
        rows: Rows::Kept {
            allow: &[MERIDIAN, NORTHWIND, ORBIT, TESSELLATE],
            show: &[NORTHWIND, MERIDIAN],
        },
    },
    Op {
        downs: 10,
        values: &["Northwind Labs"],
        preview: "name >= 'Northwind Labs'",
        chip: "name >= 'Northwind Labs'",
        rows: Rows::Kept {
            allow: &[NORTHWIND, ORBIT, TESSELLATE],
            show: &[NORTHWIND],
        },
    },
    Op {
        downs: 11,
        values: &["Fjord"],
        preview: "name < 'Fjord'",
        chip: "name < 'Fjord'",
        rows: Rows::Kept {
            allow: &[BLUEFIN],
            show: &[BLUEFIN],
        },
    },
    Op {
        downs: 12,
        values: &["Bluefin Logistics"],
        preview: "name <= 'Bluefin Logistics'",
        chip: "name <= 'Bluefin Logistics'",
        rows: Rows::Kept {
            allow: &[BLUEFIN],
            show: &[BLUEFIN],
        },
    },
    Op {
        downs: 13,
        values: &[],
        preview: "name != ''",
        chip: "name is not empty",
        rows: Rows::Same,
    },
    Op {
        downs: 14,
        values: &["Tessellate, Lumen Retail"],
        preview: "name IN ('Tessellate', 'Lumen Retail')",
        chip: "name in list 'Tessellate, Lumen Retail'",
        rows: Rows::Kept {
            allow: &[TESSELLATE, LUMEN],
            show: &[TESSELLATE],
        },
    },
    Op {
        downs: 15,
        values: &["Tessellate"],
        preview: "name NOT IN ('Tessellate')",
        chip: "name not in list 'Tessellate'",
        rows: Rows::Kept {
            allow: &[
                NORTHWIND,
                LUMEN,
                "Halden & Co",
                BLUEFIN,
                ORBIT,
                "Kestrel Health",
                "Fjord Analytics",
                JUNIPER,
                MERIDIAN,
            ],
            show: &[NORTHWIND, JUNIPER],
        },
    },
    Op {
        downs: 16,
        values: &["Lumen", "Orbit"],
        preview: "name >= 'Lumen' AND name <= 'Orbit'",
        chip: CHIP_BETWEEN,
        rows: Rows::Kept {
            allow: &[LUMEN, MERIDIAN, NORTHWIND],
            show: &[NORTHWIND],
        },
    },
    Op {
        downs: 17,
        values: &["Labs"],
        preview: "name ~ 'Labs'",
        chip: "name matches regex 'Labs'",
        rows: Rows::Kept {
            allow: &[NORTHWIND],
            show: &[NORTHWIND],
        },
    },
];

#[test]
fn tablepro_filters_route_closes_the_organizations_tab() {
    let app = launch("t02-filters-route");
    open_organizations(&app);
    let open = app.screen();
    assert!(
        text(&open).contains(TESSELLATE) && text(&open).contains("slug"),
        "organizations rows\n{}",
        text(&open)
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query tab", |screen| {
        let visible = text(screen);
        visible.contains("Query 1")
            && !visible.contains("public › organizations")
            && !visible.contains(NORTHWIND)
            && !visible.contains(TESSELLATE)
            && !visible.contains(MATCH_ALL)
    });
    assert!(
        text(&back).contains("organizations"),
        "explorer lost the table name\n{}",
        text(&back)
    );
    assert_eq!((back.cols(), back.rows()), (120, 40));
    app.finish();
}

#[test]
fn tablepro_filters_change_the_visible_organizations() {
    let app = launch("t02-filters");
    open_organizations(&app);
    let open = app.wait_screen("unfiltered organizations", |screen| {
        org_rows(screen).len() > 10 && text(screen).contains(TESSELLATE)
    });
    let baseline = org_rows(&open);
    assert_eq!(focused_org(&open), TESSELLATE, "first row\n{}", text(&open));
    assert!(
        baseline.contains(&NORTHWIND),
        "unfiltered names {baseline:?}"
    );

    app.key("down");
    let parked = app.wait_screen("cursor on Northwind", |screen| {
        grid_focused(screen) && focused_org(screen) == NORTHWIND
    });
    assert_eq!(focused_org(&parked), NORTHWIND);

    let contains = Op {
        downs: 2,
        values: &["Labs"],
        preview: "name ILIKE '%Labs%' ESCAPE '\\'",
        chip: CHIP_CONTAINS,
        rows: Rows::Kept {
            allow: &[NORTHWIND],
            show: &[NORTHWIND],
        },
    };
    let filtered = apply_filter(&app, &contains);
    assert_eq!(
        focused_org(&filtered),
        NORTHWIND,
        "selection left Northwind\n{}",
        text(&filtered)
    );
    assert_rows(&filtered, contains.rows, &baseline);
    assert!(
        !text(&filtered).contains(TESSELLATE),
        "contains Labs kept Tessellate\n{}",
        text(&filtered)
    );
    assert!(
        text(&filtered).contains(MATCH_ALL),
        "match-all lead\n{}",
        text(&filtered)
    );

    app.expire_status();
    app.wait_absent("filter applied");
    flash_settle(&app);
    shot(
        &app,
        WIDE_ID,
        "chip name contains 'Labs' on organizations at 120 by 40",
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(CHIP_CONTAINS)
    });
    assert!(
        text(&narrow).contains(NORTHWIND) || text(&narrow).contains("Northwind"),
        "narrow filter dropped the kept name\n{}",
        text(&narrow)
    );
    flash_settle(&app);
    shot(
        &app,
        NARROW_ID,
        "chip name contains 'Labs' on organizations at 80 by 24",
    );
    app.resize(120, 40);
    app.wait_screen("restored 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains("120×40")
    });

    focus_chips(&app);
    app.key("enter");
    app.wait_text("Edit filter");
    app.key("shift+tab");
    app.key("shift+tab");
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text("Retail");
    app.key("enter");
    let edited = wait_chip(&app, CHIP_RETAIL);
    assert_kept(&edited, &[LUMEN], &[LUMEN]);
    assert!(
        !text(&edited).contains(NORTHWIND),
        "edited filter kept Northwind\n{}",
        text(&edited)
    );

    focus_chips(&app);
    app.key("x");
    let removed = app.wait_text(REMOVED);
    assert!(
        text(&removed).contains(TESSELLATE) && text(&removed).contains(NORTHWIND),
        "removing the chip did not restore the dropped names\n{}",
        text(&removed)
    );
    assert!(
        !text(&removed).contains(CHIP_RETAIL),
        "removed chip stayed\n{}",
        text(&removed)
    );

    focus_grid(&app);
    let kept = focused_org(&app.screen());
    app.key("f");
    let cell = app.wait_text(&format!("name = '{kept}'"));
    assert!(
        text(&cell).contains("WHERE "),
        "filter on cell did not open the editor\n{}",
        text(&cell)
    );
    app.key("enter");
    let cell_chip = format!("name = '{kept}'");
    let cell_rows = wait_chip(&app, &cell_chip);
    assert_eq!(focused_org(&cell_rows), kept);
    assert_kept(&cell_rows, &[kept], &[kept]);

    clear_filters(&app);
    let restored = app.wait_screen("reset names", |screen| {
        same_rows(screen, &baseline) && text(screen).contains(NORTHWIND)
    });
    assert_same(&restored, &baseline);

    focus_grid(&app);
    app.key("/");
    app.wait_text("WHERE ");
    app.type_text("zzz");
    app.key("escape");
    app.key("escape");
    let cancelled = app.wait_screen("filter cancelled", |screen| {
        let visible = text(screen);
        !visible.contains("WHERE ") && !visible.contains("zzz") && visible.contains(TESSELLATE)
    });
    assert!(
        !text(&cancelled).contains(MATCH_ALL),
        "cancel added a chip\n{}",
        text(&cancelled)
    );

    let labs = apply_filter(&app, &contains);
    assert_kept(&labs, &[NORTHWIND], &[NORTHWIND]);
    focus_chips(&app);
    app.key("+");
    app.wait_text("WHERE ");
    choose_op(&app, 17);
    app.key("tab");
    app.type_text("(");
    app.key("enter");
    let rejected = app.wait_text("invalid regular expression");
    let shown = text(&rejected);
    assert!(
        shown.contains("WHERE ") && shown.contains(CHIP_CONTAINS),
        "editor closed on an invalid regex\n{shown}"
    );
    assert!(
        !shown.contains("matches regex '('"),
        "an invalid regex became a chip\n{shown}"
    );
    app.key("escape");
    app.key("escape");
    let kept = app.wait_screen("rejected filter cancelled", |screen| {
        let visible = text(screen);
        !visible.contains("WHERE ") && visible.contains(CHIP_CONTAINS)
    });
    assert!(
        !text(&kept).contains("matches regex '('"),
        "cancelling kept the invalid chip\n{}",
        text(&kept)
    );
    assert_kept(&kept, &[NORTHWIND], &[NORTHWIND]);
    focus_chips(&app);
    app.key("+");
    app.wait_text("WHERE ");
    choose_op(&app, 2);
    app.key("tab");
    app.type_text("Retail");
    let both_preview = app.wait_text("name ILIKE '%Retail%' ESCAPE '\\'");
    assert!(text(&both_preview).contains("WHERE "));
    app.key("enter");
    let both = app.wait_screen("two chips", |screen| {
        let visible = text(screen);
        visible.contains(CHIP_CONTAINS)
            && visible.contains(CHIP_RETAIL)
            && visible.contains(MATCH_ALL)
            && !visible.contains("WHERE ")
    });
    assert_empty(&both);

    focus_chips(&app);
    app.key("m");
    let any = app.wait_text(MATCH_ANY);
    assert!(
        text(&any).contains(CHIP_CONTAINS) && text(&any).contains(CHIP_RETAIL),
        "match any dropped a chip\n{}",
        text(&any)
    );
    assert!(
        !text(&any).contains(MATCH_ALL),
        "match all stayed after m\n{}",
        text(&any)
    );
    assert_kept(&any, &[NORTHWIND, LUMEN], &[NORTHWIND, LUMEN]);
    clear_filters(&app);
    focus_grid(&app);
    let before_same = focused_org(&app.screen());
    for op in OPS {
        apply_filter(&app, op);
        let screen = wait_op(&app, op, &baseline, before_same);
        assert!(
            text(&screen).contains(op.chip),
            "chip {}\n{}",
            op.chip,
            text(&screen)
        );
        if op.chip == CHIP_NULL {
            assert!(
                !text(&screen).contains("is not NULL"),
                "NULL chip collided\n{}",
                text(&screen)
            );
        }
        assert_rows(&screen, op.rows, &baseline);
        if matches!(op.rows, Rows::Same) {
            assert_eq!(
                focused_org(&screen),
                before_same,
                "selection moved under {}\n{}",
                op.chip,
                text(&screen)
            );
        }
        clear_filters(&app);
    }

    let reset = app.wait_screen("operators reset", |screen| {
        same_rows(screen, &baseline)
            && text(screen).contains(NORTHWIND)
            && !text(screen).contains(MATCH_ALL)
    });
    assert_same(&reset, &baseline);
    assert!(
        !text(&reset).contains(MATCH_ALL),
        "a chip survived the last reset\n{}",
        text(&reset)
    );
    app.finish();
}
