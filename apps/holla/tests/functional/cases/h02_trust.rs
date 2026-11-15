//! Holla trust review, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h02_trust/` and each checkpoint's dots become underscores.
//! `monorepo-child` opens the untrusted project config and the untrusted
//! child mise file. The checkpoints are the open custom-config dialog, which
//! is the page that shows the digest.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

/// sha256 of the `monorepo-child` `.holla.toml` body `seed_custom` writes.
const DIGEST: &str = "4abea82557e55d9a5a325425792e1569110e81b25e7c6f1930f539e33978b1c5";
const FILE: &str = "~/work/acme/.holla.toml";
const TITLE: &str = "Trust ~/work/acme/.holla.toml?";
const COMMAND: &str = "tools/deploy-preview.sh --env preview";
const CONFIG_DIR: &str = "~/work/acme";
const MISE_TITLE: &str = "Trust ~/work/acme/apps/frontend/mise.toml?";
const MISE_FILE: &str = "~/work/acme/apps/frontend/mise.toml";
const MISE_DIR: &str = "~/work/acme/apps/frontend";
const MISE_COMMAND: &str = "$ mise run //apps/frontend:test";
const FOCUS: &str = "▎";
const WIDE: &str = "H02.trust.trust.120x40";
const NARROW: &str = "H02.trust.trust.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h02_trust/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = ["--scenario", "monorepo-child"]
        .into_iter()
        .map(str::to_string)
        .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("From acme · the parent root");
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

fn shown_digest(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let rest = visible
        .split_once("sha256:")
        .unwrap_or_else(|| panic!("no digest\n{visible}"))
        .1;
    let mut hex = String::new();
    for c in rest.chars() {
        if hex.len() == 64 {
            break;
        }
        if c.is_ascii_hexdigit() {
            hex.push(c);
        } else if c.is_ascii_alphabetic() {
            break;
        }
    }
    hex
}

fn cancel_focused(screen: &TerminalSnapshot) -> bool {
    let marked = format!("{FOCUS}Cancel");
    let other = format!("{FOCUS}Trust");
    (0..screen.rows()).any(|row| row_text(screen, row).contains(&marked))
        && !(0..screen.rows()).any(|row| row_text(screen, row).contains(&other))
}

fn trust_focused(screen: &TerminalSnapshot) -> bool {
    let marked = format!("{FOCUS}Trust");
    let other = format!("{FOCUS}Cancel");
    (0..screen.rows()).any(|row| row_text(screen, row).contains(&marked))
        && !(0..screen.rows()).any(|row| row_text(screen, row).contains(&other))
}

fn assert_custom_open(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "trust dialog size\n{}",
        text(screen)
    );
    assert_eq!(shown_digest(screen), DIGEST, "{}", text(screen));
    see(
        screen,
        &[
            TITLE,
            FILE,
            "Trust this file",
            "the whole file by content digest",
            "2 custom actions · 1 destructive · 0 diagnostics",
        ],
        &[
            MISE_TITLE,
            "19 passed",
            "no simulated outcome",
            "[tasks.test]",
        ],
    );
    if cols >= 100 {
        see(
            screen,
            &[
                "\"tools/deploy-preview.sh\"",
                "\"--env\"",
                "\"preview\"",
                "run deploy.preview exactly as declared",
            ],
            &[],
        );
    }
    assert!(
        cancel_focused(screen),
        "Cancel is not the focused button\n{}",
        text(screen)
    );
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. Below about 100 columns the header omits the size chip. The nudge
/// also leaves the hidden terminal cursor on the redraw, instead of on
/// whichever footer cell the previous diff wrote last.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    if cols >= 100 {
        app.wait_text(&format!("{nudged}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow nudge", |screen| {
            screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        app.wait_screen("narrow restored", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        });
    }
}

/// The shot compares the settled frame. It does not resize.
fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn custom_drawn(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    // At 120 columns the status bar paints `· 120×40`. The PTY grid changes
    // first, and the 80-column dialog still matches the title and digest.
    // That narrow dialog omits the size chip, so it must not still say 120×40.
    let painted = if cols >= 100 {
        visible.contains(&format!("· {cols}×{rows}"))
    } else {
        !visible.contains("· 120×40")
    };
    screen.cols() == cols
        && screen.rows() == rows
        && painted
        && visible.contains(TITLE)
        && visible.contains(FILE)
        && visible.contains("sha256:")
        && visible.contains("Trust this file")
        && visible.contains("the whole file by content digest")
}

fn open_custom(app: &App) {
    app.type_text("Deploy preview");
    let listed = app.wait_text("trust required ·");
    see(
        &listed,
        &["Deploy preview", FILE, CONFIG_DIR],
        &[TITLE, "sha256:", "19 passed"],
    );
    app.key("enter");
    let dialog = app.wait_screen("custom trust dialog", |screen| {
        custom_drawn(screen, 120, 40)
    });
    assert_custom_open(&dialog, 120, 40);
}

fn open_mise(app: &App) {
    app.type_text("test");
    let results = app.wait_text("alias test");
    assert!(text(&results).contains("Run tests"), "{}", text(&results));
    app.key("enter");
    // The title paints before the definition lists [tasks.test]. The
    // checkpoint is that later frame, not the first title.
    let dialog = app.wait_screen("mise trust dialog", |screen| {
        let visible = text(screen);
        visible.contains(MISE_TITLE)
            && visible.contains("[tasks.test]")
            && visible.contains("Trust this file")
            && cancel_focused(screen)
    });
    see(
        &dialog,
        &[
            MISE_TITLE,
            MISE_FILE,
            "[tasks.test]",
            "this exact file · not the folder, not descendants",
            "Trust this file",
            "re-resolve and run //apps/frontend:test",
        ],
        &["sha256:", "19 passed", TITLE],
    );
    assert!(
        cancel_focused(&dialog),
        "Cancel is not focused\n{}",
        text(&dialog)
    );
}

fn cancel_mise(app: &App) {
    app.key("escape");
    let cancelled = app.wait_absent(MISE_TITLE);
    see(
        &cancelled,
        &["Run tests", "Not trusted · the task did not run"],
        &["19 passed", "sha256:", MISE_COMMAND],
    );
}

fn confirm_mise_child_directory(app: &App) {
    app.key("enter");
    // The title returns before the definition paints [tasks.test].
    let again = app.wait_screen("mise trust dialog again", |screen| {
        let visible = text(screen);
        visible.contains(MISE_TITLE) && visible.contains("[tasks.test]")
    });
    see(
        &again,
        &[MISE_FILE, "[tasks.test]"],
        &["sha256:", "19 passed"],
    );
    app.key("right");
    let armed = app.wait_screen("trust file focused", trust_focused);
    see(&armed, &[MISE_TITLE, "Trust this file"], &["19 passed"]);
    app.key("enter");
    let args = app.wait_text("arguments");
    see(
        &args,
        &[
            "Trusted ~/work/acme/apps/frontend/mise.toml · re-resolving the task",
            "mise run //apps/frontend:test",
        ],
        &[MISE_TITLE, "19 passed", TITLE],
    );
    app.key("ctrl+s");
    let launched = app.wait_screen("frontend test has its own activity and output", |screen| {
        let visible = text(screen);
        visible
            .lines()
            .any(|line| line.trim_start().starts_with("frontend test ·"))
            && visible.contains("╭─ output")
    });
    let launched_text = text(&launched);
    assert!(
        launched_text
            .lines()
            .any(|line| line.trim_start().starts_with(MISE_COMMAND)),
        "{launched_text}"
    );
    let activity_header = launched_text
        .lines()
        .find(|line| line.trim_start().starts_with("frontend test ·"))
        .expect("the activity header");
    let directory = activity_header
        .split("here ·")
        .nth(1)
        .and_then(|location| location.split('·').next())
        .expect("activity header states its working directory")
        .trim();
    assert_eq!(directory, MISE_DIR);
    app.wait_text("19 passed (19)");
    app.wait_text("succeeded");
}

#[test]
fn holla_trust_route_returns_to_the_child_task() {
    let app = open("h02-trust-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["From acme · the parent root"],
        &[MISE_TITLE, TITLE],
    );
    open_mise(&app);
    cancel_mise(&app);
    app.finish();
}

#[test]
fn holla_trust_cancel_then_confirm_applies_to_the_reviewed_file() {
    let app = open("h02-trust-review");
    open_custom(&app);
    flash_settle(&app);
    let wide = app.wait_screen("settled wide trust dialog", |screen| {
        custom_drawn(screen, 120, 40)
    });
    assert_custom_open(&wide, 120, 40);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrow = app.wait_screen("custom trust dialog at 80x24", |screen| {
        custom_drawn(screen, 80, 24)
    });
    assert_custom_open(&narrow, 80, 24);
    flash_settle(&app);
    let narrow = app.wait_screen("settled narrow trust dialog", |screen| {
        custom_drawn(screen, 80, 24)
    });
    assert_custom_open(&narrow, 80, 24);
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("custom trust dialog back at 120x40", |screen| {
        custom_drawn(screen, 120, 40)
    });
    assert_custom_open(&restored, 120, 40);

    app.key("escape");
    let cancelled = app.wait_absent(TITLE);
    see(
        &cancelled,
        &["Deploy preview", "Not trusted · the task did not run"],
        &[
            TITLE,
            "sha256:",
            "no simulated outcome",
            "exit 127",
            "deploy preview ·",
        ],
    );

    app.key("enter");
    let again = app.wait_screen("custom trust dialog reopened", |screen| {
        custom_drawn(screen, 120, 40)
    });
    assert_custom_open(&again, 120, 40);
    app.key("right");
    let armed = app.wait_screen("trust this file focused", trust_focused);
    see(&armed, &[TITLE, FILE, "Trust this file"], &["exit 127"]);
    assert_eq!(shown_digest(&armed), DIGEST);
    app.key("enter");
    let ran = app.wait_screen("reviewed command ran in the config directory", |screen| {
        let visible = text(screen);
        visible.contains("exit 127") && visible.contains(COMMAND)
    });
    let ran_text = text(&ran);
    let header = ran_text
        .lines()
        .find(|line| line.contains("deploy preview ·"))
        .expect("custom activity header");
    assert!(
        header.contains(&format!("parent · {CONFIG_DIR}")),
        "header {header}"
    );
    assert!(
        !header.contains("apps/frontend"),
        "confirm used the child directory: {header}"
    );
    see(
        &ran,
        &[COMMAND, "parent · ~/work/acme", "exit 127"],
        &[MISE_TITLE, "19 passed", MISE_COMMAND],
    );

    app.key("escape");
    app.wait_screen("finder after the reviewed run", |screen| {
        let visible = text(screen);
        !visible.contains("deploy preview ·") && visible.contains("Deploy preview")
    });
    app.key("escape");
    app.wait_text("From acme · the parent root");
    open_mise(&app);
    cancel_mise(&app);
    confirm_mise_child_directory(&app);
    app.finish();
}
