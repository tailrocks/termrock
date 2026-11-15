use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect, Size};

use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, LayerRes, Page, PageEvent, Report};
use junie_tui::core::event::{ClientPlatform, InputConfig, Key, Outcome};
use junie_tui::core::focus::cancel_focus_request;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keyboard::{self, KeyboardProtocol, ProtocolState};
use junie_tui::core::keymap::{KeyBinding, Nav, NavKeys, select_all_bindings};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::{FocusVisible, RenderCtx};
use junie_tui::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec, LayerTag};
use junie_tui::ui::measure::{Tile, Tiled, tiles};
use junie_tui::ui::modal::Frame;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::tabs::Tabs;

const ID: WidgetId = WidgetId::of("focusnav");

const LAYER_TAG: &str = "focuslayer";
const LAYER_GO: WidgetId = ID.sub("layer.go");

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayerDemo {
    Tip,
    Restore,
    Stay,
}

impl LayerDemo {
    fn spec(self) -> LayerSpec {
        match self {
            Self::Tip => LayerSpec::tooltip(),
            Self::Restore => LayerSpec::popover(Dismiss::LIGHT).initial_focus(LAYER_GO),
            Self::Stay => LayerSpec::popover(Dismiss::LIGHT)
                .initial_focus(LAYER_GO)
                .restore_focus(false),
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Tip => "Passive tip",
            Self::Restore => "restore_focus on",
            Self::Stay => "restore_focus off",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::Tip => "Opens and closes without moving focus.",
            Self::Restore => "Closing returns focus to the opener.",
            Self::Stay => "Closing keeps the focus the layer set.",
        }
    }
}

struct FocusLayer {
    demo: LayerDemo,
    anchor: Rect,
    go: Button,
    target: WidgetId,
    result: Option<LayerOut<LayerRes>>,
}

impl FocusLayer {
    fn new(demo: LayerDemo, anchor: Rect, target: WidgetId) -> Self {
        Self {
            demo,
            anchor,
            go: Button::primary(LAYER_GO, "Focus Cancel"),
            target,
            result: None,
        }
    }

    fn finish(&mut self, cx: &mut LayerCx<'_, ()>) -> Outcome {
        cx.focus.focus(self.target);
        self.result = Some(LayerOut::Close(LayerRes::Page(Report {
            result: None,
            values: None,
            closed: None,
        })));
        Outcome::Changed
    }
}

impl Layer<LayerRes> for FocusLayer {
    fn spec(&self) -> LayerSpec {
        self.demo.spec()
    }

    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &()) {
        let t = ctx.theme;
        let frame = match self.demo {
            LayerDemo::Tip => Frame::new(42, 3)
                .min_size(20, 3)
                .chrome(0, 0)
                .margin(ratatui::layout::Margin::new(1, 0))
                .backdrop(false)
                .anchor(self.anchor, Placement::Above),
            _ => Frame::new(44, 6)
                .title(self.demo.title())
                .backdrop(false)
                .dismiss(self.demo.spec().dismiss)
                .anchor(self.anchor, Placement::Above),
        };
        let (_, inner) = frame.render(screen, buf, ctx);
        let bg = t.surface_elevated;
        buf.set_string(
            inner.x,
            inner.y,
            junie_tui::ui::text::truncate(self.demo.note(), inner.width as usize),
            t.muted().bg(bg),
        );
        if self.demo != LayerDemo::Tip && inner.height > 2 {
            let w = self.go.width().min(inner.width);
            self.go
                .render(Rect::new(inner.x, inner.y + 2, w, 1), buf, ctx, bg);
        }
    }

    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, ()>) -> Outcome {
        if self.demo != LayerDemo::Tip && cx.focus.is(LAYER_GO) && self.go.on_key(key).1 {
            return self.finish(cx);
        }
        Outcome::Ignored
    }

    fn on_click(&mut self, id: WidgetId, _: Position, cx: &mut LayerCx<'_, ()>) -> Outcome {
        if id == LAYER_GO && self.go.on_click() {
            return self.finish(cx);
        }
        Outcome::Consumed
    }

    fn hints(&self, _: Option<WidgetId>) -> Vec<junie_tui::widgets::keyhint::Hint> {
        use junie_tui::widgets::keyhint::hint;
        match self.demo {
            LayerDemo::Tip => vec![hint("Esc", "Close")],
            _ => vec![hint("Enter", "Focus Cancel"), hint("Esc", "Close")],
        }
    }

    fn on_dismiss(&mut self, _: DismissReason) {}

    fn take_result(&mut self) -> Option<LayerOut<LayerRes>> {
        self.result.take()
    }
}

const NAVS: [Nav; 8] = [
    Nav::Up,
    Nav::Down,
    Nav::Left,
    Nav::Right,
    Nav::PageUp,
    Nav::PageDown,
    Nav::Home,
    Nav::End,
];

fn candidate_keys() -> Vec<(&'static str, Key)> {
    let k = |code| Key {
        code,
        mods: KeyModifiers::NONE,
    };
    vec![
        ("↑", k(KeyCode::Up)),
        ("↓", k(KeyCode::Down)),
        ("←", k(KeyCode::Left)),
        ("→", k(KeyCode::Right)),
        ("k", k(KeyCode::Char('k'))),
        ("j", k(KeyCode::Char('j'))),
        ("h", k(KeyCode::Char('h'))),
        ("l", k(KeyCode::Char('l'))),
        ("PgUp", k(KeyCode::PageUp)),
        ("PgDn", k(KeyCode::PageDown)),
        ("Home", k(KeyCode::Home)),
        ("End", k(KeyCode::End)),
        ("g", k(KeyCode::Char('g'))),
        ("G", k(KeyCode::Char('G'))),
    ]
}

fn nav_map(nav: NavKeys) -> Vec<(Nav, String)> {
    let keys = candidate_keys();
    NAVS.iter()
        .map(|&n| {
            let ks: Vec<&str> = keys
                .iter()
                .filter(|(_, k)| nav.action(k) == Some(n))
                .map(|(l, _)| *l)
                .collect();
            (
                n,
                if ks.is_empty() {
                    "—".into()
                } else {
                    ks.join(" ")
                },
            )
        })
        .collect()
}

fn nav_keys(v: usize) -> NavKeys {
    match v {
        1 => NavKeys::DESKTOP.vim(true),
        2 => NavKeys::DESKTOP.page(false),
        3 => NavKeys::DESKTOP.home_end(false),
        4 => NavKeys::DESKTOP.vim(true).arrows(false),
        _ => NavKeys::DESKTOP,
    }
}

pub struct FocusNavPage {
    template: Template,
    pub name: TextInput,
    pub email: TextInput,
    pub save: Button,
    pub cancel: Button,

    pub tags: Vec<Button>,
    pub list: ListBox,

    layer_buttons: [(LayerDemo, Button); 3],

    nav: usize,
    tab_stays: bool,
    focus_visible: FocusVisible,

    pub last_change: String,

    ring_len: usize,

    pub last_nav: String,
}

impl FocusNavPage {
    pub fn new() -> Self {
        let tags = ["Alpha", "Beta", "Gamma", "Delta"]
            .iter()
            .enumerate()
            .map(|(i, t)| Button::secondary(ID.sub("tag").child(i), t))
            .collect();
        let items = [
            "Inbox", "Drafts", "Sent", "Archive", "Spam", "Trash", "Notes", "Later", "Travel",
            "Receipts", "Family", "Work",
        ]
        .iter()
        .map(|s| ListItem::new(s))
        .collect();
        Self {
            template: Template::new(ID, states_grid()),
            name: TextInput::new(ID.child(0), "Name").value("Ada Lovelace"),
            email: TextInput::new(ID.child(1), "Email").value("ada@example.com"),
            save: Button::primary(ID.child(2), "Save"),
            cancel: Button::subtle(ID.child(3), "Cancel"),
            tags,
            list: ListBox::new(ID.child(4), items, SelectMode::Single).nav(NavKeys::DESKTOP),
            layer_buttons: [
                (
                    LayerDemo::Tip,
                    Button::secondary(ID.sub("layer.tip"), "Passive tip"),
                ),
                (
                    LayerDemo::Restore,
                    Button::secondary(ID.sub("layer.restore"), "Restore on"),
                ),
                (
                    LayerDemo::Stay,
                    Button::secondary(ID.sub("layer.stay"), "Restore off"),
                ),
            ],
            nav: 0,
            tab_stays: false,
            focus_visible: FocusVisible::Always,
            last_change: "none yet".into(),
            ring_len: 0,
            last_nav: "none yet".into(),
        }
    }

    fn name_of(&self, id: Option<WidgetId>) -> String {
        let Some(id) = id else {
            return "nothing".into();
        };
        if id == self.name.id {
            return "Name field".into();
        }
        if id == self.email.id {
            return "Email field".into();
        }
        if id == self.save.id {
            return "Save".into();
        }
        if id == self.cancel.id {
            return "Cancel".into();
        }
        if let Some((_, b)) = self.layer_buttons.iter().find(|(_, b)| b.id == id) {
            return format!("{} button", b.label);
        }
        if id == self.list.id {
            return "Mailbox list".into();
        }
        if id == self.template.tabs.id {
            return "section strip".into();
        }
        if let Some(t) = self.tags.iter().find(|t| t.id == id) {
            return format!("{} tag", t.label);
        }
        "outside the page".into()
    }

    fn focus_facts(&self, ctx: &RenderCtx) -> Vec<Prop> {
        let ring = if self.ring_len > 0 {
            format!("{} stops · reading order", self.ring_len)
        } else {
            "reading order".into()
        };
        vec![
            Prop::new("Focus", self.name_of(ctx.interaction.focus)),
            Prop::new("Change", self.last_change.clone()).wrap(),
            Prop::new("Ring", ring),
            Prop::new("Fallback", "Delete on a tag removes it").wrap(),
            Prop::new(
                "Field Tab",
                if self.tab_stays {
                    "stays: cancel_focus_request"
                } else {
                    "moves: request_focus_next"
                },
            ),
        ]
    }

    fn nav_facts(&self) -> Vec<Prop> {
        nav_map(self.list.nav)
            .into_iter()
            .map(|(n, k)| Prop::new(nav_label(n), k))
            .collect()
    }

    fn keys_facts(&self) -> Vec<Prop> {
        let select_all: Vec<String> = select_all_bindings(())
            .filter(|(b, _)| b.deliverable())
            .map(|(b, _)| b.label())
            .collect();
        let protocol = KeyboardProtocol::current();

        let stand_in = KeyBinding::new(
            KeyCode::Left,
            ratatui::crossterm::event::KeyModifiers::CONTROL
                | ratatui::crossterm::event::KeyModifiers::SHIFT,
        );

        let mut state = ProtocolState::default();
        let push = state
            .enter(KeyboardProtocol::On, || false)
            .unwrap_or_default();
        let pop = state.leave().unwrap_or("");

        let mac = InputConfig::for_platform(ClientPlatform::MacOs);
        let other = InputConfig::for_platform(ClientPlatform::Other);
        vec![
            Prop::new(
                "Bindings",
                format!(
                    "back {} · palette {} · select all {}",
                    KeyBinding::key(KeyCode::Tab).shift().label(),
                    KeyBinding::ctrl('k').label(),
                    select_all.join(" / ")
                ),
            )
            .wrap(),
            Prop::new(
                "Input",
                format!(
                    "Ctrl+click is right-click: macOS {}, elsewhere {} · Shift+wheel sideways {} · legacy Ctrl punctuation {}",
                    on_off(mac.ctrl_click_secondary),
                    on_off(other.ctrl_click_secondary),
                    on_off(mac.shift_wheel_horizontal && other.shift_wheel_horizontal),
                    on_off(mac.legacy_ctrl_punctuation && other.legacy_ctrl_punctuation)
                ),
            )
            .wrap(),
            Prop::new(
                "Protocol",
                format!(
                    "{protocol:?} (env) · active {} · Cmd delivered {} · On pushes {}, pops {}",
                    on_off(keyboard::active()),
                    on_off(keyboard::super_delivered()),
                    printable(&push),
                    printable(pop)
                ),
            )
            .wrap(),
            Prop::new(
                "Alt keys",
                format!(
                    "{:?} · delivered {} · resize {} · stand-in {}",
                    keyboard::AltKeys::current(),
                    on_off(keyboard::alt_delivered()),
                    KeyBinding::new(
                        KeyCode::Left,
                        ratatui::crossterm::event::KeyModifiers::ALT
                            | ratatui::crossterm::event::KeyModifiers::SHIFT
                    )
                    .label(),
                    stand_in.alt_stand_in().label()
                ),
            )
            .wrap(),
        ]
    }

    fn ring_panel() -> Panel<'static> {
        Panel::card(Some("Focus ring")).meta("Tab · Shift+Tab · click")
    }

    fn layer_panel() -> Panel<'static> {
        Panel::card(Some("Layer focus")).meta("restore_focus")
    }

    fn layer_width(&self) -> u16 {
        let buttons: u16 = self
            .layer_buttons
            .iter()
            .map(|(_, b)| b.width() + 1)
            .sum::<u16>()
            .saturating_sub(1);
        buttons.max(LAYER_NOTE.chars().count() as u16)
    }

    fn open_layer(&mut self, demo: LayerDemo, cx: &mut Cx) {
        let anchor = self
            .layer_buttons
            .iter()
            .find(|(d, _)| *d == demo)
            .map_or(Rect::ZERO, |(_, b)| b.area);
        cx.open(
            FocusLayer::new(demo, anchor, self.cancel.id),
            LayerTag::new(LAYER_TAG),
        );
    }

    fn list_panel(&self, ctx: &RenderCtx) -> Panel<'static> {
        Panel::card(Some("Mailboxes"))
            .meta("nav()")
            .focused(ctx.interaction.focused(self.list.id))
    }

    fn ring_width(&self) -> u16 {
        let buttons = self.save.width() + 2 + self.cancel.width();
        let tags: u16 = self.tags.iter().map(|b| b.width() + 1).sum();
        FIELD_W.max(buttons).max(tags.saturating_sub(1))
    }

    fn tiled(&self, width: u16, ctx: &RenderCtx) -> Tiled {
        let focus_panel = Panel::card(Some("Focus"));
        let nav_panel = Panel::card(Some("Nav map")).meta(&self.last_nav);
        let keys_panel = Panel::card(Some("Keys and input"));
        let (focus, nav, keys) = (self.focus_facts(ctx), self.nav_facts(), self.keys_facts());
        let ring_size = Self::ring_panel().outer(Size::new(self.ring_width(), RING_ROWS));
        let list_panel = self.list_panel(ctx);
        let list_size = list_panel.outer(Size::new(LIST_W, self.list.items.len() as u16));
        let layer_size = Self::layer_panel().outer(Size::new(self.layer_width(), LAYER_ROWS));
        let layer_h = |_: u16| layer_size.height;
        let ring_h = |_: u16| ring_size.height;
        let focus_h = |w: u16| sheet_height(&focus_panel, &focus, w);
        let list_h = |_: u16| list_size.height;
        let nav_h = |w: u16| sheet_height(&nav_panel, &nav, w);
        let keys_h = |w: u16| sheet_height(&keys_panel, &keys, w);
        tiles(
            width,
            &[
                &[
                    Tile::new(ring_size.width, &ring_h),
                    Tile::new(sheet_width(&focus_panel, &focus), &focus_h),
                ],
                &[
                    Tile::new(list_size.width, &list_h),
                    Tile::new(sheet_width(&nav_panel, &nav), &nav_h),
                ],
                &[
                    Tile::new(layer_size.width, &layer_h),
                    Tile::new(sheet_width(&keys_panel, &keys), &keys_h),
                ],
            ],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let tiled = self.tiled(area.width, ctx);

        let panel = Self::ring_panel();
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(0, area), buf, t);
        let w = inner.width.min(FIELD_W);
        self.name
            .render(Rect::new(inner.x, inner.y, w, 2), buf, ctx, bg);
        self.email
            .render(Rect::new(inner.x, inner.y + 2, w, 2), buf, ctx, bg);
        let y = inner.y + 5;
        let widths = [self.save.width(), self.cancel.width()];
        let r = row_layout(Rect::new(inner.x, y, inner.width, 1), &widths, 2);
        self.save.focus_visible = self.focus_visible;
        self.cancel.focus_visible = self.focus_visible;
        self.save.render(r[0], buf, ctx, bg);
        self.cancel.render(r[1], buf, ctx, bg);
        let y = inner.y + 7;
        let widths: Vec<u16> = self.tags.iter().map(|b| b.width()).collect();
        let r = row_layout(Rect::new(inner.x, y, inner.width, 1), &widths, 1);
        for (b, r) in self.tags.iter_mut().zip(r) {
            b.focus_visible = self.focus_visible;
            b.render(r, buf, ctx, bg);
        }

        let panel = Panel::card(Some("Focus"));
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(1, area), buf, t);
        props::render(inner, buf, t, &self.focus_facts(ctx), bg);

        let panel = self.list_panel(ctx);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(2, area), buf, t);
        self.list.render(inner, buf, ctx, bg);
        let panel = Panel::card(Some("Nav map")).meta(&self.last_nav);
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(3, area), buf, t);
        props::render(inner, buf, t, &self.nav_facts(), bg);

        let panel = Self::layer_panel();
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(4, area), buf, t);
        let widths: Vec<u16> = self.layer_buttons.iter().map(|(_, b)| b.width()).collect();
        let r = row_layout(Rect::new(inner.x, inner.y, inner.width, 1), &widths, 1);
        for ((_, b), r) in self.layer_buttons.iter_mut().zip(r) {
            b.render(r, buf, ctx, bg);
        }
        if inner.height > 2 {
            buf.set_string(
                inner.x,
                inner.y + 2,
                junie_tui::ui::text::truncate(LAYER_NOTE, inner.width as usize),
                t.muted().bg(bg),
            );
        }

        let panel = Panel::card(Some("Keys and input"));
        let bg = panel.bg(t);
        let inner = panel.render(tiled.at(5, area), buf, t);
        props::render(inner, buf, t, &self.keys_facts(), bg);
    }
}

const LAYER_ROWS: u16 = 3;
const LAYER_NOTE: &str = "A tip never moves focus; a popover may leave it.";

const FIELD_W: u16 = 30;
const RING_ROWS: u16 = 8;

const LIST_W: u16 = 20;

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

fn printable(s: &str) -> String {
    if s.is_empty() {
        return "nothing".into();
    }
    s.replace('\x1b', "ESC")
}

fn nav_label(n: Nav) -> &'static str {
    match n {
        Nav::Up => "Up",
        Nav::Down => "Down",
        Nav::Left => "Left",
        Nav::Right => "Right",
        Nav::PageUp => "PageUp",
        Nav::PageDown => "PageDown",
        Nav::Home => "Home",
        Nav::End => "End",
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, FocusedHover])
        .column_width(24)
        .column("Button", Button::secondary(id.child(0), "Save"))
        .column("Field", TextInput::new(id.child(1), "Name").value("Ada"))
        .column("Tab strip", Tabs::new(id.child(2), &["Data", "Logs"]))
}

impl Page for FocusNavPage {
    fn title(&self) -> &'static str {
        "Focus & navigation"
    }

    fn blurb(&self) -> &'static str {
        "The focus ring, fallback, nav keys, key bindings, input and keyboard protocol"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        self.tiled(avail.width, ctx).size
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
        if let PageEvent::Blur(ch) = ev {
            self.last_change = format!("{} → {}", self.name_of(Some(ch.from)), self.name_of(ch.to));
            self.ring_len = cx.ring.reachable().len();
            for f in [&mut self.name, &mut self.email] {
                if f.id == ch.from {
                    f.on_blur();
                }
            }
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Blur(_) => Outcome::Changed,
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if let Some(field) = [&mut self.name, &mut self.email]
                    .into_iter()
                    .find(|x| x.id == f)
                {
                    let (out, act) = field.on_key(key);
                    if matches!(act, Some(InputAction::CommittedTab { .. })) && self.tab_stays {
                        cancel_focus_request();
                    }
                    return out;
                }
                if f == self.list.id {
                    if let Some(n) = self.list.nav.action(key) {
                        self.last_nav = format!("last: {}", nav_label(n));
                    }
                    return self.list.on_key(key);
                }
                if let Some(i) = self.tags.iter().position(|b| b.id == f)
                    && key.code == KeyCode::Delete
                {
                    let gone = self.tags.remove(i);
                    cx.status(format!("Removed {}", gone.label));
                    return Outcome::Changed;
                }
                if let Some(i) = self.layer_buttons.iter().position(|(_, b)| b.id == f) {
                    let (out, activated) = self.layer_buttons[i].1.on_key(key);
                    if activated {
                        self.open_layer(self.layer_buttons[i].0, cx);
                    }
                    return out;
                }
                for b in [&mut self.save, &mut self.cancel]
                    .into_iter()
                    .chain(self.tags.iter_mut())
                {
                    if b.id == f {
                        let (out, activated) = b.on_key(key);
                        if activated {
                            cx.status(format!("{} ✓", b.label));
                        }
                        return out;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for i in 0..self.layer_buttons.len() {
                    let (demo, b) = &mut self.layer_buttons[i];
                    let (demo, id) = (*demo, b.id);
                    if let Some(r) = b.route(ev) {
                        if !ev.is(Phase::Click) {
                            return r.outcome();
                        }
                        cx.focus.focus(id);
                        if r.into_action().is_some() {
                            self.open_layer(demo, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                for field in [&mut self.name, &mut self.email] {
                    let fid = field.id;
                    if let Some(r) = field.route(ev) {
                        if ev.is(Phase::Click) {
                            cx.focus.focus(fid);
                        }
                        return r.outcome();
                    }
                }
                if ev.on(self.list.id) && ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                    cx.focus.focus(self.list.id);
                }
                if let Some(r) = self.list.route(ev) {
                    return r.outcome();
                }
                for b in [&mut self.save, &mut self.cancel]
                    .into_iter()
                    .chain(self.tags.iter_mut())
                {
                    if let Some(r) = b.route(ev) {
                        if !ev.is(Phase::Click) {
                            return r.outcome();
                        }
                        if r.into_action().is_some() {
                            cx.status(format!("{} ✓", b.label));
                        }
                        return Outcome::Changed;
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
        match focus {
            Some(f) if self.tags.iter().any(|t| t.id == f) => {
                vec![("Enter / Space", "Activate"), ("Delete", "Remove tag")]
            }
            Some(f) if f == self.list.id => vec![("↑↓", "Move")],
            Some(f) if self.layer_buttons.iter().any(|(_, b)| b.id == f) => {
                vec![("Enter / Space", "Open layer")]
            }
            _ => Vec::new(),
        }
    }

    fn editing(&self) -> bool {
        self.name.editing || self.email.editing
    }

    fn on_layer(&mut self, _: &LayerTag, _: Report, cx: &mut Cx) -> Outcome {
        cx.status(format!(
            "Layer closed · focus on {}",
            self.name_of(cx.focus.current())
        ));
        Outcome::Changed
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        match key {
            "focusnavigation.nav" => Some(self.nav),
            "focusnavigation.tab_in_field" => Some(usize::from(self.tab_stays)),
            "focusnavigation.focus_visible" => Some(usize::from(
                self.focus_visible == FocusVisible::KeyboardOnly,
            )),
            _ => None,
        }
    }

    fn set_option(&mut self, key: &str, value: usize) {
        match key {
            "focusnavigation.nav" => {
                self.nav = value;
                let items = self.list.items.clone();
                let cursor = self.list.cursor;
                let nav = nav_keys(value);

                self.list = ListBox::new(self.list.id, items, SelectMode::Single)
                    .nav(nav)
                    .type_ahead(!nav.vim);
                self.list.cursor = cursor;
            }
            "focusnavigation.tab_in_field" => self.tab_stays = value == 1,
            "focusnavigation.focus_visible" => {
                self.focus_visible = if value == 1 {
                    FocusVisible::KeyboardOnly
                } else {
                    FocusVisible::Always
                };
            }
            _ => {}
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
    Box::new(FocusNavPage::new())
}
