use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::row_menu::RowMenu;
use crate::pages::stress::{self, Mode};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::{BadgeKind, StatusKind, Tone};
use junie_tui::ui::context::ContextRequest;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::{Measure, Tile, Tiled, tiles};
use junie_tui::ui::popup::Dismiss;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::tooltip::Tip;
use junie_tui::widgets::brand::Lockup;
use junie_tui::widgets::hintbar::{HintBar, HintLayer};
use junie_tui::widgets::keyhint::hint;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::menu::{ContextMenu, MenuBar, MenuBarAction, MenuItem, OpenOn, Overflow};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::MeterTone;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::statusbar::{Group, StatusBar, StatusItem};
use junie_tui::widgets::viewport::{Span, render_spans};
use ratatui::crossterm::event::KeyCode;

const ID: WidgetId = WidgetId::of("chrome");
const BAR: WidgetId = WidgetId::of("chrome.menubar");
const SESSIONS: WidgetId = WidgetId::of("chrome.sessions");
const CONTEXT: WidgetId = WidgetId::of("chrome.context");
const USAGE: WidgetId = WidgetId::of("chrome.status.usage");
const PR: WidgetId = WidgetId::of("chrome.status.pr");
const ENV: WidgetId = WidgetId::of("chrome.strip.env");
const PENDING: WidgetId = WidgetId::of("chrome.strip.pending");

const NARROW_BAR: u16 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Session(usize),

    Env,
}

pub struct ChromePage {
    template: Template,
    bar: MenuBar,
    sessions: ListBox,
    context: RowMenu<Target>,
    last: String,

    action: Option<MenuBarAction>,
    zoomed: bool,
    hint_area: Rect,

    opts: Vec<usize>,
}

impl ChromePage {
    pub fn new() -> Self {
        let opts: Vec<usize> = page_options(PageId::Chrome)
            .iter()
            .map(|o| o.default)
            .collect();
        let items = [
            ("1 Claude Code (Work)", "working"),
            ("2 Codex (Primary)", "idle"),
            ("3 Shell", ""),
            ("4 docs", "blocked"),
        ]
        .into_iter()
        .map(|(l, m)| ListItem::new(l).meta(m))
        .collect();
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            bar: MenuBar::new(BAR, Vec::new()),
            sessions: ListBox::new(SESSIONS, items, SelectMode::Single),
            context: RowMenu::default(),
            last: "nothing yet".into(),
            action: None,
            zoomed: false,
            hint_area: Rect::ZERO,
            opts,
        };
        page.bar = page.build_bar();
        page
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::Chrome);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn on(&self, key: &str) -> bool {
        self.opt(key) == "on"
    }

    fn build_bar(&self) -> MenuBar {
        let bar = MenuBar::new(
            BAR,
            vec![
                (
                    "File",
                    vec![
                        MenuItem::new("New tab").shortcut("c"),
                        MenuItem::new("Split right").shortcut("%"),
                        MenuItem::new("Export…").separator(),
                        MenuItem::new("Close tab").shortcut("&").danger(),
                    ],
                ),
                (
                    "Edit",
                    vec![
                        MenuItem::new("Copy").shortcut("y"),
                        MenuItem::new("Paste").shortcut("p"),
                    ],
                ),
                (
                    "View",
                    vec![
                        MenuItem::new("Zoom pane").shortcut("z"),
                        MenuItem::new("Redraw").shortcut("r").separator(),
                        MenuItem::new("Usage").shortcut("u"),
                        MenuItem::new("Inspect changes").disabled(true),
                    ],
                ),
                (
                    "Session",
                    vec![
                        MenuItem::new("Rename…").shortcut("n"),
                        MenuItem::new("Detach").shortcut("d"),
                    ],
                ),
                ("Help", vec![MenuItem::new("Key reference").shortcut("?")]),
            ],
        )
        .open_on(match self.opt("chrome.open_on") {
            "release" => OpenOn::Release,
            _ => OpenOn::Press,
        })
        .overflow(match self.opt("chrome.overflow") {
            "chevron" => Overflow::Chevron,
            _ => Overflow::Hide,
        })
        .hover_switch(self.on("chrome.hover_switch"))
        .press_drag(self.on("chrome.press_drag"))
        .hide_owner_focus(self.on("chrome.hide_owner_focus"))
        .wrap(self.on("chrome.wrap"))
        .type_ahead(self.on("chrome.type_ahead"))
        .nav(match self.opt("chrome.nav") {
            "vim" => NavKeys::VIM,
            _ => NavKeys::default(),
        })
        .dismiss(match self.opt("chrome.dismiss") {
            "escape" => Dismiss::ESCAPE,
            _ => Dismiss::LIGHT,
        });
        match self.opt("chrome.brand") {
            "compact" => bar.brand(Lockup::compact("app❯")),
            "none" => bar,
            _ => bar.brand(Lockup::new("app❯")),
        }
    }

    fn readout(&self) -> [String; 2] {
        let b = &self.bar;
        let on = |v: bool| if v { "on" } else { "off" };
        let nav = if b.nav == NavKeys::VIM {
            "vim"
        } else {
            "default"
        };
        let dismiss = if b.dismiss.outside_click {
            "light"
        } else {
            "escape"
        };
        let brand = match &b.brand {
            None => "none",
            Some(l) if l.compact => "compact",
            Some(_) => "lockup",
        };
        [
            format!(
                "MenuBar · open_on {:?} · overflow {:?} · hover_switch {} · press_drag {} · hide_owner_focus {}",
                b.open_on,
                b.overflow,
                on(b.hover_switch),
                on(b.press_drag),
                on(b.hide_owner_focus),
            ),
            format!(
                "wrap {} · type_ahead {} · nav {nav} · dismiss {dismiss} · brand {brand}",
                on(b.wrap),
                on(b.type_ahead),
            ),
        ]
    }

    fn strip(&self, tick: u64, t: &junie_tui::theme::Theme) -> (Vec<Segment>, Vec<Segment>) {
        let g = &t.glyphs;
        let left = vec![
            Segment::new("payments-platform", Tone::Normal)
                .bold()
                .priority(9),
            Segment::new("production", Tone::Danger)
                .lead(g.env_production, Tone::Danger)
                .clickable(ENV)
                .priority(8),
            Segment::new("staging", Tone::Warning)
                .lead(g.env_staging, Tone::Warning)
                .priority(3),
            Segment::new("deploying", Tone::Secondary)
                .lead(g.spinner_frame(tick), Tone::Active)
                .priority(6),
        ];
        let right = vec![
            Segment::new("3 pending", Tone::Secondary)
                .lead(g.bullet, Tone::Warning)
                .clickable(PENDING)
                .priority(5),
            Segment::new("main ↑2", Tone::Muted).priority(2),
        ];
        (left, right)
    }

    fn status_bar(&self) -> StatusBar {
        let mut b = StatusBar::new();
        b.left.push(
            StatusItem::new("payments-platform", Tone::Normal)
                .strong()
                .priority(9),
        );
        b.left.push(
            StatusItem::new("PR #482 · settlement backoff", Tone::Secondary)
                .priority(7)
                .clickable(PR)
                .tooltip(Tip::new("Open the pull request").shortcut("p")),
        );
        b.center.push(
            StatusItem::new("Claude Code · working · 2 tabs", Tone::Secondary)
                .busy()
                .priority(4),
        );
        b.right.push(
            StatusItem::new("Weekly 59%", Tone::Warning)
                .chip()
                .priority(6)
                .clickable(USAGE),
        );
        b.right.push(
            StatusItem::new("disk", Tone::Muted)
                .meter(Some(72), MeterTone::Level { warn: 80, crit: 95 })
                .priority(5),
        );
        b.right.push(
            StatusItem::new("quota", Tone::Muted)
                .meter(Some(88), MeterTone::Normal)
                .meter_stale(true)
                .priority(5),
        );
        b.right.push(
            StatusItem::new("jackin-payments-7f3a", Tone::Muted)
                .chip()
                .priority(3),
        );
        b.right
            .push(StatusItem::new("run 9c41", Tone::Faint).priority(2));
        match stress::mode(self.opt("chrome.stress")) {
            Mode::Off => {}
            Mode::Items => {
                for (n, label) in stress::numbered("job").enumerate() {
                    let priority = (n % 8) as u8 + 1;
                    b.right
                        .push(StatusItem::new(label, Tone::Muted).priority(priority));
                }
            }
            Mode::Label => b.left.insert(
                0,
                StatusItem::new(stress::long_label(), Tone::Muted).priority(9),
            ),
        }
        b
    }

    fn open_context(&mut self, req: ContextRequest<usize>) {
        let row = req.target;
        let label = self.sessions.items[row].label.clone();
        self.context.open(
            req.menu(CONTEXT, self.context_items(Target::Session(row)))
                .title(label),
            Target::Session(row),
        );
    }

    fn context_items(&self, target: Target) -> Vec<MenuItem> {
        match target {
            Target::Session(row) => vec![
                MenuItem::new("Change title…").shortcut("r"),
                MenuItem::new("Move left").disabled(row == 0),
                MenuItem::new("Move right")
                    .disabled(row + 1 == self.sessions.items.len())
                    .separator(),
                MenuItem::new("Close").shortcut("x").danger(),
            ],
            Target::Env => vec![
                MenuItem::new("Switch to staging"),
                MenuItem::new("Copy cluster URL").shortcut("y"),
            ],
        }
    }

    fn layer(&self, focus: Option<WidgetId>) -> HintLayer {
        let menu = self.bar.is_open().then(|| {
            HintLayer::new(vec![
                hint("↑↓", "Move"),
                hint("←→", "Switch menu"),
                hint("Enter", "Choose"),
                hint("Esc", "Close"),
            ])
        });
        let context = self.context.is_open().then(|| {
            HintLayer::new(vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Close"),
            ])
        });
        let screen = Some(if focus == Some(BAR) {
            HintLayer::new(vec![
                hint("←→", "Menu"),
                hint("Enter", "Open"),
                hint("Tab", "Next"),
            ])
        } else {
            HintLayer::new(vec![
                hint("↑↓", "Move"),
                hint("Shift+F10", "Context menu"),
                hint("right-click", "Context menu"),
                hint("Tab", "Next"),
            ])
            .status_parts(
                vec![
                    ("last: ".to_owned(), Tone::Muted),
                    (self.last.clone(), Tone::Secondary),
                ],
                Tone::Secondary,
            )
        });
        let mut l = HintBar::resolve(&[menu, context, screen]);
        if self.zoomed {
            l = l.badge("ZOOM", BadgeKind::Edit);
        }
        l
    }

    fn act(&mut self, menu: usize, item: usize, cx: &mut Cx) {
        let label = self.bar.menus[menu][item].item.label.clone();
        if menu == 2 && item == 0 {
            self.zoomed = !self.zoomed;
        }
        self.last = format!("{} › {label}", self.bar.labels[menu]);
        cx.status(label);
    }

    fn bar_action(&mut self, ev: Option<MenuBarAction>, cx: &mut Cx) {
        let Some(ev) = ev else { return };
        self.action = Some(ev);
        match ev {
            MenuBarAction::Chosen(m, i) => self.act(m, i, cx),
            MenuBarAction::Brand => {
                self.last = "brand".into();
                cx.status("Brand lockup activated");
            }
            MenuBarAction::Opened(_) => cx.focus.focus(BAR),
            MenuBarAction::Closed => {}
        }
    }

    fn sessions_panel(&self, focused: bool) -> Panel<'static> {
        Panel::card(Some("Sessions"))
            .meta("right-click or Shift+F10 for the tab menu")
            .focused(focused)
    }

    fn checks_line() -> [Span; 6] {
        [
            Span::muted("checks "),
            Span::new("2 ok", Tone::Success),
            Span::muted(" · "),
            Span::new("1 failed", Tone::Error).bold(),
            Span::muted(" · "),
            Span::new("1 cancelled", Tone::Muted),
        ]
    }

    fn action_line(&self) -> String {
        let last = match self.action {
            Some(a) => format!("{a:?}"),
            None => "—".into(),
        };
        format!("last MenuBarAction: {last}   (Opened(menu) · Chosen(menu, item) · Closed · Brand)")
    }

    fn readout_lines(&self, w: u16) -> Vec<String> {
        self.readout()
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(self.action_line().as_str()))
            .flat_map(|l| junie_tui::ui::text::wrap(l, w.max(1) as usize))
            .collect()
    }

    fn placed_text(&self, w: u16) -> String {
        let bar = self.status_bar();
        let placed = bar.layout(Rect::new(0, 0, w, 1));
        let groups: Vec<String> = [Group::Left, Group::Center, Group::Right]
            .into_iter()
            .map(|g| {
                let n = placed.iter().filter(|p| p.group == g).count();
                format!("{g:?} {n}")
            })
            .collect();
        let total = bar.left.len() + bar.center.len() + bar.right.len();
        format!("places {} of {total} · {}", placed.len(), groups.join(", "))
    }

    fn natural_width(&self, ctx: &RenderCtx) -> u16 {
        let t = ctx.theme;
        let any = ratatui::layout::Size::new(0, 1);
        let w = |s: &str| junie_tui::ui::text::width(s) as u16;
        let spans: u16 = Self::checks_line().iter().map(|s| w(&s.text)).sum();

        let notes = NOTES_MIN.max(spans).max(44);
        let card = self
            .sessions_panel(false)
            .outer(ratatui::layout::Size::new(LIST_W.max(notes), 0))
            .width;
        let (left, right) = self.strip(0, t);

        let status = self.status_bar().min_width(any, ctx);
        let captions = [
            STRIP_CAPTION.to_owned(),
            format!(
                "status bar · StatusBar::layout {}",
                self.placed_text(status)
            ),
            format!(
                "at {NARROW_STATUS} columns · StatusBar::layout {}",
                self.placed_text(NARROW_STATUS)
            ),
            HINT_CAPTION.to_owned(),
        ];
        [
            self.bar.measure(any, ctx).width,
            card,
            segments::min_width(&left, &right, t),
            status,
            self.layer(ctx.interaction.focus).measure(any, ctx).width,
        ]
        .into_iter()
        .chain(captions.iter().map(|c| w(c) + 1))
        .max()
        .unwrap_or(0)
    }

    fn notes_lines(w: u16) -> Vec<String> {
        let w = w.max(1) as usize;
        let mut lines = junie_tui::ui::text::wrap(NOTES_TEXT, w);
        lines.push(String::new());
        lines.extend(junie_tui::ui::text::wrap(BRAND_NOTE, w));
        lines
    }

    fn card_tiles(w: u16) -> Tiled {
        let list_h = |_: u16| LIST_ROWS;
        let notes_h = |w: u16| Self::notes_lines(w).len() as u16 + 3;
        tiles(
            w,
            &[&[
                Tile::new(LIST_W, &list_h).fixed(),
                Tile::new(NOTES_MIN, &notes_h),
            ]],
            (4, 1),
        )
    }

    fn card_h(&self, w: u16) -> u16 {
        let panel = self.sessions_panel(false);
        let inner = panel.inner(ratatui::layout::Size::new(w, 0)).width;
        panel
            .outer(ratatui::layout::Size::new(
                inner,
                Self::card_tiles(inner).size.height,
            ))
            .height
    }

    fn height(&self, w: u16) -> u16 {
        2 + self.card_h(w) + 1 + self.readout_lines(w).len() as u16 + 1 + 2 + 1 + 2 + 1 + 2 + 1 + 2
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let mut y = area.y;

        let bar_w = if self.opt("chrome.bar") == "narrow" {
            area.width.min(NARROW_BAR)
        } else {
            area.width
        };
        self.bar
            .render(Rect::new(area.x, y, bar_w, 1), buf, ctx, t.canvas);
        y += 2;

        let body = Rect::new(area.x, y, area.width, self.card_h(area.width));
        let panel = self.sessions_panel(ctx.interaction.focused(SESSIONS));
        let bg = panel.bg(t);
        let inner = panel.render(body, buf, t);
        let tiled = Self::card_tiles(inner.width);
        let list_area = tiled.at(0, inner);
        self.sessions.render(list_area, buf, ctx, bg);
        let notes = tiled.at(1, inner);
        let nx = notes.x;
        let lines = Self::notes_lines(notes.width);
        for (i, n) in lines.iter().enumerate() {
            buf.set_string(nx, notes.y + i as u16, n, t.muted().bg(bg));
        }

        let sy = notes.y + lines.len() as u16 + 1;
        let max = notes.width;
        render_spans(buf, nx, sy, max, t, &Self::checks_line(), bg);
        junie_tui::widgets::keyhint::render_status(
            Rect::new(nx, sy + 1, max.min(44), 1),
            buf,
            t,
            &[hint("Ctrl+S", "Save")],
            None,
            Some(("Saved", Some(StatusKind::Ok))),
        );

        let mut y = body.bottom() + 1;
        for line in self.readout_lines(area.width) {
            buf.set_string(area.x, y, line, t.faint().bg(t.canvas));
            y += 1;
        }
        y += 1;
        buf.set_string(area.x, y, STRIP_CAPTION, t.faint().bg(t.canvas));
        let (left, right) = self.strip(ctx.interaction.tick, t);
        let row = Rect::new(area.x, y + 1, area.width, 1);
        junie_tui::ui::ctx::fill(buf, row, ratatui::style::Style::new().bg(t.surface));
        segments::render(row, buf, ctx, &left, &right, t.surface);
        y += 3;

        let status = self.status_bar();
        for (caption, w) in [
            (
                format!(
                    "status bar · StatusBar::layout {}",
                    self.placed_text(area.width)
                ),
                area.width,
            ),
            (
                format!(
                    "at {NARROW_STATUS} columns · StatusBar::layout {}",
                    self.placed_text(NARROW_STATUS)
                ),
                NARROW_STATUS.min(area.width),
            ),
        ] {
            buf.set_string(area.x + 1, y, caption, t.faint().bg(t.canvas));
            status.render(Rect::new(area.x, y + 1, w, 1), buf, ctx);
            y += 3;
        }

        let layer = self.layer(ctx.interaction.focus);
        let hint_w = layer.measure(ratatui::layout::Size::new(0, 1), ctx).width;
        let hint_row = Rect::new(area.x, y + 1, hint_w.min(area.width), 1);
        self.hint_area = hint_row;
        buf.set_string(area.x + 1, y, HINT_CAPTION, t.faint().bg(t.canvas));
        HintBar::render(hint_row, buf, t, &layer);

        self.context.render(area, buf, ctx);
    }
}

const LIST_W: u16 = 48;

const NARROW_STATUS: u16 = 64;

const NOTES_TEXT: &str = "The status bar below sits on its own plane: three groups, no \
     separator glyphs, and items that do not fit collapse into a … that \
     shows them on hover, by priority — the 64-column row shows the center going first.";
const BRAND_NOTE: &str = "Brand: one lockup, accent-filled, the only accent-filled control.";

const NOTES_MIN: u16 = 36;

const LIST_ROWS: u16 = 6;

const STRIP_CAPTION: &str = "identity strip · segments drop by priority · ◆ opens the app menu";
const HINT_CAPTION: &str = "hint bar · topmost layer wins:";

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Pressed, Busy, Narrow])
        .column_width(26)
        .column(
            "Segment",
            Segment::new("payments-platform", Tone::Normal).bold(),
        )
        .column(
            "Clickable segment",
            Segment::new("production", Tone::Danger)
                .lead("◆", Tone::Danger)
                .clickable(id.child(1)),
        )
        .column(
            "Status item",
            StatusItem::new("PR #482 · settlement backoff", Tone::Secondary).clickable(id.child(2)),
        )
        .column(
            "Status chip",
            StatusItem::new("Weekly 59%", Tone::Warning)
                .chip()
                .clickable(id.child(3)),
        )
        .column(
            "Status meter",
            StatusItem::new("disk", Tone::Muted).meter(Some(72), MeterTone::Normal),
        )
}

impl Page for ChromePage {
    fn layer_open(&self) -> bool {
        self.bar.hides_page_focus() || self.context.hides_page_focus()
    }
    fn title(&self) -> &'static str {
        "Chrome"
    }
    fn blurb(&self) -> &'static str {
        "Brand lockup · menu bar with anchored menus · status bar planes and priorities · context menu · hint layers"
    }

    fn animating(&self) -> bool {
        true
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        let w = avail.width.max(self.natural_width(ctx));
        ratatui::layout::Size::new(w, self.height(w))
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
        if let Some((o, chosen)) = self.context.handle(ev) {
            if let Some((target, i)) = chosen {
                let label = self.context_items(target)[i].item.label.clone();
                self.last = match target {
                    Target::Session(_) => format!("tab › {label}"),
                    Target::Env => format!("production › {label}"),
                };
                cx.status(label);
            }
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if self.bar.is_open() || cx.focus.is(BAR) {
                    let (o, ev) = self.bar.on_key(key);
                    self.bar_action(ev, cx);
                    if o.consumed() {
                        return o;
                    }
                }
                if cx.focus.is(SESSIONS) {
                    if let Some(req) = self.context.config.on_key(&self.sessions, key) {
                        self.open_context(req);
                        return Outcome::Changed;
                    }
                    if key.code == KeyCode::F(10) && key.plain() {
                        cx.focus.focus(BAR);
                        self.bar.open_menu(0);
                        return Outcome::Changed;
                    }
                    return self.sessions.on_key(key);
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) if ev.is(Phase::Secondary) => {
                if let Some(req) = self.context.config.on_secondary(&mut self.sessions, ev) {
                    cx.focus.focus(SESSIONS);
                    self.open_context(req);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }

            PageEvent::Pointer(ev) => {
                if let Some(r) = self.bar.route(ev) {
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.bar_action(Some(a), cx);
                    }
                    if o.consumed() || !ev.is(Phase::Click) {
                        return o;
                    }
                }
                if ev.on(SESSIONS) && ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                    cx.focus.focus(SESSIONS);
                }
                if let Some(r) = self.sessions.route(ev) {
                    return r.outcome();
                }
                if !ev.is(Phase::Click) {
                    return Outcome::Ignored;
                }
                let id = ev.id;
                if id == ENV {
                    let menu = ContextMenu::new(CONTEXT, self.context_items(Target::Env))
                        .at(ev.pos())
                        .title("production");
                    self.context.open(menu, Target::Env);
                    return Outcome::Changed;
                }
                if id == PENDING {
                    self.last = "strip › 3 pending".into();
                    cx.status("3 pending changes");
                    return Outcome::Changed;
                }
                if id == USAGE || id == PR {
                    self.last = if id == USAGE {
                        "status › usage".into()
                    } else {
                        "status › PR".into()
                    };
                    cx.status(if id == USAGE {
                        "Usage chip"
                    } else {
                        "PR context"
                    });
                    return Outcome::Changed;
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
        self.layer(focus)
            .hints
            .iter()
            .filter_map(|h| match (&h.key, &h.action) {
                (std::borrow::Cow::Borrowed(key), std::borrow::Cow::Borrowed(action)) => {
                    Some((*key, *action))
                }
                _ => None,
            })
            .collect()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::Chrome)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::Chrome);
        let Some(i) = table.iter().position(|o| o.key == key) else {
            return;
        };
        self.opts[i] = value.min(table[i].values.len() - 1);
        self.bar.close();
        self.bar = self.build_bar();
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
