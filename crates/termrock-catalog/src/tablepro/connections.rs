// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/connections.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Connection experience: grouped list, detail card, edit form, simulated connect.

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::widgets::{
    ButtonState, ButtonVariant, KeyValueList, KeyValueListState, KvEntry, KvStatus, ProgressBar,
    ProgressKind, RadioGroup, RadioOption, RadioState, Tab, Tabs, TabsState, TextInput,
    TextInputState, Tree, TreeNode, TreeOutcome, TreeState,
};

use super::db::{ConnectOutcome, Connection, Engine, Environment, SafeMode};
use super::paint;
use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("connections");
pub const TREE: WidgetId = ID.sub("tree");
const FILTER: WidgetId = ID.sub("filter");
const CONNECT: WidgetId = ID.sub("connect");
const EDIT: WidgetId = ID.sub("edit");
const DUP: WidgetId = ID.sub("dup");
const DEL: WidgetId = ID.sub("del");
const RETRY: WidgetId = ID.sub("retry");
const FORM_SAVE: WidgetId = ID.sub("form-save");
const FORM_CANCEL: WidgetId = ID.sub("form-cancel");
const FORM_CONNECT: WidgetId = ID.sub("form-saveconnect");
const FORM_NAME: WidgetId = ID.sub("form-name");
const FORM_HOST: WidgetId = ID.sub("form-host");
const FORM_TABS: WidgetId = ID.sub("form-tabs");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnState {
    Idle,
    Connecting {
        ticks: u32,
        name: String,
    },
    Failed {
        name: String,
        message: String,
        detail: String,
    },
}

pub enum ConnEvent {
    Connected(usize),
}

struct ConnForm {
    index: Option<usize>,
    tabs: TabsState<u8>,
    name: TextInputState,
    host: TextInputState,
    port: TextInputState,
    database: TextInputState,
    user: TextInputState,
    env: RadioState<u8>,
    save: ButtonState,
    cancel: ButtonState,
    save_connect: ButtonState,
}

pub struct ConnectionsScreen {
    pub connections: Vec<Connection>,
    tree: TreeState<String>,
    expanded: HashSet<String>,
    filter: TextInputState,
    pub selected: Option<usize>,
    pub state: ConnState,
    connect_btn: ButtonState,
    edit_btn: ButtonState,
    dup_btn: ButtonState,
    del_btn: ButtonState,
    retry_btn: ButtonState,
    form: Option<ConnForm>,
    kv: KeyValueListState<u8>,
}

impl ConnectionsScreen {
    #[must_use]
    pub fn new(connections: Vec<Connection>) -> Self {
        let mut expanded = HashSet::new();
        for c in &connections {
            expanded.insert(c.group.clone());
        }
        let mut filter = TextInputState::new("").with_allow_empty(true);
        filter.set_editing(false);
        let mut s = Self {
            connections,
            tree: TreeState::new(None),
            expanded,
            filter,
            selected: Some(0),
            state: ConnState::Idle,
            connect_btn: ButtonState::new(),
            edit_btn: ButtonState::new(),
            dup_btn: ButtonState::new(),
            del_btn: ButtonState::new(),
            retry_btn: ButtonState::new(),
            form: None,
            kv: KeyValueListState::new(),
        };
        if let Some(c) = s.connections.first() {
            s.tree = TreeState::new(Some(c.name.clone()));
        }
        s
    }

    #[must_use]
    pub fn selected_connection(&self) -> Option<&Connection> {
        self.selected.and_then(|i| self.connections.get(i))
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.filter.is_editing()
            || self.form.as_ref().is_some_and(|f| {
                f.name.is_editing()
                    || f.host.is_editing()
                    || f.port.is_editing()
                    || f.database.is_editing()
                    || f.user.is_editing()
            })
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        matches!(self.state, ConnState::Connecting { .. })
    }

    pub fn start_connect(&mut self, i: usize) {
        if let Some(c) = self.connections.get(i) {
            self.selected = Some(i);
            self.state = ConnState::Connecting {
                ticks: 0,
                name: c.name.clone(),
            };
        }
    }

    pub fn open_form(&mut self, index: Option<usize>) {
        let c = index.and_then(|i| self.connections.get(i));
        let env_i = c
            .map(|c| match c.environment {
                Environment::Local => 0,
                Environment::Development => 1,
                Environment::Staging => 2,
                Environment::Production => 3,
            })
            .unwrap_or(0);
        let mut tabs = TabsState::new();
        tabs.set_selected(Some(0));
        let mut name = TextInputState::new(c.map(|c| c.name.as_str()).unwrap_or(""));
        name.set_editing(false);
        let mut host = TextInputState::new(c.map(|c| c.host.as_str()).unwrap_or("localhost"))
            .with_allow_empty(true);
        host.set_editing(false);
        let mut port = TextInputState::new(
            c.map(|c| c.port.to_string())
                .unwrap_or_else(|| "5432".into()),
        )
        .with_allow_empty(true);
        port.set_editing(false);
        let mut database = TextInputState::new(c.map(|c| c.database.as_str()).unwrap_or(""))
            .with_allow_empty(true);
        database.set_editing(false);
        let mut user =
            TextInputState::new(c.map(|c| c.user.as_str()).unwrap_or("")).with_allow_empty(true);
        user.set_editing(false);
        self.form = Some(ConnForm {
            index,
            tabs,
            name,
            host,
            port,
            database,
            user,
            env: RadioState::new(Some(env_i)),
            save: ButtonState::new(),
            cancel: ButtonState::new(),
            save_connect: ButtonState::new(),
        });
    }

    pub fn tick(&mut self) -> Option<ConnEvent> {
        match &mut self.state {
            ConnState::Connecting { ticks, name } => {
                *ticks += 1;
                if *ticks >= 12 {
                    let name = name.clone();
                    let idx = self.connections.iter().position(|c| c.name == name);
                    let outcome = idx
                        .map(|i| self.connections[i].outcome)
                        .unwrap_or(ConnectOutcome::Ok);
                    match outcome {
                        ConnectOutcome::Ok => {
                            self.state = ConnState::Idle;
                            if let Some(i) = idx {
                                self.connections[i].last_used = "just now".into();
                                return Some(ConnEvent::Connected(i));
                            }
                        }
                        ConnectOutcome::AuthFailed => {
                            self.state = ConnState::Failed {
                                name,
                                message: "Authentication failed".into(),
                                detail: "FATAL: password authentication failed for user \"acme_app\" (SQLSTATE 28P01). Check the password in the keychain or use “Prompt for password”.".into(),
                            };
                        }
                        ConnectOutcome::Unreachable => {
                            self.state = ConnState::Failed {
                                name,
                                message: "Could not reach the host".into(),
                                detail: "Connection timed out after 10 s (analytics.acme.io:3306). The host may be behind a VPN or the port may be blocked.".into(),
                            };
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn groups(&self) -> Vec<String> {
        let mut groups = Vec::new();
        let q = self.filter.trimmed_value().to_ascii_lowercase();
        for c in &self.connections {
            if !q.is_empty() && !c.name.to_ascii_lowercase().contains(&q) {
                continue;
            }
            if !groups.contains(&c.group) {
                groups.push(c.group.clone());
            }
        }
        groups
    }

    fn visible(&self) -> Vec<(String, String, u16, bool, bool)> {
        let q = self.filter.trimmed_value().to_ascii_lowercase();
        let mut out = Vec::new();
        for g in self.groups() {
            let expanded = self.expanded.contains(&g);
            out.push((g.clone(), g.clone(), 0, true, expanded));
            if expanded {
                for c in &self.connections {
                    if c.group != g {
                        continue;
                    }
                    if !q.is_empty() && !c.name.to_ascii_lowercase().contains(&q) {
                        continue;
                    }
                    out.push((c.name.clone(), c.name.clone(), 1, false, false));
                }
            }
        }
        out
    }

    fn select_named(&mut self, name: &str) {
        if let Some(i) = self.connections.iter().position(|c| c.name == name) {
            self.selected = Some(i);
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        if self.form.is_some() {
            self.render_form(area, buf, ctx);
            return;
        }
        let t = ctx.theme;
        let (l, r) = layout::columns(area, 28, 2);
        let vis = self.visible();
        let nodes: Vec<TreeNode<'_, String>> = vis
            .iter()
            .map(|(id, label, depth, branch, expanded)| {
                let mut n = TreeNode::new(id.clone(), Line::from(label.as_str()), *depth);
                if *branch {
                    n = n.branch();
                    if *expanded {
                        n = n.expanded();
                    }
                }
                n
            })
            .collect();
        let rows = layout::rows(l, &[2, 0]);
        self.filter.set_focused(ctx.interaction.focused(FILTER));
        let _ = TextInput::new("", ctx.system)
            .placeholder("Filter connections")
            .paint(rows[0], buf, &mut self.filter);
        ctx.control(FILTER, rows[0], false);

        let (inner, _bg) = layout::card(
            rows[1],
            buf,
            t,
            Some("Connections"),
            None,
            ctx.interaction.focused(TREE),
        );
        StatefulWidget::render(
            &Tree::new(&nodes, ctx.system).focused(ctx.interaction.focused(TREE)),
            inner,
            buf,
            &mut self.tree,
        );
        ctx.control(TREE, inner, false);
        ctx.scrollable(TREE, inner);

        match &self.state {
            ConnState::Connecting { ticks, name } => {
                let (inner, bg) = layout::card(r, buf, t, Some("Connecting"), None, false);
                buf.set_string(inner.x, inner.y, name, t.primary().bg(bg));
                let phase = if *ticks < 4 {
                    "Opening SSH tunnel"
                } else if *ticks < 8 {
                    "Authenticating"
                } else {
                    "Loading catalog"
                };
                buf.set_string(inner.x, inner.y + 2, phase, t.secondary().bg(bg));
                ProgressBar::new(
                    ProgressKind::Indeterminate {
                        tick: ctx.interaction.tick,
                    },
                    ctx.system,
                )
                .paint(Rect::new(inner.x, inner.y + 4, inner.width.min(40), 1), buf);
            }
            ConnState::Failed {
                name,
                message,
                detail,
            } => {
                let (inner, bg) = layout::card(r, buf, t, Some("Could not connect"), None, false);
                buf.set_string(inner.x, inner.y, name, t.primary().bg(bg));
                buf.set_string(inner.x, inner.y + 2, message, t.error_fg().bg(bg));
                let mut y = inner.y + 4;
                for line in crate::text::wrap(detail, inner.width as usize) {
                    if y >= inner.bottom().saturating_sub(2) {
                        break;
                    }
                    buf.set_string(inner.x, y, &line, t.secondary().bg(bg));
                    y += 1;
                }
                let btn = Rect::new(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    paint::button_width("Retry"),
                    1,
                );
                paint::button(
                    "Retry",
                    ButtonVariant::Secondary,
                    RETRY,
                    btn,
                    buf,
                    ctx,
                    &mut self.retry_btn,
                    false,
                    bg,
                );
            }
            ConnState::Idle => self.render_detail(r, buf, ctx),
        }
    }

    fn render_detail(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let Some(c) = self.selected_connection() else {
            let (inner, bg) = layout::card(area, buf, t, Some("Connection"), None, false);
            buf.set_string(inner.x, inner.y, "Select a connection", t.muted().bg(bg));
            return;
        };
        let title = c.name.clone();
        let host = format!("{}:{}", c.host, c.port);
        let engine = c.engine.label().to_owned();
        let env = c.environment.label().to_owned();
        let db = c.database.clone();
        let user = c.user.clone();
        let last = c.last_used.clone();
        let safe = c.safe_mode.label().to_owned();
        let (inner, bg) = layout::card(area, buf, t, Some(&title), Some(&engine), false);
        let env_status = match c.environment {
            Environment::Production => KvStatus::Danger,
            Environment::Staging => KvStatus::Warning,
            _ => KvStatus::Neutral,
        };
        let entries = [
            KvEntry::pair(0, "Host", host.as_str()),
            KvEntry::pair(1, "Database", db.as_str()),
            KvEntry::pair(2, "User", user.as_str()),
            KvEntry {
                status: Some(env_status),
                ..KvEntry::pair(3, "Environment", env.as_str())
            },
            KvEntry::pair(4, "Safe Mode", safe.as_str()),
            KvEntry::pair(5, "Last used", last.as_str()),
        ];
        let kv_h = 8.min(inner.height.saturating_sub(2));
        KeyValueList::new(&entries, ctx.system).paint(
            Rect::new(inner.x, inner.y, inner.width, kv_h),
            buf,
            &mut self.kv,
        );
        let ay = inner.bottom().saturating_sub(1);
        let widths = [
            paint::button_width("Connect"),
            paint::button_width("Edit"),
            paint::button_width("Duplicate"),
            paint::button_width("Delete…"),
        ];
        let rects = layout::row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
        paint::button(
            "Connect",
            ButtonVariant::Primary,
            CONNECT,
            rects[0],
            buf,
            ctx,
            &mut self.connect_btn,
            false,
            bg,
        );
        paint::button(
            "Edit",
            ButtonVariant::Secondary,
            EDIT,
            rects[1],
            buf,
            ctx,
            &mut self.edit_btn,
            false,
            bg,
        );
        paint::button(
            "Duplicate",
            ButtonVariant::Quiet,
            DUP,
            rects[2],
            buf,
            ctx,
            &mut self.dup_btn,
            false,
            bg,
        );
        paint::button(
            "Delete…",
            ButtonVariant::Destructive,
            DEL,
            rects[3],
            buf,
            ctx,
            &mut self.del_btn,
            false,
            bg,
        );
    }

    fn render_form(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let t = ctx.theme;
        let title = if form.index.is_some() {
            "Edit connection"
        } else {
            "New connection"
        };
        let (inner, bg) = layout::card(area, buf, t, Some(title), None, false);
        let tabs = [Tab::new(0, "Basic"), Tab::new(1, "Advanced")];
        form.tabs.set_focused(ctx.interaction.focused(FORM_TABS));
        Tabs::new(&tabs, ctx.system).paint(
            Rect::new(inner.x, inner.y, inner.width, 2),
            buf,
            &mut form.tabs,
        );
        ctx.control(
            FORM_TABS,
            Rect::new(inner.x, inner.y, inner.width, 2),
            false,
        );
        let y = inner.y + 3;
        form.name.set_focused(ctx.interaction.focused(FORM_NAME));
        TextInput::new("Name", ctx.system).required(true).paint(
            Rect::new(inner.x, y, inner.width.min(40), 2),
            buf,
            &mut form.name,
        );
        ctx.control(
            FORM_NAME,
            Rect::new(inner.x, y, inner.width.min(40), 2),
            false,
        );
        form.host.set_focused(ctx.interaction.focused(FORM_HOST));
        TextInput::new("Host", ctx.system).paint(
            Rect::new(inner.x, y + 3, inner.width.min(40), 2),
            buf,
            &mut form.host,
        );
        ctx.control(
            FORM_HOST,
            Rect::new(inner.x, y + 3, inner.width.min(40), 2),
            false,
        );
        let env = [
            RadioOption::new(0, "local"),
            RadioOption::new(1, "development"),
            RadioOption::new(2, "staging"),
            RadioOption::new(3, "production"),
        ];
        form.env.set_surface_focused(false);
        RadioGroup::new(&env, ctx.system)
            .legend("Environment")
            .paint(
                Rect::new(inner.x, y + 6, inner.width.min(40), 5),
                buf,
                &mut form.env,
            );
        let ay = inner.bottom().saturating_sub(1);
        let widths = [
            paint::button_width("Save & connect"),
            paint::button_width("Save"),
            paint::button_width("Cancel"),
        ];
        let rects = layout::row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
        paint::button(
            "Save & connect",
            ButtonVariant::Primary,
            FORM_CONNECT,
            rects[0],
            buf,
            ctx,
            &mut form.save_connect,
            false,
            bg,
        );
        paint::button(
            "Save",
            ButtonVariant::Secondary,
            FORM_SAVE,
            rects[1],
            buf,
            ctx,
            &mut form.save,
            false,
            bg,
        );
        paint::button(
            "Cancel",
            ButtonVariant::Quiet,
            FORM_CANCEL,
            rects[2],
            buf,
            ctx,
            &mut form.cancel,
            false,
            bg,
        );
    }

    pub fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> (Route, Option<ConnEvent>) {
        if self.form.is_some() {
            return (self.handle_form(ev, cx), None);
        }
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = *cx.focus else {
                    return (Route::Ignored, None);
                };
                if f == FILTER {
                    let o = self.filter.handle_key(*key);
                    return (
                        if matches!(o, termrock::widgets::TextInputOutcome::Ignored) {
                            Route::Ignored
                        } else {
                            Route::Changed
                        },
                        None,
                    );
                }
                if f == TREE {
                    let vis = self.visible();
                    let nodes: Vec<TreeNode<'_, String>> = vis
                        .iter()
                        .map(|(id, label, depth, branch, expanded)| {
                            let mut n =
                                TreeNode::new(id.clone(), Line::from(label.as_str()), *depth);
                            if *branch {
                                n = n.branch();
                                if *expanded {
                                    n = n.expanded();
                                }
                            }
                            n
                        })
                        .collect();
                    let o = self.tree.handle_key(&nodes, *key);
                    return (self.apply_tree(o, cx), None);
                }
                if f == CONNECT && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    if let Some(i) = self.selected {
                        self.start_connect(i);
                    }
                    return (Route::Changed, None);
                }
                if f == EDIT && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.open_form(self.selected);
                    return (Route::Changed, None);
                }
                if f == DUP && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.duplicate(cx);
                    return (Route::Changed, None);
                }
                if f == DEL && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.delete_selected(cx);
                    return (Route::Changed, None);
                }
                if f == RETRY && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    if let Some(i) = self.selected {
                        self.start_connect(i);
                    }
                    return (Route::Changed, None);
                }
                if matches!(key.code, KeyCode::Enter) && f == TREE {
                    if let Some(i) = self.selected {
                        self.start_connect(i);
                        return (Route::Changed, None);
                    }
                }
                (Route::Ignored, None)
            }
            PageEvent::Click { id, pos } => {
                if *id == TREE {
                    cx.set_focus(TREE);
                    let o = self.tree.click(*pos);
                    let activated = matches!(o, TreeOutcome::Activated(_));
                    let route = self.apply_tree(o, cx);
                    if activated && let Some(i) = self.selected {
                        self.start_connect(i);
                    }
                    return (route, None);
                }
                if *id == FILTER {
                    cx.set_focus(FILTER);
                    self.filter.begin_edit();
                    return (Route::Changed, None);
                }
                if *id == CONNECT {
                    if let Some(i) = self.selected {
                        self.start_connect(i);
                    }
                    return (Route::Changed, None);
                }
                if *id == EDIT {
                    self.open_form(self.selected);
                    return (Route::Changed, None);
                }
                if *id == DUP {
                    self.duplicate(cx);
                    return (Route::Changed, None);
                }
                if *id == DEL {
                    self.delete_selected(cx);
                    return (Route::Changed, None);
                }
                if *id == RETRY {
                    if let Some(i) = self.selected {
                        self.start_connect(i);
                    }
                    return (Route::Changed, None);
                }
                (Route::Ignored, None)
            }
            PageEvent::Wheel { id, delta } if *id == TREE => {
                let n = self.visible().len();
                let _ = self.tree.scroll_by(*delta as isize, n);
                (Route::Changed, None)
            }
            PageEvent::Paste(text) if self.filter.is_editing() => {
                let _ = self.filter.insert_str(text);
                (Route::Changed, None)
            }
            _ => (Route::Ignored, None),
        }
    }

    fn apply_tree(&mut self, o: TreeOutcome<String>, cx: &mut PageCtx<'_>) -> Route {
        match o {
            TreeOutcome::Ignored => Route::Ignored,
            TreeOutcome::Toggle(id) => {
                if self.expanded.contains(&id) {
                    self.expanded.remove(&id);
                } else {
                    self.expanded.insert(id);
                }
                Route::Changed
            }
            TreeOutcome::SelectionChanged(id) | TreeOutcome::Activated(id) => {
                self.select_named(&id);
                cx.set_focus(TREE);
                Route::Changed
            }
            _ => Route::Changed,
        }
    }

    fn duplicate(&mut self, cx: &mut PageCtx<'_>) {
        let Some(i) = self.selected else { return };
        let mut c = self.connections[i].clone();
        c.name = format!("{} copy", c.name);
        self.connections.insert(i + 1, c);
        self.selected = Some(i + 1);
        cx.status("Duplicated connection");
    }

    fn delete_selected(&mut self, cx: &mut PageCtx<'_>) {
        let Some(i) = self.selected else { return };
        let name = self.connections[i].name.clone();
        self.connections.remove(i);
        self.selected = if self.connections.is_empty() {
            None
        } else {
            Some(i.min(self.connections.len() - 1))
        };
        cx.status(format!("Deleted {name}"));
    }

    fn handle_form(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::Esc {
                    self.form = None;
                    return Route::Changed;
                }
                let Some(form) = self.form.as_mut() else {
                    return Route::Ignored;
                };
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == FORM_NAME {
                    let o = form.name.handle_key(*key);
                    return if matches!(o, termrock::widgets::TextInputOutcome::Ignored) {
                        Route::Ignored
                    } else {
                        Route::Changed
                    };
                }
                if f == FORM_HOST {
                    let o = form.host.handle_key(*key);
                    return if matches!(o, termrock::widgets::TextInputOutcome::Ignored) {
                        Route::Ignored
                    } else {
                        Route::Changed
                    };
                }
                Route::Ignored
            }
            PageEvent::Click { id, .. } if *id == FORM_CANCEL => {
                self.form = None;
                Route::Changed
            }
            PageEvent::Click { id, .. } if *id == FORM_SAVE || *id == FORM_CONNECT => {
                let connect = *id == FORM_CONNECT;
                self.commit_form(cx, connect)
            }
            PageEvent::Key(key)
                if key.kind != KeyEventKind::Release
                    && matches!(key.code, KeyCode::Enter)
                    && (*cx.focus == Some(FORM_SAVE) || *cx.focus == Some(FORM_CONNECT)) =>
            {
                let connect = *cx.focus == Some(FORM_CONNECT);
                self.commit_form(cx, connect)
            }
            _ => Route::Ignored,
        }
    }

    fn commit_form(&mut self, cx: &mut PageCtx<'_>, connect: bool) -> Route {
        let Some(form) = self.form.as_ref() else {
            return Route::Ignored;
        };
        let name = form.name.trimmed_value().to_owned();
        if name.is_empty() {
            cx.status("Name required");
            return Route::Changed;
        }
        let env = match form.env.selected() {
            Some(&1) => Environment::Development,
            Some(&2) => Environment::Staging,
            Some(&3) => Environment::Production,
            _ => Environment::Local,
        };
        let host = form.host.value().to_owned();
        let port = form.port.value().parse().unwrap_or(5432);
        let database = form.database.value().to_owned();
        let user = form.user.value().to_owned();
        let index = form.index;
        if let Some(i) = index {
            let c = &mut self.connections[i];
            c.name = name.clone();
            c.host = host;
            c.port = port;
            c.database = database;
            c.user = user;
            c.environment = env;
            self.selected = Some(i);
        } else {
            self.connections.push(Connection {
                name: name.clone(),
                engine: Engine::Postgres,
                host,
                port,
                database,
                user,
                environment: env,
                safe_mode: SafeMode::Silent,
                ssl: false,
                ssh: None,
                group: "Personal".into(),
                last_used: "never".into(),
                outcome: ConnectOutcome::Ok,
            });
            self.selected = Some(self.connections.len() - 1);
        }
        self.form = None;
        cx.status(format!("Saved {name}"));
        if connect && let Some(i) = self.selected {
            self.start_connect(i);
        }
        Route::Changed
    }

    #[must_use]
    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.form.is_some() {
            return vec![("Esc", "Cancel"), ("Enter", "Edit")];
        }
        if focus == Some(TREE) {
            vec![("↑ ↓", "Move"), ("Enter", "Connect")]
        } else {
            vec![("Enter", "Connect"), ("e", "Edit")]
        }
    }
}
