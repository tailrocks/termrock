use crate::screens::Holla;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::ScrollState;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::shell::Page as ShellPage;
use junie_tui::theme::Tone;
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::text::{fit, width, wrap};
use junie_tui::widgets::action_row::ActionRow;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::button::RowAlign;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::page_header::{Heading, PageHeader};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::{ProgressStatus, render_bar, spinner_frame};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use junie_tui::widgets::viewport::{Span, TextViewport};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::domain::plan::{Plan, PlanPhase, StepState};
use crate::screens::{
    Cx, Go, ModalResult, Screen, StatusBits, open_context, row_anchor, scroll_drag, scroll_press,
    ticks_label,
};
use crate::sim::world::World;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::text::{plural, truncate_sep};

pub const OUTLINE: WidgetId = WidgetId::of("plan.outline");
pub const DETAIL: WidgetId = WidgetId::of("plan.detail");
pub const CANCEL: WidgetId = WidgetId::of("plan.cancel");
pub const CONFIRM: WidgetId = WidgetId::of("plan.confirm");
pub const OUTPUT: WidgetId = WidgetId::of("plan.output");
const FOLLOW_UP: WidgetId = WidgetId::of("plan.follow");

pub const STEP_MENU: WidgetId = WidgetId::of("plan.step.menu");

const REVIEW_STEP_KEYS: [char; 5] = [' ', 'u', 'p', 'c', 'x'];

fn plain_key(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::NONE,
    }
}

fn step_at(id: WidgetId, w: &World, plan: &str) -> Option<usize> {
    let n = w.plan(plan).map(|p| p.steps.len()).unwrap_or(0);
    (0..n).find(|&i| OUTLINE.child(i) == id)
}

pub fn lane(p: &Plan, i: usize) -> String {
    fn go(p: &Plan, i: usize, depth: usize) -> String {
        if depth > p.steps.len() {
            return "·".into();
        }
        let deps = &p.steps[i].deps;
        if deps.len() > 1 {
            return "join".into();
        }
        let Some(&parent) = deps.first() else {
            let roots: Vec<usize> = (0..p.steps.len())
                .filter(|&j| p.steps[j].deps.is_empty())
                .collect();
            if roots.len() > 1 {
                let k = roots.iter().position(|&r| r == i).unwrap_or(0);
                return ((b'a' + (k as u8 % 26)) as char).to_string();
            }
            return "·".into();
        };
        let siblings = p.dependents(parent);
        if siblings.len() > 1 {
            let k = siblings.iter().position(|&s| s == i).unwrap_or(0);
            return ((b'a' + (k as u8 % 26)) as char).to_string();
        }
        go(p, parent, depth + 1)
    }
    go(p, i, 0)
}

fn state_glyph(s: StepState, tick: u64) -> (&'static str, Tone, bool) {
    match s {
        StepState::Ready | StepState::Waiting => ("·", Tone::Muted, false),
        StepState::Running => (spinner_frame(tick), Tone::Success, false),
        StepState::Succeeded => ("✓", Tone::Secondary, false),
        StepState::Failed => ("!", Tone::Error, true),
        StepState::Skipped | StepState::Excluded | StepState::Cancelled => {
            ("–", Tone::Faint, false)
        }
        StepState::Blocked => ("·", Tone::Faint, false),
    }
}

fn step_props(p: &Plan, i: usize, w: &World) -> Vec<Prop> {
    let s = &p.steps[i];
    let mut v = vec![];
    for (k, c) in s.commands.iter().enumerate() {
        v.push(Prop::new(if k == 0 { "Runs" } else { "" }, c.clone()).tone(Tone::Secondary));
    }
    v.push(Prop::new(
        "In",
        format!("{} · {}", w.location.short(&s.cwd), p.host),
    ));
    let needs: Vec<String> = s
        .deps
        .iter()
        .map(|&d| format!("{:02} {}", d + 1, p.steps[d].label))
        .collect();
    v.push(
        Prop::new(
            "Needs",
            if needs.is_empty() {
                "nothing · starts first".into()
            } else {
                needs.join(" · ")
            },
        )
        .wrap(),
    );
    let unlocks: Vec<String> = p
        .dependents(i)
        .iter()
        .map(|&d| {
            format!(
                "{:02} {}{}",
                d + 1,
                p.steps[d].label,
                if !p.steps[d].included {
                    " (excluded)"
                } else {
                    ""
                }
            )
        })
        .collect();
    if !unlocks.is_empty() {
        v.push(Prop::new("Unlocks", unlocks.join(" · ")).wrap());
    }
    let l = lane(p, i);
    let beside: Vec<String> = p
        .parallel_with(i)
        .iter()
        .filter(|&&j| lane(p, j) != l && p.steps[j].deps.first() == s.deps.first())
        .map(|&j| format!("{:02}", j + 1))
        .collect();
    let lane_text = if l == "join" {
        "join · waits for every branch it needs".to_owned()
    } else if l == "·" {
        "· trunk · ordered".to_owned()
    } else if beside.is_empty() {
        format!("{l} · ordered")
    } else {
        format!("{l} · may run beside {}", beside.join(", "))
    };
    v.push(Prop::new("Lane", lane_text));
    if let Some(e) = &s.exclusive {
        v.push(Prop::new("Ordered by", e.clone()).tone(Tone::Muted));
    }
    if !s.privilege.is_empty() {
        v.push(Prop::new("Privilege", s.privilege.clone()).tone(Tone::Warning));
    }
    if !s.effects.is_empty() {
        v.push(Prop::new("Effect", s.effects.join(" · ")).wrap());
    }
    if !s.note.is_empty() {
        v.push(Prop::new("Note", s.note.clone()).tone(Tone::Warning).wrap());
    }
    let state = if s.because.is_empty() || s.because == s.state.label() {
        s.state.label().to_owned()
    } else {
        format!("{} · {}", s.state.label(), s.because)
    };
    v.push(Prop::new("State", state).tone(match s.state {
        StepState::Failed => Tone::Error,
        StepState::Blocked | StepState::Excluded => Tone::Muted,
        _ => Tone::Normal,
    }));
    v
}

fn outcome_line(
    ok: usize,
    failed: usize,
    excluded: usize,
    skipped: usize,
    never: usize,
) -> Vec<Span> {
    let parts: Vec<Span> = [
        (ok, "succeeded", Tone::Success),
        (failed, "failed", Tone::Error),
        (skipped, "skipped", Tone::Secondary),
        (never, "never started", Tone::Secondary),
        (excluded, "excluded", Tone::Secondary),
    ]
    .into_iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, w, tone)| Span::new(format!("{n} {w}"), tone))
    .collect();
    if parts.is_empty() {
        return vec![Span::new("nothing ran", Tone::Secondary)];
    }
    let mut line = vec![];
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            line.push(Span::new(" · ", Tone::Secondary));
        }
        line.push(p);
    }
    line
}

fn consequences(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.state == StepState::Blocked && s.included)
        .map(|(i, s)| format!("{:02} {} · {}", i + 1, s.label, s.because))
        .collect();
    if v.is_empty() && p.phase == PlanPhase::Review {
        let required = p.steps.iter().filter(|s| !s.optional).count();
        v.push(format!(
            "{} required · {} optional · {} run in parallel where independent",
            required,
            p.optional_count(),
            plural(p.parallel_branches(), "branch", "branches")
        ));
    }
    v
}

#[allow(clippy::too_many_arguments)]
fn render_outline(
    p: &Plan,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    id: WidgetId,
    cursor: usize,
    scroll: &mut ScrollState,
    review: bool,
) {
    let t = ctx.theme;
    let bg = t.canvas;
    let focused = ctx.interaction.focused(id);
    scroll.set_content(p.steps.len());
    scroll.set_viewport(area.height as usize);
    ctx.control(id, area, false);
    ctx.scrollable(id, area);
    let has_sb = scroll.overflows();
    let row_w = area.width.saturating_sub(u16::from(has_sb));
    let label_w = p.steps.iter().map(|s| width(&s.label)).max().unwrap_or(10) as u16;

    let needs_w = p
        .steps
        .iter()
        .map(|s| {
            if s.deps.is_empty() {
                1
            } else {
                s.deps.len() * 3 - 1
            }
        })
        .max()
        .unwrap_or(2)
        .clamp(5, 14) as u16;
    let lane_w = 4u16;
    let label_col = label_w.min(
        row_w
            .saturating_sub(3 + 4 + 3 + needs_w + 2 + lane_w + 2 + 10)
            .max(12),
    );
    for (k, i) in scroll.visible_range().enumerate() {
        let y = area.y + k as u16;
        let s = &p.steps[i];
        let rid = id.child(i);
        let mut vs = ctx.state(rid);
        vs.focused = focused && i == cursor;
        vs.selected = i == cursor;
        let st = t.row(vs, bg);
        let row = Rect::new(area.x, y, row_w, 1);
        fill(buf, row, st);
        buf.set_string(
            row.x,
            y,
            t.gutter_symbol(vs),
            t.gutter(vs, st.bg.unwrap_or(bg), false),
        );
        let plain = st.remove_modifier(Modifier::BOLD);
        let mut x = row.x + 1;
        if review {
            let quiet = if vs.selected {
                t.text_muted
            } else {
                t.text_faint
            };
            let (mark, ms) = if !s.optional {
                ("[✓]", plain.fg(quiet))
            } else if s.included {
                ("[✓]", plain.fg(ctx.chosen(focused)))
            } else {
                ("[ ]", plain.fg(t.text_muted))
            };
            buf.set_string(x, y, mark, ms);
            x += 4;
        } else {
            let (g, tone, bold) = state_glyph(s.state, ctx.interaction.tick);
            let mut gs = plain.fg(t.tone(tone));
            if bold {
                gs = gs.add_modifier(Modifier::BOLD);
            }
            buf.set_string(x + 1, y, g, gs);
            x += 3;
        }
        let num = format!("{:02}", i + 1);
        buf.set_string(
            x,
            y,
            &num,
            plain.fg(if s.state == StepState::Running || vs.selected {
                t.text_secondary
            } else {
                t.text_faint
            }),
        );
        x += 3;
        let label_style = match s.state {
            StepState::Excluded | StepState::Skipped | StepState::Cancelled => st.fg(t.text_faint),
            StepState::Blocked => st.fg(t.text_faint),
            StepState::Failed => st.fg(t.error).add_modifier(Modifier::BOLD),
            _ => st,
        };
        buf.set_string(x, y, fit(&s.label, label_col as usize), label_style);
        x += label_col + 2;
        let needs = if s.deps.is_empty() {
            "–".to_owned()
        } else {
            s.deps
                .iter()
                .map(|d| format!("{:02}", d + 1))
                .collect::<Vec<_>>()
                .join(" ")
        };
        if x + needs_w <= row.right() {
            buf.set_string(x, y, fit(&needs, needs_w as usize), plain.fg(t.text_muted));
        }
        x += needs_w + 2;
        let l = lane(p, i);
        if x + lane_w <= row.right() {
            buf.set_string(x, y, fit(&l, lane_w as usize), plain.fg(t.text_secondary));
        }
        x += lane_w + 2;
        let meta = if review {
            if !s.included {
                "excluded".to_owned()
            } else if s.state == StepState::Blocked {
                s.because.clone()
            } else if s.optional {
                "optional".to_owned()
            } else if !s.privilege.is_empty() {
                format!("required · {}", s.privilege)
            } else {
                "required".to_owned()
            }
        } else {
            match s.state {
                StepState::Running => ticks_label(
                    ctx.interaction
                        .tick
                        .saturating_sub(s.started_tick.unwrap_or(0)),
                ),
                StepState::Succeeded | StepState::Failed => {
                    let d = s
                        .ended_tick
                        .unwrap_or(0)
                        .saturating_sub(s.started_tick.unwrap_or(0));

                    let reason = if s.state == StepState::Failed {
                        s.output
                            .iter()
                            .rev()
                            .find(|l| l.contains("ERROR") || l.contains("mismatch"))
                            .map(|l| {
                                format!(
                                    " · {}",
                                    l.split_once("ERROR ").map(|(_, r)| r).unwrap_or(l)
                                )
                            })
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    format!(
                        "{}{}{reason}",
                        ticks_label(d),
                        s.exit
                            .filter(|e| *e != 0)
                            .map(|e| format!(" · exit {e}"))
                            .unwrap_or_default()
                    )
                }
                StepState::Blocked | StepState::Excluded | StepState::Cancelled => {
                    s.because.clone()
                }
                StepState::Waiting => s
                    .deps
                    .iter()
                    .find(|&&d| !p.steps[d].state.finished())
                    .map(|d| format!("waits {:02}", d + 1))
                    .unwrap_or("waiting".into()),
                StepState::Ready => "ready".into(),
                StepState::Skipped => "skipped".into(),
            }
        };

        let meta_tone = match s.state {
            StepState::Failed => t.error,
            StepState::Running => t.text_secondary,
            _ if vs.selected => t.text_muted,
            _ => t.text_faint,
        };
        let avail = row.right().saturating_sub(x + 1) as usize;
        if avail >= 6 && !meta.is_empty() {
            buf.set_string(x, y, truncate_sep(&meta, avail), plain.fg(meta_tone));
        }
        legible_on_fill(buf, row, t, bg);
        ctx.clickable(rid, row);
    }
    if has_sb {
        scrollbar::render_with_fade(
            Rect::new(area.x, area.y, area.width, area.height),
            buf,
            ctx,
            id,
            scroll,
            focused,
        );
    }
}

pub struct PlanReviewPage {
    pub plan: String,
    pub cursor: usize,
    scroll: ScrollState,
    detail_scroll: ScrollState,
    outline_area: Rect,
    detail_area: Rect,
    undo: Vec<usize>,
    cancel: Button,
    confirm: Button,
    drawer: bool,
}

impl PlanReviewPage {
    pub fn new(plan: &str) -> Self {
        Self {
            plan: plan.into(),
            cursor: 0,
            scroll: ScrollState::default(),
            detail_scroll: ScrollState::default(),
            outline_area: Rect::ZERO,
            detail_area: Rect::ZERO,
            undo: vec![],
            cancel: Button::subtle(CANCEL, "Cancel"),
            confirm: Button::primary(CONFIRM, "Confirm plan"),
            drawer: false,
        }
    }

    fn render_detail(
        &mut self,
        p: &Plan,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
    ) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(DETAIL);
        let s = &p.steps[self.cursor.min(p.steps.len().saturating_sub(1))];
        let title = format!("{:02} {}", self.cursor + 1, s.label);
        let panel = Panel::card(Some(&title))
            .focused(focused)
            .meta(if s.optional { "optional" } else { "required" });
        let bg = panel.bg(t);
        let props = step_props(p, self.cursor, w);

        let label_w = props.iter().map(|p| width(&p.label)).max().unwrap_or(4) as u16 + 2;
        let vw = area.width.saturating_sub(4 + label_w) as usize;
        let mut flat: Vec<(String, String, Tone)> = vec![];
        for pr in props {
            for (i, part) in wrap(&pr.value, vw.max(8)).into_iter().enumerate() {
                flat.push((
                    if i == 0 {
                        pr.label.clone()
                    } else {
                        String::new()
                    },
                    part,
                    pr.tone,
                ));
            }
        }
        let area = Rect::new(
            area.x,
            area.y,
            area.width,
            (flat.len() as u16 + 3).min(area.height),
        );
        let inner = panel.render(area, buf, t);
        self.detail_area = inner;
        ctx.control(DETAIL, area, false);
        ctx.scrollable(DETAIL, inner);
        self.detail_scroll.set_content(flat.len());
        self.detail_scroll.set_viewport(inner.height as usize);
        for (k, i) in self.detail_scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (l, v, tone) = &flat[i];
            buf.set_string(inner.x, y, l, t.muted().bg(bg));
            buf.set_string(
                inner.x + label_w,
                y,
                junie_tui::ui::reveal::cut(Rect::new(inner.x + label_w, y, (vw) as u16, 1), v),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
        }
        if self.detail_scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                DETAIL,
                &self.detail_scroll,
                focused,
            );
        }
    }
}

impl PlanReviewPage {
    fn step_menu(&self, i: usize, anchor: ContextAnchor, w: &World, cx: &mut Cx) -> Outcome {
        let Some(s) = w.plan(&self.plan).and_then(|p| p.steps.get(i)) else {
            return Outcome::Ignored;
        };
        let mut toggle = MenuItem::new(if s.included {
            "Exclude step"
        } else {
            "Include step"
        })
        .shortcut("Space");
        if !s.optional {
            toggle = toggle.reason("Required by the plan");
        }
        let mut undo = MenuItem::new("Undo last change").shortcut("u");
        if self.undo.is_empty() {
            undo = undo.reason("Nothing to undo");
        }
        let items = vec![
            toggle,
            undo,
            MenuItem::new(if self.drawer {
                "Hide details"
            } else {
                "Show details"
            })
            .shortcut("p")
            .separator(),
            MenuItem::new("Review & run…").shortcut("c"),
            MenuItem::new("Discard plan").shortcut("x").danger(),
        ];
        let title = format!("{:02} {}", i + 1, s.label);
        open_context(
            cx,
            STEP_MENU,
            anchor,
            title,
            items,
            LayerTag::new("plan-step").key(i.to_string()),
        )
    }
}

impl ShellPage<Holla> for PlanReviewPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let Some(p) = w.plan_mut(&self.plan) else {
            return Outcome::Ignored;
        };
        let n = p.steps.len();
        if cx.focus.is(CANCEL) {
            let (o, fired) = self.cancel.on_key(key);
            if fired {
                cx.go(Go::Pop);
                cx.status("Plan discarded · nothing was executed");
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if cx.focus.is(CONFIRM) {
            let (o, fired) = self.confirm.on_key(key);
            if fired {
                cx.go(Go::ConfirmPlan(self.plan.clone()));
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if cx.focus.is(DETAIL) {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_scroll.scroll_by(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll.scroll_by(1);
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    self.drawer = false;
                    cx.focus.focus(OUTLINE);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if cx.focus.is(OUTLINE) => {
                self.cursor = self.cursor.saturating_sub(1);
                self.detail_scroll.jump_start();
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') if cx.focus.is(OUTLINE) => {
                self.cursor = (self.cursor + 1).min(n.saturating_sub(1));
                self.detail_scroll.jump_start();
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.cursor = 0;
                self.scroll.jump_start();
                Outcome::Changed
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.cursor = n.saturating_sub(1);
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Char(' ') => {
                let i = self.cursor;
                if p.toggle(i) {
                    self.undo.push(i);
                    let s = &p.steps[i];
                    let blocked: Vec<String> = p
                        .steps
                        .iter()
                        .enumerate()
                        .filter(|(_, x)| x.state == StepState::Blocked && x.included)
                        .map(|(j, _)| format!("{:02}", j + 1))
                        .collect();
                    let msg = if s.included {
                        format!("{:02} included", i + 1)
                    } else if blocked.is_empty() {
                        format!("{:02} excluded · nothing depends on it", i + 1)
                    } else {
                        format!("{:02} excluded · {} now blocked", i + 1, blocked.join(", "))
                    };
                    cx.status(msg);
                } else {
                    cx.status(format!(
                        "{:02} {} is required by the plan and cannot be excluded",
                        i + 1,
                        p.steps[i].label
                    ));
                }
                Outcome::Changed
            }
            KeyCode::Char('u') => {
                if let Some(i) = self.undo.pop() {
                    p.toggle(i);
                    cx.status(format!("{:02} restored", i + 1));
                } else {
                    cx.status("Nothing to undo");
                }
                Outcome::Changed
            }
            KeyCode::Char('c') => {
                cx.go(Go::ConfirmPlan(self.plan.clone()));
                Outcome::Changed
            }
            KeyCode::Char('p') => {
                self.drawer = !self.drawer;
                cx.focus.focus(if self.drawer { DETAIL } else { OUTLINE });
                Outcome::Changed
            }
            KeyCode::Enter if cx.focus.is(OUTLINE) => {
                cx.focus.focus(CONFIRM);
                Outcome::Changed
            }
            KeyCode::Char('x') => {
                cx.go(Go::Pop);
                cx.status("Plan discarded · nothing was executed");
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            if axis != Axis::Vertical {
                return Outcome::Ignored;
            }
            if id == OUTLINE {
                self.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if id == DETAIL {
                self.detail_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        let n = w.plan(&self.plan).map(|p| p.steps.len()).unwrap_or(0);
        let step = (0..n).find(|&i| OUTLINE.child(i) == id);
        match ev.phase() {
            Some(Phase::Press) => {
                if id == scrollbar::id_for(OUTLINE) {
                    return scroll_press(self.outline_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(DETAIL) {
                    return scroll_press(self.detail_area, pos, &mut self.detail_scroll);
                }
                Outcome::Ignored
            }
            Some(Phase::Drag) => {
                if id == scrollbar::id_for(OUTLINE) {
                    return scroll_drag(self.outline_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(DETAIL) {
                    return scroll_drag(self.detail_area, pos, &mut self.detail_scroll);
                }
                Outcome::Ignored
            }
            Some(Phase::Click) => {
                if let Some(i) = step
                    && ClickPolicy::DATA.activates(ev.count(), self.cursor == i)
                    && let Some(p) = w.plan_mut(&self.plan)
                {
                    self.cursor = i;
                    if p.toggle(i) {
                        self.undo.push(i);
                    } else {
                        cx.status(format!("{:02} is required and cannot be excluded", i + 1));
                    }
                    return Outcome::Changed;
                }
                if id == CANCEL {
                    cx.go(Go::Pop);
                    return Outcome::Changed;
                }
                if id == CONFIRM {
                    cx.go(Go::ConfirmPlan(self.plan.clone()));
                    return Outcome::Changed;
                }
                if id == scrollbar::id_for(OUTLINE) {
                    cx.focus.focus(OUTLINE);
                    return scroll_press(self.outline_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(DETAIL) {
                    cx.focus.focus(DETAIL);
                    return scroll_press(self.detail_area, pos, &mut self.detail_scroll);
                }
                if id == DETAIL {
                    cx.focus.focus(DETAIL);
                    return Outcome::Changed;
                }
                if let Some(i) = step {
                    self.cursor = i;
                    cx.focus.focus(OUTLINE);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            Some(Phase::Secondary) => {
                let Some(i) = step_at(id, w, &self.plan) else {
                    return Outcome::Ignored;
                };
                if cx.context.moves_cursor {
                    self.cursor = i;
                }
                cx.focus.focus(OUTLINE);
                self.step_menu(i, ContextAnchor::Pointer(pos), w, cx)
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if tag.kind == "plan-step"
            && let ModalResult::MenuChosen(k) = result
            && let Ok(i) = tag.key.parse::<usize>()
            && let Some(&c) = REVIEW_STEP_KEYS.get(k)
        {
            self.cursor = i;
            return self.on_key(&plain_key(c), w, cx).or(Outcome::Changed);
        }
        Outcome::Changed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let Some(p) = w.plan(&self.plan).cloned() else {
            return;
        };
        self.cursor = self.cursor.min(p.steps.len().saturating_sub(1));

        let summary = format!(
            "{} · {} included · {} excluded · ~{}",
            plural(p.steps.len(), "step", "steps"),
            p.included_count(),
            p.steps.len() - p.included_count(),
            ticks_label(p.estimate_ticks())
        );
        PageHeader::new(&p.title)
            .status(&format!(" · {}", p.host), Tone::Secondary)
            .right(&summary)
            .subtitle(&p.intent, Tone::Muted)
            .render(area, buf, t);
        let split = area.width >= junie_tui::ui::master_detail::SPLIT_MIN;
        let buttons_y = area.bottom().saturating_sub(1);
        let cons = consequences(&p);
        let cons_h = cons.len().min(2) as u16;
        let body = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            buttons_y.saturating_sub(area.y + 3 + cons_h + 2),
        );
        let (outline_area, detail_area) = if split {
            let dw = (area.width * 40 / 100).clamp(36, 54);
            (
                Rect::new(
                    body.x,
                    body.y,
                    body.width.saturating_sub(dw + 2),
                    body.height,
                ),
                Some(Rect::new(
                    body.right().saturating_sub(dw),
                    body.y,
                    dw,
                    body.height,
                )),
            )
        } else {
            (body, None)
        };

        Heading::new("step").render(
            Rect::new(
                outline_area.x + 8,
                outline_area.y,
                outline_area.width.saturating_sub(8),
                1,
            ),
            buf,
            t,
            t.canvas,
        );
        {
            let label_w = p.steps.iter().map(|s| width(&s.label)).max().unwrap_or(10) as u16;
            let needs_w = p
                .steps
                .iter()
                .map(|s| {
                    if s.deps.is_empty() {
                        1
                    } else {
                        s.deps.len() * 3 - 1
                    }
                })
                .max()
                .unwrap_or(2)
                .clamp(5, 14) as u16;
            let label_col = label_w.min(
                outline_area
                    .width
                    .saturating_sub(3 + 4 + 3 + needs_w + 2 + 4 + 2 + 10)
                    .max(12),
            );
            let x = outline_area.x + 8 + label_col + 2;
            if x + needs_w + 6 < outline_area.right() {
                buf.set_string(x, outline_area.y, "needs", t.faint());
                buf.set_string(x + needs_w + 2, outline_area.y, "lane", t.faint());
                let nx = x + needs_w + 2 + 4 + 2;
                if nx + 4 < outline_area.right() {
                    buf.set_string(nx, outline_area.y, "note", t.faint());
                }
            }
        }
        let rows = Rect::new(
            outline_area.x,
            outline_area.y + 1,
            outline_area.width,
            outline_area.height.saturating_sub(1),
        );
        self.outline_area = Rect::ZERO;
        self.detail_area = Rect::ZERO;
        if let Some(d) = detail_area {
            if self.drawer && !split {
                self.render_detail(&p, body, buf, ctx, w);
            } else {
                render_outline(
                    &p,
                    rows,
                    buf,
                    ctx,
                    OUTLINE,
                    self.cursor,
                    &mut self.scroll,
                    true,
                );
                self.outline_area = rows;
                self.render_detail(&p, d, buf, ctx, w);
            }
        } else if self.drawer {
            ctx.control(OUTLINE, Rect::ZERO, false);
            self.render_detail(&p, body, buf, ctx, w);
        } else {
            render_outline(
                &p,
                rows,
                buf,
                ctx,
                OUTLINE,
                self.cursor,
                &mut self.scroll,
                true,
            );
            self.outline_area = rows;
            ctx.control(DETAIL, Rect::ZERO, false);
        }

        let drawer_open = self.drawer && !(detail_area.is_some() && split);
        let cy = (rows.y + p.steps.len() as u16 + 1).min(body.bottom() + 1);
        for (i, c) in cons.iter().take(2).enumerate().filter(|_| !drawer_open) {
            let tone = if c.contains("blocked") {
                t.warning
            } else {
                t.text_muted
            };
            buf.set_string(
                area.x + 3,
                cy + i as u16,
                junie_tui::ui::reveal::cut(
                    Rect::new(area.x + 3, cy + i as u16, area.width.saturating_sub(4), 1),
                    c,
                ),
                Style::new().fg(tone),
            );
        }

        let label = if p.phrase.is_some() {
            "Continue…"
        } else {
            "Confirm plan"
        };
        self.confirm = if p.phrase.is_some() {
            Button::danger(CONFIRM, label)
        } else {
            Button::primary(CONFIRM, label)
        };
        ActionRow::new().align(RowAlign::End).render(
            Rect::new(area.x, buttons_y, area.width.saturating_sub(1), 1),
            buf,
            ctx,
            t.canvas,
            &mut [&mut self.cancel, &mut self.confirm],
        );
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if focus == Some(DETAIL) {
            return vec![hint("↑↓", "Scroll"), hint("Esc", "Back to outline")];
        }
        if let Some(b) = [&self.cancel, &self.confirm]
            .into_iter()
            .find(|b| Some(b.id) == focus)
        {
            return vec![hint("←→", "Choose"), b.hint(), hint("Esc", "Discard plan")];
        }
        vec![
            hint("↑↓", "Step"),
            hint("Space", "Toggle"),
            hint("u", "Undo"),
            hint("c", "Review & run…"),
            hint("p", "Details"),
            hint("Esc", "Discard plan"),
            junie_tui::widgets::keyhint::CONTEXT_MENU,
        ]
    }

    fn crumb(&self, w: &World) -> String {
        w.plan(&self.plan)
            .map(|p| p.title.clone())
            .unwrap_or("Plan".into())
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(OUTLINE)
    }
}

impl Screen for PlanReviewPage {
    fn on_context_key(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if !cx.focus.is(OUTLINE) {
            return Outcome::Ignored;
        }
        let anchor = row_anchor(OUTLINE, self.cursor, self.outline_area, self.scroll.offset);
        self.step_menu(self.cursor, anchor, w, cx)
    }

    fn status(&self, w: &World) -> StatusBits {
        let mut bits = StatusBits::default();
        if let Some(p) = w.plan(&self.plan) {
            bits.center = Some(
                StatusItem::new(
                    format!(
                        "review · {} · {} parallel",
                        plural(p.included_count(), "step", "steps"),
                        plural(p.parallel_branches(), "branch", "branches")
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            );
        }
        bits
    }
}

pub struct PlanTab {
    pub plan: String,
    pub cursor: usize,
    scroll: ScrollState,
    view: TextViewport,
    outline_area: Rect,
    rendered: (usize, usize),
    pub maximized: bool,
    follow_ups: Vec<Button>,

    landed: bool,
}

impl PlanTab {
    pub fn new(plan: &str) -> Self {
        Self {
            plan: plan.into(),
            cursor: 0,
            scroll: ScrollState::default(),
            view: TextViewport::new(OUTPUT).max_lines(2000).wrap(true),
            outline_area: Rect::ZERO,
            rendered: (usize::MAX, usize::MAX),
            maximized: false,
            follow_ups: vec![],
            landed: false,
        }
    }

    fn follow_up(&self, i: usize, w: &World, cx: &mut Cx) {
        if let Some((_, item)) = w.plan(&self.plan).and_then(|p| p.follow_ups.get(i)) {
            cx.go(Go::Run {
                item: item.clone(),
                args: vec![],
            });
        }
    }

    fn sync(&mut self, w: &World) {
        let Some(p) = w.plan(&self.plan) else {
            return;
        };

        if p.phase == PlanPhase::Done && !self.landed {
            self.landed = true;
            let target = p
                .steps
                .iter()
                .position(|s| s.state == StepState::Failed)
                .or_else(|| {
                    p.steps
                        .iter()
                        .rposition(|s| s.state == StepState::Succeeded)
                });
            if let Some(i) = target {
                self.cursor = i;
                self.view.follow = true;
            }
        }
        if self.follow_ups.len() != p.follow_ups.len() {
            self.follow_ups = p
                .follow_ups
                .iter()
                .enumerate()
                .map(|(i, (label, _))| Button::subtle(FOLLOW_UP.child(i), label))
                .collect();
        }
        let i = self.cursor.min(p.steps.len().saturating_sub(1));
        let s = &p.steps[i];
        if self.rendered != (i, s.output.len()) {
            let lines = s
                .output
                .iter()
                .map(|l| {
                    let tone = if l.starts_with('$') {
                        Tone::Secondary
                    } else if l.contains("ERROR")
                        || l.starts_with("exit status")
                        || l.contains("mismatch")
                    {
                        Tone::Error
                    } else {
                        Tone::Normal
                    };
                    vec![Span::new(l.clone(), tone)]
                })
                .collect();
            let follow = self.view.follow;
            self.view.set_lines(lines);
            self.view.follow = follow;
            self.rendered = (i, s.output.len());
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RunStep {
    Expand,
    Retry,
    Skip,
    Cancel,
    Close,
}

const RUN_STEP_ACTIONS: [RunStep; 5] = [
    RunStep::Expand,
    RunStep::Retry,
    RunStep::Skip,
    RunStep::Cancel,
    RunStep::Close,
];

impl PlanTab {
    fn retry_step(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        let i = self.cursor;
        let Some(p) = w.plan_mut(&self.plan) else {
            return Outcome::Ignored;
        };
        if p.retry(i) {
            cx.status(format!(
                "{:02} retrying · succeeded work is not repeated",
                i + 1
            ));
        } else {
            cx.status("Only a failed step can be retried");
        }
        Outcome::Changed
    }

    fn skip_step(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        let i = self.cursor;
        let Some(p) = w.plan_mut(&self.plan) else {
            return Outcome::Ignored;
        };
        if p.skip(i) {
            cx.status(format!(
                "{:02} skipped · its dependents stay blocked",
                i + 1
            ));
        } else {
            cx.status("Only an optional failed step can be skipped");
        }
        Outcome::Changed
    }

    fn step_menu(&self, i: usize, anchor: ContextAnchor, w: &World, cx: &mut Cx) -> Outcome {
        let Some(p) = w.plan(&self.plan) else {
            return Outcome::Ignored;
        };
        let Some(s) = p.steps.get(i) else {
            return Outcome::Ignored;
        };
        let failed = s.state == StepState::Failed;
        let mut retry = MenuItem::new("Retry step").shortcut("r");
        if !failed {
            retry = retry.reason("Only a failed step can be retried");
        }
        let mut skip = MenuItem::new("Skip step").shortcut("k").separator();
        if !(failed && s.optional) {
            skip = skip.reason("Only an optional failed step can be skipped");
        }
        let mut cancel = MenuItem::new("Cancel remaining…")
            .shortcut("Ctrl+C")
            .danger();
        if p.phase != PlanPhase::Running {
            cancel = cancel.reason("The plan is not running");
        }
        let items = vec![
            MenuItem::new(if self.maximized {
                "Restore layout"
            } else {
                "Expand output"
            })
            .shortcut("Enter"),
            retry,
            skip,
            cancel,
            MenuItem::new("Close tab").shortcut("x"),
        ];
        let title = format!("{:02} {}", i + 1, s.label);
        open_context(
            cx,
            STEP_MENU,
            anchor,
            title,
            items,
            LayerTag::new("plan-step").key(i.to_string()),
        )
    }
}

impl ShellPage<Holla> for PlanTab {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        for i in 0..self.follow_ups.len() {
            if cx.focus.is(self.follow_ups[i].id) {
                let (o, fired) = self.follow_ups[i].on_key(key);
                if fired {
                    self.follow_up(i, w, cx);
                    return Outcome::Changed;
                }
                if o != Outcome::Ignored {
                    return o;
                }
            }
        }
        let Some(p) = w.plan_mut(&self.plan) else {
            return Outcome::Ignored;
        };
        let n = p.steps.len();
        if cx.focus.is(OUTPUT) {
            match key.code {
                KeyCode::Esc if self.maximized => {
                    self.maximized = false;
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    cx.focus.focus(OUTLINE);
                    return Outcome::Changed;
                }
                _ => {}
            }
            let (o, ev) = self.view.on_key(key);
            if let Some(junie_tui::widgets::viewport::ViewportAction::Copy(text)) = ev {
                cx.copy(text);
            }
            if o.consumed() {
                return o;
            }
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if cx.focus.is(OUTLINE) => {
                self.cursor = self.cursor.saturating_sub(1);
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') if cx.focus.is(OUTLINE) => {
                self.cursor = (self.cursor + 1).min(n.saturating_sub(1));
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Enter | KeyCode::Char('z') => {
                self.maximized = !self.maximized;
                cx.focus
                    .focus(if self.maximized { OUTPUT } else { OUTLINE });
                Outcome::Changed
            }
            KeyCode::Char('r') => self.retry_step(w, cx),
            KeyCode::Char('k') => self.skip_step(w, cx),
            KeyCode::Char('f') => {
                self.view.set_follow(!self.view.follow);
                Outcome::Changed
            }
            KeyCode::Char('x') => {
                cx.go(Go::CloseTab);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if tag.kind == "plan-step" {
            if let ModalResult::MenuChosen(k) = result
                && let Ok(i) = tag.key.parse::<usize>()
                && let Some(&action) = RUN_STEP_ACTIONS.get(k)
            {
                self.cursor = i;
                let key = |code, mods| Key { code, mods };
                return match action {
                    RunStep::Expand => {
                        self.on_key(&key(KeyCode::Char('z'), KeyModifiers::NONE), w, cx)
                    }
                    RunStep::Retry => self.retry_step(w, cx),
                    RunStep::Skip => self.skip_step(w, cx),
                    RunStep::Cancel => self.on_interrupt(w, cx),
                    RunStep::Close => self.on_key(&plain_key('x'), w, cx),
                }
                .or(Outcome::Changed);
            }
            return Outcome::Changed;
        }
        if tag.kind == "cancel-plan"
            && let ModalResult::Dialog {
                action: Some(1), ..
            } = result
        {
            let tick = w.tick;
            if let Some(p) = w.plan_mut(&self.plan) {
                p.cancel_remaining();
                if p.running().is_empty() {
                    p.phase = PlanPhase::Done;
                    p.ended_tick = Some(tick);
                }
            }
            cx.status("Remaining steps cancelled · running steps finish");
        }
        Outcome::Changed
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            if id == OUTLINE {
                if axis != Axis::Vertical {
                    return Outcome::Ignored;
                }
                self.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return self
                .view
                .route(ev)
                .map_or(Outcome::Ignored, |r| r.outcome());
        }
        match ev.phase() {
            Some(Phase::Press) => {
                if id == scrollbar::id_for(OUTLINE) {
                    return scroll_press(self.outline_area, pos, &mut self.scroll);
                }
                self.view
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            Some(Phase::Drag) => {
                if id == scrollbar::id_for(OUTLINE) {
                    return scroll_drag(self.outline_area, pos, &mut self.scroll);
                }
                self.view
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            Some(Phase::Click) => {
                for i in 0..self.follow_ups.len() {
                    if self.follow_ups[i].id == id {
                        cx.focus.focus(id);
                        if self.follow_ups[i].on_click() {
                            self.follow_up(i, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                let n = w.plan(&self.plan).map(|p| p.steps.len()).unwrap_or(0);
                if let Some(i) = (0..n).find(|&i| OUTLINE.child(i) == id) {
                    self.cursor = i;
                    cx.focus.focus(OUTLINE);
                    return Outcome::Changed;
                }
                if id == scrollbar::id_for(OUTLINE) {
                    return scroll_press(self.outline_area, pos, &mut self.scroll);
                }
                if id == OUTPUT {
                    cx.focus.focus(OUTPUT);
                }
                self.view
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            Some(Phase::Secondary) => {
                let Some(i) = step_at(id, w, &self.plan) else {
                    return Outcome::Ignored;
                };
                if cx.context.moves_cursor {
                    self.cursor = i;
                }
                cx.focus.focus(OUTLINE);
                self.step_menu(i, ContextAnchor::Pointer(pos), w, cx)
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        let before = self.rendered;
        self.sync(w);
        if before != self.rendered
            || w.plan(&self.plan)
                .is_some_and(|p| p.phase == PlanPhase::Running)
        {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn enter(&mut self, w: &mut World, _cx: &mut Cx) {
        self.sync(w);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.sync(w);
        let t = ctx.theme;
        let Some(p) = w.plan(&self.plan).cloned() else {
            return;
        };
        self.cursor = self.cursor.min(p.steps.len().saturating_sub(1));
        let (ok, failed, excluded, skipped, never) = p.outcome();
        let included = p.included_count();
        let running = p.running();

        buf.set_string(area.x + 1, area.y, &p.title, t.title());
        let next_wait = p
            .steps
            .iter()
            .enumerate()
            .find(|(_, s)| s.state == StepState::Waiting)
            .and_then(|(i, s)| {
                s.deps
                    .iter()
                    .find(|&&d| !p.steps[d].state.finished())
                    .map(|d| format!("{:02} waits for {:02}", i + 1, d + 1))
            });
        let blocked = p
            .steps
            .iter()
            .filter(|s| s.state == StepState::Blocked && s.included)
            .count();
        let mut mid = format!(" · {ok} of {included} done");
        if !running.is_empty() {
            mid.push_str(&format!(" · {} running", running.len()));
        }
        if blocked > 0 {
            mid.push_str(&format!(" · {blocked} blocked"));
        }
        if failed > 0 {
            mid.push_str(&format!(" · {failed} failed"));
        }
        if let Some(nw) = next_wait
            && p.phase == PlanPhase::Running
        {
            mid.push_str(&format!(" · {nw}"));
        }

        let mx = area.x + 1 + width(&p.title) as u16;
        match mid
            .find(" · ")
            .filter(|_| failed > 0)
            .and_then(|_| mid.find(&format!("{failed} failed")))
        {
            Some(at) => {
                buf.set_string(mx, area.y, &mid[..at], t.secondary());
                let fx = mx + width(&mid[..at]) as u16;
                let clause = format!("{failed} failed");
                buf.set_string(fx, area.y, &clause, t.error_fg());
                buf.set_string(
                    fx + width(&clause) as u16,
                    area.y,
                    &mid[at + clause.len()..],
                    t.secondary(),
                );
            }
            None => buf.set_string(mx, area.y, &mid, t.secondary()),
        }
        let elapsed = ticks_label(
            p.ended_tick
                .unwrap_or(w.tick)
                .saturating_sub(p.started_tick.unwrap_or(w.tick)),
        );
        let right = match p.phase {
            PlanPhase::Running => format!("{} {elapsed}", spinner_frame(ctx.interaction.tick)),
            PlanPhase::Done => format!("finished · {elapsed} · {}", p.host),
            PlanPhase::Review => "not started".into(),
        };
        let rw = width(&right) as u16;
        buf.set_string(
            area.right().saturating_sub(rw + 1),
            area.y,
            &right,
            if p.phase == PlanPhase::Running {
                t.accent_fg()
            } else {
                t.muted()
            },
        );

        let ratio = if included == 0 {
            0.0
        } else {
            ok as f64 / included as f64
        };
        let status = if p.phase == PlanPhase::Done {
            if failed > 0 {
                ProgressStatus::Error
            } else {
                ProgressStatus::Done
            }
        } else {
            ProgressStatus::Active
        };
        render_bar(
            Rect::new(
                area.x + 1,
                area.y + 1,
                area.width.saturating_sub(2).min(60),
                1,
            ),
            buf,
            ctx,
            "",
            ratio,
            status,
            t.canvas,
        );
        let has_follow = p.phase == PlanPhase::Done && !self.follow_ups.is_empty();

        let follow_w: Vec<u16> = self.follow_ups.iter().map(|b| b.width()).collect();
        let follow_lines = if has_follow {
            junie_tui::widgets::button::row_lines(area.width.saturating_sub(7), &follow_w, 2)
        } else {
            0
        };
        let summary_h: u16 = if p.phase == PlanPhase::Done {
            2 + follow_lines
        } else {
            0
        };
        let body = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3 + summary_h),
        );
        let split = area.width >= junie_tui::ui::master_detail::SPLIT_MIN && !self.maximized;
        let rail_area = if split {
            Rect::new(body.x, body.y, (body.width * 56 / 100).max(40), body.height)
        } else {
            body
        };
        let out_area = if split {
            Rect::new(
                rail_area.right() + 2,
                body.y,
                body.width.saturating_sub(rail_area.width + 2),
                body.height,
            )
        } else {
            body
        };
        let s = &p.steps[self.cursor];
        let out_title = format!("{:02} {}", self.cursor + 1, s.label);
        let out_meta = if s.state == StepState::Running {
            ticks_label(w.tick.saturating_sub(s.started_tick.unwrap_or(w.tick)))
        } else {
            s.state.label().to_owned()
        };
        if self.maximized {
            self.outline_area = Rect::ZERO;
            ctx.control(OUTLINE, Rect::ZERO, false);
            let panel = Panel::framed(Some(&out_title))
                .focused(ctx.interaction.focused(OUTPUT))
                .meta(&out_meta);
            let inner = panel.render(out_area, buf, t);
            self.view.render(inner, buf, ctx, t.canvas);
        } else if split {
            Heading::new("step").render(
                Rect::new(
                    rail_area.x + 7,
                    rail_area.y,
                    rail_area.width.saturating_sub(7),
                    1,
                ),
                buf,
                t,
                t.canvas,
            );
            let rows = Rect::new(
                rail_area.x,
                rail_area.y + 1,
                rail_area.width,
                rail_area.height.saturating_sub(1),
            );
            self.outline_area = rows;
            render_outline(
                &p,
                rows,
                buf,
                ctx,
                OUTLINE,
                self.cursor,
                &mut self.scroll,
                false,
            );
            let panel = Panel::framed(Some(&out_title))
                .focused(ctx.interaction.focused(OUTPUT))
                .meta(&out_meta);
            let inner = panel.render(out_area, buf, t);
            self.view.render(inner, buf, ctx, t.canvas);
        } else {
            self.outline_area = rail_area;
            render_outline(
                &p,
                rail_area,
                buf,
                ctx,
                OUTLINE,
                self.cursor,
                &mut self.scroll,
                false,
            );
            ctx.control(OUTPUT, Rect::ZERO, false);
        }
        if summary_h > 0 {
            let y = area.bottom().saturating_sub(2 + follow_lines - 1);
            let line = outcome_line(ok, failed, excluded, skipped, never);
            junie_tui::widgets::viewport::render_spans(
                buf,
                area.x + 1,
                y,
                area.width.saturating_sub(2),
                t,
                &line,
                t.canvas,
            );
            if has_follow {
                let y = area.bottom().saturating_sub(follow_lines);
                buf.set_string(area.x + 1, y, "Next", t.faint());
                let rects = junie_tui::widgets::button::row_layout(
                    Rect::new(area.x + 7, y, area.width.saturating_sub(7), 1),
                    &follow_w,
                    2,
                );
                for (b, r) in self.follow_ups.iter_mut().zip(rects) {
                    b.render(r, buf, ctx, t.canvas);
                }
            } else {
                for b in &mut self.follow_ups {
                    ctx.control(b.id, Rect::ZERO, false);
                }
            }
        } else {
            for b in &mut self.follow_ups {
                ctx.control(b.id, Rect::ZERO, false);
            }
        }
        let _ = props::render;
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        let p = w.plan(&self.plan);
        let failed_here = p.is_some_and(|p| {
            p.steps
                .get(self.cursor)
                .is_some_and(|s| s.state == StepState::Failed)
        });
        if focus == Some(OUTPUT) {
            let mut v = vec![hint("↑↓", "Scroll"), hint("f", "Follow"), hint("y", "Copy")];
            v.push(if self.maximized {
                hint("Esc", "Restore layout")
            } else {
                hint("Esc", "Back to steps")
            });
            return v;
        }
        let mut v = vec![hint("↑↓", "Step"), hint("Enter", "Expand output")];
        if failed_here {
            v.push(hint("r", "Retry"));
            v.push(hint("k", "Skip"));
        }
        if p.is_some_and(|p| p.phase == PlanPhase::Running) {
            v.push(hint("Ctrl+C", "Cancel remaining"));
        }
        v.push(hint("x", "Close"));
        v.push(hint("Esc", "Back to Here"));
        v
    }

    fn crumb(&self, w: &World) -> String {
        w.plan(&self.plan)
            .map(|p| p.title.clone())
            .unwrap_or_default()
    }

    fn animating(&self, w: &World) -> bool {
        w.plan(&self.plan)
            .is_some_and(|p| p.phase == PlanPhase::Running)
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(OUTLINE)
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if self.maximized {
            self.maximized = false;
            return Outcome::Changed;
        }
        cx.go(Go::Here);
        Outcome::Changed
    }

    fn on_interrupt(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        let running = w
            .plan_mut(&self.plan)
            .is_some_and(|p| p.phase == PlanPhase::Running);
        if !running {
            return Outcome::Ignored;
        }
        let d = Dialog::destructive(
            WidgetId::of("cancel-plan"),
            "Cancel the remaining steps?",
            "Running steps finish; ready and waiting steps are cancelled. Completed work stays.",
            "Cancel remaining",
        )
        .cancel_label("Keep running");
        cx.open(d, LayerTag::new("cancel-plan"));
        Outcome::Changed
    }
}

impl Screen for PlanTab {
    fn on_context_key(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if !cx.focus.is(OUTLINE) {
            return Outcome::Ignored;
        }
        let anchor = row_anchor(OUTLINE, self.cursor, self.outline_area, self.scroll.offset);
        self.step_menu(self.cursor, anchor, w, cx)
    }

    fn status(&self, w: &World) -> StatusBits {
        let mut bits = StatusBits::default();
        if let Some(p) = w.plan(&self.plan) {
            let running: Vec<String> = p
                .running()
                .iter()
                .map(|&i| format!("lane {} {:02}", lane(p, i), i + 1))
                .collect();
            let (ok, failed, ..) = p.outcome();
            bits.center = Some(match p.phase {
                PlanPhase::Done if failed > 0 => StatusItem::new(
                    format!("finished · {ok} ok · {failed} failed"),
                    Tone::Secondary,
                )
                .priority(6),
                PlanPhase::Done => {
                    StatusItem::new(format!("finished · {ok} ok"), Tone::Secondary).priority(6)
                }
                _ if running.is_empty() => StatusItem::new("waiting", Tone::Secondary).priority(6),
                _ => StatusItem::new(running.join(" · "), Tone::Secondary)
                    .busy()
                    .priority(6),
            });
        }
        bits
    }
}
