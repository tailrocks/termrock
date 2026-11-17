use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::row_menu::{MENU_HINTS, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::{Key, Outcome};
use termrock::core::id::{Part, WidgetId};
use termrock::core::intent::Phase;
use termrock::core::keymap::NavKeys;
use termrock::core::pointer::MouseTarget;
use termrock::ui::context::{ContextAnchor, ContextConfig, ContextKeys, ContextRequest};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::measure::{Tile, Tiled, tiles};
use termrock::ui::popup::{Dismiss, Place, Placement};
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text;
use termrock::widgets::button::Button;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::menu::{ContextMenu, MenuAction, MenuItem, UnknownKey};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("menus");
const ROW_MENU: WidgetId = ID.sub("row-menu");
const ACTIONS_MENU: WidgetId = ID.sub("actions-menu");

const FILES: &[(&str, &str, bool)] = &[
    ("report.pdf", "document", false),
    ("notes.md", "text", false),
    ("build.log", "log", false),
    ("archive.zip", "archive", false),
    ("photo.png", "image", false),
    ("README.md", "read-only", true),
    ("LICENSE", "read-only", true),
];

fn action_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("New file").shortcut("⌘N"),
        MenuItem::new("New folder").shortcut("⇧⌘N").separator(),
        MenuItem::new("Import…"),
        MenuItem::new("Export…").reason("Nothing selected"),
        MenuItem::new("Refresh").shortcut("F5").separator(),
        MenuItem::new("Sort by name"),
        MenuItem::new("Sort by kind"),
        MenuItem::new("Sort by size").separator(),
        MenuItem::new("Show hidden files"),
        MenuItem::new("Empty Trash").danger(),
    ]
}

fn row_items(read_only: bool) -> Vec<MenuItem> {
    let rename = MenuItem::new("Rename").shortcut("F2");
    let trash = MenuItem::new("Move to Trash")
        .shortcut("⌘⌫")
        .danger_if(!read_only);
    vec![
        MenuItem::new("Open").shortcut("Enter"),
        if read_only {
            rename.reason("Read-only file")
        } else {
            rename
        },
        MenuItem::new("Duplicate").separator(),
        MenuItem::new("Copy path"),
        MenuItem::new("Share…")
            .reason("Sharing is off")
            .separator_if(!read_only),
        if read_only {
            trash.disabled(true)
        } else {
            trash
        },
    ]
}

struct Opts {
    values: Vec<(&'static str, usize)>,
}

impl Opts {
    fn new() -> Self {
        Self {
            values: crate::options::page_options(PageId::Menus)
                .iter()
                .map(|o| (o.key, o.default))
                .collect(),
        }
    }
    fn get(&self, key: &str) -> usize {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |(_, v)| *v)
    }
    fn on(&self, key: &str) -> bool {
        self.get(key) == 1
    }

    fn style(&self, m: ContextMenu) -> ContextMenu {
        let fit = match self.get("menus.fit") {
            1 => Place::FLUSH,
            2 => Place::DEFAULT,
            _ => Place::MENU,
        };
        let mut m = m
            .fit(fit)
            .edge_margin(self.get("menus.edge_margin") as u16)
            .min_width([8, 16, 24][self.get("menus.min_width").min(2)])
            .release_selects(self.on("menus.release_selects"))
            .hide_owner_focus(self.on("menus.hide_owner_focus"))
            .wrap(self.on("menus.wrap"))
            .type_ahead(self.on("menus.type_ahead"))
            .nav(match self.get("menus.nav") {
                1 => NavKeys::VIM,
                _ => NavKeys::DESKTOP,
            })
            .dismiss(match self.get("menus.dismiss") {
                1 => Dismiss::ESCAPE,
                _ => Dismiss::LIGHT,
            })
            .unknown_key(match self.get("menus.unknown_key") {
                1 => UnknownKey::Swallow,
                _ => UnknownKey::CloseAndPass,
            });
        match self.get("menus.max_rows") {
            1 => m = m.max_rows(8),
            2 => m = m.max_rows(4),
            _ => {}
        }
        m
    }

    fn context(&self) -> ContextConfig {
        let extra = (self.get("menus.extra_key") == 1).then_some(Key {
            code: KeyCode::Char('m'),
            mods: KeyModifiers::NONE,
        });
        ContextConfig::DEFAULT
            .keys(
                ContextKeys::DEFAULT
                    .shift_f10(self.on("menus.shift_f10"))
                    .menu_key(self.on("menus.menu_key"))
                    .extra(extra),
            )
            .moves_cursor(self.on("menus.moves_cursor"))
    }

    fn placement(&self) -> Placement {
        match self.get("menus.placement") {
            1 => Placement::Above,
            2 => Placement::Right,
            3 => Placement::Left,
            4 => Placement::Center,
            _ => Placement::Below,
        }
    }
}

pub struct MenusPage {
    template: Template,
    opts: Opts,
    pub files: ListBox,
    pub actions: Button,

    pub row_menu: RowMenu<usize>,

    pub actions_menu: RowMenu<()>,

    pub request: String,
    pub last: String,
}

impl MenusPage {
    pub fn new() -> Self {
        let items = FILES
            .iter()
            .map(|(n, k, _)| ListItem::new(n).meta(k))
            .collect();
        let opts = Opts::new();
        let mut row_menu = RowMenu::default();
        row_menu.config = opts.context();
        Self {
            template: Template::new(ID, states_grid()),
            files: ListBox::new(ID.sub("files"), items, SelectMode::Single),
            actions: Button::secondary(ID.sub("actions"), "Actions ▾"),
            row_menu,
            actions_menu: RowMenu::default(),
            opts,
            request: "none yet · Pointer at a right-click, Control under the row for a key".into(),
            last: "none yet · Chosen(i) or Dismissed".into(),
        }
    }

    fn open_row_menu(&mut self, req: ContextRequest<usize>) {
        let req = req.map(|row| row.min(FILES.len() - 1));
        let (name, _, read_only) = FILES[req.target];
        self.request = match req.anchor {
            ContextAnchor::Pointer(p) => format!("{name} · at the pointer ({}, {})", p.x, p.y),
            ContextAnchor::Control { .. } => format!("{name} · under its row (keyboard)"),
        };
        let mut m = self.opts.style(req.menu(ROW_MENU, row_items(read_only)));
        if self.opts.on("menus.title") {
            m = m.title(name);
        }
        self.row_menu.open(m, req.target);
    }

    fn open_actions(&mut self) {
        let m = ContextMenu::new(ACTIONS_MENU, action_items())
            .anchor_control(self.actions.id, Rect::ZERO);
        let mut m = self.opts.style(m);
        m.list.placement = self.opts.placement();
        if self.opts.on("menus.title") {
            m = m.title("Actions");
        }
        self.request = "Actions · under its button".into();
        self.actions_menu.open(m, ());
    }

    fn chose(&mut self, what: &str, i: usize, items: &[MenuItem]) {
        let label = items.get(i).map_or("?", |it| it.item.label.as_str());
        self.last = format!("{:?} · {label} on {what}", MenuAction::Chosen(i));
    }

    fn last_facts(&self) -> Vec<Prop> {
        vec![
            Prop::new("Request", self.request.clone()).wrap(),
            Prop::new("Action", self.last.clone()).wrap(),
            Prop::new("Keys", keys_readout(&self.row_menu.config)).wrap(),
        ]
    }

    fn tiled(&self, width: u16) -> Tiled {
        let w = |s: &str| text::width(s) as u16;
        let files_p = Panel::card(Some("Files")).meta(FILES_META);
        let actions_p = Panel::card(Some("Actions menu")).meta(ACTIONS_META);
        let last_p = Panel::card(Some("Last"));
        let facts = self.last_facts();
        let rows = self.files.items.len() as u16;
        let files_w = files_p.outer(Size::new(FILES_W, 0)).width;
        let notes = NOTES.iter().map(|n| w(n)).max().unwrap_or(0);
        let actions_w = actions_p
            .outer(Size::new(notes.max(self.actions.width()), 0))
            .width;
        let files_h = |_: u16| files_p.outer(Size::new(0, rows)).height;
        let actions_h = |_: u16| actions_p.outer(Size::new(0, 2 + NOTES.len() as u16)).height;
        let last_h = |w: u16| sheet_height(&last_p, &facts, w);
        tiles(
            width,
            &[
                &[
                    Tile::new(files_w, &files_h),
                    Tile::new(actions_w, &actions_h),
                ],
                &[Tile::new(sheet_width(&last_p, &facts), &last_h)],
            ],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let tiled = self.tiled(area.width);

        let panel = Panel::card(Some("Files"))
            .meta(FILES_META)
            .focused(ctx.interaction.focused(self.files.id));
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(0, area), buf, t);
        self.files.render(inner, buf, ctx, bg);

        let panel = Panel::card(Some("Actions menu")).meta(ACTIONS_META);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(1, area), buf, t);
        let bw = self.actions.width().min(inner.width);
        self.actions
            .render(Rect::new(inner.x, inner.y, bw, 1), buf, ctx, bg);
        for (i, n) in NOTES.iter().enumerate() {
            buf.set_string(inner.x, inner.y + 2 + i as u16, n, t.muted().bg(bg));
        }

        let card = Panel::card(Some("Last"));
        let bg = card.bg(t);
        let inner = card.render(tiled.at(2, area), buf, t);
        props::render(inner, buf, t, &self.last_facts(), bg);

        let screen = *buf.area();
        self.row_menu.render(screen, buf, ctx);
        self.actions_menu.render(screen, buf, ctx);
    }
}

const FILES_META: &str = "right-click · Shift+F10";
const ACTIONS_META: &str = "click · Enter";

const FILES_W: u16 = 30;
const NOTES: [&str; 4] = [
    "Right-press a file, drag onto an item,",
    "release: the release chooses it.",
    "A right-click outside an open menu",
    "closes it and opens the one beneath.",
];

fn keys_readout(c: &ContextConfig) -> String {
    let mut parts = vec!["right-click".to_owned()];
    if c.keys.shift_f10 {
        parts.push("Shift+F10".into());
    }
    if c.keys.menu_key {
        parts.push("Menu key".into());
    }
    if let Some(k) = c.keys.extra
        && let KeyCode::Char(ch) = k.code
    {
        parts.push(format!("{ch} (extra key)"));
    }
    let mut s = parts.join(" · ");
    s.push_str(if c.moves_cursor {
        " — a click moves the cursor"
    } else {
        " — a click leaves the cursor"
    });
    s
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Pressed, Disabled, Overflow, Narrow])
        .column_width(30)
        .narrow_width(14)
        .column(
            "Commands",
            ContextMenu::new(
                id.child(0),
                vec![
                    MenuItem::new("Open").shortcut("Enter"),
                    MenuItem::new("Rename").shortcut("F2"),
                    MenuItem::new("Delete").danger(),
                ],
            ),
        )
        .column(
            "Reasons",
            ContextMenu::new(
                id.child(1),
                vec![
                    MenuItem::new("Copy path"),
                    MenuItem::new("Share…").reason("Off").separator_if(true),
                    MenuItem::new("Trash").danger_if(true),
                ],
            )
            .title("README.md"),
        )
}

impl Page for MenusPage {
    fn title(&self) -> &'static str {
        "Menus"
    }

    fn blurb(&self) -> &'static str {
        "Context menus: anchors, placement, reasons, danger, separators, keys and press-drag-release"
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
        if let Some((out, chosen)) = self.row_menu.handle(ev) {
            match chosen {
                Some((file, i)) => {
                    let items = row_items(FILES[file].2);
                    self.chose(FILES[file].0, i, &items);
                }
                None if !self.row_menu.is_open() => {
                    self.last = format!("{:?}", MenuAction::Dismissed);
                }
                None => {}
            }
            return out;
        }
        if let Some((out, chosen)) = self.actions_menu.handle(ev) {
            match chosen {
                Some(((), i)) => self.chose("the folder", i, &action_items()),
                None if !self.actions_menu.is_open() => {
                    self.last = format!("{:?}", MenuAction::Dismissed);
                }
                None => {}
            }
            return out;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                let f = cx.focus.current();
                if f == Some(self.files.id) {
                    if let Some(req) = self.row_menu.config.on_key(&self.files, key) {
                        self.open_row_menu(req);
                        return Outcome::Changed;
                    }
                    return self.files.on_key(key);
                }
                if f == Some(self.actions.id) {
                    let (out, activated) = self.actions.on_key(key);
                    if activated {
                        self.open_actions();
                        return Outcome::Changed;
                    }
                    return out;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) if ev.is(Phase::Secondary) => {
                let cfg = self.row_menu.config;
                match cfg.on_secondary(&mut self.files, ev) {
                    Some(req) => {
                        cx.focus.focus(self.files.id);
                        self.open_row_menu(req);
                        Outcome::Changed
                    }
                    None => Outcome::Ignored,
                }
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.actions.route(ev) {
                    if !ev.is(Phase::Click) {
                        return r.outcome();
                    }
                    cx.focus.focus(self.actions.id);
                    if r.has_actions() {
                        self.open_actions();
                    }
                    return Outcome::Changed;
                }
                if ev.on(self.files.id) && ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                    cx.focus.focus(self.files.id);
                }
                if let Some(r) = self.files.route(ev) {
                    return r.outcome();
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.row_menu.is_open() || self.actions_menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.files.id) {
            let mut h = vec![("↑↓", "Move")];
            if self.row_menu.config.keys.shift_f10 {
                h.push(("Shift+F10", "Menu"));
            }
            if self.row_menu.config.keys.extra.is_some() {
                h.push(("m", "Menu"));
            }
            return h;
        }
        if focus == Some(self.actions.id) {
            return vec![("Enter / Space", "Open menu")];
        }
        Vec::new()
    }

    fn layer_open(&self) -> bool {
        self.row_menu.hides_page_focus()
            || self.actions_menu.hides_page_focus()
            || self.template.popup_open()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts
            .values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if let Some(slot) = self.opts.values.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
            self.row_menu.config = self.opts.context();
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
    Box::new(MenusPage::new())
}
