use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::pointer::MouseTarget;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::button::Button;
use termrock::widgets::panel::Panel;
use termrock::widgets::progress::{
    BarLayout, Meter, MeterLevel, MeterTone, MeterVisual, ProgressBar, ProgressStatus, StaleStyle,
    Tally, render_spinner,
};

const ID: WidgetId = WidgetId::of("progress");

const JOBS: [(&str, Option<f64>, ProgressStatus); 7] = [
    ("Build image", Some(1.0), ProgressStatus::Done),
    ("Unit tests", Some(1.0), ProgressStatus::Done),
    ("Integration", Some(0.64), ProgressStatus::Error),
    ("Push to registry", Some(0.5), ProgressStatus::Active),
    ("Migrate schema", Some(0.3), ProgressStatus::Paused),
    ("Warm caches", None, ProgressStatus::Active),
    ("Notify", Some(0.0), ProgressStatus::Active),
];

const LIVE_META: &str = "ticks at 80 ms";
const JOBS_TITLE: &str = "Deploy checkout-service";
const JOBS_META: &str = "one BarLayout";
const METERS_META: &str = "capacity, not completion";

const METERS_NOTE: [&str; 2] = [
    "Capacity meters are never green: low ≤ 59 % blue, medium ≤ 84 % warning ▲, high error !.",
    "Stale keeps its level, dashed.",
];

const BAR_W: u16 = 70;

const METER_W: u16 = 34;

pub struct ProgressPage {
    template: Template,
    opts: PageValues,
    build: f64,
    running: bool,
    restart: Button,
    pause: Button,
    paused: bool,
    sections: crate::pages::layout::Sections,
}

impl ProgressPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Progress));
        Self {
            template: Template::new(ID, states_grid(&opts)),
            opts,
            build: 0.0,
            running: true,
            restart: Button::secondary(ID.sub("restart"), "Restart"),
            pause: Button::secondary(ID.sub("pause"), "Pause"),
            paused: false,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    fn layout(&self) -> BarLayout {
        let l = BarLayout::default().reserve_value(self.opts.on("progress.reserve_value"));
        match self.opts.value("progress.label_width") {
            "measured" => l,
            w => l.label_width(w.parse().unwrap_or(16)),
        }
    }

    fn stale_style(&self) -> StaleStyle {
        match self.opts.value("progress.stale_style") {
            "faint" => StaleStyle::Faint,
            _ => StaleStyle::Dashed,
        }
    }

    fn children() -> [StackChild; 3] {
        [
            StackChild::fixed(11),
            StackChild::fixed(3 + 2 + JOBS.len() as u16),
            StackChild::fixed(3 + METERS_NOTE.len() as u16 + 1 + 9),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let focus = [self.restart.id, self.pause.id]
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
            .then_some(0);
        let l = self
            .sections
            .layout(area, &Self::children(), focus, buf, ctx);
        match l.slot(0) {
            Some(r) => self.render_live(r, buf, ctx),
            None => {
                ctx.focus_stop(self.restart.id);
                ctx.focus_stop(self.pause.id);
            }
        }
        if let Some(r) = l.slot(1) {
            self.render_jobs(r, buf, ctx);
        }
        if let Some(r) = l.slot(2) {
            self.render_meters(r, buf, ctx);
        }
    }

    fn render_live(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Live")).meta(LIVE_META);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let w = inner.width.min(BAR_W);
        let status = if self.build >= 1.0 {
            ProgressStatus::Done
        } else if self.paused {
            ProgressStatus::Paused
        } else {
            ProgressStatus::Active
        };
        let layout = self.layout();
        ProgressBar::new("Building", self.build)
            .status(status)
            .layout(layout)
            .render(Rect::new(inner.x, inner.y, w, 1), buf, ctx, bg);
        ProgressBar::indeterminate("Resolving")
            .layout(layout)
            .render(Rect::new(inner.x, inner.y + 2, w, 1), buf, ctx, bg);
        render_spinner(
            Rect::new(inner.x, inner.y + 4, w, 1),
            buf,
            ctx,
            "Waiting for the test runner",
            bg,
        );
        let rects = termrock::widgets::button::row_layout(
            Rect::new(inner.x, inner.y + 6, inner.width, 1),
            &[self.restart.width(), self.pause.width()],
            2,
        );
        self.restart.render(rects[0], buf, ctx, bg);
        self.pause.render(rects[1], buf, ctx, bg);
    }

    fn render_jobs(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some(JOBS_TITLE)).meta(JOBS_META);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        Tally::of(JOBS.iter().map(|j| j.2)).render(inner, buf, ctx, bg);
        let layout = self.layout();
        let w = inner.width.min(BAR_W);
        for (i, (label, ratio, status)) in JOBS.iter().enumerate() {
            let y = inner.y + 2 + i as u16;
            if y >= inner.bottom() {
                break;
            }
            let bar = match ratio {
                Some(r) => ProgressBar::new(label, *r).status(*status),
                None => ProgressBar::indeterminate(label),
            };
            bar.layout(layout)
                .render(Rect::new(inner.x, y, w, 1), buf, ctx, bg);
        }
    }

    fn render_meters(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Meters")).meta(METERS_META);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        for (i, note) in METERS_NOTE.iter().enumerate() {
            buf.set_string(
                inner.x,
                inner.y + i as u16,
                termrock::ui::text::truncate(note, inner.width as usize),
                t.muted().bg(bg),
            );
        }
        let top = inner.y + METERS_NOTE.len() as u16 + 1;

        let visual = match self.opts.value("progress.meter_visual") {
            "block" => MeterVisual::Block,
            _ => MeterVisual::Line,
        };
        let meters: [(&str, Option<u8>, MeterTone); 9] = [
            ("Low       ", Some(38), MeterTone::Normal),
            ("Medium    ", Some(72), MeterTone::Normal),
            ("High      ", Some(91), MeterTone::Normal),
            ("Warning   ", Some(82), MeterTone::Warning),
            ("Exhausted ", Some(100), MeterTone::Exhausted),
            ("Stale     ", Some(76), MeterTone::Stale),
            ("Refreshing", Some(54), MeterTone::Refreshing),
            ("Error     ", None, MeterTone::Error),
            ("Unknown   ", None, MeterTone::Unknown),
        ];
        let disk = MeterTone::Level { warn: 80, crit: 95 };
        let options: [Meter; 6] = [
            Meter::new(Some(38))
                .value("38% used · block")
                .visual(MeterVisual::Block),
            Meter::new(Some(72))
                .value("72% disk, warns at 80")
                .tone(disk),
            Meter::new(Some(88))
                .value("88% disk, critical at 95")
                .tone(disk),
            Meter::new(Some(30))
                .value("30% · fixed high")
                .tone(MeterTone::Fixed(MeterLevel::High)),
            Meter::new(Some(82))
                .value("82% · stale warning")
                .tone(MeterTone::Warning)
                .stale(true),
            Meter::new(Some(76))
                .value("76% · faint stale style")
                .tone(MeterTone::Stale)
                .stale_style(StaleStyle::Faint),
        ];
        let col_w = inner.width.saturating_sub(12) / 2;
        for (i, (label, pct, tone)) in meters.iter().enumerate() {
            let y = top + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, label, t.primary().bg(bg));
            let value = match tone {
                MeterTone::Error => "quota read failed".to_owned(),
                MeterTone::Unknown => String::new(),
                _ => format!("{}% used", pct.unwrap_or(0)),
            };
            Meter::new(*pct)
                .value(value)
                .tone(*tone)
                .visual(visual)
                .stale_style(self.stale_style())
                .render(
                    Rect::new(inner.x + 11, y, col_w.min(METER_W), 1),
                    buf,
                    ctx,
                    bg,
                );
            if let Some(m) = options.get(i) {
                let x = inner.x + 11 + col_w + 1;
                m.render(Rect::new(x, y, col_w.min(METER_W), 1), buf, ctx, bg);
            }
        }
    }

    fn restart(&mut self) {
        self.build = 0.0;
        self.running = true;
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.pause.label = if self.paused { "Resume" } else { "Pause" }.into();
    }
}

fn states_grid(opts: &PageValues) -> StateGrid {
    use SpecimenState::*;
    let stale = match opts.value("progress.stale_style") {
        "faint" => StaleStyle::Faint,
        _ => StaleStyle::Dashed,
    };
    StateGrid::new(ID.sub("states"))
        .label_width(11)
        .column_width(17)
        .narrow_width(11)
        .rows(&[Rest, Empty, Loading, Error, Overflow, Narrow])
        .column("Bar active", ProgressBar::new("", 0.64))
        .column(
            "Bar done",
            ProgressBar::new("", 1.0).status(ProgressStatus::Done),
        )
        .column("Meter", Meter::new(Some(72)))
        .column(
            "Meter stale",
            Meter::new(Some(76)).stale(true).stale_style(stale),
        )
        .column(
            "Meter block",
            Meter::new(Some(38)).visual(MeterVisual::Block),
        )
}

impl Page for ProgressPage {
    fn title(&self) -> &'static str {
        "Progress"
    }
    fn blurb(&self) -> &'static str {
        "Determinate, indeterminate, an aggregate tally, capacity meters"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        let note = METERS_NOTE
            .iter()
            .map(|n| termrock::ui::text::width(n) as u16)
            .max()
            .unwrap_or(0);
        let width = [
            Panel::card(Some("Live"))
                .meta(LIVE_META)
                .outer(Size::new(BAR_W, 0)),
            Panel::card(Some(JOBS_TITLE))
                .meta(JOBS_META)
                .outer(Size::new(BAR_W, 0)),
            Panel::card(Some("Meters"))
                .meta(METERS_META)
                .outer(Size::new(note.max(12 + 2 * METER_W), 0)),
        ]
        .iter()
        .map(|s| s.width)
        .max()
        .unwrap_or(0);
        Size::new(width, self.sections.stack.measure(&Self::children()))
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
        if let PageEvent::Tick = ev {
            if self.running && !self.paused && self.build < 1.0 {
                self.build = (self.build + 0.006).min(1.0);
                if self.build >= 1.0 {
                    cx.status("Build finished ✓");
                }
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() == Section::Playground
            && let Some(o) = self.sections.handle(ev)
        {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(self.restart.id) {
                    let (o, act) = self.restart.on_key(key);
                    if act {
                        self.restart();
                    }
                    return o;
                }
                if cx.focus.is(self.pause.id) {
                    let (o, act) = self.pause.on_key(key);
                    if act {
                        self.toggle_pause();
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.restart.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.restart();
                    }
                    return Outcome::Changed;
                }
                if let Some(r) = self.pause.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.toggle_pause();
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        true
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Activate")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.template.grid = states_grid(&self.opts);
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
