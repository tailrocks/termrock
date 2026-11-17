//! Holla activity prompts, driven through the real binary.
//!
//! `parity-task-input` is `/srv/app` on `devbox` (production). Trusted project
//! actions are Deploy the release (`./deploy.sh`), Run the stubborn worker,
//! and Quick no-op (`true`). The deploy script asks for a secret password,
//! then a public confirmation. `send_input` stores secret bytes but
//! `echo_pending` rewrites that prompt with an empty answer
//! (`src/bin/holla/domain/activity.rs`).
//!
//! The open secret prompt is a live activity, so the tab and running chip
//! spin every tick. `App::visual` waits for a quiet frame that does not
//! exist. These tests assert the drawn text with `wait_screen`.

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const SCENARIO: &str = "parity-task-input";
const DIR: &str = "/srv/app";
const HOST: &str = "devbox";
const DEPLOY: &str = "Deploy the release";
/// The preview wraps this sentence, so each piece is one drawn line.
const ASKS: &str = "asks for a password";
const CONFIRM: &str = "then for confirmation";
const STUBBORN: &str = "Run the stubborn worker";
const QUICK: &str = "Quick no-op";
const SCRIPT: &str = "$ ./deploy.sh";
const PASSWORD: &str = "Password:";
const PUBLIC_PROMPT: &str = "Proceed with rollout? [y/N]";
const SECRET: &str = "h03-secret";
const PUBLIC: &str = "nope";
const REFUSED: &str = "zztyped";
const LETTER: &str = "z";
const INPUT_CHROME: &str = "typing goes to stdin";
const RELEASE: &str = "Release keyboard";
const TYPING: &str = "Typing goes to the program";
const RETURNED: &str = "Keyboard returned to holla";
const SENT_STOP: &str = "Sent ^C";
const FINISHED: &str = "The program has finished · nothing reads input";
const FOREIGN: &[&str] = &["~/work/poly", "~/work/acme", "~/work/team", "Bad Id"];

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), SCENARIO.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(DIR);
    app.wait_text(HOST);
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

fn prompt_held(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PASSWORD) && !visible.contains(SECRET)
}

fn secret_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    prompt_held(screen)
        && visible.contains(INPUT_CHROME)
        && visible.contains(RELEASE)
        && visible.contains(SCRIPT)
}

fn search(app: &App, query: &str, needle: &str) {
    app.key("escape");
    app.key("escape");
    app.wait_text("Search actions and resources");
    app.paste(query);
    app.wait_text(needle);
}

fn run_deploy(app: &App) {
    app.paste(DEPLOY);
    let listed = app.wait_text(ASKS);
    see(
        &listed,
        &[DEPLOY, DIR, HOST, "./deploy.sh", ASKS, CONFIRM],
        FOREIGN,
    );
    app.key("enter");
    app.wait_text(SCRIPT);
    app.wait_text(PASSWORD);
}

fn stop_and_return(app: &App) {
    app.key("s");
    app.wait_text("terminated (SIGTERM)");
    app.key("escape");
    // `/srv/app` is already on the activity status bar, so the return signal
    // is the finder hint, which this page does not draw.
    let back = app.wait_text("Type Search");
    see(&back, &[DIR, HOST, "Type Search"], &[PASSWORD, SECRET]);
}

#[test]
fn h03_input_route_returns_to_the_finder() {
    let app = open("h03-input-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[DIR, HOST], &[PASSWORD, SCRIPT, SECRET]);
    run_deploy(&app);
    let prompt = app.screen();
    see(
        &prompt,
        &[PASSWORD, SCRIPT, DIR, "connecting to release server"],
        &[SECRET, PUBLIC_PROMPT],
    );
    app.key("i");
    let typing = app.wait_text(TYPING);
    see(
        &typing,
        &[TYPING, INPUT_CHROME, RELEASE, PASSWORD],
        &[SECRET],
    );
    app.key("escape");
    let returned = app.wait_text(RETURNED);
    see(
        &returned,
        &[RETURNED, "Back to Here", PASSWORD, SCRIPT],
        &[RELEASE, SECRET],
    );
    stop_and_return(&app);
    let finder = app.screen();
    see(
        &finder,
        &[DEPLOY, ASKS, CONFIRM, DIR],
        &[PASSWORD, SCRIPT, SECRET, PUBLIC],
    );
    app.finish();
}

#[test]
fn h03_input_prompts_mask_secret_and_refuse_when_closed() {
    let app = open("h03-input-prompts");
    run_deploy(&app);
    app.key("i");
    app.wait_text(RELEASE);
    app.type_text(SECRET);
    let masked = app.wait_screen("secret prompt stays masked", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && secret_open(screen)
    });
    see(
        &masked,
        &[PASSWORD, SCRIPT, INPUT_CHROME, RELEASE, TYPING, DIR],
        &[SECRET, PUBLIC_PROMPT, PUBLIC],
    );

    app.resize(80, 24);
    let narrowed = app.wait_screen("secret prompt at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && prompt_held(screen)
    });
    see(&narrowed, &[PASSWORD, SCRIPT], &[SECRET]);
    app.resize(120, 40);

    let restored = app.wait_screen("secret prompt back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && secret_open(screen)
    });
    see(&restored, &[PASSWORD, SCRIPT, DIR], &[SECRET]);
    app.key("enter");
    let public = app.wait_text(PUBLIC_PROMPT);
    see(
        &public,
        &[PUBLIC_PROMPT, "authenticated", PASSWORD],
        &[SECRET, PUBLIC],
    );
    app.type_text(PUBLIC);
    let answered = app.wait_screen("public answer is visible", |screen| {
        let visible = text(screen);
        visible.contains(PUBLIC_PROMPT) && visible.contains(PUBLIC) && !visible.contains(SECRET)
    });
    see(
        &answered,
        &[PUBLIC_PROMPT, PUBLIC, "authenticated"],
        &[SECRET],
    );
    app.key("enter");
    let cancelled = app.wait_text("rollout cancelled by operator");
    see(
        &cancelled,
        &[
            PUBLIC,
            "rollout cancelled by operator",
            "authenticated",
            PASSWORD,
        ],
        &[SECRET],
    );

    search(&app, QUICK, QUICK);
    app.key("enter");
    let finished = app.wait_screen("quick no-op has finished", |screen| {
        let visible = text(screen);
        visible.contains("$ true") && visible.contains("succeeded")
    });
    see(
        &finished,
        &["$ true", QUICK, DIR],
        &[SECRET, REFUSED, PUBLIC],
    );
    app.key("i");
    let refused = app.wait_text(FINISHED);
    see(&refused, &[FINISHED, "$ true"], &[SECRET, REFUSED]);
    app.type_text(REFUSED);
    let quiet = app.wait_screen("refused text is not an output line", |screen| {
        let visible = text(screen);
        visible.contains(FINISHED) && visible.contains("$ true") && !visible.contains(REFUSED)
    });
    see(&quiet, &[FINISHED, "$ true"], &[REFUSED, SECRET]);

    search(&app, STUBBORN, "ignores SIGTERM");
    app.key("enter");
    app.wait_text("$ ./stubborn");
    app.wait_text("ignoring SIGTERM");
    app.key("i");
    let owned = app.wait_text(RELEASE);
    see(
        &owned,
        &[RELEASE, INPUT_CHROME, TYPING, "$ ./stubborn"],
        &[SECRET, REFUSED],
    );
    app.key("escape");
    let shell = app.wait_text(RETURNED);
    see(
        &shell,
        &[RETURNED, "Back to Here", "$ ./stubborn", "ignoring SIGTERM"],
        &[RELEASE, SECRET, LETTER],
    );
    app.type_text(LETTER);
    let ignored = app.wait_screen("plain letter stays out of the program", |screen| {
        let visible = text(screen);
        visible.contains("$ ./stubborn")
            && visible.contains("ignoring SIGTERM")
            && !visible.contains(LETTER)
    });
    see(
        &ignored,
        &["$ ./stubborn", "ignoring SIGTERM", "Back to Here"],
        &[LETTER, SECRET, RELEASE],
    );
    app.key("i");
    app.wait_text(RELEASE);
    app.key("ctrl+c");
    let cut = app.wait_screen("ctrl+c delivered to the task", |screen| {
        let visible = text(screen);
        visible.contains("^C") && visible.contains(SENT_STOP)
    });
    see(
        &cut,
        &["^C", SENT_STOP, "$ ./stubborn"],
        &[RELEASE, SECRET, LETTER],
    );
    app.wait_text("descendants reaped");
    app.key("escape");
    let home = app.wait_text("Type Search");
    see(
        &home,
        &[DIR, HOST, "Type Search"],
        &[SECRET, REFUSED, LETTER],
    );
    app.finish();
}

#[test]
fn h03_input_scenario_parity_task_input() {
    let app = open("h03-input-scenario");
    let landing = app.screen();
    see(&landing, &[DIR, HOST, "production"], FOREIGN);

    search(&app, DEPLOY, ASKS);
    let deploy = app.screen();
    see(
        &deploy,
        &[DEPLOY, "./deploy.sh", DIR, HOST, "devbox"],
        FOREIGN,
    );
    app.key("enter");
    let running = app.wait_text(PASSWORD);
    see(
        &running,
        &[SCRIPT, PASSWORD, "connecting to release server", DIR, HOST],
        &[
            SECRET,
            PUBLIC_PROMPT,
            "~/work/poly",
            "~/work/acme",
            "~/work/team",
            "Bad Id",
        ],
    );
    stop_and_return(&app);

    search(&app, STUBBORN, "ignores SIGTERM");
    let stubborn = app.screen();
    see(&stubborn, &[STUBBORN, "./stubborn", DIR, HOST], FOREIGN);
    app.key("escape");
    app.wait_text("Search actions and resources");

    app.paste(QUICK);
    let quick = app.wait_text(QUICK);
    see(&quick, &[QUICK, DIR, HOST], FOREIGN);
    app.finish();
}
