// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/inputs.rs (MIT).
// Interactive fields use termrock::widgets::TextInput. The state matrix is
// painted through JunieTheme::field_style / gutter / placeholder (public
// resolvers), matching the source reference grid.

//! Focus is a bar; editing is a cursor.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use termrock::input::{
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::style::VisualState;
use termrock::widgets::{TextInput, TextInputOutcome, TextInputState, Validation};

use crate::ctx::RenderCtx;
use crate::draw::fill;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("inputs");
const FIELD_H: u16 = 3;

struct Field {
    id: WidgetId,
    label: &'static str,
    placeholder: &'static str,
    help: &'static str,
    required: bool,
    disabled: bool,
    email: bool,
    state: TextInputState,
}

impl Field {
    fn new(id: WidgetId, label: &'static str, value: &str, allow_empty: bool) -> Self {
        let mut state = TextInputState::new(value).with_allow_empty(allow_empty);
        state.set_editing(false);
        Self {
            id,
            label,
            placeholder: "",
            help: "",
            required: false,
            disabled: false,
            email: false,
            state,
        }
    }

    fn current_error(&self) -> Option<&'static str> {
        if self.email {
            email_error(self.state.value())
        } else if self.required && self.state.value().trim().is_empty() && !self.state.is_editing()
        {
            Some("Required")
        } else {
            None
        }
    }
}

fn email_error(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        Some("Required")
    } else if !s.contains('@') || !s.contains('.') {
        Some("Enter a valid email address")
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

pub struct InputsPage {
    fields: Vec<Field>,
}

impl InputsPage {
    #[must_use]
    pub fn new() -> Self {
        let mut owner = Field::new(ID.child(2), "Owner email", "mira@example", false);
        owner.required = true;
        owner.email = true;
        let fields = vec![
            {
                let mut f = Field::new(ID.child(0), "Project name", "payments-gateway", false);
                f.required = true;
                f.help = "Used as the working directory name";
                f
            },
            {
                let mut f = Field::new(ID.child(1), "Branch", "", true);
                f.placeholder = "feat/…";
                f.help = "Leave empty to work on a detached checkout";
                f
            },
            owner,
            {
                let mut f = Field::new(ID.child(3), "API token", "jb_live_••••••••••••", true);
                f.disabled = true;
                f.state.set_enabled(false);
                f.help = "Managed by the organization";
                f
            },
            {
                let mut f = Field::new(ID.child(4), "Search files", "", true);
                f.placeholder = "Type a path or symbol…";
                f.help = "Selection: Shift+← →  ·  words: Ctrl+← →  ·  clear: Ctrl+U";
                f
            },
        ];
        Self { fields }
    }

    fn focused_index(&self, focus: Option<WidgetId>) -> Option<usize> {
        let f = focus?;
        self.fields.iter().position(|i| i.id == f)
    }
}

fn static_field(
    buf: &mut Buffer,
    t: &termrock::style::JunieTheme,
    at: Rect,
    label: &str,
    value: &str,
    s: VisualState,
) {
    let bg = t.surface;
    let (x, y, w) = (at.x, at.y, at.width);
    buf.set_string(x, y, label, t.secondary().bg(bg));
    let field = Rect::new(x + 16, y, w, 1);
    let fs = t.field_style(s);
    fill(buf, field, fs);
    buf.set_string(field.x, y, "▎", t.gutter(s, fs.bg.unwrap_or(bg), false));
    let style = if value.starts_with('(') {
        t.placeholder(s)
    } else {
        fs
    };
    let style = if s.editing {
        style
            .add_modifier(Modifier::UNDERLINED)
            .underline_color(t.accent)
    } else {
        style
    };
    buf.set_string(field.x + 2, y, value, style);
    if s.editing {
        let cx = field.x + 2 + text::width(value) as u16;
        buf.set_string(cx, y, " ", ratatui::style::Style::new().bg(t.text_primary));
    }
    if s.error {
        buf.set_string(
            field.right() - 2,
            y,
            "!",
            fs.fg(t.error).add_modifier(Modifier::BOLD),
        );
    }
}

impl Page for InputsPage {
    fn title(&self) -> &'static str {
        "Inputs"
    }
    fn blurb(&self) -> &'static str {
        "Focus is a bar; editing is a cursor. Enter to edit, Esc to revert."
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[13, 1, 0]);
        let (inner, bg) = layout::card(
            rows[0],
            buf,
            t,
            Some("Playground"),
            Some("Enter Edit · Esc Cancel · Tab Commit + next"),
            false,
        );
        let (l, r) = layout::columns(inner, inner.width / 2 - 2, 4);
        let slots = [
            Rect::new(l.x, l.y, l.width, FIELD_H),
            Rect::new(r.x, r.y, r.width, FIELD_H),
            Rect::new(l.x, l.y + FIELD_H, l.width, FIELD_H),
            Rect::new(r.x, r.y + FIELD_H, r.width, FIELD_H),
            Rect::new(inner.x, inner.y + FIELD_H * 2, inner.width, FIELD_H),
        ];
        for (f, slot) in self.fields.iter_mut().zip(slots) {
            if slot.bottom() > inner.bottom() {
                continue;
            }
            f.state.set_focused(ctx.interaction.focused(f.id));
            f.state
                .set_hovered(ctx.interaction.hovered(f.id) && !f.state.is_editing());
            let err = f.current_error();
            let validation = match err {
                Some(msg) => Validation::Invalid(msg),
                None => Validation::Valid,
            };
            let input = TextInput::new(f.label, ctx.system)
                .required(f.required)
                .placeholder(f.placeholder)
                .help(f.help)
                .optional(!f.required)
                .validation(validation);
            let parts = input.paint(slot, buf, &mut f.state);
            ctx.control(f.id, slot, f.disabled);
            if f.state.is_editing()
                && let Some(cur) = parts.cursor
            {
                ctx.set_cursor(Position::new(cur.x, cur.y));
            }
            let _ = bg;
        }

        let (inner, _) = layout::card(
            rows[2],
            buf,
            t,
            Some("State reference"),
            Some("static"),
            false,
        );
        let w = inner.width.saturating_sub(18).min(34);
        let states: [(&str, &str, VisualState); 8] = [
            ("default", "payments-gateway", VisualState::default()),
            ("placeholder", "(feat/…)", VisualState::default()),
            (
                "hover",
                "payments-gateway",
                VisualState {
                    hovered: true,
                    ..Default::default()
                },
            ),
            (
                "focused",
                "payments-gateway",
                VisualState {
                    focused: true,
                    ..Default::default()
                },
            ),
            (
                "editing",
                "payments-gateway",
                VisualState {
                    focused: true,
                    editing: true,
                    ..Default::default()
                },
            ),
            (
                "error",
                "mira@example",
                VisualState {
                    error: true,
                    ..Default::default()
                },
            ),
            (
                "error + focus",
                "mira@example",
                VisualState {
                    error: true,
                    focused: true,
                    ..Default::default()
                },
            ),
            (
                "disabled",
                "jb_live_••••",
                VisualState {
                    disabled: true,
                    ..Default::default()
                },
            ),
        ];
        for (i, (name, value, s)) in states.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            static_field(buf, t, Rect::new(inner.x, y, w, 1), name, value, *s);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(i) = self.focused_index(cx.focus_id()) else {
                    return Route::Ignored;
                };
                if is_tab(key) {
                    if self.fields[i].state.is_editing() {
                        self.fields[i].state.commit();
                    }
                    // Shell owns the focus ring; Ignored lets Tab move on.
                    return Route::Ignored;
                }
                match self.fields[i].state.handle_key(*key) {
                    TextInputOutcome::Ignored => Route::Ignored,
                    TextInputOutcome::Submitted(_) => {
                        cx.status(format!("{} saved", self.fields[i].label));
                        Route::Changed
                    }
                    TextInputOutcome::Cancelled => {
                        cx.status("Reverted");
                        Route::Changed
                    }
                    _ => Route::Changed,
                }
            }
            PageEvent::Paste(text) => {
                let Some(i) = self.focused_index(cx.focus_id()) else {
                    return Route::Ignored;
                };
                if self.fields[i].state.is_editing() {
                    match self.fields[i].state.insert_str(text) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    }
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Click { id, pos } => {
                let Some(i) = self.fields.iter().position(|f| f.id == *id) else {
                    return Route::Ignored;
                };
                let was = cx.focus_id() == Some(*id);
                cx.set_focus(*id);
                self.fields[i].state.set_focused(true);
                if self.fields[i].disabled {
                    return Route::Changed;
                }
                if was {
                    self.fields[i].state.begin_edit();
                }
                if let Some(parts) = self.fields[i].state.parts().cloned() {
                    let _ = self.fields[i]
                        .state
                        .handle_mouse(mouse_down(*pos), parts.field);
                }
                Route::Changed
            }
            _ => Route::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.fields.iter().any(|f| f.state.is_editing())
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        let editing = focus
            .and_then(|f| self.fields.iter().find(|i| i.id == f))
            .map(|i| i.state.is_editing())
            .unwrap_or(false);
        if editing {
            vec![
                ("Enter", "Commit"),
                ("Esc", "Cancel"),
                ("Shift+← →", "Select"),
                ("Ctrl+U", "Clear"),
            ]
        } else {
            vec![("Enter", "Edit")]
        }
    }
}
