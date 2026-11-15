use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, ButtonRow, RowOverflow};
use junie_tui::widgets::meter_list::{MeterList, MeterRow};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::{Meter, MeterTone, MeterVisual};
use junie_tui::widgets::props::{self, Prop, PropsAction, PropsList};

const ID: WidgetId = WidgetId::of("readonlyreport");
const FACTS: WidgetId = ID.sub("facts");
const REFRESH: WidgetId = ID.sub("refresh");
const COPY: WidgetId = ID.sub("copy");
const NEXT_DISK: WidgetId = ID.sub("next-disk");
const NEXT_ACTIVITY: WidgetId = ID.sub("next-activity");

const REFRESH_TICKS: u16 = 16;

const LOAD: [(&str, u8, &str); 5] = [
    ("CPU", 23, "8 cores · load 1.84 1.62 1.40"),
    ("Memory", 68, "21.8 of 32 GB · 3.1 GB cached"),
    ("Swap", 4, "0.2 of 4 GB"),
    ("Disk /", 62, "310 of 500 GB"),
    ("Disk /Volumes/Backup", 97, "1.94 of 2 TB"),
];

fn facts(copyable: bool) -> Vec<Prop> {
    let rows = [
        ("Host", "build-07.acme.internal"),
        ("Address", "10.4.12.37"),
        ("System", "macOS 15.1 (24B83) · arm64"),
        ("Uptime", "6 d 4 h 12 m"),
        ("Processes", "412 · 3 zombie"),
        ("Network", "en0 · 1 Gbit/s · 12.4 MB/s in"),
        ("Snapshot", "snap-2026-10-17T09-41-08Z"),
    ];
    rows.iter()
        .map(|(l, v)| {
            let p = Prop::new(l, *v);
            let p = if *l == "Processes" {
                p.tone(Tone::Warning)
            } else {
                p
            };
            if copyable { p.copyable() } else { p }
        })
        .collect()
}

pub fn page() -> Box<dyn Page> {
    Box::new(ReadOnlyReportPage::new())
}

pub struct ReadOnlyReportPage {
    template: Template,
    opts: PageValues,
    facts: PropsList,
    actions: Vec<Button>,

    refreshing: u16,

    epoch: u8,
    sections: crate::pages::layout::Sections,
}

impl ReadOnlyReportPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::ReadOnlyReport));
        Self {
            template: Template::new(ID, states_grid()),
            facts: PropsList::new(FACTS, facts(opts.on("readonlyreport.copyable"))),
            opts,
            actions: vec![
                Button::secondary(REFRESH, "Refresh"),
                Button::secondary(COPY, "Copy report"),
                Button::secondary(NEXT_DISK, "Disk cleanup →"),
                Button::secondary(NEXT_ACTIVITY, "Activity →"),
            ],
            refreshing: 0,
            epoch: 0,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    fn offline(&self) -> bool {
        self.opts.on("readonlyreport.offline")
    }

    fn meta(&self) -> &'static str {
        if self.refreshing > 0 {
            "refreshing…"
        } else if self.offline() {
            "offline · last-good 2 m ago"
        } else if self.epoch > 0 {
            "live · just now"
        } else {
            "live · 12 s ago"
        }
    }

    fn meters(&self) -> MeterList {
        let visual = match self.opts.value("readonlyreport.visual") {
            "line" => MeterVisual::Line,
            _ => MeterVisual::Block,
        };
        let disk = MeterTone::Level { warn: 80, crit: 95 };
        let rows = LOAD
            .iter()
            .map(|(label, pct, detail)| {
                let pct = if label.starts_with("Disk") || *pct > 90 {
                    *pct
                } else {
                    (*pct + self.epoch * 4).min(89)
                };
                let mut m = Meter::new(Some(pct))
                    .value(format!("{pct:>3}%"))
                    .visual(visual);
                if label.starts_with("Disk") {
                    m = m.tone(disk);
                }
                if self.refreshing > 0 {
                    m = m.tone(MeterTone::Refreshing);
                } else if self.offline() {
                    m = m.stale(true);
                }
                MeterRow::new(*label, m).detail(*detail)
            })
            .collect();
        MeterList::new(rows)
            .label_tone(Tone::Secondary)
            .meter_width(24)
            .min_meter(8)
    }

    fn notices(&self) -> Vec<Notice> {
        let mut v = Vec::new();
        if self.offline() {
            v.push(
                Notice::warning("Offline: showing the last-good snapshot")
                    .detail("build-07 stopped answering 2 m ago. Values are drawn stale until a refresh reaches it."),
            );
        }
        if self.opts.on("readonlyreport.notices") {
            v.push(
                Notice::error("/Volumes/Backup is 97 % full")
                    .detail("Time Machine needs 120 GB for the next backup; 60 GB are free.")
                    .hint("Disk cleanup → frees space on this volume"),
            );
            v.push(Notice::info("This report is read-only").detail(
                "Nothing here changes the host. Copy a value with y, or the whole report.",
            ));
        }
        v
    }

    fn report_text(&self) -> String {
        let mut s = format!("Host snapshot ({})\n", self.meta());
        for (l, p, d) in LOAD {
            s.push_str(&format!("{l}: {p}% ({d})\n"));
        }
        for p in &self.facts.props {
            s.push_str(&format!("{}: {}\n", p.label, p.value));
        }
        s
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meters = self.meters();
        let notices = self.notices();
        let row = ButtonRow::new().overflow(RowOverflow::Wrap);
        let children = self.children(area.width, t);
        let actions_at = children.len() - 1;
        let focus = if ctx.interaction.has_focus(FACTS) {
            Some(1)
        } else if self.actions.iter().any(|b| ctx.interaction.has_focus(b.id)) {
            Some(actions_at)
        } else {
            None
        };
        let l = self.sections.layout(area, &children, focus, buf, ctx);
        if let Some(r) = l.slot(0) {
            let panel = Panel::card(Some("Host snapshot")).meta(self.meta());
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            meters.render(inner, buf, ctx, bg);
        }
        match l.slot(1) {
            Some(r) => {
                let panel = Panel::framed(Some("Facts"))
                    .meta(if self.opts.on("readonlyreport.copyable") {
                        "y copies"
                    } else {
                        "read-only"
                    })
                    .focused(ctx.interaction.focused(FACTS));
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                self.facts.render(inner, buf, ctx, bg);
            }
            None => ctx.focus_stop(FACTS),
        }
        if !notices.is_empty()
            && let Some(r) = l.slot(2)
        {
            let x = r.x + 2;
            let w = r.width.saturating_sub(4);
            let mut y = r.y;
            for n in &notices {
                let h = n.height(w);
                n.render(
                    Rect::new(x, y, w, h.min(r.bottom() - y)),
                    buf,
                    ctx,
                    t.canvas,
                );
                y = (y + h + 1).min(r.bottom());
            }
        }
        for b in &mut self.actions {
            b.busy = b.id == REFRESH && self.refreshing > 0;
        }
        match l.slot(actions_at) {
            Some(r) => {
                let lay = row.layout(r, &self.actions, t);
                for (b, rect) in self.actions.iter_mut().zip(lay.rects) {
                    match rect {
                        Some(rect) => b.render(rect, buf, ctx, t.canvas),
                        None => ctx.focus_stop(b.id),
                    }
                }
            }
            None => {
                for b in &self.actions {
                    ctx.focus_stop(b.id);
                }
            }
        }
    }

    fn children(&self, width: u16, t: &junie_tui::theme::Theme) -> Vec<StackChild> {
        let notices = self.notices();
        let inner_w = width.saturating_sub(4);
        let notice_h: u16 = notices
            .iter()
            .map(|n| n.height(inner_w) + 1)
            .sum::<u16>()
            .saturating_sub(1);
        let row = ButtonRow::new().overflow(RowOverflow::Wrap);
        let mut children = vec![
            StackChild::fixed(self.meters().height() + 3),
            StackChild::fixed(self.facts.props.len() as u16 + 3),
        ];
        if !notices.is_empty() {
            children.push(StackChild::fixed(notice_h));
        }
        children.push(StackChild::fixed(row.rows(width, &self.actions, t)));
        children
    }

    fn start_refresh(&mut self, cx: &mut Cx) {
        if self.offline() {
            cx.status_in("Offline: build-07 does not answer", Tone::Warning);
            return;
        }
        self.refreshing = REFRESH_TICKS;
        cx.status("Refreshing the snapshot…");
    }

    fn act(&mut self, id: WidgetId, cx: &mut Cx) {
        match id {
            REFRESH => self.start_refresh(cx),
            COPY => {
                let text = self.report_text();
                let lines = text.lines().count();
                cx.copy_as(text, format!("the report ({lines} lines)"));
            }
            NEXT_DISK => cx.open_page(PageId::DiskCleanup),
            NEXT_ACTIVITY => cx.open_page(PageId::Activity),
            _ => {}
        }
    }

    fn props_action(&mut self, a: Option<PropsAction>, cx: &mut Cx) {
        if let Some(PropsAction::Copy(i) | PropsAction::Activate(i)) = a
            && let Some(p) = self.facts.props.get(i)
        {
            if self.opts.on("readonlyreport.copyable") {
                cx.copy_as(p.value.clone(), p.label.clone());
            } else {
                cx.status(format!("{}: {}", p.label, p.value));
            }
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(11)
        .column_width(34)
        .narrow_width(20)
        .rows(&[Rest, Focused, Loading, Error, Overflow, Narrow])
        .column(
            "Meters",
            MeterList::new(vec![
                MeterRow::new("CPU", Meter::new(Some(23)).value(" 23%")).detail("8 cores"),
                MeterRow::new(
                    "Backup",
                    Meter::new(Some(97))
                        .value(" 97%")
                        .tone(MeterTone::Level { warn: 80, crit: 95 }),
                )
                .detail("2 TB"),
            ])
            .meter_width(14),
        )
        .column(
            "Facts",
            PropsList::new(
                id.child(1),
                vec![
                    Prop::new("Host", "build-07").copyable(),
                    Prop::new("Uptime", "6 d 4 h"),
                ],
            ),
        )
}

const NOTICE_MIN: u16 = 40;

impl Page for ReadOnlyReportPage {
    fn title(&self) -> &'static str {
        "Read-only report"
    }
    fn blurb(&self) -> &'static str {
        "A host snapshot: live meta, MeterList, copyable facts, notices and Next actions"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let at = |m: &dyn Measure| m.measure(Size::ZERO, ctx);
        let notices = self
            .notices()
            .iter()
            .map(|n| (at(n).width + 4).min(NOTICE_MIN))
            .max()
            .unwrap_or(0);
        let natural = [
            Panel::card(Some("Host snapshot"))
                .meta(self.meta())
                .outer(at(&self.meters()))
                .width,
            Panel::framed(Some("Facts"))
                .meta("read-only")
                .outer(Size::new(props::width(&self.facts.props), 0))
                .width,
            notices,
            ButtonRow::new().width(&self.actions, ctx.theme),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        let width = natural.max(avail.width);
        Size::new(
            natural,
            self.sections
                .stack
                .measure(&self.children(width, ctx.theme)),
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
            if self.refreshing > 0 {
                self.refreshing -= 1;
                if self.refreshing == 0 {
                    self.epoch = self.epoch.wrapping_add(1) % 4;
                    cx.status("Snapshot refreshed ✓");
                }
                return Outcome::Changed;
            }
            return Outcome::Ignored;
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
                    if o != Outcome::Ignored {
                        self.props_action(a, cx);
                        return o;
                    }
                }
                if let Some(i) = self.actions.iter().position(|b| cx.focus.is(b.id)) {
                    let (o, act) = self.actions[i].on_key(key);
                    if act {
                        let id = self.actions[i].id;
                        self.act(id, cx);
                        return Outcome::Changed;
                    }
                    if o != Outcome::Ignored {
                        return o;
                    }
                }

                if key.code == KeyCode::Char('r') && key.mods.is_empty() {
                    self.start_refresh(cx);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for i in 0..self.actions.len() {
                    if let Some(r) = self.actions[i].route(ev)
                        && r.outcome() != Outcome::Ignored
                    {
                        if r.has_actions() {
                            let id = self.actions[i].id;
                            cx.focus.focus(id);
                            self.act(id, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                if let Some(r) = self.facts.route(ev) {
                    if ev.is(junie_tui::core::intent::Phase::Click) {
                        cx.focus.focus(FACTS);
                    }
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.props_action(Some(a), cx);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.refreshing > 0
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        let mut v = match focus {
            Some(FACTS) if self.opts.on("readonlyreport.copyable") => {
                vec![("↑↓", "Move"), ("y", "Copy")]
            }
            Some(FACTS) => vec![("↑↓", "Move")],
            Some(_) => vec![("Enter", "Activate")],
            None => Vec::new(),
        };
        v.push(("r", "Refresh"));
        v
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
        if key == "readonlyreport.copyable" {
            self.facts.set_props(facts(self.opts.on(key)));
        }
        if key == "readonlyreport.offline" {
            self.refreshing = 0;
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
