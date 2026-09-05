// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/dialogs.rs (MIT).
//
// API gap: catalog shell has no Request::OpenDialog / page-owned overlay layer
// (shell.rs help dialog is separate and must not be edited). This page paints
// termrock::widgets::Dialog via Dialog::paint_modal over the page rect and
// traps keys locally. Backdrop does not cover the catalog sidebar/header;
// Esc/Enter are consumed here so the shell does not steal them.

//! Focus is trapped, the page dims, Esc always cancels.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::widgets::{
    Action, ActionVariant, Button, ButtonState, ButtonVariant, Dialog, DialogClosePolicy,
    DialogFocusZone, DialogOutcome, DialogSize, DialogState, TextInput, TextInputOutcome,
    TextInputState, Validation,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("dialogs");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Confirm,
    Rename,
    Choice,
    Delete,
}

struct DemoButton {
    id: WidgetId,
    label: &'static str,
    variant: ButtonVariant,
    state: ButtonState,
}

fn rename_validator(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        Some("Name cannot be empty".into())
    } else if s.len() > 40 {
        Some("Keep it under 40 characters".into())
    } else {
        None
    }
}

pub struct DialogsPage {
    buttons: Vec<DemoButton>,
    history: Vec<String>,
    task_name: String,
    open: Option<Kind>,
    dialog: DialogState<&'static str>,
    input: TextInputState,
}

impl DialogsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buttons: vec![
                DemoButton {
                    id: ID.child(0),
                    label: "Confirm run",
                    variant: ButtonVariant::Primary,
                    state: ButtonState::new(),
                },
                DemoButton {
                    id: ID.child(1),
                    label: "Rename task…",
                    variant: ButtonVariant::Secondary,
                    state: ButtonState::new(),
                },
                DemoButton {
                    id: ID.child(2),
                    label: "Three choices…",
                    variant: ButtonVariant::Secondary,
                    state: ButtonState::new(),
                },
                DemoButton {
                    id: ID.child(3),
                    label: "Delete branch…",
                    variant: ButtonVariant::Destructive,
                    state: ButtonState::new(),
                },
            ],
            history: vec![],
            task_name: "Migrate sessions table".into(),
            open: None,
            dialog: DialogState::new(),
            input: TextInputState::new("Migrate sessions table"),
        }
    }

    fn open_kind(&mut self, i: usize) {
        let kind = match i {
            0 => Kind::Confirm,
            1 => Kind::Rename,
            2 => Kind::Choice,
            _ => Kind::Delete,
        };
        self.dialog = match kind {
            Kind::Confirm => DialogState::confirm("run", "cancel"),
            Kind::Rename => DialogState::prompt("rename", "cancel"),
            Kind::Choice => {
                let mut s = DialogState::new();
                s.set_close_policy(DialogClosePolicy::ConfirmOnly);
                s.set_cancel_action(Some("cancel"));
                s.set_default_action(Some("save"));
                s.set_action_cursor(Some("save"));
                s.set_require_action_focus_for_enter(false);
                s
            }
            Kind::Delete => DialogState::destructive("delete", "cancel"),
        };
        if kind == Kind::Rename {
            self.input = TextInputState::new(self.task_name.clone());
            self.input.set_editing(true);
        }
        self.open = Some(kind);
    }

    fn actions(kind: Kind) -> Vec<Action<'static, &'static str>> {
        match kind {
            Kind::Confirm => vec![
                Action {
                    id: "cancel",
                    label: "Cancel",
                    enabled: true,
                    variant: ActionVariant::Secondary,
                },
                Action {
                    id: "run",
                    label: "Run",
                    enabled: true,
                    variant: ActionVariant::Primary,
                },
            ],
            Kind::Rename => vec![
                Action {
                    id: "cancel",
                    label: "Cancel",
                    enabled: true,
                    variant: ActionVariant::Secondary,
                },
                Action {
                    id: "rename",
                    label: "Rename",
                    enabled: true,
                    variant: ActionVariant::Primary,
                },
            ],
            Kind::Choice => vec![
                Action {
                    id: "cancel",
                    label: "Cancel",
                    enabled: true,
                    variant: ActionVariant::Secondary,
                },
                Action {
                    id: "discard",
                    label: "Discard",
                    enabled: true,
                    variant: ActionVariant::Secondary,
                },
                Action {
                    id: "save",
                    label: "Save",
                    enabled: true,
                    variant: ActionVariant::Primary,
                },
            ],
            Kind::Delete => vec![
                Action {
                    id: "cancel",
                    label: "Cancel",
                    enabled: true,
                    variant: ActionVariant::Secondary,
                },
                Action {
                    id: "delete",
                    label: "Delete branch",
                    enabled: true,
                    variant: ActionVariant::Destructive,
                },
            ],
        }
    }

    fn close(&mut self, action: &'static str, cx: &mut PageCtx<'_>) {
        let kind = self.open.take();
        let (label, msg) = match (kind, action) {
            (Some(Kind::Confirm), "run") => ("Run", "Task started".to_owned()),
            (Some(Kind::Delete), "delete") => {
                ("Delete", "Branch feat/rate-limit deleted".to_owned())
            }
            (Some(Kind::Rename), "rename") => {
                self.task_name = self.input.value().to_owned();
                ("Rename", format!("Renamed to “{}”", self.task_name))
            }
            (Some(Kind::Choice), "save") => ("Save", "Description saved".to_owned()),
            (Some(Kind::Choice), "discard") => ("Discard", "Changes discarded".to_owned()),
            _ => ("Cancel", "Cancelled".to_owned()),
        };
        self.history.push(format!("{label:<8} {msg}"));
        cx.status(msg);
    }

    fn apply_dialog(&mut self, out: DialogOutcome<&'static str>, cx: &mut PageCtx<'_>) -> Route {
        match out {
            DialogOutcome::Ignored | DialogOutcome::LoadingBlocked => Route::Consumed,
            DialogOutcome::Activated(id) | DialogOutcome::DefaultActivated(id) => {
                self.close(id, cx);
                Route::Changed
            }
            DialogOutcome::Cancelled => {
                self.close("cancel", cx);
                Route::Changed
            }
            DialogOutcome::ValidationFailed => Route::Changed,
            DialogOutcome::FocusMoved | DialogOutcome::Scrolled | DialogOutcome::TypedChanged => {
                Route::Changed
            }
            _ => Route::Changed,
        }
    }
}

impl Page for DialogsPage {
    fn title(&self) -> &'static str {
        "Dialogs"
    }
    fn blurb(&self) -> &'static str {
        "Focus is trapped, the page dims, Esc always cancels"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[9, 1, 0]);
        let (inner, bg) = layout::card(rows[0], buf, t, Some("Open a dialog"), None, false);
        let mut x = inner.x;
        for b in &mut self.buttons {
            b.state.focused = ctx.interaction.focused(b.id) && self.open.is_none();
            b.state.hovered = ctx.interaction.hovered(b.id) && self.open.is_none();
            b.state.activation.set_accepts_input(self.open.is_none());
            let w = Button::new(b.label, ctx.system)
                .variant(b.variant)
                .container(bg)
                .preferred_width()
                .min(inner.right().saturating_sub(x));
            let r = Rect::new(x, inner.y, w, 1);
            let _ = Button::new(b.label, ctx.system)
                .variant(b.variant)
                .container(bg)
                .paint(r, buf, &mut b.state);
            if self.open.is_none() {
                ctx.control(b.id, r, false);
            }
            x = x.saturating_add(w).saturating_add(2);
        }
        let notes = [
            "Confirm: primary action focused first · y / n answer directly",
            "Prompt: editing inside a modal, Enter submits, validation blocks",
            "Destructive: Cancel focused first, action in danger style",
        ];
        for (i, n) in notes.iter().enumerate() {
            buf.set_string(
                inner.x,
                inner.y + 2 + i as u16,
                text::truncate(n, inner.width as usize),
                t.muted().bg(bg),
            );
        }
        buf.set_string(
            inner.x,
            inner.y + 5,
            format!("Task: {}", self.task_name),
            t.secondary().bg(bg),
        );

        let (inner, bg) = layout::card(
            Rect::new(rows[2].x, rows[2].y, rows[2].width, rows[2].height.min(12)),
            buf,
            t,
            Some("Results"),
            None,
            false,
        );
        if self.history.is_empty() {
            buf.set_string(inner.x, inner.y, "Nothing yet", t.muted().bg(bg));
        }
        for (i, h) in self
            .history
            .iter()
            .rev()
            .take(inner.height as usize)
            .enumerate()
        {
            let st = if i == 0 { t.primary() } else { t.muted() };
            buf.set_string(
                inner.x,
                inner.y + i as u16,
                text::truncate(h, inner.width as usize),
                st.bg(bg),
            );
        }

        if let Some(kind) = self.open {
            self.dialog.set_open(true);
            self.dialog.set_accepts_input(true);
            let actions = Self::actions(kind);
            // The source host places the modal against the full terminal
            // canvas. The footer is part of the modal contract, not a second
            // inset screen; using a shortened canvas shifts the frame down.
            let modal_screen = *buf.area();
            match kind {
                Kind::Confirm => {
                    Dialog::confirm(
                        "Run task now?",
                        Text::from(
                            "Junie will check out chore/uuid-sessions, apply the plan and run the test suite. You can pause at any step.",
                        ),
                        ctx.system,
                    )
                    .preferred_size(DialogSize {
                        width: 54,
                        height: 11,
                    })
                    .paint_modal(modal_screen, buf, &mut self.dialog, &actions);
                }
                Kind::Rename => {
                    let err = rename_validator(self.input.value());
                    self.dialog.set_validation_message(err.clone());
                    Dialog::prompt("Rename task", Text::from(""), ctx.system).paint_modal(
                        modal_screen,
                        buf,
                        &mut self.dialog,
                        &actions,
                    );
                    let body = self.dialog.slots().body;
                    if !body.is_empty() {
                        let validation = if let Some(e) = err.as_deref() {
                            Validation::Invalid(e)
                        } else {
                            Validation::Valid
                        };
                        let _ = TextInput::new("Task name", ctx.system)
                            .required(true)
                            .help("Shown in the task list and PR title")
                            .validation(validation)
                            .paint(body, buf, &mut self.input);
                    }
                }
                Kind::Choice => {
                    Dialog::confirm(
                        "Unsaved changes",
                        Text::from("The description was edited. Save before leaving this page?"),
                        ctx.system,
                    )
                    .preferred_size(DialogSize {
                        width: 54,
                        height: 11,
                    })
                    .paint_modal(modal_screen, buf, &mut self.dialog, &actions);
                }
                Kind::Delete => {
                    Dialog::destructive(
                        "Delete branch?",
                        Text::from(
                            "feat/rate-limit has 14 commits that are not on main. This cannot be undone.",
                        ),
                        ctx.system,
                    )
                    .preferred_size(DialogSize {
                        width: 54,
                        height: 10,
                    })
                    .paint_modal(modal_screen, buf, &mut self.dialog, &actions);
                }
            }
            ctx.control(ID.sub("modal"), area, false);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if let Some(kind) = self.open {
            let actions = Self::actions(kind);
            match ev {
                PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    if kind == Kind::Rename
                        && self.dialog.focus_zone() == DialogFocusZone::Body
                        && !matches!(key.code, KeyCode::Esc)
                    {
                        if matches!(key.code, KeyCode::Enter) {
                            if let Some(err) = rename_validator(self.input.value()) {
                                self.dialog.set_validation_message(Some(err));
                                return Route::Changed;
                            }
                            self.close("rename", cx);
                            return Route::Changed;
                        }
                        let out = self.input.handle_key(*key);
                        return match out {
                            TextInputOutcome::Ignored => Route::Consumed,
                            _ => Route::Changed,
                        };
                    }
                    let out = self.dialog.handle_key(*key, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        return Route::Consumed;
                    }
                    return self.apply_dialog(out, cx);
                }
                PageEvent::Paste(text) if kind == Kind::Rename => {
                    let _ = self.input.insert_str(text);
                    Route::Changed
                }
                PageEvent::Click { pos, .. } => {
                    let out = self.dialog.handle_click(*pos, &actions);
                    if matches!(out, DialogOutcome::Ignored) {
                        return Route::Consumed;
                    }
                    self.apply_dialog(out, cx)
                }
                _ => Route::Consumed,
            }
        } else {
            match ev {
                PageEvent::Key(key)
                    if key.kind != KeyEventKind::Release
                        && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) =>
                {
                    let Some(f) = cx.focus_id() else {
                        return Route::Ignored;
                    };
                    let Some(i) = self.buttons.iter().position(|b| b.id == f) else {
                        return Route::Ignored;
                    };
                    self.open_kind(i);
                    Route::Changed
                }
                PageEvent::Click { id, .. } => {
                    let Some(i) = self.buttons.iter().position(|b| b.id == *id) else {
                        return Route::Ignored;
                    };
                    self.open_kind(i);
                    Route::Changed
                }
                _ => Route::Ignored,
            }
        }
    }

    fn editing(&self) -> bool {
        matches!(self.open, Some(Kind::Rename))
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![("Enter", "Open")]
    }

    fn overlaying(&self) -> bool {
        self.open.is_some()
    }

    fn capture_cursor(&self) -> Option<ratatui::layout::Position> {
        (self.open == Some(Kind::Delete)).then(|| {
            self.dialog
                .action_regions()
                .first()
                .map_or(ratatui::layout::Position::ORIGIN, |region| {
                    ratatui::layout::Position::new(region.area.right(), region.area.y)
                })
        })
    }
}
