//! Regression for #275: no Jackin screen or dialog may panic on a tiny terminal.
//!
//! Dialog frames and panels return an empty inner rect when the allocation is
//! smaller than their margins. Screens hung their rows one column into the
//! gutter with `inner.x - 1`, which underflowed on that rect, and several pages
//! wrote at fixed offsets past the bottom of the buffer. The sweep
//! opens every screen and dialog reachable through the real key path at a
//! normal size, then draws each distinct screen on a coarse grid of sizes from
//! 1x1 to 20x12 through the app's own `draw` (below the supported minimum,
//! which the shell would otherwise replace with the "too small" notice) and
//! through the shell's `Application::render`.

use crate::common::*;
use jackin_console::app::App;
use jackin_console::fixtures::Scenario;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::runtime::Application;
use junie_tui::shell::App as ShellApp;
use junie_tui::ui::ctx::RenderCtx;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

const KEYS: &[&str] = &[
    "enter", "esc", "tab", "down", "right", "up", "left", "space", "insert", "n", "e", "s", "d",
    "a", "r", "t", "p", "f", "i", "l", "o", "c", "x", "u", "q", "/", "?", "1", "2", "3", "4",
];

/// Every size from 1 to 6 on each axis (where the margins of panels, dialog
/// frames and the cockpit's minimum rows bite), then a few larger ones that
/// still sit below the supported 72x20.
const WIDTHS: &[u16] = &[1, 2, 3, 4, 5, 6, 9, 14, 20];
const HEIGHTS: &[u16] = &[1, 2, 3, 4, 5, 6, 8, 12];

fn draw_below_minimum(app: &mut App, area: Rect) {
    let theme = ShellApp::theme(app);
    let interaction = app
        .shell()
        .interaction(app.now_ms(), app.flash_ms(), app.tick());
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut buf = Buffer::empty(area);
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    app.configure(&mut ctx);
    app.draw(area, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
}

fn draw_through_shell(app: &mut App, (w, h): (u16, u16)) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
}

fn opened(scenario: Scenario, keys: &str) -> App {
    let mut app = app(scenario);
    screen(&mut app);
    press(&mut app, keys);
    app
}

#[derive(Default)]
struct Failures(BTreeMap<String, (usize, String)>);

impl Failures {
    fn record(&mut self, site: String, example: String) {
        let entry = self.0.entry(site).or_insert((0, example));
        entry.0 += 1;
    }

    fn report(&self) -> String {
        self.0
            .iter()
            .map(|(site, (count, example))| {
                format!("{count:5} draws panic at {site}, first {example}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

thread_local! {
    static SITE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn record_panic_site(info: &std::panic::PanicHookInfo<'_>) {
    let own = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .filter(|at| at.starts_with("src/") || at.starts_with("apps/"));
    let at = own.unwrap_or_else(|| {
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        trace
            .lines()
            .map(str::trim)
            .find(|l| {
                (l.contains("jackin_console::") || l.contains("junie_tui::"))
                    && !l.contains("regressions::")
            })
            .unwrap_or("outside Jackin and the library")
            .to_owned()
    });
    SITE.with(|s| *s.borrow_mut() = at);
}

fn panic_site() -> String {
    SITE.with(|s| s.borrow().clone())
}

fn sweep(scenario: Scenario, keys: &str, mut app: App, failures: &mut Failures) {
    for &h in HEIGHTS {
        for &w in WIDTHS {
            let drawn = catch_unwind(AssertUnwindSafe(|| {
                draw_below_minimum(&mut app, Rect::new(0, 0, w, h));
                if keys.is_empty() {
                    draw_through_shell(&mut app, (w, h));
                }
            }));
            if drawn.is_err() {
                failures.record(
                    panic_site(),
                    format!("{} [{keys}] at {w}x{h}", scenario.name()),
                );
            }
        }
    }
}

#[test]
fn every_jackin_screen_and_dialog_draws_on_tiny_terminals() {
    let mut failures = Failures::default();
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(record_panic_site));
    let mut seen = std::collections::HashSet::new();
    let singles = Scenario::ALL
        .into_iter()
        .flat_map(|s| std::iter::once((s, "")).chain(KEYS.iter().map(move |k| (s, *k))));
    for (scenario, keys) in singles.chain(DEEPER.iter().copied()) {
        let mut app = opened(scenario, keys);
        if seen.insert(screen(&mut app)) {
            sweep(scenario, keys, app, &mut failures);
        }
    }
    std::panic::set_hook(quiet);
    assert!(
        failures.0.is_empty(),
        "screens panicked on tiny terminals:\n{}",
        failures.report()
    );
}

#[test]
fn manager_instance_detail_draws_in_a_wide_five_row_terminal() {
    let mut app = app(Scenario::Returning);
    screen_at(&mut app, 160, 30);
    press(&mut app, "down right down tab");
    draw_below_minimum(&mut app, Rect::new(0, 0, 160, 5));
    draw_through_shell(&mut app, (160, 5));
}

/// Depth-two key paths found by exploring the real key bindings: each opens a
/// dialog or a screen that a single key from the scenario's first frame does
/// not reach.
const DEEPER: &[(Scenario, &str)] = &[
    (Scenario::FirstUse, "esc enter"),
    (Scenario::FirstUse, "esc n"),
    (Scenario::FirstUse, "esc ?"),
    (Scenario::FirstUse, "down ?"),
    (Scenario::FirstUse, "n enter"),
    (Scenario::FirstUse, "n tab"),
    (Scenario::FirstUse, "n down"),
    (Scenario::FirstUse, "n space"),
    (Scenario::FirstUse, "n d"),
    (Scenario::FirstUse, "e enter"),
    (Scenario::FirstUse, "e n"),
    (Scenario::FirstUse, "e ?"),
    (Scenario::FirstUse, "s ?"),
    (Scenario::FirstUse, "o enter"),
    (Scenario::FirstUse, "o n"),
    (Scenario::FirstUse, "o ?"),
    (Scenario::FirstUse, "c a"),
    (Scenario::FirstUse, "c /"),
    (Scenario::FirstUse, "c ?"),
    (Scenario::FirstUse, "u ?"),
    (Scenario::Returning, "enter down"),
    (Scenario::Returning, "enter space"),
    (Scenario::Returning, "enter o"),
    (Scenario::Returning, "esc enter"),
    (Scenario::Returning, "esc n"),
    (Scenario::Returning, "esc ?"),
    (Scenario::Returning, "down enter"),
    (Scenario::Returning, "down d"),
    (Scenario::Returning, "down ?"),
    (Scenario::Returning, "n enter"),
    (Scenario::Returning, "n tab"),
    (Scenario::Returning, "n down"),
    (Scenario::Returning, "n space"),
    (Scenario::Returning, "n d"),
    (Scenario::Returning, "e ?"),
    (Scenario::Returning, "s ?"),
    (Scenario::Returning, "o enter"),
    (Scenario::Returning, "o n"),
    (Scenario::Returning, "o ?"),
    (Scenario::Returning, "c a"),
    (Scenario::Returning, "c /"),
    (Scenario::Returning, "c ?"),
    (Scenario::Returning, "u ?"),
    (Scenario::LaunchFailure, "esc n"),
    (Scenario::LaunchFailure, "esc p"),
    (Scenario::LaunchFailure, "esc i"),
    (Scenario::LaunchFailure, "esc ?"),
    (Scenario::LaunchFailure, "tab down"),
    (Scenario::LaunchFailure, "down enter"),
    (Scenario::LaunchFailure, "down down"),
];
