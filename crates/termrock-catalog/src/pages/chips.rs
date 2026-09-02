// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/chips.rs (MIT).

//! Removable chips, a popup select, and strips that drop what does not fit.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock::input::{
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::style::Role;
use termrock::widgets::{
    EmptyKind, EmptyState, LineSegment, Select, SelectOption, SelectOutcome, SelectRecipe,
    SelectState, TokenItem, TokenStrip, TokenStripOutcome, TokenStripState, paint_line_segments,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("chips");

const CANDIDATES: &[&str] = &[
    "created_at > '2026-01-01'",
    "currency = 'EUR'",
    "notes is not null",
    "seats between 5 and 50",
];

struct FilterChip {
    label: String,
    enabled: bool,
}

pub struct ChipsPage {
    chips: Vec<FilterChip>,
    strip: TokenStripState<usize>,
    match_all: bool,
    next_candidate: usize,
    sort: SelectState<usize>,
    page_size: SelectState<usize>,
    engine: SelectState<usize>,
    last: String,
    /// Chip gutter follows strip keys/clicks, not mere card focus (s_chips golden).
    chip_cursor_live: bool,
}

impl ChipsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chips: vec![
                FilterChip {
                    label: "status = 'pending'".into(),
                    enabled: true,
                },
                FilterChip {
                    label: "total > 100".into(),
                    enabled: true,
                },
                FilterChip {
                    label: "country in (DE, FR)".into(),
                    enabled: false,
                },
            ],
            strip: TokenStripState::new(),
            match_all: true,
            next_candidate: 0,
            sort: SelectState::new()
                .with_value(0)
                .with_recipe(SelectRecipe::Form),
            page_size: SelectState::new()
                .with_value(1)
                .with_recipe(SelectRecipe::Form),
            engine: {
                let mut s = SelectState::new()
                    .with_value(0)
                    .with_recipe(SelectRecipe::Form);
                s.set_enabled(false);
                s
            },
            last: "nothing yet".into(),
            chip_cursor_live: false,
        }
    }

    fn lead_label(&self) -> &'static str {
        if self.match_all {
            "match all ▾"
        } else {
            "match any ▾"
        }
    }

    fn sort_opts() -> [SelectOption<usize>; 4] {
        [
            SelectOption::option(0, "created_at"),
            SelectOption::option(1, "total"),
            SelectOption::option(2, "status"),
            SelectOption::option(3, "customer"),
        ]
    }
    fn size_opts() -> [SelectOption<usize>; 4] {
        [
            SelectOption::option(0, "25"),
            SelectOption::option(1, "50"),
            SelectOption::option(2, "100"),
            SelectOption::option(3, "500"),
        ]
    }
    fn engine_opts() -> [SelectOption<usize>; 1] {
        [SelectOption::option(0, "PostgreSQL")]
    }

    fn on_chip(&mut self, ev: TokenStripOutcome<usize>, cx: &mut PageCtx<'_>) {
        match ev {
            TokenStripOutcome::Activated(i)
            | TokenStripOutcome::Selected(i)
            | TokenStripOutcome::Unselected(i) => {
                if matches!(ev, TokenStripOutcome::Activated(_)) {
                    if let Some(c) = self.chips.get(i) {
                        self.last = format!("edit {}", c.label);
                        cx.status(format!("Would open the editor for {}", c.label));
                    }
                } else if let Some(c) = self.chips.get_mut(i) {
                    c.enabled = !c.enabled;
                    self.last = format!(
                        "{} {}",
                        if c.enabled { "enabled" } else { "disabled" },
                        c.label
                    );
                }
            }
            TokenStripOutcome::Remove(i) => {
                if i < self.chips.len() {
                    let c = self.chips.remove(i);
                    self.last = format!("removed {}", c.label);
                }
            }
            TokenStripOutcome::Add => {
                let label = CANDIDATES[self.next_candidate % CANDIDATES.len()];
                self.next_candidate += 1;
                self.chips.push(FilterChip {
                    label: label.to_owned(),
                    enabled: true,
                });
                self.strip.set_cursor(Some(self.chips.len() - 1));
                self.last = format!("added {label}");
            }
            _ => {}
        }
    }
}

fn mouse_down(pos: ratatui::layout::Position) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        position: pos,
        modifiers: KeyModifiers::NONE,
    }
}

impl Page for ChipsPage {
    fn title(&self) -> &'static str {
        "Chips & selects"
    }
    fn blurb(&self) -> &'static str {
        "Removable chips, a popup select, and strips that drop what does not fit"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[6, 1, 8, 1, 0]);

        let focused = ctx.interaction.focused(ID.sub("filters"));
        let active = self.chips.iter().filter(|c| c.enabled).count();
        let meta = format!("{active} active");
        let (inner, bg) = layout::card(rows[0], buf, t, Some("Filters"), Some(&meta), focused);
        // Shot `s_chips`: Filters card is focused (title ▎) but ChipBar chips
        // stay idle (gutter fg = chip fill). Source ChipBar would mark
        // cursor 0 focused; the golden does not. Paint chip focus only when
        // the strip's internal cursor has been moved.
        self.strip.set_surface_focused(focused);
        self.strip.show_chip_cursor = self.chip_cursor_live;
        let chip_snap: Vec<(usize, String, bool)> = self
            .chips
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.label.clone(), c.enabled))
            .collect();
        let items: Vec<TokenItem<'_, usize>> = chip_snap
            .iter()
            .map(|(i, label, enabled)| {
                TokenItem::chip(*i, label.as_str())
                    .removable(true)
                    .selected(*enabled)
            })
            .collect();
        let strip_area = Rect::new(inner.x, inner.y, inner.width, 1);
        TokenStrip::new(&items, ctx.system)
            .lead(Some(self.lead_label()))
            .add_label(Some("+ Add filter"))
            .paint(strip_area, buf, &mut self.strip);
        ctx.control(ID.sub("filters"), strip_area, false);
        if let Some(r) = self.strip.lead_region {
            ctx.clickable(ID.sub("lead"), r);
        }
        if inner.y + 2 < inner.bottom() {
            buf.set_string(
                inner.x,
                inner.y + 2,
                text::truncate(&format!("last action: {}", self.last), inner.width as usize),
                t.muted().bg(bg),
            );
        }

        let (inner, _bg) = layout::card(rows[2], buf, t, Some("Selects"), None, false);
        let third = inner.width / 3;
        let cells = [
            Rect::new(inner.x, inner.y, third.saturating_sub(2), 3),
            Rect::new(inner.x + third, inner.y, third.saturating_sub(2), 3),
            Rect::new(
                inner.x + third * 2,
                inner.y,
                inner.width.saturating_sub(third * 2),
                3,
            ),
        ];
        let sort_opts = Self::sort_opts();
        let size_opts = Self::size_opts();
        let engine_opts = Self::engine_opts();
        self.sort
            .set_focused(ctx.interaction.focused(ID.sub("sort")));
        self.page_size
            .set_focused(ctx.interaction.focused(ID.sub("size")));
        self.engine
            .set_focused(ctx.interaction.focused(ID.sub("engine")));
        let open = [
            self.sort.is_open(),
            self.page_size.is_open(),
            self.engine.is_open(),
        ]
        .iter()
        .position(|o| *o);
        // Source paints closed selects first, then the open one so its popup
        // covers sibling fields. Strip and properties still paint *after*
        // that, so they overwrite the popup's bottom border (`s_chips_select`
        // `╰` sits under "Segment strip", not on top of it).
        if open != Some(0) {
            Select::new(&sort_opts, ctx.system)
                .label("Sort by")
                .help("Applies to the next query")
                .paint(cells[0], Rect::default(), buf, &mut self.sort);
        }
        ctx.control(ID.sub("sort"), cells[0], false);
        if open != Some(1) {
            Select::new(&size_opts, ctx.system)
                .label("Page size")
                .paint(cells[1], Rect::default(), buf, &mut self.page_size);
        }
        ctx.control(ID.sub("size"), cells[1], false);
        if open != Some(2) {
            Select::new(&engine_opts, ctx.system)
                .label("Engine")
                .help("Fixed by the connection")
                .paint(cells[2], Rect::default(), buf, &mut self.engine);
        }
        ctx.control(ID.sub("engine"), cells[2], true);
        match open {
            Some(0) => {
                Select::new(&sort_opts, ctx.system)
                    .label("Sort by")
                    .help("Applies to the next query")
                    .paint(cells[0], Rect::default(), buf, &mut self.sort);
            }
            Some(1) => {
                Select::new(&size_opts, ctx.system)
                    .label("Page size")
                    .paint(cells[1], Rect::default(), buf, &mut self.page_size);
            }
            Some(2) => {
                Select::new(&engine_opts, ctx.system)
                    .label("Engine")
                    .help("Fixed by the connection")
                    .paint(cells[2], Rect::default(), buf, &mut self.engine);
            }
            Some(_) | None => {}
        }
        let rest = rows[4];
        let strip_h = rest.height.min(7);
        let (inner, bg) = layout::card(
            Rect::new(rest.x, rest.y, rest.width, strip_h),
            buf,
            t,
            Some("Segment strip"),
            None,
            false,
        );
        let left = [
            LineSegment::new("▪").tone(Role::Success).priority(9),
            LineSegment::new("Acme").bold().priority(9),
            LineSegment::new("◆ production")
                .tone(Role::Warning)
                .priority(8),
            LineSegment::new("acme_prod › public")
                .tone(Role::TextSecondary)
                .priority(6),
            LineSegment::new("safe").bold().priority(7),
        ];
        let right = [
            LineSegment::new("3 pending")
                .tone(Role::Warning)
                .priority(5),
            LineSegment::new("truecolor · 120×40")
                .tone(Role::TextMuted)
                .priority(2),
            LineSegment::new("? help").tone(Role::TextMuted).priority(3),
        ];
        paint_line_segments(
            Rect::new(inner.x, inner.y, inner.width, 1),
            buf,
            ctx.system,
            &left,
            &right,
            bg,
        );
        let narrow = Rect::new(inner.x, inner.y + 2, inner.width.min(44), 1).intersection(inner);
        paint_line_segments(narrow, buf, ctx.system, &left, &right, bg);
        if inner.y + 3 < inner.bottom() {
            buf.set_string(
                inner.x,
                inner.y + 3,
                text::truncate(
                    "the same strip at 44 columns: low-priority segments leave first, from the right",
                    inner.width as usize,
                ),
                t.faint().bg(bg),
            );
        }

        let below = Rect::new(
            rest.x,
            rest.y + strip_h + 1,
            rest.width,
            rest.height.saturating_sub(strip_h + 1),
        );
        if below.height < 3 {
            return;
        }
        let (l, r) = layout::columns(below, (below.width * 55 / 100).max(36), 2);
        let (inner, bg) = layout::card(
            Rect::new(l.x, l.y, l.width, l.height.min(11)),
            buf,
            t,
            Some("Properties"),
            None,
            false,
        );
        // Source props::render: labels at `area.x`, no list gutter.
        let props: [(&str, &str, bool, Role); 5] = [
            ("Engine", "PostgreSQL 16.3", false, Role::Text),
            ("Host", "prod-db-1.acme.io:5432", false, Role::Text),
            ("Environment", "production", false, Role::Warning),
            (
                "Safe Mode",
                "Writes ask for confirmation and a deliberate acknowledgement.",
                true,
                Role::Text,
            ),
            ("Last used", "1 hour ago", false, Role::TextMuted),
        ];
        let label_w = props
            .iter()
            .map(|(label, _, _, _)| text::width(label) as u16)
            .max()
            .unwrap_or(0)
            + 2;
        let mut y = inner.y;
        for (label, value, wrap, role) in props {
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, label, t.muted().bg(bg));
            let vw = inner.width.saturating_sub(label_w) as usize;
            let style = ctx.system.style(role).bg(bg);
            if wrap {
                for line in text::wrap(value, vw.max(4)) {
                    if y >= inner.bottom() {
                        break;
                    }
                    buf.set_string(inner.x.saturating_add(label_w), y, &line, style);
                    y = y.saturating_add(1);
                }
            } else {
                buf.set_string(
                    inner.x.saturating_add(label_w),
                    y,
                    text::truncate(value, vw),
                    style,
                );
                y = y.saturating_add(1);
            }
        }

        let (inner, _bg) = layout::card(
            Rect::new(r.x, r.y, r.width, r.height.min(11)),
            buf,
            t,
            Some("Empty state"),
            None,
            false,
        );
        EmptyState::new("No results yet", ctx.system)
            .kind(EmptyKind::NoResults)
            .explanation("A title and one hint, centred in whatever is left")
            .paint(inner, buf);
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == ID.sub("filters") {
                    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                        return Route::Ignored;
                    }
                    if key.code == KeyCode::Char('X') {
                        self.chips.clear();
                        self.last = "cleared all filters".into();
                        return Route::Changed;
                    }
                    if key.code == KeyCode::Char('x') {
                        if let Some(i) = self.strip.cursor().copied()
                            && i < self.chips.len()
                        {
                            let c = self.chips.remove(i);
                            self.last = format!("removed {}", c.label);
                            return Route::Changed;
                        }
                    }
                    if matches!(key.code, KeyCode::Char('+')) {
                        self.on_chip(TokenStripOutcome::Add, cx);
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Enter) {
                        if let Some(i) = self.strip.cursor().copied() {
                            self.on_chip(TokenStripOutcome::Activated(i), cx);
                            return Route::Changed;
                        }
                    }
                    let chip_snap: Vec<(usize, String, bool)> = self
                        .chips
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (i, c.label.clone(), c.enabled))
                        .collect();
                    let items: Vec<TokenItem<'_, usize>> = chip_snap
                        .iter()
                        .map(|(i, label, enabled)| {
                            TokenItem::chip(*i, label.as_str())
                                .removable(true)
                                .selected(*enabled)
                        })
                        .collect();
                    let system = termrock::style::DesignSystem::junie();
                    let strip = TokenStrip::new(&items, &system).add_label(Some("+ Add filter"));
                    let out = strip.handle_key(&mut self.strip, *key);
                    if matches!(out, TokenStripOutcome::Ignored) {
                        return Route::Ignored;
                    }
                    self.chip_cursor_live = true;
                    self.on_chip(out, cx);
                    return Route::Changed;
                }
                let bounds = Rect::new(0, 0, 120, 40);
                if f == ID.sub("sort") {
                    let opts = Self::sort_opts();
                    return select_key(&mut self.sort, *key, &opts, bounds, "Sort by", cx);
                }
                if f == ID.sub("size") {
                    let opts = Self::size_opts();
                    return select_key(&mut self.page_size, *key, &opts, bounds, "Page size", cx);
                }
                if f == ID.sub("engine") {
                    let opts = Self::engine_opts();
                    return select_key(&mut self.engine, *key, &opts, bounds, "Engine", cx);
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == ID.sub("lead") {
                    self.match_all = !self.match_all;
                    self.last = format!("match {}", if self.match_all { "all" } else { "any" });
                    return Route::Changed;
                }
                if *id == ID.sub("filters") {
                    cx.set_focus(ID.sub("filters"));
                    let chip_snap: Vec<(usize, String, bool)> = self
                        .chips
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (i, c.label.clone(), c.enabled))
                        .collect();
                    let items: Vec<TokenItem<'_, usize>> = chip_snap
                        .iter()
                        .map(|(i, label, enabled)| {
                            TokenItem::chip(*i, label.as_str())
                                .removable(true)
                                .selected(*enabled)
                        })
                        .collect();
                    let system = termrock::style::DesignSystem::junie();
                    let strip = TokenStrip::new(&items, &system)
                        .lead(Some(self.lead_label()))
                        .add_label(Some("+ Add filter"));
                    let out = strip.handle_mouse(&mut self.strip, mouse_down(*pos));
                    if !matches!(out, TokenStripOutcome::Ignored) {
                        self.chip_cursor_live = true;
                        self.on_chip(out, cx);
                    }
                    return Route::Changed;
                }
                let bounds = Rect::new(0, 0, 120, 40);
                let mut out = Route::Ignored;
                if *id == ID.sub("sort") || self.sort.is_open() {
                    let opts = Self::sort_opts();
                    let ev = self.sort.handle_mouse(mouse_down(*pos), &opts, bounds);
                    out = out.or(select_route(ev, "Sort by", &opts, cx));
                    if *id == ID.sub("sort") {
                        cx.set_focus(ID.sub("sort"));
                    }
                }
                if *id == ID.sub("size") || self.page_size.is_open() {
                    let opts = Self::size_opts();
                    let ev = self.page_size.handle_mouse(mouse_down(*pos), &opts, bounds);
                    out = out.or(select_route(ev, "Page size", &opts, cx));
                    if *id == ID.sub("size") {
                        cx.set_focus(ID.sub("size"));
                    }
                }
                if *id == ID.sub("engine") || self.engine.is_open() {
                    let opts = Self::engine_opts();
                    let ev = self.engine.handle_mouse(mouse_down(*pos), &opts, bounds);
                    out = out.or(select_route(ev, "Engine", &opts, cx));
                    if *id == ID.sub("engine") {
                        cx.set_focus(ID.sub("engine"));
                    }
                }
                if *id != ID.sub("sort") && self.sort.is_open() {
                    let _ = self.sort.close();
                    out = Route::Changed;
                }
                if *id != ID.sub("size") && self.page_size.is_open() {
                    let _ = self.page_size.close();
                    out = Route::Changed;
                }
                if *id != ID.sub("engine") && self.engine.is_open() {
                    let _ = self.engine.close();
                    out = Route::Changed;
                }
                out
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(ID.sub("filters")) {
            vec![
                ("← →", "Move"),
                ("Space", "Toggle"),
                ("Enter", "Edit / add"),
                ("x", "Remove"),
                ("X", "Clear all"),
            ]
        } else if focus
            .is_some_and(|f| [ID.sub("sort"), ID.sub("size"), ID.sub("engine")].contains(&f))
        {
            vec![("Enter", "Open"), ("↑ ↓", "Choose"), ("Esc", "Close")]
        } else {
            vec![("Tab", "Next")]
        }
    }
}

fn select_key(
    state: &mut SelectState<usize>,
    key: termrock::input::KeyEvent,
    opts: &[SelectOption<usize>],
    bounds: Rect,
    label: &str,
    cx: &mut PageCtx<'_>,
) -> Route {
    state.set_focused(true);
    let ev = state.handle_key(key, opts, bounds);
    select_route(ev, label, opts, cx)
}

fn select_route(
    ev: SelectOutcome<usize>,
    label: &str,
    opts: &[SelectOption<usize>],
    cx: &mut PageCtx<'_>,
) -> Route {
    match ev {
        SelectOutcome::Ignored => Route::Ignored,
        SelectOutcome::ValueChanged { id } => {
            if let Some(opt) = opts.iter().find(|o| o.id == id) {
                cx.status(format!("{label} → {}", opt.label));
            }
            Route::Changed
        }
        _ => Route::Changed,
    }
}
