use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, NavKeys, hint_key};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::scroll::{HScroll, ScrollbarVisibility, TrackClick};
use junie_tui::theme::Theme;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::{CardHeading, Panel, PanelCmd, PanelFocus, ScrollPanel};
use junie_tui::widgets::scrollbar;

const ID: WidgetId = WidgetId::of("panels");

const COL_MIN: u16 = 36;

const PANE_H: u16 = 9;

const NOTE_X: u16 = 32;
const TITLED: &str = "A card is a filled surface. Its title sits in the top-left and metadata on the right. It never has a border.";
const UNTITLED: &str = "Untitled card. Same surface, content starts at the padding edge.";
const NESTED_NOTE: &str =
    "A group inside a card is a muted label plus indent. The focus bar stays on the control.";

pub struct PanelsPage {
    template: Template,
    opts: PageValues,
    pub prose: ScrollPanel,
    pub log: ScrollPanel,
    pub nested: ListBox,
    sections: crate::pages::layout::Sections,
}

fn prose_style(t: &Theme, _line: &str) -> ratatui::style::Style {
    t.secondary()
}

pub fn log_style(t: &Theme, line: &str) -> ratatui::style::Style {
    if line.contains(" error ") {
        t.error_fg()
    } else if line.contains(" warn ") {
        t.primary().fg(t.warning)
    } else {
        t.secondary()
    }
}

impl PanelsPage {
    pub fn new() -> Self {
        let nested = ListBox::new(
            ID.sub("nested"),
            vec![
                ListItem::new("Local"),
                ListItem::new("CLI"),
                ListItem::new("Cloud").disabled(true),
            ],
            SelectMode::Single,
        );
        let mut page = Self {
            template: Template::new(ID, states_grid(None)),
            opts: PageValues::new(page_options(PageId::Panels)),
            prose: ScrollPanel::new(ID.sub("prose"), vec![]),
            log: ScrollPanel::new(ID.sub("log"), vec![]),
            nested,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let nav = match o.value("panels.nav") {
            "vim" => NavKeys::VIM,
            "no-paging" => NavKeys::DESKTOP.page(false),
            _ => NavKeys::default(),
        };
        let keymap = match o.value("panels.keymap") {
            "t" => PanelCmd::default_keymap()
                .into_iter()
                .map(|(k, c)| match c {
                    PanelCmd::Follow => (KeyBinding::char('t'), c),
                    _ => (k, c),
                })
                .collect(),
            _ => PanelCmd::default_keymap(),
        };
        let click = match o.value("panels.track_click") {
            "page" => Some(TrackClick::Page),
            "jump" => Some(TrackClick::Jump),
            _ => None,
        };
        let sideways = match o.value("panels.hscroll") {
            "off" => Some(HScroll::off()),
            "bar-always" => Some(HScroll::default().bar(ScrollbarVisibility::Always)),
            "bar-never" => Some(HScroll::default().bar(ScrollbarVisibility::Never)),
            "step-1" => Some(HScroll::default().step(1)),
            "jump" => Some(HScroll::default().track_click(TrackClick::Jump)),
            _ => None,
        };
        let text = crate::data::prose()
            .split('\n')
            .map(str::to_owned)
            .collect();
        let mut prose = ScrollPanel::new(ID.sub("prose"), text)
            .wrap(o.on("panels.wrap"))
            .nav(nav)
            .keymap(keymap.clone());
        let mut log = ScrollPanel::new(ID.sub("log"), crate::data::log_lines(60))
            .tail(o.on("panels.tail"))
            .nav(nav)
            .keymap(keymap);
        match o.value("panels.content") {
            "edited" => {
                log.replace_line(2, format!("{:7.2}s  info   Line edited in place", 0.74))
                    .expect("the generated log has a third line");
            }
            "appended" => {
                log.extend_lines((0..3).map(|i| {
                    format!(
                        "{:7.2}s  info   Line appended {}",
                        22.2 + f64::from(i),
                        i + 1
                    )
                }));
            }
            "replaced" => {
                log.set_lines(crate::data::log_lines(8));
            }
            "cleared" => {
                log.clear();
            }
            "long token" => {
                prose.push(
                    "Copy keeps the source text: 3f9c2a7be41d85c06f1a9e7d2b4c8a0f6e35d1b9c7a2f4e80d6b3a915c7e2f48"
                        .to_owned(),
                );
            }
            _ => {}
        }
        if let Some(c) = click {
            prose = prose.track_click(c);
            log = log.track_click(c);
        }
        if let Some(h) = sideways {
            log = log.horizontal(h);
        }
        if !o.on("panels.tail") {
            log.scroll.v.offset = self.log.scroll.offset;
        }
        prose.scroll.v.offset = self.prose.scroll.offset;
        self.prose = prose;
        self.log = log;
        self.template.grid = states_grid(self.heading());
    }

    fn heading(&self) -> Option<CardHeading> {
        CardHeading::ALL
            .into_iter()
            .find(|h| h.label() == self.opts.value("panels.heading"))
    }

    fn panels(&mut self) -> [&mut ScrollPanel; 2] {
        [&mut self.prose, &mut self.log]
    }

    fn card<'a>(&self, title: Option<&'a str>) -> Panel<'a> {
        let mut p = Panel::card(title)
            .pad_x(self.opts.number("panels.pad_x").unwrap_or(2))
            .title_min(self.opts.number("panels.title_min").unwrap_or(12));
        if self.opts.value("panels.focus_mark") == "bar" {
            p = p.focus_mark(PanelFocus::Bar);
        }
        if let Some(h) = self.heading() {
            p = p.heading_style(h);
        }
        p
    }

    fn split(width: u16) -> (u16, bool) {
        let half = (width / 2).saturating_sub(1);
        if half >= COL_MIN {
            (half, true)
        } else {
            (width, false)
        }
    }

    fn children(&self, lw: u16) -> [StackChild; 3] {
        let iw = |p: &Panel| p.inner(Size::new(lw, 0)).width as usize;
        let titled = self.card(Some("Titled card"));
        let untitled = self.card(None);
        let nested = self.card(Some("Nested"));
        let lines = |text: &str, w: usize| junie_tui::ui::text::wrap(text, w.max(1)).len() as u16;

        let nw = iw(&nested) as u16;
        let body = if Self::note_beside(nw) {
            lines(NESTED_NOTE, (nw - NOTE_X) as usize).max(4)
        } else {
            4 + 1 + lines(NESTED_NOTE, nw as usize)
        };
        [
            StackChild::fixed((lines(TITLED, iw(&titled)) + 3).max(7)),
            StackChild::fixed((lines(UNTITLED, iw(&untitled)) + 2).max(6)),
            StackChild::fixed((body + 3).max(7)),
        ]
    }

    fn note_beside(inner: u16) -> bool {
        inner > NOTE_X + 20
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (lw, side) = Self::split(area.width);
        let children = self.children(lw);
        let (l, r) = if side {
            crate::pages::layout::columns(area, lw, 2)
        } else {
            let lh = self.sections.stack.measure(&children).min(area.height);
            let below = (lh + 1).min(area.height);
            (
                Rect::new(area.x, area.y, area.width, lh),
                Rect::new(area.x, area.y + below, area.width, area.height - below),
            )
        };

        let focus = ctx.interaction.has_focus(self.nested.id).then_some(2);
        let lrows = self.sections.layout(l, &children, focus, buf, ctx);

        let panel = self.card(Some("Titled card")).meta("surface");
        let bg = panel.bg(t);
        if let Some(inner) = lrows.slot(0).and_then(|r| panel.try_render(r, buf, t)) {
            for (i, line) in junie_tui::ui::text::wrap(TITLED, inner.width as usize)
                .iter()
                .enumerate()
            {
                if i < inner.height as usize {
                    buf.set_string(inner.x, inner.y + i as u16, line, t.secondary().bg(bg));
                }
            }
        }

        let panel = self.card(None);
        let bg = panel.bg(t);
        if let Some(inner) = lrows.slot(1).and_then(|r| panel.try_render(r, buf, t)) {
            for (i, line) in junie_tui::ui::text::wrap(UNTITLED, inner.width as usize)
                .iter()
                .enumerate()
            {
                if i < inner.height as usize {
                    buf.set_string(inner.x, inner.y + i as u16, line, t.secondary().bg(bg));
                }
            }
        }

        let panel = self.card(Some("Nested"));
        let bg = panel.bg(t);
        match lrows.slot(2).and_then(|r| panel.try_render(r, buf, t)) {
            Some(inner) => self.render_nested(inner, buf, ctx, bg),
            None => ctx.focus_stop(self.nested.id),
        }
        self.render_panes(r, buf, ctx);
    }

    fn render_nested(
        &mut self,
        inner: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;

        buf.set_string(inner.x, inner.y, "Target", t.muted().bg(bg));
        let group = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width.min(NOTE_X - 2),
            inner.height.saturating_sub(1).min(3),
        );
        self.nested.render(group, buf, ctx, bg);

        let (note_x, note_y) = if Self::note_beside(inner.width) {
            (inner.x + NOTE_X, inner.y)
        } else {
            (inner.x, inner.y + 5)
        };
        for (i, line) in junie_tui::ui::text::wrap(NESTED_NOTE, (inner.right() - note_x) as usize)
            .iter()
            .enumerate()
        {
            let y = note_y + i as u16;
            if y < inner.bottom() {
                buf.set_string(note_x, y, line, t.muted().bg(bg));
            }
        }
    }

    fn render_panes(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let rrows = crate::pages::layout::rows(r, &[r.height / 2, 0]);
        let pf = ctx.interaction.focused(self.prose.id);
        let panel = Panel::framed(Some("Framed · split pane"))
            .focused(pf)
            .title_min(self.opts.number("panels.title_min").unwrap_or(12));
        let bg = panel.bg(t);
        let inner = panel.render(rrows[0], buf, t);
        self.prose.render(inner, buf, ctx, bg, prose_style);
        panel.draw_meta(
            rrows[0],
            buf,
            t,
            &scrollbar::position_label(&self.prose.scroll),
        );

        let lf = ctx.interaction.focused(self.log.id);
        let card = Rect::new(
            rrows[1].x,
            rrows[1].y + 1,
            rrows[1].width,
            rrows[1].height.saturating_sub(1),
        );
        let panel = self.card(Some("Card · scrollable")).focused(lf);
        let bg = panel.bg(t);
        let inner = panel.render(card, buf, t);
        self.log.render(inner, buf, ctx, bg, log_style);

        let pos = scrollbar::position_label(&self.log.scroll);

        let short = pos.split(" of ").next().unwrap_or_default().to_owned();
        let panel = if self.opts.value("panels.meta_short") == "custom" {
            panel.meta_short(&short)
        } else {
            panel
        };
        let meta = if self.log.follow {
            format!("{pos} · following")
        } else {
            pos.clone()
        };
        panel.draw_meta(card, buf, t, &meta);
    }
}

fn states_grid(heading: Option<CardHeading>) -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let card = |title| match heading {
        Some(h) => Panel::card(Some(title)).heading_style(h),
        None => Panel::card(Some(title)),
    };
    let label = match heading {
        Some(h) => format!("Card · {}", h.label()),
        _ => "Card".to_owned(),
    };
    StateGrid::new(id)
        .label_width(14)
        .column_width(24)
        .narrow_width(12)
        .rows(&[Rest, Focused, Overflow, Narrow])
        .column(&label, card("Settings").meta("3 items"))
        .column(
            "Card · bar mark",
            card("Log").focus_mark(PanelFocus::Bar).meta("live"),
        )
        .column("Framed", Panel::framed(Some("Preview")).meta("1–9 of 24"))
}

impl Page for PanelsPage {
    fn title(&self) -> &'static str {
        "Panels"
    }
    fn blurb(&self) -> &'static str {
        "Cards group; a frame only where a pane needs an edge; nothing boxed twice"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        let width = avail.width.max(COL_MIN);
        let (lw, side) = Self::split(width);
        let left = self.sections.stack.measure(&self.children(lw));
        let panes = 2 * PANE_H + 1;
        let height = if side {
            left.max(panes)
        } else {
            left + 1 + panes
        };
        Size::new(width, height)
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
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.nested.id {
                    return self.nested.on_key(key);
                }
                for p in self.panels() {
                    if p.id == f {
                        return p.on_key(key);
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.nested.route(ev) {
                    if ev.index_of(Part::ROW).is_some() && ev.is(Phase::Click) {
                        cx.focus.focus(self.nested.id);
                    }
                    return r.outcome();
                }
                for p in self.panels() {
                    if let Some(r) = p.route(ev) {
                        let o = r.outcome();
                        if o == Outcome::Ignored && ev.is(Phase::Click) && ev.id == p.id {
                            return Outcome::Changed;
                        }
                        return o;
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
        if focus == Some(self.nested.id) {
            return vec![("↑↓", "Move"), ("Enter", "Choose")];
        }
        let panel = if focus == Some(self.log.id) {
            &self.log
        } else {
            &self.prose
        };

        let mut v = Vec::new();
        if let Some(k) = panel.nav.move_hint(false) {
            v.push((k, "Scroll"));
        }
        if let Some(k) = panel.nav.page_hint() {
            v.push((k, "Page"));
        }
        if let Some(k) = panel.nav.ends_hint(false) {
            v.push((k, "Ends"));
        }
        if panel.tail
            && let Some(k) = hint_key(&panel.keymap, PanelCmd::Follow)
        {
            v.push((k, "Follow tail"));
        }
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
