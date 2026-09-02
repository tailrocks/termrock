// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/progress.rs (MIT).

//! Determinate, indeterminate, compact activity, terminal states.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::runtime::FrameTick;
use termrock::style::MotionPolicy;
use termrock::widgets::{
    Button, ButtonState, ButtonVariant, ProgressBar, ProgressKind, ProgressStatus, Spinner,
    SpinnerState,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("progress");

pub struct ProgressPage {
    build: f64,
    running: bool,
    restart: ButtonState,
    pause: ButtonState,
    paused: bool,
}

impl ProgressPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            build: 0.0,
            running: true,
            restart: ButtonState::new(),
            pause: ButtonState::new(),
            paused: false,
        }
    }
}

fn tick(ctx: &RenderCtx<'_>) -> FrameTick {
    FrameTick::manual(
        termrock::runtime::Instant::now(),
        std::time::Duration::from_millis(ctx.interaction.tick.saturating_mul(80)),
        std::time::Duration::from_millis(80),
    )
}

fn btn_width(label: &str) -> u16 {
    (text::width(label) + 2) as u16
}

fn paint_btn(
    label: &str,
    variant: ButtonVariant,
    id: WidgetId,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    state: &mut ButtonState,
    bg: ratatui::style::Color,
) {
    state.focused = ctx.interaction.focused(id);
    state.hovered = ctx.interaction.hovered(id);
    state.activation.set_accepts_input(true);
    state.activation.set_enabled(true);
    let _ = Button::new(label, ctx.system)
        .variant(variant)
        .container(bg)
        .paint(area, buf, state);
    ctx.control(id, area, false);
}

impl Page for ProgressPage {
    fn title(&self) -> &'static str {
        "Progress"
    }
    fn blurb(&self) -> &'static str {
        "Determinate, indeterminate, compact activity, terminal states"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[12, 1, 0]);
        let (inner, bg) =
            layout::card(rows[0], buf, t, Some("Live"), Some("ticks at 80 ms"), false);
        let w = inner.width.min(70);
        let status = if self.build >= 1.0 {
            ProgressStatus::Complete
        } else if self.paused {
            ProgressStatus::Paused
        } else {
            ProgressStatus::Running
        };
        let frame = tick(ctx);
        ProgressBar::new(
            ProgressKind::Determinate {
                fraction: self.build,
            },
            ctx.system,
        )
        .label("Building  ")
        .status(status)
        .paint(Rect::new(inner.x, inner.y, w, 1), buf);
        ProgressBar::new(
            ProgressKind::Indeterminate {
                tick: ctx.interaction.tick,
            },
            ctx.system,
        )
        .label("Resolving ")
        .paint(Rect::new(inner.x, inner.y + 2, w, 1), buf);
        let mut spin = SpinnerState::new();
        spin.set_phase(termrock::widgets::ActivityPhase::Waiting);
        Spinner::labeled("Waiting for the test runner", ctx.system).paint(
            Rect::new(inner.x, inner.y + 4, w, 1),
            buf,
            &spin,
            frame,
            MotionPolicy::Full,
        );
        let glyph = spin.frame_glyph(frame, MotionPolicy::Full);
        buf.set_string(
            inner.x,
            inner.y + 5,
            format!("{glyph} 3 of 12 files"),
            t.secondary().bg(bg),
        );
        buf.set_string(inner.x, inner.y + 5, glyph, t.accent_fg().bg(bg));

        let restart_l = "Restart";
        let pause_l = if self.paused { "Resume" } else { "Pause" };
        let rw = btn_width(restart_l).min(inner.width);
        let pw = btn_width(pause_l).min(inner.width.saturating_sub(rw.saturating_add(2)));
        paint_btn(
            restart_l,
            ButtonVariant::Secondary,
            ID.sub("restart"),
            Rect::new(inner.x, inner.y + 7, rw, 1),
            buf,
            ctx,
            &mut self.restart,
            bg,
        );
        paint_btn(
            pause_l,
            ButtonVariant::Secondary,
            ID.sub("pause"),
            Rect::new(
                inner.x.saturating_add(rw).saturating_add(2),
                inner.y + 7,
                pw,
                1,
            ),
            buf,
            ctx,
            &mut self.pause,
            bg,
        );

        let (inner, _bg) = layout::card(
            Rect::new(rows[2].x, rows[2].y, rows[2].width, rows[2].height.min(12)),
            buf,
            t,
            Some("States"),
            Some("static"),
            false,
        );
        let w = inner.width.min(70);
        let samples = [
            ("Queued    ", 0.0, ProgressStatus::Running),
            ("Halfway   ", 0.5, ProgressStatus::Running),
            ("Completed ", 1.0, ProgressStatus::Complete),
            ("Failed    ", 0.64, ProgressStatus::Failed),
            ("Paused    ", 0.3, ProgressStatus::Paused),
        ];
        for (i, (label, r, s)) in samples.iter().enumerate() {
            let y = inner.y + i as u16;
            if y < inner.bottom() {
                ProgressBar::new(ProgressKind::Determinate { fraction: *r }, ctx.system)
                    .label(label)
                    .status(*s)
                    .paint(Rect::new(inner.x, y, w, 1), buf);
            }
        }
        if inner.height > 6 {
            buf.set_string(
                inner.x,
                inner.y + 6,
                "Narrow bars keep the percentage and drop the label:",
                t.muted().bg(bg),
            );
            ProgressBar::new(ProgressKind::Determinate { fraction: 0.42 }, ctx.system)
                .label("")
                .status(ProgressStatus::Running)
                .paint(Rect::new(inner.x, inner.y + 7, 14, 1), buf);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Tick => {
                if self.running && !self.paused && self.build < 1.0 {
                    self.build = (self.build + 0.006).min(1.0);
                    if self.build >= 1.0 {
                        cx.status("Build finished ✓");
                    }
                }
                Route::Changed
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if !matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return Route::Ignored;
                }
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == ID.sub("restart") {
                    self.build = 0.0;
                    self.running = true;
                    return Route::Changed;
                }
                if f == ID.sub("pause") {
                    self.paused = !self.paused;
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Click { id, .. } => {
                if *id == ID.sub("restart") {
                    self.build = 0.0;
                    self.running = true;
                    return Route::Changed;
                }
                if *id == ID.sub("pause") {
                    self.paused = !self.paused;
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn animating(&self) -> bool {
        true
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![("Enter", "Activate")]
    }
}
