//! TablePro connection form, advanced fields and masked password.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t01_advanced/` and each case id's dots become underscores.
//! TablePro has no `--page` flag. It opens on the connections screen.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const SECRET: &str = "qx7T01advSECRET";
const MASK: &str = "•••••••••••••••";
const SAVED_HOST: &str = "gate.acme.io";
const CANCEL_HOST: &str = "drop.acme.io";
const LEAVE_HOST: &str = "leave.acme.io";
const STORED_HOST: &str = "bastion.acme.io";
const SSH_USER: &str = "notdeploy";
const STARTUP: &str = "extra_float_digits=0";
const SAVED: &str = "Connection saved";
const PLACEHOLDER: &str = "stored in the keychain";
const HELP: &str = "Never written to connections.json";

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/t01_advanced/{}",
        case_id.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
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

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn row_after(screen: &TerminalSnapshot, needle: &str) -> String {
    let index = (0..screen.rows())
        .find(|row| row_text(screen, *row).contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)));
    row_text(screen, index + 1)
}

fn password_masked(screen: &TerminalSnapshot) -> bool {
    let Some(index) = (0..screen.rows()).find(|row| row_text(screen, *row).contains("Password"))
    else {
        return false;
    };
    let next = index + 1;
    next < screen.rows() && row_text(screen, next).contains(MASK) && !text(screen).contains(SECRET)
}

fn click_above_footer(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let limit = u16::try_from(screen.rows()).expect("rows") - 1;
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.y < limit)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one {label:?} above the footer, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_label(app: &App, label: &str) {
    click_above_footer(app, label);
}

fn select_production(app: &App) {
    click_label(app, "Production");
    let detail = app.wait_text("prod-db-1.acme.io");
    see(&detail, &[STORED_HOST, "Production"], &[]);
}

fn edit(app: &App) {
    app.key("e");
    let form = app.wait_text("Edit connection");
    see(&form, &["Basic", "Advanced", "Password"], &[SECRET]);
}

fn advanced_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("SSH host")
        && visible.contains("SSH user")
        && visible.contains("Startup commands")
        && visible.contains("Local only")
        && !visible.contains("Password")
}

fn basic_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("Password") && visible.contains("Username") && !visible.contains("SSH host")
}

fn advanced(app: &App) {
    click_label(app, "Advanced");
    // The tab rule paints before the advanced column replaces the basic fields.
    let tab = app.wait_screen("advanced fields", advanced_ready);
    see(
        &tab,
        &[
            "Use SSL / TLS",
            "SSH host",
            "SSH user",
            "Startup commands",
            "Local only",
        ],
        &["Password"],
    );
}

fn basic(app: &App) {
    click_label(app, "Basic");
    app.wait_screen("basic fields", basic_ready);
}

fn replace_field(app: &App, label: &str, value: &str) {
    click_label(app, label);
    app.key("ctrl+l");
    app.paste(value);
    app.wait_text(value);
    app.key("enter");
}

fn toggle_shows(app: &App, label: &str, state: &str) {
    let other = if state == "on" { "off" } else { "on" };
    app.wait_screen(&format!("{label} is {state}"), |screen| {
        let line = row_with(screen, label);
        let at = line.find(label).expect("label");
        let tail = &line[at + label.len()..];
        tail.contains(state) && !tail.contains(other)
    });
}

fn no_secret(screen: &TerminalSnapshot) {
    assert!(
        !text(screen).contains(SECRET),
        "plaintext password is visible\n{}",
        text(screen)
    );
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

#[test]
fn route_opens_advanced_and_cancel_returns() {
    let app = open("t01-advanced-route");
    select_production(&app);
    edit(&app);
    advanced(&app);
    let fields = app.screen();
    see(
        &fields,
        &[STORED_HOST, "deploy", "Basic / Advanced"],
        &[SAVED, SECRET],
    );
    toggle_shows(&app, "Use SSL / TLS", "on");
    toggle_shows(&app, "SSH tunnel", "on");
    click_above_footer(&app, "Cancel");
    let back = app.wait_absent("Edit connection");
    see(
        &back,
        &["Filter connections", "prod-db-1.acme.io", STORED_HOST],
        &[SAVED, "Edit connection", SECRET],
    );
    app.finish();
}

#[test]
fn secrets_mask_password_and_round_trip_ssh_host() {
    let app = open("t01-advanced-secrets");
    select_production(&app);

    edit(&app);
    advanced(&app);
    replace_field(&app, "SSH host", CANCEL_HOST);
    click_above_footer(&app, "Cancel");
    let cancelled = app.wait_absent("Edit connection");
    see(
        &cancelled,
        &["Filter connections", STORED_HOST],
        &[SAVED, CANCEL_HOST, SECRET],
    );

    edit(&app);
    advanced(&app);
    replace_field(&app, "SSH host", LEAVE_HOST);
    app.key("escape");
    let leave = app.wait_text("Unsaved changes");
    see(&leave, &["Discard"], &[SAVED, SECRET]);
    click_above_footer(&app, "Discard");
    let discarded = app.wait_absent("Unsaved changes");
    see(
        &discarded,
        &["Filter connections", STORED_HOST],
        &[SAVED, "Edit connection", LEAVE_HOST, SECRET],
    );

    edit(&app);
    advanced(&app);
    replace_field(&app, "SSH host", SAVED_HOST);
    replace_field(&app, "SSH user", SSH_USER);
    click_label(&app, "Startup commands");
    app.paste(STARTUP);
    app.wait_text(STARTUP);
    click_label(&app, "Local only");
    toggle_shows(&app, "Local only", "on");
    let edited = app.screen();
    see(
        &edited,
        &[SAVED_HOST, SSH_USER, STARTUP],
        &[STORED_HOST, SAVED, SECRET],
    );
    let user = row_after(&edited, "SSH user");
    assert!(
        user.contains(SSH_USER) && !user.replace(SSH_USER, "").contains("deploy"),
        "ssh user did not replace deploy: {user:?}\n{}",
        text(&edited)
    );

    basic(&app);
    click_label(&app, "Password");
    app.paste(SECRET);
    let masked = app.wait_screen("password is masked", |screen| {
        row_after(screen, "Password").contains(MASK) && !text(screen).contains(SECRET)
    });
    see(
        &masked,
        &[MASK, HELP, "Edit connection"],
        &[SECRET, PLACEHOLDER],
    );
    no_secret(&masked);
    flash_settle(&app);
    let settled = app.screen();
    no_secret(&settled);
    assert!(
        row_after(&settled, "Password").contains(MASK),
        "mask left the password row\n{}",
        text(&settled)
    );
    shot(&app, "T01.advanced.secrets.120x40");
    no_secret(&app.screen());

    app.resize(80, 24);
    app.wait_screen("narrow size", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    // The Basic column keeps its wide scroll offset, so the focused password
    // sits below the 24-row viewport until focus leaves and returns.
    app.key("tab");
    app.key("shift+tab");
    let narrow = app.wait_screen("narrow masked password", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && password_masked(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(
        row_after(&narrow, "Password").contains(MASK),
        "mask left the password row\n{}",
        text(&narrow)
    );
    no_secret(&narrow);
    shot(&app, "T01.advanced.secrets.narrow");
    no_secret(&app.screen());

    app.resize(120, 40);
    let wide = app.wait_screen("wide restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40
    });
    no_secret(&wide);

    app.key("ctrl+s");
    let saved = app.wait_text(SAVED);
    see(
        &saved,
        &[SAVED, SAVED_HOST, "Filter connections"],
        &[SECRET, "Edit connection", STORED_HOST, SSH_USER, STARTUP],
    );
    no_secret(&saved);

    edit(&app);
    let reopened = app.wait_text(PLACEHOLDER);
    see(&reopened, &[PLACEHOLDER, HELP, "Password"], &[SECRET, MASK]);
    no_secret(&reopened);
    assert!(
        !row_after(&reopened, "Password").contains('•'),
        "reopened password is not empty\n{}",
        text(&reopened)
    );
    advanced(&app);
    let again = app.wait_text(SAVED_HOST);
    see(
        &again,
        &[SAVED_HOST, "deploy"],
        &[SECRET, SSH_USER, STARTUP, STORED_HOST],
    );
    toggle_shows(&app, "Local only", "off");
    toggle_shows(&app, "SSH tunnel", "on");
    no_secret(&app.screen());
    click_above_footer(&app, "Cancel");
    let closed = app.wait_absent("Edit connection");
    see(
        &closed,
        &["Filter connections", SAVED_HOST],
        &[SECRET, SSH_USER, STARTUP],
    );
    app.finish();
}
