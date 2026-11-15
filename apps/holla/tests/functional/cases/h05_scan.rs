//! Holla disk scan, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h05_scan/` and each checkpoint's dots become underscores.
//!
//! Catalog item `disk.usage` is labeled `Analyze disk usage` and launches
//! `Launch::Disk` of the scenario cwd. The page crumb is `Disk › Usage`.
//! `parity-disk-scan` does not open that page; its cwd paints `~/work/data`
//! and its meter is `611 of 926 GB`. `x` while a scan is running sets
//! `Scan cancelled · the partial tree stays and is labelled partial` and the
//! banner `partial · scan cancelled`. `r` starts a new scan. `disk.overview`
//! is `Disk overview` (`Launch::Disk` with an empty path). Enter on a home
//! folder is `Go::Analyze`. `disk-cleanup` pushes the disk page itself at
//! `~/work` with `842 of 926 GB` and `/Volumes/Backup`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const LABEL: &str = "Analyze disk usage";
const OVERVIEW: &str = "Disk overview";
const CRUMB: &str = "Disk › Usage";
const OVERVIEW_CRUMB: &str = "Disk › Overview";
const TITLE: &str = "Disk usage · ~/work/data";
const WORK_TITLE: &str = "Disk usage · ~/work";
const DOCUMENTS_TITLE: &str = "Disk usage · ~/Documents";
const DOCUMENTS: &str = "~/Documents";
const METER: &str = "611 of 926 GB";
const CLEANUP_METER: &str = "842 of 926 GB";
const BACKUP: &str = "/Volumes/Backup";
const COMPLETE: &str = "scan complete";
const PARTIAL: &str = "partial · scan cancelled";
const CANCELLED: &str = "Scan cancelled · the partial tree stays and is labelled partial";
const RESCAN: &str = "Rescanning · cached hints stay until live sizes arrive";
const LIVE: &str = "live · complete";
const UNREADABLE: &str = "2 unreadable · grant Full Disk Access to include them";
const HEADER: &str = "cached sizes are hints with their age · Enter analyses live";
const EVICTED: &str = "evicted to iCloud · never materialised by the scan";
const LINK_DETAIL: &str = "symbolic link · never traversed";
const FINDER: &str = "Search actions";
const QUIT: &str = "Quit holla";
const WIDE: &str = "H05.scan.scan.120x40";
const NARROW: &str = "H05.scan.scan.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h05_scan/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn launch_parity(name: &str) -> App {
    let app = launch(name, "parity-disk-scan");
    app.wait_text(FINDER);
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

fn collapsed(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn detail_origin(line: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(rel) = line[offset..].find('─') {
        let at = offset + rel;
        let run = line[at..].chars().take_while(|c| *c == '─').count();
        // The page rule is a long run of `─` after a space. The chrome rule
        // is the heavy `━` and must not be used, or wrapped detail values
        // are read through the tree column.
        if at > 40 && run >= 20 && line[..at].ends_with(' ') {
            return Some(at);
        }
        offset = at + run * '─'.len_utf8();
    }
    None
}

fn detail_collapsed(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let Some(origin) = lines.iter().find_map(|line| detail_origin(line)) else {
        return String::new();
    };
    lines
        .iter()
        .map(|line| line.get(origin..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_phrase(screen: &TerminalSnapshot, phrase: &str) -> bool {
    collapsed(screen).contains(phrase) || detail_collapsed(screen).contains(phrase)
}

fn screen_text(app: &App) -> String {
    text(&app.screen())
}

fn named(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('▎') else {
            return false;
        };
        let tail = &line[mark + '▎'.len_utf8()..];
        let Some(at) = tail.find(name) else {
            return false;
        };
        if at > 16 {
            return false;
        }
        let before = tail[..at].chars().next_back();
        let after = tail[at + name.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '.')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

fn cancelled(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PARTIAL) && visible.contains(CANCELLED) && !visible.contains(COMPLETE)
}

/// The reference capture is the cancel that froze two revealed nodes.
/// That banner is `3%` and `2 entries`, and `data` has its live size.
fn frozen_two(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PARTIAL)
        && visible.contains("3%")
        && visible.contains("2 entries")
        && visible.contains("852.0 KiB")
        && !visible.contains(COMPLETE)
}

fn count_before(line: &str, marker: &str) -> Option<u32> {
    let at = line.find(marker)?;
    let head = line[..at].trim_end();
    let start = head
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |index| index + 1);
    head[start..].parse().ok()
}

fn scanning_revealed(visible: &str) -> Option<u32> {
    let line = visible.lines().find(|line| line.contains("scanning · "))?;
    let rest = line.split_once("scanning · ")?.1;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// `Some((percent, entries))` from `scan cancelled at P% · N entries`.
fn cancelled_progress(visible: &str) -> Option<(u32, u32)> {
    let line = visible
        .lines()
        .find(|line| line.contains("scan cancelled at"))?;
    Some((count_before(line, "%")?, count_before(line, " entries")?))
}

fn restart_scan(app: &App) {
    let mut before = screen_text(app);
    app.key("r");
    let restarted = Instant::now();
    loop {
        let visible = screen_text(app);
        let revealed = scanning_revealed(&visible);
        // `r` redraws the new scan at zero revealed before the next tick.
        let fresh = visible != before
            && visible.contains(RESCAN)
            && !visible.contains(PARTIAL)
            && !visible.contains(COMPLETE);
        assert!(
            restarted.elapsed() < Duration::from_secs(3),
            "scan did not restart\n{visible}"
        );
        if fresh && matches!(revealed, Some(0) | Some(2)) {
            return;
        }
        if fresh && matches!(revealed, Some(n) if n > 2) {
            before = visible;
            app.key("r");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// `scanning · 2 of` is one sim tick. The next tick reveals four entries, and
/// cancel records whichever count is current. A late `x` stays late, so the
/// scan starts over and the key is sent again on the two-entry banner.
fn cancel_two(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "scan cancel did not freeze two entries\n{}",
            screen_text(app)
        );
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(QUIT) {
            panic!("quit guard during scan cancel\n{visible}");
        }
        let revealed = scanning_revealed(&visible);
        if revealed == Some(2) && !visible.contains(PARTIAL) && !visible.contains(COMPLETE) {
            see(
                &screen,
                &[CRUMB, TITLE, "scanning"],
                &[COMPLETE, QUIT, BACKUP],
            );
            app.key("x");
            loop {
                let screen = app.screen();
                if screen.cols() == 120 && screen.rows() == 40 && frozen_two(&screen) {
                    return screen;
                }
                let visible = text(&screen);
                let late = visible.contains(COMPLETE)
                    || matches!(cancelled_progress(&visible), Some((percent, entries)) if (percent, entries) != (3, 2))
                    || matches!(scanning_revealed(&visible), Some(n) if n > 2);
                if late {
                    restart_scan(app);
                    break;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(20),
                    "scan cancel did not freeze two entries\n{visible}"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            continue;
        }
        let missed = visible.contains(COMPLETE)
            || matches!(cancelled_progress(&visible), Some((percent, entries)) if (percent, entries) != (3, 2))
            || matches!(revealed, Some(n) if n > 2);
        if missed {
            restart_scan(app);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Busy rows paint `spinner_frame(now_ms / 80)`. Cancel freezes the revealed
/// count, and the next resize paints whichever frame the clock has reached.
/// Idle ticks advance that clock without redrawing, so sample resizes until
/// the frame is the first glyph. A later paint can still replace that glyph.
/// `visual` would lock the replacement, so compare the held first-glyph frame.
fn first_spinner(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains('⠋')
        && ["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .all(|glyph| !visible.contains(glyph))
}

fn spinner_frame_ready(
    screen: &TerminalSnapshot,
    cols: u16,
    rows: u16,
    ready: &impl Fn(&TerminalSnapshot) -> bool,
) -> bool {
    screen.cols() == u32::from(cols)
        && screen.rows() == u32::from(rows)
        && ready(screen)
        && first_spinner(screen)
}

/// The chrome names the size it laid out. A resize can report the new grid
/// while the previous layout, one column wider, is still on screen.
fn labelled_size(screen: &TerminalSnapshot, cols: u16, rows: u16) -> bool {
    text(screen).contains(&format!("{cols}×{rows}"))
}

/// The accepted paint: first glyph, frozen banner, cancel status, and the
/// chrome naming this size. A later resize can clear the footer before that
/// paint is on screen again.
fn holdable(screen: &TerminalSnapshot, cols: u16, rows: u16) -> bool {
    spinner_frame_ready(screen, cols, rows, &frozen_two)
        && labelled_size(screen, cols, rows)
        && text(screen).contains(CANCELLED)
}

/// The accepted frame is the first spinner glyph at this size, while the
/// cancel status is still up. A 200 ms identical hold skips that paint: the
/// layout corrects, or the glyph advances, and the status then expires.
fn capture_labelled_spinner(
    app: &App,
    cols: u16,
    rows: u16,
    ready: &impl Fn(&TerminalSnapshot) -> bool,
) -> Option<TerminalSnapshot> {
    let seen = app.screen();
    if !spinner_frame_ready(&seen, cols, rows, ready) {
        return None;
    }
    if labelled_size(&seen, cols, rows) && text(&seen).contains(CANCELLED) {
        return Some(seen);
    }
    // The grid is already the target size and the glyph is the first one.
    // Give the chrome one beat to drop the previous size from its status.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(150) {
        std::thread::sleep(Duration::from_millis(10));
        let seen = app.screen();
        if !spinner_frame_ready(&seen, cols, rows, ready) {
            return None;
        }
        if labelled_size(&seen, cols, rows) && text(&seen).contains(CANCELLED) {
            return Some(seen);
        }
    }
    None
}

fn settle_spinner(
    app: &App,
    cols: u16,
    rows: u16,
    ready: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let start = Instant::now();
    loop {
        let bumped = cols + 1;
        app.resize(bumped, rows);
        app.wait_screen("spinner bump", move |screen| {
            screen.cols() == u32::from(bumped) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        let screen = app.wait_screen("spinner sample", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
        });
        if let Some(held) = capture_labelled_spinner(app, cols, rows, &ready) {
            return held;
        }
        // The grid can already be the target size while the footer is still
        // blank. That paint has the banner and no cancel line. It is not the
        // status expiring; a complete frame names its size in the chrome.
        if labelled_size(&screen, cols, rows) && !text(&screen).contains(CANCELLED) {
            panic!(
                "cancel status left before the first spinner\n{}",
                text(&screen)
            );
        }
        if start.elapsed() > Duration::from_secs(12) {
            panic!("spinner did not reach the first frame\n{}", text(&screen));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str, screen: &TerminalSnapshot) {
    check_frame(app, screen, &visual_id(checkpoint));
}

fn open_catalog(app: &App, label: &str) {
    app.paste(label);
    let wanted = label.to_string();
    app.wait_screen("catalog row", move |screen| {
        text(screen).contains(&format!("▎ {wanted}"))
    });
    app.key("enter");
}

fn focus_named(app: &App, name: &str) {
    app.key("home");
    for _ in 0..120 {
        if named(&app.screen(), name) {
            return;
        }
        let before = screen_text(app);
        app.key("down");
        let wanted = name.to_string();
        let previous = before;
        app.wait_screen("cursor", move |screen| {
            named(screen, &wanted) || text(screen) != previous
        });
    }
    assert!(
        named(&app.screen(), name),
        "cursor never reached {name}\n{}",
        screen_text(app)
    );
}

fn expand_for(app: &App, child: &str) {
    let before = screen_text(app);
    app.key("right");
    let previous = before;
    let wanted = child.to_string();
    app.wait_screen("expanded", move |screen| {
        text(screen).contains(wanted.as_str()) || text(screen) != previous
    });
    until_visible(app, child);
}

fn until_visible(app: &App, needle: &str) {
    for _ in 0..120 {
        if screen_text(app).contains(needle) {
            return;
        }
        let before = screen_text(app);
        app.key("down");
        let wanted = needle.to_string();
        let previous = before;
        app.wait_screen("scroll", move |screen| {
            text(screen).contains(wanted.as_str()) || text(screen) != previous
        });
    }
    assert!(
        screen_text(app).contains(needle),
        "missing {needle}\n{}",
        screen_text(app)
    );
}

fn back_to_finder(app: &App) {
    for _ in 0..6 {
        let visible = screen_text(app);
        if !visible.contains("Disk ›") && visible.contains(FINDER) && !visible.contains(QUIT) {
            return;
        }
        app.key("escape");
        let previous = visible;
        app.wait_screen("toward finder", move |screen| text(screen) != previous);
    }
    let visible = screen_text(app);
    assert!(
        !visible.contains("Disk ›") && visible.contains(FINDER) && !visible.contains(QUIT),
        "finder did not return\n{visible}"
    );
}

#[test]
fn h05_scan_route_returns_to_the_finder() {
    let app = launch_parity("h05-scan-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[FINDER], &[CRUMB, QUIT]);

    open_catalog(&app, LABEL);
    let page = app.wait_screen("disk usage", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB) && visible.contains(TITLE) && !visible.contains(QUIT)
    });
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &[CRUMB, TITLE, METER],
        &[QUIT, BACKUP, CLEANUP_METER],
    );

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        !visible.contains(CRUMB) && visible.contains(LABEL) && !visible.contains(QUIT)
    });
    see(&finder, &[LABEL], &[CRUMB, QUIT, TITLE]);
    app.finish();
}

#[test]
fn h05_scan_scan_cancels_completes_and_changes_root() {
    let app = launch_parity("h05-scan-scan");
    open_catalog(&app, LABEL);
    let quiet = cancel_two(&app);
    see(
        &quiet,
        &[PARTIAL, CANCELLED, TITLE, METER, "3%", "2 entries"],
        &[COMPLETE, CLEANUP_METER, BACKUP, QUIT],
    );

    nudge(&app, frozen_two);
    let wide = settle_spinner(&app, 120, 40, frozen_two);
    shot(&app, WIDE, &wide);

    app.resize(80, 24);
    let narrowed = app.wait_screen("narrow cancelled tree", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && frozen_two(screen)
    });
    see(
        &narrowed,
        &[PARTIAL, TITLE],
        &[COMPLETE, CLEANUP_METER, BACKUP],
    );
    // The first 80×24 paint can already be the accepted frame. Nudging it
    // starts another spinner cycle and the footer is blank until the next draw.
    let narrow = if holdable(&narrowed, 80, 24) {
        narrowed
    } else {
        nudge(&app, frozen_two);
        settle_spinner(&app, 80, 24, frozen_two)
    };
    shot(&app, NARROW, &narrow);

    app.resize(120, 40);
    let restored = app.wait_screen("cancelled tree restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && cancelled(screen)
    });
    see(
        &restored,
        &[PARTIAL, CANCELLED, "~/work/data"],
        &[COMPLETE, CLEANUP_METER, BACKUP],
    );

    app.key("r");
    let rescanning = app.wait_screen("rescan", |screen| {
        text(screen).contains(RESCAN) && !text(screen).contains(COMPLETE)
    });
    see(&rescanning, &[RESCAN, TITLE], &[COMPLETE, PARTIAL]);

    let done = app.wait_screen("scan finished", |screen| {
        let visible = text(screen);
        visible.contains(COMPLETE)
            && visible.contains(LIVE)
            && visible.contains(UNREADABLE)
            && !visible.contains(PARTIAL)
    });
    see(
        &done,
        &[
            COMPLETE,
            LIVE,
            UNREADABLE,
            TITLE,
            METER,
            "sparse.img",
            ".hidden",
            "media",
            "locked",
            "cloud",
            "skip-me",
            "many",
        ],
        &[PARTIAL, "hint only", CLEANUP_METER, BACKUP, QUIT],
    );

    focus_named(&app, "media");
    let media = app.screen();
    assert!(
        named(&media, "media") && !text(&media).contains("hint only"),
        "revealed media still shows the cache hint\n{}",
        text(&media)
    );
    expand_for(&app, "a.mov");
    until_visible(&app, "a-copy.mov");
    until_visible(&app, "link");
    focus_named(&app, "link");
    let link = app.wait_screen("symlink detail", |screen| {
        named(screen, "link") && has_phrase(screen, LINK_DETAIL)
    });
    assert!(
        has_phrase(&link, LINK_DETAIL),
        "missing symlink detail\n{}",
        text(&link)
    );

    focus_named(&app, ".hidden");
    expand_for(&app, "h.bin");

    focus_named(&app, "locked");
    let locked = app.wait_screen("locked detail", |screen| {
        named(screen, "locked") && text(screen).contains("permission denied")
    });
    assert!(
        has_phrase(&locked, "permission denied"),
        "missing permission denied\n{}",
        text(&locked)
    );

    focus_named(&app, "cloud");
    let cloud = app.wait_screen("cloud detail", |screen| {
        named(screen, "cloud") && text(screen).contains("dataless") && has_phrase(screen, EVICTED)
    });
    assert!(
        has_phrase(&cloud, EVICTED)
            && text(&cloud).contains("dataless")
            && !text(&cloud).contains("hint only"),
        "missing dataless detail\n{}",
        text(&cloud)
    );

    focus_named(&app, "skip-me");
    expand_for(&app, "s.bin");

    focus_named(&app, "many");
    expand_for(&app, "f059.bin");
    until_visible(&app, "f000.bin");
    assert!(
        !screen_text(&app).contains("hint only"),
        "many must not paint a cache hint\n{}",
        screen_text(&app)
    );

    back_to_finder(&app);
    open_catalog(&app, OVERVIEW);
    let overview = app.wait_screen("disk overview", |screen| {
        let visible = text(screen);
        visible.contains(OVERVIEW_CRUMB) && visible.contains(HEADER) && !visible.contains(COMPLETE)
    });
    see(
        &overview,
        &[OVERVIEW_CRUMB, HEADER, "unscanned"],
        &[COMPLETE, "sparse.img", "stale", QUIT],
    );

    focus_named(&app, DOCUMENTS);
    app.key("enter");
    let documents = app.wait_screen("documents scan", |screen| {
        let visible = text(screen);
        visible.contains(DOCUMENTS_TITLE) && !visible.contains(TITLE) && !visible.contains(COMPLETE)
    });
    see(
        &documents,
        &[DOCUMENTS_TITLE],
        &[TITLE, "sparse.img", COMPLETE, QUIT],
    );

    back_to_finder(&app);
    let finder = app.screen();
    see(&finder, &[FINDER], &[CRUMB, QUIT, DOCUMENTS_TITLE]);
    app.finish();
}

#[test]
fn h05_scan_scenario_disk_cleanup_is_not_parity() {
    let app = launch("h05-scan-disk-cleanup", "disk-cleanup");
    let page = app.wait_screen("cleanup disk page", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB)
            && visible.contains(WORK_TITLE)
            && visible.contains(CLEANUP_METER)
            && visible.contains(BACKUP)
            && !visible.contains("sparse.img")
            && !visible.contains("evicted.bin")
            && !visible.contains(METER)
    });
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &[CRUMB, WORK_TITLE, CLEANUP_METER, BACKUP],
        &["sparse.img", "evicted.bin", METER, QUIT],
    );
    app.finish();
}
