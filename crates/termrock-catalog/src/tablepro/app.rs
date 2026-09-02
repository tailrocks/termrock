// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/app.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! TablePro shell: screens, overlays, identity strip, routing.
//! Same type for the standalone binary and Applications → TablePro.

use std::ops::ControlFlow;
use std::time::Duration;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::interaction::{InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind};
use termrock::runtime::FrameTick;
use termrock::style::{ColorCapability, DesignSystem, JunieTheme};
use termrock::widgets::{
    ButtonState, ButtonVariant, List, ListRow, ListState, StatusSegment, StatusStrip, TextInput,
    TextInputState,
};

use super::connections::{ConnEvent, ConnectionsScreen};
use super::db::{Catalog, Environment, connections};
use super::model::{History, SwitchItem, SwitchTarget, SwitcherIndex};
use super::paint;
use super::sql::{self, Decision};
use super::text::truncate_middle;
use super::workbench::{EXPLORER, WorkTab, Workbench};
use crate::ctx::{Interaction, LayerId, RenderCtx};
use crate::draw::fill;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, PageCtx, PageEvent};

pub const MIN_WIDTH: u16 = 72;
pub const MIN_HEIGHT: u16 = 20;

const STRIP_SAFE: WidgetId = WidgetId::of("strip.safe");
const STRIP_CONN: WidgetId = WidgetId::of("strip.conn");
const STRIP_HELP: WidgetId = WidgetId::of("strip.help");
const HELP_CLOSE: WidgetId = WidgetId::of("dialog.help.close");
const SWITCHER_INPUT: WidgetId = WidgetId::of("switcher.input");
const SWITCHER_LIST: WidgetId = WidgetId::of("switcher.list");
const CONFIRM_OK: WidgetId = WidgetId::of("dialog.confirm.ok");
const CONFIRM_CANCEL: WidgetId = WidgetId::of("dialog.confirm.cancel");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connections,
    Workbench,
}

enum Overlay {
    None,
    Help,
    Confirm {
        title: String,
        body: String,
        ok: String,
        _deliberate: bool,
    },
    Switcher,
}

struct Host {
    scene: InteractionScene<WidgetId, LayerId, ()>,
    focus: Option<WidgetId>,
    hover: Option<WidgetId>,
    pressed: Option<WidgetId>,
    hover_suppressed: bool,
    flash: Option<(WidgetId, u64)>,
    scroll_hits: Vec<(WidgetId, Rect)>,
}

impl Host {
    fn new() -> Self {
        let mut scene = InteractionScene::new();
        scene.ensure_root(root_layer());
        Self {
            scene,
            focus: Some(super::connections::TREE),
            hover: None,
            pressed: None,
            hover_suppressed: false,
            flash: None,
            scroll_hits: Vec::new(),
        }
    }
}

pub struct App {
    pub theme: JunieTheme,
    pub system: DesignSystem,
    pub screen: Screen,
    pub connections: ConnectionsScreen,
    pub workbench: Option<Workbench>,
    pub history: History,
    pub status: Option<(String, u64)>,
    pub tick: u64,
    pub elapsed_ms: u64,
    pub quit: bool,
    pub size: (u16, u16),
    /// Hardware cursor from the last `render`. None = hidden.
    pub last_cursor: Option<Position>,
    overlay: Overlay,
    switcher_q: TextInputState,
    switcher_list: ListState<usize>,
    switcher_items: Vec<SwitchItem>,
    confirm_ok: ButtonState,
    confirm_cancel: ButtonState,
    help_close: ButtonState,
    host: Host,
}

impl App {
    #[must_use]
    pub fn new(level: ColorCapability) -> Self {
        let theme = JunieTheme::for_level(level);
        let system = DesignSystem::junie().capability(level);
        let connections = ConnectionsScreen::new(connections());
        let mut switcher_q = TextInputState::new("").with_allow_empty(true);
        switcher_q.set_editing(false);
        Self {
            theme,
            system,
            screen: Screen::Connections,
            connections,
            workbench: None,
            history: History::seeded(),
            status: None,
            tick: 0,
            elapsed_ms: 0,
            quit: false,
            size: (0, 0),
            last_cursor: None,
            overlay: Overlay::None,
            switcher_q,
            switcher_list: ListState::new(Some(0)),
            switcher_items: vec![],
            confirm_ok: ButtonState::new(),
            confirm_cancel: ButtonState::new(),
            help_close: ButtonState::new(),
            host: Host::new(),
        }
    }

    /// Connect immediately (used by `--connect` and tests).
    pub fn connect(&mut self, index: usize) {
        let Some(c) = self.connections.connections.get(index).cloned() else {
            return;
        };
        let mut wb = Workbench::new(c, Catalog::acme_prod());
        wb.new_query("");
        self.workbench = Some(wb);
        self.screen = Screen::Workbench;
        self.host.focus = Some(EXPLORER);
        self.set_status(format!(
            "Connected to {}",
            self.workbench.as_ref().unwrap().connection.name
        ));
    }

    /// Case-insensitive connect by saved name. `Production` skips the list.
    pub fn connect_named(&mut self, name: &str) -> Result<(), String> {
        let i = self
            .connections
            .connections
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("no connection named {name:?}"))?;
        self.connect(i);
        Ok(())
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        self.connections.animating()
            || self.workbench.as_ref().is_some_and(Workbench::animating)
            || self.host.flash.is_some()
            || matches!(self.overlay, Overlay::None) && self.status.is_some()
    }

    fn set_status(&mut self, s: String) {
        self.status = Some((s, self.elapsed_ms));
    }

    fn interaction(&self) -> Interaction {
        let flash = match self.host.flash {
            Some((id, at)) if self.elapsed_ms.saturating_sub(at) < 140 => Some(id),
            _ => None,
        };
        Interaction {
            focus: self.host.focus,
            hover: self.host.hover,
            pressed: self.host.pressed,
            flash,
            focus_hidden: false,
            hover_suppressed: self.host.hover_suppressed,
            tick: self.tick,
        }
    }

    /// Shared surface used by the catalog page and the standalone host.
    pub fn render_surface(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        if self.connections.animating() || self.workbench.as_ref().is_some_and(Workbench::animating)
        {
            self.tick = self.tick.wrapping_add(1);
            if let Some(ConnEvent::Connected(i)) = self.connections.tick() {
                self.connect(i);
            }
            if let Some(wb) = self.workbench.as_mut() {
                let _ = wb.tick(&mut self.history);
            }
        }
        let t = ctx.theme;
        fill(buf, area, t.base());
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let msg = format!("Need {MIN_WIDTH}×{MIN_HEIGHT}");
            buf.set_string(area.x, area.y, "TablePro", t.title());
            buf.set_string(area.x, area.y + 1, "Terminal too small", t.secondary());
            buf.set_string(area.x, area.y + 2, &msg, t.muted());
            return;
        }
        let overlay = !matches!(self.overlay, Overlay::None);
        let saved = ctx.inert;
        ctx.inert = saved || overlay;
        let strip = Rect::new(area.x, area.y, area.width, 1);
        self.draw_strip(strip, buf, ctx);
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(3),
        );
        match self.screen {
            Screen::Connections => self.connections.render(body, buf, ctx),
            Screen::Workbench => {
                if let Some(wb) = self.workbench.as_mut() {
                    let hist = matches!(wb.tabs.get(wb.active), Some(WorkTab::History(_)));
                    wb.render(body, buf, ctx);
                    if hist {
                        wb.render_history_tab(&self.history, body, buf, ctx);
                    }
                }
            }
        }
        ctx.inert = saved;
        if overlay {
            self.draw_overlay(area, buf, ctx);
        }
    }

    fn draw_strip(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let mut left = vec![
            StatusSegment::new("▪").priority(9),
            StatusSegment::new("TablePro").strong().priority(9),
        ];
        let mut right_store: Vec<String> = Vec::new();
        match self.screen {
            Screen::Connections => {
                left.push(StatusSegment::new("Connections").priority(8));
                right_store.push(format!("{} saved", self.connections.connections.len()));
            }
            Screen::Workbench => {
                if let Some(w) = &self.workbench {
                    let c = &w.connection;
                    right_store.push(truncate_middle(&c.name, 18));
                    right_store.push(match c.environment {
                        Environment::Production => "◆ production".into(),
                        Environment::Staging => "◇ staging".into(),
                        Environment::Development => "development".into(),
                        Environment::Local => "local".into(),
                    });
                    right_store.push(format!("{} › {}", w.catalog.database, w.schema));
                    right_store.push(c.safe_mode.token().to_owned());
                    if w.running().is_some() {
                        right_store.push("running".into());
                    }
                    let pending = w.pending_total();
                    if pending > 0 {
                        right_store.push(format!("• {pending} pending"));
                    }
                }
            }
        }
        right_store.push(format!("{}×{}", self.size.0, self.size.1));
        right_store.push("? help".into());
        let right: Vec<StatusSegment> = right_store
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut seg = StatusSegment::new(s);
                if i + 1 == right_store.len() {
                    seg = seg.priority(4);
                }
                seg
            })
            .collect();
        let mut segs = left;
        segs.extend(right);
        StatusStrip::new(&segs, ctx.system).paint(area, buf);
        ctx.clickable(
            STRIP_HELP,
            Rect::new(area.right().saturating_sub(8), area.y, 8, 1),
        );
        ctx.clickable(STRIP_CONN, area);
        ctx.clickable(STRIP_SAFE, area);
    }

    fn draw_overlay(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        match &self.overlay {
            Overlay::Help => {
                let w = area.width.min(64).max(40);
                let h = area.height.min(16).max(10);
                let x = area.x + area.width.saturating_sub(w) / 2;
                let y = area.y + area.height.saturating_sub(h) / 2;
                let (inner, bg) =
                    layout::card(Rect::new(x, y, w, h), buf, t, Some("Keyboard"), None, true);
                let lines = [
                    "Ctrl+O  open quickly",
                    "Ctrl+T  new query",
                    "Ctrl+R  run statement",
                    "Ctrl+Y  history",
                    "0       explorer",
                    "?       help",
                    "q       quit (standalone)",
                ];
                for (i, line) in lines.iter().enumerate() {
                    buf.set_string(inner.x, inner.y + i as u16, line, t.primary().bg(bg));
                }
                let close = Rect::new(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    paint::button_width("Close"),
                    1,
                );
                paint::button(
                    "Close",
                    ButtonVariant::Secondary,
                    HELP_CLOSE,
                    close,
                    buf,
                    ctx,
                    &mut self.help_close,
                    false,
                    bg,
                );
            }
            Overlay::Confirm {
                title, body, ok, ..
            } => {
                let w = area.width.min(56).max(32);
                let h = 10;
                let x = area.x + area.width.saturating_sub(w) / 2;
                let y = area.y + area.height.saturating_sub(h) / 2;
                let (inner, bg) =
                    layout::card(Rect::new(x, y, w, h), buf, t, Some(title), None, true);
                for (i, line) in crate::text::wrap(body, inner.width as usize)
                    .iter()
                    .take(4)
                    .enumerate()
                {
                    buf.set_string(inner.x, inner.y + i as u16, line, t.secondary().bg(bg));
                }
                let ay = inner.bottom().saturating_sub(1);
                let ok_w = paint::button_width(ok);
                let cancel_w = paint::button_width("Cancel");
                let rects = layout::row_layout_right(
                    Rect::new(inner.x, ay, inner.width, 1),
                    &[cancel_w, ok_w],
                    2,
                );
                paint::button(
                    "Cancel",
                    ButtonVariant::Quiet,
                    CONFIRM_CANCEL,
                    rects[0],
                    buf,
                    ctx,
                    &mut self.confirm_cancel,
                    false,
                    bg,
                );
                paint::button(
                    ok,
                    ButtonVariant::Primary,
                    CONFIRM_OK,
                    rects[1],
                    buf,
                    ctx,
                    &mut self.confirm_ok,
                    false,
                    bg,
                );
            }
            Overlay::Switcher => {
                let w = area.width.min(60).max(36);
                let h = area.height.min(18).max(10);
                let x = area.x + area.width.saturating_sub(w) / 2;
                let y = area.y + area.height.saturating_sub(h) / 2;
                let (inner, _bg) = layout::card(
                    Rect::new(x, y, w, h),
                    buf,
                    t,
                    Some("Open quickly"),
                    None,
                    true,
                );
                self.switcher_q
                    .set_focused(ctx.interaction.focused(SWITCHER_INPUT));
                TextInput::new("", ctx.system)
                    .placeholder("Filter tables, tabs, queries")
                    .paint(
                        Rect::new(inner.x, inner.y, inner.width, 2),
                        buf,
                        &mut self.switcher_q,
                    );
                ctx.control(
                    SWITCHER_INPUT,
                    Rect::new(inner.x, inner.y, inner.width, 2),
                    false,
                );
                let list_r = Rect::new(
                    inner.x,
                    inner.y + 3,
                    inner.width,
                    inner.height.saturating_sub(3),
                );
                let rows: Vec<ListRow<usize>> = self
                    .switcher_items
                    .iter()
                    .enumerate()
                    .map(|(i, it)| {
                        ListRow::item(i, Line::from(it.label.as_str()))
                            .secondary(Line::from(it.path.as_str()))
                    })
                    .collect();
                StatefulWidget::render(
                    &List::new(&rows, ctx.system).focused(ctx.interaction.focused(SWITCHER_LIST)),
                    list_r,
                    buf,
                    &mut self.switcher_list,
                );
                ctx.control(SWITCHER_LIST, list_r, false);
            }
            Overlay::None => {}
        }
    }

    pub fn handle_surface(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if !matches!(self.overlay, Overlay::None) {
            return self.handle_overlay(ev, cx);
        }
        match ev {
            PageEvent::Tick => {
                let mut changed = false;
                if let Some(ConnEvent::Connected(i)) = self.connections.tick() {
                    self.connect(i);
                    changed = true;
                }
                if let Some(wb) = self.workbench.as_mut() {
                    changed |= wb.tick(&mut self.history);
                }
                if let Some((_, at)) = self.status
                    && self.elapsed_ms.saturating_sub(at) > 4000
                {
                    self.status = None;
                    changed = true;
                }
                if changed {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if self.handle_chords(key, cx) {
                    return Route::Changed;
                }
                match self.screen {
                    Screen::Connections => {
                        let (route, ev) = self.connections.handle(&PageEvent::Key(*key), cx);
                        if let Some(ConnEvent::Connected(i)) = ev {
                            self.connect(i);
                            return Route::Changed;
                        }
                        route
                    }
                    Screen::Workbench => {
                        if let Some(wb) = self.workbench.as_mut() {
                            wb.handle(&PageEvent::Key(*key), cx, &self.history)
                        } else {
                            Route::Ignored
                        }
                    }
                }
            }
            PageEvent::Click { id, .. } if *id == STRIP_HELP => {
                self.overlay = Overlay::Help;
                Route::Changed
            }
            PageEvent::Click { id, .. }
                if *id == STRIP_CONN && self.screen == Screen::Workbench =>
            {
                self.screen = Screen::Connections;
                self.workbench = None;
                Route::Changed
            }
            other => match self.screen {
                Screen::Connections => {
                    let (route, ev) = self.connections.handle(other, cx);
                    if let Some(ConnEvent::Connected(i)) = ev {
                        self.connect(i);
                        return Route::Changed;
                    }
                    route
                }
                Screen::Workbench => self
                    .workbench
                    .as_mut()
                    .map(|wb| wb.handle(other, cx, &self.history))
                    .unwrap_or(Route::Ignored),
            },
        }
    }

    fn handle_chords(&mut self, key: &KeyEvent, cx: &mut PageCtx<'_>) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T')) {
            if let Some(wb) = self.workbench.as_mut() {
                wb.new_query("");
                cx.status("New query");
            }
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            if let Some(wb) = self.workbench.as_mut() {
                wb.open_history();
            }
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O')) {
            self.open_switcher();
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            self.run_active(false, None, cx);
            return true;
        }
        if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('?')) {
            self.overlay = Overlay::Help;
            return true;
        }
        false
    }

    fn open_switcher(&mut self) {
        let Some(wb) = self.workbench.as_ref() else {
            return;
        };
        let open: Vec<(usize, String)> = wb
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.label()))
            .collect();
        let idx = SwitcherIndex::build(&wb.catalog, &wb.connection.name, &open, &self.history);
        self.switcher_items = idx.query("");
        self.switcher_q = TextInputState::new("").with_allow_empty(true);
        self.switcher_q.set_editing(true);
        self.switcher_list = ListState::new(Some(0));
        self.overlay = Overlay::Switcher;
    }

    fn run_active(&mut self, all: bool, explain: Option<bool>, cx: &mut PageCtx<'_>) {
        let Some(w) = self.workbench.as_mut() else {
            cx.status("Open a query tab to run SQL (Ctrl+T)");
            return;
        };
        let level = w.connection.safe_mode;
        let tab_index = w.active;
        let Some(q) = w.active_query_mut() else {
            cx.status("Open a query tab to run SQL (Ctrl+T)");
            return;
        };
        if q.is_running() {
            cx.status("Already running");
            return;
        }
        let statements = q.statements_to_run(all);
        if statements.is_empty() {
            cx.status("Nothing to run");
            return;
        }
        let mut worst: Option<(Decision, sql::Statement)> = None;
        for (text, _) in &statements {
            let Ok(mut stmt) = sql::parse(text) else {
                continue;
            };
            if let Some(analyze) = explain {
                stmt = sql::Statement::Explain {
                    analyze,
                    inner: Box::new(stmt),
                };
            }
            let d = sql::gate(level, &stmt);
            let rank = |d: &Decision| match d {
                Decision::Run => 0,
                Decision::Confirm { deliberate: false } => 1,
                Decision::Confirm { deliberate: true } => 2,
                Decision::Deny => 3,
            };
            if worst.as_ref().is_none_or(|(wd, _)| rank(&d) > rank(wd)) {
                worst = Some((d, stmt));
            }
        }
        let Some((decision, stmt)) = worst else {
            q.start(statements, explain);
            return;
        };
        match decision {
            Decision::Run => q.start(statements, explain),
            Decision::Deny => {
                cx.status("Cannot execute write queries: TablePro's Safe Mode is set to read-only for this connection");
            }
            Decision::Confirm { deliberate } => {
                let table = w.catalog.find(None, stmt.target().unwrap_or(""));
                let risk = sql::assess(&stmt, table);
                w.pending_run = Some((tab_index, statements, all, explain));
                let title = if sql::is_dangerous(&stmt) {
                    "This query may permanently modify or delete data"
                } else {
                    "Execute write query?"
                };
                self.overlay = Overlay::Confirm {
                    title: title.into(),
                    body: format!("{} — {}", risk.action, risk.scope),
                    ok: if deliberate {
                        "Type name to confirm".into()
                    } else {
                        "Run".into()
                    },
                    _deliberate: deliberate,
                };
            }
        }
    }

    fn handle_overlay(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key)
                if key.kind != KeyEventKind::Release && key.code == KeyCode::Esc =>
            {
                self.overlay = Overlay::None;
                Route::Changed
            }
            PageEvent::Click { id, .. } if *id == HELP_CLOSE || *id == CONFIRM_CANCEL => {
                self.overlay = Overlay::None;
                Route::Changed
            }
            PageEvent::Click { id, .. }
                if matches!(self.overlay, Overlay::Confirm { .. }) && *id == CONFIRM_OK =>
            {
                self.confirm_run(cx);
                Route::Changed
            }
            PageEvent::Key(key)
                if matches!(self.overlay, Overlay::Confirm { .. })
                    && key.kind != KeyEventKind::Release
                    && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) =>
            {
                self.confirm_run(cx);
                Route::Changed
            }
            PageEvent::Key(key) if matches!(self.overlay, Overlay::Switcher) => {
                if key.kind == KeyEventKind::Release {
                    return Route::Consumed;
                }
                if key.code == KeyCode::Enter {
                    self.apply_switcher(cx);
                    return Route::Changed;
                }
                if *cx.focus == Some(SWITCHER_LIST) {
                    let n = self.switcher_items.len();
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(&i) = self.switcher_list.selected() {
                                self.switcher_list
                                    .select(Some((i + 1).min(n.saturating_sub(1))));
                            }
                            return Route::Changed;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(&i) = self.switcher_list.selected() {
                                self.switcher_list.select(Some(i.saturating_sub(1)));
                            }
                            return Route::Changed;
                        }
                        _ => {}
                    }
                }
                let o = self.switcher_q.handle_key(*key);
                if !matches!(o, termrock::widgets::TextInputOutcome::Ignored)
                    && let Some(wb) = self.workbench.as_ref()
                {
                    let open: Vec<(usize, String)> = wb
                        .tabs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| (i, t.label()))
                        .collect();
                    let idx = SwitcherIndex::build(
                        &wb.catalog,
                        &wb.connection.name,
                        &open,
                        &self.history,
                    );
                    self.switcher_items = idx.query(self.switcher_q.value());
                }
                Route::Changed
            }
            PageEvent::Key(key)
                if matches!(self.overlay, Overlay::Help)
                    && key.kind != KeyEventKind::Release
                    && matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('q')
                    ) =>
            {
                self.overlay = Overlay::None;
                Route::Changed
            }
            _ => Route::Consumed,
        }
    }

    fn confirm_run(&mut self, cx: &mut PageCtx<'_>) {
        self.overlay = Overlay::None;
        let Some(w) = self.workbench.as_mut() else {
            return;
        };
        let Some((tab, stmts, _, explain)) = w.pending_run.take() else {
            return;
        };
        w.active = tab;
        if let Some(q) = w.active_query_mut() {
            q.start(stmts, explain);
            cx.status("Running…");
        }
    }

    fn apply_switcher(&mut self, cx: &mut PageCtx<'_>) {
        let Some(&i) = self.switcher_list.selected() else {
            self.overlay = Overlay::None;
            return;
        };
        let Some(item) = self.switcher_items.get(i).cloned() else {
            self.overlay = Overlay::None;
            return;
        };
        self.overlay = Overlay::None;
        let Some(wb) = self.workbench.as_mut() else {
            return;
        };
        match item.target {
            SwitchTarget::Table { schema, name } | SwitchTarget::View { schema, name } => {
                wb.open_table(&schema, &name);
                cx.status(format!("Opened {schema}.{name}"));
            }
            SwitchTarget::OpenTab(i) => {
                if i < wb.tabs.len() {
                    wb.active = i;
                }
            }
            SwitchTarget::RecentQuery(id) => {
                if let Some(e) = self.history.entries.iter().find(|e| e.id == id) {
                    let sql = e.sql.clone();
                    wb.new_query(&sql);
                }
            }
            SwitchTarget::Schema(s) => {
                wb.schema = s;
            }
            SwitchTarget::Database(_) => {}
        }
    }

    #[must_use]
    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if !matches!(self.overlay, Overlay::None) {
            return vec![("Esc", "Close"), ("Enter", "Confirm")];
        }
        match self.screen {
            Screen::Connections => self.connections.hints(focus),
            Screen::Workbench => self
                .workbench
                .as_ref()
                .map(|w| w.hints(focus))
                .unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn editing(&self) -> bool {
        match self.screen {
            Screen::Connections => self.connections.is_editing(),
            Screen::Workbench => self.workbench.as_ref().is_some_and(Workbench::is_editing),
        }
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<std::time::Instant> {
        if self.animating() || self.status.is_some() {
            Some(std::time::Instant::now() + Duration::from_millis(80))
        } else {
            None
        }
    }

    /// Standalone frame.
    pub fn render(&mut self, frame: &mut Frame<'_>, tick: FrameTick) {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(self.elapsed_ms);
        let area = frame.area();
        self.size = (area.width, area.height);
        self.host.scene.begin_frame();
        self.host.scene.ensure_root(root_layer());
        if !matches!(self.overlay, Overlay::None) {
            self.host.scene.push_layer(dialog_layer());
        }
        self.host.scroll_hits.clear();
        let mut scene = std::mem::take(&mut self.host.scene);
        let mut scroll_hits = std::mem::take(&mut self.host.scroll_hits);
        let theme = self.theme;
        let system = self.system.clone();
        let interaction = self.interaction();
        let cursor;
        {
            let buf = frame.buffer_mut();
            let mut ctx = RenderCtx {
                theme: &theme,
                system: &system,
                interaction,
                scene: &mut scene,
                layer: if matches!(self.overlay, Overlay::None) {
                    LayerId::Root
                } else {
                    LayerId::Dialog
                },
                cursor: None,
                inert: false,
                scroll_hits: &mut scroll_hits,
            };
            self.draw_standalone(area, buf, &mut ctx);
            cursor = ctx.cursor;
        }
        self.host.scene = scene;
        self.host.scroll_hits = scroll_hits;
        self.host.scene.reconcile();
        if let Some(id) = self.host.focus {
            let _ = self.host.scene.focus(id);
        }
        self.last_cursor = cursor;
        if let Some(pos) = cursor {
            frame.set_cursor_position(pos);
        }
    }

    fn draw_standalone(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        self.render_surface(area, buf, ctx);
        if area.height < MIN_HEIGHT || area.width < MIN_WIDTH {
            return;
        }
        let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        let t = ctx.theme;
        fill(buf, footer, t.base());
        let hints = self.hints(ctx.interaction.focus);
        let mut x = footer.x + 1;
        for (i, (k, v)) in hints.iter().enumerate() {
            if i > 0 {
                buf.set_string(x, footer.y, " · ", t.faint());
                x += 3;
            }
            let s = format!("{k} {v}");
            buf.set_string(x, footer.y, &s, t.muted());
            x += crate::text::width(&s) as u16 + 1;
        }
        if let Some((s, _)) = &self.status {
            let w = crate::text::width(s) as u16;
            if footer.width > w + 2 {
                buf.set_string(
                    footer.right().saturating_sub(w + 1),
                    footer.y,
                    s,
                    t.secondary(),
                );
            }
        }
    }

    pub fn handle_event(&mut self, event: Event, tick: FrameTick) -> ControlFlow<()> {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(u64::MAX);
        match event {
            Event::Resize { width, height } => self.size = (width, height),
            Event::Paste(text) => self.dispatch(PageEvent::Paste(text)),
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    return ControlFlow::Continue(());
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.quit = true;
                    return ControlFlow::Break(());
                }
                self.host.hover_suppressed = true;
                if self.size.0 < MIN_WIDTH || self.size.1 < MIN_HEIGHT {
                    if matches!(key.code, KeyCode::Char('q')) {
                        self.quit = true;
                        return ControlFlow::Break(());
                    }
                    return ControlFlow::Continue(());
                }
                if key.modifiers.is_empty()
                    && matches!(key.code, KeyCode::Char('q'))
                    && !self.editing()
                    && matches!(self.overlay, Overlay::None)
                {
                    self.quit = true;
                    return ControlFlow::Break(());
                }
                if key.code == KeyCode::Tab {
                    self.focus_step(key.modifiers.contains(KeyModifiers::SHIFT));
                    return ControlFlow::Continue(());
                }
                if key.code == KeyCode::BackTab {
                    self.focus_step(true);
                    return ControlFlow::Continue(());
                }
                self.dispatch(PageEvent::Key(key));
            }
            Event::Mouse(m) => self.on_mouse(m),
            _ => {}
        }
        if self.quit {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn dispatch(&mut self, ev: PageEvent) {
        let mut focus = self.host.focus;
        let requests = {
            let mut cx = PageCtx {
                focus: &mut focus,
                requests: Vec::new(),
            };
            let _ = self.handle_surface(&ev, &mut cx);
            cx.requests
        };
        self.host.focus = focus;
        self.apply_requests(requests);
    }

    fn apply_requests(&mut self, reqs: Vec<crate::page::Request>) {
        for r in reqs {
            match r {
                crate::page::Request::Status(s) => self.set_status(s),
                crate::page::Request::FocusNext => self.focus_step(false),
                crate::page::Request::FocusPrev => self.focus_step(true),
            }
        }
    }

    fn focus_step(&mut self, prev: bool) {
        let order: Vec<WidgetId> = self.host.scene.focus_order().into_iter().copied().collect();
        if order.is_empty() {
            return;
        }
        self.host.focus = Some(if prev {
            match self
                .host
                .focus
                .and_then(|f| order.iter().position(|&id| id == f))
            {
                Some(0) | None => *order.last().unwrap(),
                Some(i) => order[i - 1],
            }
        } else {
            match self
                .host
                .focus
                .and_then(|f| order.iter().position(|&id| id == f))
            {
                Some(i) => order[(i + 1) % order.len()],
                None => order[0],
            }
        });
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Moved => {
                self.host.hover_suppressed = false;
                self.host.hover = self.host.scene.hit_test(m.position).map(|e| e.id);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = self.host.scene.hit_test(m.position).map(|e| e.id);
                self.host.pressed = hit;
                self.host.hover = hit;
                if let Some(id) = hit
                    && self.host.scene.get(&id).is_some_and(|e| e.focusable)
                {
                    self.host.focus = Some(id);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let hit = self.host.scene.hit_test(m.position).map(|e| e.id);
                let pressed = self.host.pressed.take();
                if let (Some(id), Some(p)) = (hit, pressed)
                    && id == p
                {
                    self.dispatch(PageEvent::Click {
                        id,
                        pos: m.position,
                    });
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta = if matches!(m.kind, MouseEventKind::ScrollUp) {
                    -3
                } else {
                    3
                };
                let id = self
                    .host
                    .scroll_hits
                    .iter()
                    .rev()
                    .find(|(_, r)| r.contains(m.position))
                    .map(|(id, _)| *id)
                    .or_else(|| self.host.scene.hit_test(m.position).map(|e| e.id));
                if let Some(id) = id {
                    self.dispatch(PageEvent::Wheel { id, delta });
                }
            }
            MouseEventKind::Drag(_) => {
                if let Some(pressed) = self.host.pressed {
                    self.dispatch(PageEvent::Drag {
                        pressed,
                        pos: m.position,
                    });
                }
            }
            _ => {}
        }
    }

    pub fn on_tick(&mut self, tick: FrameTick) {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(u64::MAX);
        if self.animating() {
            self.tick = self.tick.wrapping_add(1);
        }
        self.dispatch(PageEvent::Tick);
    }
}

fn root_layer() -> InteractionLayer<LayerId, WidgetId> {
    InteractionLayer {
        id: LayerId::Root,
        kind: LayerKind::Root,
        owns_input: true,
        esc: LayerDismissPolicy::Ignore,
        outside: LayerDismissPolicy::Ignore,
        focus_return: None,
    }
}

fn dialog_layer() -> InteractionLayer<LayerId, WidgetId> {
    InteractionLayer {
        id: LayerId::Dialog,
        kind: LayerKind::Card,
        owns_input: true,
        esc: LayerDismissPolicy::Dismissible,
        outside: LayerDismissPolicy::Dismissible,
        focus_return: None,
    }
}
