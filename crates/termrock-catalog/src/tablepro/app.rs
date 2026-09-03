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
use termrock::style::{ColorCapability, DesignSystem, JunieTheme, Role};
use termrock::widgets::{
    ActivationOutcome, ButtonState, ButtonVariant, LineSegment, List, ListRow, ListState, Select,
    SelectOption, SelectOutcome, SelectRecipe, SelectState, TextInput, TextInputOutcome,
    TextInputState, paint_line_segments,
};

use super::connections::{ConnEvent, ConnectionsScreen};
use super::db::{Catalog, ColType, Environment, SafeMode, connections};
use super::model::{History, SwitchItem, SwitchTarget, SwitcherIndex};
use super::paint;
use super::sql::{self, Decision};
use super::tabs::{Filter, FilterOp, TABLE_GRID, TABLE_MODE};
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
const CONFIRM_ACK: WidgetId = WidgetId::of("dialog.confirm.ack");
const FILTER_EDITOR: WidgetId = WidgetId::of("dialog.filter");
const FILTER_COLUMN: WidgetId = FILTER_EDITOR.sub("column");
const FILTER_OPERATOR: WidgetId = FILTER_EDITOR.sub("operator");
const FILTER_VALUE: WidgetId = FILTER_EDITOR.sub("value");
const FILTER_APPLY: WidgetId = FILTER_EDITOR.sub("apply");
const FILTER_CANCEL: WidgetId = FILTER_EDITOR.sub("cancel");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connections,
    Workbench,
}

struct ConfirmOverlay {
    kind: ConfirmAction,
    title: String,
    action: String,
    target: String,
    scope: String,
    risk: String,
    reversible: String,
    safe_mode: String,
    sql: Vec<String>,
    token: Option<String>,
    dangerous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmAction {
    RunQuery,
    Quit,
    CloseTab(usize),
    DeleteConnection(usize),
}

struct FilterEditor {
    index: Option<usize>,
    columns: Vec<(String, ColType)>,
    column: SelectState<usize>,
    operator: SelectState<usize>,
    value: TextInputState,
    apply: ButtonState,
    cancel: ButtonState,
    return_focus: Option<WidgetId>,
}

enum Overlay {
    None,
    Help,
    Confirm(Box<ConfirmOverlay>),
    Switcher,
    Filter(Box<FilterEditor>),
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
    switcher_scope: usize,
    switcher_return_focus: Option<WidgetId>,
    confirm_ack: TextInputState,
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
            switcher_scope: 0,
            switcher_return_focus: None,
            confirm_ack: TextInputState::new("").with_allow_empty(true),
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
        // Source TablePro starts connected sessions in the explorer so the
        // first Enter opens a table and the footer exposes tree navigation.
        self.host.focus = Some(EXPLORER);
        self.set_status(format!(
            "Connected to {}",
            self.workbench.as_ref().unwrap().connection.name
        ));
    }

    /// Replace the active query document (capture reconstruction of a live tab).
    pub fn seed_active_query(&mut self, sql: &str) {
        if let Some(q) = self
            .workbench
            .as_mut()
            .and_then(Workbench::active_query_mut)
        {
            q.set_sql(sql);
        }
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
            area.x.saturating_add(1),
            area.y.saturating_add(2),
            area.width.saturating_sub(2),
            area.height.saturating_sub(4),
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
        let bg = ctx.theme.canvas;
        let cap = format!(
            "{} · {}×{}",
            self.theme.level.id(),
            self.size.0,
            self.size.1
        );
        let n_saved = format!("{} saved", self.connections.connections.len());
        let mut conn_name = String::new();
        let mut env_s = String::new();
        let mut scope = String::new();
        let mut level_s = String::new();
        let mut running_s = String::new();
        let mut pending_s = String::new();
        let mut env_role = Role::Text;
        let mut env_bold = false;
        let mut level_role = Role::Text;
        let mut level_bold = false;
        match self.screen {
            Screen::Connections => {}
            Screen::Workbench => {
                if let Some(w) = &self.workbench {
                    let c = &w.connection;
                    conn_name = truncate_middle(&c.name, 18);
                    match c.environment {
                        Environment::Production => {
                            env_s = "◆ production".into();
                            env_role = Role::Text;
                            env_bold = true;
                        }
                        Environment::Staging => {
                            env_s = "◇ staging".into();
                            env_role = Role::TextSecondary;
                        }
                        Environment::Development => {
                            env_s = "development".into();
                            env_role = Role::TextMuted;
                        }
                        Environment::Local => {
                            env_s = "local".into();
                            env_role = Role::TextFaint;
                        }
                    }
                    scope = format!("{} › {}", w.catalog.database, w.schema);
                    level_s = c.safe_mode.token().to_owned();
                    let (tone, bold) = match c.safe_mode {
                        SafeMode::Silent if c.environment == Environment::Production => {
                            (Role::Warning, true)
                        }
                        SafeMode::Silent => (Role::TextFaint, false),
                        SafeMode::Alert | SafeMode::AlertFull => (Role::TextSecondary, false),
                        _ => (Role::Text, true),
                    };
                    level_role = tone;
                    level_bold = bold;
                    if w.running().is_some() {
                        let frames = termrock::style::SPINNER_BRAILLE_FRAMES;
                        let frame = frames[(self.tick as usize) % frames.len()];
                        running_s = format!("{frame} running");
                    }
                    let pending = w.pending_total();
                    if pending > 0 {
                        pending_s = format!("• {pending} pending");
                    }
                }
            }
        }
        let mut left = vec![
            LineSegment::new("▪").tone(Role::Success).priority(9),
            LineSegment::new("TablePro").bold().priority(9),
        ];
        let mut right = Vec::new();
        match self.screen {
            Screen::Connections => {
                left.push(
                    LineSegment::new("Connections")
                        .tone(Role::TextSecondary)
                        .priority(8),
                );
                left.push(LineSegment::new(&n_saved).tone(Role::TextMuted).priority(3));
            }
            Screen::Workbench => {
                if !conn_name.is_empty() {
                    left.push(LineSegment::new(&conn_name).bold().clickable().priority(9));
                    let mut env = LineSegment::new(&env_s).tone(env_role).priority(8);
                    if env_bold {
                        env = env.bold();
                    }
                    left.push(env);
                    left.push(
                        LineSegment::new(&scope)
                            .tone(Role::TextSecondary)
                            .clickable()
                            .priority(7),
                    );
                    let mut lvl = LineSegment::new(&level_s)
                        .tone(level_role)
                        .clickable()
                        .priority(8);
                    if level_bold {
                        lvl = lvl.bold();
                    }
                    left.push(lvl);
                    if !running_s.is_empty() {
                        right.push(
                            LineSegment::new(&running_s)
                                .tone(Role::TextSecondary)
                                .priority(9),
                        );
                    }
                    if !pending_s.is_empty() {
                        right.push(LineSegment::new(&pending_s).tone(Role::Warning).priority(8));
                    }
                }
            }
        }
        right.push(LineSegment::new(&cap).tone(Role::TextFaint).priority(1));
        right.push(
            LineSegment::new("? help")
                .tone(Role::TextMuted)
                .clickable()
                .priority(4),
        );
        paint_line_segments(area, buf, ctx.system, &left, &right, bg);
        ctx.clickable(
            STRIP_HELP,
            Rect::new(area.right().saturating_sub(8), area.y, 8, 1),
        );
        ctx.clickable(STRIP_CONN, area);
        ctx.clickable(STRIP_SAFE, area);
    }

    fn draw_overlay(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        if let Overlay::Filter(editor) = &mut self.overlay {
            Self::draw_filter_editor(editor, area, buf, ctx);
            return;
        }
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
            Overlay::Confirm(confirm) => {
                let ConfirmOverlay {
                    title,
                    action,
                    target,
                    scope,
                    risk,
                    reversible,
                    safe_mode,
                    sql,
                    token,
                    dangerous,
                    ..
                } = confirm.as_ref();
                let w = area.width.min(74).max(32);
                let content_width = usize::from(w.saturating_sub(8).max(1));
                let scope_lines = crate::text::wrap(scope, content_width);
                let risk_lines = crate::text::wrap(risk, content_width);
                let reversible_lines = crate::text::wrap(reversible, content_width);
                let sql_lines: Vec<String> = sql
                    .iter()
                    .flat_map(|line| crate::text::wrap(line, content_width))
                    .collect();
                let content_height = 4
                    + scope_lines.len()
                    + risk_lines.len()
                    + reversible_lines.len()
                    + sql_lines.len().max(1)
                    + if token.is_some() { 3 } else { 0 };
                let h = u16::try_from(content_height.saturating_add(5))
                    .unwrap_or(u16::MAX)
                    .min(area.height.max(1));
                let x = area.x + area.width.saturating_sub(w) / 2;
                let y = area.y + area.height.saturating_sub(h) / 2;
                let (inner, bg) =
                    layout::card(Rect::new(x, y, w, h), buf, t, Some(title), None, true);
                let mut row = inner.y;
                let mut fact = |label: &str, lines: &[String]| {
                    for (i, line) in lines.iter().enumerate() {
                        if i == 0 {
                            buf.set_string(inner.x, row, &format!("{label:<12}"), t.muted().bg(bg));
                            buf.set_string(inner.x + 12, row, line, t.secondary().bg(bg));
                        } else {
                            buf.set_string(inner.x + 12, row, line, t.secondary().bg(bg));
                        }
                        row = row.saturating_add(1);
                    }
                };
                fact("Action", std::slice::from_ref(action));
                fact("Target", std::slice::from_ref(target));
                fact("Scope", &scope_lines);
                fact("Risk", &risk_lines);
                fact("Reversible", &reversible_lines);
                fact("Safe Mode", std::slice::from_ref(safe_mode));
                row = row.saturating_add(1);
                for line in sql_lines.iter().take(4) {
                    buf.set_string(inner.x, row, line, t.primary().bg(bg));
                    row = row.saturating_add(1);
                }
                if let Some(token) = token {
                    row = row.saturating_add(1);
                    buf.set_string(
                        inner.x,
                        row,
                        &format!("Type {token} to confirm"),
                        t.secondary().bg(bg),
                    );
                    row = row.saturating_add(1);
                    let ack = Rect::new(inner.x, row, inner.width, 1);
                    self.confirm_ack
                        .set_focused(ctx.interaction.focused(CONFIRM_ACK));
                    TextInput::new("", ctx.system).placeholder(token).paint(
                        ack,
                        buf,
                        &mut self.confirm_ack,
                    );
                    ctx.control(CONFIRM_ACK, ack, false);
                }
                let ay = inner.bottom().saturating_sub(1);
                let ok_w = paint::button_width("Execute");
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
                    "Execute",
                    if *dangerous {
                        ButtonVariant::Destructive
                    } else {
                        ButtonVariant::Primary
                    },
                    CONFIRM_OK,
                    rects[1],
                    buf,
                    ctx,
                    &mut self.confirm_ok,
                    token
                        .as_ref()
                        .is_some_and(|required| self.confirm_ack.value() != required),
                    bg,
                );
            }
            Overlay::Switcher => {
                let w = area.width.min(60).max(36);
                let h = area.height.min(18).max(10);
                let x = area.x + area.width.saturating_sub(w) / 2;
                let y = area.y + area.height.saturating_sub(h) / 2;
                let (inner, bg) = layout::card(
                    Rect::new(x, y, w, h),
                    buf,
                    t,
                    Some("Open quickly"),
                    None,
                    true,
                );
                let scope = format!("{} · Tab scope", self.switcher_scope_name());
                let scope_x = inner
                    .right()
                    .saturating_sub(u16::try_from(scope.len()).unwrap_or(u16::MAX));
                buf.set_string(scope_x, inner.y, &scope, t.muted().bg(bg));
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
            Overlay::Filter(_) => unreachable!("filter overlay handled above"),
        }
    }

    fn draw_filter_editor(
        editor: &mut FilterEditor,
        screen: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx<'_>,
    ) {
        let width = screen.width.saturating_sub(4).min(68).max(40);
        let height = screen.height.saturating_sub(2).min(15).max(11);
        let x = screen.x + screen.width.saturating_sub(width) / 2;
        let y = screen.y + screen.height.saturating_sub(height) / 2;
        let (inner, bg) = layout::card(
            Rect::new(x, y, width, height),
            buf,
            ctx.theme,
            Some(if editor.index.is_some() {
                "Edit filter"
            } else {
                "Add filter"
            }),
            None,
            true,
        );
        let gap = 2;
        let left_width = inner.width.saturating_sub(gap) / 2;
        let left = Rect::new(inner.x, inner.y, left_width, 2);
        let right = Rect::new(
            inner.x.saturating_add(left_width).saturating_add(gap),
            inner.y,
            inner.width.saturating_sub(left_width).saturating_sub(gap),
            2,
        );
        let columns = Self::filter_column_options(editor);
        let operators = Self::filter_operator_options(editor);
        editor
            .column
            .set_focused(ctx.interaction.focused(FILTER_COLUMN));
        Select::new(&columns, ctx.system).label("Column").paint(
            left,
            Rect::default(),
            buf,
            &mut editor.column,
        );
        ctx.control(FILTER_COLUMN, left, false);
        editor
            .operator
            .set_focused(ctx.interaction.focused(FILTER_OPERATOR));
        Select::new(&operators, ctx.system).label("Operator").paint(
            right,
            Rect::default(),
            buf,
            &mut editor.operator,
        );
        ctx.control(FILTER_OPERATOR, right, false);

        let selected_column = editor.column.value().copied().unwrap_or(0);
        let selected_ops = FilterOp::ordered_for(editor.columns[selected_column].1);
        let selected_op_index = editor
            .operator
            .value()
            .copied()
            .unwrap_or(0)
            .min(selected_ops.len().saturating_sub(1));
        let selected_op = selected_ops[selected_op_index];
        let value_area = Rect::new(inner.x, inner.y.saturating_add(3), inner.width, 2);
        if selected_op.needs_value() {
            editor
                .value
                .set_focused(ctx.interaction.focused(FILTER_VALUE));
            TextInput::new("Value", ctx.system)
                .placeholder("value")
                .paint(value_area, buf, &mut editor.value);
            ctx.control(FILTER_VALUE, value_area, false);
        } else {
            buf.set_string(
                inner.x,
                value_area.y,
                "Value not required for this operator",
                ctx.theme.muted().bg(bg),
            );
        }
        let preview = Filter {
            column: editor.columns[selected_column].0.clone(),
            op: selected_op,
            value: editor.value.trimmed_value().to_owned(),
            enabled: true,
        };
        buf.set_string(
            inner.x,
            inner.y.saturating_add(6),
            &format!("WHERE {}", preview.to_sql()),
            ctx.theme.secondary().bg(bg),
        );
        let footer = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        let cancel_width = paint::button_width("Cancel");
        let apply_label = if editor.index.is_some() {
            "Update filter"
        } else {
            "Add filter"
        };
        let apply_width = paint::button_width(apply_label);
        let buttons = layout::row_layout_right(footer, &[cancel_width, apply_width], 2);
        paint::button(
            "Cancel",
            ButtonVariant::Quiet,
            FILTER_CANCEL,
            buttons[0],
            buf,
            ctx,
            &mut editor.cancel,
            false,
            bg,
        );
        paint::button(
            apply_label,
            ButtonVariant::Primary,
            FILTER_APPLY,
            buttons[1],
            buf,
            ctx,
            &mut editor.apply,
            selected_op.needs_value() && editor.value.trimmed_value().is_empty(),
            bg,
        );
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
                    Screen::Connections => self.handle_connections_event(&PageEvent::Key(*key), cx),
                    Screen::Workbench => self.handle_workbench_event(&PageEvent::Key(*key), cx),
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
                Screen::Connections => self.handle_connections_event(other, cx),
                Screen::Workbench => self.handle_workbench_event(other, cx),
            },
        }
    }

    fn handle_connections_event(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        let (route, connection_event, delete_request) = {
            let (route, connection_event) = self.connections.handle(ev, cx);
            let delete_request = self.connections.take_delete_request();
            (route, connection_event, delete_request)
        };
        if let Some(index) = delete_request {
            self.open_delete_connection_confirmation(index);
        }
        if let Some(ConnEvent::Connected(index)) = connection_event {
            self.connect(index);
            return Route::Changed;
        }
        route
    }

    fn handle_workbench_event(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        let (route, close_request) = {
            let Some(wb) = self.workbench.as_mut() else {
                return Route::Ignored;
            };
            let route = wb.handle(ev, cx, &self.history, &self.system);
            let close_request = wb.take_close_request();
            (route, close_request)
        };
        if let Some(index) = close_request {
            self.open_close_tab_confirmation(index);
        }
        route
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
        if ctrl
            && matches!(key.code, KeyCode::Char('w') | KeyCode::Char('W'))
            && self.screen == Screen::Workbench
            && !self.editing()
        {
            let close_request = self.workbench.as_mut().and_then(|wb| {
                let index = wb.active;
                wb.request_close_tab(index).then_some(index)
            });
            if let Some(index) = close_request {
                self.open_close_tab_confirmation(index);
            } else if let Some(wb) = self.workbench.as_ref() {
                cx.set_focus(wb.primary_focus().unwrap_or(EXPLORER));
            }
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O')) {
            self.open_switcher();
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D')) {
            let Some(wb) = self.workbench.as_mut() else {
                return false;
            };
            let Some(WorkTab::Table(tab)) = wb.tabs.get_mut(wb.active) else {
                return false;
            };
            let structure = tab.mode.selected != Some(1);
            tab.mode.set_selected(Some(u8::from(structure)));
            cx.set_focus(if structure { TABLE_MODE } else { TABLE_GRID });
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            self.run_active(false, None, cx);
            return true;
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'))
        {
            self.run_active(false, Some(true), cx);
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X')) {
            self.run_active(false, Some(false), cx);
            return true;
        }
        if key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Char('0'))
            && self.screen == Screen::Workbench
        {
            if let Some(wb) = self.workbench.as_mut() {
                wb.explorer_visible = true;
                wb.maximized = false;
            }
            cx.set_focus(EXPLORER);
            return true;
        }
        if ctrl && matches!(key.code, KeyCode::Char('b') | KeyCode::Char('B')) {
            if let Some(wb) = self.workbench.as_mut() {
                wb.explorer_visible = !wb.explorer_visible;
                if wb.explorer_visible {
                    cx.set_focus(EXPLORER);
                } else if let Some(id) = wb.primary_focus() {
                    cx.set_focus(id);
                }
            }
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
        self.switcher_return_focus = self.host.focus;
        self.switcher_scope = 0;
        self.switcher_q = TextInputState::new("").with_allow_empty(true);
        self.switcher_q.set_editing(true);
        self.switcher_list = ListState::new(Some(0));
        self.host.focus = Some(SWITCHER_INPUT);
        self.overlay = Overlay::Switcher;
        self.refresh_switcher();
    }

    /// Open the shared table filter editor requested by either host.
    pub(crate) fn open_table_filter(
        &mut self,
        index: Option<usize>,
        requested_column: Option<usize>,
        prefill: Option<(String, bool)>,
    ) {
        let Some(wb) = self.workbench.as_ref() else {
            return;
        };
        let Some(WorkTab::Table(table)) = wb.active_tab() else {
            self.set_status("Filters apply to table tabs".into());
            return;
        };
        if table.grid.columns.is_empty() {
            return;
        }

        let columns = table.grid.columns.clone();
        let existing = index.and_then(|i| table.filters.get(i)).cloned();
        let column = existing
            .as_ref()
            .and_then(|filter| columns.iter().position(|(name, _)| name == &filter.column))
            .or(requested_column)
            .unwrap_or(table.grid.cursor_col)
            .min(columns.len().saturating_sub(1));
        let (operator, value) = match existing.as_ref() {
            Some(filter) => (filter.op, filter.value.clone()),
            None => match prefill {
                Some((value, true)) => (FilterOp::IsNull, value),
                Some((value, false)) => (FilterOp::Eq, value),
                None => (FilterOp::Eq, String::new()),
            },
        };
        let operators = FilterOp::ordered_for(columns[column].1);
        let operator = operators.iter().position(|op| *op == operator).unwrap_or(0);
        let value_empty = value.is_empty();
        let mut column_state = SelectState::new()
            .with_value(column)
            .with_recipe(SelectRecipe::Form);
        column_state.set_focused(false);
        let mut operator_state = SelectState::new()
            .with_value(operator)
            .with_recipe(SelectRecipe::Form);
        operator_state.set_focused(false);
        let return_focus = self.host.focus;
        self.overlay = Overlay::Filter(Box::new(FilterEditor {
            index,
            columns,
            column: column_state,
            operator: operator_state,
            value: TextInputState::new(value).with_allow_empty(true),
            apply: ButtonState::new(),
            cancel: ButtonState::new(),
            return_focus,
        }));
        // The editor focus is modal focus; retain the grid only as the return
        // target after the modal closes.
        if let Overlay::Filter(editor) = &mut self.overlay {
            editor.return_focus = return_focus;
        }
        self.host.focus = Some(if value_empty {
            FILTER_VALUE
        } else {
            FILTER_APPLY
        });
    }

    fn filter_column_options(editor: &FilterEditor) -> Vec<SelectOption<usize>> {
        editor
            .columns
            .iter()
            .enumerate()
            .map(|(i, (name, _))| SelectOption::option(i, name.clone()))
            .collect()
    }

    fn filter_operator_options(editor: &FilterEditor) -> Vec<SelectOption<usize>> {
        let column = editor.column.value().copied().unwrap_or(0);
        FilterOp::ordered_for(editor.columns[column].1)
            .into_iter()
            .enumerate()
            .map(|(i, op)| SelectOption::option(i, op.label()))
            .collect()
    }

    fn reset_filter_operator(editor: &mut FilterEditor) {
        let column = editor.column.value().copied().unwrap_or(0);
        editor.operator = SelectState::new()
            .with_value(0)
            .with_recipe(SelectRecipe::Form);
        if column >= editor.columns.len() {
            editor.column.set_value(Some(0));
        }
    }

    fn close_table_filter(&mut self) {
        let return_focus = match &self.overlay {
            Overlay::Filter(editor) => editor.return_focus,
            _ => None,
        };
        self.overlay = Overlay::None;
        self.host.focus = return_focus
            .or_else(|| self.workbench.as_ref().and_then(Workbench::primary_focus))
            .or(Some(EXPLORER));
    }

    fn apply_table_filter(&mut self, cx: &mut PageCtx<'_>) -> Route {
        let (index, filter) = {
            let Overlay::Filter(editor) = &mut self.overlay else {
                return Route::Ignored;
            };
            if editor.value.is_editing() {
                editor.value.commit();
            }
            let column = editor.column.value().copied().unwrap_or(0);
            let operators = FilterOp::ordered_for(editor.columns[column].1);
            let op_index = editor
                .operator
                .value()
                .copied()
                .unwrap_or(0)
                .min(operators.len().saturating_sub(1));
            let op = operators[op_index];
            if op.needs_value() && editor.value.trimmed_value().is_empty() {
                cx.set_focus(FILTER_VALUE);
                cx.status("A value is required for this operator");
                return Route::Changed;
            }
            (
                editor.index,
                Filter {
                    column: editor.columns[column].0.clone(),
                    op,
                    value: editor.value.trimmed_value().to_owned(),
                    enabled: true,
                },
            )
        };

        let Some(wb) = self.workbench.as_mut() else {
            self.close_table_filter();
            return Route::Changed;
        };
        let cat = wb.catalog.clone();
        let Some(WorkTab::Table(table)) = wb.tabs.get_mut(wb.active) else {
            self.close_table_filter();
            return Route::Changed;
        };
        if !table.grid.pending.is_empty() {
            cx.status("Cannot change filters while pending changes exist");
            return Route::Changed;
        }
        match index {
            Some(i) if i < table.filters.len() => table.filters[i] = filter,
            _ => table.filters.push(filter),
        }
        table.load(&cat);
        let count = table.active_filter_count();
        self.close_table_filter();
        cx.set_focus(TABLE_GRID);
        cx.status(format!(
            "{count} filter{} applied",
            if count == 1 { "" } else { "s" }
        ));
        Route::Changed
    }

    fn handle_table_filter(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        let bounds = Rect::new(0, 0, self.size.0, self.size.1);
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::Esc {
                    let handled_by_field = match &mut self.overlay {
                        Overlay::Filter(editor) if editor.value.is_editing() => {
                            editor.value.cancel_edit();
                            true
                        }
                        Overlay::Filter(editor)
                            if editor.column.is_open() || editor.operator.is_open() =>
                        {
                            let options = Self::filter_column_options(editor);
                            let _ = editor.column.handle_key(*key, &options, bounds);
                            let options = Self::filter_operator_options(editor);
                            let _ = editor.operator.handle_key(*key, &options, bounds);
                            true
                        }
                        _ => false,
                    };
                    if handled_by_field {
                        return Route::Changed;
                    }
                    self.close_table_filter();
                    return Route::Changed;
                }
                let focused = cx.focus_id();
                if matches!(focused, Some(FILTER_COLUMN) | Some(FILTER_OPERATOR))
                    && (self.overlay_is_select_open())
                {
                    // Open selects own arrows and Enter; traversal remains
                    // host-owned once the list is closed.
                } else if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    if key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT) {
                        cx.focus_prev();
                    } else {
                        cx.focus_next();
                    }
                    return Route::Changed;
                }

                match focused {
                    Some(FILTER_COLUMN) => {
                        let outcome = if let Overlay::Filter(editor) = &mut self.overlay {
                            let options = Self::filter_column_options(editor);
                            let outcome = editor.column.handle_key(*key, &options, bounds);
                            if matches!(outcome, SelectOutcome::ValueChanged { .. }) {
                                Self::reset_filter_operator(editor);
                            }
                            outcome
                        } else {
                            SelectOutcome::Ignored
                        };
                        return if matches!(outcome, SelectOutcome::Ignored) {
                            Route::Consumed
                        } else {
                            Route::Changed
                        };
                    }
                    Some(FILTER_OPERATOR) => {
                        let outcome = if let Overlay::Filter(editor) = &mut self.overlay {
                            let options = Self::filter_operator_options(editor);
                            editor.operator.handle_key(*key, &options, bounds)
                        } else {
                            SelectOutcome::Ignored
                        };
                        return if matches!(outcome, SelectOutcome::Ignored) {
                            Route::Consumed
                        } else {
                            Route::Changed
                        };
                    }
                    Some(FILTER_VALUE) => {
                        let outcome = if let Overlay::Filter(editor) = &mut self.overlay {
                            editor.value.handle_key(*key)
                        } else {
                            TextInputOutcome::Ignored
                        };
                        if matches!(outcome, TextInputOutcome::Submitted(_)) {
                            return self.apply_table_filter(cx);
                        }
                        return if matches!(outcome, TextInputOutcome::Ignored) {
                            Route::Consumed
                        } else {
                            Route::Changed
                        };
                    }
                    Some(FILTER_APPLY) => {
                        let outcome = if let Overlay::Filter(editor) = &mut self.overlay {
                            editor.apply.handle_key(*key)
                        } else {
                            ActivationOutcome::Ignored
                        };
                        if matches!(outcome, ActivationOutcome::Activated) {
                            return self.apply_table_filter(cx);
                        }
                        return Route::Changed;
                    }
                    Some(FILTER_CANCEL) => {
                        let outcome = if let Overlay::Filter(editor) = &mut self.overlay {
                            editor.cancel.handle_key(*key)
                        } else {
                            ActivationOutcome::Ignored
                        };
                        if matches!(outcome, ActivationOutcome::Activated) {
                            self.close_table_filter();
                        }
                        return Route::Changed;
                    }
                    _ => return Route::Consumed,
                }
            }
            PageEvent::Paste(text) if cx.focus_id() == Some(FILTER_VALUE) => {
                if let Overlay::Filter(editor) = &mut self.overlay {
                    editor.value.begin_edit();
                    return if matches!(editor.value.insert_str(text), TextInputOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        Route::Changed
                    };
                }
                Route::Consumed
            }
            PageEvent::Click { id, pos } => {
                if *id == FILTER_APPLY {
                    return self.apply_table_filter(cx);
                }
                if *id == FILTER_CANCEL {
                    self.close_table_filter();
                    return Route::Changed;
                }
                if *id == FILTER_VALUE {
                    cx.set_focus(FILTER_VALUE);
                    if let Overlay::Filter(editor) = &mut self.overlay {
                        editor.value.begin_edit();
                    }
                    return Route::Changed;
                }
                if *id == FILTER_COLUMN || *id == FILTER_OPERATOR {
                    cx.set_focus(*id);
                    if let Overlay::Filter(editor) = &mut self.overlay {
                        let options = if *id == FILTER_COLUMN {
                            Self::filter_column_options(editor)
                        } else {
                            Self::filter_operator_options(editor)
                        };
                        let event = MouseEvent {
                            kind: MouseEventKind::Down(MouseButton::Left),
                            position: *pos,
                            modifiers: KeyModifiers::NONE,
                        };
                        if *id == FILTER_COLUMN {
                            let outcome = editor.column.handle_mouse(event, &options, bounds);
                            if matches!(outcome, SelectOutcome::ValueChanged { .. }) {
                                Self::reset_filter_operator(editor);
                            }
                        } else {
                            let _ = editor.operator.handle_mouse(event, &options, bounds);
                        }
                    }
                    return Route::Changed;
                }
                Route::Consumed
            }
            _ => Route::Consumed,
        }
    }

    fn overlay_is_select_open(&self) -> bool {
        matches!(&self.overlay, Overlay::Filter(editor) if editor.column.is_open() || editor.operator.is_open())
    }

    fn switcher_scope_name(&self) -> &'static str {
        match self.switcher_scope {
            1 => "Tables",
            2 => "Schemas",
            3 => "Queries",
            _ => "All",
        }
    }

    fn refresh_switcher(&mut self) {
        let Some(wb) = self.workbench.as_ref() else {
            self.switcher_items.clear();
            self.switcher_list.select(None);
            return;
        };
        let open: Vec<(usize, String)> = wb
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.label()))
            .collect();
        let idx = SwitcherIndex::build(&wb.catalog, &wb.connection.name, &open, &self.history);
        let mut items = idx.query(self.switcher_q.value());
        items.retain(|item| match self.switcher_scope {
            1 => matches!(item.group, "Tables" | "Views"),
            2 => matches!(item.group, "Schemas" | "Databases"),
            3 => item.group == "Recent queries",
            _ => true,
        });
        self.switcher_items = items;
        self.switcher_list
            .select((!self.switcher_items.is_empty()).then_some(0));
    }

    fn close_switcher(&mut self) {
        self.overlay = Overlay::None;
        self.switcher_q.commit();
        self.host.focus = self
            .switcher_return_focus
            .take()
            .or_else(|| self.workbench.as_ref().and_then(Workbench::primary_focus))
            .or(Some(EXPLORER));
    }

    fn move_switcher(&mut self, delta: isize) {
        let Some(current) = self.switcher_list.selected().copied() else {
            return;
        };
        let last = self.switcher_items.len().saturating_sub(1);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta as usize).min(last)
        };
        self.switcher_list.select(Some(next));
    }

    fn handle_switcher_key(&mut self, key: KeyEvent, cx: &mut PageCtx<'_>) -> Route {
        match key.code {
            KeyCode::Esc => {
                if !self.switcher_q.value().is_empty() {
                    self.switcher_q.clear();
                    self.refresh_switcher();
                    return Route::Changed;
                }
                self.close_switcher();
                Route::Changed
            }
            KeyCode::Enter => {
                self.apply_switcher(cx);
                Route::Changed
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.move_switcher(1);
                Route::Changed
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.move_switcher(-1);
                Route::Changed
            }
            KeyCode::Char('n') | KeyCode::Char('j')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.move_switcher(1);
                Route::Changed
            }
            KeyCode::Char('p') | KeyCode::Char('k')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.move_switcher(-1);
                Route::Changed
            }
            KeyCode::PageDown => {
                self.move_switcher(12);
                Route::Changed
            }
            KeyCode::PageUp => {
                self.move_switcher(-12);
                Route::Changed
            }
            KeyCode::Tab => {
                self.switcher_scope = (self.switcher_scope + 1) % 4;
                self.refresh_switcher();
                Route::Changed
            }
            _ => {
                let outcome = self.switcher_q.handle_key(key);
                if !matches!(outcome, termrock::widgets::TextInputOutcome::Ignored) {
                    self.refresh_switcher();
                }
                Route::Changed
            }
        }
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
                let dangerous = sql::is_dangerous(&stmt);
                w.pending_run = Some((tab_index, statements.clone(), all, explain));
                let title = if dangerous {
                    "This query may permanently modify or delete data"
                } else if risk.tier == sql::Tier::Safe {
                    "Execute query?"
                } else {
                    "Execute write query?"
                };
                let target_name = stmt.target().unwrap_or("yes").to_owned();
                let target = format!(
                    "{} · {} · {} · {}",
                    w.connection.name,
                    w.connection.environment.label(),
                    w.catalog.database,
                    target_name
                );
                let safe_mode = format!(
                    "{} · {}",
                    level.label(),
                    if deliberate {
                        "deliberate confirmation required"
                    } else {
                        "confirmation required"
                    }
                );
                let sql_lines = statements
                    .iter()
                    .flat_map(|(text, _)| text.lines().map(ToOwned::to_owned))
                    .collect();
                let token = if deliberate
                    || (dangerous && w.connection.environment == Environment::Production)
                {
                    Some(target_name)
                } else {
                    None
                };
                self.confirm_ack = TextInputState::new("").with_allow_empty(true);
                self.overlay = Overlay::Confirm(Box::new(ConfirmOverlay {
                    kind: ConfirmAction::RunQuery,
                    title: title.into(),
                    action: risk.action,
                    target,
                    scope: risk.scope,
                    risk: risk.risk,
                    reversible: risk.reversible.to_owned(),
                    safe_mode,
                    sql: sql_lines,
                    token,
                    dangerous,
                }));
            }
        }
    }

    fn handle_overlay(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if matches!(self.overlay, Overlay::Filter(_)) {
            return self.handle_table_filter(ev, cx);
        }
        match ev {
            PageEvent::Key(key)
                if key.kind != KeyEventKind::Release
                    && matches!(self.overlay, Overlay::Switcher) =>
            {
                self.handle_switcher_key(*key, cx)
            }
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
                if matches!(&self.overlay, Overlay::Confirm(_)) && *id == CONFIRM_OK =>
            {
                self.confirm_action(cx);
                Route::Changed
            }
            PageEvent::Click { id, .. }
                if matches!(&self.overlay, Overlay::Confirm(confirm) if confirm.token.is_some())
                    && *id == CONFIRM_ACK =>
            {
                cx.set_focus(CONFIRM_ACK);
                self.confirm_ack.begin_edit();
                Route::Changed
            }
            PageEvent::Paste(text)
                if matches!(&self.overlay, Overlay::Confirm(confirm) if confirm.token.is_some())
                    && cx.focus_id() == Some(CONFIRM_ACK) =>
            {
                self.confirm_ack.begin_edit();
                if matches!(
                    self.confirm_ack.insert_str(text),
                    termrock::widgets::TextInputOutcome::Ignored
                ) {
                    Route::Consumed
                } else {
                    Route::Changed
                }
            }
            PageEvent::Key(key)
                if matches!(&self.overlay, Overlay::Confirm(_))
                    && key.kind != KeyEventKind::Release
                    && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) =>
            {
                if matches!(&self.overlay, Overlay::Confirm(confirm) if confirm.token.is_some())
                    && cx.focus_id() == Some(CONFIRM_ACK)
                {
                    let outcome = self.confirm_ack.handle_key(*key);
                    if matches!(outcome, termrock::widgets::TextInputOutcome::Submitted(_)) {
                        self.confirm_ack.commit();
                        cx.set_focus(CONFIRM_CANCEL);
                    }
                    return if matches!(outcome, termrock::widgets::TextInputOutcome::Ignored) {
                        Route::Consumed
                    } else {
                        Route::Changed
                    };
                }
                self.confirm_action(cx);
                Route::Changed
            }
            PageEvent::Key(key)
                if matches!(&self.overlay, Overlay::Confirm(confirm) if confirm.token.is_some())
                    && key.kind != KeyEventKind::Release
                    && cx.focus_id() == Some(CONFIRM_ACK) =>
            {
                let outcome = self.confirm_ack.handle_key(*key);
                if matches!(outcome, termrock::widgets::TextInputOutcome::Submitted(_)) {
                    self.confirm_ack.commit();
                    cx.set_focus(CONFIRM_CANCEL);
                }
                if matches!(outcome, termrock::widgets::TextInputOutcome::Ignored) {
                    Route::Consumed
                } else {
                    Route::Changed
                }
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
        let authorized = match &self.overlay {
            Overlay::Confirm(confirm) => confirm
                .token
                .as_deref()
                .is_none_or(|token| self.confirm_ack.value() == token),
            _ => false,
        };
        if !authorized {
            return;
        }
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

    fn confirm_action(&mut self, cx: &mut PageCtx<'_>) {
        let Some(kind) = (match &self.overlay {
            Overlay::Confirm(confirm) => Some(confirm.kind),
            _ => None,
        }) else {
            return;
        };
        match kind {
            ConfirmAction::RunQuery => self.confirm_run(cx),
            ConfirmAction::Quit => {
                self.overlay = Overlay::None;
                self.quit = true;
            }
            ConfirmAction::CloseTab(index) => {
                self.overlay = Overlay::None;
                if let Some(wb) = self.workbench.as_mut() {
                    wb.close_tab(index);
                    cx.set_focus(wb.primary_focus().unwrap_or(EXPLORER));
                }
            }
            ConfirmAction::DeleteConnection(index) => {
                self.overlay = Overlay::None;
                if let Some(name) = self.connections.delete_at(index) {
                    cx.status(format!("Deleted {name}"));
                }
            }
        }
    }

    fn apply_switcher(&mut self, cx: &mut PageCtx<'_>) {
        let Some(&i) = self.switcher_list.selected() else {
            self.close_switcher();
            return;
        };
        let Some(item) = self.switcher_items.get(i).cloned() else {
            self.close_switcher();
            return;
        };
        self.close_switcher();
        let Some(wb) = self.workbench.as_mut() else {
            return;
        };
        match item.target {
            SwitchTarget::Table { schema, name } | SwitchTarget::View { schema, name } => {
                wb.open_table(&schema, &name);
                self.host.focus = Some(super::tabs::TABLE_GRID);
                cx.status(format!("Opened {schema}.{name}"));
            }
            SwitchTarget::OpenTab(i) => {
                if i < wb.tabs.len() {
                    wb.active = i;
                    self.host.focus = wb.primary_focus().or(Some(EXPLORER));
                }
            }
            SwitchTarget::RecentQuery(id) => {
                if let Some(e) = self.history.entries.iter().find(|e| e.id == id) {
                    let sql = e.sql.clone();
                    wb.new_query(&sql);
                    self.host.focus = Some(super::tabs::EDITOR);
                }
            }
            SwitchTarget::Schema(s) => {
                wb.schema = s;
                self.host.focus = Some(EXPLORER);
            }
            SwitchTarget::Database(_) => {
                self.host.focus = Some(EXPLORER);
            }
        }
    }

    #[must_use]
    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if !matches!(self.overlay, Overlay::None) {
            return vec![("Esc", "Close"), ("Enter", "Confirm")];
        }
        let mut hints = match self.screen {
            Screen::Connections => self.connections.hints(focus),
            Screen::Workbench => self
                .workbench
                .as_ref()
                .map(|w| w.hints(focus))
                .unwrap_or_default(),
        };
        if !self.editing() {
            hints.push(("Tab", "Next"));
        }
        hints
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

    /// Request quit, preserving unsaved work behind a destructive confirmation.
    fn request_quit(&mut self) -> bool {
        let Some(wb) = self.workbench.as_ref() else {
            self.quit = true;
            return true;
        };
        let pending = wb.pending_total();
        let dirty_queries = wb.tabs.iter().filter(|tab| tab.dirty()).count();
        if pending == 0 && dirty_queries == 0 {
            self.quit = true;
            return true;
        }
        let mut lost = Vec::new();
        if pending > 0 {
            lost.push(format!(
                "{pending} pending row change{}",
                if pending == 1 { "" } else { "s" }
            ));
        }
        if dirty_queries > 0 {
            lost.push(format!(
                "{dirty_queries} unsaved quer{}",
                if dirty_queries == 1 { "y" } else { "ies" }
            ));
        }
        self.open_confirmation(
            ConfirmAction::Quit,
            "Quit TablePro?",
            "Quit",
            "TablePro",
            "Current work will be lost.",
            &format!("{} will be lost.", lost.join(" and ")),
            "No",
            "Not applicable",
            Vec::new(),
            true,
        );
        false
    }

    fn open_confirmation(
        &mut self,
        kind: ConfirmAction,
        title: &str,
        action: &str,
        target: &str,
        scope: &str,
        risk: &str,
        reversible: &str,
        safe_mode: &str,
        sql: Vec<String>,
        dangerous: bool,
    ) {
        self.confirm_ack = TextInputState::new("").with_allow_empty(true);
        self.overlay = Overlay::Confirm(Box::new(ConfirmOverlay {
            kind,
            title: title.to_owned(),
            action: action.to_owned(),
            target: target.to_owned(),
            scope: scope.to_owned(),
            risk: risk.to_owned(),
            reversible: reversible.to_owned(),
            safe_mode: safe_mode.to_owned(),
            sql,
            token: None,
            dangerous,
        }));
    }

    fn open_close_tab_confirmation(&mut self, index: usize) {
        let Some(wb) = self.workbench.as_ref() else {
            return;
        };
        let Some(tab) = wb.tabs.get(index) else {
            return;
        };
        let target = tab.label();
        let scope = format!("{} · {}", wb.connection.name, wb.catalog.database);
        self.open_confirmation(
            ConfirmAction::CloseTab(index),
            "Close tab with unsaved work?",
            "Close tab",
            &target,
            &scope,
            "Unsaved query text or pending row edits will be lost.",
            "No",
            "Not applicable",
            Vec::new(),
            true,
        );
    }

    fn open_delete_connection_confirmation(&mut self, index: usize) {
        let Some(connection) = self.connections.connections.get(index) else {
            return;
        };
        let target = connection.name.clone();
        let detail = format!(
            "{target} ({}@{}) will be removed from the saved connections.",
            connection.user, connection.host
        );
        self.open_confirmation(
            ConfirmAction::DeleteConnection(index),
            "Delete connection?",
            "Delete",
            &target,
            "Saved connection",
            &detail,
            "No",
            "Not applicable",
            Vec::new(),
            true,
        );
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
        let order: Vec<WidgetId> = self.host.scene.focus_order().into_iter().copied().collect();
        if self
            .host
            .focus
            .as_ref()
            .is_none_or(|id| !order.contains(id))
        {
            self.host.focus = self
                .workbench
                .as_ref()
                .and_then(Workbench::primary_focus)
                .filter(|id| order.contains(id))
                .or_else(|| order.first().copied());
        }
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
        let mut x = footer.x.saturating_add(1);
        let mut right_w = 0u16;
        if let Some((s, _)) = &self.status {
            let w = crate::text::width(s) as u16;
            if footer.width > w + 2 {
                buf.set_string(
                    footer.right().saturating_sub(w + 1),
                    footer.y,
                    s,
                    t.secondary(),
                );
                right_w = w.saturating_add(3);
            }
        }
        for (k, v) in &hints {
            let kw = crate::text::width(k) as u16;
            let w = kw
                .saturating_add(1)
                .saturating_add(crate::text::width(v) as u16)
                .saturating_add(2);
            if x.saturating_add(w).saturating_add(right_w) > footer.right() {
                break;
            }
            buf.set_string(x, footer.y, k, t.key_hint_key());
            buf.set_string(
                x.saturating_add(kw).saturating_add(1),
                footer.y,
                v,
                t.key_hint_action(),
            );
            x = x.saturating_add(w);
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
                    let cancelled = self
                        .workbench
                        .as_mut()
                        .and_then(Workbench::active_query_mut)
                        .is_some_and(|query| query.cancel());
                    if cancelled {
                        self.set_status("Query cancelled".into());
                        return ControlFlow::Continue(());
                    }
                    if self.request_quit() {
                        return ControlFlow::Break(());
                    }
                    return ControlFlow::Continue(());
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
                    if self.request_quit() {
                        return ControlFlow::Break(());
                    }
                    return ControlFlow::Continue(());
                }
                if matches!(self.overlay, Overlay::Switcher | Overlay::Filter(_))
                    && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                {
                    self.dispatch(PageEvent::Key(key));
                    return ControlFlow::Continue(());
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
                crate::page::Request::OpenTableFilter {
                    index,
                    column,
                    value,
                } => self.open_table_filter(index, column, value),
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
