//! Jackin 1Password field chooser, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j04_chooser/`. The shots are the open access-denied chooser,
//! before Esc. Hard Cases keeps one account refreshing, and the accounts strip
//! repaints that spinner from the sim clock, so the locked dialog never holds
//! a frame. First-use has an empty registry and the same 1Password fixture.
//! A same-size resize does not force a frame, and the cursor column is part
//! of the hash, so each shot nudges the width and comes back.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const ACCOUNTS: &str = "Accounts › Overview";
const NEW_ACCOUNT: &str = "New account";
const CHOOSE_BUTTON: &str = "Choose…";
const NOT_CHOSEN: &str = "not chosen";
const ACCOUNT_TITLE: &str = "Choose 1Password account";
const VAULT_TITLE: &str = "Choose vault";
const ITEM_TITLE: &str = "Choose item";
const FIELD_TITLE: &str = "Choose field";
const SIGNED_IN: &str = "signed in";
const LOCKED_TITLE: &str = "1Password is locked";
const LOCKED_HINT: &str = "Unlock it in the 1Password app, then press r to retry";
const RETRY_HINT: &str = "press r to retry";
const LOADING_ACCOUNTS: &str = "loading accounts…";
const CHAINARGOS: &str = "alexey@chainargos.com";
const ENGINEERING: &str = "Engineering";
const SHARED: &str = "Shared Infra";
const NO_ACCESS: &str = "No access to vault Shared Infra";
const CHOOSE_ANOTHER: &str = "Choose another vault";
const ACCESS_DENIED: &str = "1Password access denied: no permission for vault Shared Infra";
const WRONG_SHAPE: &str = "Broken · Wrong shape";
const NOTES: &str = "notes";
const UNSUPPORTED: &str = "Field notes is not a credential: choose a concealed field";
const NO_MATCHES: &str = "No matches";
const MISSING_QUERY: &str = "zzzz-missing";
const MISSING_FIELD: &str = "1Password field not found:";
const CODEX: &str = "OpenAI · Codex Primary";
const REFERENCE: &str = "Engineering › OpenAI · Codex Primary › credential";
const CANONICAL: &str = "op://v_eng01/it_cdx01/credential";
const MASKED: &str = "••••••••…k7Qz";
const CHOSEN: &str = "Reference chosen · Validate checks provider compatibility";
const SAVED_NOTE: &str = "only the reference is saved · the value is resolved at launch";
const CANCELLED: &str = "Cancelled · nothing saved";
const WIDE: &str = "J04.chooser.chooser.120x40";
const NARROW: &str = "J04.chooser.chooser.narrow";

/// Plaintext materials from `SimOnePassword::fixture`. The URL is `endpoint_of`.
const SECRETS: &[&str] = &[
    "openai:valid-cdx01",
    "openai:valid-cdx02",
    "xai:valid-grk01",
    "anthropic:valid-ant01",
    "xai:rotated-leg01",
    "openai:throttled-thr01",
    "opencode:valid-dep01",
    "anthropic:valid-ant02",
    "opencode:valid-ocg01",
    "https://api.x.ai/v1",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j04_chooser/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    // Account, vault, item, and field loads wait `latency_ms` (260) on the sim
    // clock inside `OpFlow::begin_load`. Reduced motion keeps that clock running.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn secrets_absent(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for secret in SECRETS {
        assert!(
            !visible.contains(secret),
            "plaintext secret {secret} is on screen\n{visible}"
        );
    }
}

fn reach(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("accounts, manager, or intro", |screen| {
        let visible = text(screen);
        visible.contains(ACCOUNTS) || visible.contains(DIRECTORY) || visible.contains(INTRO)
    });
    if text(&screen).contains(INTRO) {
        app.key("space");
        return app.wait_screen("after intro", |screen| {
            let visible = text(screen);
            visible.contains(ACCOUNTS) || visible.contains(DIRECTORY)
        });
    }
    screen
}

fn open_form(app: &App) -> TerminalSnapshot {
    app.key("a");
    app.wait_screen("new account", |screen| {
        let visible = text(screen);
        visible.contains(NEW_ACCOUNT)
            && visible.contains(NOT_CHOSEN)
            && visible.contains(CHOOSE_BUTTON)
            && visible.contains("1Password reference")
    })
}

fn focus_choose(app: &App) {
    for _ in 0..12 {
        let screen = app.screen();
        let line = focused_line(&screen);
        if line.contains(CHOOSE_BUTTON) {
            return;
        }
        assert!(
            text(&screen).contains(NEW_ACCOUNT),
            "left the new-account form\n{}",
            text(&screen)
        );
        app.key("tab");
        app.wait_screen("next field", |screen| {
            let next = focused_line(screen);
            next != line || next.contains(CHOOSE_BUTTON)
        });
    }
    let screen = app.screen();
    panic!(
        "Choose… is not focused\n{}\n{}",
        focused_line(&screen),
        text(&screen)
    );
}

fn form_unchosen(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(NEW_ACCOUNT)
        && visible.contains(NOT_CHOSEN)
        && visible.contains(CHOOSE_BUTTON)
        && !visible.contains(ACCOUNT_TITLE)
        && !visible.contains(VAULT_TITLE)
        && !visible.contains(ITEM_TITLE)
        && !visible.contains(FIELD_TITLE)
        && !visible.contains(LOCKED_TITLE)
        && !visible.contains(CHOSEN)
        && !visible.contains(CANONICAL)
        && !visible.contains(REFERENCE)
}

fn accounts_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(ACCOUNT_TITLE)
        && visible.contains(CHAINARGOS)
        && visible.contains(SIGNED_IN)
        && visible.contains("authorize")
        && !visible.contains(LOADING_ACCOUNTS)
        && !visible.contains(LOCKED_TITLE)
        && !visible.contains(CHOSEN)
        && !visible.contains(CANONICAL)
}

fn vaults_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(VAULT_TITLE)
        && visible.contains(ENGINEERING)
        && visible.contains(SHARED)
        && visible.contains("no access")
        && !visible.contains(LOADING_ACCOUNTS)
        && !visible.contains("loading vaults")
        && !visible.contains(NO_ACCESS)
        && !visible.contains(CHOSEN)
}

fn denied_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(NO_ACCESS)
        && visible.contains(CHOOSE_ANOTHER)
        && visible.contains(ITEM_TITLE)
        && !visible.contains(ACCESS_DENIED)
        && !visible.contains("loading")
        && !visible.contains(CHOSEN)
        && !visible.contains(CANONICAL)
        && !visible.contains(REFERENCE)
        && !visible.contains(FIELD_TITLE)
}

fn locked_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let hint_ok = if screen.cols() >= 120 {
        visible.contains(LOCKED_HINT)
    } else {
        visible.contains(RETRY_HINT)
    };
    visible.contains(ACCOUNT_TITLE)
        && visible.contains(LOCKED_TITLE)
        && hint_ok
        && !visible.contains(LOADING_ACCOUNTS)
        && !visible.contains(SIGNED_IN)
        && !visible.contains(CHOSEN)
        && !visible.contains(CANONICAL)
        && !visible.contains(REFERENCE)
        && !visible.contains(FIELD_TITLE)
}

fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    nudge(app, quiet_frame);
    app.visual(&visual_id(checkpoint));
}

fn open_chooser(app: &App) {
    focus_choose(app);
    app.key("enter");
}

fn wait_accounts(app: &App) -> TerminalSnapshot {
    app.wait_screen("account list", accounts_ready)
}

fn assert_unchosen(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("chooser closed", form_unchosen);
    secrets_absent(&screen);
    screen
}

fn show_denied(app: &App) {
    open_chooser(app);
    wait_accounts(app);
    app.key("enter");
    app.wait_screen("vaults", vaults_ready);
    app.type_text("Shared");
    app.wait_screen("shared infra", |screen| {
        let visible = text(screen);
        visible.contains(SHARED)
            && visible.contains("no access")
            && visible.contains(VAULT_TITLE)
            && !visible.contains(ENGINEERING)
            && !visible.contains("loading")
    });
    app.key("enter");
    app.wait_screen("access denied", denied_open);
}

fn leave_denied(app: &App) {
    app.key("escape");
    app.wait_screen("back to vaults", vaults_ready);
    app.key("escape");
    wait_accounts(app);
    app.key("escape");
    let screen = assert_unchosen(app);
    assert!(
        text(&screen).contains(NOT_CHOSEN) && !text(&screen).contains(CANONICAL),
        "inaccessible vault stored a reference\n{}",
        text(&screen)
    );
}

fn close_form(app: &App) -> TerminalSnapshot {
    app.key("escape");
    let screen = app.wait_screen("accounts after cancel", |screen| {
        let visible = text(screen);
        visible.contains(ACCOUNTS)
            && visible.contains(CANCELLED)
            && !visible.contains(NEW_ACCOUNT)
            && !visible.contains(CHOSEN)
    });
    secrets_absent(&screen);
    screen
}

#[test]
fn j04_chooser_route() {
    let app = open("j04-chooser-route", "accounts-mixed");
    let opened = reach(&app);
    assert!(text(&opened).contains(ACCOUNTS), "{}", text(&opened));
    assert!(!text(&opened).contains(NEW_ACCOUNT), "{}", text(&opened));
    assert!(!text(&opened).contains(LOCKED_TITLE), "{}", text(&opened));
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    secrets_absent(&opened);

    let form = open_form(&app);
    assert!(form_unchosen(&form), "{}", text(&form));
    assert_eq!((form.cols(), form.rows()), (120, 40));

    open_chooser(&app);
    let listed = wait_accounts(&app);
    assert!(accounts_ready(&listed), "{}", text(&listed));
    assert!(
        text(&listed).contains("alexey@donbeave.dev") && text(&listed).contains("locked"),
        "locked account is missing\n{}",
        text(&listed)
    );
    secrets_absent(&listed);

    app.key("escape");
    let back = assert_unchosen(&app);
    assert!(
        text(&back).contains(NOT_CHOSEN) && !text(&back).contains(CANONICAL),
        "cancel stored a reference\n{}",
        text(&back)
    );
    let accounts = close_form(&app);
    assert!(text(&accounts).contains(ACCOUNTS), "{}", text(&accounts));
    app.finish();
}

#[test]
fn j04_chooser_chooser() {
    let app = open("j04-chooser-denied", "first-use");
    let manager = reach(&app);
    assert!(
        text(&manager).contains(DIRECTORY) || text(&manager).contains(ACCOUNTS),
        "first-use did not open\n{}",
        text(&manager)
    );
    if !text(&manager).contains(ACCOUNTS) {
        app.key("c");
        app.wait_screen("accounts", |screen| text(screen).contains(ACCOUNTS));
    }
    open_form(&app);
    show_denied(&app);
    let failure = app.wait_screen("denied wide", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && denied_open(screen)
    });
    assert!(denied_open(&failure), "access denied\n{}", text(&failure));
    secrets_absent(&failure);
    assert!(
        text(&failure).contains(NO_ACCESS)
            && text(&failure).contains(CHOOSE_ANOTHER)
            && !text(&failure).contains(CHOSEN)
            && !text(&failure).contains(CANONICAL),
        "a field was accepted\n{}",
        text(&failure)
    );
    shoot(&app, WIDE, denied_open);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && denied_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    secrets_absent(&narrow);
    shoot(&app, NARROW, denied_open);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && denied_open(screen)
    });
    leave_denied(&app);

    open_chooser(&app);
    wait_accounts(&app);
    app.key("escape");
    assert_unchosen(&app);

    open_chooser(&app);
    wait_accounts(&app);
    app.key("enter");
    let vaults = app.wait_screen("vaults", vaults_ready);
    assert!(vaults_ready(&vaults), "{}", text(&vaults));
    secrets_absent(&vaults);
    app.key("backspace");
    let backed = wait_accounts(&app);
    assert!(
        accounts_ready(&backed) && !text(&backed).contains(CANONICAL),
        "back stored a reference\n{}",
        text(&backed)
    );
    app.key("escape");
    assert_unchosen(&app);

    open_chooser(&app);
    wait_accounts(&app);
    app.key("enter");
    app.wait_screen("vaults again", vaults_ready);
    app.type_text("Shared");
    app.wait_screen("shared infra only", |screen| {
        let visible = text(screen);
        visible.contains(SHARED)
            && visible.contains("no access")
            && visible.contains(VAULT_TITLE)
            && !visible.contains(ENGINEERING)
            && !visible.contains("loading")
    });
    app.key("enter");
    let denied = app.wait_screen("access denied", |screen| {
        let visible = text(screen);
        visible.contains(NO_ACCESS)
            && visible.contains(CHOOSE_ANOTHER)
            && visible.contains(ITEM_TITLE)
    });
    assert!(
        !text(&denied).contains(ACCESS_DENIED),
        "picker painted OpError::message for permission denied\n{}",
        text(&denied)
    );
    assert!(
        !text(&denied).contains(CHOSEN) && !text(&denied).contains(CANONICAL),
        "inaccessible vault returned a field\n{}",
        text(&denied)
    );
    secrets_absent(&denied);
    app.key("escape");
    app.wait_screen("back to vaults", vaults_ready);
    app.key("escape");
    wait_accounts(&app);
    app.key("escape");
    assert_unchosen(&app);

    open_chooser(&app);
    wait_accounts(&app);
    app.key("enter");
    app.wait_screen("vaults for items", vaults_ready);
    app.key("enter");
    app.wait_screen("items", |screen| {
        let visible = text(screen);
        visible.contains(ITEM_TITLE)
            && visible.contains(CODEX)
            && visible.contains(WRONG_SHAPE)
            && !visible.contains("loading")
    });
    app.type_text("Wrong shape");
    app.wait_screen("wrong shape only", |screen| {
        let visible = text(screen);
        visible.contains(WRONG_SHAPE) && visible.contains(ITEM_TITLE) && !visible.contains(CODEX)
    });
    app.key("enter");
    let fields = app.wait_screen("unsupported field", |screen| {
        let visible = text(screen);
        visible.contains(FIELD_TITLE) && visible.contains(NOTES) && !visible.contains("loading")
    });
    assert!(
        !text(&fields).contains(UNSUPPORTED),
        "disabled notes called describe\n{}",
        text(&fields)
    );
    secrets_absent(&fields);
    app.key("enter");
    assert!(
        app.try_sustained(Duration::from_millis(400), |screen| {
            let visible = text(screen);
            visible.contains(FIELD_TITLE)
                && visible.contains(NOTES)
                && !visible.contains(UNSUPPORTED)
                && !visible.contains(CHOSEN)
                && !visible.contains(CANONICAL)
        }),
        "unsupported field was accepted\n{}",
        text(&app.screen())
    );
    app.type_text(MISSING_QUERY);
    let missing = app.wait_screen("missing field query", |screen| {
        let visible = text(screen);
        visible.contains(NO_MATCHES)
            && visible.contains(FIELD_TITLE)
            && visible.contains(MISSING_QUERY)
    });
    assert!(
        !text(&missing).contains(MISSING_FIELD),
        "a filtered miss called reference\n{}",
        text(&missing)
    );
    app.key("enter");
    assert!(
        app.try_sustained(Duration::from_millis(400), |screen| {
            let visible = text(screen);
            visible.contains(NO_MATCHES)
                && visible.contains(FIELD_TITLE)
                && !visible.contains(MISSING_FIELD)
                && !visible.contains(CANONICAL)
        }),
        "missing field was accepted\n{}",
        text(&app.screen())
    );
    secrets_absent(&app.screen());
    app.key("escape");
    app.wait_screen("query cleared", |screen| {
        text(screen).contains(NOTES)
            && text(screen).contains(FIELD_TITLE)
            && !text(screen).contains(NO_MATCHES)
            && !text(screen).contains("loading")
    });
    app.key("escape");
    app.wait_screen("back to items", |screen| {
        let visible = text(screen);
        visible.contains(ITEM_TITLE)
            && visible.contains(WRONG_SHAPE)
            && !visible.contains("loading")
            && !visible.contains(FIELD_TITLE)
    });
    app.key("escape");
    app.wait_screen("back to vaults from items", vaults_ready);
    app.key("escape");
    wait_accounts(&app);
    app.key("escape");
    assert_unchosen(&app);

    open_chooser(&app);
    wait_accounts(&app);
    app.key("enter");
    app.wait_screen("vaults for choice", vaults_ready);
    app.key("enter");
    app.wait_screen("codex item", |screen| {
        text(screen).contains(ITEM_TITLE) && text(screen).contains(CODEX)
    });
    app.key("enter");
    app.wait_screen("credential field", |screen| {
        let visible = text(screen);
        visible.contains(FIELD_TITLE)
            && visible.contains("credential")
            && visible.contains("concealed")
    });
    app.key("enter");
    let chosen = app.wait_screen("reference chosen", |screen| {
        let visible = text(screen);
        visible.contains(CHOSEN)
            && visible.contains(REFERENCE)
            && visible.contains(CANONICAL)
            && (visible.contains(MASKED) || visible.contains("credential ·"))
            && visible.contains(SAVED_NOTE)
            && !visible.contains(NOT_CHOSEN)
            && !visible.contains(FIELD_TITLE)
    });
    secrets_absent(&chosen);
    assert!(
        text(&chosen).contains("chainargos.1password.com"),
        "account id missing\n{}",
        text(&chosen)
    );
    app.key("escape");
    let cancelled = app.wait_screen("form cancelled", |screen| {
        text(screen).contains(ACCOUNTS) && text(screen).contains(CANCELLED)
    });
    assert!(
        !text(&cancelled).contains(CANONICAL) && !text(&cancelled).contains(CHOSEN),
        "cancel kept the new reference on the accounts screen\n{}",
        text(&cancelled)
    );
    secrets_absent(&cancelled);
    app.finish();

    let locked = open("j04-chooser-locked", "hard-cases");
    let manager = reach(&locked);
    assert!(
        text(&manager).contains(DIRECTORY),
        "hard-cases did not open the manager\n{}",
        text(&manager)
    );
    locked.key("c");
    let accounts = locked.wait_screen("locked accounts", |screen| text(screen).contains(ACCOUNTS));
    assert!(
        text(&accounts).contains("1Password locked") && !text(&accounts).contains(LOCKED_TITLE),
        "overview copy is not the chooser\n{}",
        text(&accounts)
    );
    open_form(&locked);
    open_chooser(&locked);
    let failure = locked.wait_screen("locked chooser", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && locked_open(screen)
    });
    assert!(
        text(&failure).contains(LOCKED_HINT)
            && !text(&failure).contains(CHOSEN)
            && !text(&failure).contains(CANONICAL),
        "locked chooser accepted a field\n{}",
        text(&failure)
    );
    secrets_absent(&failure);
    locked.key("escape");
    let dismissed = assert_unchosen(&locked);
    assert!(
        text(&dismissed).contains(NOT_CHOSEN),
        "dismiss stored a reference\n{}",
        text(&dismissed)
    );
    open_chooser(&locked);
    locked.wait_screen("locked again", locked_open);
    locked.key("r");
    locked.wait_screen("retry loading", |screen| {
        text(screen).contains(LOADING_ACCOUNTS) && !text(screen).contains(LOCKED_TITLE)
    });
    let retried = locked.wait_screen("locked after retry", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && locked_open(screen)
    });
    secrets_absent(&retried);
    assert!(
        text(&retried).contains(LOCKED_TITLE) && !text(&retried).contains(CANONICAL),
        "retry stored a reference\n{}",
        text(&retried)
    );
    locked.key("escape");
    assert_unchosen(&locked);
    close_form(&locked);
    locked.finish();
}
