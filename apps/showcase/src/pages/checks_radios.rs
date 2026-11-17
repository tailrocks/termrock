use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::choice::{CheckHit, Checkbox, RadioGroup, Toggle};
use junie_tui::widgets::panel::Panel;

const ID: WidgetId = WidgetId::of("checksradios");

pub fn page() -> Box<dyn Page> {
    Box::new(ChecksRadiosPage::new())
}

pub struct ChecksRadiosPage {
    template: Template,
    opts: PageValues,
    checks: Vec<Checkbox>,
    radios: Vec<RadioGroup>,
    toggles: Vec<Toggle>,
    sections: crate::pages::layout::Sections,
    last: String,
}

impl ChecksRadiosPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::ChecksRadios)),
            checks: Vec::new(),
            radios: Vec::new(),
            toggles: Vec::new(),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            last: "nothing yet".into(),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let hit = match o.value("checksradios.hit") {
            "row" => CheckHit::Row,
            _ => CheckHit::Label,
        };
        let off = o.on("checksradios.disabled");
        let cid = ID.sub("checks");
        let mut checks = vec![
            Checkbox::new(cid.child(0), "Select all rows", false).mixed(o.on("checksradios.mixed")),
            Checkbox::new(cid.child(1), "Send notifications", true),
            Checkbox::new(cid.child(2), "Include archived projects", false),
            Checkbox::new(cid.child(3), "Managed by policy", true),
        ];
        checks[3].disabled = true;
        for (i, c) in checks.iter_mut().enumerate() {
            *c = c.clone().hit(hit);
            c.disabled |= off;
            if let Some(old) = self.checks.get(i)
                && i != 0
            {
                c.checked = old.checked;
            }
        }
        self.checks = checks;

        let nav = match o.value("checksradios.radio_nav") {
            "vim" => NavKeys::VIM,
            "no-home-end" => NavKeys::default().home_end(false),
            _ => NavKeys::default(),
        };
        let rid = ID.sub("radios");
        let mut radios = vec![
            RadioGroup::new(
                rid.child(0),
                "Isolation level",
                &["Read committed", "Repeatable read", "Serializable"],
                0,
            )
            .nav(nav),
            RadioGroup::new(rid.child(1), "Region", &["eu-west-1", "us-east-1"], 0).nav(nav),
        ];
        radios[1].disabled = true;
        for (i, r) in radios.iter_mut().enumerate() {
            r.disabled |= off;
            if let Some(old) = self.radios.get(i) {
                r.selected = old.selected;
                r.cursor = old.cursor;
            }
        }
        self.radios = radios;

        let tid = ID.sub("toggles");
        let mut toggles = vec![
            Toggle::new(tid.child(0), "Auto-save drafts", true).disabled(off),
            Toggle::new(tid.child(1), "Share usage data", false).disabled(off),
            Toggle::new(tid.child(2), "Beta features", false).disabled(true),
        ];
        for (t, old) in toggles.iter_mut().zip(&self.toggles) {
            t.on = old.on;
        }
        self.toggles = toggles;
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let has = |id| ctx.interaction.has_focus(id);
        let focus = if self.checks.iter().any(|c| has(c.id)) {
            Some(0)
        } else if self.radios.iter().any(|r| has(r.id)) {
            Some(1)
        } else if self.toggles.iter().any(|t| has(t.id)) {
            Some(2)
        } else {
            None
        };
        let l = self.sections.layout(
            area,
            &[
                StackChild::fixed(9),
                StackChild::fixed(8),
                StackChild::fixed(7),
            ],
            focus,
            buf,
            ctx,
        );
        match l.slot(0) {
            Some(r) => self.render_checks(r, buf, ctx),
            None => self.checks.iter().for_each(|c| ctx.focus_stop(c.id)),
        }
        match l.slot(1) {
            Some(r) => self.render_radios(r, buf, ctx),
            None => self.radios.iter().for_each(|r| ctx.focus_stop(r.id)),
        }
        match l.slot(2) {
            Some(r) => self.render_toggles(r, buf, ctx),
            None => self.toggles.iter().for_each(|t| ctx.focus_stop(t.id)),
        }
    }

    fn render_checks(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meta = match self.checks[0].hit {
            CheckHit::Row => "click target: the whole row",
            CheckHit::Label => "click target: box and label",
        };
        let panel = Panel::card(Some("Checkboxes")).meta(meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let w = inner.width.min(48);
        for (i, c) in self.checks.iter_mut().enumerate() {
            let y = inner.y + i as u16;
            if y < inner.bottom() {
                c.render(Rect::new(inner.x, y, w, 1), buf, ctx, bg);
            }
        }
        let y = inner.y + self.checks.len() as u16 + 1;
        if y < inner.bottom() {
            let text = format!("last: {}", self.last);
            buf.set_string(
                inner.x,
                y,
                junie_tui::ui::text::truncate(&text, inner.width as usize),
                t.faint().bg(bg),
            );
        }
    }

    fn render_radios(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Radio groups")).meta("↑↓ choose · one Tab stop per group");
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let (l, r) = crate::pages::layout::columns(inner, (inner.width / 2).saturating_sub(2), 4);
        for (g, col) in self.radios.iter_mut().zip([l, r]) {
            let h = g.height().min(col.height);
            g.render(Rect::new(col.x, col.y, col.width.min(36), h), buf, ctx, bg);
        }
    }

    fn render_toggles(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Toggles")).meta("Space / click switches");
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let w = inner.width.min(48);
        for (i, tg) in self.toggles.iter_mut().enumerate() {
            let y = inner.y + i as u16;
            if y < inner.bottom() {
                tg.render(Rect::new(inner.x, y, w, 1), buf, ctx, bg);
            }
        }
    }

    fn checked(&mut self, i: usize) {
        let c = &self.checks[i];
        self.last = format!(
            "{} {}",
            c.label,
            if c.checked { "checked" } else { "unchecked" }
        );
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(14)
        .column_width(13)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Disabled,
            Overflow,
        ])
        .column("Check off", Checkbox::new(id.child(0), "Label", false))
        .column("Check on", Checkbox::new(id.child(1), "Label", true))
        .column(
            "Mixed",
            Checkbox::new(id.child(2), "Label", false).mixed(true),
        )
        .column(
            "Radio group",
            RadioGroup::new(id.child(3), "Label", &["One", "Two"], 0),
        )
        .column("Toggle off", Toggle::new(id.child(4), "Label", false))
        .column("Toggle on", Toggle::new(id.child(5), "Label", true))
}

impl Page for ChecksRadiosPage {
    fn title(&self) -> &'static str {
        "Checks & radios"
    }
    fn blurb(&self) -> &'static str {
        "Checkboxes (mixed, disabled, click target), radio groups and toggles"
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
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if let Some(i) = self.checks.iter().position(|c| c.id == f) {
                    let o = self.checks[i].on_key(key);
                    if o == Outcome::Changed {
                        self.checked(i);
                    }
                    return o;
                }
                if let Some(r) = self.radios.iter_mut().find(|r| r.id == f) {
                    return r.on_key(key);
                }
                if let Some(t) = self.toggles.iter_mut().find(|t| t.id == f) {
                    return t.on_key(key);
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(i) = self.checks.iter().position(|c| ev.on(c.id)) {
                    let o = self.checks[i]
                        .route(ev)
                        .map_or(Outcome::Ignored, |r| r.outcome());
                    if o == Outcome::Changed {
                        self.checked(i);
                    }
                    return o;
                }
                for r in &mut self.radios {
                    if ev.on(r.id) {
                        if ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() && !r.disabled {
                            cx.focus.focus(r.id);
                        }
                        return r.route(ev).map_or(Outcome::Ignored, |o| o.outcome());
                    }
                }
                for t in &mut self.toggles {
                    if let Some(r) = t.route(ev) {
                        return r.outcome();
                    }
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
        if focus.is_some_and(|f| self.radios.iter().any(|r| r.id == f)) {
            vec![("↑↓", "Choose")]
        } else {
            vec![("Space", "Toggle")]
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
