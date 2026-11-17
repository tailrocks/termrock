use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::{Part, WidgetId};
use termrock::core::intent::Phase;
use termrock::core::pointer::MouseTarget;
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::button::{Button, ButtonRow};
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop, PropsAction, PropsList};
use termrock::widgets::steps::{Step, StepRail, StepState};
use termrock::widgets::viewport::{Span, TextViewport, ViewportAction, line_text};

const RAIL_W: u16 = 30;

const LOG_MIN: u16 = 48;
const ID: WidgetId = WidgetId::of("launchcockpit");
const FACTS: WidgetId = ID.sub("facts");
const RAIL: WidgetId = ID.sub("rail");
const LOG: WidgetId = ID.sub("log");
const LAUNCH: WidgetId = ID.sub("launch");
const CANCEL: WidgetId = ID.sub("cancel");
const CANCEL_DLG: WidgetId = ID.sub("cancel-dialog");
const CRED_DLG: WidgetId = ID.sub("cred-dialog");
const FAIL_DLG: WidgetId = ID.sub("fail-dialog");

const STAGES: [(&str, u32); 11] = [
    ("Identity", 3),
    ("Role", 3),
    ("Credentials", 5),
    ("Construct", 4),
    ("Agent Binaries", 5),
    ("Derived Image", 10),
    ("Workspace", 4),
    ("Network", 3),
    ("Sidecar", 4),
    ("Capsule", 5),
    ("Hardline", 2),
];
const CREDENTIALS: usize = 2;
const DERIVED_IMAGE: usize = 5;
const RUN_ID: &str = "run-7f3a91c2";

fn activity(stage: usize) -> &'static str {
    match stage {
        0 => "Resolving identity…",
        1 => "Loading role backend…",
        2 => "Resolving credentials…",
        3 => "Constructing the container…",
        4 => "Installing agent binaries…",
        5 => "Building the derived image…",
        6 => "Mounting the workspace…",
        7 => "Joining the network…",
        8 => "Starting the sidecar…",
        9 => "Starting the capsule…",
        _ => "Opening the hardline…",
    }
}

fn stage_log(stage: usize, tick: u32) -> Option<Vec<Span>> {
    let muted = |s: &str| Some(vec![Span::muted(format!("  {s}"))]);
    match (stage, tick) {
        (0, 1) => muted("identity alexey@build-07 · uid 501"),
        (1, 1) => muted("role backend · jackin.toml (workspace)"),
        (2, 2) => muted("1Password  Engineering › Anthropic · Work › credential"),
        (3, 2) => muted("base image ghcr.io/jackin/base:24.04 (cached)"),
        (4, 1) => muted("claude 1.0.71 · node 22.11.0"),
        (5, t) if t.is_multiple_of(2) => Some(vec![
            Span::muted("  #"),
            Span::plain(format!("{} ", t / 2 + 1)),
            Span::muted(match t / 2 % 5 {
                0 => "FROM ghcr.io/jackin/base:24.04",
                1 => "RUN apt-get install -y build-essential",
                2 => "COPY rust-toolchain.toml ./",
                3 => "RUN cargo fetch --locked",
                _ => "RUN cargo build --release",
            }),
        ]),
        (6, 1) => muted("~/src/payments-platform → /workspace  rw"),
        (7, 1) => muted("network jackin-payments · 172.28.0.4"),
        (8, 2) => muted("sidecar postgres:16 · healthy"),
        (9, 2) => muted("capsule cap-payments-01 · pid 4188"),
        _ => None,
    }
}

pub fn page() -> Box<dyn Page> {
    Box::new(LaunchCockpitPage::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Idle,
    Running,

    Held,
    Ready,
    Failed,
    Cancelled,
}

pub struct LaunchCockpitPage {
    template: Template,
    opts: PageValues,
    facts: PropsList,
    rail: StepRail,
    log: TextViewport,
    launch: Button,
    cancel: Button,
    run: Run,
    stage: usize,
    stage_tick: u32,

    cred_failed: bool,
    sections: crate::pages::layout::Sections,
}

impl LaunchCockpitPage {
    pub fn new() -> Self {
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::LaunchCockpit)),
            facts: PropsList::new(FACTS, facts(Run::Idle, false)),
            rail: StepRail::new(RAIL, STAGES.iter().map(|(s, _)| Step::new(s)).collect()),
            log: TextViewport::new(LOG).max_lines(2000),
            launch: Button::primary(LAUNCH, "Launch"),
            cancel: Button::secondary(CANCEL, "Cancel launch…"),
            run: Run::Idle,
            stage: 0,
            stage_tick: 0,
            cred_failed: false,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        p.log.push(vec![Span::muted(
            "Ready. Launch builds and starts the capsule for payments-platform.",
        )]);
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        let rail = std::mem::replace(&mut self.rail, StepRail::new(RAIL, vec![]));
        let mut rail = rail.selectable(self.opts.on("launchcockpit.selectable"));
        rail.numbered = self.opts.on("launchcockpit.numbered");
        self.rail = rail;
        let log = std::mem::replace(&mut self.log, TextViewport::new(LOG));
        self.log = log.wrap(self.opts.on("launchcockpit.wrap"));
    }

    fn running(&self) -> bool {
        matches!(self.run, Run::Running | Run::Held)
    }

    fn start(&mut self, cx: &mut Cx) {
        for i in 0..STAGES.len() {
            self.rail.set_state(i, StepState::Queued);
            self.rail.set_meta(i, None);
        }
        self.rail.set_state(0, StepState::Running);
        self.stage = 0;
        self.stage_tick = 0;
        self.cred_failed = false;
        self.run = Run::Running;
        self.log.clear();
        self.log.push(vec![
            Span::plain("payments-platform ❯ ").bold(),
            Span::plain("jackin launch backend"),
        ]);
        self.log.set_follow(true);
        self.facts.set_props(facts(self.run, false));
        cx.status("Launching payments-platform…");
    }

    fn stop(&mut self, run: Run, state: StepState) {
        self.run = run;
        self.rail.set_state(self.stage, state);
        for i in self.stage + 1..STAGES.len() {
            self.rail.set_state(i, StepState::Blocked);
        }
    }

    fn cancel_now(&mut self, cx: &mut Cx) {
        self.stop(Run::Cancelled, StepState::Failed);
        self.rail.set_meta(self.stage, Some("cancelled".into()));
        self.log.push(vec![Span::new(
            format!("✗ Launch cancelled at {}", STAGES[self.stage].0),
            Tone::Warning,
        )]);
        self.facts.set_props(facts(self.run, self.cred_failed));
        cx.status("Launch cancelled");
    }

    fn advance(&mut self, cx: &mut Cx) -> Outcome {
        if self.run != Run::Running {
            return Outcome::Ignored;
        }
        if let Some(line) = stage_log(self.stage, self.stage_tick) {
            self.log.push(line);
        }
        self.stage_tick += 1;
        self.rail.set_meta(
            self.stage,
            Some(format!("{:.1} s", self.stage_tick as f32 * 0.4)),
        );
        let outcome = self.opts.value("launchcockpit.outcome");
        if outcome == "credentials" && self.stage == CREDENTIALS && !self.cred_failed {
            if self.stage_tick == 3 {
                self.cred_failed = true;
                self.run = Run::Held;
                self.facts.set_props(facts(self.run, true));
                self.log.push(vec![Span::new(
                    "! 1Password: the vault Engineering is locked",
                    Tone::Error,
                )]);
                cx.open_dialog(
                    Dialog::destructive(
                        CRED_DLG,
                        "Credential source unavailable",
                        "1Password: the vault Engineering is locked. The launch is paused at the Credentials stage; nothing was injected.",
                        "Cancel launch",
                    )
                    .actions(
                        vec![
                            Button::secondary(CRED_DLG.sub("retry"), "Retry"),
                            Button::danger(CRED_DLG.sub("cancel"), "Cancel launch"),
                        ],
                        Some(1),
                    ),
                );
            }
            return Outcome::Changed;
        }
        if outcome == "build-failure" && self.stage == DERIVED_IMAGE && self.stage_tick == 7 {
            self.stop(Run::Failed, StepState::Failed);
            self.rail.set_meta(self.stage, Some("exit 1".into()));
            self.log.push(vec![Span::new(
                "✗ RUN cargo build --release: error[E0432]: unresolved import `ledger::v2`",
                Tone::Error,
            )]);
            self.facts.set_props(facts(self.run, false));
            cx.open_dialog(Dialog::facts(
                FAIL_DLG,
                "Docker build failed",
                vec![
                    Prop::new("Stage", STAGES[DERIVED_IMAGE].0).tone(Tone::Error),
                    Prop::new("Run id", RUN_ID).copyable(),
                    Prop::new(
                        "Next step",
                        "Fix the import in crates/ledger/src/lib.rs, then launch again",
                    )
                    .wrap(),
                ],
                vec![],
                None,
                Button::primary(FAIL_DLG.sub("retry"), "Launch again"),
            ));
            return Outcome::Changed;
        }
        if self.stage_tick >= STAGES[self.stage].1 {
            self.rail.set_state(self.stage, StepState::Done);
            self.log.push(vec![
                Span::new("✓ ", Tone::Secondary),
                Span::muted(STAGES[self.stage].0),
            ]);
            if self.stage == CREDENTIALS {
                self.facts.set_props(facts(Run::Ready, false));
            }
            self.stage += 1;
            self.stage_tick = 0;
            if self.stage >= STAGES.len() {
                self.stage = STAGES.len() - 1;
                self.run = Run::Ready;
                self.log
                    .push(vec![Span::plain("Hardline open · capsule ready").bold()]);
                cx.status("Launched ✓ capsule cap-payments-01");
            } else if self.stage == 3 {
                self.rail.set_state(3, StepState::Skipped);
                self.rail.set_meta(3, Some("cached".into()));
                self.stage = 4;
                self.rail.set_state(self.stage, StepState::Running);
            } else {
                self.rail.set_state(self.stage, StepState::Running);
            }
        }
        Outcome::Changed
    }

    fn activity(&self, spinner: &str) -> String {
        let lines = self.log.len();
        match self.run {
            Run::Idle => "not started".into(),
            Run::Running => format!("{spinner} {} · {lines} lines", activity(self.stage)),
            Run::Held => format!("! Credentials paused · {lines} lines"),
            Run::Ready => format!("capsule ready · {lines} lines"),
            Run::Failed => format!("! {} failed · {lines} lines", STAGES[self.stage].0),
            Run::Cancelled => format!("cancelled · {lines} lines"),
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focus = if ctx.interaction.has_focus(FACTS) {
            Some(0)
        } else if [RAIL, LOG].iter().any(|&id| ctx.interaction.has_focus(id)) {
            Some(1)
        } else if [LAUNCH, CANCEL]
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
        {
            Some(2)
        } else {
            None
        };
        let l = self
            .sections
            .layout(area, &self.children(), focus, buf, ctx);
        match l.slot(0) {
            Some(r) => {
                let (done, skipped, failed) = self.rail.counts();
                let meta = format!(
                    "{} of {} stages{}",
                    done + skipped,
                    STAGES.len(),
                    if failed > 0 { " · failed" } else { "" }
                );
                let panel = Panel::card(Some("Launch · payments-platform")).meta(&meta);
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                self.facts.render(inner, buf, ctx, bg);
            }
            None => ctx.focus_stop(FACTS),
        }
        match l.slot(1) {
            Some(r) => {
                let rail_w = RAIL_W.min(r.width / 2);
                let left = Rect::new(r.x, r.y, rail_w, r.height);
                let right = Rect::new(
                    r.x + rail_w + 1,
                    r.y,
                    r.width.saturating_sub(rail_w + 1),
                    r.height,
                );
                let panel = Panel::framed(Some("Stages"))
                    .meta("11")
                    .focused(ctx.interaction.focused(RAIL));
                let bg = panel.bg(t);
                let inner = panel.render(left, buf, t);
                self.rail.render(inner, buf, ctx, bg);
                let meta = self.activity(t.glyphs.spinner_frame(ctx.interaction.tick));
                let panel = Panel::framed(Some("Build log"))
                    .meta(&meta)
                    .focused(ctx.interaction.focused(LOG));
                let bg = panel.bg(t);
                let inner = panel.render(right, buf, t);
                self.log.render(inner, buf, ctx, bg);
            }
            None => {
                ctx.focus_stop(RAIL);
                ctx.focus_stop(LOG);
            }
        }
        self.launch.label = match self.run {
            Run::Idle => "Launch",
            _ => "Launch again",
        }
        .into();
        self.launch.disabled = self.running();
        self.cancel.disabled = !self.running();
        let buttons = [self.launch.clone(), self.cancel.clone()];
        match l.slot(2) {
            Some(r) => {
                let lay = ButtonRow::new().layout(r, &buttons, t);
                for (b, rect) in [&mut self.launch, &mut self.cancel]
                    .into_iter()
                    .zip(lay.rects)
                {
                    match rect {
                        Some(rect) => b.render(rect, buf, ctx, t.canvas),
                        None => ctx.focus_stop(b.id),
                    }
                }
            }
            None => {
                ctx.focus_stop(LAUNCH);
                ctx.focus_stop(CANCEL);
            }
        }
    }

    fn children(&self) -> [StackChild; 3] {
        [
            StackChild::fixed(self.facts.props.len() as u16 + 3),
            StackChild::flex(STAGES.len() as u16 + 2, STAGES.len() as u16 + 8),
            StackChild::fixed(1),
        ]
    }

    fn ask_cancel(&mut self, cx: &mut Cx) {
        cx.open_dialog(
            Dialog::destructive(
                CANCEL_DLG,
                "Cancel launch?",
                "The capsule stops before it starts. Stages already done keep their results; nothing is handed off.",
                "Cancel launch",
            )
            .cancel_label("Keep launching"),
        );
    }

    fn act(&mut self, id: WidgetId, cx: &mut Cx) {
        match id {
            LAUNCH if !self.running() => self.start(cx),
            CANCEL if self.running() => self.ask_cancel(cx),
            _ => {}
        }
    }
}

fn facts(run: Run, cred_error: bool) -> Vec<Prop> {
    let creds = if cred_error && run == Run::Held {
        Prop::new("Credentials", "1Password · vault Engineering is locked").tone(Tone::Error)
    } else if matches!(run, Run::Ready) {
        Prop::new(
            "Credentials",
            "resolved in memory · discarded after injection",
        )
        .tone(Tone::Secondary)
    } else {
        Prop::new(
            "Credentials",
            "1Password · Engineering › Anthropic · Work › credential",
        )
        .tone(Tone::Muted)
    };
    vec![
        Prop::new("Target", "payments-platform · role backend"),
        Prop::new("Agent", "claude 1.0.71 · Opus"),
        Prop::new("Account", "Work (default) · resolved from the workspace"),
        creds,
        Prop::new("Run id", RUN_ID).copyable(),
    ]
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let states = [
        ("Identity", StepState::Done),
        ("Construct", StepState::Skipped),
        ("Derived Image", StepState::Running),
        ("Network", StepState::Queued),
    ];
    let mut rail = StepRail::new(
        id.child(0),
        states.iter().map(|(l, _)| Step::new(l)).collect(),
    )
    .selectable(true);
    for (i, (_, s)) in states.into_iter().enumerate() {
        rail.set_state(i, s);
    }
    let mut failed = StepRail::new(
        id.child(1),
        ["Credentials", "Construct", "Capsule"]
            .iter()
            .map(|l| Step::new(l))
            .collect(),
    );
    failed.set_state(0, StepState::Done);
    failed.set_state(1, StepState::Failed);
    failed.set_state(2, StepState::Blocked);
    StateGrid::new(id)
        .label_width(10)
        .column_width(28)
        .rows(&[Rest, Focused, Empty, Overflow, Narrow])
        .column("Stage rail", rail)
        .column("Failed launch", failed)
        .column(
            "Build log",
            TextViewport::with_lines(
                id.child(2),
                vec![
                    vec![Span::muted("  #3 COPY rust-toolchain.toml")],
                    vec![
                        Span::new("✓ ", Tone::Secondary),
                        Span::muted("Derived Image"),
                    ],
                ],
            ),
        )
}

impl Page for LaunchCockpitPage {
    fn title(&self) -> &'static str {
        "Launch cockpit"
    }
    fn blurb(&self) -> &'static str {
        "A launch: stage rail, build log, credentials and failure dialogs, Cancel launch"
    }

    fn measure(&self, _avail: Size, ctx: &RenderCtx) -> Size {
        let facts = Panel::card(Some("Launch · payments-platform"))
            .meta("11 of 11 stages · failed")
            .outer(Size::new(props::width(&self.facts.props), 0))
            .width;
        let meta = self.activity(ctx.theme.glyphs.spinner_frame(0));

        let widest = self
            .log
            .lines()
            .map(|l| termrock::ui::text::width(&line_text(l)) as u16)
            .max()
            .unwrap_or(0);
        let widest = widest.min(LOG_MIN);
        let log = Panel::framed(Some("Build log"))
            .meta(&meta)
            .outer(Size::new(widest, 0))
            .width
            .max(RAIL_W);
        let buttons = [self.launch.clone(), self.cancel.clone()];
        Size::new(
            facts
                .max(RAIL_W + 1 + log)
                .max(ButtonRow::new().width(&buttons, ctx.theme)),
            self.sections.stack.measure(&self.children()),
        )
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
            return self.advance(cx);
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev {
            match (*id, result) {
                (CANCEL_DLG, DialogResult::Action(1)) if self.running() => self.cancel_now(cx),
                (CRED_DLG, DialogResult::Action(0)) => {
                    self.run = Run::Running;
                    self.facts.set_props(facts(self.run, false));
                    self.log
                        .push(vec![Span::muted("  retry · 1Password unlocked")]);
                    cx.status("Retrying the credential source");
                }
                (CRED_DLG, _) => {
                    self.run = Run::Running;
                    self.cancel_now(cx);
                }
                (FAIL_DLG, DialogResult::Action(1)) => self.start(cx),
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
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(FACTS) {
                    let (o, a) = self.facts.on_key(key);
                    if let Some(PropsAction::Copy(i)) = a
                        && let Some(p) = self.facts.props.get(i)
                    {
                        cx.copy_as(p.value.clone(), p.label.clone());
                    }
                    return o;
                }
                if cx.focus.is(RAIL) {
                    return self.rail.on_key(key);
                }
                if cx.focus.is(LOG) {
                    let (o, a) = self.log.on_key(key);
                    if let Some(ViewportAction::Copy(text)) = a {
                        let n = text.lines().count();
                        cx.copy_as(
                            text,
                            format!("{n} log line{}", if n == 1 { "" } else { "s" }),
                        );
                        return Outcome::Changed;
                    }
                    return o;
                }
                for b in [LAUNCH, CANCEL] {
                    if cx.focus.is(b) {
                        let btn = if b == LAUNCH {
                            &mut self.launch
                        } else {
                            &mut self.cancel
                        };
                        let (o, act) = btn.on_key(key);
                        if act {
                            self.act(b, cx);
                            return Outcome::Changed;
                        }
                        return o;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for b in [LAUNCH, CANCEL] {
                    let btn = if b == LAUNCH {
                        &mut self.launch
                    } else {
                        &mut self.cancel
                    };
                    if let Some(r) = btn.route(ev)
                        && r.outcome() != Outcome::Ignored
                    {
                        if r.has_actions() {
                            cx.focus.focus(b);
                            self.act(b, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                if let Some(r) = self.log.route(ev) {
                    if ev.id == LOG && (ev.is(Phase::Press) || ev.is(Phase::Click)) {
                        cx.focus.focus(LOG);
                    }
                    return r.outcome();
                }
                if let Some(r) = self.rail.route(ev) {
                    if ev.index_of(Part::ROW).is_some() && ev.is(Phase::Click) {
                        cx.focus.focus(RAIL);
                    }
                    return r.outcome();
                }
                if let Some(r) = self.facts.route(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(FACTS);
                    }
                    return r.into_parts().0;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.run == Run::Running
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        match focus {
            Some(LOG) => vec![
                ("↑↓", "Scroll"),
                ("Home End", "Oldest / live"),
                ("f", "Follow"),
                ("y", "Copy"),
            ],
            Some(RAIL) => vec![("↑↓", "Move")],
            Some(FACTS) => vec![("↑↓", "Move"), ("y", "Copy")],
            _ => vec![("Enter", "Activate")],
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
