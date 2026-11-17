use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::refusal::{Control, Reason, take_refusal};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::choice::{Checkbox, RadioGroup};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::select::Select;

const ID: WidgetId = WidgetId::of("fixedcontrols");

const KEEP_AWAKE: &str = "Always off · the host decides";
const DIRTY_EXIT: &str = "Always ask · the policy is managed";
const TYPED_KEY: &str = "Not available · managed by policy";
const DISABLE: &str = "Cannot disable · all stay on";
const MANAGED_ROW: &str = "Managed by policy · cannot be changed";

pub fn page() -> Box<dyn Page> {
    Box::new(FixedControlsPage::new())
}

fn control(fixed: bool, text: &str) -> Control {
    if fixed {
        Control::fixed(Reason::new(text).short("managed"))
    } else {
        Control::Live
    }
}

pub struct FixedControlsPage {
    template: Template,
    opts: PageValues,
    keep_awake: Checkbox,
    notify: Checkbox,
    dirty_exit: Select,
    disable: Button,
    source: RadioGroup,
    rows: ListBox,
}

impl FixedControlsPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::FixedControls)),
            keep_awake: Checkbox::new(ID.sub("keep"), "Keep awake", false),
            notify: Checkbox::new(ID.sub("notify"), "Notify on exit", true),
            dirty_exit: Select::new(
                ID.sub("dirty"),
                "On dirty exit",
                &["ask", "stop", "keep"],
                0,
            ),
            disable: Button::secondary(ID.sub("disable"), "Disable"),
            source: RadioGroup::new(
                ID.sub("source"),
                "Credential source",
                &["Local folder", "Password manager", "Typed key"],
                0,
            ),
            rows: ListBox::new(ID.sub("rows"), Vec::new(), SelectMode::Multi),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let fixed = self.opts.value("fixedcontrols.state") == "fixed";
        self.keep_awake.control = control(fixed, KEEP_AWAKE);
        self.dirty_exit.control = control(fixed, DIRTY_EXIT);
        self.dirty_exit.help = if fixed {
            DIRTY_EXIT.into()
        } else {
            "Choose what happens to unsaved work".into()
        };
        self.disable.control = control(fixed, DISABLE);
        self.source.controls[2] = control(fixed, TYPED_KEY);
        let checked = self.rows.core.selection.clone();
        let cursor = self.rows.core.cursor;
        self.rows.items = vec![
            ListItem::new("Team policy").meta("on"),
            ListItem::new("Managed policy")
                .meta("on")
                .control(control(fixed, MANAGED_ROW)),
            ListItem::new("Personal policy").meta("off"),
        ];
        self.rows.core.selection = checked;
        self.rows.core.cursor = cursor.min(self.rows.items.len() - 1);
    }

    fn reason_beside(
        control: &Control,
        at: Rect,
        buf: &mut Buffer,
        ctx: &RenderCtx,
        bg: ratatui::style::Color,
    ) {
        if let Some(reason) = control.reason() {
            let text = junie_tui::ui::text::truncate(&reason.text, usize::from(at.width));
            buf.set_string(at.x, at.y, text, ctx.theme.faint().bg(bg));
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let rows = crate::pages::layout::rows(area, &[11, 8]);
        let panel = Panel::card(Some("Fixed controls")).meta("Space or click shows the reason");
        let bg = panel.bg(t);
        let inner = panel.render(rows[0], buf, t);
        let beside = inner.x + 26;
        let room = inner.right().saturating_sub(beside);

        self.keep_awake
            .render(Rect::new(inner.x, inner.y, 24, 1), buf, ctx, bg);
        Self::reason_beside(
            &self.keep_awake.control,
            Rect::new(beside, inner.y, room, 1),
            buf,
            ctx,
            bg,
        );
        self.notify
            .render(Rect::new(inner.x, inner.y + 1, 24, 1), buf, ctx, bg);
        self.dirty_exit.render(
            Rect::new(inner.x, inner.y + 3, inner.width.min(48), 3),
            buf,
            ctx,
            bg,
        );
        self.disable
            .render(Rect::new(inner.x, inner.y + 7, 12, 1), buf, ctx, bg);
        Self::reason_beside(
            &self.disable.control,
            Rect::new(beside, inner.y + 7, room, 1),
            buf,
            ctx,
            bg,
        );

        let panel = Panel::card(Some("Radio option and list row"))
            .meta("a fixed option or row can be focused");
        let bg = panel.bg(t);
        let inner = panel.render(rows[1], buf, t);
        let (l, r) = crate::pages::layout::columns(inner, (inner.width / 2).saturating_sub(2), 4);
        let h = self.source.height().min(l.height);
        self.source
            .render(Rect::new(l.x, l.y, l.width.min(36), h), buf, ctx, bg);
        self.rows.render(r, buf, ctx, bg);
    }

    fn refusal(&self, cx: &mut Cx) {
        if let Some(reason) = take_refusal() {
            if self.opts.value("fixedcontrols.status") == "sticky" {
                cx.sticky(reason.text);
            } else {
                cx.refused(reason);
            }
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(14)
        .column_width(16)
        .rows(&[Rest, Hover, Focused, FocusedHover, Disabled])
        .column("Check live", Checkbox::new(id.child(0), "Label", true))
        .column(
            "Check fixed",
            Checkbox::new(id.child(1), "Label", true).fixed(Reason::new("Always on")),
        )
        .column(
            "Radio fixed",
            RadioGroup::new(id.child(2), "Label", &["One", "Two"], 0)
                .fixed_option(0, Reason::new("Always one")),
        )
}

impl Page for FixedControlsPage {
    fn title(&self) -> &'static str {
        "Fixed controls"
    }
    fn blurb(&self) -> &'static str {
        "One true value: focusable, drawn disabled, refuses every edit"
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
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let out = if f == self.keep_awake.id {
                    self.keep_awake.on_key(key)
                } else if f == self.notify.id {
                    self.notify.on_key(key)
                } else if f == self.dirty_exit.id {
                    self.dirty_exit.on_key(key).0
                } else if f == self.disable.id {
                    self.disable.on_key(key).0
                } else if f == self.source.id {
                    self.source.on_key(key)
                } else if f == self.rows.id {
                    self.rows.on_key(key)
                } else {
                    Outcome::Ignored
                };
                self.refusal(cx);
                out
            }
            PageEvent::Pointer(ev) => {
                let mut hit = None;
                let out = if let Some(r) = self.keep_awake.route(ev) {
                    hit = Some(self.keep_awake.id);
                    r.outcome()
                } else if let Some(r) = self.notify.route(ev) {
                    hit = Some(self.notify.id);
                    r.outcome()
                } else if let Some(r) = self.dirty_exit.route(ev) {
                    hit = Some(self.dirty_exit.id);
                    r.outcome()
                } else if let Some(r) = self.disable.route(ev) {
                    hit = Some(self.disable.id);
                    r.outcome()
                } else if let Some(r) = self.source.route(ev) {
                    hit = Some(self.source.id);
                    r.outcome()
                } else if let Some(r) = self.rows.route(ev) {
                    hit = Some(self.rows.id);
                    r.outcome()
                } else {
                    Outcome::Ignored
                };
                if ev.is(Phase::Click)
                    && let Some(id) = hit
                {
                    cx.focus.focus(id);
                }
                self.refusal(cx);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Space", "Try to change")]
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
