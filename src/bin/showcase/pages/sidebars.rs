use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::nav_list::{NavAction, NavItem, NavList};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::rail::{RailAction, SidebarRail};

const ID: WidgetId = WidgetId::of("sidebars");

const ITEMS: [(&str, &str, &str, Option<&str>, bool); 8] = [
    ("Tasks", "T", "Workspace", Some("3"), false),
    ("Runs", "R", "Workspace", None, false),
    ("Branches", "B", "Workspace", None, false),
    ("Members", "M", "Project", None, false),
    ("Environment", "E", "Project", None, false),
    ("Billing", "$", "Project", None, true),
    ("Keyboard", "K", "Preferences", None, false),
    ("Appearance", "A", "Preferences", None, false),
];

/// The item set named by the `sidebars.items` option. `full` is the approved
/// playground; the other sets exercise shrink, empty and all-disabled lists.
fn items(badges: bool, set: &str) -> Vec<NavItem> {
    let full: Vec<NavItem> = ITEMS
        .iter()
        .map(|&(label, icon, section, badge, disabled)| {
            let it = NavItem::new(label)
                .icon(icon)
                .section(section)
                .disabled(disabled);
            match badge {
                Some(b) if badges => it.badge(b),
                _ => it,
            }
        })
        .collect();
    match set {
        "short" => full.into_iter().take(3).collect(),
        "empty" => Vec::new(),
        "all-disabled" => full.into_iter().map(|it| it.disabled(true)).collect(),
        _ => full,
    }
}

pub struct SidebarsPage {
    template: Template,
    opts: PageValues,
    pub nav: NavList,
    collapse: Button,

    rail: SidebarRail,

    pub hidden: bool,

    last: String,
}

impl SidebarsPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Sidebars));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            nav: NavList::new(ID.sub("nav"), items(true, "full")).current(0),
            collapse: Button::secondary(ID.sub("collapse"), "Collapse"),
            rail: SidebarRail::new(ID.sub("rail"), "Navigation"),
            hidden: false,
            last: "none yet".into(),
            opts,
        };
        page.apply();
        page
    }

    fn apply(&mut self) {
        let o = &self.opts;
        let width = o.number("sidebars.width").unwrap_or(24);
        let collapsed_w = o.number("sidebars.collapsed_width").unwrap_or(6);
        self.nav
            .set_items(items(o.on("sidebars.badges"), o.value("sidebars.items")));
        self.nav.collapsed = o.on("sidebars.collapsed");
        self.nav.widths = (width, collapsed_w);
        self.nav.nav = match o.value("sidebars.keys") {
            "desktop" => NavKeys::DEFAULT,
            _ => NavKeys::VIM,
        };
        self.rail = SidebarRail::new(ID.sub("rail"), o.value("sidebars.rail_label"))
            .collapsed_rail(o.on("sidebars.collapsed_rail"));
        self.hidden = o.on("sidebars.hidden");
    }

    fn act(&mut self, a: Option<NavAction>) {
        if let Some(NavAction::Activated(i)) = a {
            self.last = format!("NavAction::Activated({i}) · {}", self.nav.items[i].label);
        }
    }

    fn toggle_collapsed(&mut self) {
        let on = !self.nav.collapsed;
        self.opts.set("sidebars.collapsed", usize::from(on));
        self.nav.collapsed = on;
    }

    fn set_hidden(&mut self, on: bool, cx: &mut Cx) {
        self.hidden = on;
        self.opts.set("sidebars.hidden", usize::from(on));
        self.last = if on {
            "RailAction::Collapse"
        } else {
            "RailAction::Expand"
        }
        .into();
        if !on {
            cx.focus.focus(self.nav.id);
        }
    }

    fn lines(&self) -> [String; 11] {
        let keys = if self.opts.value("sidebars.keys") == "desktop" {
            "One focus stop. ↑↓ move the cursor, Enter opens."
        } else {
            "One focus stop. ↑↓ (or j k) move the cursor, Enter opens."
        };
        [
            keys,
            "",
            "›  current item · persists when focus leaves",
            "▎  keyboard cursor · only while focused",
            "░  hover · follows the pointer",
            "",
            "Disabled items are skipped and ignore the pointer.",
            "Collapse keeps rows and markers, initials only.",
            "‹ on the frame or Ctrl+B hides the sidebar behind its rail.",
            "",
            &format!("Last: {}", self.last),
        ]
        .map(str::to_owned)
    }

    /// The label of the current item, or `None` when the list is empty.
    fn current_label(&self) -> Option<&str> {
        self.nav
            .items
            .get(self.nav.current)
            .map(|it| it.label.as_str())
    }

    fn card_height(&self, width: u16) -> u16 {
        let panel = Panel::card(self.current_label());
        let inner = panel.inner(Size::new(width, 0)).width.max(1) as usize;
        let rows: usize = self
            .lines()
            .iter()
            .map(|l| junie_tui::ui::text::wrap(l, inner).len().max(1))
            .sum();
        panel.outer(Size::new(0, rows as u16)).height
    }

    fn side_width(&self) -> u16 {
        if self.hidden {
            self.rail.width() + 1
        } else {
            self.nav.width() + 4 + 2
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let content_x = if self.hidden {
            let col = Rect::new(area.x, area.y, SidebarRail::WIDTH, area.height.min(20));
            self.rail.render_rail(col, buf, ctx, t.canvas);
            area.x + self.side_width()
        } else {
            let w = self.nav.width() + 4;
            let side = Rect::new(area.x, area.y, w, area.height.min(20));
            let panel = Panel::card(None);
            let bg = panel.bg(t);
            let inner = Rect::new(side.x, side.y + 1, side.width, side.height - 2);
            panel.render(side, buf, t);
            self.rail.render_collapse(side, buf, ctx, bg);
            self.nav.render(
                Rect::new(
                    inner.x,
                    inner.y,
                    inner.width,
                    inner.height.saturating_sub(2),
                ),
                buf,
                ctx,
                bg,
            );
            self.collapse.label = if self.nav.collapsed {
                "›".into()
            } else {
                "Collapse".into()
            };
            self.collapse.render(
                Rect::new(inner.x + 1, inner.bottom() - 1, inner.width, 1),
                buf,
                ctx,
                bg,
            );
            area.x + self.side_width()
        };

        let content = Rect::new(
            content_x,
            area.y,
            area.right().saturating_sub(content_x),
            area.height,
        );
        let current = self.current_label().map(str::to_owned);
        let panel = Panel::card(current.as_deref());
        let bg = panel.bg(t);
        let inner = panel.render(
            Rect::new(
                content.x,
                content.y,
                content.width,
                content.height.min(self.card_height(content.width).max(20)),
            ),
            buf,
            t,
        );
        let lines = self.lines();
        let mut y = inner.y;
        let mut clipped = false;
        for (i, l) in lines.iter().enumerate() {
            let accent = (2..=4).contains(&i);
            for (j, wl) in junie_tui::ui::text::wrap(l, inner.width as usize)
                .iter()
                .enumerate()
            {
                if y >= inner.bottom() {
                    clipped |= !wl.is_empty();
                    break;
                }
                if accent && j == 0 {
                    let (glyph, rest) =
                        wl.split_at(wl.chars().next().map(|c| c.len_utf8()).unwrap_or(0));
                    buf.set_string(inner.x, y, glyph, t.accent_fg().bg(bg));
                    buf.set_string(inner.x + 1, y, rest, t.secondary().bg(bg));
                } else {
                    buf.set_string(inner.x, y, wl, t.secondary().bg(bg));
                }
                y += 1;
            }
        }
        if clipped {
            panel.mark_clipped(inner, buf, t);
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let sample = || NavList::new(ID.sub("states-nav"), items(true, "full")).widths(22, 6);
    StateGrid::new(ID.sub("states"))
        .label_width(14)
        .column_width(24)
        .narrow_width(14)
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
        .column("Open", sample())
        .column("Collapsed", sample().collapsed(true))
        .column("Rail", SidebarRail::new(ID.sub("states-rail"), "Nav"))
}

const CARD_MIN: u16 = 24;

impl Page for SidebarsPage {
    fn title(&self) -> &'static str {
        "Sidebars"
    }
    fn blurb(&self) -> &'static str {
        "Sections, current item, focus cursor, hover, collapsed mode; text first, no icons"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        let card = avail.width.saturating_sub(self.side_width()).max(CARD_MIN);
        Size::new(self.side_width() + card, self.card_height(card))
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
            self.set_hidden(!self.hidden, cx);
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(self.nav.id) {
                    let (o, a) = self.nav.on_key(key);
                    self.act(a);
                    return o;
                }
                if cx.focus.is(self.collapse.id) {
                    let (o, act) = self.collapse.on_key(key);
                    if act {
                        self.toggle_collapsed();
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.rail.route(ev) {
                    let (o, actions) = r.into_parts();
                    if !actions.is_empty() {
                        for a in actions {
                            self.set_hidden(a == RailAction::Collapse, cx);
                        }
                        return Outcome::Changed;
                    }
                    return o;
                }
                if ev.on(self.nav.id) && ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                    cx.focus.focus(self.nav.id);
                }
                if let Some(r) = self.nav.route(ev) {
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.act(Some(a));
                    }
                    return o;
                }
                if let Some(r) = self.collapse.route(ev) {
                    if !ev.is(Phase::Click) {
                        return r.outcome();
                    }
                    if r.has_actions() {
                        self.toggle_collapsed();
                    }
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
        vec![("↑↓", "Move"), ("Enter", "Open"), ("Ctrl+B", "Sidebar")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.apply();
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

#[cfg(test)]
mod tests {
    use super::*;
    use junie_tui::core::event::Key;
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::Interaction;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    const SET: &str = "sidebars.items";

    fn key(code: KeyCode) -> PageEvent {
        PageEvent::Key(Key {
            code,
            mods: KeyModifiers::NONE,
        })
    }

    fn set(page: &mut SidebarsPage, value: usize) {
        page.set_option(SET, value);
    }

    fn screen(buf: &Buffer) -> String {
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    fn render(page: &mut SidebarsPage) -> Buffer {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        page.render(buf.area, &mut buf, &mut ctx);
        buf
    }

    /// Focuses the sidebar list and feeds it one page event.
    fn press(page: &mut SidebarsPage, ev: &PageEvent) -> Outcome {
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        focus.focus(page.nav.id);
        let mut cx = Cx::new(&mut focus, &ring);
        page.handle(ev, &mut cx)
    }

    #[test]
    fn default_item_set_is_the_full_playground() {
        let page = SidebarsPage::new();
        assert_eq!(page.nav.items.len(), ITEMS.len());
    }

    #[test]
    fn shrinking_under_the_cursor_repairs_it_before_the_next_key() {
        let mut page = SidebarsPage::new();
        press(&mut page, &key(KeyCode::End));
        assert_eq!(page.nav.cursor, 7);

        set(&mut page, 1);
        render(&mut page);
        assert_eq!(page.nav.items.len(), 3);
        assert_eq!(page.nav.cursor, 2, "last enabled row of the short set");

        let outcome = press(&mut page, &key(KeyCode::Up));
        assert_eq!(outcome, Outcome::Changed);
        press(&mut page, &key(KeyCode::Enter));
        assert_eq!(page.last, "NavAction::Activated(1) · Runs");
    }

    #[test]
    fn empty_item_set_renders_and_activates_nothing() {
        let mut page = SidebarsPage::new();
        set(&mut page, 2);

        assert!(page.nav.items.is_empty());
        let text = screen(&render(&mut page));
        assert!(!text.contains("Tasks"), "no row paints for an empty set");
        assert!(text.contains("Last: none yet"));
        press(&mut page, &key(KeyCode::Down));
        press(&mut page, &key(KeyCode::Enter));
        assert_eq!(page.last, "none yet");
    }

    #[test]
    fn all_disabled_item_set_renders_and_activates_nothing() {
        let mut page = SidebarsPage::new();
        set(&mut page, 3);

        assert!(page.nav.items.iter().all(|it| it.disabled));
        assert!(screen(&render(&mut page)).contains("Tasks"));
        for code in [KeyCode::Down, KeyCode::Up, KeyCode::End, KeyCode::Enter] {
            press(&mut page, &key(code));
        }
        assert_eq!(page.last, "none yet");
    }
}
