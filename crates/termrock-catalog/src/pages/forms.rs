// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/forms.rs (MIT).
// Composes public widgets (TextInput, TextArea, RadioGroup, Checkbox, Switch,
// Button) the way the source page composes its live fields. Form is field-chrome
// only and is not a host for those widgets — see COORDINATION.md gaps.

//! Sections, required fields, validation, submission.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::widgets::{
    ActivationOutcome, Button, ButtonState, ButtonVariant, Checkbox, CheckboxOutcome,
    CheckboxState, RadioGroup, RadioOption, RadioOutcome, RadioState, Switch, SwitchOutcome,
    SwitchState, TextArea, TextAreaOutcome, TextAreaState, TextInput, TextInputOutcome,
    TextInputState, Validation,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("forms");
const FIELD_H: u16 = 3;
const MODES: [&str; 3] = ["Fast", "Balanced", "Thorough"];

fn email(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        None
    } else if !s.contains('@') || !s.contains('.') {
        Some("Enter a valid email address")
    } else {
        None
    }
}

fn name(s: &str) -> Option<&'static str> {
    if s.trim().is_empty() {
        Some("Required")
    } else if s.len() < 4 {
        Some("At least 4 characters")
    } else {
        None
    }
}

fn is_tab(key: &termrock::input::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
}

fn mouse_down(pos: Position) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        position: pos,
        modifiers: KeyModifiers::NONE,
    }
}

fn row_layout(area: Rect, widths: &[u16], gap: u16) -> Vec<Rect> {
    let mut x = area.x;
    widths
        .iter()
        .map(|&w| {
            let w = w.min(area.right().saturating_sub(x));
            let r = Rect::new(x, area.y, w, area.height);
            x = x.saturating_add(w).saturating_add(gap);
            r
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Submit {
    Idle,
    Busy(Instant),
    Done,
}

pub struct FormsPage {
    name: TextInputState,
    description: TextAreaState,
    mode: RadioState<u8>,
    run_tests: CheckboxState,
    open_pr: CheckboxState,
    auto_approve: SwitchState,
    notify: SwitchState,
    reviewer: TextInputState,
    submit: ButtonState,
    reset: ButtonState,
    state: Submit,
    attempted: bool,
    name_touched: bool,
    reviewer_touched: bool,
}

impl FormsPage {
    #[must_use]
    pub fn new() -> Self {
        let mut name = TextInputState::new("").with_allow_empty(false);
        name.set_editing(false);
        let mut description = TextAreaState::new("");
        description.set_editing(false);
        let mut reviewer = TextInputState::new("").with_allow_empty(true);
        reviewer.set_editing(false);
        let mut notify = SwitchState::new(true);
        notify.set_enabled(false);
        Self {
            name,
            description,
            mode: RadioState::new(Some(1)),
            run_tests: CheckboxState::new(true),
            open_pr: CheckboxState::new(false),
            auto_approve: SwitchState::new(false),
            notify,
            reviewer,
            submit: ButtonState::new(),
            reset: ButtonState::new(),
            state: Submit::Idle,
            attempted: false,
            name_touched: false,
            reviewer_touched: false,
        }
    }

    fn name_error(&self) -> Option<&'static str> {
        if self.attempted || self.name_touched {
            name(self.name.value())
        } else {
            None
        }
    }

    fn reviewer_error(&self) -> Option<&'static str> {
        if self.attempted || self.reviewer_touched {
            email(self.reviewer.value())
        } else {
            None
        }
    }

    fn validate(&self) -> bool {
        name(self.name.value()).is_none() && email(self.reviewer.value()).is_none()
    }

    fn do_submit(&mut self, cx: &mut PageCtx<'_>) {
        self.attempted = true;
        self.name_touched = true;
        self.reviewer_touched = true;
        if self.name.is_editing() {
            self.name.commit();
        }
        if self.reviewer.is_editing() {
            self.reviewer.commit();
        }
        if self.description.is_editing() {
            self.description.set_editing(false);
        }
        if !self.validate() {
            cx.status("Fix the highlighted fields");
            if name(self.name.value()).is_some() {
                cx.set_focus(ID.sub("name"));
            } else {
                cx.set_focus(ID.sub("reviewer"));
            }
            return;
        }
        self.submit.activation.set_loading(true);
        self.state = Submit::Busy(Instant::now());
        cx.status("Creating task…");
    }

    fn do_reset(&mut self, cx: &mut PageCtx<'_>) {
        *self = Self::new();
        cx.set_focus(ID.sub("name"));
        cx.status("Form reset");
    }
}

impl Page for FormsPage {
    fn title(&self) -> &'static str {
        "Forms"
    }
    fn blurb(&self) -> &'static str {
        "Sections, required fields, validation, submission"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let (inner, bg) = layout::card(
            Rect::new(area.x, area.y, area.width, area.height.min(24)),
            buf,
            t,
            Some("New task"),
            Some("Ctrl+S Submit"),
            false,
        );
        let (l, r) = layout::columns(inner, inner.width / 2 - 2, 4);

        let mut y = l.y;
        buf.set_string(l.x, y, "Task", t.faint().bg(bg));
        y += 1;
        self.name
            .set_focused(ctx.interaction.focused(ID.sub("name")));
        self.name
            .set_hovered(ctx.interaction.hovered(ID.sub("name")) && !self.name.is_editing());
        let name_err = self.name_error();
        let name_v = match name_err {
            Some(msg) => Validation::Invalid(msg),
            None => Validation::Valid,
        };
        let parts = TextInput::new("Task name", ctx.system)
            .required(true)
            .placeholder("Short imperative summary")
            .validation(name_v)
            .paint(Rect::new(l.x, y, l.width, FIELD_H), buf, &mut self.name);
        ctx.control(ID.sub("name"), Rect::new(l.x, y, l.width, FIELD_H), false);
        if self.name.is_editing()
            && let Some(cur) = parts.cursor
        {
            ctx.set_cursor(Position::new(cur.x, cur.y));
        }
        y += FIELD_H;

        let desc_h = 6u16.min(l.bottom().saturating_sub(y));
        let desc_area = Rect::new(l.x, y, l.width, desc_h);
        self.description
            .set_accepts_input(ctx.interaction.focused(ID.sub("desc")));
        TextArea::new(ctx.system)
            .title("Description")
            .placeholder("What should Junie do, and what does done look like?")
            .help("Optional · Markdown")
            .rows(4)
            .render(desc_area, buf, &mut self.description);
        ctx.control(ID.sub("desc"), desc_area, false);
        ctx.scrollable(ID.sub("desc"), desc_area);
        if self.description.is_editing()
            && let Some(cur) = self.description.cursor_cell()
        {
            ctx.set_cursor(cur);
        }
        y = y.saturating_add(desc_h).saturating_add(1);
        if y < l.bottom() {
            buf.set_string(l.x, y, "Review", t.faint().bg(bg));
            y += 1;
        }
        if y + FIELD_H <= l.bottom() {
            self.reviewer
                .set_focused(ctx.interaction.focused(ID.sub("reviewer")));
            self.reviewer.set_hovered(
                ctx.interaction.hovered(ID.sub("reviewer")) && !self.reviewer.is_editing(),
            );
            let rev_err = self.reviewer_error();
            let rev_v = match rev_err {
                Some(msg) => Validation::Invalid(msg),
                None => Validation::Valid,
            };
            let parts = TextInput::new("Reviewer", ctx.system)
                .placeholder("name@company.com")
                .help("Optional")
                .optional(true)
                .validation(rev_v)
                .paint(Rect::new(l.x, y, l.width, FIELD_H), buf, &mut self.reviewer);
            ctx.control(
                ID.sub("reviewer"),
                Rect::new(l.x, y, l.width, FIELD_H),
                false,
            );
            if self.reviewer.is_editing()
                && let Some(cur) = parts.cursor
            {
                ctx.set_cursor(Position::new(cur.x, cur.y));
            }
        }

        let mut y = r.y;
        buf.set_string(r.x, y, "Options", t.faint().bg(bg));
        y += 1;
        let mode_opts = [
            RadioOption::new(0, MODES[0]),
            RadioOption::new(1, MODES[1]),
            RadioOption::new(2, MODES[2]),
        ];
        let mode_h = (MODES.len() as u16).saturating_add(1);
        let mode_area = Rect::new(r.x, y, r.width, mode_h.min(r.bottom().saturating_sub(y)));
        self.mode
            .set_surface_focused(ctx.interaction.focused(ID.sub("mode")));
        RadioGroup::new(&mode_opts, ctx.system)
            .legend("Mode")
            .paint(mode_area, buf, &mut self.mode);
        ctx.control(ID.sub("mode"), mode_area, false);
        y = y.saturating_add(mode_h).saturating_add(1);

        self.run_tests
            .set_focused(ctx.interaction.focused(ID.sub("tests")));
        self.run_tests.hovered = ctx.interaction.hovered(ID.sub("tests"));
        let tests_area = Rect::new(r.x, y, r.width, 1);
        Checkbox::new(ID.sub("tests"), "Run tests before opening a PR", ctx.system).paint(
            tests_area,
            buf,
            &mut self.run_tests,
        );
        ctx.control(ID.sub("tests"), tests_area, false);
        y += 1;

        self.open_pr
            .set_focused(ctx.interaction.focused(ID.sub("pr")));
        self.open_pr.hovered = ctx.interaction.hovered(ID.sub("pr"));
        let pr_area = Rect::new(r.x, y, r.width, 1);
        Checkbox::new(ID.sub("pr"), "Open a pull request when done", ctx.system).paint(
            pr_area,
            buf,
            &mut self.open_pr,
        );
        ctx.control(ID.sub("pr"), pr_area, false);
        y += 2;

        self.auto_approve
            .set_focused(ctx.interaction.focused(ID.sub("auto")));
        self.auto_approve.hovered = ctx.interaction.hovered(ID.sub("auto"));
        let auto_area = Rect::new(r.x, y, r.width, 1);
        Switch::new(ID.sub("auto"), "Auto-approve changes", ctx.system)
            .compact()
            .paint(auto_area, buf, &mut self.auto_approve);
        ctx.control(ID.sub("auto"), auto_area, false);
        y += 1;

        self.notify
            .set_focused(ctx.interaction.focused(ID.sub("notify")));
        self.notify.hovered = ctx.interaction.hovered(ID.sub("notify"));
        let notify_area = Rect::new(r.x, y, r.width, 1);
        Switch::new(ID.sub("notify"), "Notify on completion", ctx.system)
            .compact()
            .paint(notify_area, buf, &mut self.notify);
        ctx.control(ID.sub("notify"), notify_area, true);
        y += 1;
        if y < r.bottom() {
            buf.set_string(r.x + 2, y, "Managed by your organization", t.faint().bg(bg));
        }

        let ay = inner.bottom().saturating_sub(1);
        let submit_btn = Button::new("Create task", ctx.system)
            .as_primary()
            .container(bg);
        let reset_btn = Button::new("Reset", ctx.system)
            .variant(ButtonVariant::Quiet)
            .container(bg);
        let widths = [submit_btn.preferred_width(), reset_btn.preferred_width()];
        let rects = row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
        let busy = matches!(self.state, Submit::Busy(_));
        self.submit.focused = ctx.interaction.focused(ID.sub("submit"));
        self.submit.hovered = ctx.interaction.hovered(ID.sub("submit"));
        self.submit.activation.set_accepts_input(!busy);
        self.submit.activation.set_loading(busy);
        let _ = submit_btn.paint(rects[0], buf, &mut self.submit);
        ctx.control(ID.sub("submit"), rects[0], busy);
        self.reset.focused = ctx.interaction.focused(ID.sub("reset"));
        self.reset.hovered = ctx.interaction.hovered(ID.sub("reset"));
        self.reset.activation.set_accepts_input(true);
        let _ = reset_btn.paint(rects[1], buf, &mut self.reset);
        ctx.control(ID.sub("reset"), rects[1], false);
        let msg = match self.state {
            Submit::Idle if self.attempted && !self.validate() => {
                Some(("Fix the highlighted fields", t.error_fg()))
            }
            Submit::Busy(_) => Some(("Creating task…", t.secondary())),
            Submit::Done => Some(("Task created ✓", t.accent_fg())),
            _ => None,
        };
        if let Some((m, st)) = msg {
            let x = rects[1].right() + 3;
            if x + m.len() as u16 <= inner.right() {
                buf.set_string(x, ay, m, st.bg(bg));
            }
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Tick => {
                if let Submit::Busy(at) = self.state
                    && at.elapsed() > Duration::from_millis(1800)
                {
                    self.state = Submit::Done;
                    self.submit.activation.set_loading(false);
                    cx.status("Task created ✓");
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
                {
                    self.do_submit(cx);
                    return Route::Changed;
                }
                if is_tab(key) {
                    if self.name.is_editing() {
                        self.name.commit();
                        self.name_touched = true;
                    }
                    if self.reviewer.is_editing() {
                        self.reviewer.commit();
                        self.reviewer_touched = true;
                    }
                    if self.description.is_editing() {
                        self.description.set_editing(false);
                    }
                    return Route::Ignored;
                }
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == ID.sub("name") {
                    return match self.name.handle_key(*key) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        TextInputOutcome::Submitted(_) => {
                            self.name_touched = true;
                            Route::Changed
                        }
                        TextInputOutcome::Cancelled => Route::Changed,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("reviewer") {
                    return match self.reviewer.handle_key(*key) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        TextInputOutcome::Submitted(_) => {
                            self.reviewer_touched = true;
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("desc") {
                    self.description.set_accepts_input(true);
                    return match self.description.handle_key(*key) {
                        TextAreaOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("mode") {
                    self.mode.set_surface_focused(true);
                    let opts = [
                        RadioOption::new(0, MODES[0]),
                        RadioOption::new(1, MODES[1]),
                        RadioOption::new(2, MODES[2]),
                    ];
                    let sys = termrock::style::DesignSystem::junie();
                    return match RadioGroup::new(&opts, &sys).handle_key(&mut self.mode, *key) {
                        RadioOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("tests") {
                    self.run_tests.set_focused(true);
                    return match self.run_tests.handle_key(*key, &ID.sub("tests")) {
                        CheckboxOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("pr") {
                    self.open_pr.set_focused(true);
                    return match self.open_pr.handle_key(*key, &ID.sub("pr")) {
                        CheckboxOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("auto") {
                    self.auto_approve.set_focused(true);
                    return match self.auto_approve.handle_key(*key, &ID.sub("auto")) {
                        SwitchOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("notify") {
                    return Route::Consumed;
                }
                if f == ID.sub("submit") {
                    self.submit.activation.set_accepts_input(true);
                    return match self.submit.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.do_submit(cx);
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ID.sub("reset") {
                    self.reset.activation.set_accepts_input(true);
                    return match self.reset.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.do_reset(cx);
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                Route::Ignored
            }
            PageEvent::Paste(text) => {
                if self.name.is_editing() {
                    return match self.name.insert_str(text) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if self.reviewer.is_editing() {
                    return match self.reviewer.insert_str(text) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if self.description.is_editing() {
                    return match self.description.insert_text(text) {
                        TextAreaOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == ID.sub("name") {
                    cx.set_focus(*id);
                    self.name.set_focused(true);
                    if let Some(parts) = self.name.parts().cloned() {
                        let _ = self.name.handle_mouse(mouse_down(*pos), parts.field);
                    }
                    return Route::Changed;
                }
                if *id == ID.sub("reviewer") {
                    cx.set_focus(*id);
                    self.reviewer.set_focused(true);
                    if let Some(parts) = self.reviewer.parts().cloned() {
                        let _ = self.reviewer.handle_mouse(mouse_down(*pos), parts.field);
                    }
                    return Route::Changed;
                }
                if *id == ID.sub("desc") {
                    let was = cx.focus_id() == Some(*id);
                    cx.set_focus(*id);
                    self.description.set_accepts_input(true);
                    if was && !self.description.is_editing() {
                        self.description.set_editing(true);
                    }
                    let _ = self
                        .description
                        .handle_event(Event::Mouse(mouse_down(*pos)));
                    return Route::Changed;
                }
                if *id == ID.sub("mode") {
                    cx.set_focus(*id);
                    self.mode.set_surface_focused(true);
                    let opts = [
                        RadioOption::new(0, MODES[0]),
                        RadioOption::new(1, MODES[1]),
                        RadioOption::new(2, MODES[2]),
                    ];
                    let sys = termrock::style::DesignSystem::junie();
                    let _ =
                        RadioGroup::new(&opts, &sys).handle_mouse(&mut self.mode, mouse_down(*pos));
                    return Route::Changed;
                }
                if *id == ID.sub("tests") {
                    cx.set_focus(*id);
                    self.run_tests.set_focused(true);
                    if self.run_tests.can_activate() {
                        self.run_tests.set_checked(!self.run_tests.is_checked());
                    }
                    return Route::Changed;
                }
                if *id == ID.sub("pr") {
                    cx.set_focus(*id);
                    self.open_pr.set_focused(true);
                    if self.open_pr.can_activate() {
                        self.open_pr.set_checked(!self.open_pr.is_checked());
                    }
                    return Route::Changed;
                }
                if *id == ID.sub("auto") {
                    cx.set_focus(*id);
                    self.auto_approve.set_focused(true);
                    if self.auto_approve.can_activate() {
                        self.auto_approve.set_on(!self.auto_approve.is_on());
                    }
                    return Route::Changed;
                }
                if *id == ID.sub("notify") {
                    return Route::Changed;
                }
                if *id == ID.sub("submit") {
                    self.do_submit(cx);
                    return Route::Changed;
                }
                if *id == ID.sub("reset") {
                    self.do_reset(cx);
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.name.is_editing() || self.reviewer.is_editing() || self.description.is_editing()
    }

    fn animating(&self) -> bool {
        matches!(self.state, Submit::Busy(_))
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.editing() {
            return vec![
                ("Enter", "Commit"),
                ("Esc", "Cancel"),
                ("Tab", "Next field"),
            ];
        }
        match focus {
            Some(f) if f == ID.sub("mode") => vec![("↑ ↓", "Choose"), ("Ctrl+S", "Submit")],
            Some(f) if f == ID.sub("tests") || f == ID.sub("pr") || f == ID.sub("auto") => {
                vec![("Space", "Toggle"), ("Ctrl+S", "Submit")]
            }
            Some(f) if f == ID.sub("submit") || f == ID.sub("reset") => {
                vec![("Enter", "Activate"), ("Ctrl+S", "Submit")]
            }
            _ => vec![("Enter", "Edit"), ("Ctrl+S", "Submit")],
        }
    }
}
