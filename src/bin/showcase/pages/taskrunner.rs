use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::MouseTarget;
use termrock::theme::{EnvKind, Glyphs, StatusKind, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::button::{Button, row_layout};
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::panel::{Panel, ScrollPanel};
use termrock::widgets::progress::{ProgressBar, ProgressStatus, render_bar, render_indeterminate};
use termrock::widgets::scrollbar;
use termrock::widgets::tree::TreeNode;
use termrock::widgets::tree::TreeView;

const ID: WidgetId = WidgetId::of("taskrunner");
const CANCEL_DLG: WidgetId = ID.sub("cancel-dialog");

#[derive(Debug, Clone)]
struct Task {
    name: &'static str,
    progress: f64,
    state: TaskState,
    speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

pub struct TaskRunnerPage {
    template: Template,
    opts: PageValues,
    tree: TreeView,
    tasks: Vec<Task>,
    log: ScrollPanel,
    run: Button,
    cancel: Button,
    running: bool,
    ticks: u64,
    sections: crate::pages::layout::Sections,
}

fn targets(glyphs: bool, meta: bool) -> Vec<TreeNode> {
    let env = |name: &str, kind: EnvKind| {
        if glyphs {
            env(name, kind)
        } else {
            TreeNode::leaf(name)
        }
    };
    let dir = |name: &str, kids: Vec<TreeNode>| {
        let n = kids.len();
        let node = TreeNode::dir(name, kids);
        if meta {
            node.meta(&n.to_string())
        } else {
            node
        }
    };
    vec![
        dir(
            "payments-gateway",
            vec![
                dir(
                    "build",
                    vec![
                        TreeNode::leaf("compile"),
                        TreeNode::leaf("lint"),
                        TreeNode::leaf("typecheck"),
                    ],
                ),
                dir(
                    "test",
                    vec![
                        TreeNode::leaf("unit"),
                        TreeNode::leaf("integration"),
                        TreeNode::leaf("e2e"),
                    ],
                ),
                dir(
                    "deploy",
                    vec![
                        env("staging", EnvKind::Staging),
                        env("production", EnvKind::Production),
                    ],
                ),
            ],
        ),
        dir(
            "shared-libs",
            vec![TreeNode::leaf("compile"), TreeNode::leaf("publish")],
        ),
    ]
}

fn env(name: &str, kind: EnvKind) -> TreeNode {
    let node = TreeNode::leaf(name)
        .glyph(Glyphs::default().env(kind))
        .glyph_tone(kind.tone());
    if kind == EnvKind::Production {
        node.label_tone(Tone::Danger)
    } else {
        node
    }
}

impl TaskRunnerPage {
    pub fn new() -> Self {
        let tasks = ["compile", "lint", "typecheck", "unit", "integration", "e2e"]
            .iter()
            .enumerate()
            .map(|(i, n)| Task {
                name: n,
                progress: 0.0,
                state: TaskState::Queued,
                speed: 0.012 + (i as f64 % 3.0) * 0.006,
            })
            .collect();
        let log = ScrollPanel::new(
            ID.sub("log"),
            vec!["Ready. Press r or Run to start the pipeline.".into()],
        )
        .tail(true);
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::TaskRunner)),
            tree: TreeView::new(ID.sub("tree"), Vec::new()),
            tasks,
            log,
            run: Button::primary(ID.sub("run"), "Run pipeline"),
            cancel: Button::secondary(ID.sub("cancel"), "Cancel"),
            running: false,
            ticks: 0,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.apply();
        page
    }

    fn apply(&mut self) {
        let o = &self.opts;
        let mut tree = TreeView::new(
            ID.sub("tree"),
            targets(o.on("taskrunner.env_glyphs"), o.on("taskrunner.tree_meta")),
        )
        .indent(o.number("taskrunner.indent").unwrap_or(2))
        .show_meta(o.on("taskrunner.tree_meta"));
        if o.on("taskrunner.expanded") {
            tree.expand_all();
        }
        self.tree = tree;
        self.log = ScrollPanel::new(ID.sub("log"), self.log.lines().to_vec())
            .tail(o.on("taskrunner.follow"))
            .wrap(o.on("taskrunner.wrap"));
    }

    fn start(&mut self, cx: &mut Cx) {
        for t in &mut self.tasks {
            t.progress = 0.0;
            t.state = TaskState::Queued;
        }
        self.running = true;
        self.log.clear();
        self.log.push("Pipeline started".into());
        self.log.follow = true;
        cx.status("Pipeline running");
    }

    fn cancel_now(&mut self, cx: &mut Cx) {
        self.running = false;
        for t in &mut self.tasks {
            if t.state == TaskState::Running || t.state == TaskState::Queued {
                t.state = TaskState::Cancelled;
            }
        }
        self.log.push("Pipeline cancelled by user".into());
        cx.status("Cancelled");
    }

    fn step(&mut self, cx: &mut Cx) -> bool {
        if !self.running {
            return false;
        }
        self.ticks += 1;

        let running = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Running)
            .count();
        if running < 2
            && let Some(t) = self.tasks.iter_mut().find(|t| t.state == TaskState::Queued)
        {
            t.state = TaskState::Running;
            self.log.push(format!("▶ {} started", t.name));
        }
        let mut changed = false;
        for i in 0..self.tasks.len() {
            let t = &mut self.tasks[i];
            if t.state != TaskState::Running {
                continue;
            }
            t.progress = (t.progress + t.speed).min(1.0);
            changed = true;
            if self.ticks.is_multiple_of(9) {
                self.log.push(format!(
                    "  {}: step {} of 12",
                    t.name,
                    ((t.progress * 12.0) as u32).min(12)
                ));
            }
            if t.progress >= 1.0 {
                if t.name == "integration" {
                    t.state = TaskState::Failed;
                    self.log.push(format!(
                        "✗ {} failed: checkout::places_order (assertion)",
                        t.name
                    ));
                } else {
                    t.state = TaskState::Done;
                    self.log.push(format!("✓ {} finished", t.name));
                }
            }
        }
        if self.tasks.iter().all(|t| {
            matches!(
                t.state,
                TaskState::Done | TaskState::Failed | TaskState::Cancelled
            )
        }) {
            self.running = false;
            let failed = self
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Failed)
                .count();
            if failed > 0 {
                self.log
                    .push(format!("Pipeline finished with {failed} failure"));
                cx.status(format!("{failed} task failed"));
            } else {
                self.log.push("Pipeline finished ✓".into());
                cx.status("Pipeline finished ✓");
            }
        }
        changed
    }
}

impl TaskRunnerPage {
    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let tree_h = self.tree.rows().len() as u16 + 3;
        let pipe_h = self.tasks.len() as u16 + 5;
        let has = |id| ctx.interaction.has_focus(id);
        let in_tree = has(self.tree.id);
        let in_pipe = has(self.run.id) || has(self.cancel.id);
        let in_log = has(self.log.id);
        self.run.disabled = self.running;
        self.cancel.disabled = !self.running;
        let log_kid = |h: u16| StackChild::flex(5, h.max(5));
        let (tree_slot, pipe_slot, log_slot) = if area.width >= 30 + 2 + 20 {
            let l = Rect::new(area.x, area.y, 30, area.height);
            let r = Rect::new(area.x + 32, area.y, area.width - 32, area.height);
            let focus = if in_pipe {
                Some(0)
            } else if in_log {
                Some(1)
            } else {
                None
            };
            let s = self.sections.layout(
                r,
                &[
                    StackChild::fixed(pipe_h),
                    log_kid(r.height.saturating_sub(pipe_h + 1)),
                ],
                focus,
                buf,
                ctx,
            );
            (
                Some(Rect::new(l.x, l.y, l.width, tree_h.min(l.height))),
                s.slot(0),
                s.slot(1),
            )
        } else {
            let focus = if in_tree {
                Some(0)
            } else if in_pipe {
                Some(1)
            } else if in_log {
                Some(2)
            } else {
                None
            };
            let s = self.sections.layout(
                area,
                &[
                    StackChild::flex(6, tree_h),
                    StackChild::fixed(pipe_h),
                    log_kid(area.height.saturating_sub(tree_h + pipe_h + 2)),
                ],
                focus,
                buf,
                ctx,
            );
            (s.slot(0), s.slot(1), s.slot(2))
        };
        match tree_slot {
            Some(r) => self.render_targets(r, buf, ctx),
            None => ctx.focus_stop(self.tree.id),
        }
        match pipe_slot {
            Some(r) => self.render_pipeline(r, buf, ctx),
            None => {
                for b in [&self.run, &self.cancel] {
                    if !b.disabled {
                        ctx.focus_stop(b.id);
                    }
                }
            }
        }
        match log_slot {
            Some(r) => self.render_log(r, buf, ctx),
            None => ctx.focus_stop(self.log.id),
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let mut tree = TreeView::new(ID.sub("states-tree"), targets(true, false));
    tree.expand_all();
    let log = ScrollPanel::new(
        ID.sub("states-log"),
        vec![
            "▶ unit started".into(),
            "  unit: step 6 of 12".into(),
            "✓ unit finished".into(),
            "✗ integration failed".into(),
            "Pipeline finished with 1 failure".into(),
        ],
    )
    .tail(true);
    StateGrid::new(ID.sub("states"))
        .label_width(14)
        .column_width(26)
        .narrow_width(14)
        .rows(&[
            Rest, Hover, Pressed, Focused, Selected, Disabled, Error, Busy, Empty, Overflow, Narrow,
        ])
        .column("Targets", tree)
        .column("Log", log)
        .column("Task bar", ProgressBar::new("unit", 0.5))
}

impl Page for TaskRunnerPage {
    fn title(&self) -> &'static str {
        "Task runner"
    }
    fn blurb(&self) -> &'static str {
        "Composed: tree, live progress, following log, busy states"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        let buttons = self.run.width() + 2 + self.cancel.width();
        let tasks = 14 + termrock::ui::text::width("cancelled") as u16;
        let pipe = Panel::card(Some("Pipeline · running"))
            .meta("99 of 99 done")
            .outer(Size::new(buttons.max(tasks), 0))
            .width;
        let tree_h = self.tree.rows().len() as u16 + 3;
        let pipe_h = self.tasks.len() as u16 + 5;
        Size::new(32 + pipe, tree_h.max(pipe_h + 1 + 5))
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
            return if self.step(cx) {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev
            && *id == CANCEL_DLG
        {
            if *result == DialogResult::Action(1) {
                self.cancel_now(cx);
            }
            return Outcome::Changed;
        }
        let playground = self.template.section() == Section::Playground;

        if let PageEvent::Key(key) = ev
            && playground
            && key.is_char('r')
            && !self.running
            && !cx.focus.is(self.log.id)
            && !cx.focus.is(self.tree.id)
        {
            self.start(cx);
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if !playground {
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
                if f == self.tree.id {
                    return self.tree.on_key(key).outcome();
                }
                if f == self.log.id {
                    return self.log.on_key(key);
                }
                if f == self.run.id {
                    let (o, act) = self.run.on_key(key);
                    if act {
                        self.start(cx);
                    }
                    return o;
                }
                if f == self.cancel.id {
                    let (o, act) = self.cancel.on_key(key);
                    if act {
                        cx.open_dialog(Dialog::destructive(
                            CANCEL_DLG,
                            "Cancel pipeline?",
                            "Running tasks stop immediately. Finished tasks keep their results.",
                            "Cancel pipeline",
                        )
                        .cancel_label("Keep running"));
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.tree.route(ev) {
                    if ev.is(Phase::Click) && self.tree.row_at(ev).is_some() {
                        cx.focus.focus(self.tree.id);
                    }
                    return r.outcome();
                }
                if let Some(r) = self.log.route(ev) {
                    return r.outcome();
                }
                if ev.is(Phase::Click) {
                    if let Some(r) = self.run.route(ev) {
                        if r.has_actions() {
                            self.start(cx);
                        }
                        return Outcome::Changed;
                    }
                    if let Some(r) = self.cancel.route(ev) {
                        if r.has_actions() {
                            cx.open_dialog(Dialog::destructive(
                                CANCEL_DLG,
                                "Cancel pipeline?",
                                "Running tasks stop immediately. Finished tasks keep their results.",
                                "Cancel pipeline",
                            )
                            .cancel_label("Keep running"));
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.running
    }

    fn leave_stakes(&self) -> termrock::ui::leave::Stakes {
        if self.running {
            termrock::ui::leave::Stakes::Running {
                what: "The pipeline".into(),
            }
        } else {
            termrock::ui::leave::Stakes::Clean
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.log.id) {
            vec![("↑↓", "Scroll"), ("f", "Follow"), ("r", "Run")]
        } else if focus == Some(self.tree.id) {
            vec![("↑↓", "Move"), ("←→", "Fold")]
        } else {
            vec![("r", "Run pipeline"), ("Enter", "Activate")]
        }
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

impl TaskRunnerPage {
    fn render_targets(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let panel = Panel::card(Some("Targets")).focused(ctx.interaction.focused(self.tree.id));
        let bg = panel.bg(ctx.theme);
        if let Some(inner) = panel.try_render(r, buf, ctx.theme) {
            self.tree.render(inner, buf, ctx, bg);
        }
    }

    fn render_pipeline(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let done = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Done)
            .count();
        let meta = format!("{done} of {} done", self.tasks.len());
        let title = if self.running {
            "Pipeline · running"
        } else {
            "Pipeline"
        };
        let panel = Panel::card(Some(title)).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(r, buf, t);
        for (i, task) in self.tasks.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            let label = format!("{:<12}", task.name);
            let name = termrock::ui::text::truncate(&label, 10);
            let bar = Rect::new(inner.x + 14, y, inner.width.saturating_sub(14).min(50), 1);

            let kind = match task.state {
                TaskState::Queued => StatusKind::Queued,
                TaskState::Running => StatusKind::Running,
                TaskState::Done => StatusKind::Ok,
                TaskState::Failed => StatusKind::Fail,
                TaskState::Cancelled => StatusKind::Skipped,
            };
            let (glyph, glyph_style) = t.status_at(kind, ctx.interaction.tick);
            buf.set_string(inner.x, y, glyph, glyph_style.bg(bg));
            let name_style = match task.state {
                TaskState::Queued | TaskState::Cancelled => t.muted(),
                TaskState::Done => t.secondary(),

                TaskState::Running => t.status_label(kind),
                TaskState::Failed => t.primary(),
            };
            buf.set_string(inner.x + 2, y, &name, name_style.bg(bg));
            let word = |buf: &mut Buffer, text: &str| {
                let fg = t.tone_on(kind.tone(), bg);
                buf.set_string(bar.x, y, text, ratatui::style::Style::new().fg(fg).bg(bg));
            };
            match task.state {
                TaskState::Queued => word(buf, "queued"),
                TaskState::Cancelled => word(buf, "cancelled"),
                TaskState::Running => {
                    render_bar(bar, buf, ctx, "", task.progress, ProgressStatus::Active, bg);
                }
                TaskState::Done => {
                    render_bar(bar, buf, ctx, "", 1.0, ProgressStatus::Done, bg);
                }
                TaskState::Failed => {
                    render_bar(bar, buf, ctx, "", task.progress, ProgressStatus::Error, bg);
                }
            }
        }
        let ay = inner.bottom().saturating_sub(1);
        if self.running {
            render_indeterminate(
                Rect::new(inner.x, ay - 1, inner.width.min(64), 1),
                buf,
                ctx,
                "Overall   ",
                bg,
            );
        }
        self.run.disabled = self.running;
        self.cancel.disabled = !self.running;
        let widths = [self.run.width(), self.cancel.width()];
        if widths[0] + 2 + widths[1] <= inner.width {
            let rects = row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
            self.run.render(rects[0], buf, ctx, bg);
            self.cancel.render(rects[1], buf, ctx, bg);
        } else if self.running {
            self.cancel
                .render(Rect::new(inner.x, ay, inner.width, 1), buf, ctx, bg);
        } else {
            self.run
                .render(Rect::new(inner.x, ay, inner.width, 1), buf, ctx, bg);
        }
    }

    fn render_log(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let lf = ctx.interaction.focused(self.log.id);
        let panel = Panel::card(Some("Log")).focused(lf);
        let bg = panel.bg(t);
        let inner = panel.render(r, buf, t);
        self.log.render(inner, buf, ctx, bg, |t, line| {
            if line.starts_with('✗') || line.contains("failure") || line.contains("cancelled") {
                t.error_fg()
            } else if line.starts_with('✓') || line.ends_with('✓') {
                t.accent_fg()
            } else if line.starts_with('▶') {
                t.primary()
            } else {
                t.secondary()
            }
        });
        let pos = scrollbar::position_label(&self.log.scroll);
        let meta = if self.log.follow {
            format!("{pos} · following")
        } else {
            pos
        };
        panel.draw_meta(r, buf, t, &meta);
    }
}
