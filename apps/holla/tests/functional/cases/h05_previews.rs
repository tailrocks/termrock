//! Holla file previews, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h05_previews/` and each checkpoint's dots become underscores.
//! The shots are the open `big.log` preview after End, while the drawer is
//! still focused. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.
//! At 120 columns the list keeps the name and clips the kind tail
//! (`render_browse_list` stops when fewer than 6 columns remain), so
//! permission, special-file, and broken-link text is read from the preview
//! and the footer status.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const SITE_LABEL: &str = "Browse ~/work/site";
const SITE: &str = "~/work/site";
const SITE_ABS: &str = "/Users/alex/work/site";
const WIDE: &str = "H05.previews.preview.120x40";
const NARROW: &str = "H05.previews.preview.narrow";
const QUIT_DIALOG: &str = "Quit holla";
const HTML: &str = "<html>";
const HELLO: &str = "hello";
const SECRET: &str = "SECRET=1";
const SIXTEEN: &str = "16 entries";
const HIDDEN_ON: &str = "Hidden entries shown · from the same listing";
const HIDDEN_OFF: &str = "Hidden entries hidden";
// The preview panel at 120 columns cuts this sentence before "not shown".
const NUL: &str = "binary · NUL byte in the first 256 KiB";
const UTF8: &str = "binary · invalid UTF-8 · not shown";
// The 120-column preview pane cuts these sentences. The visible stems are
// still the FsError text: broken symbolic link, permission denied, and
// special file · not read.
const BROKEN_LINK: &str = "/Users/alex/work/site/broken: broken symbol";
const BROKEN_STATUS: &str = "broken · broken link → /Users/alex/work/site/missing";
const PRIVATE_DETAIL: &str = "/Users/alex/work/site/private: permission";
const PRIVATE_STATUS: &str = "private · permission denied";
const SPECIAL: &str = "/Users/alex/work/site/pipe: special file ·";
const PIPE_STATUS: &str = "pipe · special file";
const LONG_META: &str = "long lines cut at 4096";
const LONG_END: &str = "x …";
const FIRST_LINES: &str = "first 2000 lines";
const LINE_0: &str = "line 0";
const LINE_1500: &str = "line 1500";
const LINE_1999: &str = "line 1999";
const LINE_2099: &str = "line 2099";
const FIND_HIT: &str = "“line 1500”";
const FIND_COUNT: &str = "1 of 1";
const BACK: &str = "Back to list";
const LIST_TOP: u32 = 7;
const LIST_BOTTOM: u32 = 37;
const LIST_CHARS: usize = 48;

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h05_previews/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), "parity-browser".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Type Search");
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

fn title_path(screen: &TerminalSnapshot) -> String {
    let marker = "Files · ";
    for line in text(screen).lines() {
        if let Some(rest) = line.split(marker).nth(1) {
            return rest.split_whitespace().next().unwrap_or("").to_string();
        }
    }
    String::new()
}

fn status_path(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let Some(line) = visible.lines().find(|line| line.contains(" · hidden")) else {
        return String::new();
    };
    let Some(at) = line.find(" · hidden") else {
        return String::new();
    };
    line[..at]
        .split_whitespace()
        .next_back()
        .unwrap_or("")
        .to_string()
}

fn browser_path(screen: &TerminalSnapshot) -> String {
    let title = title_path(screen);
    let status = status_path(screen);
    assert!(
        status.is_empty() || status == title,
        "title {title:?} and status {status:?} disagree\n{}",
        text(screen)
    );
    title
}

fn at_site(screen: &TerminalSnapshot) -> bool {
    let title = title_path(screen);
    let status = status_path(screen);
    title == SITE && (status.is_empty() || status == title)
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn entry_name(line: &str) -> String {
    let Some(at) = line.find('▎') else {
        return String::new();
    };
    let body = line[at + '▎'.len_utf8()..].trim_start();
    let body = body.trim_start_matches(['▸', '→', '◇']).trim_start();
    body.split_whitespace().next().unwrap_or("").to_string()
}

fn finder_row(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains('›') && !line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn query_has(screen: &TerminalSnapshot, label: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains('▎') && line.contains(label)
    })
}

fn paste_browse(app: &App, label: &str) {
    app.paste(label);
    app.wait_screen("browse row", |screen| {
        query_has(screen, label) && finder_row(screen).contains(label)
    });
    app.key("enter");
}

fn open_site(app: &App) -> TerminalSnapshot {
    paste_browse(app, SITE_LABEL);
    app.wait_screen("site directory", |screen| {
        at_site(screen) && text(screen).contains(SIXTEEN) && text(screen).contains("index.html")
    })
}

fn assert_named(screen: &TerminalSnapshot, name: &str) {
    let line = focused_line(screen);
    assert_eq!(
        entry_name(&line),
        name,
        "focused row is not {name}\n{line}\n{}",
        text(screen)
    );
}

fn select_named(app: &App, name: &str) {
    let before = app.screen();
    let started = entry_name(&focused_line(&before));
    // Home is still in the PTY when this frame already shows `name`.
    // Returning then lets the caller's next key (Enter) follow Home onto
    // the first row. `assets` is that row, and Enter would open it.
    if started == name {
        assert_eq!(browser_path(&before), SITE, "{}", text(&before));
        return;
    }
    app.key("home");
    let sent = std::time::Instant::now();
    let mut screen = app.wait_screen("home", |screen| {
        let got = entry_name(&focused_line(screen));
        if got.is_empty() {
            return false;
        }
        // An unchanged row is the first row, where Home does not paint.
        // Do not treat `got == name` here: that matches the pre-Home frame.
        got != started || sent.elapsed() >= Duration::from_millis(250)
    });
    for step in 0..24 {
        let got = entry_name(&focused_line(&screen));
        if got == name {
            assert_eq!(browser_path(&screen), SITE, "{}", text(&screen));
            return;
        }
        app.key("down");
        let previous = got;
        screen = app.wait_screen(&format!("down toward {name} {step}"), |screen| {
            let got = entry_name(&focused_line(screen));
            !got.is_empty() && got != previous
        });
    }
    panic!("never selected {name}\n{}", text(&app.screen()));
}

fn log_rows(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .filter(|line| line.contains(" line "))
        .collect()
}

fn list_pane(screen: &TerminalSnapshot) -> String {
    (LIST_TOP..LIST_BOTTOM)
        .filter(|row| *row < screen.rows())
        .map(|row| {
            let line = row_text(screen, row);
            line.chars().take(LIST_CHARS).collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tail_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("big.log")
        && visible.contains(LINE_1999)
        && visible.contains(FIRST_LINES)
        && !visible.contains(LINE_2099)
        && !visible.contains(LINE_0)
        && at_site(screen)
        && footer(screen).contains(BACK)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the preview stays put.
fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(quiet(&screen), "nudge left the frame\n{}", text(&screen));
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    // The hidden-toggle status is still on the footer until sim time passes
    // 5s. Idle ticks are 200ms and advance that clock 80ms, so the line stays
    // for about 13s. The shot waits until it is gone.
    app.wait_screen("preview quiet", |screen| {
        tail_open(screen) && !footer(screen).contains(HIDDEN_OFF)
    });
    nudge(app, tail_open);
    app.visual(&visual_id(checkpoint));
}

fn quiet_exit(app: &App) {
    assert!(
        !text(&app.screen()).contains(QUIT_DIALOG),
        "quit dialog before Ctrl+Q\n{}",
        text(&app.screen())
    );
    app.finish();
}

fn footer_has(screen: &TerminalSnapshot, needle: &str) {
    let line = footer(screen);
    assert!(
        line.contains(needle),
        "footer missing {needle:?}\n{line}\n{}",
        text(screen)
    );
}

fn stay(screen: &TerminalSnapshot, name: &str) {
    assert_named(screen, name);
    assert_eq!(browser_path(screen), SITE, "{}", text(screen));
    assert!(
        !footer(screen).contains(BACK),
        "drawer still open\n{}",
        text(screen)
    );
}

#[test]
fn h05_previews_route() {
    let app = open("h05-previews-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[SITE, "mbp", "Type Search"],
        &["index.html", SIXTEEN, QUIT_DIALOG],
    );
    let page = open_site(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &["index.html", "assets", SIXTEEN, "Files · ~/work/site"],
        &[".env", SECRET, "todo.md", QUIT_DIALOG],
    );
    select_named(&app, "index.html");
    let preview = app.wait_screen("index.html body", |screen| {
        let visible = text(screen);
        visible.contains(HTML) && visible.contains(HELLO) && at_site(screen)
    });
    assert_named(&preview, "index.html");
    app.key("esc");
    let finder = app.wait_screen("finder after files", |screen| {
        text(screen).contains("Type Search")
            && finder_row(screen).contains(SITE_LABEL)
            && title_path(screen).is_empty()
    });
    see(
        &finder,
        &[SITE_LABEL, SITE, "mbp", "Type Search"],
        &["Files · ", "index.html", HTML, QUIT_DIALOG],
    );
    quiet_exit(&app);
}

#[test]
fn h05_previews_preview() {
    let app = open("h05-previews-preview");
    let page = open_site(&app);
    see(
        &page,
        &[SIXTEEN, "index.html", "assets", "big.log"],
        &[".env", ".git", SECRET, LINE_2099, QUIT_DIALOG],
    );

    select_named(&app, "index.html");
    let html = app.wait_screen("text preview", |screen| {
        let visible = text(screen);
        visible.contains(HTML) && visible.contains(HELLO)
    });
    assert_named(&html, "index.html");
    assert_eq!(browser_path(&html), SITE);

    select_named(&app, "empty.txt");
    let empty = app.wait_screen("empty preview", |screen| {
        let visible = text(screen);
        visible.contains("empty file") && visible.contains("Empty file")
    });
    assert_named(&empty, "empty.txt");

    select_named(&app, "logo.png");
    let nul = app.wait_screen("nul preview", |screen| text(screen).contains(NUL));
    assert_named(&nul, "logo.png");
    assert!(!footer(&nul).contains(BACK), "binary opened the drawer");

    select_named(&app, "bad.txt");
    let utf8 = app.wait_screen("utf-8 preview", |screen| text(screen).contains(UTF8));
    assert_named(&utf8, "bad.txt");

    select_named(&app, "current");
    let link = app.wait_screen("symlink preview", |screen| {
        let visible = text(screen);
        visible.contains("link →")
            && visible.contains("~/work/site/index.html")
            && visible.contains(HTML)
            && visible.contains(HELLO)
            && at_site(screen)
    });
    assert_named(&link, "current");

    select_named(&app, "broken");
    let broken = app.wait_screen("broken preview", |screen| {
        text(screen).contains(BROKEN_LINK) && at_site(screen)
    });
    assert_named(&broken, "broken");
    app.key("enter");
    let broken_stay = app.wait_screen("broken stays", |screen| {
        footer(screen).contains("broken · broken link →") && at_site(screen)
    });
    footer_has(&broken_stay, BROKEN_STATUS);
    assert!(text(&broken_stay).contains(&format!("{SITE_ABS}/missing")));
    stay(&broken_stay, "broken");

    select_named(&app, "private");
    let private = app.wait_screen("private preview", |screen| {
        text(screen).contains(PRIVATE_DETAIL)
    });
    assert_named(&private, "private");
    app.key("enter");
    let private_stay = app.wait_screen("private stays", |screen| {
        footer(screen).contains(PRIVATE_STATUS) && at_site(screen)
    });
    stay(&private_stay, "private");

    select_named(&app, "pipe");
    let pipe = app.wait_screen("fifo preview", |screen| text(screen).contains(SPECIAL));
    assert_named(&pipe, "pipe");
    assert!(text(&pipe).contains("special file"));
    app.key("enter");
    let pipe_stay = app.wait_screen("fifo stays", |screen| {
        footer(screen).contains(PIPE_STATUS) && at_site(screen)
    });
    stay(&pipe_stay, "pipe");

    select_named(&app, "long.txt");
    let long = app.wait_screen("long meta", |screen| text(screen).contains(LONG_META));
    assert_named(&long, "long.txt");
    app.key("enter");
    app.wait_screen("long drawer", |screen| {
        footer(screen).contains(BACK) && text(screen).contains(LONG_META)
    });
    app.key("shift+end");
    let revealed = app.wait_screen("long line end", |screen| text(screen).contains(LONG_END));
    assert!(
        footer(&revealed).contains(BACK),
        "shift+end closed the drawer\n{}",
        text(&revealed)
    );
    app.key("left");
    let long_back = app.wait_screen("long closed", |screen| {
        entry_name(&focused_line(screen)) == "long.txt" && at_site(screen)
    });
    stay(&long_back, "long.txt");

    select_named(&app, "index.html");
    let armed = app.screen();
    assert_named(&armed, "index.html");
    app.key("ctrl+h");
    // show_hidden flips before current(), so the old cursor index is read in
    // the new list. index.html is index 10, and that slot becomes `current`.
    let shown = app.wait_screen("hidden shown", |screen| {
        footer(screen).contains(HIDDEN_ON)
            && text(screen).contains(".env")
            && text(screen).contains(".git")
            && entry_name(&focused_line(screen)) == "current"
            && at_site(screen)
    });
    assert_named(&shown, "current");
    assert!(text(&shown).contains("18 entries"), "{}", text(&shown));
    assert!(!text(&shown).contains(SIXTEEN), "{}", text(&shown));

    select_named(&app, ".env");
    let secret = app.wait_screen("env preview", |screen| text(screen).contains(SECRET));
    assert_named(&secret, ".env");
    see(&secret, &[SECRET, ".env"], &[HTML, HELLO]);

    app.key("ctrl+h");
    // .env is index 3 while hidden rows are shown. Hiding them leaves big.log
    // in that slot, which is still a visible row in this directory.
    let hidden = app.wait_screen("hidden again", |screen| {
        footer(screen).contains(HIDDEN_OFF)
            && text(screen).contains(SIXTEEN)
            && entry_name(&focused_line(screen)) == "big.log"
            && at_site(screen)
    });
    see(
        &hidden,
        &[SIXTEEN, "big.log", "index.html"],
        &[".env", ".git", SECRET, HTML],
    );
    stay(&hidden, "big.log");

    select_named(&app, "big.log");
    app.key("enter");
    let opened = app.wait_screen("log drawer", |screen| {
        let visible = text(screen);
        footer(screen).contains(BACK)
            && visible.contains(FIRST_LINES)
            && visible.contains(LINE_0)
            && visible.contains("big.log")
            && !visible.contains(LINE_2099)
    });
    let pane = list_pane(&opened);
    assert!(pane.contains("big.log"), "list pane\n{pane}");
    let top = log_rows(&opened)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no log rows\n{}", text(&opened)));

    app.key("pagedown");
    let paged = app.wait_screen("page down", |screen| {
        list_pane(screen) == pane && !text(screen).contains(LINE_0)
    });
    let paged_top = log_rows(&paged)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no log rows after page down\n{}", text(&paged)));
    assert_ne!(paged_top, top, "page down did not scroll the preview");
    assert!(footer(&paged).contains(BACK));

    app.key("down");
    let down = app.wait_screen("preview down", |screen| {
        list_pane(screen) == pane
            && log_rows(screen)
                .into_iter()
                .next()
                .is_some_and(|row| row != paged_top)
    });
    let down_top = log_rows(&down).into_iter().next().unwrap_or_default();

    app.key("up");
    let up = app.wait_screen("preview up", |screen| {
        list_pane(screen) == pane
            && log_rows(screen)
                .into_iter()
                .next()
                .is_some_and(|row| row == paged_top)
    });
    assert_ne!(
        log_rows(&up).into_iter().next().unwrap_or_default(),
        down_top
    );

    app.key("end");
    let tail = app.wait_screen("log tail", |screen| {
        let visible = text(screen);
        list_pane(screen) == pane
            && visible.contains(LINE_1999)
            && !visible.contains(LINE_0)
            && !visible.contains(LINE_2099)
            && footer(screen).contains(BACK)
            && at_site(screen)
    });
    assert!(list_pane(&tail).contains("big.log"));
    assert_eq!((tail.cols(), tail.rows()), (120, 40));

    // Idle ticks are 200ms and each one advances the sim clock 80ms.
    // The hidden-toggle status lasts 5s of that clock, about 13s of wall time.
    std::thread::sleep(Duration::from_secs(13));
    shot(&app, WIDE);
    app.resize(80, 24);
    app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && tail_open(screen)
    });
    shot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && tail_open(screen)
    });

    app.key("/");
    app.paste(LINE_1500);
    let found = app.wait_screen("find line 1500", |screen| {
        let visible = text(screen);
        visible.contains(FIND_COUNT) && visible.contains(FIND_HIT) && visible.contains(LINE_1500)
    });
    assert!(text(&found).contains("find"), "{}", text(&found));
    app.key("esc");
    app.wait_screen("find closed", |screen| {
        let visible = text(screen);
        !visible.contains(FIND_COUNT)
            && visible.contains(LINE_1500)
            && footer(screen).contains(BACK)
            && !footer(screen).contains("Close find")
    });
    park_line(&app, LINE_1500);
    // Key::plain treats Shift as plain, and the preview owner closes on
    // plain Left before the viewport sees the key. Shift+Home stays on
    // this line and extends the selection across it.
    app.key("shift+home");
    app.key("y");
    let copied_log = app.wait_screen("copy log line", |screen| footer(screen).contains("Copied"));
    let log_status = footer(&copied_log);
    assert!(
        log_status.contains(LINE_1500),
        "copied status\n{log_status}"
    );
    assert!(
        !log_status.contains(HTML),
        "copied another file\n{log_status}"
    );
    assert!(
        !log_status.contains(SECRET),
        "copied another file\n{log_status}"
    );
    app.key("left");
    let log_closed = app.wait_screen("log closed", |screen| {
        entry_name(&focused_line(screen)) == "big.log" && at_site(screen)
    });
    stay(&log_closed, "big.log");

    select_named(&app, "index.html");
    app.key("enter");
    app.wait_screen("html drawer", |screen| {
        footer(screen).contains(BACK) && text(screen).contains(HTML) && text(screen).contains(HELLO)
    });
    app.key("shift+up");
    app.key("shift+up");
    app.key("y");
    // The log copy is still on the footer until this key lands.
    let copied_html = app.wait_screen("copy html", |screen| {
        let line = footer(screen);
        line.contains("Copied")
            && (line.contains(HELLO) || line.contains(HTML))
            && !line.contains(LINE_1500)
    });
    let html_status = footer(&copied_html);
    assert!(
        html_status.contains(HELLO) || html_status.contains(HTML),
        "copied status\n{html_status}"
    );
    assert!(
        !html_status.contains(LINE_1500),
        "copied another file\n{html_status}"
    );
    assert!(
        !html_status.contains(SECRET),
        "copied another file\n{html_status}"
    );
    app.key("esc");
    let cleared = app.wait_screen("selection cleared", |screen| {
        footer(screen).contains(BACK) && text(screen).contains(HTML) && text(screen).contains(HELLO)
    });
    assert_eq!(browser_path(&cleared), SITE);
    app.key("left");
    let html_closed = app.wait_screen("html closed", |screen| {
        entry_name(&focused_line(screen)) == "index.html" && at_site(screen)
    });
    stay(&html_closed, "index.html");
    quiet_exit(&app);
}

fn park_line(app: &App, needle: &str) {
    for step in 0..40 {
        let rows = log_rows(&app.screen());
        if rows.last().is_some_and(|row| row.contains(needle)) {
            return;
        }
        assert!(
            rows.iter().any(|row| row.contains(needle)),
            "{needle} left the preview before it was last\n{}",
            text(&app.screen())
        );
        let before = rows.last().cloned().unwrap_or_default();
        app.key("up");
        app.wait_screen(&format!("park {needle} {step}"), |screen| {
            let next = log_rows(screen);
            next.last()
                .is_some_and(|row| row.contains(needle) || row != &before)
        });
    }
    panic!(
        "{needle} never reached the last preview row\n{}",
        text(&app.screen())
    );
}
