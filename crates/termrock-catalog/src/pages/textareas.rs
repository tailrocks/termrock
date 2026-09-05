// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/textareas.rs (MIT).

//! Multi-line editing, wrapping cursor motion, scroll position.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::widgets::{TextArea, TextAreaOutcome, TextAreaState, TextWrap};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("textareas");

struct Area {
    id: WidgetId,
    title: &'static str,
    placeholder: &'static str,
    help: &'static str,
    error: Option<&'static str>,
    rows: u16,
    disabled: bool,
    state: TextAreaState,
}

impl Area {
    fn new(id: WidgetId, title: &'static str, rows: u16, text: &str) -> Self {
        let mut state = TextAreaState::new(text);
        state.set_editing(false);
        Self {
            id,
            title,
            placeholder: "",
            help: "",
            error: None,
            rows,
            disabled: false,
            state,
        }
    }
}

fn is_tab(key: &termrock::input::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
}

fn mouse_at(kind: MouseEventKind, pos: Position) -> MouseEvent {
    MouseEvent {
        kind,
        position: pos,
        modifiers: KeyModifiers::NONE,
    }
}

fn route_area(out: TextAreaOutcome) -> Route {
    match out {
        TextAreaOutcome::Ignored => Route::Ignored,
        _ => Route::Changed,
    }
}

pub struct TextAreasPage {
    areas: Vec<Area>,
}

impl TextAreasPage {
    #[must_use]
    pub fn new() -> Self {
        let long = (1..=28)
            .map(|i| match i % 4 {
                0 => format!("{i:>2}. Run the integration suite and attach the report."),
                1 => format!("{i:>2}. Read src/api/billing.rs before touching invoices."),
                2 => format!("{i:>2}. Keep the public API stable; add, never rename."),
                _ => format!("{i:>2}. Open a PR against main with a clear summary."),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let areas = vec![
            {
                let mut a = Area::new(ID.child(0), "Task description", 8, &long);
                a.help = "Enter inserts a newline · Esc finishes";
                a
            },
            {
                let mut a = Area::new(ID.child(1), "Notes", 8, "");
                a.placeholder = "Anything the agent should know…";
                a.help = "Optional";
                a
            },
            {
                let mut a = Area::new(
                    ID.child(2),
                    "Read-only transcript",
                    4,
                    "Junie: Reading 14 files…\nJunie: Plan ready. 3 steps.",
                );
                a.disabled = true;
                a.state.set_read_only(true);
                a
            },
            {
                let mut a = Area::new(ID.child(3), "Commit message", 4, "fix stuff");
                a.error = Some("Use the imperative mood and explain why");
                a
            },
        ];
        Self { areas }
    }

    fn index_of(&self, id: WidgetId) -> Option<usize> {
        self.areas
            .iter()
            .position(|a| a.id == id || a.id.sub("scrollbar") == id)
    }
}

impl Page for TextAreasPage {
    fn title(&self) -> &'static str {
        "Text areas"
    }
    fn blurb(&self) -> &'static str {
        "Multi-line editing, wrapping cursor motion, scroll position"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[13, 1, 10]);
        let (inner, _bg) = layout::card(
            rows[0],
            buf,
            t,
            Some("Playground"),
            Some("Enter Edit · Esc Done · Tab Next"),
            false,
        );
        let (l, r) = layout::columns(inner, inner.width / 2 - 2, 4);
        for (a, slot) in self.areas.iter_mut().zip([l, r]) {
            paint_area(a, slot, buf, ctx);
        }

        let (inner, _bg) = layout::card(rows[2], buf, t, Some("Disabled and error"), None, false);
        let (l, r) = layout::columns(inner, inner.width / 2 - 2, 4);
        for (a, slot) in self.areas[2..].iter_mut().zip([l, r]) {
            paint_area(a, slot, buf, ctx);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(i) = cx.focus_id().and_then(|f| self.index_of(f)) else {
                    return Route::Ignored;
                };
                if is_tab(key) {
                    if self.areas[i].state.is_editing() {
                        self.areas[i].state.set_editing(false);
                    }
                    return Route::Ignored;
                }
                self.areas[i].state.set_accepts_input(true);
                let out = self.areas[i].state.handle_key(*key);
                if matches!(out, TextAreaOutcome::Changed) && !self.areas[i].state.is_editing() {
                    // Esc finishes a document (source InputEvent::Committed).
                    if matches!(key.code, KeyCode::Esc) {
                        cx.status("Saved");
                    }
                }
                route_area(out)
            }
            PageEvent::Paste(text) => {
                let Some(i) = cx.focus_id().and_then(|f| self.index_of(f)) else {
                    return Route::Ignored;
                };
                if self.areas[i].state.is_editing() {
                    route_area(self.areas[i].state.insert_text(text))
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Click { id, pos } => {
                let Some(i) = self.index_of(*id) else {
                    return Route::Ignored;
                };
                if self.areas[i].disabled {
                    cx.set_focus(self.areas[i].id);
                    return Route::Changed;
                }
                let was = cx.focus_id() == Some(self.areas[i].id);
                cx.set_focus(self.areas[i].id);
                self.areas[i].state.set_accepts_input(true);
                if was && !self.areas[i].state.is_editing() {
                    self.areas[i].state.set_editing(true);
                }
                route_area(self.areas[i].state.handle_event(Event::Mouse(mouse_at(
                    MouseEventKind::Down(MouseButton::Left),
                    *pos,
                ))))
            }
            PageEvent::Drag { pressed, pos } => {
                let Some(i) = self.index_of(*pressed) else {
                    return Route::Ignored;
                };
                route_area(self.areas[i].state.handle_event(Event::Mouse(mouse_at(
                    MouseEventKind::Drag(MouseButton::Left),
                    *pos,
                ))))
            }
            PageEvent::Wheel { id, delta } => match self.index_of(*id) {
                Some(i) => {
                    if self.areas[i].state.scroll_by(0, *delta as isize) {
                        Route::Changed
                    } else {
                        Route::Ignored
                    }
                }
                None => Route::Ignored,
            },
            _ => Route::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.areas.iter().any(|a| a.state.is_editing())
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        let editing = focus
            .and_then(|f| self.index_of(f))
            .map(|i| self.areas[i].state.is_editing())
            .unwrap_or(false);
        if editing {
            vec![
                ("Enter", "Newline"),
                ("Esc", "Done"),
                ("Shift+↑↓", "Select"),
                ("Tab", "Next"),
            ]
        } else {
            vec![("Enter", "Edit"), ("↑ ↓", "Scroll")]
        }
    }
}

fn paint_area(a: &mut Area, slot: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let focused = ctx.interaction.focused(a.id);
    a.state.set_accepts_input(focused && !a.disabled);
    a.state.set_read_only(a.disabled);
    let mut widget = TextArea::new(ctx.system)
        .title(a.title)
        .rows(a.rows)
        .wrap(TextWrap::None);
    if !a.placeholder.is_empty() {
        widget = widget.placeholder(a.placeholder);
    }
    if !a.help.is_empty() {
        widget = widget.help(a.help);
    }
    if let Some(err) = a.error {
        widget = widget.error(err);
    }
    widget.render(slot, buf, &mut a.state);
    ctx.control(a.id, slot, a.disabled);
    ctx.scrollable(a.id, slot);
    if a.state.is_editing()
        && let Some(cur) = a.state.cursor_cell()
    {
        ctx.set_cursor(cur);
    }
}
