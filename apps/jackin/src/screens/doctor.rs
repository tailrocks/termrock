use crate::screens::Jackin;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::shell::Page;
use junie_tui::theme::{ButtonKind, StatusKind, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::Split;
use junie_tui::widgets::empty::{self, EmptyAction, EmptyState};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::segments::Segment;
use junie_tui::widgets::viewport::Span;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use super::row_meta::ClockStamp;
use super::{Cx, Go, Screen};
use crate::domain::doctor::{DoctorCheck, DoctorReport};
use crate::world::{Msg, World, job};
use junie_tui::ui::text::plural;
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};

pub const LIST: WidgetId = WidgetId::of("doctor.list");
pub const DETAIL: WidgetId = WidgetId::of("doctor.detail");
const RUN: WidgetId = WidgetId::of("doctor.empty.run");

pub struct DoctorScreen {
    ld: ListDetail,
    clock: ClockStamp,
}

impl Default for DoctorScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn word(kind: StatusKind) -> &'static str {
    match kind {
        StatusKind::Ok => "ok",
        StatusKind::Warn => "warn",
        StatusKind::Fail => "fail",
        StatusKind::Skipped => "skipped",
        StatusKind::Running => "running",
        StatusKind::Pending | StatusKind::Queued => "queued",
        StatusKind::Info => "info",
    }
}

const EMPTY_HINT: &str =
    "The doctor checks the binaries, the gateway and the Docker engine. It never changes anything.";

impl DoctorScreen {
    pub fn new() -> Self {
        Self {
            ld: ListDetail::new(LIST, DETAIL, Split::new(50, 36, 40)),
            clock: ClockStamp::default(),
        }
    }

    fn report<'a>(&self, w: &'a World) -> Option<&'a DoctorReport> {
        w.surfaces.doctor.as_ref().and_then(|d| d.report())
    }

    fn running(&self, w: &World) -> bool {
        w.surfaces.doctor.as_ref().is_some_and(|d| d.running())
    }

    fn selected<'a>(&self, w: &'a World) -> Option<&'a DoctorCheck> {
        self.report(w)?.checks.get(self.ld.cursor)
    }

    fn run(&mut self, w: &mut World, cx: &mut Cx) {
        if w.start_doctor() {
            cx.progress(job::DOCTOR, "Running checks…");
        }
    }

    fn copy(&self, w: &World, cx: &mut Cx) {
        let Some(report) = self.report(w) else {
            cx.status("Nothing to copy · run the checks first");
            return;
        };
        let noun = w
            .surfaces
            .doctor
            .as_ref()
            .map_or("doctor", |d| d.noun.as_str());
        let mut text = format!("{noun} checks · {}\n", w.clock.ago(report.at_secs));
        for c in &report.checks {
            text.push_str(&format!(
                "{:<8} {:<18} {}\n",
                word(c.status),
                c.label,
                c.summary
            ));
            if let Some(fix) = &c.fix {
                text.push_str(&format!("         fix: {fix}\n"));
            }
        }
        cx.copy(text);
        cx.status("Report copied");
    }

    fn rows(&self, w: &World) -> Vec<ListRow> {
        let running = self.running(w);
        self.report(w)
            .map(|r| {
                r.checks
                    .iter()
                    .map(|c| {
                        let mark = if running {
                            StatusKind::Running
                        } else {
                            c.status
                        };
                        let row = ListRow::new(&c.label).mark(mark).detail(&c.summary);
                        let tone = c.status.tone();
                        if running {
                            row.meta("running", Tone::Active)
                        } else {
                            row.meta(word(c.status), tone)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn head(&self, w: &World) -> Vec<Span> {
        let muted = |t: &str| Span::new(t.to_owned(), Tone::Muted);
        if let Some((failure, at_secs)) = w.surfaces.doctor.as_ref().and_then(|d| d.failure()) {
            return vec![
                Span::new(w.surfaces.vocabulary.doctor_failure(failure), Tone::Error).bold(),
                muted(" · "),
                muted(&format!("tried {}", w.clock.ago(at_secs))),
            ];
        }
        let Some(report) = self.report(w) else {
            return vec![];
        };
        let ago = format!("checked {}", w.clock.ago(report.at_secs));
        if report.no_gateway() {
            return vec![
                Span::new("degraded", Tone::Error).bold(),
                muted(" · "),
                Span::new(w.surfaces.vocabulary.doctor_no_gateway.clone(), Tone::Error),
                muted(" · "),
                muted(&ago),
            ];
        }
        if report.stale(w.now_secs()) {
            return vec![
                Span::new("stale", Tone::Warning).bold(),
                muted(" · "),
                muted(&ago),
            ];
        }
        let (failing, warnings) = (report.failing(), report.warnings());
        if failing + warnings == 0 {
            return vec![
                Span::new("all checks pass", Tone::Normal),
                muted(" · "),
                muted(&ago),
            ];
        }
        let mut v = vec![if failing > 0 {
            Span::new("degraded", Tone::Error).bold()
        } else {
            Span::new("attention", Tone::Warning)
        }];
        if failing > 0 {
            v.push(muted(" · "));
            v.push(Span::new(format!("{failing} failing"), Tone::Error));
        }
        if warnings > 0 {
            v.push(muted(" · "));
            v.push(Span::new(
                plural(warnings, "warning", "warnings"),
                Tone::Warning,
            ));
        }
        v.push(muted(" · "));
        v.push(muted(&ago));
        v
    }

    fn meta(&self, w: &World) -> String {
        match self.report(w) {
            _ if w
                .surfaces
                .doctor
                .as_ref()
                .is_some_and(|d| d.failure().is_some()) =>
            {
                String::new()
            }
            Some(r) if r.no_gateway() => w.surfaces.vocabulary.doctor_no_gateway.clone(),
            Some(r) if r.failing() > 0 => format!("{} failing", r.failing()),
            Some(r) if r.warnings() > 0 => plural(r.warnings(), "warning", "warnings"),
            Some(_) => "all checks pass".into(),
            None => String::new(),
        }
    }

    fn lines(&self, w: &World, c: &DoctorCheck) -> Vec<Line> {
        let gateway = self
            .report(w)
            .and_then(|r| r.gateway.clone())
            .unwrap_or_else(|| "none".into());
        let status_tone = c.status.tone();
        let mut v = vec![
            Line::Prop("Status".into(), word(c.status).into(), status_tone),
            Line::Prop(
                "Found".into(),
                c.found.clone().unwrap_or_else(|| "—".into()),
                Tone::Normal,
            ),
        ];
        if let Some(expected) = &c.expected {
            v.push(Line::Prop(
                "Expected".into(),
                expected.clone(),
                Tone::Normal,
            ));
        }
        v.push(Line::Prop("Gateway".into(), gateway, Tone::Normal));
        v.push(Line::Blank);
        match c.status {
            StatusKind::Fail | StatusKind::Warn => {
                let mut n = if c.status == StatusKind::Fail {
                    Notice::error(c.summary.clone())
                } else {
                    Notice::warning(c.summary.clone())
                };
                if let Some(fix) = &c.fix {
                    n = n.hint(fix.clone());
                }
                v.push(Line::Notice(n));
            }
            StatusKind::Skipped => v.push(Line::Text(c.summary.clone(), Tone::Muted)),
            _ => {}
        }
        v
    }

    fn empty(&self, w: &World) -> EmptyState {
        if self.running(w) {
            return EmptyState::new("Running checks…").hint(EMPTY_HINT);
        }
        let title = match w.surfaces.doctor.as_ref().and_then(|d| d.failure()) {
            Some((failure, _)) => w.surfaces.vocabulary.doctor_failure(failure),
            None => "Checks have not run yet".to_owned(),
        };
        EmptyState::new(&title)
            .hint(EMPTY_HINT)
            .actions(vec![EmptyAction::primary(RUN, "Run checks")])
    }

    fn fire(&mut self, name: &str, w: &mut World, cx: &mut Cx) {
        match name {
            "run" => self.run(w, cx),
            "copy" => self.copy(w, cx),
            _ => {}
        }
    }
}

impl Page<Jackin> for DoctorScreen {
    fn enter(&mut self, _w: &mut World, cx: &mut Cx) {
        self.ld.close_detail(cx.focus);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(LIST)
    }

    fn animating(&self, w: &World) -> bool {
        self.running(w)
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.running(w) || self.clock.due(w, true, false) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, _w: &mut World, _cx: &mut Cx) -> Outcome {
        match msg {
            Msg::DoctorFinished(_) => Outcome::Changed,
            _ => Outcome::Ignored,
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let len = self.report(w).map_or(0, |r| r.checks.len());
        if key.plain() {
            match key.code {
                KeyCode::Char('r') | KeyCode::F(5) => {
                    self.run(w, cx);
                    return Outcome::Changed;
                }
                KeyCode::Char('y') => {
                    self.copy(w, cx);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if len == 0 {
            if key.is(KeyCode::Enter) {
                self.run(w, cx);
                return Outcome::Changed;
            }
            if key.is(KeyCode::Esc) {
                cx.go(Go::Manager);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        let (o, ev) = self.ld.on_key(key, cx.focus, len);
        if let Some(Event::Action(name)) = ev {
            self.fire(name, w, cx);
        }
        if o.consumed() {
            return o;
        }
        if key.is(KeyCode::Esc) && cx.focus.is(LIST) {
            cx.go(Go::Manager);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.ld.on_pointer(ev) {
            return o;
        }
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                junie_tui::core::hit::Axis::Vertical => self.ld.wheel(ev.id, delta),
                junie_tui::core::hit::Axis::Horizontal => Outcome::Ignored,
            };
        }
        match ev.phase() {
            Some(Phase::Click) => {
                if self.empty(w).on_click(ev.id).is_some() {
                    self.run(w, cx);
                    return Outcome::Changed;
                }
                let len = self.report(w).map_or(0, |r| r.checks.len());
                let (o, event) = self.ld.click(ev.id, cx.focus, len);
                if let Some(Event::Action(name)) = event {
                    self.fire(name, w, cx);
                }
                o
            }
            _ => Outcome::Ignored,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.clock.painted(w);
        let t = ctx.theme;
        let Some(report) = self.report(w) else {
            ctx.control(LIST, Rect::new(area.x, area.y, 1, 1), false);
            empty::render_ctx(area, buf, ctx, &self.empty(w), t.canvas);
            return;
        };
        let rows = self.rows(w);
        let selected = self.selected(w).cloned();
        let (detail_title, detail_meta, lines) = match &selected {
            Some(c) => (c.label.clone(), word(c.status).to_owned(), self.lines(w, c)),
            None => (String::new(), String::new(), vec![]),
        };
        let _ = report;
        let frame = Frame {
            title: "Doctor",
            meta: self.meta(w),
            head: self.head(w),
            rows: &rows,
            under: None,
            detail_title,
            detail_meta,
            lines,
            actions: vec![
                Action::new("run", "Run checks", ButtonKind::Secondary).disabled(self.running(w)),
                Action::new("copy", "Copy report", ButtonKind::Secondary),
            ],
        };
        self.ld.render(area, buf, ctx, &frame);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.report(w).is_none() {
            return if self.running(w) {
                vec![hint("Esc", "Back")]
            } else {
                vec![hint("Enter", "Run checks"), hint("Esc", "Back")]
            };
        }
        if let Some(v) = self.ld.hints(focus, &[]) {
            return v;
        }
        vec![
            hint("↑↓", "Move"),
            hint("Enter", "Details"),
            hint("r", "Run checks"),
            hint("y", "Copy report"),
            hint("Esc", "Back"),
        ]
    }

    fn crumb(&self, _w: &World) -> String {
        "Doctor".into()
    }
}

impl Screen for DoctorScreen {
    fn strip_right(&self, w: &World) -> Vec<Segment> {
        if self.running(w) {
            return vec![
                Segment::new(
                    format!(
                        "{} running checks",
                        junie_tui::widgets::progress::spinner_frame(w.now_ms() as u64 / 80)
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            ];
        }
        vec![]
    }
}
