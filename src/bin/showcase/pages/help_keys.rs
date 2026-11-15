use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{KeyBinding, NavKeys, label_for};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::{Tile, Tiled, tiles};
use junie_tui::ui::popup::Dismiss;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::help::{HelpCmd, HelpOverlay};
use junie_tui::widgets::keyref::KeyReference;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("helpkeys");

pub const OPEN: WidgetId = WidgetId::of("helpkeys.open");

pub const HELP: WidgetId = WidgetId::of("helpkeys.help");

fn sections() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "Navigation",
            vec![
                ("Ctrl+P", "open a file by name"),
                ("Ctrl+G", "go to line"),
                ("Ctrl+Tab", "next editor tab"),
                ("Alt+←/→", "back / forward"),
                ("F12", "go to definition"),
            ],
        ),
        (
            "Editing",
            vec![
                ("Ctrl+Z", "undo"),
                ("Ctrl+Y", "redo"),
                ("Ctrl+D", "select the next match"),
                ("Alt+↑/↓", "move the line"),
                ("Ctrl+/", "toggle a comment"),
                ("Tab", "indent the selection"),
            ],
        ),
        (
            "Search",
            vec![
                ("Ctrl+F", "find in the file"),
                ("Enter", "next match"),
                ("Shift+Enter", "previous match"),
                ("Ctrl+H", "replace"),
            ],
        ),
        (
            "View",
            vec![
                ("Ctrl+B", "show / hide the sidebar"),
                ("Ctrl+J", "show / hide the panel"),
                ("Ctrl+=", "zoom in"),
                ("Ctrl+-", "zoom out"),
            ],
        ),
        (
            "Window",
            vec![
                ("Ctrl+W", "close the tab"),
                ("Ctrl+Shift+T", "reopen the closed tab"),
                ("Ctrl+\\", "split the editor"),
                ("Ctrl+Q", "quit"),
            ],
        ),
    ]
}

fn reference() -> KeyReference {
    KeyReference::new()
        .section("Results")
        .key("↑↓ PgUp PgDn", "move between rows")
        .key("Enter", "open the row in the detail pane")
        .key("Ctrl+C", "copy the selected cells as tab-separated text")
        .section("Query")
        .key("Ctrl+Enter", "run the statement under the cursor")
        .key(
            "Ctrl+Shift+Enter",
            "run every statement in the editor, top to bottom",
        )
        .key("Esc", "cancel the running query")
}

pub struct HelpKeysPage {
    template: Template,
    open: Button,

    opts: Vec<usize>,
}

impl HelpKeysPage {
    pub fn new() -> Self {
        Self {
            template: Template::new(ID, states_grid()),
            open: Button::secondary(OPEN, "Open help overlay"),
            opts: page_options(PageId::HelpKeys)
                .iter()
                .map(|o| o.default)
                .collect(),
        }
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::HelpKeys);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    pub fn build_help(&self) -> HelpOverlay {
        let content = match self.opt("helpkeys.content") {
            "none" => Vec::new(),
            _ => sections(),
        };
        let mut h = HelpOverlay::new(HELP, "Editor", content)
            .max_percent(self.opt("helpkeys.max_percent").parse().unwrap_or(80))
            .nav(match self.opt("helpkeys.nav") {
                "vim" => NavKeys::VIM,
                _ => NavKeys::default(),
            })
            .dismiss(match self.opt("helpkeys.dismiss") {
                "escape" => Dismiss::ESCAPE,
                _ => Dismiss::LIGHT,
            });
        if self.opt("helpkeys.title") == "custom" {
            h = h.title("Editor keys");
        }
        if self.opt("helpkeys.keymap") == "f1" {
            h = h.keymap(vec![(
                KeyBinding::key(ratatui::crossterm::event::KeyCode::F(1)),
                HelpCmd::Close,
            )]);
        }
        h
    }

    fn build_reference(&self) -> KeyReference {
        reference()
            .key_max_share(self.opt("helpkeys.key_max_share").parse().unwrap_or(40))
            .gap(self.opt("helpkeys.gap").parse().unwrap_or(3))
    }

    fn ref_width(&self) -> usize {
        match self.opt("helpkeys.width") {
            "full" => (REF_W - 4) as usize,
            n => n.parse().unwrap_or(40),
        }
    }

    fn help_facts(&self) -> Vec<Prop> {
        let help = self.build_help();

        let first = label_for(&help.keymap, HelpCmd::Close).unwrap_or_else(|| "none".into());
        let rest: Vec<String> = help
            .keymap
            .iter()
            .filter(|(_, a)| *a == HelpCmd::Close)
            .map(|(b, _)| b.label())
            .filter(|l| *l != first)
            .collect();
        let close = std::iter::once(first)
            .chain(rest)
            .collect::<Vec<_>>()
            .join(" ");
        vec![
            Prop::new("Title", help.title.clone()).wrap(),
            Prop::new("Scope", help.scope.clone()).wrap(),
            Prop::new("Max share", format!("{}%", help.max_percent)),
            Prop::new(
                "Scroll keys",
                if help.nav == NavKeys::VIM {
                    "↑↓ PgUp PgDn Home End j k g G"
                } else {
                    "↑↓ PgUp PgDn Home End"
                },
            )
            .wrap(),
            Prop::new("HelpCmd::Close", format!("{close} · Esc · Enter")).wrap(),
            Prop::new(
                "Dismiss",
                if help.dismiss.outside_click {
                    "light: Esc, ×, outside"
                } else {
                    "Esc and × only"
                },
            )
            .wrap(),
        ]
    }

    fn tiled(&self, width: u16) -> Tiled {
        let lines = self.build_reference().lines(self.ref_width()).len() as u16;
        let ref_h = |_: u16| lines + 3;
        let facts = self.help_facts();
        let panel = Panel::card(Some("HelpOverlay"));
        let card_h = |w: u16| sheet_height(&panel, &facts, w) + 2;
        let right_h = |w: u16| card_h(w) + 1 + text::wrap(NOTE, w as usize).len() as u16;
        let right_w = sheet_width(&panel, &facts).max(self.open.width() + 4);
        tiles(
            width,
            &[&[
                Tile::new(REF_W, &ref_h).fixed(),
                Tile::new(right_w, &right_h),
            ]],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let tiled = self.tiled(area.width);
        let (left, right) = (tiled.at(0, area), tiled.at(1, area));

        let r = self.build_reference();
        let w = self.ref_width();
        let lines = r.lines(w);
        let title = format!("KeyReference at width {w}");
        let meta = format!("share {}% · gap {}", r.key_max_share, r.gap.max(3));
        let panel = Panel::card(Some(&title)).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(left, buf, t);
        for (i, l) in lines.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            let heading = matches!(l.as_str(), "Results" | "Query");
            let st = if heading {
                t.secondary()
                    .bg(bg)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                t.secondary().bg(bg)
            };
            buf.set_string(inner.x, y, l, st);

            let edge = inner.x + w as u16;
            if edge < inner.right() {
                buf.set_string(edge, y, "┊", t.faint().bg(bg));
            }
        }

        let facts = self.help_facts();
        let panel = Panel::card(Some("HelpOverlay"));
        let bg = panel.bg(t);
        let h = sheet_height(&panel, &facts, right.width) + 2;
        let inner = panel.render(Rect::new(right.x, right.y, right.width, h), buf, t);
        let bw = self.open.width();
        self.open
            .render(Rect::new(inner.x, inner.y, bw, 1), buf, ctx, bg);
        props::render(
            Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(2),
            ),
            buf,
            t,
            &facts,
            bg,
        );
        for (i, l) in text::wrap(NOTE, right.width as usize).iter().enumerate() {
            let y = right.y + h + 1 + i as u16;
            buf.set_string(right.x, y, l, t.faint().bg(t.canvas));
        }
    }
}

const REF_W: u16 = 58;

const NOTE: &str = "It opens as the shell's modal and never over another one. \
                    The showcase's own ? opens one too, listing the shell's keys.";

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    StateGrid::new(ID.sub("states"))
        .rows(&[Rest, Overflow, Narrow])
        .column_width(40)
        .column(
            "Reference",
            KeyReference::new()
                .section("Query")
                .key("Ctrl+Enter", "run the statement")
                .key("Esc", "cancel"),
        )
        .column(
            "Narrow keys",
            KeyReference::new()
                .key_max_share(25)
                .key("Ctrl+Enter", "run the statement")
                .key("Esc", "cancel"),
        )
}

impl Page for HelpKeysPage {
    fn title(&self) -> &'static str {
        "Help & keys"
    }

    fn blurb(&self) -> &'static str {
        "HelpOverlay and KeyReference: sections, columns, scrolling, hanging wrap"
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
        let open = match ev {
            PageEvent::Key(key) if cx.focus.is(OPEN) => {
                let (o, act) = self.open.on_key(key);
                if !act {
                    return o;
                }
                true
            }
            PageEvent::Pointer(p) if p.is(Phase::Click) && p.on(OPEN) => self.open.on_click(),
            _ => return Outcome::Ignored,
        };
        if open {
            cx.focus.focus(OPEN);
            cx.open_help(self.build_help());
        }
        Outcome::Changed
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(OPEN) {
            return vec![("Enter", "Open help")];
        }
        Vec::new()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::HelpKeys)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::HelpKeys);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
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
    Box::new(HelpKeysPage::new())
}
