use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::KeyBinding;
use termrock::core::pointer::MouseTarget;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::{Split, SplitCmd, SplitDir, SplitKeys, StackChild};
use termrock::ui::master_detail::{MasterDetail, Mode};
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text::{thousands, truncate};
use termrock::widgets::button::Button;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop, PropsList};
use termrock::widgets::splitter::Splitter;

const ID: WidgetId = WidgetId::of("masterdetail");
const LIST: WidgetId = WidgetId::of("masterdetail").sub("tables");
const DETAIL: WidgetId = WidgetId::of("masterdetail").sub("facts");
const TOGGLE: WidgetId = WidgetId::of("masterdetail").sub("toggle");

const OPEN: WidgetId = WidgetId::of("masterdetail").sub("open");

struct Table {
    name: &'static str,
    rows: usize,
    size: &'static str,
    columns: usize,
    indexes: usize,
    owner: &'static str,
    analyzed: &'static str,
}

const TABLES: &[Table] = &[
    Table {
        name: "orders",
        rows: 12_408,
        size: "3.1 MB",
        columns: 14,
        indexes: 4,
        owner: "app",
        analyzed: "2 h ago",
    },
    Table {
        name: "order_items",
        rows: 48_221,
        size: "7.9 MB",
        columns: 9,
        indexes: 3,
        owner: "app",
        analyzed: "2 h ago",
    },
    Table {
        name: "customers",
        rows: 3_112,
        size: "820 kB",
        columns: 18,
        indexes: 5,
        owner: "app",
        analyzed: "1 d ago",
    },
    Table {
        name: "products",
        rows: 942,
        size: "312 kB",
        columns: 12,
        indexes: 2,
        owner: "catalog",
        analyzed: "3 d ago",
    },
    Table {
        name: "invoices",
        rows: 10_870,
        size: "2.4 MB",
        columns: 11,
        indexes: 3,
        owner: "billing",
        analyzed: "5 h ago",
    },
    Table {
        name: "payments",
        rows: 10_412,
        size: "1.9 MB",
        columns: 10,
        indexes: 3,
        owner: "billing",
        analyzed: "5 h ago",
    },
    Table {
        name: "shipments",
        rows: 9_655,
        size: "1.6 MB",
        columns: 13,
        indexes: 2,
        owner: "logistics",
        analyzed: "never",
    },
    Table {
        name: "audit_log",
        rows: 1_204_377,
        size: "412 MB",
        columns: 7,
        indexes: 1,
        owner: "ops",
        analyzed: "12 d ago",
    },
    Table {
        name: "sessions",
        rows: 88_014,
        size: "21 MB",
        columns: 6,
        indexes: 2,
        owner: "auth",
        analyzed: "1 h ago",
    },
    Table {
        name: "feature_flags",
        rows: 37,
        size: "16 kB",
        columns: 5,
        indexes: 1,
        owner: "ops",
        analyzed: "9 d ago",
    },
];

pub fn page() -> Box<dyn Page> {
    Box::new(MasterDetailPage::new())
}

pub struct MasterDetailPage {
    template: Template,
    opts: PageValues,
    md: MasterDetail,
    list: ListBox,
    facts: PropsList,
    toggle: Button,
    open: Button,

    shown: usize,
    sections: crate::pages::layout::Sections,
}

impl MasterDetailPage {
    pub fn new() -> Self {
        let items = TABLES
            .iter()
            .map(|t| ListItem::new(t.name).meta(&thousands(t.rows)))
            .collect();
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::MasterDetail)),
            md: MasterDetail::new(LIST, DETAIL),
            list: ListBox::new(LIST, items, SelectMode::Single),
            facts: PropsList::new(DETAIL, facts(&TABLES[0])),
            toggle: Button::secondary(TOGGLE, "Show details"),
            open: Button::secondary(OPEN, "Open table"),
            shown: 0,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let collapse = o.number("masterdetail.collapse").unwrap_or(80);
        let step = o.number("masterdetail.step").unwrap_or(2);
        let keys = match o.value("masterdetail.keys") {
            "custom" => SplitKeys::default().keymap(vec![
                (KeyBinding::ctrl('h'), SplitCmd::Left),
                (KeyBinding::ctrl('l'), SplitCmd::Right),
            ]),
            "none" => SplitKeys::default().keymap(vec![]),
            _ => SplitKeys::default(),
        }
        .step(step);
        let mut md = MasterDetail::new(LIST, DETAIL)
            .collapse_below(collapse)
            .summary_rows(o.number("masterdetail.summary").unwrap_or(1))
            .esc_returns(o.on("masterdetail.esc_returns"))
            .keys(keys)
            .gap(o.number("masterdetail.gap").unwrap_or(2))
            .detail_owns(&[OPEN])
            .seam(Splitter::new(LIST.sub("seam"), SplitDir::Horizontal));
        md = match o.value("masterdetail.size") {
            "share" => md.detail_share(40, 34, 56),
            "master" => md.master_share(33, 26, 40),
            "share-100" => md.detail_share(50, 100, 100),
            "master-100" => md.master_share(50, 100, 100),
            _ => md.resizable(Split::new(40, 24, 34)),
        };
        self.md = md;
    }

    fn frame_width(&self, avail: u16) -> u16 {
        self.opts
            .number("masterdetail.width")
            .map_or(avail, |w| w.min(avail))
    }

    fn sync_detail(&mut self) {
        let cur = self.list.cursor.min(TABLES.len() - 1);
        if cur != self.shown {
            self.shown = cur;
            self.facts.set_props(facts(&TABLES[cur]));
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let l = self
            .sections
            .layout(area, &[StackChild::flex(10, 24)], None, buf, ctx);
        match l.slot(0) {
            Some(r) => self.render_demo(r, buf, ctx),
            None => {
                for id in [TOGGLE, LIST, DETAIL, OPEN] {
                    ctx.focus_stop(id);
                }
            }
        }
    }

    fn render_demo(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.sync_detail();
        let t = ctx.theme;
        let w = self.frame_width(area.width);
        let frame = Rect::new(area.x, area.y, w, area.height);
        let mode = format!("{w} cols · collapses below {}", self.md.collapse_below);
        let panel = Panel::card(Some("Database browser")).meta(&mode);
        let bg = panel.bg(t);
        let inner = panel.render(frame, buf, t);
        self.toggle.label = self.md.menu_item().item.label;
        let tw = self.toggle.width();
        self.toggle.render(
            Rect::new(inner.x, inner.y, tw.min(inner.width), 1),
            buf,
            ctx,
            bg,
        );
        let body = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        let l = self.md.layout(body, buf, ctx, bg);
        if !l.master.is_empty() {
            let p = Panel::framed(Some("Tables"))
                .meta("10")
                .focused(ctx.interaction.focused(LIST));
            let pbg = p.bg(t);
            let r = p.render(l.master, buf, t);
            self.list.render(r, buf, ctx, pbg);
        }
        if !l.detail.is_empty() {
            let table = &TABLES[self.shown];
            let p = Panel::framed(Some(table.name))
                .meta(if l.mode == Mode::Drawer {
                    "Esc back"
                } else {
                    "facts"
                })
                .focused(ctx.interaction.focused(DETAIL));
            let pbg = p.bg(t);
            let r = p.render(l.detail, buf, t);

            let facts = Rect::new(r.x, r.y, r.width, r.height.saturating_sub(2));
            self.facts.render(facts, buf, ctx, pbg);
            if r.height > 2 && r.width > 2 {
                let bw = self.open.width().min(r.width - 1);
                self.open
                    .render(Rect::new(r.x + 1, r.bottom() - 1, bw, 1), buf, ctx, pbg);
            } else {
                ctx.focus_stop(OPEN);
            }
        }
        if !l.summary.is_empty() {
            let tb = &TABLES[self.shown];
            let line = format!(
                "→ {} · {} rows · {} · Tab shows details",
                tb.name,
                thousands(tb.rows),
                tb.size
            );
            buf.set_string(
                l.summary.x + 1,
                l.summary.y,
                truncate(&line, l.summary.width.saturating_sub(1) as usize),
                t.muted().bg(bg),
            );
        }
        self.md.finish(ctx);
    }
}

fn facts(t: &Table) -> Vec<Prop> {
    vec![
        Prop::new("Schema", "public"),
        Prop::new("Rows", thousands(t.rows)),
        Prop::new("Size", t.size),
        Prop::new("Columns", t.columns.to_string()),
        Prop::new("Indexes", t.indexes.to_string()),
        Prop::new("Owner", t.owner),
        Prop::new("Analyzed", t.analyzed),
        Prop::new("Path", format!("shop.public.{}", t.name)).copyable(),
    ]
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(12)
        .column_width(40)
        .rows(&[Rest, Hover, Pressed, Narrow])
        .column("Master-detail", MasterDetail::new(id.child(0), id.child(1)))
}

pub(crate) fn page_hint(h: termrock::widgets::keyhint::Hint) -> Hint {
    let key = match h.key {
        Cow::Borrowed(key) => key,
        Cow::Owned(_) => "",
    };
    let action = match h.action {
        Cow::Borrowed(s) => s,
        Cow::Owned(_) => "",
    };
    (key, action)
}

impl Page for MasterDetailPage {
    fn title(&self) -> &'static str {
        "Master-detail"
    }
    fn blurb(&self) -> &'static str {
        "MasterDetail: wide split with a resizable seam, narrow summary and drawer"
    }

    fn measure(&self, _avail: Size, ctx: &RenderCtx) -> Size {
        let height = self.sections.stack.measure(&[StackChild::flex(10, 24)]);
        if let Some(w) = self.opts.number("masterdetail.width") {
            return Size::new(w, height);
        }
        let list = self.list.measure(Size::ZERO, ctx);
        let master = Panel::framed(Some("Tables")).meta("10").outer(list).width;
        let detail = TABLES
            .iter()
            .map(|t| {
                let facts = facts(t);
                let content = Size::new(
                    props::width(&facts).max(self.open.width() + 1),
                    facts.len() as u16 + 2,
                );
                Panel::framed(Some(t.name))
                    .meta("facts")
                    .outer(content)
                    .width
            })
            .max()
            .unwrap_or(0);
        let mode = format!("999 cols · collapses below {}", self.md.collapse_below);
        let frame = Panel::card(Some("Database browser"))
            .meta(&mode)
            .outer(Size::new(self.md.natural_width(master, detail), 0))
            .width;
        Size::new(frame, height)
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
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if let Some(o) = self.md.on_key(key, cx.focus) {
                    return o;
                }
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == TOGGLE {
                    let (o, act) = self.toggle.on_key(key);
                    if act {
                        self.md.toggle_drawer(cx.focus);
                        return Outcome::Changed;
                    }
                    return o;
                }
                if f == LIST {
                    return self.list.on_key(key);
                }
                if f == DETAIL {
                    return self.facts.on_key(key).0;
                }
                if f == OPEN {
                    let (o, act) = self.open.on_key(key);
                    if act {
                        cx.status(format!("Opened {}", TABLES[self.shown].name));
                        return Outcome::Changed;
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.md.on_pointer(ev) {
                    return o;
                }
                if ev.id == TOGGLE && ev.is(Phase::Click) {
                    cx.focus.focus(TOGGLE);
                    if self.toggle.on_click() {
                        self.md.toggle_drawer(cx.focus);
                    }
                    return Outcome::Changed;
                }
                if ev.id == OPEN && ev.is(Phase::Click) {
                    cx.focus.focus(OPEN);
                    if self.open.on_click() {
                        cx.status(format!("Opened {}", TABLES[self.shown].name));
                    }
                    return Outcome::Changed;
                }
                if let Some(r) = self.list.route(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(LIST);
                    }
                    return r.outcome();
                }
                if let Some(r) = self.facts.route(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(DETAIL);
                    }
                    return r.into_parts().0;
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
        let mut v: Vec<Hint> = match focus {
            Some(LIST) => vec![("↑↓", "Move"), ("Tab", "Details")],
            Some(DETAIL) => vec![("↑↓", "Move")],
            Some(TOGGLE) | Some(OPEN) => vec![("Enter", "Activate"), ("Tab", "Next")],
            _ => Vec::new(),
        };
        v.extend(self.md.hints(focus).into_iter().map(page_hint));
        v
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
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
