use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Margin, Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::row_menu::{MENU_HINTS, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::{Backdrop, RenderCtx};
use junie_tui::ui::measure::{Measure, Tile, Tiled, tiles};
use junie_tui::ui::modal::Frame;
use junie_tui::ui::popup::{self, Dismiss, Overflow, Place, Placement, RefuseFeedback, VAlign};
use junie_tui::ui::reveal::{Reveal, RevealMode, RevealState};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::ui::tooltip::{Tip, Tooltips};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::menu::{ContextMenu, MenuAction, MenuItem};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::statusbar::{StatusBar, StatusItem};

const ID: WidgetId = WidgetId::of("popupstooltips");
const POPOVER: WidgetId = ID.sub("popover");

const PATHS: &[&str] = &[
    "src/widgets/dialog.rs · the modal dialog and its typed acknowledgement",
    "src/ui/popup.rs · placement, fitting and the dismiss policy of every popup",
    "src/ui/modal.rs · the frame every modal is drawn in",
    "src/ui/tooltip.rs · tooltips with their shortcut",
    "README.md",
];

fn popover_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Open").shortcut("Enter"),
        MenuItem::new("Copy link").shortcut("⌘C"),
        MenuItem::new("Share…").reason("Sharing is off"),
        MenuItem::new("Remove").danger(),
    ]
}

pub struct PopupsTooltipsPage {
    template: Template,
    opts: PageValues,
    pub open: Button,
    pub popover: RowMenu<()>,
    pub save: Button,
    pub new_tab: Button,
    pub status: StatusBar,
    pub paths: ListBox,

    pub last: String,
}

impl PopupsTooltipsPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::PopupsTooltips));
        let mut page = Self {
            template: Template::new(ID, frame_grid(&opts)),
            open: Button::secondary(ID.sub("open"), "Open popover ▾"),
            popover: RowMenu::default(),
            save: Button::primary(ID.sub("save"), "Save draft")
                .tooltip("Save the draft to this device")
                .shortcut("⌘S"),
            new_tab: {
                let mut b = Button::secondary(ID.sub("newtab"), "New tab");
                b.tip = Some(Tip::new("Open an empty query tab").shortcut("⌘T"));
                b
            },
            status: StatusBar {
                left: vec![
                    StatusItem::new("◆ production", Tone::Error)
                        .tooltip(Tip::new("Connected to the production database")),
                    StatusItem::new("eu-west-1", Tone::Secondary)
                        .tooltip(Tip::new("Region of the connection").shortcut("⌘R")),
                ],
                ..StatusBar::new()
            },
            paths: ListBox::new(
                ID.sub("paths"),
                PATHS.iter().map(|p| ListItem::new(p)).collect(),
                SelectMode::Single,
            ),
            opts,
            last: "none yet · Chosen(i) or Dismissed".into(),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        self.paths.reveal = RevealState::new(Some(self.reveal()));
        self.template.grid = frame_grid(&self.opts);
    }

    fn placement(&self, key: &str) -> Placement {
        match self.opts.value(key) {
            "above" => Placement::Above,
            "right" => Placement::Right,
            "left" => Placement::Left,
            "center" => Placement::Center,
            _ => Placement::Below,
        }
    }

    fn place(&self) -> Place {
        let o = &self.opts;
        Place::DEFAULT
            .margin(o.value("popupstooltips.margin").parse().unwrap_or(1))
            .vertical_margin(match o.value("popupstooltips.vertical_margin") {
                "0" => Some(0),
                "2" => Some(2),
                _ => None,
            })
            .v_align(match o.value("popupstooltips.v_align") {
                "third" => VAlign::Third,
                _ => VAlign::Center,
            })
            .overflow(match o.value("popupstooltips.overflow") {
                "slide" => Overflow::Slide,
                _ => Overflow::Shrink,
            })
            .text_inset(o.value("popupstooltips.text_inset").parse().unwrap_or(0))
    }

    fn dismiss(&self) -> Dismiss {
        let o = &self.opts;
        let mut d = Dismiss::LIGHT;
        d.escape = o.on("popupstooltips.escape");
        d.outside_click = o.on("popupstooltips.outside_click");
        d.pass_through(o.on("popupstooltips.pass_through"))
            .refuse_feedback(match o.value("popupstooltips.refuse_feedback") {
                "none" => RefuseFeedback::None,
                _ => RefuseFeedback::Flash,
            })
            .focus_out(o.on("popupstooltips.focus_out"))
    }

    fn tooltips(&self) -> Tooltips {
        let o = &self.opts;
        Tooltips::default()
            .enabled(o.on("popupstooltips.tooltips"))
            .delay_ms(o.value("popupstooltips.tip_delay").parse().unwrap_or(600))
            .max_width(o.value("popupstooltips.tip_width").parse().unwrap_or(48))
    }

    fn reveal(&self) -> Reveal {
        let o = &self.opts;
        Reveal::default()
            .hover(o.on("popupstooltips.reveal_hover"))
            .keyboard(match o.value("popupstooltips.reveal_key") {
                "none" => None,
                "r" => Some(KeyCode::Char('r')),
                _ => Some(KeyCode::Right),
            })
            .mode(match o.value("popupstooltips.reveal_mode") {
                "inline" => RevealMode::InlineWrap,
                _ => RevealMode::Tooltip,
            })
            .max_rows(o.value("popupstooltips.reveal_rows").parse().unwrap_or(0))
            .scroll(o.on("popupstooltips.reveal_scroll"))
            .wheel_lines(o.value("popupstooltips.reveal_wheel").parse().unwrap_or(3))
            .delay_ms(
                o.value("popupstooltips.reveal_delay")
                    .parse()
                    .unwrap_or(300),
            )
            .wheel_passes_at_ends(o.on("popupstooltips.reveal_passes"))
    }

    fn open_popover(&mut self) {
        let mut m = ContextMenu::new(POPOVER, popover_items())
            .anchor_control(self.open.id, Rect::ZERO)
            .fit(self.place())
            .dismiss(self.dismiss())
            .title("report.pdf");
        m.list.placement = self.placement("popupstooltips.placement");
        self.popover.open(m, ());
    }

    fn readout(&self, screen: Rect) -> Vec<Prop> {
        let place = self.place();
        let at = popup::place_with(
            screen,
            self.open.area,
            24,
            8,
            self.placement("popupstooltips.placement"),
            place,
        );
        let d = self.dismiss();
        let on = |b: bool| if b { "on" } else { "off" };
        let t = self.tooltips();
        let r = self.reveal();
        let state = &self.paths.reveal;
        vec![
            Prop::new(
                "Popover",
                format!(
                    "{:?} · place_with → {}×{} at {},{}",
                    self.placement("popupstooltips.placement"),
                    at.width,
                    at.height,
                    at.x,
                    at.y
                ),
            ),
            Prop::new(
                "Place",
                format!(
                    "margin {} · vertical margin {} · {:?} · {:?} · text inset {}",
                    place.margin,
                    place
                        .vertical_margin
                        .map_or("as margin".into(), |m| m.to_string()),
                    place.v_align,
                    place.overflow,
                    place.text_inset
                ),
            ),
            Prop::new(
                "Dismiss",
                format!(
                    "Esc {} · outside click {} · pass through {} · focus out {} · refused {:?}",
                    on(d.escape),
                    on(d.outside_click),
                    on(d.pass_through),
                    on(d.focus_out),
                    d.refuse_feedback
                ),
            ),
            Prop::new(
                "Tooltips",
                format!(
                    "{} · after {} ms · wrap at {} cells",
                    on(t.enabled),
                    t.delay_ms,
                    t.max_width
                ),
            ),
            Prop::new(
                "Reveal",
                format!(
                    "hover {} · key {} · {:?} · rows {} · scroll {} · wheel {} · after {} ms · wheel passes {}",
                    on(r.hover),
                    r.key_label().unwrap_or("none"),
                    r.mode,
                    if r.max_rows == 0 {
                        "as fit".into()
                    } else {
                        r.max_rows.to_string()
                    },
                    on(r.scroll),
                    r.wheel_lines,
                    r.delay_ms,
                    on(r.wheel_passes_at_ends)
                ),
            ),
            Prop::new(
                "RevealState",
                format!(
                    "per-widget policy {} · cut row now {}",
                    if state.config.is_some() {
                        "set"
                    } else {
                        "none"
                    },
                    state.hint().map_or("—".into(), |(k, l)| format!("{k} {l}"))
                ),
            ),
            Prop::new(
                "Tips",
                [&self.save, &self.new_tab]
                    .iter()
                    .filter_map(|b| b.tip.as_ref())
                    .chain(self.status.left.iter().filter_map(|i| i.tip.as_ref()))
                    .map(|t| match &t.shortcut {
                        Some(k) => format!("{} {k}", t.text),
                        None => t.text.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            Prop::new("Last", self.last.clone()),
        ]
    }

    fn tiled(&self, width: u16, screen: Rect, ctx: &RenderCtx) -> Tiled {
        let w = |s: &str| text::width(s) as u16;
        let pop = Panel::card(Some("Popover")).meta("click · Enter");
        let tips = Panel::card(Some("Tooltips")).meta("rest the pointer");
        let reveal = Panel::card(Some("Reveal")).meta(REVEAL_META);
        let policies = Panel::card(Some("Policies")).meta(POLICIES_META);
        let facts: Vec<Prop> = self.readout(screen).into_iter().map(Prop::wrap).collect();
        let status = self.status.measure(Size::new(0, 1), ctx).width;
        let tips_w = tips
            .outer(Size::new(
                (self.save.width() + 2 + self.new_tab.width())
                    .max(status)
                    .max(w(GRID_NOTE)),
                0,
            ))
            .width;
        let pop_w = pop
            .outer(Size::new(self.open.width().max(POP_NOTE_W), 0))
            .width;
        let pop_h = |w: u16| {
            let inner = pop.inner(Size::new(w, 0)).width;
            let rows = text::wrap(POP_NOTE, inner.max(1) as usize).len() as u16;
            pop.outer(Size::new(inner, 2 + rows)).height
        };
        let tips_h = |_: u16| tips.outer(Size::new(0, 5)).height;
        let reveal_h = |_: u16| REVEAL_H;
        let policies_h = |w: u16| sheet_height(&policies, &facts, w);
        tiles(
            width,
            &[
                &[Tile::new(pop_w, &pop_h), Tile::new(tips_w, &tips_h)],
                &[Tile::new(reveal.outer(Size::new(44, 0)).width, &reveal_h)],
                &[Tile::new(sheet_width(&policies, &facts), &policies_h)],
            ],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let screen = *buf.area();
        let tiled = self.tiled(area.width, screen, ctx);
        let (left, right) = (tiled.at(0, area), tiled.at(1, area));

        let panel = Panel::card(Some("Popover")).meta("click · Enter");
        let bg = panel.bg(t);
        let inner = panel.render(left, buf, t);
        let bw = self.open.width().min(inner.width);
        self.open
            .render(Rect::new(inner.x, inner.y, bw, 1), buf, ctx, bg);
        for (i, line) in junie_tui::ui::text::wrap(POP_NOTE, inner.width.max(1) as usize)
            .iter()
            .enumerate()
        {
            buf.set_string(inner.x, inner.y + 2 + i as u16, line, t.muted().bg(bg));
        }

        let app_tooltips = ctx.tooltips;
        ctx.tooltips = self.tooltips();
        let panel = Panel::card(Some("Tooltips")).meta("rest the pointer");
        let bg = panel.bg(t);
        let inner = panel.render(right, buf, t);
        let sw = self.save.width().min(inner.width);
        self.save
            .render(Rect::new(inner.x, inner.y, sw, 1), buf, ctx, bg);
        let nx = inner.x + sw + 2;
        let nw = self.new_tab.width().min(inner.right().saturating_sub(nx));
        self.new_tab
            .render(Rect::new(nx, inner.y, nw, 1), buf, ctx, bg);
        if inner.height > 2 {
            self.status
                .render(Rect::new(inner.x, inner.y + 2, inner.width, 1), buf, ctx);
        }
        if inner.height > 4 {
            buf.set_string(
                inner.x,
                inner.y + 4,
                junie_tui::ui::text::truncate(GRID_NOTE, inner.width as usize),
                t.muted().bg(bg),
            );
        }
        ctx.tooltips = app_tooltips;

        let panel = Panel::card(Some("Reveal")).meta(REVEAL_META);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(2, area), buf, t);
        let w = inner.width.min(44);
        self.paths
            .render(Rect::new(inner.x, inner.y, w, inner.height), buf, ctx, bg);

        let panel = Panel::card(Some("Policies")).meta(POLICIES_META);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(3, area), buf, t);
        let facts: Vec<Prop> = self.readout(screen).into_iter().map(Prop::wrap).collect();
        props::render(inner, buf, t, &facts, bg);

        self.popover.render(screen, buf, ctx);
    }
}

const POP_NOTE: &str = "A menu anchored to its button: placement, fit and dismissal are options.";

const POP_NOTE_W: u16 = 30;
const GRID_NOTE: &str = "Grid headers show their column type the same way (Data grid).";
const REVEAL_META: &str = "cut rows · hover or the reveal key";
const POLICIES_META: &str = "read back from the page's builders";

const REVEAL_H: u16 = 8;

fn frame_grid(o: &PageValues) -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let mut f = Frame::new(36, 6)
        .title("Frame")
        .meta(Some("meta"))
        .dismiss(match o.value("popupstooltips.frame_dismiss") {
            "never" => Dismiss::NEVER,
            _ => Dismiss::ESCAPE.close_button(o.on("popupstooltips.frame_close")),
        })
        .fit(match o.value("popupstooltips.frame_fit") {
            "flush" => Place::FLUSH,
            "menu" => Place::MENU,
            _ => Place::DEFAULT,
        })
        .margin(match o.value("popupstooltips.frame_margin") {
            "1x0" => Margin::new(1, 0),
            _ => Margin::new(3, 1),
        })
        .chrome(
            o.value("popupstooltips.frame_chrome").parse().unwrap_or(1),
            1,
        )
        .backdrop(o.on("popupstooltips.frame_backdrop"))
        .dim(match o.value("popupstooltips.frame_dim") {
            "full" => Backdrop::FULL,
            "keep-top" => Backdrop::DEFAULT.keep_top(1),
            "keep-bottom" => Backdrop::FULL.keep_bottom(2),
            _ => Backdrop::DEFAULT,
        });
    if o.value("popupstooltips.frame_min") == "40x8" {
        f = f.min_size(40, 8);
    }
    if o.value("popupstooltips.frame_close_button") == "danger" {
        f = f.close_button(
            IconButton::close(junie_tui::ui::modal::CLOSE)
                .with_glyph("✕")
                .danger_hover(true),
        );
    }
    f = match o.value("popupstooltips.frame_placement") {
        "below" => f.anchor(Rect::ZERO, Placement::Below),
        "right" => f.anchor(Rect::ZERO, Placement::Right),
        _ => f.placement(Placement::Center),
    };
    StateGrid::new(id)
        .rows(&[Rest, Hover, Pressed])
        .column_width(64)
        .column("Frame", f)
}

impl Page for PopupsTooltipsPage {
    fn title(&self) -> &'static str {
        "Popups & tooltips"
    }

    fn blurb(&self) -> &'static str {
        "Where popups go and how they close, the modal frame, tooltips and revealing cut text"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let screen = Rect::new(0, 0, avail.width, avail.height);
        self.tiled(avail.width, screen, ctx).size
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
        if let Some((out, chosen)) = self.popover.handle(ev) {
            match chosen {
                Some(((), i)) => {
                    let items = popover_items();
                    let label = items.get(i).map_or("?", |it| it.item.label.as_str());
                    self.last = format!("{:?} · {label}", MenuAction::Chosen(i));
                }
                None if !self.popover.is_open() => {
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
                if f == Some(self.open.id) {
                    let (out, act) = self.open.on_key(key);
                    if act {
                        self.open_popover();
                        return Outcome::Changed;
                    }
                    return out;
                }
                if f == Some(self.paths.id) {
                    return self.paths.on_key(key);
                }
                for b in [&mut self.save, &mut self.new_tab] {
                    if f == Some(b.id) {
                        return b.on_key(key).0;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(p) => {
                let click = p.is(Phase::Click);
                if let Some(r) = self.open.route(p) {
                    if click {
                        cx.focus.focus(self.open.id);
                    }
                    if r.has_actions() {
                        self.open_popover();
                    }
                    return r.outcome().or(Outcome::Changed);
                }
                if p.on(self.paths.id) {
                    if click && p.index_of(Part::ROW).is_some() {
                        cx.focus.focus(self.paths.id);
                    }
                    return self.paths.pointer(p).outcome();
                }
                for b in [&mut self.save, &mut self.new_tab] {
                    if let Some(r) = b.route(p) {
                        if click {
                            cx.focus.focus(b.id);
                        }
                        return r.outcome().or(Outcome::Changed);
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.popover.is_open() {
            return MENU_HINTS.to_vec();
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.paths.id) {
            let mut h = vec![("↑↓", "Move")];
            if let Some(r) = self.paths.reveal.hint() {
                h.push(r);
            }
            return h;
        }
        if focus == Some(self.open.id) {
            return vec![("Enter / Space", "Open popover")];
        }
        Vec::new()
    }

    fn layer_open(&self) -> bool {
        self.popover.hides_page_focus() || self.template.popup_open()
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

pub fn page() -> Box<dyn Page> {
    Box::new(PopupsTooltipsPage::new())
}
