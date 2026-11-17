//! Lane `shell` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:shell` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_shell.rs`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use junie_tui::clock::{Moment, TestRequest};
use junie_tui::core::event::{Input, Key, Mouse, MouseKind, Outcome};
use junie_tui::core::focus::{Focus, FocusChange, FocusRing};
use junie_tui::core::id::WidgetId;
use junie_tui::core::keymap::KeyBinding;
use junie_tui::core::pointer::{PointerHost, PointerParts, PointerRouter};
use junie_tui::core::refusal::Reason;
use junie_tui::core::response::Response;
use junie_tui::motion::Motion;
use junie_tui::runtime::{
    Application, Coalesce, DRAIN_BUDGET, ExternalReceiver, ExternalSender, FocusClickGuard,
    InputPump, SizeWatch, drain_ready_inputs, external_channel,
};
use junie_tui::shell::clipboard::{self, Clipboard, Copied, NativeClipboard};
use junie_tui::shell::reach::{self, Case, Harness, Reveal, SIZES, Words};
use junie_tui::shell::{
    App, Chrome, Cx, Footer, FooterStatus, JobKey, KeyHost, KeyRouter, LayerTag, Menus, Nav,
    PageKey, Regions, Request, Router, Shell, ShellAction, ShellKeys, StatusLine, StopHandle,
    background, blur_fan_out, deliverable, handle, render,
};
use junie_tui::shell::{
    Page, PageCx, PageTypes, open_context, rebuild_on_tick, replay_key, row_anchor, row_choice,
};
use junie_tui::theme::{ColorLevel, HintAlign, StatusKind, Strings, Theme, Tone};
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::context::{ContextConfig, ContextKeys};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::Layer;
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::leave::QuitKeys;
use junie_tui::ui::leave::{LeaveGuard, Stakes};
use junie_tui::ui::popup::Dismiss;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::hintbar::HintBar;
use junie_tui::widgets::keyhint::Hint;
use junie_tui::widgets::menu::{MenuBarAction, MenuItem};
use junie_tui::widgets::too_small::TooSmall;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{ScenarioCase, ScenarioId, paint, text_of};

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::ShellStatusLine, run: shell_status_line },
    ScenarioCase { id: ScenarioId::ShellCxRequests, run: shell_cx_requests },
    ScenarioCase { id: ScenarioId::ShellRouterStack, run: shell_router_stack },
    ScenarioCase { id: ScenarioId::ShellChromeMenus, run: shell_chrome_menus },
    ScenarioCase { id: ScenarioId::ShellFooterHints, run: shell_footer_hints },
    ScenarioCase { id: ScenarioId::ShellKeyRouting, run: shell_key_routing },
    ScenarioCase { id: ScenarioId::ShellHostLoop, run: shell_host_loop },
    ScenarioCase { id: ScenarioId::ShellPageHelpers, run: shell_page_helpers },
    ScenarioCase { id: ScenarioId::ShellClipboardCopy, run: shell_clipboard_copy },
    ScenarioCase { id: ScenarioId::ShellMotionPolicy, run: shell_motion_policy },
    ScenarioCase { id: ScenarioId::RuntimeInputPump, run: runtime_input_pump },
    ScenarioCase { id: ScenarioId::RuntimeExternalChannel, run: runtime_external_channel },
];

fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

/// A status line through its whole life: posting, replacing, expiring on the
/// sim clock, the error and sticky messages that wait for input, a job's
/// progress, a payload, and a stop handle shared between clones.
fn shell_status_line() {
    assert_eq!(StatusLine::<()>::DEFAULT_TTL_MS, 5_000);
    let line = StatusLine::<()>::new();
    assert!(line.is_none() && !line.is_some());
    assert_eq!(line.revision(), 0);
    let mut line = line.ttl_ms(1_000);

    line.set_at("Saved", Tone::Secondary, Moment::from_millis(100));
    assert!(line.is_some() && !line.is_none());
    assert_eq!(line.get(), Some(("Saved", Tone::Secondary)));
    assert_eq!(line.text(), Some("Saved"));
    assert_eq!(line.entry(), Some(("Saved", Tone::Secondary, &())));
    assert_eq!(line.payload(), Some(&()));
    assert_eq!(line.revision(), 1);
    line.set("Saved", Tone::Secondary);
    assert_eq!(line.revision(), 1, "posting the same message is no change");
    line.set("Saved twice", Tone::Secondary);
    assert_eq!(line.revision(), 2);

    // The message was posted at 100 ms, so it outlives 999 ms and ends at 1000.
    line.set_at("Saved", Tone::Secondary, Moment::from_millis(100));
    assert!(!line.expire(Moment::from_millis(1_099)));
    assert!(line.expire(Moment::from_millis(1_100)));
    assert!(line.is_none());
    assert!(!line.clear(), "nothing is left to clear");

    // `observe` only moves forward: an older moment never rewinds the clock.
    line.observe(Moment::from_millis(2_000));
    line.observe(Moment::from_millis(500));
    line.set("Late", Tone::Warning);
    assert!(!line.expire(Moment::from_millis(2_999)));
    assert!(line.expire(Moment::from_millis(3_000)));

    // An error waits for input instead of expiring; a tick or resize is not input.
    line.error("Disk full");
    assert!(line.is_error());
    assert!(!line.expire(Moment::from_millis(60_000)));
    assert!(!line.on_input(&Input::Tick));
    assert!(!line.on_input(&Input::Resize(80, 24)));
    assert!(line.on_input(&Input::Key(plain(KeyCode::Char('x')))));
    assert!(line.is_none() && !line.is_error());

    // A sticky message does too, and a paste is input.
    line.sticky_at("Hold on", Moment::from_millis(4_000));
    assert!(line.is_sticky());
    assert!(!line.expire(Moment::from_millis(90_000)));
    assert!(line.on_input(&Input::Paste("x".into())));
    line.sticky("Again");
    assert!(line.is_sticky() && line.text() == Some("Again"));
    assert!(line.clear());
    assert!(!line.is_sticky());

    // A job's progress is replaced in place and ends only with its own key.
    const BUILD: JobKey = JobKey::new("build");
    assert_eq!(BUILD.name(), "build");
    assert_eq!(line.job(), None);
    line.progress_at(BUILD, "Building 1/3", Moment::from_millis(5_000));
    assert_eq!(line.job(), Some(BUILD));
    line.progress(BUILD, "Building 2/3");
    assert_eq!(line.text(), Some("Building 2/3"));
    assert!(!line.end_job(JobKey::new("other")));
    assert!(line.end_job(BUILD));
    assert!(line.is_none());

    // A payload travels with the message.
    let mut tagged = StatusLine::<u8>::new();
    tagged.set_with("Copied", Tone::Secondary, 7);
    assert_eq!(tagged.entry(), Some(("Copied", Tone::Secondary, &7)));
    tagged.set_with_at("Slow", Tone::Warning, 8, Moment::from_millis(50));
    assert_eq!(tagged.payload(), Some(&8));
    tagged.set_sticky_with("Pinned", 9);
    assert!(tagged.is_sticky());
    assert_eq!(tagged.payload(), Some(&9));

    let stop = StopHandle::default();
    let seen_by_clone = stop.clone();
    assert!(!seen_by_clone.is_stopped());
    stop.stop();
    assert!(seen_by_clone.is_stopped(), "a clone shares the stop flag");
}

/// What a page asks of the shell through its `Cx`: every request, in order,
/// and which of them a page that is not shown may still make.
fn shell_cx_requests() {
    const JOB: JobKey = JobKey::new("sync");
    let (a, b, c) = (
        WidgetId::of("scenario.cx.a"),
        WidgetId::of("scenario.cx.b"),
        WidgetId::of("scenario.cx.c"),
    );
    let mut ring = FocusRing::default();
    ring.register(a);
    ring.register(b);
    ring.register(c);
    let mut focus = Focus::default();
    focus.set(Some(a));

    let mut cx: Cx<'_, LayerResult, (), &'static str> =
        Cx::new(&mut focus, &ring).context(ContextConfig::DEFAULT.keys(ContextKeys::NONE));
    assert_eq!(cx.context.keys, ContextKeys::NONE);
    cx.focus_next();
    assert_eq!(cx.focus.current(), Some(b));
    cx.focus_prev();
    assert_eq!(cx.focus.current(), Some(a));
    cx.focus_prev();
    assert_eq!(cx.focus.current(), Some(c), "previous wraps to the last");
    cx.focus_next();
    assert_eq!(cx.focus.current(), Some(a), "next wraps to the first");

    cx.status("Saved");
    cx.status_in("Careful", Tone::Warning);
    cx.notice("Refresh failed", Tone::Error);
    cx.status_as(StatusKind::Ok, "Done");
    cx.sticky("Pinned");
    cx.refused(Reason::new("Read-only"));
    cx.progress(JOB, "Syncing");
    cx.error("Boom");
    cx.open(
        Dialog::confirm(WidgetId::of("scenario.cx.dialog"), "Close", "Sure?", "Yes"),
        LayerTag::new("confirm").key("a:b").n(2),
    );
    cx.close();
    cx.with_top::<Dialog>(|d| d.title = "Renamed".into());
    cx.go("settings");
    cx.back();
    cx.copy("abc");
    cx.copy_as("abc", "the id");
    cx.help();
    cx.refuse_outside_click(Dismiss::default());

    let requests = cx.take();
    assert!(cx.requests.is_empty(), "take drains the queue");
    assert_eq!(requests.len(), 17);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.allowed_in_background())
            .count(),
        8,
        "only the status requests may come from a page that is not shown"
    );

    let mut layer = Dialog::confirm(WidgetId::of("scenario.cx.top"), "Close", "Sure?", "Yes");
    let mut seen = Vec::new();
    for request in requests {
        match request {
            Request::Status(text, tone) => seen.push(format!("status {text} {tone:?}")),
            Request::StatusAs(text, kind) => seen.push(format!("status_as {text} {kind:?}")),
            Request::Notice(text, tone) => seen.push(format!("notice {text} {tone:?}")),
            Request::Sticky(text) => seen.push(format!("sticky {text}")),
            Request::Progress(text, job) => seen.push(format!("progress {text} {}", job.name())),
            Request::Open(layer, tag) => {
                let _: &dyn Layer<LayerResult, ()> = &*layer;
                seen.push(format!("open {} {} {}", tag.kind, tag.key, tag.n));
            }
            Request::Close => seen.push("close".into()),
            Request::Top(apply) => {
                apply(&mut layer);
                seen.push("top".into());
            }
            Request::Go(to) => seen.push(format!("go {to}")),
            Request::Back => seen.push("back".into()),
            Request::Copy(text, what) => seen.push(format!("copy {text} {what:?}")),
            Request::Help => seen.push("help".into()),
            Request::RefusedOutsideClick(dismiss) => {
                assert_eq!(dismiss, Dismiss::default());
                seen.push("refused".into());
            }
        }
    }
    assert_eq!(layer.title, "Renamed", "with_top reaches the layer on top");
    assert_eq!(
        seen,
        [
            "status Saved Secondary",
            "status Careful Warning",
            "notice Refresh failed Error",
            "status_as Done Ok",
            "sticky Pinned",
            "status Read-only Secondary",
            "progress Syncing sync",
            "status Boom Error",
            "open confirm a:b 2",
            "close",
            "top",
            "go settings",
            "back",
            "copy abc None",
            "copy abc Some(\"the id\")",
            "help",
            "refused",
        ]
    );
    shell_background_requests();
}

/// A screen that is not shown may still post status, but nothing that opens,
/// closes, navigates or copies.
fn shell_background_requests() {
    let ring = FocusRing::default();
    let mut focus = Focus::default();
    let mut cx: Cx<'_, LayerResult, (), &'static str> = Cx::new(&mut focus, &ring);
    cx.status("Synced");
    cx.go("elsewhere");
    cx.notice("Refresh failed", Tone::Warning);
    cx.copy("secret");
    cx.close();
    cx.sticky("Pinned");
    let kept: Vec<_> = background(cx.take()).collect();
    assert_eq!(kept.len(), 3);
    assert!(kept.iter().all(Request::allowed_in_background));
    assert!(matches!(kept[0], Request::Status(..)));
    assert!(matches!(kept[1], Request::Notice(..)));
    assert!(matches!(kept[2], Request::Sticky(..)));
}

/// Navigation through the router: a stack per tab, what each move drops, the
/// leave guard in front of a move that drops state, and the breadcrumb.
fn shell_router_stack() {
    let mut router = Router::with_tabs(vec!["Home", "Files"]);
    assert_eq!(
        (router.tab_count(), router.active(), router.depth()),
        (2, 0, 1)
    );
    assert_eq!(*router.current(), "Home");
    assert_eq!(router.key(), PageKey(1));
    assert_ne!(
        router.key(),
        PageKey::ROOT,
        "the root key belongs to no router page"
    );
    assert_eq!(PageKey::ROOT, PageKey(0));
    assert_eq!(router.top_of(1), Some(&"Files"));
    assert_eq!(router.top_of(2), None);

    router.push("Detail");
    router.push("Edit");
    assert_eq!(router.depth(), 3);
    assert_eq!(router.key(), PageKey(4), "every page gets its own key");
    assert_eq!(
        router.stack().collect::<Vec<_>>(),
        [&"Home", &"Detail", &"Edit"]
    );
    assert_eq!(router.stack().len(), 3);
    for page in router.stack_mut() {
        assert!(!page.is_empty());
    }
    *router.current_mut() = "Edit 2";
    assert_eq!(*router.current(), "Edit 2");
    *router.top_of_mut(1).expect("the second tab has a page") = "Files 2";
    assert!(router.top_of_mut(9).is_none());
    assert_eq!(router.tab(1).collect::<Vec<_>>(), [&"Files 2"]);
    assert_eq!(router.pages().count(), 4);
    for page in router.pages_mut() {
        assert!(!page.is_empty());
    }

    // What each move would drop, before it is made.
    assert!(!router.drops(&Nav::Push("Next")));
    assert!(!router.drops(&Nav::<&str>::Tab(1)));
    assert!(router.drops(&Nav::Replace("Other")));
    assert!(router.drops(&Nav::Reset("Root")));
    assert!(router.drops(&Nav::<&str>::Back));
    assert!(
        !Router::new("Solo").drops(&Nav::<&str>::Back),
        "the root has nothing to go back from"
    );
    let mut pair = Router::new("Solo");
    pair.push("Next");
    assert!(
        pair.drops(&Nav::<&str>::Back),
        "two pages are enough for a back to drop one"
    );
    assert!(router.drops(&Nav::<&str>::PopTo(0)));
    assert!(
        !router.drops(&Nav::<&str>::PopTo(2)),
        "popping to the top drops nothing"
    );
    assert_eq!(
        router
            .dropped_by(&Nav::<&str>::PopTo(0))
            .collect::<Vec<_>>(),
        [&"Edit 2", &"Detail"]
    );
    assert_eq!(
        router.dropped_by(&Nav::<&str>::Back).collect::<Vec<_>>(),
        [&"Edit 2"]
    );
    assert_eq!(router.dropped_by(&Nav::Replace("x")).count(), 1);
    assert_eq!(router.dropped_by(&Nav::Reset("x")).count(), 3);
    assert_eq!(router.dropped_by(&Nav::Push("x")).count(), 0);
    assert_eq!(router.dropped_by(&Nav::<&str>::Tab(1)).count(), 0);

    // A guard asks before a move that drops, and lets a harmless one through.
    let mut guard = LeaveGuard::new(WidgetId::of("scenario.router.guard"));
    let strings = Strings::default();
    let clean = router.request(
        &mut guard,
        Nav::Push("Next"),
        || Stakes::Unsaved { changes: 2 },
        &strings,
    );
    assert_eq!(clean.expect("a push drops nothing"), Nav::Push("Next"));
    let asked = router.request(
        &mut guard,
        Nav::Back,
        || Stakes::Unsaved { changes: 2 },
        &strings,
    );
    let dialog = asked.expect_err("going back with unsaved changes asks first");
    assert!(guard.is_dialog(dialog.id));
    let nothing_at_stake = router.request(&mut guard, Nav::Back, || Stakes::Clean, &strings);
    assert_eq!(nothing_at_stake.expect("clean state leaves"), Nav::Back);

    // Applying a move reports what it dropped.
    assert_eq!(router.apply(Nav::Push("Next"), same), Some(vec![]));
    assert_eq!(router.depth(), 4);
    assert_eq!(
        router.apply(Nav::PopTo(1), same),
        Some(vec!["Edit 2", "Next"])
    );
    assert_eq!(
        router.apply(Nav::PopTo(1), same),
        None,
        "nothing above to drop"
    );
    assert_eq!(router.apply(Nav::Back, same), Some(vec!["Detail"]));
    assert_eq!(router.apply(Nav::Back, same), None, "the root stays");
    assert_eq!(
        router.apply(Nav::Replace("Home 2"), same),
        Some(vec!["Home"])
    );
    assert_eq!(router.apply(Nav::Tab(1), same), Some(vec![]));
    assert_eq!(router.apply(Nav::Tab(1), same), None, "already there");
    assert_eq!(*router.current(), "Files 2");
    assert_eq!(
        router.apply(Nav::Reset("Files 3"), same),
        Some(vec!["Files 2"])
    );

    // Direct moves: back, pop_to, replace, reset, select, open_tab, close_tab.
    router.push("A");
    router.push("B");
    router.push("C");
    assert_eq!(router.back(), Some("C"));
    assert_eq!(router.pop_to(1), vec!["B"]);
    assert_eq!(router.pop_to(5), Vec::<&str>::new());
    assert_eq!(router.replace("A2"), "A");
    assert_eq!(router.reset("Files 4"), vec!["Files 3", "A2"]);
    assert_eq!(router.back(), None);
    assert!(router.select(0));
    assert!(
        !router.select(0),
        "selecting the active tab changes nothing"
    );
    assert!(!router.select(7));
    assert_eq!(router.open_tab("Third"), 2);
    assert_eq!(router.active(), 2);
    assert_eq!(router.close_tab(2), Some(vec!["Third"]));
    assert_eq!(
        router.active(),
        1,
        "closing the active tab selects the one before"
    );
    assert_eq!(router.close_tab(5), None);
    assert_eq!(
        Router::new("Solo").close_tab(0),
        None,
        "the last tab cannot close"
    );

    // The breadcrumb is the stack; a click on a crumb pops to it.
    let mut path = Router::new("Home");
    path.push("Detail");
    path.push("Edit");
    let crumb = path.breadcrumb(WidgetId::of("scenario.router.crumb"), |p| p.to_uppercase());
    assert_eq!(crumb.segments, ["HOME", "DETAIL", "EDIT"]);
    assert_eq!(
        path.crumb_click::<&str>(&crumb, 0, crumb.segment_id(0)),
        Some(Nav::PopTo(0))
    );
    assert_eq!(
        path.crumb_click::<&str>(&crumb, 0, crumb.segment_id(1)),
        Some(Nav::PopTo(1))
    );
    assert_eq!(
        path.crumb_click::<&str>(&crumb, 0, crumb.segment_id(2)),
        None,
        "the current page is not a link"
    );
}

/// The chrome's rows and the menu bar built from a menu table.
fn shell_chrome_menus() {
    let screen = Rect::new(0, 0, 40, 12);
    let chrome = Chrome::new();
    assert_eq!(chrome.rows(), (1, 1));
    assert_eq!(chrome, Chrome::default());
    let regions = chrome.regions(screen);
    assert_eq!(
        regions,
        Regions {
            header: Rect::new(0, 0, 40, 1),
            body: Rect::new(0, 2, 40, 8),
            footer: Rect::new(0, 11, 40, 1),
        }
    );
    let roomy = Chrome::new().header(2).gap(0).footer(3).inset(4);
    assert_eq!(roomy.rows(), (2, 3));
    assert_eq!(
        (roomy.header, roomy.gap, roomy.footer, roomy.inset),
        (2, 0, 3, 4)
    );
    let regions = roomy.regions(screen);
    assert_eq!(regions.header, Rect::new(0, 0, 40, 2));
    assert_eq!(regions.footer, Rect::new(0, 9, 40, 3));
    assert_eq!(
        regions.body,
        Rect::new(4, 2, 32, 7),
        "the inset narrows the body only"
    );
    let tiny = Chrome::new().regions(Rect::new(0, 0, 10, 2));
    assert_eq!(tiny.body.height, 0, "a screen with no room leaves no body");

    let mut dialog = Dialog::confirm(
        WidgetId::of("scenario.chrome.dialog"),
        "Close",
        "Sure?",
        "Yes",
    );
    let before = dialog.chrome;
    roomy.fit(&mut dialog);
    assert_eq!(dialog.chrome, (before.0.max(2), before.1.max(3)));

    let menus = Menus::new()
        .menu(
            "File",
            vec![
                (MenuItem::new("Open"), "open"),
                (MenuItem::new("Quit"), "quit"),
            ],
        )
        .menu("Help", vec![(MenuItem::new("About"), "about")]);
    assert_eq!(menus.titles().collect::<Vec<_>>(), ["File", "Help"]);
    assert_eq!(menus.items(0).len(), 2);
    assert!(menus.items(9).is_empty());
    assert_eq!(menus.get(1, 0), Some("about"));
    assert_eq!(menus.get(0, 5), None);
    assert_eq!(menus.command(MenuBarAction::Chosen(0, 1)), Some("quit"));
    assert_eq!(menus.command(MenuBarAction::Opened(0)), None);
    assert_eq!(menus.command(MenuBarAction::Closed), None);

    let mut bar = menus.bar(WidgetId::of("scenario.chrome.bar"));
    let buf = paint(Rect::new(0, 0, 40, 6), |area, buf, ctx| {
        let bg = ctx.theme.canvas;
        bar.render(Rect::new(area.x, area.y, area.width, 1), buf, ctx, bg);
    });
    let shown = text_of(&buf);
    assert!(shown.contains("File") && shown.contains("Help"), "{shown}");
}

fn same(page: &'static str) -> &'static str {
    page
}

fn hints(pairs: &[(&'static str, &'static str)]) -> Vec<Hint> {
    pairs.iter().map(|&(k, a)| Hint::new(k, a)).collect()
}

fn footer_buf(footer: Footer, shell: &mut Shell<()>, peek: Option<&str>) -> Buffer {
    paint(Rect::new(0, 0, 60, 1), |area, buf, ctx| {
        ctx.peek = peek.map(str::to_owned);
        footer.render(area, buf, ctx, shell);
    })
}

fn footer_row(footer: Footer, shell: &mut Shell<()>, peek: Option<&str>) -> String {
    text_of(&footer_buf(footer, shell, peek))
}

/// The footer: which hints it composes, what the status does to them, and
/// what it draws.
fn shell_footer_hints() {
    let open = hints(&[("Enter", "Open"), ("F2", "Rename")]);

    // Layer hints win over the fallback; an empty layer list falls back.
    let fallback = Footer::new().fallback(hints(&[("Esc", "Back")]));
    assert_eq!(fallback.compose(None), hints(&[("Esc", "Back")]));
    let layered = Footer::new()
        .fallback(hints(&[("Esc", "Back")]))
        .layer(Some(open.clone()));
    assert_eq!(layered.compose(None), open);
    assert_eq!(layered.hints.as_ref().map(Vec::len), Some(2));
    let emptied = Footer::new()
        .fallback(hints(&[("Esc", "Back")]))
        .layer(Some(vec![]));
    assert_eq!(emptied.compose(None), hints(&[("Esc", "Back")]));
    let first_wins = layered.clone().layer(Some(hints(&[("F5", "Reload")])));
    assert_eq!(
        first_wins.compose(None),
        open,
        "the first layer's hints stay"
    );

    // A revealed hint goes in after the first; Tab is added unless editing.
    let revealed = layered.compose(Some(("F1", "Help")));
    assert_eq!(
        revealed.iter().map(|h| &*h.key).collect::<Vec<_>>(),
        ["Enter", "F1", "F2"]
    );
    let tabbed = layered.clone().tab_next(true);
    assert!(tabbed.tab_next);
    assert!(
        tabbed
            .compose(None)
            .iter()
            .any(|h| h.key == "Tab" && h.action == "Next")
    );
    let editing = tabbed.clone().editing(true);
    assert!(editing.editing);
    assert!(!editing.compose(None).iter().any(|h| h.key == "Tab"));

    // A hint for a key the terminal cannot deliver is dropped.
    assert!(deliverable("Enter"));
    assert!(!deliverable("Shift+click"));
    let undeliverable =
        Footer::new().layer(Some(hints(&[("Enter", "Open"), ("Shift+click", "Add")])));
    assert_eq!(undeliverable.compose(None), hints(&[("Enter", "Open")]));

    // A dirty page pins its save hint; an error puts every hint back to normal.
    let dirty = Footer::new()
        .layer(Some(hints(&[("Ctrl+S", "Save"), ("Esc", "Back")])))
        .pin_dirty(Some("Ctrl+S"))
        .dirty(true);
    assert!(dirty.dirty);
    assert_eq!(dirty.pin_dirty, Some("Ctrl+S"));
    let composed = dirty.compose(None);
    assert_eq!(
        composed[0].priority,
        junie_tui::widgets::keyhint::HintPriority::Pinned
    );
    assert_ne!(
        composed[1].priority,
        junie_tui::widgets::keyhint::HintPriority::Pinned
    );
    let failing = dirty.clone().status(Some(("Save failed", Tone::Error)));
    assert!(
        failing
            .compose(None)
            .iter()
            .all(|h| h.priority == junie_tui::widgets::keyhint::HintPriority::Normal)
    );

    // The three ways to set a status, and which of them is an error.
    let toned = Footer::new().status(Some(("Saved", Tone::Secondary)));
    assert!(
        matches!(&toned.status, Some(FooterStatus::Toned { text, tone: Tone::Secondary, parts }) if text == "Saved" && parts.is_empty())
    );
    assert!(!toned.status.as_ref().is_some_and(FooterStatus::is_error));
    let parted = Footer::new().status_parts(
        Some(("Synced", Tone::Secondary)),
        vec![("3 rows".to_owned(), Tone::Warning)],
    );
    assert!(matches!(&parted.status, Some(FooterStatus::Toned { parts, .. }) if parts.len() == 1));
    let kind = Footer::new().status_kind(Some(("Failed", Some(StatusKind::Fail))));
    assert!(matches!(
        &kind.status,
        Some(FooterStatus::Kind {
            kind: Some(StatusKind::Fail),
            ..
        })
    ));
    assert!(kind.status.as_ref().is_some_and(FooterStatus::is_error));
    let ok = Footer::new().status_kind(Some(("Done", Some(StatusKind::Ok))));
    assert!(!ok.status.as_ref().is_some_and(FooterStatus::is_error));
    assert!(Footer::new().status(None).status.is_none());
    assert!(Footer::new().status_parts(None, vec![]).status.is_none());
    assert!(Footer::new().status_kind(None).status.is_none());
    assert!(
        Footer::new()
            .status_kind(Some(("Plain", None)))
            .status
            .is_some()
    );

    // Drawn: hints, the status, the EDIT badge, the peek text and alignment.
    let mut shell = Shell::<()>::new();
    let base = Footer::new().layer(Some(open.clone()));
    let at_rest = footer_row(base.clone(), &mut shell, None);
    assert!(
        at_rest.contains("Enter") && at_rest.contains("Open"),
        "{at_rest}"
    );
    assert!(at_rest.contains("Rename"), "{at_rest}");
    let with_status = footer_row(
        base.clone().status(Some(("Saved 3 rows", Tone::Secondary))),
        &mut shell,
        None,
    );
    assert!(with_status.contains("Saved 3 rows"), "{with_status}");
    // Parts recolour stretches of the status text from its start.
    let parted = base.clone().status_parts(
        Some(("Synced 3 rows", Tone::Secondary)),
        vec![
            ("Synced ".to_owned(), Tone::Secondary),
            ("3 rows".to_owned(), Tone::Warning),
        ],
    );
    let buf = footer_buf(parted, &mut shell, None);
    let with_parts = text_of(&buf);
    let at = with_parts
        .find("Synced 3 rows")
        .expect("the status is drawn") as u16;
    assert_ne!(
        buf[(at, 0)].fg,
        buf[(at + 7, 0)].fg,
        "the part after the first is drawn in its own tone"
    );
    let with_kind = footer_row(
        base.clone()
            .status_kind(Some(("Build failed", Some(StatusKind::Fail)))),
        &mut shell,
        None,
    );
    assert!(with_kind.contains("Build failed"), "{with_kind}");
    let badge = footer_row(base.clone().editing(true), &mut shell, None);
    assert!(badge.contains("EDIT"), "{badge}");
    assert!(!at_rest.contains("EDIT"));
    let peeked = footer_row(base.clone(), &mut shell, Some("Opens the selected row"));
    assert!(peeked.contains("Opens the selected row"), "{peeked}");
    let hidden = footer_row(
        base.clone().peek(false),
        &mut shell,
        Some("Opens the selected row"),
    );
    assert!(!hidden.contains("Opens the selected row"), "{hidden}");
    let shadowed = footer_row(
        base.clone().status(Some(("Saved", Tone::Secondary))),
        &mut shell,
        Some("Opens the selected row"),
    );
    assert!(
        !shadowed.contains("Opens the selected row"),
        "a status takes the peek's place"
    );
    let centred = footer_row(base.clone().align(HintAlign::Center), &mut shell, None);
    let lead = |row: &str| row.len() - row.trim_start().len();
    assert!(
        lead(&centred) > lead(&at_rest),
        "centred hints start further in"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cmd {
    Ping,
}

const FIRST: WidgetId = WidgetId::of("scenario.host.first");
const SECOND: WidgetId = WidgetId::of("scenario.host.second");

/// An application built only from the shell's parts: two buttons, a footer,
/// the key router and the pointer router, with every hook recorded.
struct Host {
    shell: Shell<()>,
    keys: ShellKeys<Cmd>,
    now: u64,
    page: &'static str,
    layer: bool,
    capture: bool,
    text: bool,
    dirty: bool,
    wide: bool,
    bare: bool,
    page_out: Outcome,
    esc_top_out: Outcome,
    interrupt_out: Outcome,
    blur_out: Outcome,
    post: Option<&'static str>,
    pages: Vec<KeyCode>,
    actions: Vec<ShellAction<Cmd>>,
    notes: Vec<String>,
    quits: u32,
    interrupts: u32,
    pastes: Vec<String>,
    blurs: Vec<FocusChange>,
    steps: Vec<u64>,
    after: Vec<Outcome>,
}

impl Host {
    fn new(keys: ShellKeys<Cmd>) -> Self {
        Self {
            shell: Shell::new(),
            keys,
            now: 0,
            page: "one",
            layer: false,
            capture: false,
            text: false,
            dirty: false,
            wide: false,
            bare: false,
            page_out: Outcome::Ignored,
            esc_top_out: Outcome::Ignored,
            interrupt_out: Outcome::Ignored,
            blur_out: Outcome::Ignored,
            post: None,
            pages: Vec::new(),
            actions: Vec::new(),
            notes: Vec::new(),
            quits: 0,
            interrupts: 0,
            pastes: Vec::new(),
            blurs: Vec::new(),
            steps: Vec::new(),
            after: Vec::new(),
        }
    }

    fn key(&mut self, code: KeyCode) -> Outcome {
        handle(self, Input::Key(plain(code)))
    }

    fn chord(&mut self, code: KeyCode, mods: KeyModifiers) -> Outcome {
        handle(self, Input::Key(Key { code, mods }))
    }

    fn mouse(&mut self, kind: MouseKind, x: u16, y: u16) -> Outcome {
        handle(self, Input::Mouse(Mouse::new(kind, Position::new(x, y))))
    }

    fn frame(&mut self, term: &mut Terminal<TestBackend>) -> Buffer {
        term.draw(|f| render(self, f))
            .expect("the test backend draws");
        term.backend().buffer().clone()
    }
}

impl App for Host {
    type Status = ();

    fn shell(&self) -> &Shell<()> {
        &self.shell
    }

    fn shell_mut(&mut self) -> &mut Shell<()> {
        &mut self.shell
    }

    fn theme(&self) -> Theme {
        Theme::for_level(ColorLevel::TrueColor)
    }

    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new("host").min(20, 4)
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn tick(&self) -> u64 {
        self.now / 100
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let bg = ctx.theme.canvas;
        Button::primary(FIRST, "Alpha").render(Rect::new(0, 0, 12, 1), buf, ctx, bg);
        Button::primary(SECOND, "Beta").render(Rect::new(0, 1, 12, 1), buf, ctx, bg);
        if self.wide && area.width >= 100 {
            buf.set_string(14, 0, "Gigantic", Style::default());
        }
        if self.bare {
            return;
        }
        let footer = Footer::new()
            .fallback(hints(&[("Enter", "Open"), ("F1", "Help")]))
            .status(self.shell.status.get());
        let row = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        footer.render(row, buf, ctx, &mut self.shell);
    }

    fn on_input(&mut self, input: Input) -> Outcome {
        if let Some(text) = self.post {
            self.shell.status.set(text, Tone::Secondary);
        }
        match input {
            Input::Mouse(m) => PointerRouter::route(self, m),
            other => KeyRouter::input(self, other),
        }
    }

    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        self.blurs.push(change);
        self.blur_out
    }

    fn after_input(&mut self, _input: &Input, out: Outcome) -> Outcome {
        self.after.push(out);
        out
    }

    fn layer_open(&self) -> bool {
        self.layer
    }

    fn page_key(&self) -> PageKey {
        PageKey::of(self.page)
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(FIRST)
    }

    fn advance_clock(&mut self, ms: u64) -> Option<Outcome> {
        self.steps.push(ms);
        Some(Outcome::Changed)
    }
}

impl PointerHost for Host {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        self.shell.pointer_parts()
    }

    fn layer_open(&self) -> bool {
        self.layer
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn deliver(&mut self, _ev: &junie_tui::core::pointer::PointerEvent) -> Response<()> {
        Response::default()
    }
}

impl KeyHost for Host {
    type Cmd = Cmd;

    fn shell_keys(&self) -> &ShellKeys<Cmd> {
        &self.keys
    }

    fn layer_key(&mut self, _key: &Key) -> Option<Outcome> {
        self.layer.then_some(Outcome::Ignored)
    }

    fn capturing(&self) -> bool {
        self.capture
    }

    fn text_focus(&self) -> bool {
        self.capture || self.text
    }

    fn interrupt(&mut self) -> Outcome {
        self.interrupts += 1;
        self.interrupt_out
    }

    fn dirty(&mut self) -> bool {
        self.dirty
    }

    fn shell_action(&mut self, action: ShellAction<Cmd>, _key: &Key) -> Outcome {
        self.actions.push(action);
        Outcome::Consumed
    }

    fn page_key(&mut self, key: &Key) -> Outcome {
        self.pages.push(key.code);
        self.page_out
    }

    fn esc_top(&mut self) -> Outcome {
        self.esc_top_out
    }

    fn layer_paste(&mut self, _text: &str) -> Option<Outcome> {
        self.layer.then_some(Outcome::Ignored)
    }

    fn page_paste(&mut self, text: &str) -> Outcome {
        self.pastes.push(text.to_owned());
        Outcome::Consumed
    }

    fn request_quit(&mut self) -> Outcome {
        self.quits += 1;
        Outcome::Changed
    }

    fn note(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }
}

fn ctrl(c: char) -> KeyModifiers {
    let _ = c;
    KeyModifiers::CONTROL
}

/// Routes one key through any `KeyHost`, as the shell's own handler does.
fn through_the_router<H: KeyHost>(host: &mut H, code: KeyCode) -> Outcome {
    KeyRouter::route(host, &plain(code))
}

/// The key router in front of a host: the order of layers, menus, priority
/// bindings, the page, the quit keys and the fallback bindings.
fn shell_key_routing() {
    let mut host = Host::new(ShellKeys::new());
    let mut term = Terminal::new(TestBackend::new(40, 8)).expect("test terminal");
    host.frame(&mut term);
    assert_eq!(host.shell.focus.current(), Some(FIRST));

    // Tab reaches the page first, then moves focus through the fallback map.
    assert_eq!(host.key(KeyCode::Tab), Outcome::Changed);
    assert_eq!(host.pages, [KeyCode::Tab]);
    assert_eq!(host.shell.focus.current(), Some(SECOND));
    assert_eq!(
        host.chord(KeyCode::BackTab, KeyModifiers::SHIFT),
        Outcome::Changed
    );
    assert_eq!(host.shell.focus.current(), Some(FIRST));

    // F1 is a shell binding: the page never sees it.
    host.pages.clear();
    assert_eq!(host.key(KeyCode::F(1)), Outcome::Consumed);
    assert_eq!(host.actions, [ShellAction::Help]);
    assert!(host.pages.is_empty());

    // The other shell actions come from their default keys.
    host.actions.clear();
    host.key(KeyCode::F(10));
    host.chord(KeyCode::PageDown, ctrl('d'));
    host.chord(KeyCode::PageUp, ctrl('u'));
    assert_eq!(
        host.actions,
        [
            ShellAction::MenuBar,
            ShellAction::NextPage,
            ShellAction::PrevPage
        ]
    );
    host.actions.clear();
    host.pages.clear();

    // Esc never quits by default; the note says so, and a page may take it first.
    assert_eq!(host.key(KeyCode::Esc), Outcome::Changed);
    assert_eq!(host.notes, ["Esc never quits · Ctrl+Q quits"]);
    host.esc_top_out = Outcome::Consumed;
    assert_eq!(host.key(KeyCode::Esc), Outcome::Consumed);
    assert_eq!(host.notes.len(), 1, "a page that takes Esc posts no note");
    host.esc_top_out = Outcome::Ignored;

    // The quit keys: Ctrl+Q, the letter, and the dirty letter that only warns.
    assert_eq!(host.chord(KeyCode::Char('q'), ctrl('q')), Outcome::Changed);
    assert_eq!(host.quits, 1);
    assert_eq!(host.key(KeyCode::Char('q')), Outcome::Changed);
    assert_eq!(host.quits, 2);
    host.dirty = true;
    host.notes.clear();
    assert_eq!(host.key(KeyCode::Char('q')), Outcome::Changed);
    assert_eq!(host.quits, 2, "a dirty page does not quit on the letter");
    assert_eq!(
        host.notes,
        ["Unsaved changes · Ctrl+Q asks before quitting"]
    );
    host.dirty = false;

    // Ctrl+C is an interrupt first; a page that takes it stops the quit.
    host.interrupt_out = Outcome::Consumed;
    assert_eq!(host.chord(KeyCode::Char('c'), ctrl('c')), Outcome::Consumed);
    assert_eq!((host.interrupts, host.quits), (1, 2));
    host.interrupt_out = Outcome::Ignored;
    assert_eq!(host.chord(KeyCode::Char('c'), ctrl('c')), Outcome::Changed);
    assert_eq!((host.interrupts, host.quits), (2, 3));

    // An unbound key reaches only the page. While the pointer hovers a control
    // the first key also changes the interaction (hover is suppressed), and a
    // replay does not.
    let mut plain_host = Host::new(ShellKeys::new());
    plain_host.frame(&mut term);
    plain_host.mouse(MouseKind::Move, 1, 0);
    plain_host.frame(&mut term);
    assert!(
        plain_host.shell.pointer.hover.is_some(),
        "the pointer hovers a button"
    );
    let z = plain(KeyCode::Char('z'));
    assert_eq!(KeyRouter::route(&mut plain_host, &z), Outcome::Changed);
    assert_eq!(KeyRouter::route(&mut plain_host, &z), Outcome::Ignored);
    assert_eq!(KeyRouter::replay(&mut plain_host, &z), Outcome::Ignored);
    assert_eq!(plain_host.pages.len(), 3);
    assert_eq!(
        KeyRouter::input(&mut plain_host, Input::Tick),
        Outcome::Ignored
    );
    assert_eq!(plain_host.pages.len(), 3, "a tick is no key");
    assert_eq!(
        KeyRouter::input(&mut plain_host, Input::Key(z)),
        Outcome::Ignored
    );
    assert_eq!(plain_host.pages.len(), 4);

    // Bindings: a printable key is quiet while text is being typed, a priority
    // chord is not, and `idle_only` quiets a chord too.
    let ping = ShellAction::App(Cmd::Ping);
    let mut bound = Host::new(ShellKeys::new().bind('p', ping));
    bound.frame(&mut term);
    assert_eq!(bound.key(KeyCode::Char('p')), Outcome::Consumed);
    assert_eq!(bound.actions, [ping]);
    bound.text = true;
    bound.key(KeyCode::Char('p'));
    assert_eq!(bound.actions.len(), 1, "typing 'p' is not the shortcut");
    assert_eq!(bound.pages, [KeyCode::Char('p')]);

    let k = KeyBinding::ctrl('k');
    let mut priority = Host::new(ShellKeys::new().bind_priority(k, ping));
    priority.capture = true;
    priority.frame(&mut term);
    assert_eq!(
        priority.chord(KeyCode::Char('k'), ctrl('k')),
        Outcome::Consumed
    );
    assert_eq!(
        priority.actions,
        [ping],
        "a priority chord beats a capturing page"
    );
    assert!(priority.pages.is_empty());
    let mut idle = Host::new(ShellKeys::new().bind_priority(k, ping).idle_only(ping));
    idle.capture = true;
    idle.frame(&mut term);
    idle.chord(KeyCode::Char('k'), ctrl('k'));
    assert!(
        idle.actions.is_empty(),
        "idle-only actions wait for an idle page"
    );
    assert_eq!(idle.pages, [KeyCode::Char('k')]);
    idle.capture = false;
    idle.chord(KeyCode::Char('k'), ctrl('k'));
    assert_eq!(idle.actions, [ping]);

    // A key bound to the Quit action asks the host to quit.
    let mut quitting = Host::new(ShellKeys::new().bind('x', ShellAction::Quit));
    quitting.frame(&mut term);
    assert_eq!(quitting.key(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(quitting.quits, 1);

    let mut late = Host::new(ShellKeys::new().bind_fallback(KeyCode::F(7), ShellAction::Help));
    late.frame(&mut term);
    late.key(KeyCode::F(7));
    assert_eq!(
        late.pages,
        [KeyCode::F(7)],
        "the page sees a fallback key first"
    );
    assert_eq!(late.actions, [ShellAction::Help]);

    let mut unbound = Host::new(ShellKeys::new().unbind(KeyBinding::key(KeyCode::F(1))));
    unbound.frame(&mut term);
    assert_eq!(
        unbound.key(KeyCode::F(1)),
        Outcome::Ignored,
        "no binding, no page taker"
    );
    assert!(unbound.actions.is_empty());
    assert_eq!(unbound.pages, [KeyCode::F(1)]);

    // Quit keys and notes are settings.
    let mut letterless = Host::new(ShellKeys::new().quit(QuitKeys::DEFAULT.letter(None)));
    letterless.frame(&mut term);
    letterless.key(KeyCode::Char('q'));
    assert_eq!(letterless.quits, 0);
    let mut esc_quits = Host::new(ShellKeys::new().quit(QuitKeys::DEFAULT.esc_quits(true)));
    esc_quits.frame(&mut term);
    esc_quits.key(KeyCode::Esc);
    assert_eq!(esc_quits.quits, 1);
    let mut silent = Host::new(ShellKeys::new().esc_note(None).dirty_letter_note(None));
    silent.frame(&mut term);
    assert_eq!(silent.key(KeyCode::Esc), Outcome::Consumed);
    silent.dirty = true;
    silent.key(KeyCode::Char('q'));
    assert!(silent.notes.is_empty());
    assert_eq!(silent.quits, 0);

    // A layer on top swallows keys, but Ctrl+Q still quits.
    let mut layered = Host::new(ShellKeys::new());
    layered.frame(&mut term);
    layered.layer = true;
    assert_eq!(layered.key(KeyCode::Char('x')), Outcome::Consumed);
    assert!(layered.pages.is_empty());
    layered.chord(KeyCode::Char('q'), ctrl('q'));
    assert_eq!(layered.quits, 1);

    // Too small to draw: everything is swallowed, quit keys excepted.
    let mut small = Host::new(ShellKeys::new());
    small.frame(&mut term);
    small.shell.too_small = true;
    assert_eq!(small.key(KeyCode::Char('x')), Outcome::Consumed);
    assert_eq!(small.key(KeyCode::Char('q')), Outcome::Changed);
    assert_eq!(small.chord(KeyCode::Char('q'), ctrl('q')), Outcome::Changed);
    assert_eq!(small.quits, 2);
    assert!(small.pages.is_empty());
    assert_eq!(KeyRouter::paste(&mut small, "text"), Outcome::Consumed);
    assert!(small.pastes.is_empty());

    // Paste goes to the page, or to a layer that is open.
    assert_eq!(KeyRouter::paste(&mut plain_host, "abc"), Outcome::Consumed);
    assert_eq!(
        KeyRouter::input(&mut plain_host, Input::Paste("def".into())),
        Outcome::Consumed
    );
    assert_eq!(plain_host.pastes, ["abc", "def"]);
    assert_eq!(KeyRouter::paste(&mut layered, "abc"), Outcome::Consumed);
    assert!(layered.pastes.is_empty(), "a layer takes the paste");

    // Any `KeyHost` is routed alike.
    let mut generic = Host::new(ShellKeys::new());
    generic.frame(&mut term);
    assert_eq!(
        through_the_router(&mut generic, KeyCode::F(1)),
        Outcome::Consumed
    );
    assert_eq!(generic.actions, [ShellAction::Help]);

    // The help section lists what the shell binds, and the app's own command.
    let section = ShellKeys::new()
        .bind('p', ping)
        .help_section("Shell", |c| match c {
            Cmd::Ping => Some("Ping the app"),
        });
    assert_eq!(section.title, "Shell");
    let actions: Vec<&str> = section.rows.iter().map(|(_, a)| a.as_str()).collect();
    for wanted in ["This reference", "Ping the app", "Back", "Quit"] {
        assert!(actions.contains(&wanted), "{wanted} in {actions:?}");
    }
    let hidden = ShellKeys::new()
        .bind('p', ping)
        .help_section("Shell", |_| None);
    assert!(!hidden.rows.iter().any(|(_, a)| a == "Ping the app"));
    for action in [
        ShellAction::<Cmd>::Quit,
        ShellAction::MenuBar,
        ShellAction::NextPage,
        ShellAction::PrevPage,
        ShellAction::FocusNext,
        ShellAction::FocusPrev,
        ShellAction::Back,
    ] {
        let keys = ShellKeys::<Cmd>::new().bind(KeyCode::F(9), action);
        let row = keys.help_section("Shell", |_| None);
        assert!(row.rows.len() >= hidden.rows.len(), "{action:?} is listed");
    }
}

/// The shell around a host: the frame it draws, the focus it keeps, the
/// requests a test can send, the clipboard and the way it stops.
fn shell_host_loop() {
    let mut host = Host::new(ShellKeys::new());
    let mut term = Terminal::new(TestBackend::new(40, 8)).expect("test terminal");
    let first = host.frame(&mut term);
    let shown = text_of(&first);
    assert!(shown.contains("Alpha") && shown.contains("Beta"), "{shown}");
    assert!(
        shown.contains("Enter") && shown.contains("Help"),
        "the footer hints are drawn"
    );
    assert_eq!(host.shell.size, (40, 8));
    assert!(!host.shell.too_small);
    assert_eq!(
        host.shell.focus.current(),
        Some(FIRST),
        "the page's primary control has focus"
    );
    assert!(host.shell.ring.contains(SECOND));
    assert!(
        !Application::take_render_invalidated(&mut host),
        "a settled frame is not pending"
    );

    // The footer's hints are clickable through the registry the frame left.
    let hint_hit = host
        .shell
        .hits
        .regions()
        .iter()
        .find(|r| r.owner == HintBar::ID)
        .copied()
        .expect("the footer registered its hints");
    let at = Position::new(hint_hit.area.x, hint_hit.area.y);
    let click = junie_tui::core::pointer::PointerEvent::new(
        &host.shell.hits,
        junie_tui::core::intent::Phase::Click,
        hint_hit.id,
        at,
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(
        host.shell.footer_keys.clicked(&click).map(|k| k.code),
        Some(KeyCode::Enter)
    );

    // A footer drawn by hand: the hints and spots the shell was told about are
    // the ones clicked, and the next frame forgets them again.
    host.bare = true;
    host.shell
        .footer_drawn(hints(&[("F9", "Direct")]), vec![(0, Rect::new(2, 7, 6, 1))]);
    host.frame(&mut term);
    let direct = host
        .shell
        .hits
        .regions()
        .iter()
        .find(|r| r.owner == HintBar::ID)
        .copied()
        .expect("the told footer registered its hint");
    let click = junie_tui::core::pointer::PointerEvent::new(
        &host.shell.hits,
        junie_tui::core::intent::Phase::Click,
        direct.id,
        Position::new(direct.area.x, direct.area.y),
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(
        host.shell.footer_keys.clicked(&click).map(|k| k.code),
        Some(KeyCode::F(9))
    );
    host.bare = false;
    host.frame(&mut term);
    assert_eq!(
        host.shell.footer_keys.clicked(&click).map(|k| k.code),
        Some(KeyCode::Enter),
        "a later frame replaces the told footer with the drawn one"
    );

    // The pointer parts are the shell's own registry, ring, focus and router.
    let parts = host.shell.pointer_parts();
    assert!(parts.ring.contains(SECOND) && parts.focus.current().is_some());
    assert!(!parts.hits.regions().is_empty());
    assert!(parts.router.hover.is_none(), "no pointer has moved yet");

    // Pointer: a click moves focus through the shell's pointer parts.
    let beta = host
        .shell
        .hits
        .regions()
        .iter()
        .find(|r| r.owner == SECOND)
        .map(|r| (r.area.x + 1, r.area.y))
        .expect("the second button is on screen");
    host.mouse(MouseKind::Move, beta.0, beta.1);
    host.mouse(MouseKind::Down, beta.0, beta.1);
    host.mouse(MouseKind::Up, beta.0, beta.1);
    assert_eq!(host.shell.focus.current(), Some(SECOND));
    host.frame(&mut term);

    // Focus moves by key and by the shell's own calls, and a move tells the page.
    host.shell.focus_prev();
    assert_eq!(host.shell.focus.current(), Some(FIRST));
    host.shell.focus_next();
    assert_eq!(host.shell.focus.current(), Some(SECOND));
    let moved = host.blurs.len();
    assert_eq!(
        blur_fan_out(&mut host),
        Outcome::Ignored,
        "the page declined the blur"
    );
    assert_eq!(
        host.blurs.len(),
        moved + 1,
        "the moves since the last frame are reported"
    );
    host.blur_out = Outcome::Changed;
    assert_eq!(
        blur_fan_out(&mut host),
        Outcome::Ignored,
        "no change, no blur"
    );
    host.shell.focus.set(Some(FIRST));
    assert_eq!(blur_fan_out(&mut host), Outcome::Changed);
    assert_eq!(
        host.blurs.last().map(|c| (c.from, c.to)),
        Some((SECOND, Some(FIRST)))
    );
    host.blur_out = Outcome::Ignored;

    // Upkeep: a new page with no valid focus takes the page's primary control.
    host.shell.focus.set(None);
    host.shell.upkeep(false, PageKey::of("two"), Some(SECOND));
    assert_eq!(host.shell.focus.current(), Some(SECOND));
    host.shell.focus.set(None);
    host.shell.upkeep(true, PageKey::of("three"), Some(FIRST));
    assert!(
        host.shell
            .focus
            .current()
            .is_some_and(|f| host.shell.ring.contains(f))
    );

    // The interaction the frame is drawn with: hover time stops when motion pauses.
    let live = host.shell.interaction(500, 500, 5);
    assert_eq!((live.tick, live.focus), (5, host.shell.focus.current()));
    assert!(live.hover_ms.is_some());
    host.shell.motion = Motion::Paused;
    assert_eq!(host.shell.interaction(500, 500, 5).hover_ms, None);
    host.shell.motion = Motion::Full;

    // A status posted by the page is a change and is drawn in the footer.
    host.post = Some("Saved 3 rows");
    assert_eq!(host.key(KeyCode::Char('z')), Outcome::Changed);
    assert!(text_of(&host.frame(&mut term)).contains("Saved 3 rows"));
    assert_eq!(
        host.after.last(),
        Some(&Outcome::Changed),
        "after_input sees the outcome"
    );
    host.post = None;

    // Test requests advance the stepped clock and fence the output.
    host.shell.test_requests = true;
    host.steps.clear();
    assert_eq!(
        handle(&mut host, Input::Paste(TestRequest::advance(250))),
        Outcome::Changed
    );
    assert_eq!(host.steps, [250]);
    assert_eq!(
        handle(&mut host, Input::Paste(TestRequest::FENCE.to_owned())),
        Outcome::Ignored
    );
    assert!(
        junie_tui::shell::clipboard::take_output().contains(&TestRequest::FENCE_ACK.to_owned()),
        "a fence is acknowledged"
    );
    host.shell.test_requests = false;
    host.shell.fence_requests = true;
    host.steps.clear();
    handle(&mut host, Input::Paste(TestRequest::advance(250)));
    assert!(
        host.steps.is_empty(),
        "the fence alone does not step the clock"
    );
    host.shell.fence_requests = false;

    // A requests batch from a page: shown pages keep everything, others only status.
    let (out, all) = host
        .shell
        .deliver::<junie_tui::ui::layers::LayerResult, (), ()>(true, |cx| {
            cx.status("Saved");
            cx.help();
            Outcome::Changed
        });
    assert_eq!((out, all.len()), (Outcome::Changed, 2));
    let (_, hidden) = host
        .shell
        .deliver::<junie_tui::ui::layers::LayerResult, (), ()>(false, |cx| {
            cx.status("Saved");
            cx.help();
            Outcome::Changed
        });
    assert_eq!(hidden.len(), 1);
    host.shell.context = junie_tui::ui::context::ContextConfig::DEFAULT
        .keys(junie_tui::ui::context::ContextKeys::NONE);
    let cx = host
        .shell
        .cx::<junie_tui::ui::layers::LayerResult, (), ()>();
    assert_eq!(cx.context.keys, junie_tui::ui::context::ContextKeys::NONE);

    // The runtime's view of the same app.
    assert!(!Application::should_quit(&host));
    assert_eq!(
        Application::tick_interval(&host),
        Duration::from_millis(100)
    );
    assert_eq!(
        Application::coalesce(&host),
        junie_tui::runtime::Coalesce::default()
    );
    assert_eq!(
        Application::input_config(&host),
        junie_tui::core::event::InputConfig::default()
    );
    assert_eq!(
        Application::raw_input_policy(&host),
        junie_tui::runtime::RawInputPolicy::default()
    );
    assert!(Application::external(&host).is_none());
    let _ = Application::shapes(&host);
    let _ = Application::keyboard_protocol(&host);
    assert_eq!(
        Application::handle(&mut host, Input::Tick),
        Outcome::Ignored
    );

    // It stops by a quit request, by the stop handle, and through the leave guard.
    let stop = host.shell.stop_handle();
    assert!(!Application::should_quit(&host));
    stop.stop();
    assert!(
        Application::should_quit(&host),
        "the stop handle ends the run"
    );

    let mut clean = Host::new(ShellKeys::new());
    let mut guard =
        junie_tui::ui::leave::LeaveGuard::<()>::new(WidgetId::of("scenario.host.leave"));
    let strings = Strings::default();
    let asked = clean
        .shell
        .quit_through(&mut guard, &Stakes::Unsaved { changes: 1 }, &strings)
        .expect_err("unsaved changes ask first");
    assert!(guard.is_dialog(asked.id));
    assert!(!Application::should_quit(&clean));
    clean
        .shell
        .quit_through(&mut guard, &Stakes::Clean, &strings)
        .expect("nothing at stake quits");
    assert!(clean.shell.quit);
    assert!(Application::should_quit(&clean));

    // Too small: the host's too-small screen replaces the page.
    let mut tight = Terminal::new(TestBackend::new(10, 3)).expect("test terminal");
    let mut cramped = Host::new(ShellKeys::new());
    let drawn = text_of(&cramped.frame(&mut tight));
    assert!(cramped.shell.too_small);
    assert!(!drawn.contains("Alpha"), "{drawn}");
}

struct Pages;

impl PageTypes for Pages {
    type State = u32;
    type Msg = ();
    type Result = LayerResult;
    type LayerState = ();
    type Go = &'static str;
}

struct Counter;

impl Page<Pages> for Counter {
    fn on_key(&mut self, key: &Key, s: &mut u32, cx: &mut PageCx<Pages>) -> Outcome {
        if key.is_char('+') {
            *s += 1;
            cx.status(format!("count {s}"));
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _ctx: &mut RenderCtx, s: &u32) {
        buf.set_string(area.x, area.y, format!("count {s}"), Style::default());
    }

    fn hints(&self, _focus: Option<WidgetId>, _s: &u32) -> Vec<Hint> {
        hints(&[("+", "Add")])
    }

    fn crumb(&self, s: &u32) -> String {
        format!("Counter {s}")
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(FIRST)
    }
}

/// What a page author writes: a page type, the helpers that turn a row into a
/// context menu anchor and a chosen menu item into a replayed key, and the
/// tick helper that reports a change only when the view changed.
fn shell_page_helpers() {
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: PageCx<Pages> = Cx::new(&mut focus, &ring);
    let mut state: <Pages as PageTypes>::State = 0;
    let mut page = Counter;
    assert_eq!(
        page.on_key(&plain(KeyCode::Char('x')), &mut state, &mut cx),
        Outcome::Ignored
    );
    assert_eq!(
        page.on_key(&plain(KeyCode::Char('+')), &mut state, &mut cx),
        Outcome::Changed
    );
    assert_eq!(state, 1);
    assert_eq!(page.crumb(&state), "Counter 1");
    assert_eq!(page.primary_focus(), Some(FIRST));
    assert_eq!(page.hints(None, &state), hints(&[("+", "Add")]));
    let buf = paint(Rect::new(0, 0, 20, 2), |area, buf, ctx| {
        page.render(area, buf, ctx, &state)
    });
    assert!(text_of(&buf).contains("count 1"));
    // The defaults: nothing handled, not editing, and Esc goes back.
    assert_eq!(page.on_esc_top(&mut state, &mut cx), Outcome::Changed);
    assert_eq!(page.on_interrupt(&mut state, &mut cx), Outcome::Ignored);
    assert_eq!(page.on_tick(&mut state, &mut cx), Outcome::Ignored);
    assert!(!page.is_editing() && !page.animating(&state));
    assert_eq!(page.stakes(&state), Stakes::Clean);
    let requests = cx.take();
    assert!(matches!(requests[0], Request::Status(ref text, _) if text == "count 1"));
    assert!(matches!(requests[1], Request::Back));

    // replay_key: the keys a menu item replays.
    assert_eq!(replay_key('\n').code, KeyCode::Enter);
    assert_eq!(replay_key('←').code, KeyCode::Left);
    assert_eq!(replay_key('→').code, KeyCode::Right);
    assert_eq!(replay_key('\u{1b}').code, KeyCode::Esc);
    assert_eq!(replay_key('x'), plain(KeyCode::Char('x')));

    // row_choice: "<row>:<keys>" in the layer tag, the chosen index picks the key.
    let tag = LayerTag::new("row").key("3:xy");
    assert_eq!(
        row_choice(&tag, "row", Some(1)),
        Some((3, Some(plain(KeyCode::Char('y'))), 1))
    );
    assert_eq!(
        row_choice(&tag, "row", Some(5)),
        Some((3, None, 5)),
        "no key for that index"
    );
    assert_eq!(row_choice(&tag, "other", Some(1)), None);
    assert_eq!(row_choice(&tag, "row", None), None);
    assert_eq!(
        row_choice(&LayerTag::new("row").key("3"), "row", Some(0)),
        None
    );
    assert_eq!(
        row_choice(&LayerTag::new("row").key("x:ab"), "row", Some(0)),
        None
    );

    // row_anchor: the anchor follows the scrolled row and stays inside the list.
    let list = WidgetId::of("scenario.page.list");
    let area = Rect::new(2, 5, 30, 4);
    let ContextAnchor::Control { id, fallback } = row_anchor(list, 3, area, 1) else {
        panic!("a row anchors to its control");
    };
    assert_eq!(id, list.child(3));
    assert_eq!(fallback, Rect::new(2, 7, 30, 1));
    let ContextAnchor::Control { fallback, .. } = row_anchor(list, 40, area, 0) else {
        panic!("a row anchors to its control");
    };
    assert_eq!(
        fallback.y, 8,
        "a row below the list anchors to its last line"
    );
    let ContextAnchor::Control { fallback, .. } = row_anchor(list, 0, Rect::new(0, 0, 0, 0), 4)
    else {
        panic!("a row anchors to its control");
    };
    assert_eq!(fallback, Rect::new(0, 0, 1, 1));

    // open_context: a menu opens as a layer anchored to the row, and draws.
    let mut focus = Focus::default();
    let mut cx: Cx<'_, LayerResult, (), ()> = Cx::new(&mut focus, &ring);
    let out = open_context(
        &mut cx,
        WidgetId::of("scenario.page.menu"),
        row_anchor(list, 0, area, 0),
        "Row actions",
        vec![MenuItem::new("Rename"), MenuItem::new("Delete")],
        LayerTag::new("row").key("0:rd"),
    );
    assert_eq!(out, Outcome::Changed);
    let Some(Request::Open(mut menu, tag)) = cx.take().pop() else {
        panic!("open_context opens a layer");
    };
    assert_eq!((tag.kind, tag.key.as_str()), ("row", "0:rd"));
    let buf = paint(Rect::new(0, 0, 60, 14), |area, buf, ctx| {
        menu.render(area, buf, ctx, &());
    });
    let shown = text_of(&buf);
    assert!(
        shown.contains("Rename") && shown.contains("Delete"),
        "{shown}"
    );

    // rebuild_on_tick: a change is reported only when the view differs.
    let mut rows = vec![1, 2, 3];
    assert_eq!(
        rebuild_on_tick(&mut rows, Clone::clone, |r| r.reverse()),
        Outcome::Changed
    );
    assert_eq!(rows, [3, 2, 1]);
    assert_eq!(
        rebuild_on_tick(&mut rows, Clone::clone, |r| r.sort_by(|a, b| b.cmp(a))),
        Outcome::Ignored
    );
}

#[derive(Debug)]
struct Sink {
    written: Mutex<Vec<String>>,
    accept: bool,
}

impl NativeClipboard for Sink {
    fn write(&self, text: &str) -> bool {
        self.written
            .lock()
            .expect("sink lock")
            .push(text.to_owned());
        self.accept
    }
}

/// Copying: the OSC 52 escape a terminal receives, the limit, the switches,
/// a native clipboard in front of it, the hook widgets copy through and the
/// message the status line shows.
fn shell_clipboard_copy() {
    let _ = clipboard::take_output();
    let plain_copy = Clipboard::new();
    assert!(plain_copy.osc52 && plain_copy.write && plain_copy.native.is_none());
    assert_eq!(plain_copy.limit, Clipboard::DEFAULT_LIMIT);
    assert_eq!(Clipboard::default().limit, Clipboard::DEFAULT_LIMIT);

    let copied = plain_copy.copy("hello");
    assert_eq!(copied, Copied::Copied { chars: 5 });
    assert!(copied.ok());
    assert_eq!(clipboard::osc52("hello"), "\x1b]52;c;aGVsbG8=\x07");
    assert_eq!(clipboard::osc52("hi"), "\x1b]52;c;aGk=\x07");
    assert_eq!(clipboard::take_output(), [clipboard::osc52("hello")]);
    assert!(
        clipboard::take_output().is_empty(),
        "taking output empties the queue"
    );
    clipboard::queue_output("\x1b[2J".to_owned());
    assert_eq!(clipboard::take_output(), ["\x1b[2J"]);
    assert_eq!(plain_copy.copy("héllo wörld"), Copied::Copied { chars: 11 });
    let _ = clipboard::take_output();

    // The switches: no OSC 52, no write, and a byte limit.
    let off = Clipboard::new().osc52(false);
    assert_eq!(off.copy("hello"), Copied::Unsupported);
    assert!(!Copied::Unsupported.ok());
    assert!(clipboard::take_output().is_empty());
    let silent = Clipboard::new().write(false);
    assert_eq!(silent.copy("hello"), Copied::Copied { chars: 5 });
    assert!(
        clipboard::take_output().is_empty(),
        "write(false) sends nothing"
    );
    let small = Clipboard::new().limit(4);
    assert_eq!(small.copy("hello"), Copied::TooLarge { bytes: 5, limit: 4 });
    assert!(!Copied::TooLarge { bytes: 5, limit: 4 }.ok());
    assert_eq!(small.copy("abcd"), Copied::Copied { chars: 4 });
    let _ = clipboard::take_output();

    // A native clipboard answers first; when it declines the terminal is asked.
    let sink: Arc<dyn NativeClipboard> = Arc::new(Sink {
        written: Mutex::new(vec![]),
        accept: true,
    });
    assert!(
        sink.write("probe"),
        "a native clipboard answers whether it took the text"
    );
    let accepting = Arc::new(Sink {
        written: Mutex::new(vec![]),
        accept: true,
    });
    let native = Clipboard::new().native(accepting.clone());
    assert_eq!(native.copy("native"), Copied::Copied { chars: 6 });
    assert_eq!(*accepting.written.lock().expect("sink lock"), ["native"]);
    assert!(
        clipboard::take_output().is_empty(),
        "the native clipboard took it"
    );
    let declining = Arc::new(Sink {
        written: Mutex::new(vec![]),
        accept: false,
    });
    let fallback = Clipboard::new().native(declining.clone());
    assert_eq!(fallback.copy("native"), Copied::Requested { chars: 6 });
    assert!(Copied::Requested { chars: 6 }.ok());
    assert_eq!(clipboard::take_output(), [clipboard::osc52("native")]);
    let nothing_left = Clipboard::new().osc52(false).native(declining);
    assert_eq!(nothing_left.copy("native"), Copied::Unsupported);

    // The hook: a widget that copies through the core clipboard reaches the
    // shell's clipboard until the guard drops.
    let hook: clipboard::ClipboardHook = plain_copy.install();
    junie_tui::core::edit::clipboard::copy("from a widget");
    assert_eq!(
        clipboard::take_output(),
        [clipboard::osc52("from a widget")]
    );
    drop(hook);
    junie_tui::core::edit::clipboard::copy("after the guard");
    assert!(
        clipboard::take_output().is_empty(),
        "a dropped hook delivers nothing"
    );

    // What the status line says about each result.
    let (text, tone) = Clipboard::report(&Copied::Copied { chars: 5 }, "hello", None);
    assert_eq!(
        (text.as_str(), tone),
        ("Copied 5 characters · hello", Tone::Secondary)
    );
    let (text, _) = Clipboard::report(&Copied::Copied { chars: 1 }, "a\nb", None);
    assert!(text.starts_with("Copied 1 character"), "{text}");
    let (text, tone) = Clipboard::report(&Copied::Copied { chars: 5 }, "a\nb", Some("the id"));
    assert_eq!((text.as_str(), tone), ("Copied the id", Tone::Secondary));
    let (text, _) = Clipboard::report(&Copied::Copied { chars: 3 }, "a\nb", None);
    assert!(
        text.contains("a · b"),
        "a newline is shown as a separator: {text}"
    );
    let (text, tone) = Clipboard::report(&Copied::Requested { chars: 5 }, "hello", None);
    assert_eq!(tone, Tone::Warning);
    assert!(text.contains("OSC 52"), "{text}");
    let (text, tone) = Clipboard::report(&Copied::Unsupported, "hello", None);
    assert_eq!(tone, Tone::Error);
    assert!(text.contains("does not accept OSC 52"), "{text}");
    let (text, tone) = Clipboard::report(&Copied::TooLarge { bytes: 9, limit: 4 }, "hello", None);
    assert_eq!(tone, Tone::Error);
    assert!(
        text.contains("9 bytes") && text.contains("4 byte"),
        "{text}"
    );

    // The shell owns one: copying through it posts the result.
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().limit(8));
    assert_eq!(shell.clipboard().limit, 8);
    assert_eq!(
        shell.copy_at("hello", None, Moment::from_millis(10)),
        Copied::Copied { chars: 5 }
    );
    assert_eq!(shell.status.text(), Some("Copied 5 characters · hello"));
    assert_eq!(
        shell.copy_at("far too long", None, Moment::from_millis(20)),
        Copied::TooLarge {
            bytes: 12,
            limit: 8
        }
    );
    assert!(shell.status.is_error());
    let _ = clipboard::take_output();
}

/// The motion setting on a page that animates: the showcase Activity page
/// opens with its feed running, so a tick moves it under `Full`, moves the
/// state but not the decoration under `Reduced`, and moves nothing under
/// `Paused`.
fn shell_motion_policy() {
    use crate::app::{App as Showcase, PageId};
    let theme = || Theme::for_level(ColorLevel::TrueColor);
    let mut frames = Vec::new();
    // (setting, whether ticks advance state, whether decoration runs, paused)
    for (motion, ticks, animates, paused) in [
        (Motion::Full, true, true, false),
        (Motion::Reduced, true, false, false),
        (Motion::Paused, false, false, true),
    ] {
        assert_eq!(
            (motion.ticks(), motion.animates(), motion.is_paused()),
            (ticks, animates, paused),
            "{motion:?}"
        );
        let mut app = Showcase::with_motion(theme(), motion, 0);
        app.open_page(PageId::Activity);
        let mut term = Terminal::new(TestBackend::new(140, 44)).expect("test terminal");
        let mut draw = |app: &mut Showcase| {
            term.draw(|f| render(app, f))
                .expect("the test backend draws");
            text_of(term.backend().buffer())
        };
        let before = draw(&mut app);
        assert_eq!(app.shell.motion, motion);
        assert_eq!(
            app.animating(),
            ticks,
            "{motion:?}: a running page animates unless motion is paused"
        );
        let decor = app.tick;
        let mut changed = false;
        for _ in 0..12 {
            changed |= handle(&mut app, Input::Tick) == Outcome::Changed;
        }
        let after = draw(&mut app);
        assert_eq!(changed, ticks, "{motion:?}: ticks change the view");
        assert_eq!(
            app.tick > decor,
            animates,
            "{motion:?}: only full motion runs the decorative clock"
        );
        assert_eq!(
            before != after,
            ticks,
            "{motion:?}: the drawn page moves exactly when ticks advance"
        );
        frames.push(after);
    }
    assert_ne!(
        frames[0], frames[2],
        "the same page, ticked the same, differs between full and paused motion"
    );
}

/// Draining raw input: what folds into one frame, what waits for the next,
/// the budget for input that changes nothing, the size watch and the guard
/// that swallows the click that focused the window.
fn runtime_input_pump() {
    struct Pumped {
        handled: Vec<Input>,
        out: Outcome,
    }
    impl Application for Pumped {
        fn handle(&mut self, input: Input) -> Outcome {
            self.handled.push(input);
            self.out
        }
        fn render(&mut self, _frame: &mut ratatui::Frame) {}
        fn should_quit(&self) -> bool {
            false
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(50)
        }
    }
    let wheel = || Input::Mouse(Mouse::new(MouseKind::WheelDown, Position::new(1, 1)));
    let mov = |x| Input::Mouse(Mouse::new(MouseKind::Move, Position::new(x, 1)));
    let key = |c| Input::Key(plain(KeyCode::Char(c)));
    let patient = Coalesce {
        max_latency: Duration::from_secs(60),
        ..Coalesce::default()
    };
    let feed = |inputs: Vec<Input>| {
        let mut queue: VecDeque<Input> = inputs.into();
        move || Ok(queue.pop_front())
    };

    // What folds: same-modifier wheel, moves, repeated navigation keys, resizes.
    let (a, b) = (wheel(), wheel());
    assert!(patient.folds(&a, &b));
    let shifted = Input::Mouse(Mouse {
        mods: KeyModifiers::SHIFT,
        ..Mouse::new(MouseKind::WheelDown, Position::new(1, 1))
    });
    assert!(
        !patient.folds(&a, &shifted),
        "a different modifier is a different gesture"
    );
    assert!(patient.folds(&mov(1), &mov(2)));
    assert!(!patient.folds(&mov(1), &a));
    let down = Input::Key(plain(KeyCode::Down));
    assert!(patient.folds(&down, &down));
    assert!(
        !patient.folds(&key('a'), &key('a')),
        "typing is never folded"
    );
    assert!(patient.folds(&Input::Resize(1, 1), &Input::Resize(2, 2)));
    assert!(!patient.folds(&Input::Tick, &Input::Tick));
    assert!(!Coalesce::OFF.folds(&a, &b) && !Coalesce::OFF.folds(&down, &down));
    assert_eq!(
        (Coalesce::OFF.max_batch, Coalesce::OFF.max_latency),
        (1, Duration::ZERO)
    );

    // A burst of wheel turns is handled in one drain; a batch limit cuts it.
    let mut app = Pumped {
        handled: vec![],
        out: Outcome::Changed,
    };
    let mut pump = InputPump::new(patient);
    assert!(
        pump.drain(&mut app, feed(vec![wheel(); 5]))
            .expect("drains")
    );
    assert_eq!(app.handled.len(), 5);
    app.handled.clear();
    pump.set_policy(Coalesce {
        max_batch: 3,
        ..patient
    });
    let mut rest = feed(vec![wheel(); 5]);
    assert!(pump.drain(&mut app, &mut rest).expect("drains"));
    assert_eq!(app.handled.len(), 3, "the batch limit ends the burst");
    app.handled.clear();
    let mut off = InputPump::new(Coalesce::OFF);
    assert!(off.drain(&mut app, feed(vec![wheel(); 5])).expect("drains"));
    assert_eq!(
        app.handled.len(),
        1,
        "with coalescing off each input is its own frame"
    );

    // An input that does not fold is held for the next drain, not lost.
    app.handled.clear();
    let mut pump = InputPump::new(patient);
    let mut queue = feed(vec![wheel(), wheel(), key('a')]);
    assert!(pump.drain(&mut app, &mut queue).expect("drains"));
    assert_eq!(app.handled.len(), 2);
    assert!(pump.has_pending());
    assert!(pump.drain(&mut app, &mut queue).expect("drains"));
    assert_eq!(app.handled.len(), 3);
    assert!(matches!(&app.handled[2], Input::Key(k) if k.is_char('a')));
    assert!(!pump.has_pending());

    // A folded run of pointer moves is settled once on the last position.
    app.handled.clear();
    assert!(!pump.settle(&mut app), "nothing to settle yet");
    assert!(
        pump.drain(&mut app, feed(vec![mov(1), mov(2), mov(3)]))
            .expect("drains")
    );
    assert_eq!(app.handled.len(), 3);
    assert!(pump.settle(&mut app));
    assert!(matches!(app.handled.last(), Some(Input::Mouse(m)) if m.pos.x == 3));
    assert!(!pump.settle(&mut app), "a move settles once");

    // Input that changes nothing is bounded.
    let mut idle = Pumped {
        handled: vec![],
        out: Outcome::Ignored,
    };
    let keys: Vec<Input> = (0..DRAIN_BUDGET + 40).map(|_| key('a')).collect();
    assert!(!drain_ready_inputs(&mut idle, feed(keys)).expect("drains"));
    assert_eq!(idle.handled.len(), DRAIN_BUDGET);
    let mut busy = Pumped {
        handled: vec![],
        out: Outcome::Changed,
    };
    assert!(drain_ready_inputs(&mut busy, feed(vec![key('a'), key('b')])).expect("drains"));
    assert_eq!(busy.handled.len(), 1, "the ready drain does not coalesce");

    // The size watch reports a resize once.
    let mut sizes = SizeWatch::new(Some((80, 24)));
    assert_eq!(sizes.check((80, 24)), None);
    assert_eq!(sizes.check((100, 30)), Some(Input::Resize(100, 30)));
    assert_eq!(sizes.check((100, 30)), None);
    sizes.observe(&Input::Resize(120, 40));
    assert_eq!(
        sizes.check((120, 40)),
        None,
        "an observed resize is already known"
    );
    sizes.observe(&Input::Tick);
    assert_eq!(sizes.check((120, 40)), None);
    assert_eq!(
        SizeWatch::new(None).check((80, 24)),
        Some(Input::Resize(80, 24))
    );

    // The guard drops focus events and the click that focused the window.
    assert_eq!(FocusClickGuard::DEFAULT_WINDOW, Duration::from_millis(300));
    assert_eq!(
        FocusClickGuard::default().window,
        FocusClickGuard::DEFAULT_WINDOW
    );
    let mouse = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        })
    };
    let t0 = Instant::now();
    let mut guard = FocusClickGuard::new(Duration::from_millis(300));
    assert!(guard.drops(&Event::FocusGained, t0));
    assert!(guard.drops(
        &mouse(MouseEventKind::Down(MouseButton::Left)),
        t0 + Duration::from_millis(100)
    ));
    assert!(guard.drops(
        &mouse(MouseEventKind::Drag(MouseButton::Left)),
        t0 + Duration::from_millis(120)
    ));
    assert!(guard.drops(
        &mouse(MouseEventKind::Up(MouseButton::Left)),
        t0 + Duration::from_millis(140)
    ));
    assert!(
        !guard.drops(
            &mouse(MouseEventKind::Down(MouseButton::Left)),
            t0 + Duration::from_millis(200)
        ),
        "only the first click"
    );
    assert!(!guard.drops(
        &Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        t0
    ));
    assert!(guard.drops(&Event::FocusGained, t0));
    assert!(
        !guard.drops(
            &mouse(MouseEventKind::Down(MouseButton::Left)),
            t0 + Duration::from_millis(301)
        ),
        "a click after the window is a click"
    );
    assert!(guard.drops(&Event::FocusGained, t0));
    assert!(guard.drops(&Event::FocusLost, t0));
    assert!(
        !guard.drops(
            &mouse(MouseEventKind::Down(MouseButton::Left)),
            t0 + Duration::from_millis(10)
        ),
        "losing focus cancels the guard"
    );
}

/// The channel a background thread uses to reach the application.
fn runtime_external_channel() {
    assert_eq!(ExternalReceiver::DEFAULT_LATENCY, Duration::from_millis(10));
    let (tx, rx): (ExternalSender, ExternalReceiver) = external_channel();
    let rx = rx.latency(Duration::from_millis(3));
    let producer = tx.clone();
    let worker = std::thread::spawn(move || producer.send(7_u32));
    assert!(worker.join().expect("the producer thread ends"));
    assert!(tx.send("two"));
    let first = rx.try_recv().expect("a queued message");
    assert_eq!(first.downcast_ref::<u32>(), Some(&7));
    assert!(tx.send(3_u8));
    #[allow(deprecated)]
    let rest = rx.drain();
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0].downcast_ref::<&str>(), Some(&"two"));
    assert_eq!(rest[1].downcast_ref::<u8>(), Some(&3));
    assert!(rx.try_recv().is_none(), "the queue is empty");
    drop(rx);
    assert!(!tx.send(1_u8), "a send to a dropped receiver is refused");
}

/// A host in a terminal of its own, so a reach case can build, resize and
/// redraw it.
struct Rig {
    host: Host,
    term: Terminal<TestBackend>,
}

impl Harness for Rig {
    type App = Host;

    fn fresh() -> Self {
        Self {
            host: Host::new(ShellKeys::new()),
            term: Terminal::new(TestBackend::new(80, 24)).expect("test terminal"),
        }
    }

    fn into_parts(self) -> (Host, Terminal<TestBackend>) {
        (self.host, self.term)
    }

    fn from_parts(host: Host, term: Terminal<TestBackend>) -> Self {
        Self { host, term }
    }
}

fn leave_alone(_rig: &mut Rig) {}

fn show_the_wide_word(rig: &mut Rig) {
    rig.host.wide = true;
}

fn open_a_layer(rig: &mut Rig) {
    rig.host.layer = true;
}

fn let_time_pass(rig: &mut Rig) {
    rig.host.now += 1;
}

const LATER: &[Reveal<Rig>] = &[Reveal {
    step: let_time_pass,
    times: 2,
}];

fn reach_cases() -> [Case<Rig>; 2] {
    [
        Case::page("calm", leave_alone),
        Case::layer("layered", leave_alone, open_a_layer).reveals(LATER),
    ]
}

/// The macro apps use to generate one reach test per case and size, invoked
/// for real on the host above with an allow file that lists nothing.
mod reach_macro {
    use super::*;

    junie_tui::reach_tests!("scenario", Rig, reach_cases();
        calm = "calm",
        layered = "layered",
    );
}

/// The reachability probe, as `reach_tests!` expands it: it reads what a page
/// shows at a huge reference size, then looks for each of those words at every
/// smaller size, through `reach::check` against an allow file, plus the
/// `reach::dump` hook and the size list the macro pins.
#[test]
fn catalog_shell_reach_probe() {
    // The `reach_tests!` expansion in `reach_macro` above runs `reach::check` per
    // size for its cases; this test drives the same calls directly and adds the
    // hidden-word case the macro's empty allow file could not hold.
    let macro_name = stringify!(reach_tests);
    let _fresh: fn() -> Rig = <Rig as Harness>::fresh;
    let calm = Case::page("calm", leave_alone);
    let wide = Case::page("wide", show_the_wide_word);
    let layered = Case::layer("layered", leave_alone, open_a_layer).reveals(LATER);
    let cases = [calm, wide, layered];
    type Step = reach::Setup<Rig>;
    let _calm_step: Step = leave_alone;
    assert_eq!(SIZES.len(), 8);
    assert_eq!(SIZES[0], (40, 12));
    assert!(SIZES.contains(&(80, 24)));
    let [calm, wide, layered] = &cases;

    for size in SIZES {
        let reach: reach::Reach = calm.run(size);
        assert_eq!(reach.size, size);
        assert!(reach.missing.is_empty(), "{size:?}: {:?}", reach.missing);
    }
    assert!(
        calm.run((10, 3)).too_small,
        "{macro_name}: a screen below the minimum is reported as too small"
    );
    assert!(calm.run((10, 3)).missing.is_empty());
    assert!(layered.run((100, 25)).missing.is_empty());
    assert_eq!(layered.reveals.len(), 1);
    let later: Reveal<Rig> = LATER[0];
    assert_eq!((layered.reveals[0].times, later.times), (2, 2));
    assert!(calm.overlay.is_none() && layered.overlay.is_some());

    // A word that only the huge screen shows is reported missing below it.
    let hidden = wide.run((80, 24));
    assert!(hidden.missing.contains("Gigantic"), "{:?}", hidden.missing);
    assert!(
        wide.run((160, 50)).missing.is_empty() || wide.run((160, 50)).missing.contains("Gigantic")
    );
    let allow = "scenario wide 80x24 Gigantic # R-scenario hidden on purpose";
    reach::check::<Rig>("scenario", &cases, "wide", (80, 24), allow);
    reach::check::<Rig>("scenario", &cases, "calm", (80, 24), allow);
    let refused = |allow: &str, name: &str| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reach::check::<Rig>("scenario", &cases, name, (80, 24), allow);
        }))
        .is_err()
    };
    assert!(
        refused("", "wide"),
        "a hidden word with no allow line fails the gate"
    );
    assert!(
        refused("scenario calm 80x24 Gigantic # R-scenario stale", "calm"),
        "an allow line for a word that is reachable fails the gate"
    );
    assert!(refused("", "no such case"));
    reach::dump::<Rig>(&cases);

    // Words: what a buffer shows whole and cut, and what is missing or cut.
    let mut buf = Buffer::empty(Rect::new(0, 0, 24, 3));
    buf.set_string(0, 0, "Alpha Beta Gamma-ray", Style::default());
    buf.set_string(0, 1, "Truncat\u{2026}", Style::default());
    buf.set_string(0, 2, "last row", Style::default());
    let mut words = Words::default();
    words.add_area(&buf, Rect::new(0, 0, 24, 2));
    for whole in ["Alpha", "Beta", "Gamma", "ray"] {
        assert!(words.whole.contains(whole), "{whole}");
    }
    assert!(words.cut.contains("Truncat"));
    assert!(!words.whole.contains("last"), "the third row was not read");
    let mut more = Words::default();
    more.add_area(&buf, Rect::new(0, 2, 24, 1));
    more.add_area(&buf, Rect::new(30, 30, 5, 5));
    words.extend(more);
    assert!(words.whole.contains("last") && words.whole.contains("row"));

    let want = Words {
        whole: ["Alpha", "Gamma", "Truncated", "Absent"]
            .map(String::from)
            .into(),
        cut: Default::default(),
    };
    let (missing, cut) = want.against(&words);
    assert_eq!(
        missing.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Absent"]
    );
    assert_eq!(
        cut.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Truncated"]
    );

    // Text a widget could not fit counts as reached once it is revealable.
    let _ = paint(Rect::new(0, 0, 20, 2), |_, _, ctx| {
        junie_tui::ui::reveal::truncated(
            ctx,
            None,
            WidgetId::of("scenario.reach.cell"),
            Rect::new(0, 0, 8, 1),
            "Supercalifragilistic hidden",
        );
    });
    let mut overflow = Words::default();
    overflow.add_overflowed();
    assert!(
        overflow.cut.contains("Supercalifragilistic"),
        "{:?}",
        overflow.cut
    );
}

#[test]
fn catalog_shell_clock_moments_and_wall_clock() {
    use junie_tui::clock::{Clock, WallClock, step_wall};
    assert_eq!(Moment::ZERO.as_millis(), 0);
    let t = Moment::from_millis(1_500);
    assert_eq!(t.as_millis(), 1_500);
    assert_eq!(t.since(Moment::from_millis(500)), 1_000);
    assert_eq!(Moment::from_millis(500).since(t), 0, "since saturates");
    assert_eq!(t.after(500), Moment::from_millis(2_000));
    assert_eq!(
        t.after(u64::MAX),
        Moment::from_millis(u64::MAX),
        "after saturates"
    );
    assert!(Moment::ZERO < t);

    let origin = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    let wall = WallClock::with_origin(origin);
    assert_eq!(wall.origin(), origin);
    assert!(wall.elapsed() >= Duration::from_millis(1));
    assert!(wall.now_ms() >= wall.elapsed().as_millis() as u64 - 1);
    assert!(
        Clock::now(&wall).as_millis() <= wall.now_ms(),
        "the clock only moves forward"
    );
    let mut unstepped = WallClock::new();
    let before = unstepped.now_ms();
    unstepped.advance(60_000);
    assert!(
        unstepped.now_ms() < before + 60_000,
        "a wall clock that is not stepped ignores advance"
    );
    assert!(WallClock::default().now_ms() < 60_000);

    // A run clock is stepped only when the test clock variable asked for it.
    let mut run = WallClock::for_run();
    run.advance(250);
    if TestRequest::opted_in() {
        assert_eq!(Clock::now(&run), Moment::from_millis(250));
    } else {
        assert!(Clock::now(&run).as_millis() < 60_000);
    }

    // `step_wall` ticks once per interval, with a shorter last tick.
    struct Wall {
        clock: WallClock,
        ticks: u32,
    }
    let mut app = Wall {
        clock: WallClock::new(),
        ticks: 0,
    };
    let out = step_wall(
        &mut app,
        |a| &mut a.clock,
        250,
        |_| 100,
        |a| {
            a.ticks += 1;
            Outcome::Changed
        },
    );
    assert_eq!((out, app.ticks), (Outcome::Changed, 3));
    let out = step_wall(&mut app, |a| &mut a.clock, 0, |_| 100, |_| Outcome::Changed);
    assert_eq!(out, Outcome::Ignored, "no time, no tick");
    let before = app.ticks;
    step_wall(
        &mut app,
        |a| &mut a.clock,
        5,
        |_| 0,
        |a| {
            a.ticks += 1;
            Outcome::Ignored
        },
    );
    assert_eq!(
        app.ticks,
        before + 5,
        "a zero interval still advances by one"
    );
}

#[test]
fn catalog_shell_sim_clock_and_stepping() {
    use junie_tui::clock::{Clock, SimClock, run_step};
    // 2023-11-14 22:13:20 UTC, a Tuesday.
    let epoch = 1_700_000_000;
    let mut sim = SimClock::new(epoch, 0);
    assert_eq!(
        (sim.now_ms, sim.running, sim.epoch_secs, sim.utc_offset_secs),
        (0, true, epoch, 0)
    );
    assert_eq!(Clock::now(&sim), Moment::ZERO);
    assert_eq!(sim.now_secs(), epoch);
    sim.advance(90_000);
    assert_eq!(sim.now_ms, 90_000);
    assert_eq!(sim.now_secs(), epoch + 90);
    assert_eq!(Clock::now(&sim), Moment::from_millis(90_000));
    sim.running = false;
    sim.advance(10_000);
    assert_eq!(sim.now_ms, 90_000, "a stopped clock does not advance");
    sim.running = true;

    assert_eq!(sim.hhmm(epoch), "22:13");
    assert_eq!(sim.stamp(epoch), "2023-11-14 22:13");
    assert_eq!(sim.weekday(epoch), "Tue");
    let east = SimClock::new(epoch, 3_600);
    assert_eq!(east.hhmm(epoch), "23:13");
    assert_eq!(east.stamp(epoch + 3_000), "2023-11-15 00:03");
    assert_eq!(east.weekday(epoch + 3_000), "Wed");
    assert_eq!(sim.hhmm(0), "00:00");
    assert_eq!(sim.stamp(0), "1970-01-01 00:00");
    assert_eq!(sim.weekday(0), "Thu");

    let now = sim.now_secs();
    assert_eq!(sim.ago(now), "0 s ago");
    assert_eq!(sim.ago(now + 50), "0 s ago", "the future reads as now");
    assert_eq!(sim.ago(now - 59), "59 s ago");
    assert_eq!(sim.ago(epoch), "1 min ago");
    assert_eq!(sim.ago(now - 5 * 3_600), "5 h ago");
    assert_eq!(sim.ago(now - 90_000), "yesterday");
    assert_eq!(sim.ago(now - 3 * 86_400), "3 d ago");

    assert_eq!(sim.reset_label(now), "resets now");
    assert_eq!(sim.reset_label(now - 5), "resets now");
    assert!(sim.reset_label(now + 7_200).starts_with("resets in "));
    assert_eq!(sim.reset_label(epoch + 2 * 86_400), "resets Thu 22:13");

    // `run_step` ticks until the clock reaches the step, then restores `running`.
    struct World {
        clock: SimClock,
        ticks: u32,
    }
    let mut world = World {
        clock: SimClock::new(epoch, 0),
        ticks: 0,
    };
    world.clock.running = false;
    let out = run_step(
        &mut world,
        |w| &mut w.clock,
        250,
        |w| {
            w.ticks += 1;
            w.clock.now_ms += 100;
            Outcome::Changed
        },
    );
    assert_eq!(
        (out, world.ticks, world.clock.now_ms),
        (Outcome::Changed, 3, 300)
    );
    assert!(
        !world.clock.running,
        "a paused clock is paused again afterwards"
    );
    let out = run_step(
        &mut world,
        |w| &mut w.clock,
        1_000,
        |w| {
            w.ticks += 1;
            Outcome::Ignored
        },
    );
    assert_eq!(
        (out, world.ticks),
        (Outcome::Ignored, 4),
        "a tick that stalls the clock ends the step"
    );
}

#[test]
fn catalog_shell_test_requests() {
    assert_eq!(TestRequest::ADVANCE_PREFIX, "junie-tui:advance-clock:");
    assert_eq!(TestRequest::FENCE, "junie-tui:fence");
    assert_eq!(TestRequest::FENCE_ACK, "\x07");
    assert_eq!(TestRequest::ENV, "JUNIE_TUI_TEST_CLOCK");
    assert_eq!(TestRequest::advance(250), "junie-tui:advance-clock:250");
    assert_eq!(
        TestRequest::parse(&TestRequest::advance(250)),
        Some(TestRequest::Advance(250))
    );
    assert_eq!(
        TestRequest::parse(TestRequest::FENCE),
        Some(TestRequest::Fence)
    );
    for refused in [
        "junie-tui:advance-clock:",
        "junie-tui:advance-clock:0",
        "junie-tui:advance-clock:-5",
        "junie-tui:advance-clock:5x",
        "junie-tui:advance-clock:99999999999999999999999",
        "hello",
        "",
    ] {
        assert_eq!(TestRequest::parse(refused), None, "{refused:?}");
    }
    assert_eq!(
        TestRequest::opted_in(),
        std::env::var_os(TestRequest::ENV).is_some_and(|v| v == "1")
    );
}

#[test]
fn catalog_shell_motion_resolution() {
    // An explicit choice wins over the environment.
    for chosen in [Motion::Full, Motion::Reduced, Motion::Paused] {
        assert_eq!(Motion::resolve(Some(chosen), true), chosen);
        assert_eq!(Motion::resolve(Some(chosen), false), chosen);
    }
    assert_eq!(Motion::resolve(None, true), Motion::Reduced);
    assert_eq!(Motion::resolve(None, false), Motion::Full);
    assert_eq!(Motion::default(), Motion::Full);

    // PATH is set and non-empty in any test run; the other name is never set.
    let unset = "JUNIE_TUI_CATALOG_SHELL_NEVER_SET";
    assert!(Motion::env_requests_reduced("PATH"));
    assert!(!Motion::env_requests_reduced(unset));
    assert_eq!(Motion::from_cli_or_env(None, "PATH"), Motion::Reduced);
    assert_eq!(Motion::from_cli_or_env(None, unset), Motion::Full);
    assert_eq!(
        Motion::from_cli_or_env(Some(Motion::Paused), "PATH"),
        Motion::Paused
    );

    assert!(Motion::Full.ticks() && Motion::Reduced.ticks() && !Motion::Paused.ticks());
    assert!(Motion::Full.animates() && !Motion::Reduced.animates() && !Motion::Paused.animates());
    assert!(
        Motion::Paused.is_paused() && !Motion::Full.is_paused() && !Motion::Reduced.is_paused()
    );
}

#[test]
fn catalog_shell_cli_arguments() {
    use clap::{Parser, ValueEnum};
    use junie_tui::cli::{ColorArg, CommonArgs, GlyphsArg, motion_help};
    use junie_tui::theme::{Charset, ColorLevel};

    // Each value and its aliases, and what it turns into.
    for (name, arg, level) in [
        ("truecolor", ColorArg::TrueColor, ColorLevel::TrueColor),
        ("24bit", ColorArg::TrueColor, ColorLevel::TrueColor),
        ("256", ColorArg::Ansi256, ColorLevel::Ansi256),
        ("16", ColorArg::Ansi16, ColorLevel::Ansi16),
        ("none", ColorArg::Mono, ColorLevel::Mono),
        ("mono", ColorArg::Mono, ColorLevel::Mono),
    ] {
        assert_eq!(ColorArg::from_str(name, true), Ok(arg), "{name}");
        assert_eq!(ColorLevel::from(arg), level, "{name}");
    }
    assert!(ColorArg::from_str("neon", true).is_err());
    for (name, arg, set) in [
        ("unicode", GlyphsArg::Unicode, Charset::Unicode),
        ("utf8", GlyphsArg::Unicode, Charset::Unicode),
        ("ascii", GlyphsArg::Ascii, Charset::Ascii),
    ] {
        assert_eq!(GlyphsArg::from_str(name, true), Ok(arg), "{name}");
        assert_eq!(Charset::from(arg), set, "{name}");
    }

    // The common options, parsed the way an application's own parser would.
    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        common: CommonArgs,
    }
    let cli = Cli::try_parse_from([
        "app", "--color", "16", "--glyphs", "ascii", "--motion", "paused", "--frame", "7",
    ])
    .expect("the common options parse");
    assert_eq!(cli.common.frame, 7);
    assert_eq!(cli.common.level(), ColorLevel::Ansi16);
    assert_eq!(cli.common.charset(), Charset::Ascii);
    assert_eq!(
        cli.common.motion("JUNIE_TUI_CATALOG_SHELL_NEVER_SET"),
        Motion::Paused
    );
    assert_eq!(
        cli.common.theme(),
        Charset::Ascii.apply(Theme::for_level(ColorLevel::Ansi16)),
        "the theme is the colour level with the glyph set applied"
    );
    let bare = Cli::try_parse_from(["app"]).expect("no options parse");
    assert_eq!(bare.common, CommonArgs::default());
    assert_eq!(
        bare.common.motion("JUNIE_TUI_CATALOG_SHELL_NEVER_SET"),
        Motion::Full
    );
    assert_eq!(
        bare.common.motion("PATH"),
        Motion::Reduced,
        "the app's variable asks for reduced motion"
    );
    assert!(Cli::try_parse_from(["app", "--color", "neon"]).is_err());

    let help = motion_help("SHOWCASE_NO_MOTION");
    assert!(
        help.starts_with("Motion:") && help.contains("SHOWCASE_NO_MOTION=1"),
        "{help}"
    );

    // `cli::parse` reads the process arguments, which here are the test
    // runner's: a command that ignores unknown arguments takes the defaults.
    #[derive(Parser)]
    #[command(ignore_errors = true)]
    struct Lenient {
        #[command(flatten)]
        common: CommonArgs,
    }
    let parsed: Lenient = junie_tui::cli::parse("After help text".to_owned());
    assert_eq!(parsed.common, CommonArgs::default());
}

#[test]
fn catalog_shell_docs_pages() {
    use junie_tui::core::list::NavProfile;
    use junie_tui::docs::{
        Documented, Knob, KnobLayer, MouseRow, StateHints, WidgetDoc, documented, files, index,
        page, widget,
    };

    let all: Vec<Documented> = documented();
    let expected = [
        widget::LIST_BOX,
        widget::TREE_VIEW,
        widget::SELECT,
        widget::TABS,
        widget::DATA_TABLE,
        widget::DATA_GRID,
    ];
    assert_eq!(all.len(), expected.len());
    for (documented, wanted) in all.iter().zip(&expected) {
        assert_eq!(documented.doc.name, wanted.name);
        assert_eq!(documented.doc.file, wanted.file);
        assert!(!documented.doc.purpose.is_empty() && !documented.doc.standards.is_empty());
    }

    // A page is generated from the widget's live key map and hints.
    for documented in &all {
        let desktop = (documented.bindings)(NavProfile::Desktop);
        assert!(
            !desktop.is_empty(),
            "{} lists its keys",
            documented.doc.name
        );
        let states: Vec<StateHints> = (documented.states)(NavProfile::Vim);
        assert!(
            !states.is_empty(),
            "{} lists its hint states",
            documented.doc.name
        );
        let text = page(documented);
        assert!(text.starts_with("<!-- Generated by"), "{text}");
        assert!(text.contains(&format!("# {}\n", documented.doc.name)));
        for section in [
            "## Purpose and role",
            "## Keyboard\n",
            "## Mouse\n",
            "## Hints by state",
            "## Knobs",
            "## Keyboard-only path",
            "## Mouse-first path",
            "## Standards",
        ] {
            assert!(text.contains(section), "{}: {section}", documented.doc.name);
        }
        for (state, _) in &states {
            assert!(text.contains(state), "the state {state} is listed");
        }
        for knob in documented.doc.knobs {
            assert!(text.contains(&format!("`{}`", knob.name)), "{}", knob.name);
        }
        for row in documented.doc.mouse {
            assert!(text.contains(row.target), "{}", row.target);
        }
    }

    // A custom doc through the same generator: one knob per layer, escaped cells.
    const KNOBS: [Knob; 3] = [
        Knob {
            name: "nav_profile",
            default: "Desktop",
            layer: KnobLayer::Profile,
            standard: "APG listbox",
        },
        Knob {
            name: "max_rows",
            default: "8",
            layer: KnobLayer::Widget,
            standard: "",
        },
        Knob {
            name: "keymap",
            default: "built in",
            layer: KnobLayer::Keymap,
            standard: "user remap",
        },
    ];
    const MOUSE: [MouseRow; 1] = [MouseRow {
        target: "Row | cell",
        click: "selects",
        double: "opens",
        secondary: "menu",
        drag: "none",
        wheel: "scrolls",
    }];
    const DOC: WidgetDoc = WidgetDoc {
        name: "Custom",
        file: "custom",
        role: "listbox",
        purpose: "A custom widget doc.",
        standards: &[("Example", "https://example.test/apg")],
        knobs: &KNOBS,
        mouse: &MOUSE,
    };
    let custom = Documented {
        doc: &DOC,
        bindings: |_| vec![],
        states: |_| vec![("idle", vec![])],
    };
    let text = page(&custom);
    assert!(
        text.contains("| `nav_profile` | Desktop | profile | APG listbox |"),
        "{text}"
    );
    assert!(text.contains("| `max_rows` | 8 | widget | — |"), "{text}");
    assert!(
        text.contains("| `keymap` | built in | key map | user remap |"),
        "{text}"
    );
    assert!(
        text.contains("| Row \\| cell | selects | opens | menu | none | scrolls |"),
        "{text}"
    );
    assert!(
        text.contains("[Example](https://example.test/apg)"),
        "{text}"
    );
    assert!(text.contains("| idle | — | — |"), "{text}");

    // The index and the file set.
    let listing = index(&all);
    for documented in &all {
        assert!(
            listing.contains(&format!(
                "[{}]({}.md)",
                documented.doc.name, documented.doc.file
            )),
            "{listing}"
        );
    }
    let written = files();
    assert_eq!(written.len(), all.len() + 1);
    assert_eq!(
        written.last().map(|(name, _)| name.as_str()),
        Some("README.md")
    );
    assert!(
        written
            .iter()
            .all(|(name, body)| name.ends_with(".md") && !body.is_empty())
    );
}

#[test]
fn catalog_shell_capability_table() {
    use junie_tui::catalog::{CAPABILITIES, CapKind, Capability, MEMBERS, find};

    for (kind, name) in [
        (CapKind::Type, "type"),
        (CapKind::Variant, "variant"),
        (CapKind::Builder, "builder"),
        (CapKind::Token, "token"),
        (CapKind::Function, "function"),
        (CapKind::State, "state"),
        (CapKind::Field, "field"),
        (CapKind::TraitItem, "trait_item"),
    ] {
        assert_eq!(kind.name(), name);
    }
    for (capability, kind) in [
        (Capability::ty("m::A"), CapKind::Type),
        (Capability::variant("m::A::B"), CapKind::Variant),
        (Capability::builder("m::A::b"), CapKind::Builder),
        (Capability::token("m::T"), CapKind::Token),
        (Capability::function("m::f"), CapKind::Function),
        (Capability::state("m::s"), CapKind::State),
        (Capability::field("m::A::x"), CapKind::Field),
        (Capability::trait_item("m::A::t"), CapKind::TraitItem),
    ] {
        assert_eq!(capability.kind, kind);
        assert!(capability.id.starts_with("m::"));
    }

    // The two tables hold each declared id once, and `find` reaches both.
    assert!(!CAPABILITIES.is_empty() && !MEMBERS.is_empty());
    let mut seen = std::collections::BTreeSet::new();
    for capability in CAPABILITIES.iter().chain(MEMBERS) {
        assert!(
            seen.insert(capability.id),
            "{} is declared twice",
            capability.id
        );
        let found = find(capability.id).expect("a declared id is found");
        assert_eq!((found.id, found.kind), (capability.id, capability.kind));
    }
    assert!(find("no::such::Capability").is_none());
    assert_eq!(
        find("catalog::find").map(|c| c.kind),
        Some(CapKind::Function)
    );
    assert_eq!(
        find("catalog::MEMBERS").map(|c| c.kind),
        Some(CapKind::Token)
    );
    assert_eq!(
        find("catalog::CAPABILITIES").map(|c| c.kind),
        Some(CapKind::Token)
    );
    assert_eq!(
        find("catalog::CapKind").map(|c| c.kind),
        Some(CapKind::Type)
    );
    assert_eq!(
        find("catalog::CapKind::Field").map(|c| c.kind),
        Some(CapKind::Variant)
    );
    for kind in [
        CapKind::Type,
        CapKind::Variant,
        CapKind::Builder,
        CapKind::Token,
        CapKind::Function,
    ] {
        assert!(
            CAPABILITIES.iter().chain(MEMBERS).any(|c| c.kind == kind),
            "{kind:?} is declared at least once"
        );
    }
}

#[test]
fn catalog_shell_external_latency_builder() {
    let (tx, rx) = external_channel();
    let rx = rx.latency(Duration::from_millis(1));
    assert!(tx.send(1_u8));
    assert!(tx.send(2_u8));
    assert_eq!(
        rx.try_recv().and_then(|m| m.downcast_ref::<u8>().copied()),
        Some(1),
        "the latency only paces the event loop: messages keep their order"
    );
    assert_eq!(ExternalReceiver::DEFAULT_LATENCY, Duration::from_millis(10));
}

const CHILD: &str = "JUNIE_TUI_CATALOG_SHELL_CHILD";
const CHILD_DONE: &str = "catalog-shell-child-done";

/// The command that re-runs one of this file's tests in a child process of
/// its own, so a test that changes process-wide state (the SIGTSTP action, the
/// controlling terminal) never touches the process that runs the others.
fn child_of(name: &str) -> std::process::Command {
    let module = module_path!().split_once("::").expect("crate prefix").1;
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command
        .args([
            "--exact",
            &format!("{module}::{name}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, name)
        .stdin(std::process::Stdio::null());
    command
}

fn is_child(name: &str) -> bool {
    std::env::var(CHILD).as_deref() == Ok(name)
}

fn assert_child_done(name: &str, output: &std::process::Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains(CHILD_DONE),
        "child {name} failed: {}\n--- stdout\n{stdout}\n--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Cooperative Ctrl+Z: the guard owns SIGTSTP, the handler only sets a flag,
/// `take_request` consumes it, and `stop_self` really stops the process until
/// the shell continues it.
#[cfg(unix)]
#[test]
fn catalog_shell_job_control_cycle() {
    use junie_tui::runtime::job_control::{self, JobControlGuard};
    use std::os::unix::process::CommandExt;
    const NAME: &str = "catalog_shell_job_control_cycle";

    if is_child(NAME) {
        let mut guard: JobControlGuard = job_control::install().expect("the first owner installs");
        let second = job_control::install()
            .map(|_| ())
            .expect_err("a second owner is refused");
        assert_eq!(second.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!job_control::take_request(), "nothing was requested yet");
        assert_eq!(unsafe { libc::raise(libc::SIGTSTP) }, 0);
        assert!(job_control::take_request(), "the handler left a request");
        assert!(
            !job_control::take_request(),
            "taking the request consumes it"
        );
        guard
            .stop_self()
            .expect("stop_self returns once the process is continued");
        assert!(
            !job_control::take_request(),
            "continuing is not a new request"
        );
        drop(guard);
        drop(job_control::install().expect("the claim is released on drop"));
        println!("{CHILD_DONE}");
        return;
    }
    let child = child_of(NAME)
        .process_group(0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn the child");
    let pid = child.id() as libc::pid_t;
    let mut status = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) };
    let stopped = waited == pid && libc::WIFSTOPPED(status);
    unsafe { libc::kill(pid, libc::SIGCONT) };
    let output = child.wait_with_output().expect("the child's output");
    assert!(
        stopped,
        "stop_self did not stop the child (status {status})"
    );
    assert_child_done(NAME, &output);
}

/// With no terminal at all, starting a session fails cleanly and leaves the
/// claim free for the next try, and the entry points built on it report the
/// same failure instead of hanging.
#[cfg(unix)]
#[test]
fn catalog_shell_terminal_session_without_a_terminal() {
    use junie_tui::core::keyboard::KeyboardProtocol;
    use junie_tui::runtime::{self, TerminalSession, drain_pending_input, set_focus_click_guard};
    use std::os::unix::process::CommandExt;
    const NAME: &str = "catalog_shell_terminal_session_without_a_terminal";

    struct Idle;
    impl Application for Idle {
        fn handle(&mut self, _input: Input) -> Outcome {
            Outcome::Ignored
        }
        fn render(&mut self, _frame: &mut ratatui::Frame) {}
        fn should_quit(&self) -> bool {
            true
        }
        fn tick_interval(&self) -> Duration {
            Duration::from_millis(10)
        }
    }

    if is_child(NAME) {
        let enter: fn() -> std::io::Result<TerminalSession> = TerminalSession::enter;
        let first = enter().map(|_| ()).expect_err("no terminal, no session");
        assert_ne!(first.kind(), std::io::ErrorKind::AlreadyExists);
        let second = TerminalSession::enter_with(KeyboardProtocol::Off)
            .map(|_| ())
            .expect_err("still no terminal");
        assert_ne!(
            second.kind(),
            std::io::ErrorKind::AlreadyExists,
            "the failed try released its claim"
        );
        assert!(
            drain_pending_input().is_err(),
            "raw mode cannot be switched on"
        );
        set_focus_click_guard(Duration::from_millis(50));
        assert!(runtime::run(&mut Idle).is_err());
        assert!(junie_tui::shell::run(&mut Idle).is_err());
        println!("{CHILD_DONE}");
        return;
    }
    let mut command = child_of(NAME);
    // A session of its own has no controlling terminal.
    let output = unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        })
    }
    .output()
    .expect("spawn the child");
    assert_child_done(NAME, &output);
}
