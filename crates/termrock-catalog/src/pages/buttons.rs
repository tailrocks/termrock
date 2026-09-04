// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/buttons.rs (MIT).
// Interactive buttons use termrock::widgets::Button. The state matrix is
// painted through JunieTheme::button (public resolver), matching the source
// reference grid.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::{ButtonKind, Role, VisualState};
use termrock::widgets::{Button, ButtonState, ButtonVariant};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("buttons");

struct DemoButton {
    id: WidgetId,
    label: &'static str,
    kind: ButtonKind,
    disabled: bool,
    on: Option<bool>,
    busy: bool,
    state: ButtonState,
}

impl DemoButton {
    fn primary(id: WidgetId, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: ButtonKind::Primary,
            disabled: false,
            on: None,
            busy: false,
            state: ButtonState::new(),
        }
    }
    fn secondary(id: WidgetId, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: ButtonKind::Secondary,
            disabled: false,
            on: None,
            busy: false,
            state: ButtonState::new(),
        }
    }
    fn subtle(id: WidgetId, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: ButtonKind::Subtle,
            disabled: false,
            on: None,
            busy: false,
            state: ButtonState::new(),
        }
    }
    fn danger(id: WidgetId, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: ButtonKind::Danger,
            disabled: false,
            on: None,
            busy: false,
            state: ButtonState::new(),
        }
    }
    fn toggle(id: WidgetId, label: &'static str, on: bool) -> Self {
        Self {
            id,
            label,
            kind: ButtonKind::Toggle,
            disabled: false,
            on: Some(on),
            busy: false,
            state: ButtonState::new(),
        }
    }
    fn disabled(mut self) -> Self {
        self.disabled = true;
        self.state.activation.set_enabled(false);
        self
    }

    fn variant(&self) -> ButtonVariant {
        match self.kind {
            ButtonKind::Primary => ButtonVariant::Primary,
            ButtonKind::Secondary | ButtonKind::Toggle => ButtonVariant::Secondary,
            ButtonKind::Subtle => ButtonVariant::Quiet,
            ButtonKind::Danger => ButtonVariant::Destructive,
        }
    }

    fn width(&self) -> u16 {
        let marker = if self.on.is_some() || self.busy { 2 } else { 0 };
        (crate::text::width(self.label) + 2 + marker) as u16
    }
}

pub struct ButtonsPage {
    buttons: Vec<DemoButton>,
    clicks: u32,
    last: Option<String>,
    busy_until: Option<u64>,
}

impl ButtonsPage {
    #[must_use]
    pub fn new() -> Self {
        let buttons = vec![
            DemoButton::primary(ID.child(0), "Run task"),
            DemoButton::secondary(ID.child(1), "Preview"),
            DemoButton::subtle(ID.child(2), "Cancel"),
            DemoButton::danger(ID.child(3), "Delete branch"),
            DemoButton::toggle(ID.child(4), "Auto-approve", false),
            DemoButton::toggle(ID.child(5), "Verbose", true),
            DemoButton::primary(ID.child(6), "Disabled primary").disabled(),
            DemoButton::secondary(ID.child(7), "Disabled").disabled(),
            DemoButton::secondary(ID.child(8), "Start long job"),
        ];
        Self {
            buttons,
            clicks: 0,
            last: None,
            busy_until: None,
        }
    }

    fn activated(&mut self, i: usize, cx: &mut PageCtx<'_>, now_ms: u64) {
        self.clicks += 1;
        let b = &self.buttons[i];
        let msg = match b.on {
            Some(true) => format!("{} on", b.label),
            Some(false) => format!("{} off", b.label),
            None => format!("{} ✓", b.label),
        };
        if i == 8 {
            self.buttons[8].busy = true;
            self.buttons[8].state.activation.set_loading(true);
            self.busy_until = Some(now_ms.saturating_add(2200));
            cx.status("Working…".to_owned());
        } else {
            cx.status(msg.clone());
        }
        self.last = Some(msg);
    }
}

impl Page for ButtonsPage {
    fn title(&self) -> &'static str {
        "Buttons"
    }
    fn blurb(&self) -> &'static str {
        "Primary, secondary, subtle, danger, toggle, disabled, busy"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[15, 1, 11]);
        let (inner, bg) = layout::card(
            rows[0],
            buf,
            t,
            Some("Playground"),
            Some("hover · click · Tab · Enter / Space"),
            false,
        );
        let mut y = inner.y;
        let groups: [(&str, &[usize]); 4] = [
            ("Actions", &[0, 1, 2, 3]),
            ("Toggles", &[4, 5]),
            ("Disabled", &[6, 7]),
            ("Busy", &[8]),
        ];
        for (label, idx) in groups {
            if y + 1 >= inner.bottom() {
                break;
            }
            layout::caption(inner.x, y, buf, t, label, bg);
            let mut x = inner.x;
            for &i in idx {
                let b = &mut self.buttons[i];
                let w = b.width().min(inner.right().saturating_sub(x));
                let r = Rect::new(x, y + 1, w, 1);
                b.state.focused = ctx.interaction.focused(b.id);
                b.state.hovered = ctx.interaction.hovered(b.id);
                b.state.activation.set_accepts_input(!b.disabled);
                b.state.activation.set_enabled(!b.disabled);
                b.state.activation.set_loading(b.busy);
                let leading = match b.on {
                    Some(true) => Some("●"),
                    Some(false) => Some("○"),
                    None => None,
                };
                let mut btn = Button::new(b.label, ctx.system)
                    .variant(b.variant())
                    .container(bg);
                if let Some(g) = leading {
                    btn = btn.leading(g).leading_role(if b.on == Some(true) {
                        Role::Accent
                    } else {
                        Role::TextMuted
                    });
                }
                let _ = btn.paint(r, buf, &mut b.state);
                ctx.control(b.id, r, b.disabled);
                x = x.saturating_add(w).saturating_add(2);
            }
            y += 3;
        }

        let (inner, bg) = layout::card(
            rows[2],
            buf,
            t,
            Some("State matrix"),
            Some("reference rendering"),
            false,
        );
        let states: [(&str, VisualState); 6] = [
            ("default", VisualState::default()),
            (
                "hover",
                VisualState {
                    hovered: true,
                    ..Default::default()
                },
            ),
            (
                "focus",
                VisualState {
                    focused: true,
                    ..Default::default()
                },
            ),
            (
                "focus + hover",
                VisualState {
                    focused: true,
                    hovered: true,
                    ..Default::default()
                },
            ),
            (
                "pressed",
                VisualState {
                    pressed: true,
                    focused: true,
                    ..Default::default()
                },
            ),
            (
                "disabled",
                VisualState {
                    disabled: true,
                    ..Default::default()
                },
            ),
        ];
        let kinds = [
            (ButtonKind::Primary, "Primary"),
            (ButtonKind::Secondary, "Secondary"),
            (ButtonKind::Subtle, "Subtle"),
            (ButtonKind::Danger, "Danger"),
        ];
        let col_w = 15u16;
        let label_w = 15u16;
        for (k, (_, name)) in kinds.iter().enumerate() {
            let x = inner.x + label_w + k as u16 * col_w;
            if x + col_w > inner.right() + 1 {
                break;
            }
            buf.set_string(x, inner.y, name, t.muted().bg(bg));
        }
        for (si, (sname, s)) in states.iter().enumerate() {
            let y = inner.y + 1 + si as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, sname, t.secondary().bg(bg));
            for (k, (kind, _)) in kinds.iter().enumerate() {
                let x = inner.x + label_w + k as u16 * col_w;
                if x + col_w > inner.right() + 1 {
                    break;
                }
                let style = t.button(*kind, *s, bg);
                let on_accent = *kind == ButtonKind::Primary && !s.disabled;
                let gutter = t.gutter(*s, style.bg.unwrap_or(bg), on_accent);
                buf.set_string(x, y, "▎", gutter);
                buf.set_string(x + 1, y, " Label ", style);
            }
        }
        if let Some(last) = &self.last {
            let body = format!("last: {last} · {} activations", self.clicks);
            let y = rows[2].bottom() + 1;
            if y < area.bottom() {
                buf.set_string(area.x, y, &body, t.faint());
            }
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Tick => {
                if let Some(until) = self.busy_until {
                    // elapsed is driven by the shell via status clock; pages
                    // treat Tick as "maybe done" after first animation ticks.
                    let _ = until;
                }
                Route::Ignored
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if !matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return Route::Ignored;
                }
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                let Some(i) = self.buttons.iter().position(|b| b.id == f) else {
                    return Route::Ignored;
                };
                if self.buttons[i].disabled || self.buttons[i].busy {
                    return Route::Consumed;
                }
                if let Some(on) = self.buttons[i].on.as_mut() {
                    *on = !*on;
                }
                self.activated(i, cx, 0);
                Route::Changed
            }
            PageEvent::Click { id, .. } => {
                let Some(i) = self.buttons.iter().position(|b| b.id == *id) else {
                    return Route::Ignored;
                };
                if self.buttons[i].disabled || self.buttons[i].busy {
                    return Route::Changed;
                }
                if let Some(on) = self.buttons[i].on.as_mut() {
                    *on = !*on;
                }
                self.activated(i, cx, 0);
                Route::Changed
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![("Enter / Space", "Activate")]
    }

    fn animating(&self) -> bool {
        self.busy_until.is_some()
    }
}
