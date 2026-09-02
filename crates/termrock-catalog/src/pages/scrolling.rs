// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/scrolling.rs (MIT).

//! Wheel under the pointer, keys on the focused container, thumb shows where you are.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind, MouseEvent, MouseEventKind};
use termrock::style::JunieTheme;
use termrock::widgets::{
    List, ListClickPolicy, ListRow, ListState, ScrollArea, ScrollAreaState, ScrollOutcome,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("scrolling");

pub struct ScrollingPage {
    prose: ScrollAreaState,
    prose_lines: Vec<String>,
    prose_area: Rect,
    list: ListState<usize>,
    list_labels: Vec<String>,
    list_flagged: Vec<bool>,
    list_area: Rect,
    log: ScrollAreaState,
    log_lines: Vec<String>,
    log_area: Rect,
    prose_view: u16,
    list_view: usize,
    log_view: u16,
}

fn prose_source() -> &'static str {
    "Junie works through a task the way a careful engineer would: it reads the \
relevant code, forms a plan, makes focused changes, runs the tests, and reports \
back with a summary you can review before anything is merged.\n\n\
Each step is visible. You can pause, redirect, or take over at any point, and \
every change lands as an ordinary diff in your working tree.\n\n\
The design system in this prototype exists so that the terminal version of that \
experience feels as deliberate as the web version: quiet surfaces, one accent, \
clear focus, and no decoration that does not carry information.\n\n\
Scroll with the mouse wheel, PageUp/PageDown, or the arrow keys while this panel \
has focus. The scrollbar on the right shows where you are and how much remains."
}

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

fn log_style(t: &JunieTheme, line: &str) -> Style {
    if line.contains(" error ") {
        t.error_fg()
    } else if line.contains(" warn ") {
        t.primary().fg(t.warning)
    } else {
        t.secondary()
    }
}

fn track_click(scroll: &mut ScrollAreaState, area: Rect, pos: Position) -> ScrollOutcome {
    let bar = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
    if !bar.contains(pos) {
        return ScrollOutcome::Ignored;
    }
    let next = termrock::scroll::offset_for_track_position_u16(
        usize::from(scroll.content_h()),
        usize::from(scroll.viewport_h()),
        usize::from(bar.height),
        usize::from(pos.y.saturating_sub(bar.y)),
    );
    let before = scroll.offset_y();
    scroll.set_offset_y(next);
    if scroll.offset_y() != before {
        ScrollOutcome::Scrolled
    } else {
        ScrollOutcome::Ignored
    }
}

fn route_scroll(out: ScrollOutcome) -> Route {
    if out.consumed() {
        Route::Changed
    } else {
        Route::Ignored
    }
}

impl ScrollingPage {
    #[must_use]
    pub fn new() -> Self {
        let mut text: Vec<String> = Vec::new();
        for _ in 0..3 {
            text.extend(prose_source().split('\n').map(str::to_owned));
            text.push(String::new());
        }
        let mut list_labels = Vec::new();
        let mut list_flagged = Vec::new();
        for i in 1..=120 {
            list_labels.push(format!("Row {i:03}"));
            list_flagged.push(i % 7 == 0);
        }
        let mut log = ScrollAreaState::new().axes(true, false).wheel_steps(3, 4);
        log.follow_tail();
        Self {
            prose: ScrollAreaState::new().axes(true, false).wheel_steps(3, 4),
            prose_lines: text,
            list: {
                let mut s = ListState::new(None);
                s.set_click_policy(ListClickPolicy::Select);
                s
            },
            list_labels,
            list_flagged,
            list_area: Rect::default(),
            log,
            log_lines: log_lines(400),
            log_area: Rect::default(),
            prose_area: Rect::default(),
            prose_view: 0,
            list_view: 0,
            log_view: 0,
        }
    }
}

fn paint_scroll_body(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    id: WidgetId,
    scroll: &mut ScrollAreaState,
    focused: bool,
    hovered: bool,
    content_h: u16,
    mut paint_line: impl FnMut(u16, u16, &mut Buffer),
) {
    let sa = ScrollArea::new(ctx.system)
        .focused(focused)
        .hovered(hovered);
    scroll.set_content_size(area.width.saturating_sub(1).max(1), content_h);
    scroll.set_viewport(area.width.saturating_sub(1).max(1), area.height);
    let body = sa.body_area(area, scroll);
    let start = usize::from(scroll.offset_y());
    for i in 0..body.height {
        let idx = start.saturating_add(usize::from(i));
        if idx >= usize::from(content_h) {
            break;
        }
        paint_line(idx as u16, body.y + i, buf);
    }
    termrock::scroll::paint_overflow_scrollbar(
        buf,
        Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
        usize::from(content_h),
        usize::from(area.height).max(1),
        scroll.offset_y(),
        focused,
        ctx.system,
    );
    ctx.control(id, area, false);
    ctx.scrollable(id, area);
    if sa.body_area(area, scroll).width < area.width {
        ctx.clickable(
            id.sub("scrollbar"),
            Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
        );
    }
}

impl Page for ScrollingPage {
    fn title(&self) -> &'static str {
        "Scrolling"
    }
    fn blurb(&self) -> &'static str {
        "Wheel under the pointer, keys on the focused container, thumb shows where you are"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let third = area.width / 3;
        let cols = [
            Rect::new(area.x, area.y, third.saturating_sub(1), area.height),
            Rect::new(
                area.x + third + 1,
                area.y,
                third.saturating_sub(1),
                area.height,
            ),
            Rect::new(
                area.x + 2 * third + 2,
                area.y,
                area.width.saturating_sub(2 * third + 2),
                area.height,
            ),
        ];

        let wrap_w = cols[0].width.saturating_sub(6).max(8) as usize;
        let mut wrapped: Vec<String> = Vec::new();
        for line in &self.prose_lines {
            if line.is_empty() {
                wrapped.push(String::new());
            } else {
                wrapped.extend(text::wrap(line, wrap_w));
            }
        }
        let pos = crate::layout::overflow_label(
            usize::from(self.prose.offset_y()),
            usize::from(self.prose_view),
            wrapped.len(),
        );
        self.prose
            .set_content_size(1, u16::try_from(wrapped.len()).unwrap_or(u16::MAX));
        self.prose.set_viewport(1, cols[0].height.saturating_sub(3));
        self.prose_view = cols[0].height.saturating_sub(3);
        let focused = ctx.interaction.focused(ID.sub("prose"));
        let (inner, bg) = layout_card(cols[0], buf, t, Some("Wrapped text"), Some(&pos), focused);
        let hovered = ctx.interaction.hovered(ID.sub("prose"));
        let lines = wrapped;
        self.prose_area = inner;
        paint_scroll_body(
            inner,
            buf,
            ctx,
            ID.sub("prose"),
            &mut self.prose,
            focused,
            hovered,
            u16::try_from(lines.len()).unwrap_or(u16::MAX),
            |idx, y, buf| {
                if let Some(line) = lines.get(usize::from(idx)) {
                    buf.set_string(
                        inner.x,
                        y,
                        text::truncate(line, inner.width.saturating_sub(2) as usize),
                        t.secondary().bg(bg),
                    );
                }
            },
        );

        let pos = crate::layout::overflow_label(
            self.list.offset(),
            self.list_view,
            self.list_labels.len(),
        );
        let focused = ctx.interaction.focused(ID.sub("list"));
        let (inner, _bg) = layout_card(cols[1], buf, t, Some("Long list"), Some(&pos), focused);
        self.list_area = inner;
        let rows: Vec<ListRow<'_, usize>> = self
            .list_labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let mut row = ListRow::item(i, Line::from(label.as_str()));
                if self.list_flagged[i] {
                    row = row.status(Line::from("flagged"));
                }
                row
            })
            .collect();
        StatefulWidget::render(
            &List::new(&rows, ctx.system).focused(focused),
            inner,
            buf,
            &mut self.list,
        );
        ctx.control(ID.sub("list"), inner, false);
        ctx.scrollable(ID.sub("list"), inner);
        self.list_view = usize::from(inner.height);

        let pos = crate::layout::overflow_label(
            usize::from(self.log.offset_y()),
            usize::from(self.log_view),
            self.log_lines.len(),
        );
        let meta = if self.log.is_following() {
            format!("{pos} · following")
        } else {
            pos
        };
        let lf = ctx.interaction.focused(ID.sub("log"));
        let (inner, bg) = layout_card(cols[2], buf, t, Some("Log"), Some(&meta), lf);
        self.log_area = inner;
        let hovered = ctx.interaction.hovered(ID.sub("log"));
        let log_lines = &self.log_lines;
        paint_scroll_body(
            inner,
            buf,
            ctx,
            ID.sub("log"),
            &mut self.log,
            lf,
            hovered,
            u16::try_from(log_lines.len()).unwrap_or(u16::MAX),
            |idx, y, buf| {
                if let Some(line) = log_lines.get(usize::from(idx)) {
                    buf.set_string(
                        inner.x,
                        y,
                        text::truncate(line, inner.width.saturating_sub(2) as usize),
                        log_style(t, line).bg(bg),
                    );
                }
            },
        );
        self.log_view = inner.height;
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Tick => {
                if self.log_lines.len() < 2000 && self.log.is_following() {
                    let n = self.log_lines.len();
                    if let Some(line) = log_lines(n + 1).pop() {
                        self.log_lines.push(line);
                    }
                    self.log.set_content_size(
                        1,
                        u16::try_from(self.log_lines.len()).unwrap_or(u16::MAX),
                    );
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == ID.sub("prose") {
                    return handle_scroll_keys(&mut self.prose, *key);
                }
                if f == ID.sub("log") {
                    if matches!(key.code, KeyCode::Char('f')) && key.modifiers.is_empty() {
                        if self.log.is_following() {
                            self.log.pause_follow();
                        } else {
                            self.log.follow_tail();
                        }
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Char('G') | KeyCode::End) {
                        let _ = self.log.end();
                        self.log.follow_tail();
                        return Route::Changed;
                    }
                    return handle_scroll_keys(&mut self.log, *key);
                }
                if f == ID.sub("list") {
                    let rows: Vec<ListRow<'_, usize>> = self
                        .list_labels
                        .iter()
                        .enumerate()
                        .map(|(i, label)| ListRow::item(i, Line::from(label.as_str())))
                        .collect();
                    match self.list.handle_key(&rows, *key) {
                        termrock::interaction::Outcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    }
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Click { id, pos } => {
                if *id == ID.sub("list") {
                    cx.set_focus(ID.sub("list"));
                    return match self.list.click(*pos) {
                        termrock::interaction::Outcome::Ignored => Route::Changed,
                        _ => Route::Changed,
                    };
                }
                if *id == ID.sub("prose").sub("scrollbar") {
                    return route_scroll(track_click(&mut self.prose, self.prose_area, *pos));
                }
                if *id == ID.sub("log").sub("scrollbar") {
                    return route_scroll(track_click(&mut self.log, self.log_area, *pos));
                }
                if *id == ID.sub("prose") {
                    cx.set_focus(ID.sub("prose"));
                    return Route::Changed;
                }
                if *id == ID.sub("log") {
                    cx.set_focus(ID.sub("log"));
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Drag { pressed, pos } => {
                if *pressed == ID.sub("prose").sub("scrollbar") {
                    return route_scroll(track_click(&mut self.prose, self.prose_area, *pos));
                }
                if *pressed == ID.sub("log").sub("scrollbar") {
                    return route_scroll(track_click(&mut self.log, self.log_area, *pos));
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } => {
                if *id == ID.sub("list") {
                    let _ = self.list.scroll_by(*delta as isize, self.list_labels.len());
                    return Route::Changed;
                }
                if *id == ID.sub("prose") || *id == ID.sub("prose").sub("scrollbar") {
                    return route_scroll(self.prose.handle_mouse(MouseEvent {
                        kind: if *delta < 0 {
                            MouseEventKind::ScrollUp
                        } else {
                            MouseEventKind::ScrollDown
                        },
                        position: Position::new(0, 0),
                        modifiers: termrock::input::KeyModifiers::NONE,
                    }));
                }
                if *id == ID.sub("log") || *id == ID.sub("log").sub("scrollbar") {
                    return route_scroll(self.log.scroll_by(*delta as isize, 0));
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.log.is_following() && self.log_lines.len() < 2000
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(ID.sub("log")) {
            vec![("↑ ↓", "Scroll"), ("f", "Follow"), ("G", "End")]
        } else if focus == Some(ID.sub("list")) {
            vec![("↑ ↓", "Move"), ("PgUp PgDn", "Page"), ("g G", "Ends")]
        } else {
            vec![("↑ ↓", "Scroll"), ("PgUp PgDn", "Page"), ("g G", "Ends")]
        }
    }
}

fn handle_scroll_keys(scroll: &mut ScrollAreaState, key: termrock::input::KeyEvent) -> Route {
    match key.code {
        KeyCode::Char('g') if key.modifiers.is_empty() => route_scroll(scroll.home()),
        KeyCode::Char('G') => route_scroll(scroll.end()),
        _ => route_scroll(scroll.handle_key(key)),
    }
}

fn layout_card(
    area: Rect,
    buf: &mut Buffer,
    t: &JunieTheme,
    title: Option<&str>,
    meta: Option<&str>,
    focused: bool,
) -> (Rect, ratatui::style::Color) {
    crate::layout::card(area, buf, t, title, meta, focused)
}
