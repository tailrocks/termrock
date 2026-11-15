use tui_test::terminal::cell::Attrs;
use tui_test::{SnapshotColor, TerminalSnapshot};

use super::support::{App, Point, SHOWCASE, cell_bg, crop, find, find_all, row_text, text};

fn inside(point: Point) -> Point {
    Point {
        x: point.x + 2,
        ..point
    }
}
fn bg(screen: &TerminalSnapshot, p: Point) -> SnapshotColor {
    screen
        .cell_at(u32::from(p.x), u32::from(p.y))
        .expect("cell in viewport")
        .bg()
}

fn button_lifecycle(color: &str) {
    let app = App::new(
        &format!("showcase-buttons-{color}"),
        SHOWCASE,
        &["--page", "buttons", "--motion", "paused"],
        160,
        50,
        color,
    );
    let initial = app.wait_text("Disabled primary");
    let run = find(&initial, "Run task");
    let preview = find(&initial, "Preview");
    let disabled = find(&initial, "Disabled primary");
    let toggle = find(&initial, "Auto-approve");
    let outside = find(&initial, "Actions");
    let rest = crop(&initial, run, 8, 1);
    assert!(!text(&initial).contains("Run task ✓"));

    app.move_to(inside(run));
    let hovered = app.wait_screen("Run task visibly responds to hover", |screen| {
        crop(screen, run, 8, 1) != rest
    });
    let hover = crop(&hovered, run, 8, 1);
    assert!(!text(&hovered).contains("Run task ✓"));
    app.down(inside(run));
    let pressed = app.wait_screen("held Run task differs from hover and rest", |screen| {
        let current = crop(screen, run, 8, 1);
        current != hover && current != rest
    });
    assert!(
        !text(&pressed).contains("Run task ✓"),
        "press must not activate"
    );
    app.up(inside(run));
    app.wait_text("Run task ✓");

    app.click(inside(preview));
    app.wait_text("Preview ✓");
    app.down(inside(run));
    app.drag_to(outside);
    app.up(outside);
    let cancelled = app.settle();
    assert!(text(&cancelled).contains("Preview ✓"));
    assert!(!text(&cancelled).contains("Run task ✓"));

    app.click(inside(toggle));
    let on = app.wait_text("Auto-approve on");
    assert!(row_text(&on, u32::from(toggle.y)).contains("● Auto-approve"));
    app.click(inside(toggle));
    let off = app.wait_text("Auto-approve off");
    assert!(row_text(&off, u32::from(toggle.y)).contains("○ Auto-approve"));

    app.move_to(outside);
    let disabled_rest = crop(&app.settle(), disabled, 16, 1);
    app.move_to(inside(disabled));
    assert_eq!(crop(&app.settle(), disabled, 16, 1), disabled_rest);
    app.down(inside(disabled));
    assert_eq!(crop(&app.settle(), disabled, 16, 1), disabled_rest);
    app.up(inside(disabled));
    let rejected = app.settle();
    assert!(text(&rejected).contains("Auto-approve off"));
    assert!(!text(&rejected).contains("Disabled primary ✓"));
    app.click(inside(preview));
    let after_disabled = app.wait_text("Preview ✓");
    assert_eq!(crop(&after_disabled, disabled, 16, 1), disabled_rest);
    app.visual(&format!("functional/showcase_buttons_{color}"));
    app.finish();
}

#[test]
fn showcase_button_hover_hold_release_cancel_and_disabled_truecolor() {
    button_lifecycle("truecolor");
}

#[test]
fn showcase_button_hover_hold_release_cancel_and_disabled_without_color() {
    button_lifecycle("nocolor");
}

#[test]
fn showcase_button_hover_hold_release_cancel_and_disabled_256_colors() {
    button_lifecycle("256");
}

#[test]
fn showcase_button_hover_hold_release_cancel_and_disabled_16_colors() {
    button_lifecycle("16");
}

#[test]
fn showcase_button_hover_hold_release_cancel_and_disabled_monochrome() {
    button_lifecycle("none");
}

fn visible_list_rows(screen: &TerminalSnapshot) -> Vec<u16> {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            let (_, number) = line.split_once("Row ")?;
            number.get(..3)?.parse().ok()
        })
        .collect()
}

fn cropped_text(screen: &TerminalSnapshot, point: Point, width: u16, height: u16) -> String {
    crop(screen, point, width, height)
        .iter()
        .map(|cell| cell.text())
        .collect()
}

#[test]
fn showcase_wheel_targets_the_pointed_pane_then_keys_follow_clicked_focus() {
    let app = App::new(
        "showcase-nested-scroll",
        SHOWCASE,
        &[
            "--page",
            "scrolling",
            "--motion",
            "paused",
            "--frame",
            "1600",
        ],
        160,
        50,
        "truecolor",
    );
    let initial = app.wait_text("739.63s");
    let prose = find(&initial, "Junie works through a task");
    let list = find(&initial, "Row 001");
    app.click(inside(prose));
    let focused = app.wait_text("↑↓ Scroll");
    let prose_before = cropped_text(&focused, prose, 30, 10);
    let rows_before = visible_list_rows(&focused);
    assert_eq!(rows_before.first(), Some(&1));
    app.key("pagedown");
    app.wait_screen("PageDown scrolls the focused prose pane", |screen| {
        cropped_text(screen, prose, 30, 10) != prose_before
    });
    app.key("home");
    app.wait_screen("Home restores the focused prose pane", |screen| {
        cropped_text(screen, prose, 30, 10) == prose_before
    });

    app.wheel(inside(list), true, 3);
    let scrolled = app.wait_screen("wheel advances the list under the pointer", |screen| {
        visible_list_rows(screen)
            .first()
            .is_some_and(|first| *first > 1)
    });
    assert_eq!(cropped_text(&scrolled, prose, 30, 10), prose_before);
    assert!(
        text(&scrolled).contains("row 1 · vertical"),
        "wheel must not move the list cursor"
    );

    app.visual("functional/showcase_nested_scroll");
    let row = (0..scrolled.rows())
        .find_map(|y| {
            let line = row_text(&scrolled, y);
            if line.contains("retired") {
                return None;
            }
            line.split_once("Row ")?.1.get(..3)?.parse::<u16>().ok()
        })
        .expect("a visible list row without the retired label");
    app.click_text(&format!("Row {row:03}"));
    app.wait_text(&format!("row {row} · vertical"));
    app.key("end");
    let end = app.wait_text("row 119 · vertical");
    assert!(text(&end).contains("Row 119"));
    assert!(!visible_list_rows(&end).contains(&1));
    app.key("home");
    let home = app.wait_text("row 1 · vertical");
    assert_eq!(visible_list_rows(&home).first(), Some(&1));
    assert_eq!(cropped_text(&home, prose, 30, 10), prose_before);
    app.finish();
}

#[test]
fn showcase_primary_button_exact_rest_hover_held_colours() {
    let app = App::new(
        "showcase-primary-colours",
        SHOWCASE,
        &["--page", "buttons", "--motion", "reduced", "--frame", "40"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Run task");
    let run = inside(find(&initial, "Run task"));

    assert_eq!(bg(&initial, run), SnapshotColor::Rgb(72, 224, 84));
    assert!(!text(&initial).contains("Run task ✓"));

    app.move_to(run);
    let hover = app.wait_screen("primary hover colour", |s| {
        bg(s, run) == SnapshotColor::Rgb(58, 179, 67)
    });
    assert!(!text(&hover).contains("Run task ✓"));

    app.down(run);
    let held = app.wait_screen("primary held colour", |s| {
        bg(s, run) == SnapshotColor::Rgb(46, 154, 56)
    });
    assert!(
        !text(&held).contains("Run task ✓"),
        "press must not activate"
    );
    assert_eq!(
        held.cells().iter().filter(|c| c.text() == "▎").count(),
        1,
        "one keyboard-focus gutter, independent of hover/selection"
    );

    app.up(run);
    app.wait_text("Run task ✓");
    app.wait_screen("release flash returns to hover", |s| {
        bg(s, run) == SnapshotColor::Rgb(58, 179, 67)
    });

    app.wait_absent("Run task \u{2713}");
    app.visual("functional/showcase_primary_colours");
    app.finish();
}

#[test]
fn showcase_wheel_scrolls_pointed_grid_without_moving_sidebar() {
    let app = App::new(
        "showcase-datagrid-wheel",
        SHOWCASE,
        &["--page", "datagrid", "--motion", "paused", "--frame", "40"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Northwind Traders");
    let cell = find(&initial, "1001");

    let sidebar = |s: &TerminalSnapshot| {
        (2..39u32)
            .map(|y| {
                (0..28u32)
                    .filter_map(|x| s.cell_at(x, y).map(|c| c.text().to_string()))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
    };
    let before = sidebar(&initial);
    assert!(row_text(&initial, u32::from(cell.y)).contains("1001"));

    app.wheel(inside(cell), true, 1);
    let down = app.wait_screen("wheel brings row 1004 to the top", |s| {
        let row = row_text(s, u32::from(cell.y));
        row.contains("1004") && row.contains("Fabrikam Robotics")
    });
    assert_eq!(sidebar(&down), before, "sidebar must not move");

    app.wheel(inside(cell), false, 1);
    let up = app.wait_screen("wheel returns to row 1001", |s| {
        let row = row_text(s, u32::from(cell.y));
        row.contains("1001") && row.contains("Northwind Traders")
    });
    assert_eq!(sidebar(&up), before);
    app.visual("functional/showcase_datagrid_wheel");
    app.finish();
}

#[test]
fn showcase_checks_radio_exclusivity_mixed_and_disabled() {
    let app = App::new(
        "showcase-checks",
        SHOWCASE,
        &["--page", "checks", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_screen("checks page fully rendered", |s| {
        let t = text(s);
        t.contains("Select all rows")
            && t.contains("[\u{2713}] Send notifications")
            && t.contains("(\u{25cf}) eu-west-1")
    });
    assert!(text(&initial).contains("(●) eu-west-1"));

    let first = |s: &TerminalSnapshot, needle: &str| find_all(s, needle)[0];

    app.click(inside(first(&initial, "Send notifications")));
    app.wait_text("[ ] Send notifications");
    app.click(inside(first(&app.screen(), "Send notifications")));
    app.wait_text("[✓] Send notifications");

    assert!(text(&initial).contains("(●) eu-west-1"));

    app.click(inside(find(&app.screen(), "Repeatable read")));
    let radio = app.wait_text("(●) Repeatable read");
    assert!(text(&radio).contains("( ) Read committed"));
    assert!(text(&radio).contains("( ) Serializable"));
    app.key("down");
    let next = app.wait_text("(●) Serializable");
    assert!(text(&next).contains("( ) Repeatable read"));
    app.key("up");
    app.wait_text("(●) Repeatable read");

    app.click(inside(find(&app.screen(), "Select all rows")));
    app.wait_screen("select-all checks the mixed box", |s| {
        !text(s).contains("[–] Select all rows")
    });

    let beta = find(&app.screen(), "Beta features");
    let rest = crop(&app.settle(), beta, 20, 1);
    app.move_to(inside(beta));
    assert_eq!(crop(&app.settle(), beta, 20, 1), rest);
    app.down(inside(beta));
    assert_eq!(crop(&app.settle(), beta, 20, 1), rest);
    app.up(inside(beta));
    assert!(text(&app.screen()).contains("Beta features off"));
    app.click(inside(find(&app.screen(), "Auto-save drafts")));
    app.wait_text("Auto-save drafts off");
    app.visual("functional/showcase_checks");
    app.finish();
}

#[test]
fn showcase_tab_selection_survives_focus_move_and_single_gutter() {
    let app = App::new(
        "showcase-tabs",
        SHOWCASE,
        &["--page", "tabs", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Nested strips");
    app.click(inside(find(&app.screen(), "DDL")));
    let selected = app.wait_screen("DDL strip selection changes the breadcrumb", |s| {
        text(s).contains("orders › DDL")
    });
    assert_eq!(
        selected.cells().iter().filter(|c| c.text() == "▎").count(),
        1,
        "exactly one focus gutter"
    );

    app.key("tab");
    let moved = app.settle();
    assert!(
        text(&moved).contains("orders › DDL"),
        "selection survives focus move:\n{}",
        text(&moved)
    );
    assert_eq!(moved.cells().iter().filter(|c| c.text() == "▎").count(), 1);
    app.click(inside(find(&app.screen(), "Structure")));
    app.wait_screen("reselect Structure", |s| {
        text(s).contains("orders › Structure › Columns")
    });
    app.visual("functional/showcase_tabs_selection");
    app.finish();
}

#[test]
fn showcase_select_outside_click_and_escape_dismiss_without_change() {
    let app = App::new(
        "showcase-select-dismiss",
        SHOWCASE,
        &["--page", "chipsselects", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Sort by");
    assert!(text(&initial).contains("created_at"));
    let field = find(&initial, "created_at");
    let arrow = Point {
        x: field.x + 22,
        y: field.y,
    };
    app.click(arrow);
    app.wait_text("customer");

    app.click(inside(find(&app.screen(), "Filters")));
    let dismissed = app.wait_screen("outside click closes the select popup", |s| {
        !text(s).contains("customer")
    });
    assert!(text(&dismissed).contains("created_at"));

    app.click(arrow);
    app.wait_text("customer");
    app.key("escape");
    let escaped = app.wait_screen("escape closes the select popup", |s| {
        !text(s).contains("customer")
    });
    assert!(text(&escaped).contains("created_at"));

    app.click(arrow);
    app.wait_text("customer");
    let option = find_all(&app.screen(), "total")
        .into_iter()
        .find(|pt| pt.x >= field.x && pt.y > field.y)
        .expect("popup total option");
    app.click(inside(option));
    let chosen = app.wait_screen("chosen option becomes the value", |s| {
        !text(s).contains("customer")
    });
    assert!(row_text(&chosen, u32::from(field.y)).contains("total"));
    app.visual("functional/showcase_select_dismiss");
    app.finish();
}

#[test]
fn showcase_menu_outside_click_dismisses() {
    let app = App::new(
        "showcase-menus",
        SHOWCASE,
        &["--page", "menus", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Actions ▾");
    app.click(inside(find(&app.screen(), "Actions ▾")));
    let open = app.wait_screen("actions menu opens", |s| {
        let t = text(s);
        t.contains("Rename") || t.contains("Duplicate") || t.contains("Export")
    });
    assert!(text(&open).contains("Actions"));
    app.click(inside(find(&open, "report.pdf")));
    app.wait_screen("outside click dismisses the menu", |s| {
        !text(s).contains("Rename") && !text(s).contains("Duplicate")
    });
    app.visual("functional/showcase_menu_dismiss");
    app.finish();
}

#[test]
fn showcase_splitter_seam_drag_resizes_panes() {
    let app = App::new(
        "showcase-splits",
        SHOWCASE,
        &["--page", "splits", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_screen("splits page with Editor and splitter.rs", |s| {
        !find_all(s, "Editor").is_empty() && !find_all(s, "splitter.rs").is_empty()
    });
    let editor_before = find(&initial, "Editor");
    let content = find(&initial, "splitter.rs");

    let seam = Point {
        x: editor_before.x - 5,
        y: content.y,
    };
    app.down(seam);

    let mut last_x = editor_before.x;
    for step in 1..=4 {
        app.drag_to(Point {
            x: seam.x + step * 2,
            y: seam.y,
        });
        let moved = app.wait_screen("seam drag step moves the Editor pane", |s| {
            find_all(s, "Editor").first().is_some_and(|p| p.x > last_x)
        });
        last_x = find(&moved, "Editor").x;
    }
    app.up(Point {
        x: seam.x + 8,
        y: seam.y,
    });
    let resized = app.wait_screen("dragging the seam moves the Editor pane", |s| {
        find_all(s, "Editor")
            .first()
            .is_some_and(|p| p.x >= editor_before.x + 4)
    });
    assert!(
        find(&resized, "Editor").x >= editor_before.x + 4,
        "editor before={editor_before:?} after={:?}",
        find(&resized, "Editor")
    );
    app.visual("functional/showcase_splitter_drag");
    app.finish();
}

#[test]
fn showcase_scrollbar_thumb_drag_moves_list() {
    let app = App::new(
        "showcase-scrollbar",
        SHOWCASE,
        &[
            "--page",
            "scrolling",
            "--motion",
            "paused",
            "--frame",
            "1600",
        ],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Row 001");
    let initial = app.stable_screen();
    let row1 = find(&initial, "Row 001");

    let thumb = (row1.x..row1.x + 30)
        .flat_map(|x| (row1.y..row1.y + 3).map(move |y| Point { x, y }))
        .find(|p| {
            app.screen()
                .cell_at(u32::from(p.x), u32::from(p.y))
                .is_some_and(|c| c.text() == "┃")
        })
        .expect("scrollbar thumb at top of Long list track");
    let neighbour_before = cropped_text(&initial, find(&initial, "Junie works through a"), 24, 7);
    app.down(thumb);
    for step in 1..=4 {
        app.drag_to(Point {
            x: thumb.x,
            y: thumb.y + step * 2,
        });
    }
    app.up(Point {
        x: thumb.x,
        y: thumb.y + 8,
    });
    app.wait_screen("thumb drag advances the list", |s| {
        visible_list_rows(s).first().is_some_and(|f| *f > 1)
    });
    let scrolled = app.stable_screen();
    assert_eq!(
        cropped_text(&scrolled, find(&scrolled, "Junie works through a"), 24, 7),
        neighbour_before,
        "neighbour pane unchanged"
    );
    app.visual("functional/showcase_scrollbar_drag");
    app.finish();
}

fn gutter_on(screen: &TerminalSnapshot, label: &str) -> bool {
    let point = find(screen, label);
    (1..=3).any(|back| {
        point.x >= back
            && screen
                .cell_at(u32::from(point.x - back), u32::from(point.y))
                .is_some_and(|cell| cell.text() == "▎")
    })
}

#[test]
fn showcase_keyboard_activates_focused_run_task() {
    let app = App::new(
        "showcase-keyboard",
        SHOWCASE,
        &["--page", "buttons", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Run task");
    assert!(!text(&initial).contains("Run task ✓"));
    assert!(!text(&initial).contains("Preview ✓"));

    let mut screen = initial;
    for _ in 0..16 {
        if gutter_on(&screen, "Run task") {
            break;
        }
        app.key("tab");
        screen = app.settle();
    }
    assert!(
        gutter_on(&app.screen(), "Run task"),
        "Run task starts unactivated and receives keyboard focus:\n{}",
        text(&app.screen())
    );
    assert!(!text(&app.screen()).contains("Run task ✓"));
    assert!(!text(&app.screen()).contains("Preview ✓"));
    app.key("enter");
    let done = app.wait_text("Run task ✓");
    assert!(
        !text(&done).contains("Preview ✓"),
        "Preview stays idle when Run task is the activated control:\n{}",
        text(&done)
    );
    app.visual("functional/showcase_keyboard_run_task");
    app.finish();
}

#[test]
fn showcase_list_selection_keyboard_and_multiselect() {
    let app = App::new(
        "showcase-lists",
        SHOWCASE,
        &["--page", "lists", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Chosen: Rust");
    app.click(inside(find(&app.screen(), "Python")));
    app.key("enter");
    app.wait_text("Chosen: Python");
    app.key("down");
    app.key("enter");
    app.wait_text("Chosen: Kotlin");
    app.key("up");
    app.key("enter");
    app.wait_text("Chosen: Python");

    app.click(inside(find(&app.screen(), "src/api/auth.rs")));
    app.wait_text("1 selected");
    app.key("space");
    app.wait_screen("space toggles the file off", |s| {
        !text(s).contains("1 selected")
    });
    app.visual("functional/showcase_lists");
    app.finish();
}

#[test]
fn showcase_tree_expand_and_file_selection() {
    let app = App::new(
        "showcase-trees",
        SHOWCASE,
        &["--page", "trees", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Nothing selected");
    assert!(text(&initial).contains("▾ src"));
    app.click(inside(find(&initial, "config.rs")));
    app.key("enter");
    let selected = app.wait_screen("file selection is reported", |s| {
        !text(s).contains("Nothing selected")
    });
    assert!(text(&selected).contains("config.rs"));

    let folder = find_all(&app.screen(), "src")
        .into_iter()
        .find(|pt| pt.x > 28 && pt.y < 12)
        .expect("src folder row");
    app.click(inside(folder));
    app.key("left");
    app.wait_screen("collapsing src hides its files", |s| {
        text(s).contains("▸ src") && !text(s).contains("1.9 KB")
    });
    app.key("right");
    app.wait_screen("expanding src restores its files", |s| {
        text(s).contains("▾ src") && text(s).contains("1.9 KB")
    });

    let label = find_all(&app.screen(), "src")
        .into_iter()
        .find(|pt| pt.x > 28 && pt.y < 12)
        .expect("src folder label");
    let gutter = Point {
        x: label.x - 3,
        y: label.y,
    };
    let gutter_text = |s: &TerminalSnapshot| {
        s.cell_at(u32::from(gutter.x), u32::from(gutter.y))
            .map(|c| c.text().to_string())
    };
    let settled = |s: &TerminalSnapshot| {
        gutter_text(s).as_deref() == Some("▎")
            && cell_bg(s, label) != tui_test::SnapshotColor::Rgb(56, 56, 62)
    };
    app.move_to(Point { x: 0, y: 0 });
    app.key("tab");
    app.wait_screen("focus leaves the tree", |s| {
        gutter_text(s).as_deref() != Some("▎")
    });
    app.key("shift+tab");
    app.wait_screen("focus returns to the tree", |s| {
        gutter_text(s).as_deref() == Some("▎")
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if app.try_sustained(std::time::Duration::from_millis(600), |s| settled(s)) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tree row must settle without hover"
        );
        app.move_to(Point { x: 0, y: 0 });
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    app.visual("functional/showcase_trees");
    app.finish();
}

#[test]
fn showcase_input_edit_apply_and_escape_reverts() {
    let app = App::new(
        "showcase-inputs",
        SHOWCASE,
        &["--page", "inputs", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("payments-gateway");

    app.key("tab");
    app.key("tab");
    app.wait_screen("field is in edit mode", |s| text(s).contains("EDIT"));
    app.key("enter");
    app.type_text("-renamed");
    app.wait_text("payments-gateway-renamed");
    app.key("enter");
    let applied = app.settle();
    assert!(text(&applied).contains("payments-gateway-renamed"));

    app.key("enter");
    app.type_text("-bad");
    app.wait_text("payments-gateway-renamed-bad");
    app.key("escape");
    let reverted = app.wait_screen("escape reverts the unapplied edit", |s| {
        !text(s).contains("-bad")
    });
    assert!(text(&reverted).contains("payments-gateway-renamed"));
    app.visual("functional/showcase_inputs");
    app.finish();
}

#[test]
fn showcase_dialog_opens_traps_and_escape_cancels() {
    let app = App::new(
        "showcase-dialogs",
        SHOWCASE,
        &["--page", "dialogs", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Open a dialog");
    app.click(inside(find(&app.screen(), "Confirm run")));
    let open = app.wait_text("Run task now?");
    assert!(text(&open).contains("Nothing yet"));
    assert!(text(&open).contains("Cancel"));

    app.visual("functional/showcase_dialogs");
    app.key("tab");
    let tabbed = app.wait_screen("tab stays inside the confirm dialog", |s| {
        text(s).contains("Run task now?") && text(s).contains("Nothing yet")
    });
    assert!(!text(&tabbed).contains("Delete branch?"));
    app.key("shift+tab");
    let shifted = app.wait_screen("shift+tab stays inside the confirm dialog", |s| {
        text(s).contains("Run task now?") && text(s).contains("Nothing yet")
    });
    assert!(!text(&shifted).contains("Task started"));
    app.key("d");
    let trapped = app.settle();
    assert!(
        text(&trapped).contains("Run task now?"),
        "the page shortcut must not open Delete branch while the dialog is up:\n{}",
        text(&trapped)
    );
    assert!(!text(&trapped).contains("Delete branch?"));
    assert!(text(&trapped).contains("Nothing yet"));
    app.key("escape");
    let cancelled = app.wait_screen("escape cancels and returns to the opener", |s| {
        let t = text(s);
        t.contains("Cancelled") && !t.contains("Run task now?") && gutter_on(s, "Confirm run")
    });
    assert!(!text(&cancelled).contains("Nothing yet"));
    assert!(!text(&cancelled).contains("Task started"));
    app.finish();
}

#[test]
fn showcase_table_header_sort_changes_row_order() {
    let app = App::new(
        "showcase-tables",
        SHOWCASE,
        &["--page", "tables", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("#1040");
    assert!(text(&initial).contains("unsorted"));
    let before = task_lines(&initial);
    let on_1040 = before
        .iter()
        .find(|line| line.contains("#1040"))
        .expect("row identity on #1040")
        .clone();
    assert!(
        on_1040.contains("Add rate limiting") && on_1040.contains("Done"),
        "{on_1040}"
    );
    assert!(
        before.iter().any(|line| line.contains("#1041")),
        "unsorted neighbor #1041 is visible:\n{}",
        text(&initial)
    );
    app.click(inside(find(&initial, "Status")));
    let sorted = app.wait_screen("header click sorts the table", |s| {
        !text(s).contains("unsorted") && text(s).contains("sorted by status")
    });
    let after = task_lines(&sorted);
    assert_ne!(before, after, "status sort changes row order");
    let moved = after
        .iter()
        .find(|line| !line.contains("#1040"))
        .expect("a row other than #1040")
        .clone();
    assert_ne!(on_1040, moved);
    assert!(
        after.iter().any(|line| line.contains("#1040")),
        "the #1040 identity stays in the table"
    );
    assert!(
        !after.iter().take(2).any(|line| line.contains("#1041")),
        "Running #1041 leaves the top of the status order:\n{after:?}"
    );
    app.visual("functional/showcase_table_sort");
    app.finish();
}

fn task_lines(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .filter(|line| line.contains("#10"))
        .collect()
}

#[test]
fn showcase_form_required_validation_and_reset() {
    let app = App::new(
        "showcase-forms",
        SHOWCASE,
        &["--page", "forms", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("New task");
    app.click(inside(find(&app.screen(), "Create task")));
    let invalid = app.wait_text("Required");
    assert!(text(&invalid).contains("New task"));
    assert!(!text(&invalid).contains("Task created ✓"));
    app.visual("functional/showcase_forms");
    app.finish();

    let app = App::new(
        "showcase-forms-reset",
        SHOWCASE,
        &["--page", "forms", "--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Short imperative summary");
    app.click(inside(find(&app.screen(), "Short imperative summary")));
    app.type_text("Ship login fix");
    app.wait_text("Ship login fix");
    app.click(inside(find(&app.screen(), "Reset")));
    let reset = app.wait_text("Form reset");
    assert!(
        !text(&reset).contains("Ship login fix"),
        "Reset clears the task name:\n{}",
        text(&reset)
    );
    assert!(text(&reset).contains("Short imperative summary"));
    app.click(inside(find(&app.screen(), "Short imperative summary")));
    app.type_text("Ship login fix");
    app.wait_text("Ship login fix");
    app.click(inside(find(&app.screen(), "Create task")));
    let created = app.wait_text("Task created ✓");
    assert!(text(&created).contains("Ship login fix"));
    assert!(!text(&created).contains("Required"));
    app.finish();
}

fn visit_page(slug: &str, heading: &str) {
    let app = App::new(
        &format!("showcase-page-{slug}"),
        SHOWCASE,
        &["--page", slug, "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text(heading);
    app.settle();
    app.visual(&format!("functional/showcase_page_{slug}"));
    app.finish();
}

macro_rules! page_test {
    ($fn_name:ident, $slug:expr, $heading:expr) => {
        #[test]
        fn $fn_name() {
            visit_page($slug, $heading);
        }
    };
}

page_test!(showcase_page_chrome, "chrome", "Chrome");
page_test!(showcase_page_statushints, "statushints", "Status & hints");
page_test!(
    showcase_page_breadcrumbsrails,
    "breadcrumbsrails",
    "Breadcrumbs & rails"
);
page_test!(showcase_page_helpkeys, "helpkeys", "Help & keys");
page_test!(showcase_page_quitleave, "quitleave", "Quit & leave");
page_test!(
    showcase_page_focusnavigation,
    "focusnavigation",
    "Focus & navigation"
);
page_test!(showcase_page_textareas, "textareas", "Text areas");
page_test!(showcase_page_toolbars, "toolbars", "Toolbars");
page_test!(showcase_page_editable, "editable", "Editable");
page_test!(showcase_page_selection, "selection", "Selection");
page_test!(showcase_page_celleditors, "celleditors", "Cell editors");
page_test!(showcase_page_columns, "columns", "Columns");
page_test!(showcase_page_sidebars, "sidebars", "Sidebars");
page_test!(showcase_page_pickers, "pickers", "Pickers");

#[test]
fn showcase_multi_picker_modes_filter_disabled_and_report_selection() {
    let app = App::new(
        "showcase-multi-pickers",
        SHOWCASE,
        &["--page", "pickers", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.click_text("Multi select");
    app.wait_screen("multi picker shows the disabled row", |screen| {
        let visible = text(screen);
        visible.contains("Space Toggle")
            && visible.contains("Row1")
            && visible.contains("Row2")
            && visible.contains("Row3")
    });
    app.key("space");
    app.key("down");
    app.key("space");
    app.key("enter");
    let chosen = app.wait_text("ChoseMany: Row1, Row3");
    assert!(text(&chosen).contains("2 selected"));
    assert!(!text(&chosen).contains("Row2, Row3"));

    app.click_text("Multi search");
    app.wait_text("Insert Toggle");
    app.type_text("Row3");
    app.wait_screen("search filters to the third row", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains("Row3") && line.contains("multi row 3")
        })
    });
    app.key("insert");
    app.key("escape");
    app.type_text("Row1");
    app.wait_screen("search filters to the first row", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains("Row1") && line.contains("multi row 1")
        })
    });
    app.key("insert");
    app.key("enter");
    let searched = app.wait_text("ChoseMany: Row3, Row1");
    assert!(text(&searched).contains("2 selected"));
    app.finish();
}
page_test!(showcase_page_dialogkinds, "dialogkinds", "Dialog kinds");
page_test!(showcase_page_filebrowser, "filebrowser", "File browser");
page_test!(showcase_page_progress, "progress", "Progress");
page_test!(showcase_page_emptyerror, "emptyerror", "Empty & error");
page_test!(showcase_page_meters, "meters", "Meters");
page_test!(showcase_page_notices, "notices", "Notices");
page_test!(showcase_page_panels, "panels", "Panels");
page_test!(showcase_page_masterdetail, "masterdetail", "Master-detail");
page_test!(showcase_page_panetree, "panetree", "Pane tree");
page_test!(showcase_page_codeeditor, "codeeditor", "Code editor");
page_test!(showcase_page_diff, "diff", "Diff");
page_test!(showcase_page_terminal, "terminal", "Terminal");
page_test!(
    showcase_page_textutilities,
    "textutilities",
    "Text utilities"
);
page_test!(showcase_page_settings, "settings", "Settings");
page_test!(showcase_page_taskrunner, "taskrunner", "Task runner");
page_test!(
    showcase_page_queryworkbench,
    "queryworkbench",
    "Query workbench"
);
page_test!(showcase_page_connections, "connections", "Connections");
page_test!(showcase_page_finder, "finder", "Finder");
page_test!(showcase_page_diskcleanup, "diskcleanup", "Disk cleanup");
page_test!(showcase_page_activity, "activity", "Activity");
page_test!(showcase_page_workspaces, "workspaces", "Workspaces");
page_test!(
    showcase_page_accountsusage,
    "accountsusage",
    "Accounts & usage"
);
page_test!(
    showcase_page_launchcockpit,
    "launchcockpit",
    "Launch cockpit"
);
page_test!(
    showcase_page_readonlyreport,
    "readonlyreport",
    "Read-only report"
);

fn terminal_pane(name: &str, opts: &[&str]) -> App {
    let mut args = vec!["--page", "terminalpane", "--motion", "paused"];
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    let app = App::new(name, SHOWCASE, &args, 120, 40, "truecolor");
    app.wait_text("Terminal pane");
    app.settle();
    app
}

fn pane_row(screen: &TerminalSnapshot, title: &str, row: u16) -> (Point, String) {
    let top = find(screen, title);
    let y = top.y + 1 + row;
    (Point { x: 0, y }, row_text(screen, u32::from(y)))
}

fn cell_at(
    screen: &TerminalSnapshot,
    row: (Point, String),
    needle: &str,
) -> tui_test::SnapshotCell {
    let at = row
        .1
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} in {:?}", row.1));
    let col = row.1[..at].chars().count();
    screen
        .cell_at(col as u32, u32::from(row.0.y))
        .expect("pane cell")
        .clone()
}

fn replay_row(replay: &str) -> (TerminalSnapshot, (Point, String)) {
    let app = terminal_pane(
        "showcase-terminal-replay-start",
        &[
            "terminalpane.fixture=midstream",
            &format!("terminalpane.replay={replay}"),
        ],
    );
    app.click_text("Replay");
    let screen = app.wait_screen("replayed starting chunk", |s| {
        text(s).contains("2 of 4 chunks")
            || text(s).contains("3 of 4 chunks")
            || text(s).contains("1 of 4 chunks")
    });
    let row = pane_row(&screen, "midstream", 0);
    app.finish();
    (screen, row)
}

#[test]
fn showcase_terminal_replay_mid_sequence() {
    let app = terminal_pane("showcase-terminal-pane", &[]);
    let screen = app.settle();
    assert!(text(&screen).contains("dev@jackin"));
    for mode in ["app cursor", "bracketed paste", "alt screen", "hide cursor"] {
        assert!(
            row_text(&screen, u32::from(find(&screen, mode).y)).contains("off"),
            "{mode} reads off"
        );
    }
    let coloured = cell_at(&screen, pane_row(&screen, "session", 2), "rgb-green");
    let bold = cell_at(&screen, pane_row(&screen, "session", 3), "bold");
    assert!(bold.has(Attrs::BOLD));
    app.finish();

    let plain = terminal_pane("showcase-terminal-pane-plain", &["terminalpane.colour=off"]);
    let screen = plain.settle();
    let uncoloured = cell_at(&screen, pane_row(&screen, "session", 2), "rgb-green");
    assert_ne!(
        format!("{:?}", uncoloured.fg()),
        format!("{:?}", coloured.fg()),
        "colour off drops the foreground colour"
    );
    assert!(
        cell_at(&screen, pane_row(&screen, "session", 3), "bold").has(Attrs::BOLD),
        "bold survives without colour"
    );
    plain.finish();

    let full = terminal_pane(
        "showcase-terminal-fullscreen",
        &["terminalpane.fixture=fullscreen"],
    );
    full.click_text("Play all");
    let screen = full.settle();
    for mode in ["app cursor", "bracketed paste", "alt screen", "hide cursor"] {
        assert!(
            row_text(&screen, u32::from(find(&screen, mode).y)).contains("on"),
            "{mode} reads on"
        );
    }
    let pane = find(&screen, "fullscreen");
    full.click(Point {
        x: pane.x + 4,
        y: pane.y + 4,
    });
    full.key("enter");
    full.wait_text("Ctrl+G Release");
    full.key("up");
    full.wait_text("ESC O A");
    full.key("ctrl+g");
    full.wait_absent("Ctrl+G Release");
    full.click_text("Reset");
    full.wait_text("0 of 4 chunks");
    let pane = find(&full.settle(), "fullscreen");
    full.click(Point {
        x: pane.x + 4,
        y: pane.y + 4,
    });
    full.key("enter");
    full.key("up");
    full.wait_text("ESC [ A");
    full.key("ctrl+g");
    full.finish();

    for (replay, shown) in [
        ("inside a CSI sequence", ";224;84mtail"),
        ("inside an OSC sequence", "0;titleafter"),
        ("inside a UTF-8 character", "x"),
    ] {
        let (_, row) = replay_row(replay);
        assert!(row.1.contains(shown), "{replay}: {:?}", row.1);
    }
    let (screen, row) = replay_row("stream start");
    let tail = cell_at(&screen, row, "tail");
    assert!(format!("{:?}", tail.fg()).contains("72"), "{:?}", tail.fg());

    let mid = terminal_pane(
        "showcase-terminal-midstream",
        &[
            "terminalpane.fixture=midstream",
            "terminalpane.replay=inside a CSI sequence",
        ],
    );
    mid.click_text("Replay");
    let literal = mid.wait_text(";224;84mtail");
    let row = pane_row(&literal, "midstream", 0);
    let cell = cell_at(&literal, row, ";224;84mtail");
    assert_ne!(format!("{:?}", cell.fg()), format!("{:?}", tail.fg()));
    for _ in 0..3 {
        mid.click_text("Step");
        mid.settle();
    }
    let ready = mid.wait_text("ready $");
    assert!(!text(&ready).contains(";224;84mtail"));
    mid.click_text("Replay");
    mid.wait_text(";224;84mtail");
    mid.settle();
    mid.visual("functional/showcase_terminal_replay");
    mid.finish();
}

fn mode_reads(screen: &TerminalSnapshot, mode: &str, value: &str) -> bool {
    row_text(screen, u32::from(find(screen, mode).y)).contains(value)
}

#[test]
fn showcase_terminal_play_all_counts_the_leave_chunk() {
    let app = terminal_pane(
        "showcase-terminal-leave-chunk",
        &["terminalpane.fixture=fullscreen"],
    );
    app.click_text("Play all");
    let played = app.wait_text("3 of 4 chunks");
    assert!(mode_reads(&played, "alt screen", "on"));
    app.click_text("Step");
    let left = app.wait_text("4 of 4 chunks");
    assert!(mode_reads(&left, "alt screen", "off"));
    app.click_text("Step");
    app.settle();
    app.wait_text("4 of 4 chunks");
    app.finish();
}

const SESSION: &str = "terminalpane.host=session controller";

fn session_page(name: &str, script: &str, area: &str) -> App {
    terminal_pane(
        name,
        &[
            SESSION,
            &format!("terminalpane.script={script}"),
            &format!("terminalpane.area={area}"),
        ],
    )
}

#[test]
fn showcase_terminal_session_owner_script() {
    let app = session_page("showcase-terminal-session-owner", "owner", "fits");
    app.wait_text("attaching");
    app.wait_text("Attaching…");
    app.click_text("Run 1 s");
    app.wait_text("attached · owner");
    app.wait_text("demo$ ls");
    app.click_text("Run 1 s");
    let screen = app.wait_text("clipboard write refused");
    assert!(
        text(&screen).contains("reply ESC [ 3 ; 5 R"),
        "the owner answers the program's query:\n{}",
        text(&screen)
    );
    app.click_text("Type");
    app.wait_text("a → sent to the program");

    app.click_text("Paste 3 lines");
    app.wait_text("paste of 3 lines is held");
    app.click_text("Confirm");
    app.wait_text("paste → sent to the program");
    app.wait_absent("Confirm");
    app.click_text("Reset");
    app.wait_text("Attaching…");
    app.finish();
}

#[test]
fn showcase_terminal_session_captured_keys_reach_the_controller() {
    let app = session_page("showcase-terminal-session-keys", "owner", "fits");
    let screen = app.wait_text("Attaching…");
    let pane = find(&screen, "Attaching…");
    app.click(pane);
    app.key("enter");
    app.wait_text("Ctrl+G Release");
    app.key("x");
    let typed = app.wait_text("x → sent to the program");
    assert!(
        text(&typed).contains("input x"),
        "the controller forwarded the key to the link:\n{}",
        text(&typed)
    );
    app.key("ctrl+g");
    app.wait_absent("Ctrl+G Release");
    app.finish();
}

#[test]
fn showcase_terminal_session_viewer_script() {
    let app = session_page("showcase-terminal-session-viewer", "viewer", "fits");
    app.click_text("Run 1 s");
    app.wait_text("attached · viewer");
    app.wait_text("notice: another client controls this session");
    app.click_text("Type");
    app.wait_text("a → sent to the program");
    app.click_text("Run 1 s");
    app.click_text("Run 1 s");
    app.wait_text("attached · owner");
    app.finish();
}

#[test]
fn showcase_terminal_session_detach_exit_and_failure_scripts() {
    let detaches = session_page("showcase-terminal-session-detaches", "detaches", "fits");
    detaches.click_text("Run 1 s");
    detaches.click_text("Run 1 s");
    detaches.wait_text("detached · operator");
    detaches.click_text("Retry");
    detaches.wait_text("Attaching…");
    detaches.finish();

    let exits = session_page("showcase-terminal-session-exits", "exits", "fits");
    exits.click_text("Run 1 s");
    exits.click_text("Run 1 s");
    exits.wait_text("exited · code 0");
    exits.click_text("Retry");
    exits.click_text("Run 1 s");
    exits.wait_text("restarted");
    exits.finish();

    let fails = session_page("showcase-terminal-session-fails", "fails", "fits");
    fails.wait_text("Attach failed");
    fails.wait_text("no capacity, the host is busy");
    fails.click_text("Retry");
    fails.click_text("Run 1 s");
    fails.wait_text("attached · owner");
    fails.finish();
}

#[test]
fn showcase_terminal_session_drain_budget_option_stretches_the_replay() {
    let app = terminal_pane(
        "showcase-terminal-session-budget",
        &[SESSION, "terminalpane.budget=8"],
    );
    app.wait_text("8 bytes");
    for _ in 0..3 {
        app.click_text("Tick 200 ms");
    }
    app.wait_text("clock at 600 ms");
    app.wait_text("Attaching…");
    app.click_text("Tick 200 ms");
    app.wait_text("attached · owner");
    app.finish();

    let generous = terminal_pane("showcase-terminal-session-generous", &[SESSION]);
    for _ in 0..3 {
        generous.click_text("Tick 200 ms");
    }
    generous.wait_text("attached · owner");
    generous.finish();
}

#[test]
fn showcase_terminal_session_below_minimum_waits_for_room() {
    let app = session_page("showcase-terminal-session-small", "owner", "below minimum");
    app.wait_text("pending");
    app.wait_text("Waiting for room");
    app.wait_text("needs 20x4");
    app.click_text("Paste 3 lines");
    app.wait_text("paste → no program to take it");
    app.finish();
}

fn small_terminal_keeps_controls(cols: u16, rows: u16) {
    let app = App::new(
        &format!("showcase-terminal-pane-{cols}x{rows}"),
        SHOWCASE,
        &["--page", "terminalpane", "--motion", "paused"],
        cols,
        rows,
        "truecolor",
    );
    let screen = app.wait_text("Replay");
    assert!(
        text(&screen).contains("Step") && text(&screen).contains("Play all"),
        "the controls stay on screen at {cols}x{rows}:\n{}",
        text(&screen)
    );
    app.finish();
}

#[test]
fn showcase_terminal_pane_fits_80x24() {
    small_terminal_keeps_controls(80, 24);
}

#[test]
fn showcase_terminal_pane_fits_120x20() {
    small_terminal_keeps_controls(120, 20);
}

const FIXED_KEEP_AWAKE: &str = "Always off · the host decides";
const FIXED_DIRTY_EXIT: &str = "Always ask · the policy is managed";
const FIXED_TYPED_KEY: &str = "Not available · managed by policy";
const FIXED_DISABLE: &str = "Cannot disable · all stay on";
const FIXED_MANAGED_ROW: &str = "Managed by policy · cannot be changed";

fn fixed_page_in(name: &str, opts: &[&str], cols: u16, rows: u16, motion: &str) -> App {
    let mut args = vec!["--page", "fixedcontrols", "--motion", motion];
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    let app = App::new(name, SHOWCASE, &args, cols, rows, "truecolor");
    app.wait_text("Keep awake");
    app.settle();
    app
}

fn fixed_page(name: &str, opts: &[&str], cols: u16, rows: u16) -> App {
    fixed_page_in(name, opts, cols, rows, "reduced")
}

fn footer_row(screen: &TerminalSnapshot) -> String {
    let rows = text(screen).lines().count() as u32;
    row_text(screen, rows - 1)
}

fn refused_with(app: &App, reason: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("status reads {reason}"), |s| {
        footer_row(s).contains(reason)
    })
}

fn status_gone(app: &App, reason: &str) -> TerminalSnapshot {
    app.wait_screen("status expired", |s| !footer_row(s).contains(reason))
}

fn status_cleared(app: &App, reason: &str) -> TerminalSnapshot {
    app.key("f9");
    app.wait_screen("status cleared", |s| !footer_row(s).contains(reason))
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    row_text(screen, u32::from(find(screen, needle).y))
}

#[test]
fn showcase_fixed_controls_refuse_with_a_reason_and_never_leak_it() {
    let app = fixed_page_in(
        "showcase-fixed",
        &["fixedcontrols.status=sticky"],
        120,
        40,
        "paused",
    );
    let initial = app.screen();
    assert!(row_with(&initial, "Keep awake").contains(FIXED_KEEP_AWAKE));
    assert!(text(&initial).contains(FIXED_DIRTY_EXIT));
    assert!(!footer_row(&initial).contains(FIXED_KEEP_AWAKE));

    app.key("tab");
    app.key("tab");
    app.key("space");
    let keyed = refused_with(&app, FIXED_KEEP_AWAKE);
    assert!(text(&keyed).contains("[ ] Keep awake"));
    assert!(row_with(&keyed, "Keep awake").contains('▎'));
    assert_eq!(footer_row(&keyed).matches(FIXED_KEEP_AWAKE).count(), 1);
    status_cleared(&app, FIXED_KEEP_AWAKE);
    app.click(inside(find(&app.screen(), "Keep awake")));
    let clicked = refused_with(&app, FIXED_KEEP_AWAKE);
    assert!(text(&clicked).contains("[ ] Keep awake"));
    status_cleared(&app, FIXED_KEEP_AWAKE);

    app.key("tab");
    app.key("space");
    app.wait_text("[ ] Notify on exit");
    assert!(!footer_row(&app.screen()).contains(FIXED_KEEP_AWAKE));
    app.click(inside(find(&app.screen(), "Notify on exit")));
    app.wait_text("[✓] Notify on exit");
    assert!(!footer_row(&app.screen()).contains(FIXED_KEEP_AWAKE));

    app.key("tab");
    app.key("enter");
    let select = refused_with(&app, FIXED_DIRTY_EXIT);
    assert!(!text(&select).contains("keep"));
    status_cleared(&app, FIXED_DIRTY_EXIT);
    let label = find(&app.screen(), "On dirty exit");
    app.click(Point {
        x: label.x + 2,
        y: label.y + 1,
    });
    let select = refused_with(&app, FIXED_DIRTY_EXIT);
    assert!(!text(&select).contains("keep"));
    status_cleared(&app, FIXED_DIRTY_EXIT);

    app.key("tab");
    app.key("enter");
    refused_with(&app, FIXED_DISABLE);
    status_cleared(&app, FIXED_DISABLE);
    app.click(inside(find(&app.screen(), "Disable")));
    refused_with(&app, FIXED_DISABLE);
    status_cleared(&app, FIXED_DISABLE);

    app.key("tab");
    app.key("down");
    app.key("down");
    let landed = app.wait_screen("cursor lands on the fixed option", |s| {
        row_with(s, "Typed key").contains('▎')
    });
    assert!(text(&landed).contains("(●) Password manager"));
    assert!(text(&landed).contains("( ) Typed key"));
    assert!(!footer_row(&landed).contains(FIXED_TYPED_KEY));
    app.key("space");
    let radio = refused_with(&app, FIXED_TYPED_KEY);
    assert!(text(&radio).contains("(●) Password manager"));
    status_cleared(&app, FIXED_TYPED_KEY);
    app.click(inside(find(&app.screen(), "Typed key")));
    let radio = refused_with(&app, FIXED_TYPED_KEY);
    assert!(text(&radio).contains("(●) Password manager"));
    status_cleared(&app, FIXED_TYPED_KEY);

    app.key("tab");
    app.key("down");
    app.wait_screen("cursor lands on the fixed row", |s| {
        row_with(s, "Managed policy").contains('▎')
    });
    app.key("space");
    let list = refused_with(&app, FIXED_MANAGED_ROW);
    assert!(!row_with(&list, "Managed policy").contains('✓'));
    status_cleared(&app, FIXED_MANAGED_ROW);
    app.click(inside(find(&app.screen(), "Managed policy")));
    let list = refused_with(&app, FIXED_MANAGED_ROW);
    assert!(!row_with(&list, "Managed policy").contains('✓'));
    status_cleared(&app, FIXED_MANAGED_ROW);
    app.click(inside(find(&app.screen(), "Team policy")));
    let live = app.wait_screen("a live row ticks", |s| {
        row_with(s, "Team policy").contains('✓')
    });
    assert!(!footer_row(&live).contains(FIXED_MANAGED_ROW));
    app.finish();
}

#[test]
fn showcase_fixed_controls_reason_fades_after_five_seconds() {
    let app = fixed_page("showcase-fixed-fade", &[], 120, 40);
    app.key("tab");
    app.key("tab");
    app.key("space");
    refused_with(&app, FIXED_KEEP_AWAKE);
    let gone = status_gone(&app, FIXED_KEEP_AWAKE);
    assert!(text(&gone).contains("[ ] Keep awake"));
    app.finish();
}

#[test]
fn showcase_fixed_controls_sticky_reason_stays_until_the_next_key() {
    for (mode, stays) in [("5 s", true), ("sticky", false)] {
        let value = format!("fixedcontrols.status={mode}");
        let app = fixed_page_in(
            &format!("showcase-fixed-{}", mode.replace(' ', "")),
            &[&value],
            120,
            40,
            "paused",
        );
        app.key("tab");
        app.key("tab");
        app.key("space");
        refused_with(&app, FIXED_KEEP_AWAKE);
        app.key("tab");
        let next = app.wait_screen("focus moves to the live sibling", |s| {
            row_with(s, "Notify on exit").contains('▎')
        });
        if stays {
            assert!(footer_row(&next).contains(FIXED_KEEP_AWAKE), "{mode}");
        } else {
            let cleared = app.wait_screen("the next key clears a sticky reason", |s| {
                !footer_row(s).contains(FIXED_KEEP_AWAKE)
            });
            assert!(!footer_row(&cleared).contains(FIXED_KEEP_AWAKE), "{mode}");
        }
        app.finish();
    }
}

#[test]
fn showcase_fixed_controls_are_live_when_the_option_says_so() {
    let app = fixed_page(
        "showcase-fixed-live",
        &["fixedcontrols.state=live"],
        120,
        40,
    );
    let initial = app.screen();
    assert!(!text(&initial).contains(FIXED_KEEP_AWAKE));
    app.click(inside(find(&initial, "Keep awake")));
    let on = app.wait_text("[✓] Keep awake");
    assert!(!footer_row(&on).contains(FIXED_KEEP_AWAKE));
    app.click(inside(find(&app.screen(), "Typed key")));
    app.wait_text("(●) Typed key");
    app.finish();
}

#[test]
fn showcase_fixed_controls_show_the_full_reason_at_80x24() {
    let app = fixed_page("showcase-fixed-small", &[], 80, 24);
    app.click(inside(find(&app.screen(), "Keep awake")));
    let s = refused_with(&app, FIXED_KEEP_AWAKE);
    assert!(text(&s).contains("[ ] Keep awake"));
    app.finish();
}

#[test]
fn showcase_fixedcontrols_focused_refused() {
    let app = App::new(
        "showcase-fixed-refused",
        SHOWCASE,
        &["--page", "fixedcontrols", "--motion", "paused"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Keep awake");
    app.settle();
    app.key("tab");
    app.key("tab");
    app.key("space");
    let refused = refused_with(&app, FIXED_KEEP_AWAKE);
    assert!(row_with(&refused, "Keep awake").contains('▎'));
    assert!(text(&refused).contains("[ ] Keep awake"));
    app.visual("functional/showcase_fixedcontrols_focused_refused");
}

fn terminal_viewport(name: &str, opts: &[&str]) -> App {
    let mut args = vec!["--page", "terminalviewport", "--motion", "paused"];
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    let app = App::new(name, SHOWCASE, &args, 120, 40, "truecolor");
    app.wait_text("Terminal viewport");
    app.settle();
    app
}

page_test!(
    showcase_page_terminalviewport,
    "terminalviewport",
    "Terminal viewport"
);

fn viewport_top(screen: &TerminalSnapshot) -> Point {
    find(screen, "23 step 23 finished")
}

fn at(top: Point, col: u16, row: u16) -> Point {
    Point {
        x: top.x + col,
        y: top.y + row,
    }
}

fn status_row(screen: &TerminalSnapshot) -> String {
    let rows = text(screen).lines().count() as u32;
    row_text(screen, rows - 1)
}

#[test]
fn showcase_terminal_viewport_focus_shows_the_program_cursor() {
    let app = terminal_viewport("showcase-terminal-viewport-focus", &[]);
    let screen = app.settle();
    assert!(
        !screen.cursor().visible(),
        "an unfocused pane shows no cursor"
    );
    let top = viewport_top(&screen);
    app.click(at(top, 3, 2));
    let focused = app.wait_screen("the pane takes focus and shows its cursor", |s| {
        s.cursor().visible()
    });
    let prompt = find(&focused, "dev@jackin:~/src$");
    let cursor = focused.cursor();
    assert_eq!(
        (cursor.col(), cursor.row()),
        (
            u32::from(prompt.x) + "dev@jackin:~/src$ ".len() as u32,
            u32::from(prompt.y)
        ),
        "the hardware cursor is the program's cursor"
    );
    app.wheel(at(top, 3, 2), false, 1);
    let scrolled = app.wait_screen("scrolled back", |s| text(s).contains("20–31 of 34"));
    assert!(
        !scrolled.cursor().visible(),
        "a scrolled-back pane has no cursor"
    );
    app.key("end");
    let live = app.wait_screen("End returns to the live tail", |s| {
        text(s).contains("live") && s.cursor().visible()
    });
    assert!(text(&live).contains("23 step 23 finished"));
    app.settle();
    app.visual("functional/showcase_terminal_viewport_focused");
    app.finish();
}

#[test]
fn showcase_terminal_viewport_wheel_scrolls_and_keeps_its_place() {
    let app = terminal_viewport("showcase-terminal-viewport-wheel", &[]);
    let screen = app.settle();
    let top = viewport_top(&screen);
    assert!(text(&screen).contains("scroll      live tail"));
    app.wheel(at(top, 3, 2), false, 1);
    let scrolled = app.wait_screen("one notch moves three rows", |s| {
        text(s).contains("20 step 20 finished") && text(s).contains("20–31 of 34")
    });
    assert!(!text(&scrolled).contains("dev@jackin:~/src$"));
    assert!(text(&scrolled).contains("scroll      3 rows up"));

    app.click_text("Feed line");
    let fed = app.wait_screen("new output does not move what is read", |s| {
        text(s).contains("20–31 of 35")
    });
    assert!(text(&fed).contains("20 step 20 finished"));
    assert!(text(&fed).contains("scroll      4 rows up"));
    assert!(
        !text(&fed).contains("31 step 31 finished"),
        "the new line is below the view"
    );
    app.wheel(at(top, 3, 2), true, 2);
    let tail = app.wait_screen("the wheel returns to the live tail", |s| {
        text(s).contains("scroll      live tail")
    });
    assert!(text(&tail).contains("31 step 31 finished"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_scrolled_back_shows_its_place() {
    let app = terminal_viewport("showcase-terminal-viewport-scrolled", &[]);
    let screen = app.settle();
    let top = viewport_top(&screen);
    app.wheel(at(top, 3, 2), false, 1);
    app.wait_text("20–31 of 34");
    app.settle();
    app.visual("functional/showcase_terminal_viewport_scrolled");
    app.finish();
}

#[test]
fn showcase_terminal_viewport_copy_joins_a_wrapped_path() {
    let app = terminal_viewport("showcase-terminal-viewport-copy", &[]);
    let screen = app.settle();
    let path = find(&screen, "open /home/dev");
    app.down(at(path, 5, 0));
    app.drag_to(at(path, 20, 1));
    app.drag_to(at(path, 4, 2));
    app.up(at(path, 4, 2));
    let copied = app.wait_screen("a drag release copies the selection", |s| {
        status_row(s).contains("Copied 75 characters")
    });
    assert!(text(&copied).contains("copied      1 line · 75 characters"));
    assert!(text(&copied).contains("selection   75 characters"));
    app.settle();
    app.visual("functional/showcase_terminal_viewport_selection");

    app.key("esc");
    let cleared = app.wait_screen("Esc clears the selection", |s| {
        text(s).contains("selection   none")
    });
    assert!(text(&cleared).contains("copied      1 line · 75 characters"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_double_click_selects_a_word() {
    let app = terminal_viewport("showcase-terminal-viewport-word", &[]);
    let screen = app.settle();
    let top = viewport_top(&screen);
    let word = at(top, 14, 0);
    app.click(word);
    app.click(word);
    let selected = app.wait_screen("a double click selects the word", |s| {
        text(s).contains("selection   8 characters")
            && text(s).contains("copied      1 line · 8 characters")
    });
    assert!(status_row(&selected).contains("Copied 8 characters · finished"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_keys_page_and_clear() {
    let app = terminal_viewport("showcase-terminal-viewport-keys", &[]);
    let screen = app.settle();
    let top = viewport_top(&screen);
    app.click(at(top, 3, 2));
    app.key("pageup");
    app.wait_text("11–22 of 34");
    app.key("home");
    app.wait_text("1–12 of 34");
    app.key("pagedown");
    app.wait_text("13–24 of 34");
    app.key("end");
    let live = app.wait_screen("End returns to the live tail", |s| {
        text(s).contains("scroll      live tail")
    });
    assert!(text(&live).contains("23 step 23 finished"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_fullscreen_has_no_history_to_scroll() {
    let app = terminal_viewport(
        "showcase-terminal-viewport-fullscreen",
        &["terminalviewport.fixture=fullscreen"],
    );
    let screen = app.settle();
    assert!(text(&screen).contains("the alternate screen has no"));
    assert!(text(&screen).contains("alt screen  on"));
    let top = find(&screen, "the alternate screen has no");
    app.wheel(at(top, 3, 1), false, 2);
    app.settle();
    let after = app.settle();
    assert!(text(&after).contains("the alternate screen has no"));
    assert!(text(&after).contains("scroll      live tail"));
    assert!(!text(&after).contains("step 1"));
    app.visual("functional/showcase_terminal_viewport_fullscreen");
    app.finish();
}

#[test]
fn showcase_terminal_viewport_states() {
    let app = terminal_viewport("showcase-terminal-viewport-states", &[]);
    app.click_text("States");
    app.wait_text("overflow");
    app.settle();
    app.visual("functional/showcase_terminal_viewport_states");
    app.finish();
}

#[test]
fn showcase_terminal_viewport_follows_the_pane_size() {
    let app = terminal_viewport("showcase-terminal-viewport-size", &[]);
    let screen = app.settle();
    assert!(text(&screen).contains("grid        38x12 + track"));
    app.resize(120, 20);
    let small = app.wait_screen("the grid takes the rows the pane has", |s| {
        text(s).contains("grid        38x7 + track")
    });
    assert!(text(&small).contains("23 step 23 finished"));
    app.resize(120, 40);
    let again = app.wait_screen("and gives them back", |s| {
        text(s).contains("grid        38x12 + track")
    });
    assert!(text(&again).contains("23 step 23 finished"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_reset_drops_scroll_and_selection() {
    let app = terminal_viewport("showcase-terminal-viewport-reset", &[]);
    let screen = app.settle();
    let top = viewport_top(&screen);
    app.wheel(at(top, 3, 2), false, 2);
    app.wait_text("scroll      6 rows up");
    app.down(at(top, 0, 1));
    app.drag_to(at(top, 6, 1));
    app.up(at(top, 6, 1));
    app.wait_text("selection   6 characters");
    app.click_text("Reset");
    let fresh = app.wait_screen("Reset starts the session over", |s| {
        text(s).contains("scroll      live tail") && text(s).contains("selection   none")
    });
    assert!(text(&fresh).contains("history     22 rows"));
    assert!(text(&fresh).contains("copied      nothing yet"));
    app.finish();
}

#[test]
fn showcase_terminal_viewport_pointer_policy_is_an_option() {
    let app = terminal_viewport(
        "showcase-terminal-viewport-policy",
        &["terminalviewport.pointer_select=off"],
    );
    let screen = app.settle();
    let top = viewport_top(&screen);
    app.down(at(top, 0, 1));
    app.drag_to(at(top, 6, 1));
    app.up(at(top, 6, 1));
    app.settle();
    let after = app.settle();
    assert!(
        text(&after).contains("selection   none"),
        "with pointer selection off a drag selects nothing"
    );
    app.finish();
}

fn list_detail(name: &str, cols: u16, rows: u16) -> App {
    let app = App::new(
        name,
        SHOWCASE,
        &["--page", "listdetail", "--motion", "paused"],
        cols,
        rows,
        "truecolor",
    );
    app.wait_text("nightly-backup");
    app.settle();
    app
}

#[test]
fn showcase_page_listdetail() {
    let app = list_detail("showcase-page-listdetail", 140, 40);
    app.advance_clock(100);
    app.settle();
    app.visual("functional/showcase_page_listdetail");
    app.finish();
}

#[test]
fn showcase_listdetail_selecting_a_row_shows_its_detail_and_an_action_reports() {
    let app = list_detail("showcase-listdetail-select", 140, 40);
    let initial = app.screen();
    assert!(text(&initial).contains("volume"));
    assert!(!text(&initial).contains("CDN_KEY"));

    app.click(inside(find(&initial, "image-resize")));
    let failing = app.wait_screen("the failing job's detail shows", |s| {
        text(s).contains("CDN_KEY")
    });
    assert!(text(&failing).contains("failing"));

    app.click_text("Run now");
    let ran = app.wait_screen("the action reports", |s| text(s).contains("action run"));
    assert!(text(&ran).contains("image-resize"));
    app.finish();
}

#[test]
fn showcase_listdetail_dragging_the_detail_scrollbar_scrolls_the_detail() {
    let app = list_detail("showcase-listdetail-drag", 140, 22);
    let screen = app.settle();
    assert!(
        !text(&screen).contains("port 443 only"),
        "the last binding starts below the fold"
    );
    let thumb = find_all(&screen, "┃");
    let track = find_all(&screen, "┆");
    assert!(!thumb.is_empty(), "the overflowing detail draws a thumb");
    let top = thumb[0];
    let bottom = *track
        .iter()
        .chain(thumb.iter())
        .max_by_key(|p| p.y)
        .expect("a track");

    app.down(top);
    app.drag_to(bottom);
    app.wait_screen(
        "dragging to the bottom of the track reaches the end before the release",
        |s| text(s).contains("port 443 only"),
    );
    app.up(bottom);
    app.finish();
}

#[test]
fn showcase_listdetail_narrow_terminal_shows_the_detail_as_a_drawer() {
    let app = list_detail("showcase-listdetail-drawer", 90, 30);
    let list = app.screen();
    assert!(
        !text(&list).contains("archive.internal"),
        "below 100 columns the detail is not beside the list"
    );
    app.key("tab");
    app.key("tab");
    app.key("enter");
    let drawer = app.wait_screen("Enter opens the detail as a drawer", |s| {
        text(s).contains("archive.internal")
    });
    assert!(!text(&drawer).contains("report-mailer"));
    app.settle();
    app.visual("functional/showcase_listdetail_drawer");
    app.key("escape");
    app.wait_text("report-mailer");
    app.finish();
}

fn fixed_dialogs(name: &str, opts: &[&str]) -> App {
    let mut args = vec!["--page", "fixeddialogs", "--motion", "paused"];
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    let app = App::new(name, SHOWCASE, &args, 120, 40, "truecolor");
    app.wait_text("Choice dialog");
    app.settle();
    app
}

page_test!(showcase_page_fixeddialogs, "fixeddialogs", "Fixed dialogs");

#[test]
fn showcase_fixeddialogs_choice_starts_on_its_button_when_the_radio_is_disabled() {
    let app = fixed_dialogs("showcase-fixeddialogs-choice", &[]);
    app.click_text("Choice dialog");
    let open = app.wait_text("Where the output goes");
    assert!(
        text(&open).contains("▎Next"),
        "focus starts on the primary button"
    );
    app.key("down");
    let after = app.settle();
    assert!(
        text(&after).contains("(●) Project folder"),
        "a disabled radio keeps its option"
    );
    app.key("escape");
    app.wait_absent("Where the output goes");
    app.finish();
}

#[test]
fn showcase_fixeddialogs_live_choice_starts_on_its_radio() {
    let app = fixed_dialogs(
        "showcase-fixeddialogs-choice-live",
        &["fixeddialogs.state=live"],
    );
    app.click_text("Choice dialog");
    let open = app.wait_text("Where the output goes");
    assert!(row_with(&open, "(●) Project folder").contains('▎'));
    app.key("down");
    app.wait_screen("the live radio moves", |s| {
        text(s).contains("(●) Edit destination")
    });
    app.key("escape");
    app.wait_absent("Where the output goes");
    app.finish();
}

#[test]
fn showcase_fixeddialogs_form_does_not_offer_its_fixed_action() {
    let app = fixed_dialogs("showcase-fixeddialogs-form", &[]);
    app.click_text("Form dialog");
    app.wait_text("Plain text");
    app.key("tab");
    let on_fixed = app.settle();
    assert!(text(&on_fixed).contains("▎Plain text"));
    assert!(
        !status_row(&on_fixed).contains("Enter"),
        "no Enter hint on a fixed action: {}",
        status_row(&on_fixed)
    );
    app.key("tab");
    let on_live = app.settle();
    assert!(text(&on_live).contains("▎Check"));
    assert!(status_row(&on_live).contains("Enter Check"));
    app.key("escape");
    app.wait_absent("Plain text");
    app.finish();
}

#[test]
fn showcase_fixeddialogs_browser_box_refuses_and_shows_its_reason() {
    let app = fixed_dialogs("showcase-fixeddialogs-browser", &[]);
    app.click_text("File browser");
    let open = app.wait_text("Read-only");
    assert!(text(&open).contains("[ ] Read-only"));
    assert!(text(&open).contains("Always read-only · the host decides"));
    app.click_text("Read-only");
    let on_box = app.wait_screen("the fixed box takes focus", |s| {
        row_with(s, "[ ] Read-only").contains('▎')
    });
    assert!(
        text(&on_box).contains("[ ] Read-only"),
        "a click on a fixed box leaves it unchecked"
    );
    app.key("space");
    let refused = app.settle();
    assert!(text(&refused).contains("[ ] Read-only"));
    app.key("escape");
    app.wait_absent("Read-only");
    app.finish();
}

#[test]
fn showcase_picker_multi_select_order_toggle_and_confirm() {
    let app = App::new(
        "showcase-picker-multi",
        SHOWCASE,
        &["--page", "pickers", "--motion", "paused"],
        160,
        50,
        "truecolor",
    );
    let start = app.wait_text("Multi select");
    let multi_btn = find(&start, "Multi select");
    app.click(inside(multi_btn));
    let opened = app.wait_text("Row3");
    assert!(text(&opened).contains("Space Toggle"));
    assert!(text(&opened).contains("Enter Confirm"));
    app.key("enter");
    assert!(
        text(&app.settle()).contains("Row3"),
        "empty Enter must be a no-op"
    );
    app.key("down");
    app.key("space");
    app.key("up");
    app.key("space");
    app.key("enter");
    app.wait_text("ChoseMany: Row3, Row1");
    app.click(inside(multi_btn));
    app.wait_text("Row4");
    app.click(inside(find(&app.settle(), "Row4")));
    app.click(inside(find(&app.settle(), "Row4")));
    app.key("enter");
    assert!(
        text(&app.settle()).contains("Row4"),
        "second click must uncheck, so Enter is a no-op"
    );
    app.key("escape");
    app.wait_absent("Row4");
    app.visual("functional/showcase_picker_multi_select");
    app.finish();
}

#[test]
fn showcase_picker_multi_search_space_edits_query_and_hidden_checked_retained() {
    let app = App::new(
        "showcase-picker-multi-search",
        SHOWCASE,
        &["--page", "pickers", "--motion", "paused"],
        160,
        50,
        "truecolor",
    );
    let start = app.wait_text("Multi search");
    app.click(inside(find(&start, "Multi search")));
    app.wait_text("Row3");
    assert!(text(&app.settle()).contains("Insert Toggle"));
    app.key("down");
    app.key("down");
    app.click(inside(find(&app.settle(), "Row3")));
    app.type_text("Row1");
    let filtered = app.settle();
    assert!(text(&filtered).contains("Row1"));
    app.key("insert");
    app.key("space");
    let after_space = app.settle();
    assert!(
        !text(&after_space).contains("ChoseMany"),
        "Space in searchable picker must edit the query, not toggle or confirm"
    );
    app.key("escape");
    let unfiltered = app.wait_text("Row3");
    app.click(inside(find(&unfiltered, "Row1")));
    app.key("enter");
    let chosen = app.wait_text("ChoseMany: Row3");
    assert!(text(&chosen).contains("ChosenMany([2])"));
    assert!(text(&chosen).contains("Row3") && text(&chosen).contains("1 selected"));
    assert!(!text(&chosen).contains("Text("));
    app.visual("functional/showcase_picker_multi_search");
    app.finish();
}

#[test]
fn showcase_picker_returns_both_named_rows_selected_across_a_filter() {
    let app = App::new(
        "showcase-picker-multi-search-labels",
        SHOWCASE,
        &["--page", "pickers", "--motion", "paused"],
        160,
        50,
        "truecolor",
    );
    let start = app.wait_text("Multi search");
    app.click(inside(find(&start, "Multi search")));
    let opened = app.wait_text("Row3");
    app.click(inside(find(&opened, "Row3")));
    app.type_text("Row1");
    let filtered = app.wait_screen("Row1 query and filtered option", |screen| {
        let rows: Vec<String> = (0..screen.rows())
            .map(|row| row_text(screen, row))
            .collect();
        rows.iter().any(|line| line.contains("▎ Row1"))
            && rows
                .iter()
                .any(|line| line.contains("Row1") && line.contains("multi row 1"))
            && !rows
                .iter()
                .any(|line| line.contains("Row3") && line.contains("multi row 3"))
    });
    let row1 = find_all(&filtered, "Row1")
        .into_iter()
        .find(|point| row_text(&filtered, u32::from(point.y)).contains("multi row 1"))
        .expect("the filtered Row1 option");
    app.click(inside(row1));
    app.key("enter");

    let chosen = app.wait_text("2 selected");
    let shown = text(&chosen);
    assert!(
        !shown.contains("Text("),
        "opaque item keys leaked into the result:\n{shown}"
    );
    assert!(shown.contains("ChoseMany: Row3, Row1"), "{shown}");
    assert!(shown.contains("2 selected"), "{shown}");
    app.finish();
}

#[test]
fn showcase_picker_reports_group_conflicts_and_keeps_selection() {
    let app = App::new(
        "showcase-picker-group-conflict",
        SHOWCASE,
        &["--page", "pickers", "--motion", "paused"],
        160,
        50,
        "truecolor",
    );
    let start = app.wait_text("Multi select");
    let multi = find(&start, "Multi select");
    app.click(inside(multi));
    let opened = app.wait_text("Row3");
    let row3 = find(&opened, "Row3");
    let row4 = find(&opened, "Row4");
    app.click(inside(row3));
    app.click(inside(row4));

    let conflict = app.wait_text("Row4 conflicts with selected Row3");
    let shown = text(&conflict);
    assert!(
        shown.contains("Multi select"),
        "picker must remain open\n{shown}"
    );
    assert!(row_text(&conflict, u32::from(row3.y)).contains("✓"));
    assert!(!row_text(&conflict, u32::from(row4.y)).contains("✓"));

    app.click(inside(row3));
    let unchecked = app.settle();
    assert!(!row_text(&unchecked, u32::from(row3.y)).contains("✓"));
    app.click(inside(row4));
    let row4_selected = app.settle();
    assert!(row_text(&row4_selected, u32::from(row4.y)).contains("✓"));
    app.key("enter");
    let chose_row4 = app.wait_text("ChoseMany: Row4");
    assert!(text(&chose_row4).contains("1 selected"));

    app.click(inside(multi));
    let _reopened = app.wait_text("Row3");
    app.click(inside(row3));
    let selected_row3 = app.settle();
    app.click(inside(find(&selected_row3, "Row1")));
    app.key("enter");
    let nonconflicting = app.wait_text("ChoseMany: Row3, Row1");
    assert!(text(&nonconflicting).contains("2 selected"));
    app.finish();
}

#[test]
fn showcase_picker_nav_profiles_toggle_with_the_displayed_key() {
    for (name, option, hint, toggle) in [
        ("desktop", "pickers.nav=desktop", "Insert Toggle", "insert"),
        ("vim", "pickers.nav=vim", "Ctrl+T Toggle", "ctrl+t"),
    ] {
        let app = App::new(
            &format!("showcase-picker-nav-{name}"),
            SHOWCASE,
            &["--page", "pickers", "--motion", "paused", "--opt", option],
            160,
            50,
            "truecolor",
        );
        app.click_text("Multi search");
        let shown = app.wait_text(hint);
        assert!(text(&shown).contains("Row1"));
        app.key(toggle);
        app.key("enter");
        let selected = app.wait_text("ChoseMany: Row1");
        assert!(text(&selected).contains("1 selected"));
        app.finish();
    }
}
