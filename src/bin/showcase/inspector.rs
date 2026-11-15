use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::app::PageId;
use crate::options::{Group, OptSet, OptionsView, SHELL, page_options};
use crate::pages::{Cx, Hint, PageEvent};
use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::scroll_view::{Axes, ScrollView};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::tabs::Tabs;

pub const ID: WidgetId = WidgetId::of("app.inspector");

pub const LOG_LEN: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    State,
    Options,
}

impl Tab {
    pub const ALL: [Self; 2] = [Self::State, Self::Options];
    pub fn label(self) -> &'static str {
        match self {
            Self::State => "State",
            Self::Options => "Options",
        }
    }
}

pub struct Inspector {
    pub tabs: Tabs,
    pub options: OptionsView,

    pub view: ScrollView,

    page: PageId,

    pub log: VecDeque<(String, String)>,

    pub inputs: u64,
    pub ticks: u64,
}

fn view(page: PageId) -> OptionsView {
    let heading: &'static str = page.entry().label;
    OptionsView::new(
        ID.sub("options"),
        &[
            Group {
                heading,
                opts: page_options(page),
                empty: "No page options yet",
            },
            Group {
                heading: "Shell",
                opts: SHELL,
                empty: "",
            },
        ],
    )
}

impl Inspector {
    pub fn new(page: PageId) -> Self {
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        Self {
            tabs: Tabs::new(ID.sub("tabs"), &labels),
            options: view(page),
            view: ScrollView::new(ID.sub("body")).axes(Axes::VERTICAL),
            page,
            log: VecDeque::new(),
            inputs: 0,
            ticks: 0,
        }
    }

    pub fn tab(&self) -> Tab {
        Tab::ALL[self.tabs.active.min(Tab::ALL.len() - 1)]
    }

    #[allow(dead_code)]
    pub fn set_tab(&mut self, tab: Tab) {
        let i = Tab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
        self.tabs.set_active(i);
        self.tabs.cursor = i;
        self.view.reset();
    }

    pub fn for_page(&mut self, page: PageId) {
        if self.page != page {
            self.page = page;
            self.options = view(page);
        }
    }

    pub fn record(&mut self, input: &Input, out: Outcome) {
        let what = match input {
            Input::Tick => {
                self.ticks += 1;
                return;
            }
            Input::Key(k) => format!(
                "Key {}",
                junie_tui::core::keymap::KeyBinding::new(k.code, k.mods).label()
            ),
            Input::Mouse(m) => {
                let mods = crate::app::describe_mods(m.mods);
                format!("{mods}{:?} {}·{}", m.kind, m.pos.x, m.pos.y)
            }
            Input::Resize(w, h) => format!("Resize {w}×{h}"),
            Input::Paste(text) => format!("Paste {} chars", text.chars().count()),
            Input::External(_) => "External message".to_owned(),
        };
        self.inputs += 1;
        self.log.push_front((what, format!("{out:?}")));
        self.log.truncate(LOG_LEN);
    }

    pub fn has_focus(&self, focus: Option<WidgetId>) -> bool {
        focus.is_some_and(|f| f == self.tabs.id || self.options.has_control(f))
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        state: &[Prop],
    ) {
        let strip = Rect::new(area.x, area.y, area.width, area.height.min(2));
        self.tabs.render(strip, buf, ctx, bg);
        let top = area.y.saturating_add(3).min(area.bottom());
        let body = Rect::new(area.x, top, area.width, area.bottom() - top);
        let avail = body.as_size();
        let log: Vec<Prop> = self
            .log
            .iter()
            .map(|(input, out)| Prop::new(out, input.clone()))
            .collect();
        let size = match self.tab() {
            Tab::State => {
                let w = avail.width.saturating_sub(2).max(1);
                let rows = props::measure(state, w) + 2 + props::measure(&log, w).max(1);
                ratatui::layout::Size::new(avail.width, rows)
            }
            Tab::Options => self.options.measure(avail),
        };
        let tab = self.tab();
        let options = &mut self.options;
        self.view
            .render(body, size, buf, ctx, |area, buf, ctx| match tab {
                Tab::State => render_state(area, buf, ctx, bg, state, &log),
                Tab::Options => options.render(area, buf, ctx, bg),
            });
    }

    pub fn on_blur(&mut self, from: Option<WidgetId>) -> Outcome {
        self.options.on_blur(from)
    }

    pub fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Option<(Outcome, Option<OptSet>)> {
        if let PageEvent::Pointer(p) = ev
            && let Some(r) = self.view.pointer(p)
        {
            return Some((r.outcome(), None));
        }
        let before = self.tabs.active;
        let out = self.handle_inner(ev, cx);
        if self.tabs.active != before {
            self.view.reset();
        }
        out
    }

    fn handle_inner(&mut self, ev: &PageEvent, cx: &mut Cx) -> Option<(Outcome, Option<OptSet>)> {
        match ev {
            PageEvent::Key(key) if cx.focus.current() == Some(self.tabs.id) => {
                return Some((self.tabs.on_key(key).0, None));
            }
            PageEvent::Pointer(p) => {
                if let Some(r) = self.tabs.route(p) {
                    if p.is(junie_tui::core::intent::Phase::Click) {
                        cx.focus.focus(self.tabs.id);
                    }
                    return Some((r.outcome(), None));
                }
            }
            _ => {}
        }
        if self.tab() == Tab::Options {
            return self.options.handle(ev, cx);
        }
        None
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Option<Vec<Hint>> {
        if focus == Some(self.tabs.id) {
            return Some(vec![("←→", "Switch tab")]);
        }
        self.options.hints(focus)
    }
}

fn render_state(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    bg: Color,
    state: &[Prop],
    log: &[Prop],
) {
    let t = ctx.theme;
    let used = props::render(area, buf, t, state, bg);
    let y = area.y + used + 1;
    if y + 1 >= area.bottom() {
        return;
    }
    buf.set_string(area.x, y, "Events", t.title().bg(bg));
    let rest = Rect::new(area.x, y + 1, area.width, area.bottom() - y - 1);
    if log.is_empty() {
        buf.set_string(rest.x, rest.y, "No input yet", t.faint().bg(bg));
    } else {
        props::render(rest, buf, t, log, bg);
    }
}
