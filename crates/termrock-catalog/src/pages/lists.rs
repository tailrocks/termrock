// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/lists.rs (MIT).
// Demo data copied from junie-tui src/bin/showcase/data.rs (MIT).

//! Single and multiple selection, disabled items, scrolling, empty state.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::KeyEventKind;
use termrock::style::Role;
use termrock::widgets::{
    List, ListClickPolicy, ListRow, ListSelectionMode, ListState, Outcome as ListOutcome,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("lists");

const LANGUAGES: &[&str] = &[
    "Rust",
    "TypeScript",
    "Python",
    "Kotlin",
    "Go",
    "Java",
    "Swift",
    "C#",
    "Ruby",
    "Scala",
    "Elixir",
    "Haskell",
    "Zig",
    "Dart",
    "PHP",
    "C++",
    "Lua",
    "OCaml",
    "Clojure",
    "Erlang",
];

const FILES: &[(&str, &str, bool)] = &[
    ("src/api/auth.rs", "modified", false),
    ("src/api/billing.rs", "modified", false),
    ("src/db/schema.rs", "generated", true),
    ("tests/checkout.rs", "new", false),
    ("Cargo.lock", "locked", true),
    ("docs/webhooks.md", "modified", false),
    ("src/workers/mailer.rs", "modified", false),
    ("src/config.rs", "modified", false),
    ("README.md", "modified", false),
    ("src/main.rs", "modified", false),
    ("tests/auth_flow.rs", "new", false),
    ("src/db/pool.rs", "modified", false),
];

fn list_route<T>(out: ListOutcome<T>) -> Route {
    match out {
        ListOutcome::Ignored | ListOutcome::Cancelled => Route::Ignored,
        _ => Route::Changed,
    }
}

/// Paint the source list contract: the card owns focus chrome, while the
/// active row keeps the card surface and carries focus on its marker only.
fn render_source_list(
    rows: &[ListRow<'_, usize>],
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    state: &mut ListState<usize>,
    focused: bool,
    accent_marker: Option<usize>,
) {
    // The source card's right padding cell is part of its row hit band. The
    // shared list reserves one scrollbar cell and uses half-open rectangles,
    // so map that one boundary cell back into the row before stateful hover
    // resolution.
    let pointer = ctx.interaction.pointer.map(|position| {
        if position.x == area.right() && position.y >= area.y && position.y < area.bottom() {
            Position::new(position.x.saturating_sub(2), position.y)
        } else {
            position
        }
    });
    if let Some(pointer) = pointer {
        state.hover(pointer);
    }
    List::new(rows, ctx.system)
        .focused(focused)
        .render(area, buf, state);
    if rows.len() > usize::from(area.height) {
        termrock::scroll::paint_overflow_scrollbar(
            buf,
            Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
            rows.len(),
            usize::from(area.height),
            u16::try_from(state.offset()).unwrap_or(u16::MAX),
            focused,
            ctx.system,
        );
    }

    let Some(active) = accent_marker else {
        return;
    };
    let accent = ctx.system.style(Role::Accent);
    for region in state.regions() {
        if region.id != active || region.area.x.saturating_add(1) >= region.area.right() {
            continue;
        }
        let cell = &mut buf[(region.area.x.saturating_add(1), region.area.y)];
        let mut style = accent;
        style.bg = cell.style().bg;
        cell.set_style(style);
    }
}

pub struct ListsPage {
    single: ListState<usize>,
    multi: ListState<usize>,
    empty: ListState<usize>,
    single_view: usize,
}

impl ListsPage {
    #[must_use]
    pub fn new() -> Self {
        let mut single = ListState::new(Some(0));
        single.set_click_policy(ListClickPolicy::Activate);
        let mut multi = ListState::new(Some(0));
        multi.set_selection_mode(ListSelectionMode::Range);
        multi.set_click_policy(ListClickPolicy::Activate);
        if let Some(sel) = multi.selection_mut() {
            sel.toggle(&0);
            sel.toggle(&1);
        }
        let mut empty = ListState::new(None);
        empty.set_click_policy(ListClickPolicy::Select);
        Self {
            single,
            multi,
            empty,
            single_view: 0,
        }
    }

    fn lists(&mut self) -> [(&mut ListState<usize>, WidgetId, usize); 3] {
        [
            (&mut self.single, ID.sub("single"), LANGUAGES.len()),
            (&mut self.multi, ID.sub("multi"), FILES.len()),
            (&mut self.empty, ID.sub("empty"), 0),
        ]
    }
}

impl Page for ListsPage {
    fn title(&self) -> &'static str {
        "Lists"
    }
    fn blurb(&self) -> &'static str {
        "Single and multiple selection, disabled items, scrolling, empty state"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let third = area.width / 3;
        let cols = [
            Rect::new(area.x, area.y, third.saturating_sub(1), area.height.min(18)),
            Rect::new(
                area.x + third + 1,
                area.y,
                third.saturating_sub(1),
                area.height.min(18),
            ),
            Rect::new(
                area.x + 2 * third + 2,
                area.y,
                area.width.saturating_sub(2 * third + 2),
                area.height.min(18),
            ),
        ];

        let chosen = self
            .single
            .chosen()
            .and_then(|i| LANGUAGES.get(*i).copied())
            .unwrap_or("");
        let pos = layout::overflow_label(self.single.offset(), self.single_view, LANGUAGES.len());
        let (inner, bg) = layout::card(
            cols[0],
            buf,
            t,
            Some("Language"),
            Some(&pos),
            ctx.interaction.focused(ID.sub("single")),
        );
        buf.set_string(
            inner.x,
            inner.y,
            format!("Chosen: {chosen}"),
            t.muted().bg(bg),
        );
        let list_area = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        let lang_rows: Vec<ListRow<'_, usize>> = LANGUAGES
            .iter()
            .enumerate()
            .map(|(i, l)| ListRow::item(i, Line::from(*l)))
            .collect();
        let single_focused = ctx.interaction.focused(ID.sub("single"));
        let single_chosen = self.single.chosen().copied();
        render_source_list(
            &lang_rows,
            list_area,
            buf,
            ctx,
            &mut self.single,
            single_focused,
            single_chosen.filter(|_| single_focused),
        );
        ctx.control(ID.sub("single"), list_area, false);
        ctx.scrollable(ID.sub("single"), list_area);
        self.single_view = usize::from(list_area.height);

        let count = format!(
            "{} selected",
            self.multi
                .selection()
                .map(|s| s.checked().len())
                .unwrap_or(0)
        );
        let (inner, bg) = layout::card(
            cols[1],
            buf,
            t,
            Some("Files to include"),
            Some(&count),
            ctx.interaction.focused(ID.sub("multi")),
        );
        buf.set_string(
            inner.x,
            inner.y,
            text::truncate("Space toggle · a all · Shift+↓ range", inner.width as usize),
            t.muted().bg(bg),
        );
        let list_area = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        let file_rows: Vec<ListRow<'_, usize>> = FILES
            .iter()
            .enumerate()
            .map(|(i, (n, m, d))| {
                let mut row = ListRow::item(i, Line::from(*n)).status(Line::from(*m));
                if *d {
                    row = row.disabled();
                }
                row
            })
            .collect();
        render_source_list(
            &file_rows,
            list_area,
            buf,
            ctx,
            &mut self.multi,
            false,
            None,
        );
        ctx.control(ID.sub("multi"), list_area, false);
        ctx.scrollable(ID.sub("multi"), list_area);

        let (inner, _bg) = layout::card(cols[2], buf, t, Some("Search results"), None, false);
        let empty_rows: [ListRow<'_, usize>; 0] = [];
        // Source insets the empty pane two cells inside the card on each side,
        // so the message clips at the pane rather than at the card.
        let empty_area = Rect::new(
            inner.x.saturating_add(2),
            inner.y,
            inner.width.saturating_sub(4),
            inner.height,
        );
        if let Some(pointer) = ctx.interaction.pointer {
            self.empty.hover(pointer);
        }
        List::new(&empty_rows, ctx.system)
            .empty_message(Line::from("No results for “retry”"))
            .focused(false)
            .render(empty_area, buf, &mut self.empty);
        ctx.control(ID.sub("empty"), inner, false);
        ctx.scrollable(ID.sub("empty"), inner);
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == ID.sub("single") {
                    let rows: Vec<ListRow<'_, usize>> = LANGUAGES
                        .iter()
                        .enumerate()
                        .map(|(i, l)| ListRow::item(i, Line::from(*l)))
                        .collect();
                    return list_route(self.single.handle_key(&rows, *key));
                }
                if f == ID.sub("multi") {
                    let rows: Vec<ListRow<'_, usize>> = FILES
                        .iter()
                        .enumerate()
                        .map(|(i, (n, m, d))| {
                            let mut row = ListRow::item(i, Line::from(*n)).status(Line::from(*m));
                            if *d {
                                row = row.disabled();
                            }
                            row
                        })
                        .collect();
                    let out = self.multi.handle_key(&rows, *key);
                    if let ListOutcome::Activated(id) = &out
                        && let Some(sel) = self.multi.selection_mut()
                    {
                        sel.toggle(id);
                    }
                    return list_route(out);
                }
                if f == ID.sub("empty") {
                    let rows: [ListRow<'_, usize>; 0] = [];
                    return list_route(self.empty.handle_key(&rows, *key));
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                for (list, lid, n) in self.lists() {
                    if *id != lid {
                        continue;
                    }
                    cx.set_focus(lid);
                    if pos.x
                        >= list
                            .regions()
                            .first()
                            .map(|r| r.area.right())
                            .unwrap_or(pos.x.saturating_add(1))
                    {
                        if list.scroll_to_position(*pos, n) {
                            return Route::Changed;
                        }
                    }
                    let out = list.click(*pos);
                    if lid == ID.sub("multi")
                        && let ListOutcome::Activated(row) = &out
                        && let Some(sel) = list.selection_mut()
                    {
                        sel.toggle(row);
                    }
                    return list_route(out).or(Route::Changed);
                }
                Route::Ignored
            }
            PageEvent::Drag { pressed, pos } => {
                for (list, lid, n) in self.lists() {
                    if *pressed == lid && list.scroll_to_position(*pos, n) {
                        return Route::Changed;
                    }
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } => {
                for (list, lid, n) in self.lists() {
                    if *id == lid && list.scroll_by(*delta as isize, n) {
                        return Route::Changed;
                    }
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(ID.sub("multi")) {
            vec![
                ("↑ ↓", "Move"),
                ("Space", "Toggle"),
                ("a", "All / none"),
                ("Shift+↓", "Range"),
            ]
        } else {
            vec![("↑ ↓", "Move"), ("Enter", "Choose"), ("g G", "Ends")]
        }
    }
}
