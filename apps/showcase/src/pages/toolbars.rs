use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};

use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::stress::{self, Mode};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::NavKeys;
use termrock::core::pointer::MouseTarget;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::measure::{Tile, Tiled, tiles};
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text;
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};
use termrock::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};

const ID: WidgetId = WidgetId::of("toolbars");

fn query_items(running: bool) -> Vec<ToolItem> {
    vec![
        busy_if(
            ToolItem::new("Run")
                .shortcut("F5")
                .tip("Run the statement under the cursor"),
            running,
            "A query is running",
        ),
        busy_if(
            ToolItem::new("Run all")
                .shortcut("Shift+F5")
                .tip("Run every statement"),
            running,
            "A query is running",
        ),
        busy_if(
            ToolItem::new("Stop")
                .tip("Stop the running query")
                .separator(),
            !running,
            "No query is running",
        ),
        ToolItem::new("Format").tip("Format the script"),
        ToolItem::new("Explain")
            .tip("Show the query plan")
            .separator(),
        ToolItem::new("Wrap").toggle(false).tip("Wrap long lines"),
        ToolItem::new("Line numbers").toggle(true),
    ]
}

/// Flips Run, Run all and Stop in place. Only the items that change are
/// edited, so every other item, the cursor and the toggles keep their state.
fn set_running(bar: &mut Toolbar, running: bool) {
    for i in 0..bar.items().len() {
        let (off, why) = match bar.items()[i].label.as_str() {
            "Run" | MUTATED => (running, "A query is running"),
            "Stop" => (!running, "No query is running"),
            _ => continue,
        };
        bar.update_item(i, |item| {
            item.disabled = off;
            item.reason = off.then(|| why.to_owned());
        });
    }
}

fn busy_if(item: ToolItem, off: bool, why: &str) -> ToolItem {
    if off { item.reason(why) } else { item }
}

/// The narrow bar holds the query commands, or the stress data when it is set.
fn narrow_items(mode: Mode, running: bool) -> Vec<ToolItem> {
    match mode {
        Mode::Off => query_items(running),
        Mode::Items => stress::numbered("Action")
            .map(|label| ToolItem::new(&label))
            .collect(),
        Mode::Label => std::iter::once(ToolItem::new(&stress::long_label()))
            .chain(query_items(running))
            .collect(),
    }
}

const NARROW_W: u16 = 32;

/// Hidden labels named in the overflow readout, and the cells each keeps.
const BEHIND_SHOWN: usize = 8;
const BEHIND_LABEL_CELLS: usize = 40;

fn format_items() -> Vec<ToolItem> {
    vec![
        ToolItem::new("Bold").toggle(false),
        ToolItem::new("Italic").toggle(false),
        ToolItem::new("Code").toggle(false).separator(),
        ToolItem::new("Left").toggle(true),
        ToolItem::new("Center").toggle(false),
        ToolItem::new("Right").toggle(false),
    ]
}

pub struct ToolbarsPage {
    template: Template,
    pub query: Toolbar,

    pub narrow: Toolbar,
    pub format: Toolbar,
    pub running: bool,
    pub last: String,

    opts: [usize; 7],
}

const OPT_KEYS: [&str; 7] = [
    "toolbars.nav",
    "toolbars.wrap",
    "toolbars.skip_disabled",
    "toolbars.gap",
    "toolbars.overflow",
    "toolbars.mutate_open",
    "toolbars.stress",
];
const OPT_DEFAULTS: [usize; 7] = [0, 0, 1, 1, 0, 0, 0];

const MUTATED: &str = "Run all";
const STRESS: usize = 6;

impl ToolbarsPage {
    pub fn new() -> Self {
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            query: Toolbar::new(ID.sub("query"), query_items(false)),
            narrow: Toolbar::new(ID.sub("narrow"), narrow_items(Mode::Off, false)),
            format: Toolbar::new(ID.sub("format"), format_items()),
            running: false,
            last: "none yet · Activated(i) or Toggled(i, on)".into(),
            opts: OPT_DEFAULTS,
        };
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        let style = |t: &Toolbar, o: &[usize; 7]| {
            let cursor = t.cursor;
            let mut n = Toolbar::new(t.id, t.items().to_vec())
                .nav(if o[0] == 1 {
                    NavKeys::VIM
                } else {
                    NavKeys::DESKTOP
                })
                .wrap(o[1] == 1)
                .skip_disabled(o[2] == 1)
                .gap(o[3] as u16)
                .overflow(o[4] != 2);
            if o[4] == 1 {
                n = n.more_glyph("…");
            }
            n.cursor = cursor;
            n
        };
        self.query = style(&self.query, &self.opts);
        self.narrow = style(&self.narrow, &self.opts);
        self.format = style(&self.format, &self.opts);
    }

    fn bar_mut(&mut self, k: usize) -> &mut Toolbar {
        match k {
            0 => &mut self.query,
            1 => &mut self.narrow,
            _ => &mut self.format,
        }
    }

    fn mutates_open_menu(&self) -> bool {
        self.opts[5] == 1 && self.narrow.menu_open()
    }

    /// Removes the hidden "Run all" item, or puts it back, while the » menu is
    /// open, so the menu must close instead of acting on the changed bar.
    fn mutate_open_menu(&mut self, cx: &mut Cx) {
        let at = self.narrow.items().iter().position(|i| i.label == MUTATED);
        match at {
            Some(i) => {
                self.narrow.remove_item(i);
                cx.status(format!("{MUTATED} removed while » was open"));
            }
            None => {
                let item = query_items(self.running).remove(1);
                self.narrow.insert_item(1, item);
                cx.status(format!("{MUTATED} restored while » was open"));
            }
        }
    }

    fn act(&mut self, a: ToolbarAction, k: usize, cx: &mut Cx) {
        let label = match a {
            ToolbarAction::Activated(i) | ToolbarAction::Toggled(i, _) => {
                self.bar_mut(k).items()[i].label.clone()
            }
        };
        self.last = format!("{a:?} · {label}");
        if k < 2
            && let ToolbarAction::Activated(i) = a
        {
            match self.bar_mut(k).items()[i].label.as_str() {
                "Run" | MUTATED => self.running = true,
                "Stop" => self.running = false,
                _ => {}
            }

            set_running(&mut self.query, self.running);
            set_running(&mut self.narrow, self.running);
        }
        cx.status(self.last.clone());
    }

    fn state_text(&self) -> &'static str {
        if self.running {
            "running · Stop is enabled, Run waits"
        } else {
            "idle · Stop is disabled until a run starts"
        }
    }

    fn stress_mode(&self) -> Mode {
        stress::mode(stress::VALUES[self.opts[STRESS]])
    }

    fn behind_text(&self) -> String {
        let hidden = self.narrow.hidden();
        if hidden.is_empty() {
            return "everything fits: no » button".to_owned();
        }
        let named: Vec<String> = hidden
            .iter()
            .take(BEHIND_SHOWN)
            .map(|&i| {
                let it = &self.narrow.items()[i];
                let label = text::truncate(&it.label, BEHIND_LABEL_CELLS);
                match (&it.reason, it.disabled) {
                    (Some(r), true) => format!("{label} ({r})"),
                    _ => label,
                }
            })
            .collect();
        match hidden.len().saturating_sub(BEHIND_SHOWN) {
            0 => format!("behind »: {}", named.join(" · ")),
            more => format!("behind »: {} · and {more} more", named.join(" · ")),
        }
    }

    fn last_facts(&self) -> Vec<Prop> {
        vec![
            Prop::new("Action", self.last.clone()).wrap(),
            Prop::new(
                "Keys",
                "Tab enters once · ←→ move (disabled skipped) · Home End · Enter Space run",
            )
            .wrap(),
        ]
    }

    fn tiled(&self, width: u16) -> Tiled {
        let w = |s: &str| text::width(s) as u16;
        let card = |p: Panel, content: u16| p.outer(Size::new(content, 0)).width;
        let query_p = Panel::card(Some("Query toolbar")).meta(QUERY_META);
        let narrow_p = Panel::card(Some("Narrow toolbar")).meta(NARROW_META);
        let format_p = Panel::card(Some("Formatting toolbar")).meta("toggles");
        let last_p = Panel::card(Some("Last"));
        let facts = self.last_facts();
        let behind = self.behind_text();
        let query_w = card(
            Panel::card(Some("Query toolbar")).meta(QUERY_META),
            self.query.width().max(w(self.state_text())),
        );
        let narrow_w = card(
            Panel::card(Some("Narrow toolbar")).meta(NARROW_META),
            NARROW_W,
        );
        let format_w = card(
            Panel::card(Some("Formatting toolbar")).meta("toggles"),
            self.format.width(),
        );
        let query_h = |_: u16| query_p.outer(Size::new(0, 3)).height;
        let narrow_h = |w: u16| {
            let inner = narrow_p.inner(Size::new(w, 0)).width;
            let rows = text::wrap(&behind, inner.max(1) as usize).len() as u16;
            narrow_p.outer(Size::new(inner, 2 + rows)).height
        };
        let format_h = |_: u16| format_p.outer(Size::new(0, 1)).height;
        let last_h = |w: u16| sheet_height(&last_p, &facts, w);
        tiles(
            width,
            &[
                &[Tile::new(query_w, &query_h)],
                &[Tile::new(narrow_w, &narrow_h)],
                &[Tile::new(format_w, &format_h)],
                &[Tile::new(sheet_width(&last_p, &facts), &last_h)],
            ],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let tiled = self.tiled(area.width);

        let panel = Panel::card(Some("Query toolbar")).meta(QUERY_META);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(0, area), buf, t);
        self.query
            .render(Rect::new(inner.x, inner.y, inner.width, 1), buf, ctx, bg);
        buf.set_string(inner.x, inner.y + 2, self.state_text(), t.muted().bg(bg));

        let panel = Panel::card(Some("Narrow toolbar")).meta(NARROW_META);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(1, area), buf, t);
        let w = NARROW_W.min(inner.width);
        self.narrow
            .render(Rect::new(inner.x, inner.y, w, 1), buf, ctx, bg);
        let behind = self.behind_text();
        for (i, line) in text::wrap(&behind, inner.width.max(1) as usize)
            .iter()
            .enumerate()
        {
            buf.set_string(inner.x, inner.y + 2 + i as u16, line, t.muted().bg(bg));
        }

        let panel = Panel::card(Some("Formatting toolbar")).meta("toggles");
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(2, area), buf, t);
        self.format
            .render(Rect::new(inner.x, inner.y, inner.width, 1), buf, ctx, bg);

        let panel = Panel::card(Some("Last"));
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(3, area), buf, t);
        props::render(inner, buf, t, &self.last_facts(), bg);
    }
}

const QUERY_META: &str = "one Tab stop · ←→ · Enter";
const NARROW_META: &str = "» holds what does not fit";

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Selected,
            Disabled,
            Overflow,
            Narrow,
        ])
        .column_width(30)
        .narrow_width(16)
        .column(
            "Commands",
            Toolbar::new(
                id.child(0),
                vec![
                    ToolItem::new("Run").shortcut("F5"),
                    ToolItem::new("Stop").separator(),
                    ToolItem::new("Wrap").toggle(false),
                ],
            ),
        )
        .column(
            "Toggles",
            Toolbar::new(
                id.child(1),
                vec![
                    ToolItem::new("Bold").toggle(false),
                    ToolItem::new("Italic").toggle(false),
                    ToolItem::new("Code").toggle(false),
                ],
            ),
        )
}

impl Page for ToolbarsPage {
    fn title(&self) -> &'static str {
        "Toolbars"
    }

    fn blurb(&self) -> &'static str {
        "A row of commands that is one focus stop: shortcuts, tips, toggles, separators"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        self.tiled(avail.width).size
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if self.mutates_open_menu() && key.plain() && key.code == KeyCode::Char('m') {
                    self.mutate_open_menu(cx);
                    return Outcome::Changed;
                }
                let f = cx.focus.current();
                for k in 0..3 {
                    let bar = self.bar_mut(k);
                    if f == Some(bar.id) {
                        let (out, a) = bar.on_key(key);
                        if let Some(a) = a {
                            self.act(a, k, cx);
                        }
                        return out;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for k in 0..3 {
                    let bar = self.bar_mut(k);
                    let bid = bar.id;

                    if let Some(r) = bar.route(ev) {
                        if !ev.is(Phase::Click) {
                            return r.outcome();
                        }
                        let (out, actions) = r.into_parts();
                        cx.focus.focus(bid);
                        for a in actions {
                            self.act(a, k, cx);
                        }
                        return out.or(Outcome::Changed);
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.narrow.menu_open() {
            let mut keys = vec![("↑↓", "Move"), ("Enter", "Run"), ("Esc", "Close")];
            if self.mutates_open_menu() {
                keys.push(("m", "Edit bar"));
            }
            return keys;
        }
        if focus == Some(self.narrow.id) && self.narrow.on_more() {
            return vec![("←→", "Move"), ("Enter / ↓", "Show hidden")];
        }
        if [self.query.id, self.narrow.id, self.format.id]
            .iter()
            .any(|&id| focus == Some(id))
        {
            return vec![("←→", "Move"), ("Enter / Space", "Run")];
        }
        Vec::new()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        OPT_KEYS
            .iter()
            .position(|k| *k == key)
            .map(|i| self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if let Some(i) = OPT_KEYS.iter().position(|k| *k == key) {
            if i == STRESS {
                self.opts[i] = value.min(stress::VALUES.len() - 1);
                let _ = self
                    .narrow
                    .set_items(narrow_items(self.stress_mode(), self.running));
                self.narrow.cursor = 0;
            } else {
                self.opts[i] = value;
            }
            self.rebuild();
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

pub fn page() -> Box<dyn Page> {
    Box::new(ToolbarsPage::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use termrock::core::focus::{Focus, FocusRing};

    /// Activates the narrow bar's item labelled `label` as the » menu would.
    fn activate(page: &mut ToolbarsPage, label: &str) {
        let i = page
            .narrow
            .items()
            .iter()
            .position(|item| item.label == label)
            .expect("the narrow bar has the item");
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.act(ToolbarAction::Activated(i), 1, &mut cx);
    }

    fn without_run_all() -> ToolbarsPage {
        let mut page = ToolbarsPage::new();
        let at = page
            .narrow
            .items()
            .iter()
            .position(|item| item.label == MUTATED)
            .expect("the narrow bar starts with Run all");
        page.narrow.remove_item(at);
        page
    }

    #[test]
    fn stop_stops_the_run_after_run_all_left_the_bar() {
        let mut page = without_run_all();
        page.running = true;

        activate(&mut page, "Stop");

        assert!(
            !page.running,
            "Stop stops the run wherever it sits in the bar"
        );
    }

    #[test]
    fn an_item_that_moved_into_the_stop_slot_does_not_stop_the_run() {
        let mut page = without_run_all();
        page.running = true;

        activate(&mut page, "Format");

        assert!(page.running, "Format never touches the run");
    }

    fn stressed(value: &str) -> ToolbarsPage {
        let mut page = ToolbarsPage::new();
        let at = stress::VALUES
            .iter()
            .position(|v| *v == value)
            .expect("a stress value");
        page.set_option("toolbars.stress", at);
        page
    }

    #[test]
    fn stress_items_do_not_start_or_stop_a_run() {
        for value in ["20k items", "65k label"] {
            let mut page = stressed(value);
            for item in 0..3 {
                let label = page.narrow.items()[item].label.clone();
                if label == "Run" || label == MUTATED {
                    continue;
                }
                activate(&mut page, &label);
                assert!(!page.running, "{value}: activating {label:?} started a run");
            }
        }
    }

    #[test]
    fn run_all_and_stop_keep_working_after_a_stress_label_shifts_them() {
        let mut page = stressed("65k label");
        let run_all = page
            .narrow
            .items()
            .iter()
            .position(|i| i.label == MUTATED)
            .expect("Run all");
        assert_eq!(run_all, 2, "the long label shifts Run all by one");
        activate(&mut page, MUTATED);
        assert!(page.running, "Run all starts a run");
        activate(&mut page, "Stop");
        assert!(!page.running, "Stop ends it");
    }
}
