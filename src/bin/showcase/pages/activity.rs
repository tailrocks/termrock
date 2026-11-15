use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::feed::{Feed, FeedLine};
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{External, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::runtime::{
    ExternalPolicy, ExternalReceiver, ExternalSender, bounded_external_channel,
};
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::find_ranges;
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::chips::{Chip, ChipAction, ChipBar};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::spinner_frame;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::viewport::{Line, Mark, Span, TextViewport, ViewportAction};

const ID: WidgetId = WidgetId::of("activity");

const LOG_ROWS: u16 = 10;

pub const LOG: WidgetId = ID.sub("log");

pub const STREAMS: WidgetId = ID.sub("streams");

const SERVICES: [(&str, Tone); 4] = [
    ("api", Tone::Info),
    ("worker", Tone::Secondary),
    ("scheduler", Tone::Warning),
    ("db", Tone::Muted),
];

const CHATTER: [(usize, &str); 12] = [
    (0, "GET /health 200 2 ms"),
    (1, "job 4821 send-digest done in 184 ms"),
    (0, "POST /v1/orders 201 38 ms"),
    (3, "checkpoint complete: wrote 412 buffers (2.5%)"),
    (1, "job 4822 resize-avatar done in 61 ms"),
    (0, "GET /v1/orders?page=2 200 12 ms"),
    (2, "tick · 3 jobs due · next in 30 s"),
    (0, "GET /v1/customers/88 404 4 ms"),
    (1, "job 4823 send-digest retry 1 of 3: error SMTP 421"),
    (0, "PATCH /v1/orders/91 200 22 ms"),
    (3, "connection received: host=172.18.0.4 port=51424"),
    (1, "job 4823 send-digest done in 233 ms"),
];

const STAGES: [&str; 5] = [
    "Resolve tools",
    "Install dependencies",
    "Migrate database",
    "Start services",
    "Healthy",
];

const LINE_TICKS: u32 = 3;

const STOP_TICKS: u32 = 6;

const FEED_CAPACITY: usize = 32;

const FEED_BURST: usize = 96;

const FEED_PAUSE: std::time::Duration = std::time::Duration::from_millis(400);

const FEED_POLICY: ExternalPolicy = ExternalPolicy {
    max_messages: 16,
    max_duration: ExternalPolicy::DEFAULT_MAX_DURATION,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Run {
    Running,
    Stopping,
    Stopped,
    Failed,
}

pub fn page() -> Box<dyn Page> {
    Box::new(ActivityPage::new())
}

pub struct ActivityPage {
    template: Template,
    opts: PageValues,
    view: TextViewport,
    chips: ChipBar,
    rail: StepRail,
    stop: Button,
    restart: Button,

    log: Vec<(Option<usize>, Line)>,
    run: Run,

    tick: u32,

    stopping: u32,

    next: usize,
    exit: Option<i32>,

    feed_rx: ExternalReceiver,
    feed_tx: ExternalSender,
    feed: Option<Feed>,
    fed: u64,
}

fn service_line(svc: usize, text: &str) -> Line {
    let (name, tone) = SERVICES[svc];
    let body = if text.contains("error") || text.contains(" 404 ") {
        Span::new(text, Tone::Error)
    } else {
        Span::plain(text)
    };
    vec![Span::new(format!("{name:<9} │ "), tone), body]
}

fn holla_line(text: &str, tone: Tone) -> Line {
    vec![Span::new(text, tone)]
}

impl ActivityPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Activity));
        let mut chips = ChipBar::new(STREAMS);
        chips.add_label = None;
        chips.lead = Some("streams".into());
        chips.chips = SERVICES
            .iter()
            .map(|(name, _)| Chip {
                removable: false,
                ..Chip::new(name)
            })
            .collect();
        let capacity = std::num::NonZeroUsize::new(FEED_CAPACITY).expect("nonzero capacity");
        let (feed_tx, feed_rx) = bounded_external_channel(capacity, FEED_POLICY);
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            opts,
            view: TextViewport::new(LOG),
            chips,
            rail: StepRail::new(
                ID.sub("rail"),
                STAGES.iter().map(|s| Step::new(s)).collect(),
            ),
            stop: Button::secondary(ID.sub("stop"), "Stop"),
            restart: Button::secondary(ID.sub("restart"), "Restart"),
            log: vec![],
            run: Run::Running,
            tick: 0,
            stopping: 0,
            next: 0,
            exit: None,
            feed_rx,
            feed_tx,
            feed: None,
            fed: 0,
        };
        p.rebuild();
        p.start();
        p
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let view = std::mem::replace(&mut self.view, TextViewport::new(LOG));
        self.view = view
            .wrap(o.on("activity.wrap"))
            .max_lines(o.number("activity.retention").unwrap_or(2000) as usize);
        self.chips = std::mem::replace(&mut self.chips, ChipBar::new(STREAMS))
            .digit_toggles(o.on("activity.digit_toggles"));
        self.sync_view();
    }

    fn start(&mut self) {
        self.log.clear();
        self.tick = 0;
        self.next = 0;
        self.stopping = 0;
        self.exit = None;
        self.push(None, holla_line("$ mise run dev", Tone::Secondary));
        self.push(
            None,
            holla_line(
                "resolved node 22.9.0 · pnpm 9.12 · postgres 16",
                Tone::Muted,
            ),
        );

        for _ in 0..CHATTER.len() * 2 {
            self.chatter();
        }
        let state = match self.opts.value("activity.state") {
            "stopped" => Run::Stopped,
            "failed" => Run::Failed,
            _ => Run::Running,
        };
        self.run = Run::Running;
        match state {
            Run::Stopped => {
                self.push(
                    None,
                    holla_line(
                        "stop requested by holla · SIGTERM to the process group",
                        Tone::Muted,
                    ),
                );
                self.finish(Run::Stopped, 143);
            }
            Run::Failed => {
                self.push(
                    Some(0),
                    service_line(
                        0,
                        "error: listen EADDRINUSE: address already in use :::8080",
                    ),
                );
                self.finish(Run::Failed, 1);
            }
            _ => {}
        }
        self.sync_rail();
        self.view.set_follow(true);
    }

    fn push(&mut self, svc: Option<usize>, line: Line) {
        self.log.push((svc, line.clone()));

        let keep = self.view.retention().unwrap_or(usize::MAX).max(1) * 2;
        if self.log.len() > keep {
            self.log.drain(..self.log.len() - keep);
        }
        if svc.is_none_or(|s| self.chips.chips[s].enabled) {
            self.view.push(line);
            self.mark();
        }
    }

    fn chatter(&mut self) {
        let (svc, text) = CHATTER[self.next % CHATTER.len()];
        self.next += 1;
        self.push(Some(svc), service_line(svc, text));
    }

    fn fed_line(&mut self, seq: u64) {
        let (svc, text) = CHATTER[self.next % CHATTER.len()];
        self.next += 1;
        self.fed += 1;
        self.push(
            Some(svc),
            service_line(svc, &format!("{text} · feed #{seq}")),
        );
    }

    fn set_feed(&mut self, on: bool) {
        if on == self.feed.is_some() {
            return;
        }
        self.fed = 0;
        self.feed = on.then(|| Feed::start(self.feed_tx.clone(), FEED_BURST, FEED_PAUSE));
    }

    fn finish(&mut self, run: Run, code: i32) {
        self.run = run;
        self.exit = Some(code);
        let (text, tone) = match run {
            Run::Failed => (
                format!("api exited with code {code} · 3 services stopped"),
                Tone::Error,
            ),
            _ => (
                format!("stopped · exit {code} · descendants reaped"),
                Tone::Muted,
            ),
        };
        self.push(None, holla_line(&text, tone));
        self.sync_rail();
    }

    fn sync_view(&mut self) {
        let lines = self
            .log
            .iter()
            .filter(|(s, _)| s.is_none_or(|s| self.chips.chips[s].enabled))
            .map(|(_, l)| l.clone())
            .collect();
        let follow = self.view.is_at_tail();
        self.view.set_lines(lines);
        self.view.set_follow(follow);
        self.mark();
    }

    fn mark(&mut self) {
        let q = match self.opts.value("activity.find") {
            "off" => {
                self.view.clear_marks();
                return;
            }
            q => q,
        };
        let marks: Vec<Mark> = (0..self.view.len())
            .flat_map(|i| {
                let text = self.view.line_text(i).unwrap_or_default().to_owned();
                find_ranges(&text, q, false)
                    .into_iter()
                    .map(move |range| Mark {
                        line: i,
                        range,
                        current: false,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut marks = marks;
        if let Some(m) = marks.last_mut() {
            m.current = true;
        }
        self.view.set_marks(marks);
    }

    fn sync_rail(&mut self) {
        let states: [StepState; 5] = match self.run {
            Run::Running => [
                StepState::Done,
                StepState::Done,
                StepState::Done,
                StepState::Done,
                StepState::Running,
            ],
            Run::Stopping => [
                StepState::Done,
                StepState::Done,
                StepState::Done,
                StepState::Running,
                StepState::Blocked,
            ],
            Run::Stopped => [
                StepState::Done,
                StepState::Done,
                StepState::Skipped,
                StepState::Done,
                StepState::Done,
            ],
            Run::Failed => [
                StepState::Done,
                StepState::Done,
                StepState::Done,
                StepState::Failed,
                StepState::Blocked,
            ],
        };
        for (i, s) in states.into_iter().enumerate() {
            self.rail.set_state(i, s);
        }
        self.rail.set_meta(0, Some("0.4 s".into()));
        self.rail.set_meta(1, Some("6.1 s".into()));
        self.rail.set_meta(
            2,
            Some(if self.run == Run::Stopped {
                "up to date".into()
            } else {
                "2 migrations".into()
            }),
        );
        self.rail.set_meta(
            3,
            Some(match self.run {
                Run::Failed => "exit 1".into(),
                Run::Stopping => "stopping".into(),
                _ => "4 services".into(),
            }),
        );
        self.rail.set_meta(4, None);
    }

    fn elapsed(&self) -> String {
        let s = 42 + self.tick / 10;
        format!("{} min {:02} s", 12 + s / 60, s % 60)
    }

    fn header(&self) -> (Vec<Segment>, Vec<Segment>) {
        let spin = spinner_frame(self.tick as u64);
        let state = match self.run {
            Run::Running => Segment::new(format!("running · {}", self.elapsed()), Tone::Secondary)
                .lead(spin, Tone::Active),
            Run::Stopping => {
                Segment::new("stopping · SIGTERM sent", Tone::Warning).lead(spin, Tone::Warning)
            }
            Run::Stopped => Segment::new(
                format!("stopped · exit {}", self.exit.unwrap_or(0)),
                Tone::Muted,
            )
            .lead("○", Tone::Muted),
            Run::Failed => Segment::new(
                format!("failed · exit {}", self.exit.unwrap_or(1)),
                Tone::Error,
            )
            .lead("!", Tone::Error),
        };
        let shown = self.chips.chips.iter().filter(|c| c.enabled).count();
        (
            vec![
                Segment::new("api dev", Tone::Normal).bold().priority(9),
                state.priority(8),
                Segment::new(format!("{shown} of 4 streams"), Tone::Muted).priority(2),
            ],
            vec![Segment::new("here · ~/work/acme · mbp", Tone::Muted).priority(3)],
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let rows = crate::pages::layout::rows(area, &[1, 1, 1, 1, 1, area.height]);
        let (left, right) = self.header();
        segments::render(rows[0], buf, ctx, &left, &right, t.canvas);
        segments::render(
            rows[1],
            buf,
            ctx,
            &[Segment::new(
                "$ mise run dev · started by Run api dev · 4 services",
                Tone::Muted,
            )],
            &[],
            t.canvas,
        );
        self.chips.render(rows[3], buf, ctx, t.canvas);
        let body = rows[5];
        let side = self.side_size(ctx);
        if self.wide(body.width, side, ctx) {
            let side_w = side.width.max(36.min(body.width / 3));
            let out = Rect::new(body.x, body.y, body.width - side_w - 2, body.height);
            self.render_log(out, buf, ctx);
            let r = Rect::new(body.right() - side_w, body.y, side_w, body.height);
            self.render_side(r, false, buf, ctx);
        } else {
            let (w, h, beside) = self.under(body.width, side);
            let log_h = body.height.saturating_sub(h + 1).max(LOG_ROWS);
            let out = Rect::new(body.x, body.y, body.width, log_h);
            self.render_log(out, buf, ctx);
            let y = out.bottom() + 1;
            let r = Rect::new(
                body.x,
                y,
                w.min(body.width),
                body.bottom().saturating_sub(y),
            );
            self.render_side(r, beside, buf, ctx);
        }
    }

    fn log_width(&self, ctx: &RenderCtx) -> u16 {
        Panel::framed(Some("output"))
            .meta("scrollback ↑9999 · last 2000 kept")
            .outer(self.view.measure(Size::ZERO, ctx))
            .width
    }

    fn wide(&self, width: u16, side: Size, ctx: &RenderCtx) -> bool {
        width >= self.log_width(ctx) + 2 + side.width
    }

    fn run_facts(&self) -> Vec<Prop> {
        let mut facts = vec![
            Prop::new("Command", "mise run dev"),
            Prop::new("Scope", "here · ~/work/acme"),
            Prop::new("Host", "mbp · local"),
            Prop::new("Exit", self.exit.map_or("—".into(), |c| c.to_string())).tone(
                if self.run == Run::Failed {
                    Tone::Error
                } else {
                    Tone::Normal
                },
            ),
        ];
        if let Some(feed) = &self.feed {
            facts.extend([
                Prop::new("Feed sent", feed.sent().to_string()),
                Prop::new("Handled", self.fed.to_string()),
                Prop::new("Full retries", feed.full().to_string()).tone(Tone::Warning),
            ]);
        }
        facts
    }

    fn failure(&self) -> Option<Notice> {
        (self.run == Run::Failed).then(|| {
            Notice::error("api exited with code 1")
                .detail("listen EADDRINUSE :::8080")
                .hint("Free the port, then Restart")
        })
    }

    fn side_size(&self, ctx: &RenderCtx) -> Size {
        let steps = Panel::card(Some("Steps"))
            .meta("4 of 5")
            .outer(self.rail.measure(Size::ZERO, ctx));
        let facts = self.run_facts();
        let run = Panel::card(Some("Run"));
        let buttons = self.stop.width() + 2 + self.restart.width();
        let width = steps.width.max(
            run.outer(Size::new(props::width(&facts).max(buttons), 0))
                .width,
        );
        let inner = run.inner(Size::new(width, 0)).width;
        let mut h = props::measure(&facts, inner) + 1;
        if let Some(n) = self.failure() {
            h += n.height(inner) + 1;
        }
        let run_h = run.outer(Size::new(inner, h + 1)).height;
        Size::new(width, steps.height + 1 + run_h)
    }

    fn under(&self, width: u16, side: Size) -> (u16, u16, bool) {
        let steps_h = STAGES.len() as u16 + 3;
        let run_h = side.height.saturating_sub(steps_h + 1);
        if width >= side.width * 2 + 2 {
            (side.width * 2 + 2, steps_h.max(run_h), true)
        } else {
            (side.width, side.height, false)
        }
    }

    fn render_log(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let marks = self.view.marks().len();
        let mut meta = if marks > 0 {
            format!("{marks} matches for “{}”", self.opts.value("activity.find"))
        } else if self.view.has_selection() {
            "selection · y copies".into()
        } else if self.view.is_at_tail() {
            "following".into()
        } else {
            format!("scrollback ↑{}", self.view.scrollback_depth())
        };
        if let Some(n) = self.view.retention()
            && self.view.len() >= n
        {
            meta = format!("{meta} · last {n} kept");
        }
        let panel = Panel::framed(Some("output"))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.view.id));
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.view.render(inner, buf, ctx, bg);
    }

    fn render_side(&mut self, area: Rect, beside: bool, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (done, skipped, failed) = self.rail.counts();
        let meta = if failed > 0 {
            "failed".to_owned()
        } else {
            format!("{} of {}", done + skipped, STAGES.len())
        };
        let panel = Panel::card(Some("Steps"))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.rail.id));
        let bg = panel.bg(t);
        let steps_h = (STAGES.len() as u16 + 3).min(area.height);

        let (card, rest) = if beside {
            let w = area.width.saturating_sub(2) / 2;
            (
                Rect::new(area.x, area.y, w, steps_h),
                Rect::new(area.x + w + 2, area.y, w, area.height),
            )
        } else {
            let card = Rect::new(area.x, area.y, area.width, steps_h);
            let rest = Rect::new(
                area.x,
                card.bottom() + 1,
                area.width,
                area.bottom().saturating_sub(card.bottom() + 1),
            );
            (card, rest)
        };
        let inner = panel.render(card, buf, t);
        self.rail.render(inner, buf, ctx, bg);

        if rest.height < 4 {
            ctx.focus_stop(self.stop.id);
            ctx.focus_stop(self.restart.id);
            return;
        }
        let panel = Panel::card(Some("Run"));
        let bg = panel.bg(t);
        let inner = panel.render(rest, buf, t);
        let facts = self.run_facts();
        let h = props::render(inner, buf, t, &facts, bg);
        let mut y = inner.y + h + 1;
        if let Some(n) = self.failure() {
            let nh = n.height(inner.width);
            if y + nh + 2 <= inner.bottom() {
                n.render(Rect::new(inner.x, y, inner.width, nh), buf, ctx, bg);
                y += nh + 1;
            }
        }
        let y = y.min(inner.bottom().saturating_sub(1));
        self.render_buttons(Rect::new(inner.x, y, inner.width, 1), buf, ctx, bg);
    }

    fn render_buttons(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        self.stop.busy = self.run == Run::Stopping;
        self.stop.disabled = matches!(self.run, Run::Stopped | Run::Failed);
        self.restart.disabled = self.run == Run::Stopping;
        let rects = row_layout(area, &[self.stop.width(), self.restart.width()], 2);
        self.stop.render(rects[0], buf, ctx, bg);
        self.restart.render(rects[1], buf, ctx, bg);
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.run != Run::Running {
            return;
        }
        self.run = Run::Stopping;
        self.stopping = STOP_TICKS;
        self.push(
            None,
            holla_line(
                "stop requested by holla · SIGTERM to the process group",
                Tone::Muted,
            ),
        );
        self.sync_rail();
        cx.status("Stopping api dev…");
    }

    fn restart(&mut self, cx: &mut Cx) {
        if self.run == Run::Stopping {
            return;
        }

        self.opts.set("activity.state", 0);
        self.start();
        cx.status("Restarted api dev");
    }

    fn on_chips(&mut self, a: Option<ChipAction>, cx: &mut Cx) {
        if let Some(ChipAction::Toggle(i) | ChipAction::Activate(i)) = a
            && let Some(c) = self.chips.chips.get_mut(i)
        {
            c.enabled = !c.enabled;
            let (name, on) = (c.label.clone(), c.enabled);
            self.sync_view();
            cx.status(format!("{name} {}", if on { "shown" } else { "hidden" }));
        }
    }

    fn on_view(&mut self, a: Option<ViewportAction>, cx: &mut Cx) {
        if let Some(ViewportAction::Copy(text)) = a {
            cx.status(format!("Copied {} lines", text.lines().count()));
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let lines = vec![
        service_line(0, "GET /health 200 2 ms"),
        service_line(1, "job 4821 done in 184 ms"),
        service_line(0, "GET /v1/c/88 404 4 ms"),
    ];
    let mut chips = ChipBar::new(id.child(1));
    chips.add_label = None;
    chips.chips = ["api", "worker", "db"]
        .iter()
        .map(|n| Chip {
            removable: false,
            ..Chip::new(n)
        })
        .collect();
    chips.chips[1].enabled = false;
    let mut rail = StepRail::new(
        id.child(2),
        ["Resolve", "Install", "Start", "Healthy"]
            .iter()
            .map(|s| Step::new(s))
            .collect(),
    );
    for (i, s) in [
        StepState::Done,
        StepState::Done,
        StepState::Failed,
        StepState::Blocked,
    ]
    .into_iter()
    .enumerate()
    {
        rail.set_state(i, s);
    }
    StateGrid::new(id)
        .label_width(10)
        .column_width(28)
        .rows(&[Rest, Focused, Selected, Empty, Overflow, Narrow])
        .column("Log", TextViewport::with_lines(id.child(0), lines))
        .column("Streams", chips)
        .column("Steps", rail)
}

impl Page for ActivityPage {
    fn title(&self) -> &'static str {
        "Activity"
    }
    fn blurb(&self) -> &'static str {
        "A live job: state clauses, stream chips, a following log and its steps"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let side = self.side_size(ctx);
        let head = 5;
        if self.wide(avail.width, side, ctx) {
            return Size::new(avail.width, avail.height.max(head + side.height));
        }
        let width = avail.width.max(self.log_width(ctx)).max(side.width);
        let (_, h, _) = self.under(width, side);
        let log_h = avail.height.saturating_sub(head + h + 1).max(LOG_ROWS);
        Size::new(width, head + log_h + 1 + h)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn animating(&self) -> bool {
        matches!(self.run, Run::Running | Run::Stopping)
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            match self.run {
                Run::Running => {
                    self.tick += 1;
                    if self.feed.is_none() && self.tick.is_multiple_of(LINE_TICKS) {
                        self.chatter();
                    }
                }
                Run::Stopping => {
                    self.tick += 1;
                    self.stopping = self.stopping.saturating_sub(1);
                    if self.stopping == 0 {
                        self.finish(Run::Stopped, 143);
                        cx.status("api dev stopped · exit 143");
                    }
                }
                _ => return Outcome::Ignored,
            }
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
                if cx.focus.is(self.view.id) {
                    let (o, a) = self.view.on_key(key);
                    self.on_view(a, cx);
                    return o;
                }
                if cx.focus.is(self.chips.id) {
                    let (o, a) = self.chips.on_key(key);
                    self.on_chips(a, cx);
                    return o;
                }
                if cx.focus.is(self.rail.id) {
                    return self.rail.on_key(key);
                }
                if cx.focus.is(self.stop.id) {
                    let (o, act) = self.stop.on_key(key);
                    if act {
                        self.stop(cx);
                    }
                    return o;
                }
                if cx.focus.is(self.restart.id) {
                    let (o, act) = self.restart.on_key(key);
                    if act {
                        self.restart(cx);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.chips.route(ev) {
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.on_chips(Some(a), cx);
                    }
                    return o;
                }
                if let Some(r) = self.view.route(ev) {
                    return r.outcome();
                }
                if let Some(r) = self.rail.route(ev) {
                    return r.outcome();
                }
                if let Some(r) = self.stop.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.stop(cx);
                    }
                    return Outcome::Changed;
                }
                if let Some(r) = self.restart.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.restart(cx);
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
        if focus == Some(self.view.id) {
            return vec![
                ("↑↓", "Scroll"),
                ("f", "Follow"),
                ("End", "Tail"),
                ("y", "Copy"),
            ];
        }
        if focus == Some(self.chips.id) {
            let mut v = vec![("←→", "Move"), ("Space", "Show / hide")];
            if self.chips.digit_toggles {
                v.push(("1–4", "Toggle stream"));
            }
            return v;
        }
        vec![("Enter", "Activate")]
    }

    fn external(&self) -> Option<&ExternalReceiver> {
        self.feed.as_ref().map(|_| &self.feed_rx)
    }

    fn on_external(&mut self, message: &External, _cx: &mut Cx) -> Outcome {
        match message.downcast_ref::<FeedLine>() {
            Some(line) if self.feed.is_some() && self.run == Run::Running => {
                self.fed_line(line.seq);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.rebuild();
            if key == "activity.state" {
                self.start();
            }
            if key == "activity.feed" {
                let channel = self.opts.value("activity.feed") == "channel";
                self.set_feed(channel);
            }
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
