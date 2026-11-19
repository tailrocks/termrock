//! The Capsule over terminal panes, in the scenario `capsule-terminal`: its
//! seven tabs hold a pane in each link state, over scripted links, and the
//! looks that belong to them are the baselines here.

use super::support::{App, text};

const JACKIN: &str = env!("CARGO_BIN_EXE_jackin-preview");

fn capsule(name: &str) -> App {
    App::new(
        name,
        JACKIN,
        &[
            "--scenario",
            "capsule-terminal",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    )
}

/// The first frame, in which no pane has a size yet: the clock may move on
/// only once the Capsule has drawn and so knows where its panes are.
fn drawn(app: &App) {
    let first = app.wait_text("Waiting for room");
    assert!(text(&first).contains("1 live"), "{}", text(&first));
}

/// The screen's text without the box lines, so a sentence that a dialog
/// wrapped reads as one.
fn flat(screen: &tui_test::TerminalSnapshot) -> String {
    text(screen)
        .chars()
        .filter(|c| !"│┆╭╮╰╯─━".contains(*c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn detach(app: &App) {
    chord(app, "d");
    app.wait_text("Detached · payments-platform keeps running");
}

fn chord(app: &App, key: &str) {
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key(key);
}

#[test]
fn jackin_terminal_live_tab_dialogs_and_info() {
    let app = capsule("jackin-terminal-live");
    drawn(&app);
    app.advance_clock(1_000);
    let live = app.wait_text("gpt-5-codex");
    for line in ["日本語のコメントは残します", "top · 3 tasks"] {
        assert!(text(&live).contains(line), "{line:?}:\n{}", text(&live));
    }
    let strip = text(&live);
    for tab in [
        "live",
        "pending",
        "attaching",
        "viewing",
        "detached",
        "exited",
        "failed",
    ] {
        assert!(strip.contains(tab), "tab {tab}:\n{strip}");
    }

    chord(&app, "x");
    let codex = app.wait_text("Codex keeps running in the sandbox.");
    assert!(
        flat(&codex)
            .contains("Close pane? Codex keeps running in the sandbox. This pane detaches."),
        "{}",
        text(&codex)
    );
    app.visual("functional/jackin_close_pane_codex");
    app.key("esc");
    app.wait_absent("Close pane?");

    chord(&app, "l");
    chord(&app, "x");
    let shell = app.wait_text("The shell ends.");
    assert!(
        flat(&shell).contains("Close pane? The shell ends. Unsaved state in it is lost."),
        "{}",
        text(&shell)
    );
    app.visual("functional/jackin_close_pane_shell");
    app.key("esc");
    app.wait_absent("Close pane?");

    chord(&app, "&");
    let tab = app.wait_text("Closes every pane in this tab.");
    assert!(
        flat(&tab).contains(
            "Close tab? Closes every pane in this tab. Codex panes keep running; shells end."
        ),
        "{}",
        text(&tab)
    );
    app.visual("functional/jackin_close_tab");
    app.key("esc");
    app.wait_absent("Close tab?");

    chord(&app, "h");
    app.paste("git status\ngit diff\ncargo test");
    let paste = app.wait_text("Paste 3 lines?");
    assert!(
        flat(&paste).contains(
            "Paste 3 lines? This pane has no paste protection, so each line runs as you paste it."
        ),
        "{}",
        text(&paste)
    );
    app.visual("functional/jackin_paste_confirm");
    app.key("esc");
    app.wait_absent("Paste 3 lines?");

    chord(&app, "i");
    let info = app.wait_text("payments-platform · gpt-5-codex");
    assert!(text(&info).contains("Title"), "{}", text(&info));
    app.visual("functional/jackin_pane_info_title");
    app.key("esc");
    detach(&app);
    app.finish();
}

#[test]
fn jackin_terminal_pending_attaching_and_viewing_panes() {
    let app = capsule("jackin-terminal-states-a");
    drawn(&app);
    app.advance_clock(1_000);
    app.wait_text("gpt-5-codex");

    chord(&app, "n");
    app.advance_clock(1_500);
    // "Pane too small" is on screen as soon as the tab opens, before the clock
    // step has been handled, so it cannot tell that the step has run. The pane
    // that has room leaves "Attaching" only on that step: wait for both.
    app.wait_screen("the pending pane and the shell it sits beside", |screen| {
        let screen = text(screen);
        screen.contains("Pane too small · zoom with Ctrl+B z") && screen.contains("dev@sandbox")
    });
    app.visual("functional/jackin_pane_pending");
    chord(&app, "z");
    app.advance_clock(300);
    let zoomed = app.wait_text("Attaching to Codex…");
    assert!(
        !text(&zoomed).contains("Pane too small"),
        "zoom gave it room and it started:\n{}",
        text(&zoomed)
    );
    chord(&app, "z");

    chord(&app, "n");
    app.advance_clock(6_000);
    let attaching = app.wait_text("Attaching to Codex…");
    assert!(
        text(&attaching).contains("Ctrl+B x cancels"),
        "{}",
        text(&attaching)
    );
    app.visual("functional/jackin_pane_attaching");

    chord(&app, "n");
    app.advance_clock(1_500);
    let viewing = app.wait_text("Read-only · another client controls this pane");
    assert!(text(&viewing).contains("(viewing)"), "{}", text(&viewing));
    app.visual("functional/jackin_pane_viewing");
    detach(&app);
    app.finish();
}

#[test]
fn jackin_terminal_detached_exited_and_failed_panes() {
    let app = capsule("jackin-terminal-states-b");
    drawn(&app);
    app.advance_clock(1_000);
    app.wait_text("gpt-5-codex");

    chord(&app, "5");
    app.advance_clock(2_500);
    let detached = app.wait_text("Detached from Codex · press Enter to reattach");
    assert!(
        text(&detached).contains("gpt-5-codex"),
        "the last screen stays"
    );
    app.visual("functional/jackin_pane_detached");
    app.key("enter");
    app.advance_clock(6_000);
    let again = app.wait_absent("Detached from Codex");
    assert!(text(&again).contains("gpt-5-codex"), "{}", text(&again));

    chord(&app, "6");
    app.advance_clock(2_500);
    let exited =
        app.wait_text("Codex exited (code 0) · press Enter to restart · the sandbox keeps running");
    assert!(text(&exited).contains("gpt-5-codex"));
    app.visual("functional/jackin_pane_exited");

    chord(&app, "7");
    app.advance_clock(300);
    let failed = app.wait_text("could not attach to Codex");
    for line in [
        "Press Enter to retry · could not attach to Codex: the gateway is unreachable · run jackin doctor",
        "Codex is not attached",
    ] {
        assert!(text(&failed).contains(line), "{line:?}:\n{}", text(&failed));
    }
    app.visual("functional/jackin_pane_failed");

    chord(&app, "6");
    app.key("enter");
    app.advance_clock(300);
    let attaching = app.wait_text("Attaching to Codex…");
    assert!(
        !text(&attaching).contains("Codex exited"),
        "Enter started Codex again:\n{}",
        text(&attaching)
    );
    app.advance_clock(1_000);
    let restarted = app.wait_absent("Attaching to Codex…");
    assert!(
        text(&restarted).contains("gpt-5-codex"),
        "{}",
        text(&restarted)
    );
    detach(&app);
    app.finish();
}

#[test]
fn jackin_terminal_task_panel_shows_task_and_statuses() {
    let app = capsule("jackin-terminal-task");
    drawn(&app);
    app.advance_clock(1_000);
    app.wait_text("gpt-5-codex");
    chord(&app, "i");
    let info = app.wait_text("Ship the multi-agent launch");
    assert!(text(&info).contains("working"), "{}", text(&info));
    assert!(text(&info).contains("OMP"), "{}", text(&info));
    app.visual("functional/jackin_terminal_task_panel");
    app.key("esc");
    detach(&app);
    app.finish();
}
