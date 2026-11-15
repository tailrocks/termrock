use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::stress::{self, Mode};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::widgets::breadcrumb::Breadcrumb;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::rail::{RailAction, SidebarRail};

const ID: WidgetId = WidgetId::of("breadcrumbsrails");

pub const LIST: WidgetId = WidgetId::of("breadcrumbsrails.list");

pub const CRUMBS: WidgetId = WidgetId::of("breadcrumbsrails.crumbs");

pub const RAIL: WidgetId = WidgetId::of("breadcrumbsrails.rail");

const DEEP: [&str; 6] = [
    "Disk",
    "Users",
    "alex",
    "Projects",
    "payments-platform",
    "settlement-backoff-retry-policy-v2.md",
];

type Entry = (&'static str, &'static str, bool);

fn entries(path: &[String]) -> Vec<Entry> {
    match path.join("/").as_str() {
        "Disk" => vec![
            ("Applications", "folder", true),
            ("Library", "folder", true),
            ("System", "locked", false),
            ("Users", "folder", true),
        ],
        "Disk/Users" => vec![("alex", "folder", true), ("Shared", "folder", true)],
        "Disk/Users/alex" => vec![
            ("Desktop", "folder", true),
            ("Documents", "folder", true),
            ("Downloads", "folder", true),
            ("Projects", "folder", true),
        ],
        "Disk/Users/alex/Projects" => vec![
            ("design-system", "folder", true),
            ("payments-platform", "folder", true),
            ("notes.md", "4 KB", false),
        ],
        "Disk/Users/alex/Projects/payments-platform" => vec![
            ("src", "folder", true),
            ("Cargo.toml", "1 KB", false),
            ("settlement-backoff-retry-policy-v2.md", "12 KB", false),
        ],
        "Disk/Applications" => vec![("Terminal", "app", false), ("Xcode", "app", false)],
        "Disk/Library" => vec![("Caches", "folder", true), ("Fonts", "folder", true)],
        _ => vec![],
    }
}

pub struct BreadcrumbsRailsPage {
    template: Template,

    path: Vec<String>,
    list: ListBox,
    crumbs: Breadcrumb,
    rail: SidebarRail,

    collapsed: bool,

    opts: Vec<usize>,

    last_crumb: String,
    last_rail: String,
}

impl BreadcrumbsRailsPage {
    pub fn new() -> Self {
        let path = vec!["Disk".to_owned(), "Users".to_owned(), "alex".to_owned()];
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            list: ListBox::new(LIST, vec![], SelectMode::Single),
            crumbs: Breadcrumb::new(CRUMBS, path.clone()),
            path,
            rail: SidebarRail::new(RAIL, "Explorer"),
            collapsed: false,
            opts: page_options(PageId::BreadcrumbsRails)
                .iter()
                .map(|o| o.default)
                .collect(),
            last_crumb: "—".into(),
            last_rail: "—".into(),
        };
        page.rebuild();
        page.load_folder(0);
        page
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::BreadcrumbsRails);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn rebuild(&mut self) {
        self.crumbs = Breadcrumb::new(CRUMBS, self.path.clone())
            .clickable(self.opt("breadcrumbsrails.clickable") == "on");
        self.rail = SidebarRail::new(RAIL, "Explorer")
            .collapsed_rail(self.opt("breadcrumbsrails.collapsed_rail") == "on");
    }

    fn load_folder(&mut self, cursor: usize) {
        let items = entries(&self.path)
            .into_iter()
            .map(|(name, meta, opens)| {
                let item = ListItem::new(name).meta(meta);
                if meta == "locked" {
                    item.meta_tone(Tone::Warning).disabled(true)
                } else if opens {
                    item
                } else {
                    item.meta_tone(Tone::Secondary)
                }
            })
            .collect::<Vec<_>>();
        let n = items.len();
        self.list = ListBox::new(LIST, items, SelectMode::Single).empty_text("Empty folder");
        self.list.cursor = cursor.min(n.saturating_sub(1));
        self.crumbs.segments = self.path.clone();
    }

    fn go_to_level(&mut self, level: usize, cx: &mut Cx) {
        let left = self.path.get(level + 1).cloned();
        self.path.truncate(level + 1);
        let cursor = left
            .and_then(|l| entries(&self.path).iter().position(|e| e.0 == l))
            .unwrap_or(0);
        self.load_folder(cursor);
        cx.status(format!("Went back to {}", self.path.join(" › ")));
    }

    fn open_row(&mut self, i: usize, cx: &mut Cx) {
        let Some((name, _, opens)) = entries(&self.path).get(i).copied() else {
            return;
        };
        if opens {
            self.path.push(name.to_owned());
            self.load_folder(0);
        } else {
            cx.status(format!("Opened {name}"));
        }
    }

    fn set_collapsed(&mut self, on: bool, cx: &mut Cx) {
        self.collapsed = on;
        self.last_rail = if on { "Collapse" } else { "Expand" }.into();
        if on {
            if cx.focus.is(LIST) {
                cx.focus.focus(self.template.tabs.id);
            }
        } else {
            cx.focus.focus(LIST);
        }
    }

    fn location_facts(&self) -> Vec<Prop> {
        vec![
            Prop::new("Depth", self.path.len().to_string()),
            Prop::new("Last crumb", self.last_crumb.clone()),
            Prop::new("Last RailAction", self.last_rail.clone()),
            Prop::new("Keys", "Enter opens · Backspace goes up · Ctrl+B explorer").wrap(),
        ]
    }

    fn rail_facts(&self) -> Vec<Prop> {
        vec![
            Prop::new("RailAction::Collapse", "‹ in the frame, or Ctrl+B").wrap(),
            Prop::new("RailAction::Expand", "a click on the › rail, or Ctrl+B").wrap(),
            Prop::new(
                "collapsed_rail",
                if self.rail.collapsed_rail {
                    "on: a collapsed sidebar leaves its rail"
                } else {
                    "off: only the keyboard brings it back"
                },
            )
            .wrap(),
        ]
    }

    fn location_panel(&self) -> Panel<'static> {
        let meta = if self.crumbs.clickable {
            "clickable"
        } else {
            "clickable(false)"
        };
        Panel::card(Some("Location")).meta(meta)
    }

    fn trail_pair(&self) -> (Breadcrumb, Breadcrumb) {
        (
            Breadcrumb::new(ID.sub("plain"), self.path.clone()).clickable(false),
            Breadcrumb::new(ID.sub("deep"), self.deep_trail()),
        )
    }

    /// The cut-to-fit trail: the fixed six levels, or the stress data.
    fn deep_trail(&self) -> Vec<String> {
        let mut trail: Vec<String> = DEEP.iter().map(|s| (*s).to_owned()).collect();
        match stress::mode(self.opt("breadcrumbsrails.stress")) {
            Mode::Off => {}
            Mode::Items => trail = stress::numbered("Folder").collect(),
            Mode::Label => {
                if let Some(last) = trail.last_mut() {
                    *last = stress::long_label();
                }
            }
        }
        trail
    }

    fn geometry(&self, avail: Size, ctx: &RenderCtx) -> Geometry {
        let side_w = if self.collapsed {
            self.rail.width() + 1
        } else {
            SIDE_W + 2
        };
        let loc_facts = self.location_facts();
        let rail_facts = self.rail_facts();
        let loc_panel = self.location_panel();
        let rail_panel = Panel::card(Some("SidebarRail"));
        let wide = |w: u16| Size::new(w, 0);

        let crumbs = self.crumbs.measure(avail, ctx).width;
        let right_nat = loc_panel
            .outer(wide(crumbs.max(props::width(&loc_facts))))
            .width
            .max(rail_panel.outer(wide(props::width(&rail_facts))).width);
        let right_w = right_nat.max(avail.width.saturating_sub(side_w));
        let inner_w = loc_panel.inner(wide(right_w)).width;
        let loc_h = loc_panel
            .outer(Size::new(inner_w, 2 + props::measure(&loc_facts, inner_w)))
            .height;
        let rail_h = rail_panel
            .outer(Size::new(inner_w, props::measure(&rail_facts, inner_w)))
            .height;
        let top = SIDE_H.max(loc_h + 1 + rail_h);

        let (plain, deep) = self.trail_pair();
        let caption = text::width("clickable(false) · full width") as u16;
        // The deep trail is cut to fit, so a saturated stress trail never widens the page.
        let deep_w = stress::clamp_saturated(deep.measure(avail, ctx).width, avail.width);
        let trail_w = deep_w.max(plain.measure(avail, ctx).width).max(caption);
        let trails = Panel::card(Some("Trails")).outer(Size::new(trail_w, TRAILS_ROWS));
        let width = avail.width.max(side_w + right_nat).max(trails.width);
        Geometry {
            side_w,
            right_w: width - side_w,
            loc_h,
            rail_h,
            top,
            trails_h: trails.height,
            size: Size::new(width, top + 1 + trails.height),
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let g = self.geometry(area.as_size(), ctx);

        if self.collapsed {
            let col = Rect::new(area.x, area.y, SidebarRail::WIDTH, SIDE_H);
            self.rail.render_rail(col, buf, ctx, t.canvas);
        } else {
            let side = Rect::new(area.x, area.y, SIDE_W, SIDE_H);

            let meta = format!("{} items    ", self.list.items.len());
            let panel = Panel::framed(Some("Explorer"))
                .focused(ctx.interaction.focused(LIST))
                .meta(&meta);
            let bg = panel.bg(t);
            let inner = panel.render(side, buf, t);
            self.rail.render_collapse(side, buf, ctx, t.canvas);
            self.list.render(inner, buf, ctx, bg);
        }
        let right = Rect::new(area.x + g.side_w, area.y, g.right_w, g.top);

        let panel = self.location_panel();
        let bg = panel.bg(t);
        let card = Rect::new(right.x, right.y, right.width, g.loc_h);
        let inner = panel.render(card, buf, t);
        self.crumbs
            .render(Rect::new(inner.x, inner.y, inner.width, 1), buf, ctx, bg);
        let facts = self.location_facts();
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

        let panel = Panel::card(Some("SidebarRail"));
        let bg = panel.bg(t);
        let card = Rect::new(right.x, card.bottom() + 1, right.width, g.rail_h);
        let inner = panel.render(card, buf, t);
        props::render(inner, buf, t, &self.rail_facts(), bg);

        let panel = Panel::card(Some("Trails"));
        let bg = panel.bg(t);
        let card = Rect::new(area.x, area.y + g.top + 1, g.size.width, g.trails_h);
        let inner = panel.render(card, buf, t);

        let (w, width) = match self.opt("breadcrumbsrails.width") {
            "full" => (inner.width, "full width".to_owned()),
            n => {
                let w = n.parse::<u16>().unwrap_or(inner.width).min(inner.width);
                (w, format!("width {w}"))
            }
        };
        let (plain, deep) = self.trail_pair();
        let rows = [
            (
                "clickable(false)",
                plain,
                inner.width,
                "full width".to_owned(),
            ),
            ("deep, cut to fit", deep, w, width),
        ];
        let mut yy = inner.y;
        for (label, mut trail, w, width) in rows {
            let cap = format!("{label} · {width}");
            buf.set_string(
                inner.x,
                yy,
                text::truncate(&cap, inner.width as usize),
                t.faint().bg(bg),
            );
            trail.render(Rect::new(inner.x, yy + 1, w, 1), buf, ctx, bg);
            yy += 3;
        }
    }
}

const SIDE_W: u16 = 34;
const SIDE_H: u16 = 18;

const TRAILS_ROWS: u16 = 5;

struct Geometry {
    side_w: u16,

    right_w: u16,
    loc_h: u16,
    rail_h: u16,

    top: u16,
    trails_h: u16,
    size: Size,
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let trail = || {
        ["Disk", "Users", "alex", "Projects"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
    };
    StateGrid::new(id)
        .rows(&[Rest, Hover, Pressed, Overflow, Narrow])
        .column_width(34)
        .column("Clickable", Breadcrumb::new(id.child(0), trail()))
        .column(
            "Plain text",
            Breadcrumb::new(id.child(1), trail()).clickable(false),
        )
        .column("Rail", SidebarRail::new(id.child(2), "Files"))
}

impl Page for BreadcrumbsRailsPage {
    fn title(&self) -> &'static str {
        "Breadcrumbs & rails"
    }

    fn blurb(&self) -> &'static str {
        "Breadcrumb trails (static, clickable, collapsing) and the collapsible SidebarRail"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        self.geometry(avail, ctx).size
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
        if let PageEvent::Key(key) = ev
            && key.ctrl_char('b')
            && self.template.section() == Section::Playground
        {
            self.set_collapsed(!self.collapsed, cx);
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        match ev {
            PageEvent::Key(key) if cx.focus.is(LIST) => {
                if key.code == KeyCode::Backspace && key.plain() {
                    if self.path.len() > 1 {
                        let level = self.path.len() - 2;
                        self.go_to_level(level, cx);
                        self.last_crumb = format!("Backspace → level {level}");
                    }
                    return Outcome::Changed;
                }
                let o = self.list.on_key(key);
                if let Some(i) = self.list.chosen.take() {
                    self.open_row(i, cx);
                    return Outcome::Changed;
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.crumbs.route(ev) {
                    let (o, level) = r.into_parts();
                    if let Some(level) = level {
                        self.last_crumb = format!("{} (level {level})", self.path[level]);
                        self.go_to_level(level, cx);
                        return Outcome::Changed;
                    }
                    return o;
                }
                if let Some(r) = self.rail.route(ev) {
                    let (o, a) = r.into_parts();
                    if let Some(a) = a {
                        self.set_collapsed(a == RailAction::Collapse, cx);
                        return Outcome::Changed;
                    }
                    return o;
                }
                if ev.on(LIST) && ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                    cx.focus.focus(LIST);
                }
                if let Some(r) = self.list.route(ev) {
                    let o = r.outcome();
                    if let Some(i) = self.list.chosen.take() {
                        self.open_row(i, cx);
                        return Outcome::Changed;
                    }
                    return o;
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
        if focus == Some(LIST) {
            return vec![
                ("↑↓", "Move"),
                ("Enter", "Open"),
                ("Backspace", "Up a level"),
                ("Ctrl+B", "Hide explorer"),
            ];
        }
        if self.collapsed {
            return vec![("Ctrl+B", "Show explorer")];
        }
        Vec::new()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::BreadcrumbsRails)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::BreadcrumbsRails);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
            if key == "breadcrumbsrails.collapsed" {
                self.collapsed = self.opts[i] == 1;
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
    Box::new(BreadcrumbsRailsPage::new())
}
