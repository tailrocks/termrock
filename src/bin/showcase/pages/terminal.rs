use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, NavKeys};
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::{HScroll, TrackClick};
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::viewport::{Mark, Span, TextViewport, ViewportAction, ViewportCmd};

const ID: WidgetId = WidgetId::of("terminal");
const SEAM: WidgetId = WidgetId::of("terminal.seam");

const STAGES: [&str; 7] = [
    "Resolve workspace",
    "Pull base image",
    "Build container",
    "Mount sources",
    "Resolve credentials",
    "Start agent",
    "Ready",
];

const DURATIONS: [u32; 7] = [10, 26, 40, 8, 18, 14, 1];

pub struct TerminalPage {
    template: Template,
    opts: PageValues,
    term: TextViewport,
    rail: StepRail,
    split: Split,
    seam: Splitter,
    container: Rect,
    run: Button,
    fail: Button,
    tick: u32,
    stage: usize,
    stage_tick: u32,
    running: bool,
    fail_at: Option<usize>,
    copied: Option<String>,
}

fn stage_lines(stage: usize, tick: u32) -> Vec<Vec<Span>> {
    let mut v = vec![];
    if tick == 0 {
        v.push(vec![
            Span::new(format!("▶ {}", STAGES[stage]), Tone::Secondary).bold(),
        ]);
    }
    match stage {
        1 if tick.is_multiple_of(4) => v.push(vec![Span::muted(format!(
            "  layer {:02}/12  {:>3}%",
            (tick / 4 + 1).min(12),
            ((tick + 1) * 100 / DURATIONS[1]).min(100)
        ))]),
        2 if tick.is_multiple_of(3) => v.push(vec![
            Span::muted("  #"),
            Span::plain(format!("{} ", tick / 3 + 1)),
            Span::muted(match tick / 3 % 4 {
                0 => "RUN apt-get install -y build-essential",
                1 => "COPY rust-toolchain.toml ./",
                2 => "RUN cargo fetch --locked",
                _ => "RUN cargo build --release",
            }),
        ]),
        3 if tick == 2 => v.push(vec![Span::muted(
            "  ~/src/payments-platform → /workspace/payments-platform  rw",
        )]),
        3 if tick == 5 => v.push(vec![Span::muted(
            "  ~/src/shared-libs → /workspace/libs  ro",
        )]),
        4 if tick == 6 => v.push(vec![Span::muted(
            "  1Password  Engineering › Anthropic · Work › credential  ok",
        )]),
        5 if tick == 4 => v.push(vec![Span::muted("  claude --resume  pid 4188")]),
        _ => {}
    }
    v
}

impl TerminalPage {
    pub fn new() -> Self {
        let mut term = TextViewport::new(ID.sub("term")).max_lines(2000);
        term.push(vec![
            Span::plain("payments-platform ❯ ").bold(),
            Span::plain("jackin launch"),
        ]);
        let rail = StepRail::new(
            ID.sub("rail"),
            STAGES.iter().map(|s| Step::new(s)).collect(),
        );
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Terminal)),
            term,
            rail,
            split: Split::new(62, 24, 16),
            seam: Splitter::new(SEAM, SplitDir::Horizontal),
            container: Rect::ZERO,
            run: Button::primary(ID.sub("run"), "Run"),
            fail: Button::secondary(ID.sub("fail"), "Run with a failure"),
            tick: 0,
            stage: 0,
            stage_tick: 0,
            running: false,
            fail_at: None,
            copied: None,
        };
        p.reset(None);
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let nav = |key| match o.value(key) {
            "vim" => NavKeys::VIM,
            "no-page" => NavKeys::default().page(false),
            _ => NavKeys::default(),
        };
        let track = |key| match o.value(key) {
            "jump" => TrackClick::Jump,
            _ => TrackClick::Page,
        };
        let keymap = match o.value("terminal.keymap") {
            "custom" => vec![
                (KeyBinding::char('F'), ViewportCmd::ToggleFollow),
                (KeyBinding::char('c'), ViewportCmd::Copy),
                (KeyBinding::ctrl('a'), ViewportCmd::SelectAll),
            ],
            _ => ViewportCmd::default_keymap(),
        };
        let term = std::mem::replace(&mut self.term, TextViewport::new(ID.sub("term")));
        let mut term = term
            .wrap(o.on("terminal.wrap"))
            .max_lines(o.number("terminal.max_lines").unwrap_or(2000) as usize)
            .scrollbar_gap(o.number("terminal.scrollbar_gap").unwrap_or(1))
            .track_click(track("terminal.track_click"))
            .nav(nav("terminal.nav"))
            .keymap(keymap);
        match o.value("terminal.horizontal") {
            "on" => term = term.horizontal(HScroll::default()),
            "off" => term = term.horizontal(HScroll::off()),
            _ => {}
        }
        if o.on("terminal.mark") {
            let at = term.line_text(0).and_then(|l| l.find("jackin"));
            term.set_marks(
                at.map(|a| Mark {
                    line: 0,
                    range: a..a + 6,
                    current: true,
                })
                .into_iter()
                .collect(),
            );
        } else {
            term.clear_marks();
        }
        self.term = term;
        let rail = std::mem::replace(&mut self.rail, StepRail::new(ID.sub("rail"), vec![]));
        let mut rail = rail
            .selectable(o.on("terminal.selectable"))
            .nav(nav("terminal.rail_nav"))
            .track_click(track("terminal.track_click"));
        rail.numbered = o.on("terminal.numbered");
        self.rail = rail;
    }

    fn reset(&mut self, fail_at: Option<usize>) {
        self.tick = 0;
        self.stage = 0;
        self.stage_tick = 0;
        self.running = true;
        self.fail_at = fail_at;
        for i in 0..STAGES.len() {
            self.rail.set_state(i, StepState::Queued);
            self.rail.set_meta(i, None);
        }
        self.rail.set_state(0, StepState::Running);
        self.term.clear();
        self.term.push(vec![
            Span::plain("payments-platform ❯ ").bold(),
            Span::plain("jackin launch"),
        ]);
        self.term.push(vec![
            Span::muted("  profile "),
            Span::plain("dev").italic(),
            Span::muted(" · config "),
            Span::plain("jackin.toml").underline(),
            Span::muted(" · "),
            Span::new(" LIVE ", Tone::Secondary).reversed(),
        ]);
        self.term.set_follow(true);
    }

    fn advance(&mut self) -> Outcome {
        if !self.running {
            return Outcome::Ignored;
        }
        self.tick += 1;
        for line in stage_lines(self.stage, self.stage_tick) {
            self.term.push(line);
        }
        self.stage_tick += 1;
        self.rail.set_meta(
            self.stage,
            Some(format!("{:.1} s", self.stage_tick as f32 * 0.08)),
        );
        if self.fail_at == Some(self.stage) && self.stage_tick >= DURATIONS[self.stage] / 2 {
            self.rail.set_state(self.stage, StepState::Failed);
            self.rail.set_meta(self.stage, Some("exit 1".into()));
            for i in self.stage + 1..STAGES.len() {
                self.rail.set_state(i, StepState::Blocked);
            }
            self.term.push(vec![
                Span::new("✗ ", Tone::Error),
                Span::new(
                    format!(
                        "{} failed: network unreachable (curl: 6)",
                        STAGES[self.stage]
                    ),
                    Tone::Error,
                ),
            ]);
            self.running = false;
            return Outcome::Changed;
        }
        if self.stage_tick >= DURATIONS[self.stage] {
            self.rail.set_state(self.stage, StepState::Done);
            self.term.push(vec![
                Span::new("✓ ", Tone::Secondary),
                Span::muted(STAGES[self.stage]),
            ]);
            self.stage += 1;
            self.stage_tick = 0;
            if self.stage >= STAGES.len() {
                self.running = false;
                self.term
                    .push(vec![Span::plain("payments-platform ❯ ").bold()]);
            } else {
                if self.stage == 3 {
                    self.rail.set_state(self.stage, StepState::Skipped);
                    self.rail.set_meta(self.stage, Some("cached".into()));
                    self.stage += 1;
                }
                self.rail.set_state(self.stage, StepState::Running);
            }
        }
        Outcome::Changed
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let lines = || {
        vec![
            vec![
                Span::plain("❯ ").bold(),
                Span::plain("cargo build --release"),
            ],
            vec![Span::muted("   Compiling junie-tui v0.4.0")],
            vec![Span::new("✓ ", Tone::Secondary), Span::muted("Finished")],
        ]
    };
    let mut rail = StepRail::new(
        id.child(2),
        [
            ("Resolve", StepState::Done),
            ("Pull", StepState::Skipped),
            ("Build", StepState::Running),
            ("Mount", StepState::Failed),
            ("Start", StepState::Blocked),
            ("Ready", StepState::Queued),
        ]
        .iter()
        .map(|(l, _)| Step::new(l))
        .collect(),
    )
    .selectable(true);
    for (i, s) in [
        StepState::Done,
        StepState::Skipped,
        StepState::Running,
        StepState::Failed,
        StepState::Blocked,
        StepState::Queued,
    ]
    .into_iter()
    .enumerate()
    {
        rail.set_state(i, s);
    }
    StateGrid::new(id)
        .label_width(10)
        .column_width(30)
        .rows(&[Rest, Focused, Selected, Empty, Overflow, Narrow])
        .column("Viewport", TextViewport::with_lines(id.child(0), lines()))
        .column(
            "Wrapped",
            TextViewport::with_lines(id.child(1), lines()).wrap(true),
        )
        .column("Step rail", rail)
}

impl TerminalPage {
    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        self.container = area;
        let (left, right) = self.split.horizontal(area, 2);
        let handle = self.split.handle(SplitDir::Horizontal, area, 2);
        let depth = self.term.scrollback_depth();
        let meta = if self.term.has_selection() {
            "selection · y copies".to_owned()
        } else if self.term.is_at_tail() {
            format!("{} lines · following", self.term.len())
        } else {
            format!("scrollback ↑{depth}")
        };

        let meta = match self.term.retention() {
            Some(n) if self.term.len() >= n => format!("{meta} · last {n} kept"),
            _ => meta,
        };
        let panel = Panel::framed(Some("Viewport"))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.term.id));
        let bg = panel.bg(t);
        let inner = panel.render(left, buf, t);
        self.term.render(inner, buf, ctx, bg);
        self.seam.render(
            Rect::new(handle.x + 1, handle.y, 1, handle.height),
            buf,
            ctx,
            t.canvas,
        );

        let (done, skipped, failed) = self.rail.counts();
        let total = STAGES.len();
        let done = done + skipped;
        let meta = if failed > 0 {
            format!("{done} of {total} · failed")
        } else if self.running {
            format!("{done} of {total} · running")
        } else {
            format!("{done} of {total}")
        };
        let panel = Panel::card(Some("Step rail"))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.rail.id));
        let bg = panel.bg(t);
        let inner = panel.render(right, buf, t);
        let rail_area = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(3),
        );
        self.rail.render(rail_area, buf, ctx, bg);

        let widths = [self.run.width(), self.fail.width()];
        let lines = junie_tui::widgets::button::row_lines(inner.width, &widths, 2);
        let rects = row_layout(
            Rect::new(
                inner.x,
                inner.bottom().saturating_sub(lines),
                inner.width,
                1,
            ),
            &widths,
            2,
        );
        self.run.render(rects[0], buf, ctx, bg);
        self.fail.render(rects[1], buf, ctx, bg);

        let work = self.term.work_counters();
        let y = inner.bottom().saturating_sub(5);
        if y > rail_area.y + STAGES.len() as u16 {
            buf.set_string(
                inner.x,
                y,
                junie_tui::ui::text::truncate(
                    &format!(
                        "viewport work: {} lines segmented · {} reflowed",
                        work.segmented_lines, work.reflowed_lines
                    ),
                    inner.width as usize,
                ),
                t.faint().bg(bg),
            );
        }
        if let Some(c) = &self.copied {
            let y = inner.bottom().saturating_sub(3);
            buf.set_string(
                inner.x,
                y,
                junie_tui::ui::text::truncate(&format!("copied: {c}"), inner.width as usize),
                t.muted().bg(bg),
            );
        }
    }
}

impl TerminalPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if ev.id == SEAM {
            return match ev.phase() {
                Some(Phase::Press) => {
                    self.seam.on_press(&self.split, self.container, 2, ev.pos());
                    Outcome::Consumed
                }
                Some(Phase::Click) => self.seam.on_click(&mut self.split, ev.count()),
                Some(Phase::Drag) => {
                    self.seam
                        .on_drag(&mut self.split, self.container, 2, ev.pos())
                }
                _ => Outcome::Ignored,
            };
        }
        if let Some(r) = self.term.route(ev) {
            if ev.id == self.term.id && (ev.is(Phase::Press) || ev.is(Phase::Click)) {
                cx.focus.focus(self.term.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.rail.route(ev) {
            if ev.index_of(Part::ROW).is_some() && ev.is(Phase::Click) {
                cx.focus.focus(self.rail.id);
            }
            return r.outcome();
        }
        if self.run.route(ev).is_some_and(|r| r.has_actions()) {
            self.reset(None);
            return Outcome::Changed;
        }
        if self.fail.route(ev).is_some_and(|r| r.has_actions()) {
            self.reset(Some(1));
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}

impl Page for TerminalPage {
    fn title(&self) -> &'static str {
        "Terminal"
    }
    fn blurb(&self) -> &'static str {
        "Viewport with scrollback, selection and copy · drag the seam · step rail reports a job"
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
            return self.advance();
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(self.term.id) {
                    let (o, ev) = self.term.on_key(key);
                    match ev {
                        Some(ViewportAction::Copy(text)) => {
                            let n = text.lines().count();
                            self.copied = Some(text.lines().next().unwrap_or("").to_owned());
                            cx.status(format!("Copied {n} line{}", if n == 1 { "" } else { "s" }));
                            return Outcome::Changed;
                        }
                        Some(ViewportAction::FollowChanged(on)) => {
                            cx.status(if on {
                                "Following the tail"
                            } else {
                                "Paused at the scrollback position"
                            });
                            return Outcome::Changed;
                        }
                        _ => {}
                    }
                    return o;
                }
                if cx.focus.is(self.rail.id) {
                    return self.rail.on_key(key);
                }
                if cx.focus.is(self.run.id) {
                    let (o, act) = self.run.on_key(key);
                    if act {
                        self.reset(None);
                    }
                    return o;
                }
                if cx.focus.is(self.fail.id) {
                    let (o, act) = self.fail.on_key(key);
                    if act {
                        self.reset(Some(1));
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.running
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.term.id) {
            vec![
                ("↑↓", "Scroll"),
                ("←→", "Pan"),
                ("Home End", "Oldest / live"),
                ("f", "Follow"),
                ("drag", "Select"),
                ("y", "Copy"),
                ("Esc", "Clear"),
            ]
        } else if focus == Some(self.rail.id) {
            vec![("↑↓", "Move"), ("wheel", "Scroll")]
        } else {
            vec![("Enter", "Activate"), ("drag ┃", "Resize")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.rebuild();
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
