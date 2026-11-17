use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use termrock::core::event::{Key, Outcome};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys, hint_key, label_for};
use termrock::core::pointer::MouseTarget;
use termrock::theme::Strings;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::dialog::DialogResult;
use termrock::widgets::icon_button::IconButton;
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};
use termrock::widgets::tabs::{CloseReveal, TabAction, TabCmd, TabItem, TabRule, Tabs};

const ID: WidgetId = WidgetId::of("tabs-page");
const CLOSE_DLG: WidgetId = ID.sub("close-dialog");

fn editor_items(many: bool) -> Vec<TabItem> {
    let mut items = vec![
        TabItem::new("Overview").prefix("◆"),
        TabItem::new("orders.sql")
            .prefix("▷")
            .closable()
            .dirty(true),
        TabItem::new("Import").prefix("⇣").closable().busy(true),
        TabItem::new("Migrate").prefix("⇡").closable().error(true),
        TabItem::new("Export").prefix("⇡").closable().done(true),
        TabItem::new("users").prefix("▦").suffix("●").closable(),
    ];
    if !many {
        items.truncate(4);
    } else {
        for name in ["audit_log", "sessions", "invoices", "payments", "events"] {
            items.push(TabItem::new(name).prefix("▦").closable());
        }
    }
    items
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Opts {
    values: Vec<(&'static str, usize)>,
}

impl Opts {
    fn new() -> Self {
        Self {
            values: crate::options::page_options(PageId::Tabs)
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
    fn set(&mut self, key: &str, value: usize) -> bool {
        match self.values.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => {
                slot.1 = value;
                true
            }
            None => false,
        }
    }
}

fn editor_strip(o: &Opts, items: Vec<TabItem>) -> Tabs {
    let id = ID.sub("editor");
    let rule = match o.get("tabs.rule") {
        1 => TabRule::Accent,
        2 => TabRule::Neutral,
        3 => TabRule::FocusOnly,
        _ => TabRule::Auto,
    };
    let reveal = match o.get("tabs.close_reveal") {
        1 => CloseReveal::Hover,
        _ => CloseReveal::Always,
    };
    let mut t = Tabs::with_items(id, items)
        .rule(rule)
        .close_reveal(reveal)
        .allow_new(o.on("tabs.allow_new"))
        .wrap(o.on("tabs.wrap"))
        .middle_click_closes(o.on("tabs.middle_click"))
        .overflow_menu(o.on("tabs.overflow_menu"))
        .nav(match o.get("tabs.nav") {
            1 => NavKeys::VIM,
            _ => NavKeys::DESKTOP,
        });
    if o.get("tabs.close_button") == 1 {
        t = t.close_button(IconButton::close(id.sub("close-template")).danger_hover(true));
    }
    t = t.close_target_w(match o.get("tabs.close_w") {
        0 => 1,
        2 => 5,
        _ => 3,
    });
    if o.get("tabs.more_button") == 1 {
        t = t.more_button(IconButton::new(id.sub("more-template"), "+").pad(0));
    }
    if o.get("tabs.keymap") == 1 {
        let mut map: KeyMap<TabCmd> = vec![
            (KeyBinding::ctrl('w'), TabCmd::Close),
            (KeyBinding::ctrl('t'), TabCmd::New),
        ];
        map.extend(TabCmd::digit_keys());
        t = t.keymap(map);
    } else {
        t = t.keymap(TabCmd::default_keymap());
        if o.get("tabs.close_keys") == 1 {
            t = t.close_keys(&[KeyCode::Delete]);
        }
        t = t.new_key(match o.get("tabs.new_key") {
            1 => Some('t'),
            2 => None,
            _ => Some('n'),
        });
    }

    t.digit_jump(o.on("tabs.digit_jump"))
        .passive(o.get("tabs.passive") >= 1)
        .passive_focused(o.get("tabs.passive") == 2)
}

pub struct TabsPage {
    template: Template,
    opts: Opts,
    pub editor: Tabs,

    pub outer: Tabs,
    pub sub: Tabs,

    opened: usize,

    closing: Option<usize>,
    pub last: String,
}

impl TabsPage {
    pub fn new() -> Self {
        let opts = Opts::new();
        let editor = editor_strip(&opts, editor_items(true));
        Self {
            template: Template::new(ID, states_grid()),
            editor,
            outer: {
                let mut t = Tabs::new(ID.sub("outer"), &["Data", "Structure", "DDL"]);
                t.set_active(1);
                t.cursor = 1;
                t
            },
            sub: Tabs::new(
                ID.sub("sub"),
                &[
                    "Columns",
                    "Indexes",
                    "Foreign keys",
                    "Triggers",
                    "Checks",
                    "Options",
                ],
            )
            .allow_new(false),
            opts,
            opened: 0,
            closing: None,
            last: "none yet".into(),
        }
    }

    fn rebuild(&mut self, reset: bool) {
        let items = if reset {
            editor_items(self.opts.on("tabs.count"))
        } else {
            self.editor.items().to_vec()
        };
        let (active, cursor) = (self.editor.active, self.editor.cursor);
        self.editor = editor_strip(&self.opts, items);
        self.editor
            .set_active(active.min(self.editor.items().len().saturating_sub(1)));
        self.editor.cursor = cursor.min(self.editor.items().len().saturating_sub(1));
    }

    fn act(&mut self, a: TabAction, cx: &mut Cx) {
        match a {
            TabAction::Activated(i) => {
                self.last = format!("{a:?} · {}", self.editor.items()[i].label);
            }
            TabAction::New => {
                self.opened += 1;
                let name = format!("untitled-{}.sql", self.opened);
                self.editor.push(TabItem::new(&name).prefix("▷").closable());
                let n = self.editor.items().len() - 1;
                self.editor.set_active(n);
                self.editor.cursor = n;
                self.last = format!("{a:?} · opened {name}");
            }
            TabAction::Close(i) => {
                let it = &self.editor.items()[i];
                if it.dirty {
                    self.closing = Some(i);
                    self.last = format!("{a:?} · {} has unsaved changes: asked first", it.label);
                    cx.open_dialog(termrock::ui::leave::unsaved_dialog(
                        CLOSE_DLG,
                        1,
                        &Strings::DEFAULT,
                    ));
                } else {
                    self.last = format!("{a:?} · closed {}", it.label);
                    self.editor.remove(i);
                }
            }
        }
        cx.status(self.last.clone());
    }

    /// `d` toggles the active tab's dirty mark. `[` and `]` move it left or
    /// right. Both show in the hint bar. A refused move still consumes the key,
    /// so `[` and `]` do not leak to page navigation at the strip's ends.
    fn edit_active_tab(&mut self, key: &Key, cx: &mut Cx) -> Option<Outcome> {
        if !key.plain() {
            return None;
        }
        let active = self.editor.active;
        let label = self.editor.items().get(active)?.label.clone();
        let changed = match key.code {
            KeyCode::Char('d') => {
                let changed = self.editor.update_item(active, |it| it.dirty = !it.dirty);
                if changed {
                    self.last = format!("Toggled unsaved mark on {label}");
                }
                changed
            }
            KeyCode::Char(c @ ('[' | ']')) => {
                let to = if c == '[' {
                    active.saturating_sub(1)
                } else {
                    active + 1
                };
                let changed = self.editor.reorder(active, to);
                if changed {
                    self.last = format!("Moved {label} to position {}", to + 1);
                }
                changed
            }
            _ => return None,
        };
        if !changed {
            return Some(Outcome::Consumed);
        }
        cx.status(self.last.clone());
        Some(Outcome::Changed)
    }

    fn facts(&self) -> Vec<Prop> {
        let active = self.editor.items().get(self.editor.active);
        let state = active.map_or(String::new(), |it| {
            let mut s = vec![];
            if it.dirty {
                s.push("unsaved");
            }
            if it.busy {
                s.push("busy");
            }
            if it.error {
                s.push("failed");
            }
            if it.done {
                s.push("done");
            }
            s.push(if it.closable {
                "closable"
            } else {
                "pinned (not closable)"
            });
            format!("{} · {}", it.label, s.join(" · "))
        });
        let (before, after) = self.editor.hidden();
        vec![
            Prop::new("Active", state).wrap(),
            Prop::new("Hidden", format!("{before} before · {after} after")),
            Prop::new("Keys", keys_readout(&self.editor)).wrap(),
            Prop::new("Emits", "TabAction::Activated · Close · New").wrap(),
            Prop::new("Last", self.last.clone()).wrap(),
        ]
    }

    fn layout_at(&self, width: u16, ctx: &RenderCtx) -> (u16, u16, u16) {
        let any = Size::new(0, 2);
        let card = Panel::card(Some("Editor strip")).meta(EDITOR_META);
        let nest = Panel::card(Some("Nested strips")).meta(NEST_META);
        let facts = self.facts();
        let strips = self
            .outer
            .measure(any, ctx)
            .width
            .max(self.sub.measure(any, ctx).width);

        let w = width
            .max(sheet_width(&card, &facts))
            .max(nest.outer(Size::new(strips, 0)).width);
        (w, sheet_height(&card, &facts, w), NEST_H)
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (_, card_h, nest_h) = self.layout_at(area.width, ctx);
        let rows = crate::pages::layout::rows(area, &[2, 1, card_h, 1, nest_h, 0]);
        self.editor.render(rows[0], buf, ctx, t.canvas);

        let card = Panel::card(Some("Editor strip")).meta(EDITOR_META);
        let bg = card.bg(t);
        let inner = card.render(rows[2], buf, t);
        props::render(inner, buf, t, &self.facts(), bg);

        let nest = Panel::card(Some("Nested strips")).meta(NEST_META);
        let nbg = nest.bg(t);
        let inner = nest.render(rows[4], buf, t);

        let split = |r: Rect| match self.opts.get("tabs.gap") {
            0 => Tabs::layout(r),
            _ => Tabs::layout_gap(r, 0),
        };
        let (strip, body) = split(inner);
        self.outer.render(strip, buf, ctx, nbg);
        let (strip, body) = split(body);
        let crumb = match self.outer.active {
            1 => {
                self.sub.render(strip, buf, ctx, nbg);
                format!(
                    "orders › Structure › {}",
                    self.sub.items()[self.sub.active].label
                )
            }
            i => format!("orders › {}", self.outer.items()[i].label),
        };
        buf.set_string(body.x, body.y, crumb, t.muted().bg(nbg));
    }
}

const EDITOR_META: &str = "click · × · + · ‹ › · wheel · middle-click";
const NEST_META: &str = "Tabs::layout · 1–6 jump";

const NEST_H: u16 = 9;

impl TabsPage {
    fn strip_mut(&mut self, id: WidgetId) -> Option<&mut Tabs> {
        if id == self.editor.id {
            Some(&mut self.editor)
        } else if id == self.outer.id {
            Some(&mut self.outer)
        } else if self.outer.active == 1 && id == self.sub.id {
            Some(&mut self.sub)
        } else {
            None
        }
    }
}

fn keys_readout(t: &Tabs) -> String {
    if t.passive {
        return if t.passive_focused {
            "passive · the owner holds the keys and shows focus".into()
        } else {
            "passive · the owner holds the keys".into()
        };
    }
    let mut parts = vec![
        if t.nav.vim {
            "←→ h l move"
        } else {
            "←→ move"
        }
        .to_owned(),
    ];
    if let Some(k) = label_for(&t.keymap, TabCmd::Jump(0)) {
        parts.push(format!("{k}–9 jump"));
    }
    if let Some(k) = label_for(&t.keymap, TabCmd::Close) {
        parts.push(format!("{k} close"));
    }
    if t.allow_new
        && let Some(k) = label_for(&t.keymap, TabCmd::New)
    {
        parts.push(format!("{k} new tab"));
    }

    if t.overflow_menu
        && let Some(k) = label_for(&t.keymap, TabCmd::ShowHidden)
    {
        parts.push(format!("{k} hidden tabs"));
    }
    if t.wrap {
        parts.push("wraps".into());
    }
    if t.middle_click_closes {
        parts.push("middle-click closes".into());
    }
    parts.join(" · ")
}

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
            Busy,
            Error,
            Overflow,
            Narrow,
        ])
        .column_width(26)
        .narrow_width(14)
        .column("Plain", Tabs::new(id.child(0), &["Data", "Logs"]))
        .column(
            "Closable",
            Tabs::with_items(
                id.child(1),
                vec![
                    TabItem::new("a.sql").closable(),
                    TabItem::new("b.sql").closable(),
                ],
            ),
        )
        .column(
            "Icons",
            Tabs::with_items(
                id.child(2),
                vec![
                    TabItem::new("users").prefix("▦"),
                    TabItem::new("q1").prefix("▷").suffix("●"),
                ],
            ),
        )
}

impl Page for TabsPage {
    fn title(&self) -> &'static str {
        "Tabs"
    }

    fn blurb(&self) -> &'static str {
        "Tab items (icons, dirty, busy, error, closable), rules, overflow and nested strips"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let (w, card_h, nest_h) = self.layout_at(avail.width, ctx);
        Size::new(w, 2 + 1 + card_h + 1 + nest_h)
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
            PageEvent::Tick => {
                if self.editor.items().iter().any(|t| t.busy) {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.editor.id {
                    let (out, a) = self.editor.on_key(key);
                    if let Some(a) = a {
                        self.act(a, cx);
                    }
                    if out == Outcome::Ignored {
                        return self.edit_active_tab(key, cx).unwrap_or(out);
                    }
                    return out;
                }
                match self.strip_mut(f) {
                    Some(s) => s.on_key(key).0,
                    None => Outcome::Ignored,
                }
            }

            PageEvent::Pointer(ev) => {
                let click = ev.is(Phase::Click);
                if let Some(r) = self.editor.route(ev) {
                    if click {
                        cx.focus.focus(self.editor.id);
                    }
                    let (out, actions) = r.into_parts();
                    for a in actions {
                        self.act(a, cx);
                    }
                    return if click { out.or(Outcome::Changed) } else { out };
                }
                let sub_shown = self.outer.active == 1;
                for s in [&mut self.outer]
                    .into_iter()
                    .chain(sub_shown.then_some(&mut self.sub))
                {
                    let sid = s.id;
                    if let Some(r) = s.route(ev) {
                        let out = r.outcome();
                        if click {
                            cx.focus.focus(sid);
                            return out.or(Outcome::Changed);
                        }
                        return out;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::DialogClosed { id, result, .. } if *id == CLOSE_DLG => {
                let Some(i) = self.closing.take() else {
                    return Outcome::Ignored;
                };
                let label = self.editor.items()[i].label.clone();
                self.last = match result {
                    DialogResult::Action(1) => {
                        self.editor.remove(i);
                        format!("Closed {label}, changes discarded")
                    }
                    DialogResult::Action(2) => {
                        self.editor.remove(i);
                        format!("Saved and closed {label}")
                    }
                    _ => format!("Kept {label} open"),
                };
                cx.status(self.last.clone());
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        match focus {
            Some(f) if f == self.editor.id => {
                let map = &self.editor.keymap;
                let mut h = vec![("←→", "Tab")];
                if hint_key(map, TabCmd::Jump(0)).is_some() {
                    h.push(("1–9", "Jump"));
                }
                if let Some(k) = hint_key(map, TabCmd::Close) {
                    h.push((k, "Close"));
                }
                if self.editor.allow_new
                    && let Some(k) = hint_key(map, TabCmd::New)
                {
                    h.push((k, "New tab"));
                }
                h.push(("d", "Dirty"));
                h.push(("[ ]", "Move"));
                h
            }
            Some(f) if f == self.outer.id || f == self.sub.id => {
                vec![("←→", "Tab"), ("1–6", "Jump")]
            }
            _ => Vec::new(),
        }
    }

    fn animating(&self) -> bool {
        self.template.section() == Section::Playground && self.editor.items().iter().any(|t| t.busy)
    }

    fn layer_open(&self) -> bool {
        self.editor.menu_open() || self.template.popup_open()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts
            .values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.rebuild(key == "tabs.count");
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
    Box::new(TabsPage::new())
}
