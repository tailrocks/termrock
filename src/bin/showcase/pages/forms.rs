use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::focus::{BlurAction, forward_blur};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::button::{Button, row_layout};
use termrock::widgets::choice::{Checkbox, RadioGroup, Toggle};
use termrock::widgets::edit::ValidateWhen;
use termrock::widgets::input::{InputAction, TextInput};
use termrock::widgets::panel::Panel;
use termrock::widgets::textarea::TextArea;

const ID: WidgetId = WidgetId::of("forms");
const SUBMIT_TICKS: u8 = 23;

fn email(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else if !s.contains('@') || !s.contains('.') {
        Some("Enter a valid email address".into())
    } else {
        None
    }
}

fn name(s: &str, required: bool) -> Option<String> {
    if s.trim().is_empty() {
        required.then(|| "Required".into())
    } else if s.len() < 4 {
        Some("At least 4 characters".into())
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Submit {
    Idle,
    Busy(u8),
    Done,
}

const CARD_H: u16 = 24;

const MANAGED: &str = "Managed by your organization";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Name,
    Description,
    Reviewer,
    Mode,
    Checks,
    Toggles,
    Actions,
}

impl Part {
    fn heading(self) -> Option<&'static str> {
        match self {
            Self::Name => Some("Task"),
            Self::Reviewer => Some("Review"),
            Self::Mode => Some("Options"),
            _ => None,
        }
    }

    const ALL: [Self; 7] = [
        Self::Name,
        Self::Description,
        Self::Reviewer,
        Self::Mode,
        Self::Checks,
        Self::Toggles,
        Self::Actions,
    ];
}

pub struct FormsPage {
    template: Template,
    opts: PageValues,
    name: TextInput,
    description: TextArea,
    mode: RadioGroup,
    run_tests: Checkbox,
    open_pr: Checkbox,
    auto_approve: Toggle,
    notify: Toggle,
    reviewer: TextInput,
    submit: Button,
    reset: Button,
    state: Submit,
    attempted: bool,

    sections: crate::pages::layout::Sections,
}

impl FormsPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Forms));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts,
            name: TextInput::new(ID.sub("name"), "Task name"),
            description: TextArea::new(ID.sub("desc"), "Description", 4),
            mode: RadioGroup::new(ID.sub("mode"), "Mode", &["Fast", "Balanced", "Thorough"], 1),
            run_tests: Checkbox::new(ID.sub("tests"), "Run tests before opening a PR", true),
            open_pr: Checkbox::new(ID.sub("pr"), "Open a pull request when done", false),
            auto_approve: Toggle::new(ID.sub("auto"), "Auto-approve changes", false),
            notify: Toggle::new(ID.sub("notify"), "Notify on completion", true),
            reviewer: TextInput::new(ID.sub("reviewer"), "Reviewer"),
            submit: Button::primary(ID.sub("submit"), "Create task"),
            reset: Button::subtle(ID.sub("reset"), "Reset"),
            state: Submit::Idle,
            attempted: false,
            sections: {
                let mut s = crate::pages::layout::Sections::new(ID.sub("parts"));
                s.stack = s.stack.gap(0);
                s
            },
        };
        page.apply();
        page
    }

    fn apply(&mut self) {
        let o = &self.opts;
        let when = match o.value("forms.validate_when") {
            "commit" => ValidateWhen::Commit,
            "live" => ValidateWhen::Live,
            _ => ValidateWhen::AfterError,
        };
        let blur = match o.value("forms.blur") {
            "cancel" => BlurAction::Cancel,
            "keep" => BlurAction::Keep,
            _ => BlurAction::Commit,
        };
        let required = o.on("forms.required");
        let help = |h: &'static str| if o.on("forms.help") { h } else { "" };
        let hint = |p: &'static str| if o.on("forms.placeholders") { p } else { "" };
        let rows = o.number("forms.desc_rows").unwrap_or(4);
        self.name = TextInput::new(ID.sub("name"), "Task name")
            .required(required)
            .validator(move |s| name(s, required))
            .validate_when(when)
            .blur_action(blur)
            .placeholder(hint("Short imperative summary"))
            .value(self.name.text());
        self.description = TextArea::new(ID.sub("desc"), "Description", rows)
            .placeholder(hint("What should Junie do, and what does done look like?"))
            .help(help("Optional · Markdown"))
            .blur_action(blur)
            .value(self.description.field.text());
        self.reviewer = TextInput::new(ID.sub("reviewer"), "Reviewer")
            .validator(email)
            .validate_when(when)
            .blur_action(blur)
            .placeholder(hint("name@company.com"))
            .help(help("Optional"))
            .value(self.reviewer.text());
        self.notify = Toggle::new(ID.sub("notify"), "Notify on completion", self.notify.on)
            .disabled(o.on("forms.managed"));
    }

    fn desc_rows(&self) -> u16 {
        self.opts.number("forms.desc_rows").unwrap_or(4)
    }

    fn validate(&mut self) -> bool {
        let a = self.name.validate();
        let b = self.reviewer.validate();
        a && b
    }

    fn do_submit(&mut self, cx: &mut Cx) {
        self.attempted = true;
        if self.name.editing {
            self.name.commit();
        }
        if self.reviewer.editing {
            self.reviewer.commit();
        }
        if !self.validate() {
            cx.status("Fix the highlighted fields");
            if self.name.field.error.is_some() {
                cx.focus.focus(self.name.id);
            } else {
                cx.focus.focus(self.reviewer.id);
            }
            return;
        }
        self.submit.busy = true;
        self.state = Submit::Busy(SUBMIT_TICKS);
        cx.status("Creating task…");
    }

    fn do_reset(&mut self, cx: &mut Cx) {
        let opts = self.opts.clone();
        *self = Self::new();
        self.opts = opts;
        self.apply();
        cx.focus.focus(self.name.id);
        cx.status("Form reset");
    }

    fn message(&self, t: &termrock::theme::Theme) -> Option<(&'static str, ratatui::style::Style)> {
        match self.state {
            Submit::Idle
                if self.attempted
                    && (self.name.field.error.is_some() || self.reviewer.field.error.is_some()) =>
            {
                Some(("Fix the highlighted fields", t.error_fg()))
            }
            Submit::Busy(_) => Some(("Creating task…", t.secondary())),
            Submit::Done => Some(("Task created ✓", t.accent_fg())),
            _ => None,
        }
    }

    fn render_actions(
        &mut self,
        row: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        let widths = [self.submit.width(), self.reset.width()];
        let rects = row_layout(row, &widths, 2);
        self.submit.render(rects[0], buf, ctx, bg);
        self.reset.render(rects[1], buf, ctx, bg);
        if let Some((m, st)) = self.message(t) {
            let x = rects[1].right() + 3;
            if x + termrock::ui::text::width(m) as u16 <= row.right() {
                buf.set_string(x, row.y, m, st.bg(bg));
            }
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("New task")).meta("Ctrl+S Submit");
        let bg = panel.bg(t);

        let two = panel.inner(area.as_size()).width >= self.two_column_width(ctx);
        let rows = if two {
            CARD_H.max(self.two_column_height() + 2)
        } else {
            CARD_H.max(self.column_height())
        };
        let card = panel.outer(Size::new(0, rows)).height;
        let inner = panel.render(
            Rect::new(area.x, area.y, area.width, area.height.min(card)),
            buf,
            t,
        );
        if !two || inner.height < 11 {
            self.render_column(inner, buf, ctx, bg);
            return;
        }
        let compact = inner.height < 17;
        self.description.rows = if compact { 1 } else { self.desc_rows() };
        let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 2);
        let (l, r) = crate::pages::layout::columns(body, body.width / 2 - 2, 4);

        let mut y = l.y;
        if !compact {
            buf.set_string(l.x, y, "Task", t.faint().bg(bg));
            y += 1;
        }
        self.name
            .render(Rect::new(l.x, y, l.width, TextInput::HEIGHT), buf, ctx, bg);
        y += TextInput::HEIGHT;

        let room = (l.bottom().saturating_sub(y))
            .saturating_sub(TextInput::HEIGHT + 2 * u16::from(!compact));
        self.description.rows = self.description.rows.min(room.saturating_sub(2).max(1));
        self.description.render(
            Rect::new(l.x, y, l.width, self.description.height()),
            buf,
            ctx,
            bg,
        );
        y += self.description.height();
        if !compact {
            y += 1;
            buf.set_string(l.x, y, "Review", t.faint().bg(bg));
            y += 1;
        }
        self.reviewer
            .render(Rect::new(l.x, y, l.width, TextInput::HEIGHT), buf, ctx, bg);

        let mut y = r.y;
        if !compact {
            buf.set_string(r.x, y, "Options", t.faint().bg(bg));
            y += 1;
        }
        self.mode
            .render(Rect::new(r.x, y, r.width, self.mode.height()), buf, ctx, bg);
        y += self.mode.height() + u16::from(!compact);
        self.run_tests
            .render(Rect::new(r.x, y, r.width, 1), buf, ctx, bg);
        y += 1;
        self.open_pr
            .render(Rect::new(r.x, y, r.width, 1), buf, ctx, bg);
        y += 1 + u16::from(!compact);
        self.render_toggles(Rect::new(r.x, y, r.width, 3), buf, ctx, bg);

        let ay = inner.bottom().saturating_sub(1);
        self.render_actions(Rect::new(inner.x, ay, inner.width, 1), buf, ctx, bg);
    }

    fn render_toggles(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        self.auto_approve
            .render(Rect::new(area.x, area.y, area.width, 1), buf, ctx, bg);
        self.notify
            .render(Rect::new(area.x, area.y + 1, area.width, 1), buf, ctx, bg);
        if self.notify.disabled && area.height > 2 {
            buf.set_string(
                area.x + 2,
                area.y + 2,
                termrock::ui::text::truncate(MANAGED, area.width.saturating_sub(2) as usize),
                t.faint().bg(bg),
            );
        }
    }

    fn part_height(&self, part: Part) -> u16 {
        u16::from(part.heading().is_some())
            + match part {
                Part::Name | Part::Reviewer => TextInput::HEIGHT,
                Part::Description => self.desc_height(),
                Part::Mode => self.mode.height(),
                Part::Checks => 2,
                Part::Toggles => 2 + u16::from(self.notify.disabled),
                Part::Actions => 1,
            }
    }

    fn desc_height(&self) -> u16 {
        self.desc_rows() + self.description.height() - self.description.rows
    }

    fn column_widths(&self, ctx: &RenderCtx) -> (u16, u16, u16) {
        let field = |f: &TextInput| f.measure(Size::ZERO, ctx).width;
        let left = field(&self.name)
            .max(field(&self.reviewer))
            .max(self.description.min_width(Size::ZERO, ctx));
        let right = self
            .mode
            .width()
            .max(self.run_tests.width())
            .max(self.open_pr.width())
            .max(self.auto_approve.width())
            .max(self.notify.width())
            .max(2 + termrock::ui::text::width(MANAGED) as u16);
        let actions = self.submit.width() + 2 + self.reset.width();
        (left, right, actions)
    }

    fn two_column_width(&self, ctx: &RenderCtx) -> u16 {
        let (l, r, a) = self.column_widths(ctx);
        (2 * l.max(r) + 5).max(a).max(44)
    }

    fn column_height(&self) -> u16 {
        let parts: Vec<StackChild> = Part::ALL
            .iter()
            .map(|p| StackChild::fixed(self.part_height(*p)))
            .collect();
        self.sections.stack.measure(&parts)
    }

    fn two_column_height(&self) -> u16 {
        let left = 1 + TextInput::HEIGHT + self.desc_height() + 2 + TextInput::HEIGHT;
        let right = 1 + self.mode.height() + 1 + 2 + 1 + 2 + u16::from(self.notify.disabled);
        left.max(right)
    }

    fn part_ids(&self, part: Part) -> Vec<WidgetId> {
        match part {
            Part::Name => vec![self.name.id],
            Part::Description => vec![self.description.id],
            Part::Reviewer => vec![self.reviewer.id],
            Part::Mode => vec![self.mode.id],
            Part::Checks => vec![self.run_tests.id, self.open_pr.id],
            Part::Toggles => vec![self.auto_approve.id, self.notify.id],
            Part::Actions => vec![self.submit.id, self.reset.id],
        }
    }

    fn render_column(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        self.description.rows = self.desc_rows();
        let children: Vec<StackChild> = Part::ALL
            .iter()
            .map(|p| StackChild::fixed(self.part_height(*p)))
            .collect();
        let focus = Part::ALL.iter().position(|p| {
            self.part_ids(*p)
                .iter()
                .any(|&id| ctx.interaction.has_focus(id))
        });
        let l = self.sections.layout(area, &children, focus, buf, ctx);
        for (i, part) in Part::ALL.into_iter().enumerate() {
            let Some(mut r) = l.slot(i) else {
                for id in self.part_ids(part) {
                    if !(id == self.notify.id && self.notify.disabled) {
                        ctx.focus_stop(id);
                    }
                }
                continue;
            };
            if let Some(h) = part.heading() {
                buf.set_string(r.x, r.y, h, ctx.theme.faint().bg(bg));
                r = Rect::new(r.x, r.y + 1, r.width, r.height.saturating_sub(1));
            }
            match part {
                Part::Name => self.name.render(r, buf, ctx, bg),
                Part::Description => self.description.render(r, buf, ctx, bg),
                Part::Reviewer => self.reviewer.render(r, buf, ctx, bg),
                Part::Mode => self.mode.render(r, buf, ctx, bg),
                Part::Checks => {
                    self.run_tests
                        .render(Rect::new(r.x, r.y, r.width, 1), buf, ctx, bg);
                    self.open_pr
                        .render(Rect::new(r.x, r.y + 1, r.width, 1), buf, ctx, bg);
                }
                Part::Toggles => self.render_toggles(r, buf, ctx, bg),
                Part::Actions => self.render_actions(r, buf, ctx, bg),
            }
        }
    }
}

impl FormsPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        for inp in [&mut self.name, &mut self.reviewer] {
            if let Some(r) = inp.route(ev) {
                if click {
                    cx.focus.focus(inp.id);
                }
                return r.outcome();
            }
        }
        if let Some(r) = self.description.route(ev) {
            if click {
                cx.focus.focus(self.description.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.mode.route(ev) {
            if click && r.outcome() != Outcome::Ignored {
                cx.focus.focus(self.mode.id);
            }
            return r.outcome();
        }
        let toggles = [
            self.run_tests.route(ev),
            self.open_pr.route(ev),
            self.auto_approve.route(ev),
            self.notify.route(ev),
        ];
        if let Some(r) = toggles.into_iter().flatten().next() {
            return r.outcome();
        }
        if let Some(r) = self.submit.route(ev) {
            if r.has_actions() {
                self.do_submit(cx);
            }
            return if click {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        if let Some(r) = self.reset.route(ev) {
            if r.has_actions() {
                self.do_reset(cx);
            }
            return if click {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }

        self.sections
            .handle(&PageEvent::Pointer(*ev))
            .unwrap_or(Outcome::Ignored)
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    StateGrid::new(ID.sub("states"))
        .label_width(14)
        .column_width(26)
        .narrow_width(16)
        .rows(&[
            Rest, Hover, Focused, Editing, Error, Disabled, Empty, Narrow,
        ])
        .column(
            "Required field",
            TextInput::new(ID.sub("states-name"), "Task name")
                .required(true)
                .validator(|s| name(s, true))
                .placeholder("Short imperative summary")
                .value("Fix the login flow"),
        )
        .column(
            "Optional field",
            TextInput::new(ID.sub("states-reviewer"), "Reviewer")
                .validator(email)
                .placeholder("name@company.com")
                .help("Optional")
                .value("ana@company.com"),
        )
        .column(
            "Description",
            TextArea::new(ID.sub("states-desc"), "Description", 2)
                .placeholder("What should Junie do?")
                .value("Reproduce the bug, fix it,\nadd a regression test."),
        )
}

impl Page for FormsPage {
    fn title(&self) -> &'static str {
        "Forms"
    }
    fn blurb(&self) -> &'static str {
        "Sections, required fields, validation, submission"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let panel = Panel::card(Some("New task")).meta("Ctrl+S Submit");
        let inner = panel.inner(Size::new(avail.width, 0)).width;
        let two = self.two_column_width(ctx);
        if inner >= two {
            let h = panel
                .outer(Size::new(0, self.two_column_height() + 2))
                .height;
            return Size::new(avail.width, h.max(avail.height.min(CARD_H)));
        }
        let (l, r, a) = self.column_widths(ctx);
        let card = panel.outer(Size::new(l.max(r).max(a), self.column_height()));
        Size::new(avail.width.max(card.width), card.height)
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
        if let PageEvent::Blur(ch) = ev
            && (ch.from == self.name.id
                || ch.from == self.reviewer.id
                || ch.from == self.description.id)
        {
            return forward_blur(
                *ch,
                &mut [&mut self.name, &mut self.reviewer, &mut self.description],
            );
        }
        if let PageEvent::Tick = ev {
            if let Submit::Busy(ticks) = self.state {
                if ticks > 1 {
                    self.state = Submit::Busy(ticks - 1);
                } else {
                    self.state = Submit::Done;
                    self.submit.busy = false;
                    cx.status("Task created ✓");
                }
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }

        if let PageEvent::Key(key) = ev
            && key.ctrl_char('s')
            && self.template.section() == Section::Playground
        {
            self.do_submit(cx);
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
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let route_input = |inp: &mut TextInput, cx: &mut Cx| -> Outcome {
                    let (out, iev) = inp.on_key(key);
                    match iev {
                        Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                        Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                        _ => {}
                    }
                    out
                };
                if f == self.name.id {
                    return route_input(&mut self.name, cx);
                }
                if f == self.reviewer.id {
                    return route_input(&mut self.reviewer, cx);
                }
                if f == self.description.id {
                    let (out, iev) = self.description.on_key(key);
                    match iev {
                        Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                        Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                        _ => {}
                    }
                    return out;
                }
                if f == self.mode.id {
                    return self.mode.on_key(key);
                }
                if f == self.run_tests.id {
                    return self.run_tests.on_key(key);
                }
                if f == self.open_pr.id {
                    return self.open_pr.on_key(key);
                }
                if f == self.auto_approve.id {
                    return self.auto_approve.on_key(key);
                }
                if f == self.notify.id {
                    return self.notify.on_key(key);
                }
                if f == self.submit.id {
                    let (out, act) = self.submit.on_key(key);
                    if act {
                        self.do_submit(cx);
                    }
                    return out;
                }
                if f == self.reset.id {
                    let (out, act) = self.reset.on_key(key);
                    if act {
                        self.do_reset(cx);
                    }
                    return out;
                }
                Outcome::Ignored
            }
            PageEvent::Paste(text) => {
                if self.name.editing {
                    return self.name.on_paste(text);
                }
                if self.reviewer.editing {
                    return self.reviewer.on_paste(text);
                }
                if self.description.editing {
                    return self.description.on_paste(text);
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.name.editing || self.reviewer.editing || self.description.editing
    }

    fn animating(&self) -> bool {
        matches!(self.state, Submit::Busy(_))
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.description.editing {
            return vec![("Esc", "Done"), ("Tab", "Next field")];
        }
        if self.editing() {
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next field")];
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        match focus {
            Some(f) if f == self.mode.id => vec![("↑↓", "Choose"), ("Ctrl+S", "Submit")],
            Some(f)
                if f == self.run_tests.id || f == self.open_pr.id || f == self.auto_approve.id =>
            {
                vec![("Space", "Toggle"), ("Ctrl+S", "Submit")]
            }
            Some(f) if f == self.submit.id || f == self.reset.id => {
                vec![("Enter", "Activate"), ("Ctrl+S", "Submit")]
            }
            _ => vec![
                termrock::widgets::widget_hints::EDIT_PAIR,
                ("Ctrl+S", "Submit"),
            ],
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
