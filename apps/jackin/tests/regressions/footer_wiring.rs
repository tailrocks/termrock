//! Regression for #75 through the real Jackin screens.
//!
//! The editor, settings and manager screens size their action footer from
//! `row_lines` and hand that height to `row_layout` / `row_layout_right`. A
//! screen that went back to a one-row allocation would silently drop every
//! wrapped action, and widget-level tests cannot see that wiring.
//!
//! Which surface wraps which footer (measured, see the probes in the PR):
//!
//! - Manager instance footer (six actions) wraps to two rows at widths 152 to
//!   184 on a 30 row terminal, where the list and detail panes sit side by
//!   side. That is a realistic size, so the manager test uses 160x30.
//! - Editor and settings footers (Cancel, Save…) need under 22 columns to
//!   wrap, and the narrowest supported surface is `TooSmall::MIN_WIDTH` (72).
//!   No supported width wraps them, so at 72x24 those tests assert the one-row
//!   placement and that both actions are hit-testable. They also drive the
//!   same screens through `App::draw` at 20 columns, below the supported
//!   minimum, which is the only place a one-row revert is observable.

#[path = "../common/mod.rs"]
mod common;

use common::*;
use jackin_console::app::App;
use jackin_console::fixtures::Scenario;
use jackin_console::screens::{editor, manager, settings};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::{HitRegion, HitRegistry};
use junie_tui::core::id::WidgetId;
use junie_tui::runtime::Application;
use junie_tui::shell::App as ShellApp;
use junie_tui::ui::ctx::RenderCtx;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

const SUPPORTED: (u16, u16) = (72, 24);
const BELOW_MINIMUM: (u16, u16) = (20, 24);
const MANAGER: (u16, u16) = (160, 30);

struct Painted {
    buf: Buffer,
    hits: HitRegistry,
}

fn paint(app: &mut App, (w, h): (u16, u16)) -> Painted {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    Painted {
        buf: terminal.backend().buffer().clone(),
        hits: app.shell.hits.clone(),
    }
}

fn paint_below_minimum(app: &mut App, (w, h): (u16, u16)) -> Painted {
    let area = Rect::new(0, 0, w, h);
    let theme = ShellApp::theme(app);
    let interaction = app
        .shell()
        .interaction(app.now_ms(), app.flash_ms(), app.tick());
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut buf = Buffer::empty(area);
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        app.configure(&mut ctx);
        app.draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Painted { buf, hits }
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
}

fn region_of(hits: &HitRegistry, id: WidgetId) -> Option<HitRegion> {
    hits.regions().iter().rev().find(|r| r.id == id).copied()
}

fn painted_label(buf: &Buffer, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| buf[(x, area.y)].symbol())
        .collect::<String>()
}

struct Action {
    id: WidgetId,
    label: &'static str,
}

fn action(id: WidgetId, label: &'static str) -> Action {
    Action { id, label }
}

/// Asserts every action is painted whole, hit-testable, inside the screen,
/// disjoint from its neighbours, and that its rows hold nothing but actions.
/// Returns the distinct rows the actions occupy, top first.
fn assert_footer(shown: &Painted, screen: (u16, u16), actions: &[Action]) -> Vec<u16> {
    let bounds = Rect::new(0, 0, screen.0, screen.1);
    let mut areas = Vec::new();
    for a in actions {
        let region = region_of(&shown.hits, a.id)
            .unwrap_or_else(|| panic!("{} has no hit region at {screen:?}", a.label));
        let area = region.area;
        assert!(
            !area.is_empty() && bounds.contains(Position::new(area.x, area.y)),
            "{} hit region {area:?} is empty or outside {bounds:?}",
            a.label
        );
        assert_eq!(
            area.intersection(bounds),
            area,
            "{} hit region {area:?} leaves {bounds:?}",
            a.label
        );
        assert_eq!(area.height, 1, "{} spans one row", a.label);
        let text = painted_label(&shown.buf, area);
        assert!(
            text.contains(a.label),
            "{} is not painted inside its hit region {area:?}: {text:?}",
            a.label
        );
        let hit = shown
            .hits
            .resolve(Position::new(area.x + area.width / 2, area.y))
            .unwrap_or_else(|| panic!("{} centre is not hit-testable", a.label));
        assert_eq!(
            hit.owner, region.owner,
            "{} centre resolves to another widget",
            a.label
        );
        areas.push(area);
    }
    for (i, a) in areas.iter().enumerate() {
        for b in &areas[i + 1..] {
            assert!(
                a.intersection(*b).is_empty(),
                "action regions overlap: {a:?} {b:?}"
            );
        }
    }

    let mut rows: Vec<u16> = areas.iter().map(|a| a.y).collect();
    rows.sort_unstable();
    rows.dedup();
    for &y in &rows {
        let lo = areas
            .iter()
            .filter(|a| a.y == y)
            .map(|a| a.x)
            .min()
            .unwrap();
        let hi = areas
            .iter()
            .filter(|a| a.y == y)
            .map(|a| a.right())
            .max()
            .unwrap();
        for x in lo..hi {
            let inside = areas.iter().any(|a| a.contains(Position::new(x, y)));
            let symbol = shown.buf[(x, y)].symbol();
            assert!(
                inside || symbol == " ",
                "row {y} holds {symbol:?} between actions at x={x}: {:?}",
                row_text(&shown.buf, y)
            );
        }
    }
    let top = rows[0];
    assert!(top > 0, "footer starts below the top row");
    let above = row_text(&shown.buf, top - 1);
    assert!(
        actions.iter().all(|a| !above.contains(a.label)),
        "an action label spilled into the row above the footer: {above:?}"
    );
    rows
}

/// Row of the bottom border of the Workspaces card, which sits at column 1 of
/// the manager screen. The detail pane's action footer ends one row above it.
fn card_bottom_row(buf: &Buffer) -> u16 {
    (0..buf.area.height)
        .rev()
        .find(|&y| buf[(1, y)].symbol() == "╰")
        .expect("Workspaces card bottom border at column 1")
}

fn assert_contiguous(rows: &[u16], count: usize, what: &str) {
    assert_eq!(rows.len(), count, "{what}: footer rows {rows:?}");
    assert!(
        rows.windows(2).all(|w| w[1] == w[0] + 1),
        "{what}: footer rows {rows:?} are not adjacent"
    );
}

fn editor_app() -> App {
    let mut app = app(Scenario::Returning);
    paint(&mut app, SUPPORTED);
    press(&mut app, "e");
    let shown = screen_at(&mut app, SUPPORTED.0, SUPPORTED.1);
    assert!(
        shown.contains("Cancel") && shown.contains("Save…"),
        "{shown}"
    );
    app
}

fn settings_app() -> App {
    let mut app = app(Scenario::Returning);
    paint(&mut app, SUPPORTED);
    press(&mut app, "s");
    let shown = screen_at(&mut app, SUPPORTED.0, SUPPORTED.1);
    assert!(
        shown.contains("Cancel") && shown.contains("Save…"),
        "{shown}"
    );
    app
}

fn manager_instance_app() -> App {
    let mut app = app(Scenario::Returning);
    paint(&mut app, MANAGER);
    press(&mut app, "down right down tab");
    app
}

fn manager_actions() -> [Action; 6] {
    [
        action(manager::DETAIL.sub("reconnect"), "Reconnect"),
        action(manager::DETAIL.sub("session"), "New session"),
        action(manager::DETAIL.sub("shell"), "Shell"),
        action(manager::DETAIL.sub("inspect"), "Inspect"),
        action(manager::DETAIL.sub("stop"), "Stop"),
        action(manager::DETAIL.sub("purge"), "Purge…"),
    ]
}

#[test]
fn manager_footer_wraps_into_reserved_rows_and_every_action_is_reachable() {
    let mut app = manager_instance_app();
    let shown = paint(&mut app, MANAGER);
    let rows = assert_footer(&shown, MANAGER, &manager_actions());
    assert_contiguous(&rows, 2, "manager at 160x30");
}

#[test]
fn manager_wrapped_action_on_the_second_row_responds_to_a_click() {
    let mut app = manager_instance_app();
    let shown = paint(&mut app, MANAGER);
    let purge = region_of(&shown.hits, manager::DETAIL.sub("purge")).expect("purge region");
    let first = region_of(&shown.hits, manager::DETAIL.sub("reconnect")).expect("first row");
    assert!(
        purge.area.y > first.area.y,
        "Purge… sits on a wrapped row below Reconnect: {purge:?} {first:?}"
    );
    let before = screen_at(&mut app, MANAGER.0, MANAGER.1);
    click(&mut app, purge.area.x + 2, purge.area.y);
    let after = screen_at(&mut app, MANAGER.0, MANAGER.1);
    assert!(
        !before.contains("▎Purge…"),
        "Purge… is focused before the click:\n{before}"
    );
    assert!(
        after.contains("▎Purge…") && after.contains("Enter Purge…"),
        "clicking the second-row Purge… did not focus it:\n{after}"
    );
    press(&mut app, "enter");
    let asked = screen_at(&mut app, MANAGER.0, MANAGER.1);
    assert!(
        asked.contains("Purge instance"),
        "Enter on the focused Purge… did not open the purge confirm dialog:\n{asked}"
    );
}

#[test]
fn manager_footer_keeps_every_action_when_the_terminal_is_short() {
    let mut app = manager_instance_app();
    let size = (MANAGER.0, 22);
    let shown = paint(&mut app, size);
    let rows = assert_footer(&shown, size, &manager_actions());
    assert_contiguous(&rows, 2, "manager at 160x22");
    let card_bottom = card_bottom_row(&shown.buf);
    assert_eq!(
        rows,
        vec![card_bottom - 2, card_bottom - 1],
        "footer is not the two rows above the card's bottom border at row {card_bottom}"
    );
}

#[test]
fn editor_footer_places_both_actions_on_one_row_at_the_narrowest_supported_size() {
    let mut app = editor_app();
    let shown = paint(&mut app, SUPPORTED);
    let rows = assert_footer(
        &shown,
        SUPPORTED,
        &[
            action(editor::CANCEL, "Cancel"),
            action(editor::SAVE, "Save…"),
        ],
    );
    assert_eq!(rows.len(), 1, "no supported width wraps the editor footer");
}

#[test]
fn editor_footer_wraps_into_the_reserved_rows_below_the_supported_minimum() {
    let mut app = editor_app();
    let shown = paint_below_minimum(&mut app, BELOW_MINIMUM);
    let rows = assert_footer(
        &shown,
        BELOW_MINIMUM,
        &[
            action(editor::CANCEL, "Cancel"),
            action(editor::SAVE, "Save…"),
        ],
    );
    assert_contiguous(&rows, 2, "editor at 20x24");
}

#[test]
fn settings_footer_places_both_actions_on_one_row_at_the_narrowest_supported_size() {
    let mut app = settings_app();
    let shown = paint(&mut app, SUPPORTED);
    let rows = assert_footer(
        &shown,
        SUPPORTED,
        &[
            action(settings::CANCEL, "Cancel"),
            action(settings::SAVE, "Save…"),
        ],
    );
    assert_eq!(
        rows.len(),
        1,
        "no supported width wraps the settings footer"
    );
}

#[test]
fn settings_footer_wraps_into_the_reserved_rows_below_the_supported_minimum() {
    let mut app = settings_app();
    let shown = paint_below_minimum(&mut app, BELOW_MINIMUM);
    let rows = assert_footer(
        &shown,
        BELOW_MINIMUM,
        &[
            action(settings::CANCEL, "Cancel"),
            action(settings::SAVE, "Save…"),
        ],
    );
    assert_contiguous(&rows, 2, "settings at 20x24");
}

#[test]
fn manager_actions_never_draw_above_the_detail_panel_on_a_short_terminal() {
    let mut checked = 0;
    for h in 6..=19 {
        let mut app = manager_instance_app();
        let shown = paint_below_minimum(&mut app, (MANAGER.0, h));
        let Some(panel) = region_of(&shown.hits, manager::DETAIL) else {
            continue;
        };
        for a in manager_actions() {
            let Some(region) = region_of(&shown.hits, a.id) else {
                continue;
            };
            checked += 1;
            assert!(
                region.area.y >= panel.area.y,
                "{} at height {h} sits at row {} above the panel top {}",
                a.label,
                region.area.y,
                panel.area.y
            );
        }
    }
    assert!(checked > 0, "no short terminal showed an action");
}
