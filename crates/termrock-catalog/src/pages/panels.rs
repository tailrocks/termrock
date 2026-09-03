// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/panels.rs (MIT).
// Prose and log copy from junie-tui src/bin/showcase/data.rs (MIT).

//! Cards group; a frame only where a pane needs an edge; nothing boxed twice.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::JunieTheme;
use termrock::widgets::{
    List, ListClickPolicy, ListRow, ListState, Outcome as ListOutcome, ScrollArea, ScrollAreaState,
    ScrollOutcome,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("panels");
const PROSE: WidgetId = ID.sub("prose");
const LOG: WidgetId = ID.sub("log");
const NESTED: WidgetId = ID.sub("nested");

const PROSE_TEXT: &str = "Junie works through a task the way a careful engineer would: it reads the \
relevant code, forms a plan, makes focused changes, runs the tests, and reports \
back with a summary you can review before anything is merged.\n\n\
Each step is visible. You can pause, redirect, or take over at any point, and \
every change lands as an ordinary diff in your working tree.\n\n\
The design system in this prototype exists so that the terminal version of that \
experience feels as deliberate as the web version: quiet surfaces, one accent, \
clear focus, and no decoration that does not carry information.\n\n\
Scroll with the mouse wheel, PageUp/PageDown, or the arrow keys while this panel \
has focus. The scrollbar on the right shows where you are and how much remains.";

fn log_lines(n: usize) -> Vec<String> {
    let steps = [
        ("info", "Resolving workspace members"),
        ("info", "Fetching crates.io index"),
        ("info", "Compiling proc-macro2 v1.0.86"),
        ("info", "Compiling serde v1.0.210"),
        ("warn", "unused import: `std::fmt` in src/api/mod.rs:3"),
        ("info", "Compiling tokio v1.40.0"),
        ("info", "Running unittests src/lib.rs"),
        ("info", "test api::auth::tests::rejects_expired ... ok"),
        ("info", "test db::pool::tests::reuses_connections ... ok"),
        ("error", "test checkout::places_order ... FAILED"),
        (
            "info",
            "test workers::scheduler::tests::respects_timezone ... ok",
        ),
        ("info", "Linking target/debug/deps/app-4f2c1b"),
    ];
    (0..n)
        .map(|i| {
            let (level, msg) = steps[i % steps.len()];
            let secs = i as f64 * 0.37;
            format!("{secs:7.2}s  {level:<5}  {msg}")
        })
        .collect()
}

fn log_style(t: &JunieTheme, line: &str) -> ratatui::style::Style {
    if line.contains(" error ") {
        t.error_fg()
    } else if line.contains(" warn ") {
        t.primary().fg(t.warning)
    } else {
        t.secondary()
    }
}

fn nested_rows() -> Vec<ListRow<'static, &'static str>> {
    vec![
        ListRow::item("local", Line::from("Local")),
        ListRow::item("cli", Line::from("CLI")),
        ListRow::item("cloud", Line::from("Cloud")).disabled(),
    ]
}

fn scroll_route(out: ScrollOutcome) -> Route {
    if out.consumed() {
        Route::Changed
    } else {
        Route::Ignored
    }
}

pub struct PanelsPage {
    prose: ScrollAreaState,
    log: ScrollAreaState,
    log_lines: Vec<String>,
    nested: ListState<&'static str>,
    prose_view: u16,
    log_view: u16,
}

impl PanelsPage {
    #[must_use]
    pub fn new() -> Self {
        let mut nested = ListState::new(None);
        nested.set_click_policy(ListClickPolicy::Select);
        Self {
            prose: ScrollAreaState::new().axes(true, false),
            log: ScrollAreaState::new().axes(true, false),
            log_lines: log_lines(60),
            nested,
            prose_view: 0,
            log_view: 0,
        }
    }

    fn handle_scroll_key(state: &mut ScrollAreaState, key: termrock::input::KeyEvent) -> Route {
        match key.code {
            KeyCode::Char('g') => scroll_route(state.home()),
            KeyCode::Char('G') => scroll_route(state.end()),
            KeyCode::Char('k') => scroll_route(state.scroll_by(-1, 0)),
            KeyCode::Char('j') => scroll_route(state.scroll_by(1, 0)),
            _ => scroll_route(state.handle_key(key)),
        }
    }
}

impl Page for PanelsPage {
    fn title(&self) -> &'static str {
        "Panels"
    }
    fn blurb(&self) -> &'static str {
        "Cards group; a frame only where a pane needs an edge; nothing boxed twice"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let (l, r) = layout::columns(area, area.width / 2 - 1, 2);
        let lrows = layout::rows(l, &[7, 1, 6, 1, 7, 0]);

        let (inner, bg) = layout::card(
            lrows[0],
            buf,
            t,
            Some("Titled card"),
            Some("surface"),
            false,
        );
        for (i, line) in text::wrap(
            "A card is a filled surface. Its title sits in the top-left and metadata on the right. It never has a border.",
            inner.width as usize,
        )
        .iter()
        .enumerate()
        {
            if i < inner.height as usize {
                buf.set_string(inner.x, inner.y + i as u16, line, t.secondary().bg(bg));
            }
        }

        let (inner, bg) = layout::card(lrows[2], buf, t, None, None, false);
        for (i, line) in text::wrap(
            "Untitled card. Same surface, content starts at the padding edge.",
            inner.width as usize,
        )
        .iter()
        .enumerate()
        {
            if i < inner.height as usize {
                buf.set_string(inner.x, inner.y + i as u16, line, t.secondary().bg(bg));
            }
        }

        let (inner, bg) = layout::card(lrows[4], buf, t, Some("Nested"), None, false);
        buf.set_string(inner.x, inner.y, "Target", t.muted().bg(bg));
        let group = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width.min(30),
            inner.height.saturating_sub(1).min(3),
        );
        let nested_rows = nested_rows();
        StatefulWidget::render(
            &List::new(&nested_rows, ctx.system).focused(ctx.interaction.focused(NESTED)),
            group,
            buf,
            &mut self.nested,
        );
        ctx.control(NESTED, group, false);
        let note_x = group.right() + 2;
        if note_x + 20 < inner.right() {
            for (i, line) in text::wrap(
                "A group inside a card is a muted label plus indent. The focus bar stays on the control.",
                (inner.right() - note_x) as usize,
            )
            .iter()
            .enumerate()
            {
                if (i as u16) < inner.height {
                    buf.set_string(note_x, inner.y + i as u16, line, t.muted().bg(bg));
                }
            }
        }

        let rrows = layout::rows(r, &[r.height / 2, 0]);
        let pf = ctx.interaction.focused(PROSE);
        let mut prose_lines: Vec<String> = Vec::new();
        // The source prose wraps against the framed body width, including the
        // one-cell terminal scrollbar gutter; this is one cell wider than the
        // clipped text paint width.
        let wrap_w = rrows[0].width.saturating_sub(7).max(8) as usize;
        for para in PROSE_TEXT.split('\n') {
            if para.is_empty() {
                prose_lines.push(String::new());
            } else {
                prose_lines.extend(text::wrap(para, wrap_w));
            }
        }
        let pos = crate::layout::overflow_label(
            usize::from(self.prose.offset_y()),
            usize::from(self.prose_view),
            prose_lines.len(),
        );
        self.prose
            .set_content_size(rrows[0].width, prose_lines.len() as u16);
        self.prose
            .set_viewport(rrows[0].width, rrows[0].height.saturating_sub(2));
        self.prose_view = rrows[0].height.saturating_sub(2);
        let (inner, bg) = layout::framed(rrows[0], buf, t, Some("Framed · split pane"), pf);
        if !pos.is_empty() {
            let mw = text::width(&pos) as u16;
            if rrows[0].width > mw + 4 {
                buf.set_string(
                    rrows[0].right().saturating_sub(mw + 2),
                    rrows[0].y,
                    &pos,
                    t.faint().bg(t.canvas),
                );
            }
        }
        self.prose.set_viewport(inner.width, inner.height);
        let bars = ScrollArea::new(ctx.system).focused(pf);
        let body = bars.body_area(inner, &self.prose);
        let text_w = inner.width.saturating_sub(2);
        let start = usize::from(self.prose.offset_y());
        for (i, line) in prose_lines.iter().skip(start).enumerate() {
            let y = body.y + i as u16;
            if y >= body.bottom() {
                break;
            }
            buf.set_string(
                body.x,
                y,
                &text::truncate(line, text_w as usize),
                t.secondary().bg(bg),
            );
        }
        termrock::scroll::paint_overflow_scrollbar(
            buf,
            Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height),
            prose_lines.len(),
            usize::from(inner.height).max(1),
            self.prose.offset_y(),
            pf,
            ctx.system,
        );
        ctx.control(PROSE, inner, false);
        ctx.scrollable(PROSE, inner);

        let lf = ctx.interaction.focused(LOG);
        let log_area = Rect::new(
            rrows[1].x,
            rrows[1].y + 1,
            rrows[1].width,
            rrows[1].height.saturating_sub(1),
        );
        let pos = crate::layout::overflow_label(
            usize::from(self.log.offset_y()),
            usize::from(self.log_view),
            self.log_lines.len(),
        );
        self.log
            .set_content_size(log_area.width, self.log_lines.len() as u16);
        self.log
            .set_viewport(log_area.width, log_area.height.saturating_sub(2));
        self.log_view = log_area.height.saturating_sub(2);
        let follow = if self.log.is_following() {
            "following"
        } else {
            ""
        };
        let meta = if follow.is_empty() {
            pos
        } else if pos.is_empty() {
            follow.to_owned()
        } else {
            format!("{pos} · {follow}")
        };
        let (inner, bg) =
            layout::card(log_area, buf, t, Some("Card · scrollable"), Some(&meta), lf);
        self.log.set_viewport(inner.width, inner.height);
        let bars = ScrollArea::new(ctx.system).focused(lf);
        let body = bars.body_area(inner, &self.log);
        let text_w = inner.width.saturating_sub(2);
        let start = usize::from(self.log.offset_y());
        for (i, line) in self.log_lines.iter().skip(start).enumerate() {
            let y = body.y + i as u16;
            if y >= body.bottom() {
                break;
            }
            buf.set_string(
                body.x,
                y,
                &text::truncate(line, text_w as usize),
                log_style(t, line).bg(bg),
            );
        }
        termrock::scroll::paint_overflow_scrollbar(
            buf,
            Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height),
            self.log_lines.len(),
            usize::from(inner.height).max(1),
            self.log.offset_y(),
            lf,
            ctx.system,
        );
        ctx.control(LOG, inner, false);
        ctx.scrollable(LOG, inner);
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == NESTED {
                    let rows = nested_rows();
                    return match self.nested.handle_key(&rows, *key) {
                        ListOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == PROSE {
                    return Self::handle_scroll_key(&mut self.prose, *key);
                }
                if f == LOG {
                    if matches!(key.code, KeyCode::Char('f')) {
                        if self.log.is_following() {
                            self.log.pause_follow();
                        } else {
                            self.log.follow_tail();
                        }
                        return Route::Changed;
                    }
                    return Self::handle_scroll_key(&mut self.log, *key);
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == NESTED {
                    cx.set_focus(NESTED);
                    let _ = self.nested.click(*pos);
                    return Route::Changed;
                }
                if *id == PROSE {
                    cx.set_focus(PROSE);
                    return Route::Changed;
                }
                if *id == LOG {
                    cx.set_focus(LOG);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } => {
                let dy = *delta as isize;
                if *id == NESTED {
                    let rows = nested_rows();
                    return match self.nested.handle_key(
                        &rows,
                        termrock::input::KeyEvent::new(
                            if dy < 0 { KeyCode::Up } else { KeyCode::Down },
                            termrock::input::KeyModifiers::NONE,
                        ),
                    ) {
                        ListOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if *id == PROSE {
                    return scroll_route(self.prose.scroll_by(dy, 0));
                }
                if *id == LOG {
                    return scroll_route(self.log.scroll_by(dy, 0));
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(NESTED) {
            vec![("↑ ↓", "Move"), ("Enter", "Choose")]
        } else if focus == Some(LOG) {
            vec![("↑ ↓", "Scroll"), ("f", "Follow tail"), ("g G", "Ends")]
        } else {
            vec![("↑ ↓", "Scroll"), ("PgUp PgDn", "Page"), ("g G", "Ends")]
        }
    }
}
