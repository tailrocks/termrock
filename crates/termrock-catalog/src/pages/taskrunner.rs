// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/taskrunner.rs (MIT).

//! Composed: tree, live progress, following log, busy states.

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::MotionPolicy;
use termrock::widgets::{
    Action, ActionVariant, ActivationOutcome, ButtonState, ButtonVariant, Dialog, DialogOutcome,
    DialogState, LogPaneState, ProgressBar, ProgressKind, ProgressStatus, SpinnerState, Tree,
    TreeNode, TreeOutcome, TreeState,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::tablepro::paint;
use crate::text;

const ID: WidgetId = WidgetId::of("taskrunner");
const TREE: WidgetId = ID.sub("tree");
const LOG: WidgetId = ID.sub("log");
const RUN: WidgetId = ID.sub("run");
const CANCEL: WidgetId = ID.sub("cancel");

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

struct Node {
    label: &'static str,
    children: Vec<Node>,
}

impl Node {
    fn dir(label: &'static str, children: Vec<Node>) -> Self {
        Self { label, children }
    }
    fn leaf(label: &'static str) -> Self {
        Self {
            label,
            children: vec![],
        }
    }
}

fn targets() -> Vec<Node> {
    vec![
        Node::dir(
            "payments-gateway",
            vec![
                Node::dir(
                    "build",
                    vec![
                        Node::leaf("compile"),
                        Node::leaf("lint"),
                        Node::leaf("typecheck"),
                    ],
                ),
                Node::dir(
                    "test",
                    vec![
                        Node::leaf("unit"),
                        Node::leaf("integration"),
                        Node::leaf("e2e"),
                    ],
                ),
                Node::dir(
                    "deploy",
                    vec![Node::leaf("staging"), Node::leaf("production")],
                ),
            ],
        ),
        Node::dir(
            "shared-libs",
            vec![Node::leaf("compile"), Node::leaf("publish")],
        ),
    ]
}

fn flatten<'a>(
    nodes: &'a [Node],
    expanded: &HashSet<Vec<usize>>,
    parent: &[usize],
    depth: u16,
    out: &mut Vec<(Vec<usize>, &'a Node, u16, bool)>,
) {
    for (i, n) in nodes.iter().enumerate() {
        let mut path = parent.to_vec();
        path.push(i);
        let branch = !n.children.is_empty();
        let open = branch && expanded.contains(&path);
        out.push((path.clone(), n, depth, open));
        if open {
            flatten(&n.children, expanded, &path, depth.saturating_add(1), out);
        }
    }
}

fn expand_all(nodes: &[Node], parent: &mut Vec<usize>, expanded: &mut HashSet<Vec<usize>>) {
    for (i, n) in nodes.iter().enumerate() {
        parent.push(i);
        if !n.children.is_empty() {
            expanded.insert(parent.clone());
            expand_all(&n.children, parent, expanded);
        }
        parent.pop();
    }
}

fn log_style(t: &termrock::style::JunieTheme, line: &str) -> ratatui::style::Style {
    if line.starts_with('✗') || line.contains("failure") || line.contains("cancelled") {
        t.error_fg()
    } else if line.starts_with('✓') || line.ends_with('✓') {
        t.accent_fg()
    } else if line.starts_with('▶') {
        t.primary()
    } else {
        t.secondary()
    }
}

fn position_label(offset: usize, view: usize, total: usize) -> String {
    if view == 0 || total <= view {
        return String::new();
    }
    let start = offset.saturating_add(1);
    let end = offset.saturating_add(view).min(total);
    format!("{start}–{end} of {total}")
}

pub struct TaskRunnerPage {
    nodes: Vec<Node>,
    expanded: HashSet<Vec<usize>>,
    tree: TreeState<Vec<usize>>,
    tasks: Vec<Task>,
    log: LogPaneState,
    run: ButtonState,
    cancel: ButtonState,
    running: bool,
    ticks: u64,
    overlay: bool,
    dialog: DialogState<&'static str>,
    log_view: usize,
}

impl TaskRunnerPage {
    #[must_use]
    pub fn new() -> Self {
        let nodes = targets();
        let mut expanded = HashSet::new();
        expand_all(&nodes, &mut Vec::new(), &mut expanded);
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
        let mut log = LogPaneState::new();
        log.append("Ready. Press r or Run to start the pipeline.");
        log.follow();
        Self {
            nodes,
            expanded,
            tree: TreeState::new(Some(vec![0])),
            tasks,
            log,
            run: ButtonState::new(),
            cancel: ButtonState::new(),
            running: false,
            ticks: 0,
            overlay: false,
            dialog: DialogState::destructive("ok", "cancel"),
            log_view: 0,
        }
    }

    fn visible(&self) -> Vec<(Vec<usize>, &Node, u16, bool)> {
        let mut out = Vec::new();
        flatten(&self.nodes, &self.expanded, &[], 0, &mut out);
        out
    }

    fn tree_rows(visible: &[(Vec<usize>, &Node, u16, bool)]) -> Vec<TreeNode<'static, Vec<usize>>> {
        visible
            .iter()
            .map(|(path, n, depth, open)| {
                let mut node = TreeNode::new(path.clone(), Line::from(n.label), *depth);
                if !n.children.is_empty() {
                    node = node.branch();
                    if *open {
                        node = node.expanded();
                    }
                }
                node
            })
            .collect()
    }

    fn apply_tree(&mut self, out: TreeOutcome<Vec<usize>>) -> Route {
        match out {
            TreeOutcome::Toggle(path) => {
                if !self.expanded.remove(&path) {
                    self.expanded.insert(path);
                }
                Route::Changed
            }
            TreeOutcome::Activated(_) | TreeOutcome::SelectionChanged(_) => Route::Changed,
            TreeOutcome::Ignored | TreeOutcome::Cancelled => Route::Ignored,
            _ => Route::Changed,
        }
    }

    fn push_log(&mut self, line: String) {
        self.log.append(line);
        if self.log.is_following() {
            self.log.follow();
        }
    }

    fn start(&mut self, cx: &mut PageCtx<'_>) {
        for t in &mut self.tasks {
            t.progress = 0.0;
            t.state = TaskState::Queued;
        }
        self.running = true;
        self.log.clear();
        self.push_log("Pipeline started".into());
        self.log.follow();
        cx.status("Pipeline running");
    }

    fn cancel_now(&mut self, cx: &mut PageCtx<'_>) {
        self.running = false;
        for t in &mut self.tasks {
            if t.state == TaskState::Running || t.state == TaskState::Queued {
                t.state = TaskState::Cancelled;
            }
        }
        self.push_log("Pipeline cancelled by user".into());
        cx.status("Cancelled");
    }

    fn step(&mut self, cx: &mut PageCtx<'_>) -> bool {
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
            let name = t.name;
            self.push_log(format!("▶ {name} started"));
        }
        let mut changed = false;
        let mut events = Vec::new();
        for i in 0..self.tasks.len() {
            let t = &mut self.tasks[i];
            if t.state != TaskState::Running {
                continue;
            }
            t.progress = (t.progress + t.speed).min(1.0);
            changed = true;
            if self.ticks.is_multiple_of(9) {
                events.push(format!(
                    "  {}: step {} of 12",
                    t.name,
                    ((t.progress * 12.0) as u32).min(12)
                ));
            }
            if t.progress >= 1.0 {
                if t.name == "integration" {
                    t.state = TaskState::Failed;
                    events.push(format!(
                        "✗ {} failed: checkout::places_order (assertion)",
                        t.name
                    ));
                } else {
                    t.state = TaskState::Done;
                    events.push(format!("✓ {} finished", t.name));
                }
            }
        }
        for e in events {
            self.push_log(e);
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
                self.push_log(format!("Pipeline finished with {failed} failure"));
                cx.status(format!("{failed} task failed"));
            } else {
                self.push_log("Pipeline finished ✓".into());
                cx.status("Pipeline finished ✓");
            }
        }
        changed
    }

    fn open_cancel(&mut self) {
        self.dialog = DialogState::destructive("ok", "cancel");
        self.overlay = true;
    }

    fn cancel_actions() -> [Action<'static, &'static str>; 2] {
        [
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "ok",
                label: "Cancel pipeline",
                enabled: true,
                variant: ActionVariant::Destructive,
            },
        ]
    }

    fn apply_dialog(&mut self, out: DialogOutcome<&'static str>, cx: &mut PageCtx<'_>) -> Route {
        match out {
            DialogOutcome::Ignored | DialogOutcome::LoadingBlocked => Route::Consumed,
            DialogOutcome::Activated("ok") | DialogOutcome::DefaultActivated("ok") => {
                self.overlay = false;
                self.cancel_now(cx);
                Route::Changed
            }
            DialogOutcome::Activated(_) | DialogOutcome::Cancelled => {
                self.overlay = false;
                Route::Changed
            }
            _ => Route::Changed,
        }
    }
}

impl Page for TaskRunnerPage {
    fn title(&self) -> &'static str {
        "Task runner"
    }
    fn blurb(&self) -> &'static str {
        "Composed: tree, live progress, following log, busy states"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let overlay = self.overlay;
        let saved_inert = ctx.inert;
        ctx.inert = saved_inert || overlay;

        let (l, r) = layout::columns(area, 30, 2);
        let visible = self.visible();
        let view_h = l.height.saturating_sub(3) as usize;
        let pos = position_label(self.tree.offset(), view_h, visible.len());
        let th = (visible.len() as u16 + 3).min(l.height);
        let rows = Self::tree_rows(&visible);
        drop(visible);
        let (inner, _) = layout::card(
            Rect::new(l.x, l.y, l.width, th),
            buf,
            t,
            Some("Targets"),
            Some(&pos),
            ctx.interaction.focused(TREE) && !overlay,
        );
        Tree::new(&rows, ctx.system)
            .focused(ctx.interaction.focused(TREE) && !overlay)
            .render(inner, buf, &mut self.tree);
        ctx.control(TREE, inner, overlay);
        ctx.scrollable(TREE, inner);

        let rrows = layout::rows(r, &[self.tasks.len() as u16 + 5, 1, 0]);
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
        let (inner, bg) = layout::card(rrows[0], buf, t, Some(title), Some(&meta), false);
        let frame = paint::tick_frame(ctx.interaction.tick);
        for (i, task) in self.tasks.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            let label = format!("{:<12}", task.name);
            let bar = Rect::new(inner.x + 14, y, inner.width.saturating_sub(14).min(50), 1);
            match task.state {
                TaskState::Queued => {
                    buf.set_string(inner.x, y, &label, t.muted().bg(bg));
                    buf.set_string(inner.x + 14, y, "queued", t.faint().bg(bg));
                }
                TaskState::Cancelled => {
                    buf.set_string(inner.x, y, &label, t.muted().bg(bg));
                    buf.set_string(inner.x + 14, y, "cancelled", t.faint().bg(bg));
                }
                TaskState::Running => {
                    let spin = SpinnerState::new();
                    let glyph = spin.frame_glyph(frame, MotionPolicy::Full);
                    buf.set_string(inner.x, y, glyph, t.accent_fg().bg(bg));
                    buf.set_string(
                        inner.x + 2,
                        y,
                        &label[..label.len().min(10)],
                        t.primary().bg(bg),
                    );
                    ProgressBar::new(
                        ProgressKind::Determinate {
                            fraction: task.progress,
                        },
                        ctx.system,
                    )
                    .status(ProgressStatus::Running)
                    .paint(bar, buf);
                }
                TaskState::Done => {
                    buf.set_string(inner.x, y, "✓", t.secondary().bg(bg));
                    buf.set_string(
                        inner.x + 2,
                        y,
                        &label[..label.len().min(10)],
                        t.secondary().bg(bg),
                    );
                    ProgressBar::new(ProgressKind::Determinate { fraction: 1.0 }, ctx.system)
                        .status(ProgressStatus::Complete)
                        .paint(bar, buf);
                }
                TaskState::Failed => {
                    buf.set_string(inner.x, y, "!", t.error_fg().bg(bg));
                    buf.set_string(
                        inner.x + 2,
                        y,
                        &label[..label.len().min(10)],
                        t.primary().bg(bg),
                    );
                    ProgressBar::new(
                        ProgressKind::Determinate {
                            fraction: task.progress,
                        },
                        ctx.system,
                    )
                    .status(ProgressStatus::Failed)
                    .paint(bar, buf);
                }
            }
        }
        let ay = inner.bottom().saturating_sub(1);
        if self.running {
            ProgressBar::new(
                ProgressKind::Indeterminate {
                    tick: ctx.interaction.tick,
                },
                ctx.system,
            )
            .label("Overall   ")
            .paint(
                Rect::new(inner.x, ay.saturating_sub(1), inner.width.min(64), 1),
                buf,
            );
        }
        let run_w = paint::button_width("Run pipeline");
        let cancel_w = paint::button_width("Cancel");
        if run_w + 2 + cancel_w <= inner.width {
            let rects = layout::row_layout(
                Rect::new(inner.x, ay, inner.width, 1),
                &[run_w, cancel_w],
                2,
            );
            paint::button(
                "Run pipeline",
                ButtonVariant::Primary,
                RUN,
                rects[0],
                buf,
                ctx,
                &mut self.run,
                self.running || overlay,
                bg,
            );
            paint::button(
                "Cancel",
                ButtonVariant::Secondary,
                CANCEL,
                rects[1],
                buf,
                ctx,
                &mut self.cancel,
                !self.running || overlay,
                bg,
            );
        } else if self.running {
            paint::button(
                "Cancel",
                ButtonVariant::Secondary,
                CANCEL,
                Rect::new(inner.x, ay, inner.width, 1),
                buf,
                ctx,
                &mut self.cancel,
                overlay,
                bg,
            );
        } else {
            paint::button(
                "Run pipeline",
                ButtonVariant::Primary,
                RUN,
                Rect::new(inner.x, ay, inner.width, 1),
                buf,
                ctx,
                &mut self.run,
                overlay,
                bg,
            );
        }

        let lf = ctx.interaction.focused(LOG) && !overlay;
        let n = self.log.len();
        let pos = crate::layout::overflow_label(0, self.log_view, n);
        let meta = if self.log.is_following() {
            format!("{pos} · following")
        } else {
            pos
        };
        let (inner, bg) = layout::card(rrows[2], buf, t, Some("Log"), Some(&meta), lf);
        // Source ScrollPanel: unframed lines from the card origin, `fit(width-2)`.
        let text_w = inner.width.saturating_sub(2);
        let lines = self.log.lines();
        let h = usize::from(inner.height);
        let offset = if self.log.is_following() {
            lines.len().saturating_sub(h)
        } else {
            0
        };
        for (i, line) in lines.iter().skip(offset).take(h).enumerate() {
            let y = inner.y.saturating_add(i as u16);
            if y >= inner.bottom() {
                break;
            }
            let s = line.to_string();
            let st = log_style(t, &s).bg(bg);
            buf.set_string(inner.x, y, &text::fit(&s, text_w as usize), st);
        }
        if lines.len() > h {
            termrock::scroll::paint_overflow_scrollbar(
                buf,
                Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height),
                lines.len(),
                h.max(1),
                u16::try_from(offset).unwrap_or(u16::MAX),
                lf,
                ctx.system,
            );
        }
        ctx.control(LOG, inner, overlay);
        ctx.scrollable(LOG, inner);
        self.log_view = usize::from(inner.height);

        ctx.inert = saved_inert;
        if overlay {
            self.dialog.set_open(true);
            self.dialog.set_accepts_input(true);
            let actions = Self::cancel_actions();
            Dialog::destructive(
                "Cancel pipeline?",
                Text::from("Running tasks stop immediately. Finished tasks keep their results."),
                ctx.system,
            )
            .paint_modal(area, buf, &mut self.dialog, &actions);
            ctx.control(ID.sub("modal"), area, false);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if self.overlay {
            let actions = Self::cancel_actions();
            return match ev {
                PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    let out = self.dialog.handle_key(*key, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        self.apply_dialog(out, cx)
                    }
                }
                PageEvent::Click { pos, .. } => {
                    let out = self.dialog.handle_click(*pos, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        self.apply_dialog(out, cx)
                    }
                }
                _ => Route::Consumed,
            };
        }
        match ev {
            PageEvent::Tick => {
                if self.step(cx) {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if matches!(key.code, KeyCode::Char('r'))
                    && !self.running
                    && cx.focus_id() != Some(LOG)
                    && cx.focus_id() != Some(TREE)
                {
                    self.start(cx);
                    return Route::Changed;
                }
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == TREE {
                    let rows = {
                        let visible = self.visible();
                        Self::tree_rows(&visible)
                    };
                    let out = self.tree.handle_key(&rows, *key);
                    if let Some(flag) = self.tree.take_bulk_disclosure() {
                        if flag {
                            expand_all(&self.nodes, &mut Vec::new(), &mut self.expanded);
                        } else {
                            self.expanded.clear();
                        }
                        return Route::Changed;
                    }
                    return self.apply_tree(out);
                } else if f == LOG {
                    if matches!(key.code, KeyCode::Char('f')) && key.modifiers.is_empty() {
                        self.log.follow();
                        return Route::Changed;
                    }
                    return match self.log.handle_key(*key) {
                        termrock::widgets::Outcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                } else if f == RUN {
                    self.run.activation.set_accepts_input(!self.running);
                    return match self.run.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            if !self.running {
                                self.start(cx);
                            }
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                } else if f == CANCEL {
                    self.cancel.activation.set_accepts_input(self.running);
                    return match self.cancel.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            if self.running {
                                self.open_cancel();
                            }
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Click { id, pos } => {
                if *id == TREE {
                    cx.set_focus(TREE);
                    let n = self.visible().len();
                    if self.tree.scroll_to_position(*pos, n) {
                        return Route::Changed;
                    }
                    let out = self.tree.click(*pos);
                    return self.apply_tree(out);
                }
                if *id == RUN {
                    if !self.running {
                        self.start(cx);
                    }
                    return Route::Changed;
                }
                if *id == CANCEL {
                    if self.running {
                        self.open_cancel();
                    }
                    return Route::Changed;
                }
                if *id == LOG {
                    cx.set_focus(LOG);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Drag { pressed, pos } if *pressed == TREE => {
                let n = self.visible().len();
                if self.tree.scroll_to_position(*pos, n) {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Wheel { id, delta } => {
                if *id == TREE {
                    let n = self.visible().len();
                    return if self.tree.scroll_by(*delta as isize, n) {
                        Route::Changed
                    } else {
                        Route::Ignored
                    };
                }
                if *id == LOG {
                    let _ = self.log.scroll_by(*delta as isize);
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.running
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(LOG) {
            vec![("↑ ↓", "Scroll"), ("f", "Follow"), ("r", "Run")]
        } else if focus == Some(TREE) {
            vec![("↑ ↓", "Move"), ("← →", "Fold")]
        } else {
            vec![("r", "Run pipeline"), ("Enter", "Activate")]
        }
    }
}
