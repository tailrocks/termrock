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
use termrock::style::Tone;
use termrock::widgets::{
    ButtonState, ButtonVariant, Checkbox, CheckboxState, PasswordInput, PasswordInputState, Prop,
    RadioGroup, RadioOption, RadioState, Select, SelectOption, SelectRecipe, SelectState, Spinner,
    SpinnerState, Tab, Tabs, TabsState, TextArea, TextAreaState, TextInput, TextInputState, Toggle,
    ToggleState, ToggleValue, Tree, TreeNode, TreeOutcome, TreeState,
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
const FORM_SAVE: WidgetId = ID.sub("form-save");
const FORM_CANCEL: WidgetId = ID.sub("form-cancel");
const FORM_CONNECT: WidgetId = ID.sub("form-saveconnect");
const FORM_NAME: WidgetId = ID.sub("form-name");
const FORM_ENGINE: WidgetId = ID.sub("form-engine");
const FORM_HOST: WidgetId = ID.sub("form-host");
const FORM_PORT: WidgetId = ID.sub("form-port");
const FORM_DATABASE: WidgetId = ID.sub("form-database");
const FORM_USER: WidgetId = ID.sub("form-user");
const FORM_PASSWORD: WidgetId = ID.sub("form-password");
const FORM_PROMPT: WidgetId = ID.sub("form-prompt-password");
const FORM_ENV: WidgetId = ID.sub("form-environment");
const FORM_GROUP: WidgetId = ID.sub("form-group");
const FORM_SAFE: WidgetId = ID.sub("form-safe-mode");
const FORM_SSL: WidgetId = ID.sub("form-ssl");
const FORM_SSH: WidgetId = ID.sub("form-ssh");
const FORM_SSH_HOST: WidgetId = ID.sub("form-ssh-host");
const FORM_SSH_USER: WidgetId = ID.sub("form-ssh-user");
const FORM_STARTUP: WidgetId = ID.sub("form-startup-sql");
const FORM_LOCAL: WidgetId = ID.sub("form-local-only");
const FORM_TEST: WidgetId = ID.sub("form-test");
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
    Testing {
        ticks: u32,
    },
    Tested(Result<String, String>),
}

pub enum ConnEvent {
    Connected(usize),
}

struct ConnForm {
    index: Option<usize>,
    tabs: TabsState<u8>,
    name: TextInputState,
    engine: SelectState<u8>,
    host: TextInputState,
    port: TextInputState,
    database: TextInputState,
    user: TextInputState,
    password: PasswordInputState,
    password_editing: bool,
    prompt_pw: CheckboxState,
    env: RadioState<u8>,
    group: SelectState<u8>,
    safe: RadioState<u8>,
    ssl: ToggleState,
    ssh: ToggleState,
    ssh_host: TextInputState,
    ssh_user: TextInputState,
    startup: TextAreaState,
    local_only: ToggleState,
    test: ButtonState,
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
    pending_delete: Option<usize>,
    connect_btn: ButtonState,
    edit_btn: ButtonState,
    dup_btn: ButtonState,
    del_btn: ButtonState,
    form: Option<ConnForm>,
}

fn engine_options() -> Vec<SelectOption<u8>> {
    vec![
        SelectOption::option(0, "PostgreSQL"),
        SelectOption::option(1, "MySQL"),
        SelectOption::option(2, "SQLite"),
    ]
}

fn group_options() -> Vec<SelectOption<u8>> {
    vec![
        SelectOption::option(0, "Personal"),
        SelectOption::option(1, "Acme"),
        SelectOption::option(2, "Clients"),
    ]
}

fn env_options() -> [RadioOption<'static, u8>; 4] {
    [
        RadioOption::new(0, "local"),
        RadioOption::new(1, "development"),
        RadioOption::new(2, "staging"),
        RadioOption::new(3, "production"),
    ]
}

fn safe_options() -> [RadioOption<'static, u8>; 6] {
    [
        RadioOption::new(0, "Silent"),
        RadioOption::new(1, "Alert"),
        RadioOption::new(2, "Alert (Full)"),
        RadioOption::new(3, "Safe Mode"),
        RadioOption::new(4, "Safe Mode (Full)"),
        RadioOption::new(5, "Read-Only"),
    ]
}

fn engine_index(engine: Engine) -> u8 {
    match engine {
        Engine::Postgres => 0,
        Engine::MySql => 1,
        Engine::Sqlite => 2,
    }
}

fn environment_index(environment: Environment) -> u8 {
    match environment {
        Environment::Local => 0,
        Environment::Development => 1,
        Environment::Staging => 2,
        Environment::Production => 3,
    }
}

fn safe_index(safe_mode: SafeMode) -> u8 {
    SafeMode::ALL
        .iter()
        .position(|candidate| *candidate == safe_mode)
        .unwrap_or(0) as u8
}

fn port_valid(value: &str) -> bool {
    value.is_empty()
        || value
            .parse::<u32>()
            .is_ok_and(|port| (1..=u32::from(u16::MAX)).contains(&port))
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
            pending_delete: None,
            connect_btn: ButtonState::new(),
            edit_btn: ButtonState::new(),
            dup_btn: ButtonState::new(),
            del_btn: ButtonState::new(),
            form: None,
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
                    || f.password_editing
                    || f.host.is_editing()
                    || f.port.is_editing()
                    || f.database.is_editing()
                    || f.user.is_editing()
                    || f.ssh_host.is_editing()
                    || f.ssh_user.is_editing()
                    || f.startup.is_editing()
            })
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        matches!(
            self.state,
            ConnState::Connecting { .. } | ConnState::Testing { .. }
        )
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
        let mut ssh_host = TextInputState::new(c.and_then(|c| c.ssh.as_deref()).unwrap_or(""))
            .with_allow_empty(true);
        ssh_host.set_editing(false);
        let mut ssh_user = TextInputState::new("deploy").with_allow_empty(true);
        ssh_user.set_editing(false);
        let engine = SelectState::new()
            .with_value(c.map(|c| engine_index(c.engine)).unwrap_or(0))
            .with_recipe(SelectRecipe::Form);
        let group = SelectState::new()
            .with_value(
                c.map(|c| if c.group == "Acme" { 1 } else { 0 })
                    .unwrap_or(0),
            )
            .with_recipe(SelectRecipe::Form);
        let safe = RadioState::new(Some(c.map(|c| safe_index(c.safe_mode)).unwrap_or(0)));
        let ssh_on = c.is_some_and(|c| c.ssh.is_some());
        self.form = Some(ConnForm {
            index,
            tabs,
            name,
            engine,
            host,
            port,
            database,
            user,
            password: PasswordInputState::new(),
            password_editing: false,
            prompt_pw: CheckboxState::new(false),
            env: RadioState::new(Some(
                c.map(|c| environment_index(c.environment)).unwrap_or(0),
            )),
            group,
            safe,
            ssl: ToggleState::with_value(ToggleValue::from_pressed(c.is_some_and(|c| c.ssl))),
            ssh: ToggleState::with_value(ToggleValue::from_pressed(ssh_on)),
            ssh_host,
            ssh_user,
            startup: TextAreaState::new(""),
            local_only: ToggleState::new(),
            test: ButtonState::new(),
            save: ButtonState::new(),
            cancel: ButtonState::new(),
            save_connect: ButtonState::new(),
        });
    }

    pub fn tick(&mut self) -> Option<ConnEvent> {
        match &mut self.state {
            ConnState::Testing { ticks } => {
                *ticks += 1;
                if *ticks >= 10 {
                    let result = self
                        .form
                        .as_ref()
                        .map(|form| {
                            if form.host.value().contains("analytics") {
                                Err("Connection timed out after 10 s".into())
                            } else {
                                Ok("Connected · PostgreSQL 16.3 · 12 ms".into())
                            }
                        })
                        .unwrap_or_else(|| Err("Connection form is closed".into()));
                    self.state = ConnState::Tested(result);
                }
            }
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
        let list_w = (area.width / 3).clamp(26, 40);
        let (l, r) = if area.width >= 80 {
            (
                Rect::new(area.x, area.y, list_w, area.height),
                Rect::new(
                    area.x + list_w + 2,
                    area.y,
                    area.width.saturating_sub(list_w + 2),
                    area.height,
                ),
            )
        } else {
            (area, Rect::ZERO)
        };
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
                } else if let Some(c) = self.connections.iter().find(|c| c.name == *id) {
                    let glyph = match c.environment {
                        Environment::Production => "◆",
                        Environment::Staging => "◇",
                        _ => "·",
                    };
                    n = n
                        .leading(Line::from(glyph))
                        .badge(Line::from(c.engine.short()));
                }
                n
            })
            .collect();

        let focus_list = ctx.interaction.focused(TREE) || ctx.interaction.focused(FILTER);
        let count = self.connections.len().to_string();
        let (inner, bg) = layout::framed(l, buf, t, Some("Connections"), focus_list);
        // `layout::framed` reserves the source's empty meta slot. Fill the
        // same slot with the saved-connection count.
        buf.set_string(
            l.right().saturating_sub(5),
            l.y,
            &format!(" {count} "),
            t.faint().bg(bg),
        );

        self.filter.set_focused(ctx.interaction.focused(FILTER));
        let _ = TextInput::new(" ", ctx.system)
            .placeholder("Filter connections")
            .paint(
                Rect::new(
                    inner.x.saturating_sub(1),
                    inner.y,
                    inner.width.saturating_add(1),
                    2,
                ),
                buf,
                &mut self.filter,
            );
        ctx.control(
            FILTER,
            Rect::new(
                inner.x.saturating_sub(1),
                inner.y,
                inner.width.saturating_add(1),
                2,
            ),
            false,
        );

        let tree_area = Rect::new(
            inner.x.saturating_sub(1),
            inner.y + 2,
            inner.width.saturating_add(1),
            inner.height.saturating_sub(2),
        );
        StatefulWidget::render(
            &Tree::new(&nodes, ctx.system)
                .focused(ctx.interaction.focused(TREE))
                .background(bg),
            tree_area,
            buf,
            &mut self.tree,
        );
        ctx.control(TREE, tree_area, false);
        ctx.scrollable(TREE, tree_area);

        if r.is_empty() {
            return;
        }
        self.render_detail(r, buf, ctx);
    }

    fn render_detail(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let Some(c) = self.selected_connection() else {
            let (inner, bg) = layout::card(area, buf, t, Some("Connection"), None, false);
            buf.set_string(inner.x, inner.y, "Select a connection", t.muted().bg(bg));
            return;
        };
        let title = c.name.clone();
        let card_h = 17.min(area.height);
        let (inner, bg) = layout::card(
            Rect::new(area.x, area.y, area.width.min(70), card_h),
            buf,
            t,
            Some(&title),
            None,
            false,
        );
        let ssl = if c.ssl { "on" } else { "off" };
        let ssh = c.ssh.clone().unwrap_or_else(|| "off".into());
        let safe_value = format!("{} · {}", c.safe_mode.label(), c.safe_mode.description());
        let safe_tone = if c.safe_mode >= SafeMode::Safe {
            Tone::Normal
        } else {
            Tone::Secondary
        };
        let value_width = usize::from(inner.width.saturating_sub(6).saturating_sub(13)).max(4);
        let safe_lines = crate::text::wrap(&safe_value, value_width);
        let mut facts = vec![
            Prop::new("Engine", c.engine.label()),
            Prop::new(
                "Host",
                if c.port > 0 {
                    format!("{}:{}", c.host, c.port)
                } else {
                    c.host.clone()
                },
            ),
            Prop::new(
                "Database",
                if c.database.is_empty() {
                    "—".into()
                } else {
                    c.database.clone()
                },
            ),
            Prop::new(
                "User",
                if c.user.is_empty() {
                    "—".into()
                } else {
                    c.user.clone()
                },
            ),
            Prop::new("Environment", c.environment.label()).tone(match c.environment {
                Environment::Production => Tone::Normal,
                Environment::Staging => Tone::Secondary,
                Environment::Development => Tone::Muted,
                Environment::Local => Tone::Faint,
            }),
        ];
        facts.extend(safe_lines.iter().enumerate().map(|(i, line)| {
            Prop::new(if i == 0 { "Safe Mode" } else { "" }, line.clone()).tone(safe_tone)
        }));
        let safe_end = facts.len();
        if c.environment == Environment::Production && c.safe_mode == SafeMode::Silent {
            facts.insert(
                safe_end,
                Prop::new(
                    "",
                    "Production with Silent safe mode: writes run without asking",
                )
                .tone(Tone::Warning)
                .wrap(),
            );
        }
        facts.push(Prop::new("SSL / SSH", format!("{ssl} / {ssh}")).tone(Tone::Secondary));
        facts.push(Prop::new("Last used", c.last_used.clone()).tone(Tone::Muted));
        let used = termrock::widgets::render_props(
            Rect::new(
                inner.x,
                inner.y,
                inner.width.saturating_sub(6),
                inner.height.saturating_sub(3),
            ),
            buf,
            t,
            &facts,
            bg,
        );
        let sy = inner.y + used + 1;
        match &self.state {
            ConnState::Connecting { ticks, name } if *name == c.name => {
                let phase = if *ticks < 4 {
                    "Opening SSH tunnel…"
                } else if *ticks < 8 {
                    "Authenticating…"
                } else {
                    "Loading schema…"
                };
                Spinner::labeled(phase, ctx.system).paint(
                    Rect::new(inner.x, sy, inner.width, 1),
                    buf,
                    &SpinnerState::new(),
                    paint::tick_frame(ctx.interaction.tick),
                    termrock::style::MotionPolicy::Full,
                );
            }
            ConnState::Failed {
                name,
                message,
                detail,
            } if *name == c.name => {
                buf.set_string(
                    inner.x,
                    sy,
                    "!",
                    t.error_fg()
                        .bg(bg)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
                buf.set_string(inner.x + 2, sy, message, t.error_fg().bg(bg));
                for (i, line) in crate::text::wrap(detail, inner.width.saturating_sub(2) as usize)
                    .iter()
                    .take(2)
                    .enumerate()
                {
                    buf.set_string(inner.x + 2, sy + 1 + i as u16, line, t.muted().bg(bg));
                }
            }
            _ => {}
        }

        let connecting =
            matches!(&self.state, ConnState::Connecting { name, .. } if *name == c.name);
        let failed = matches!(&self.state, ConnState::Failed { name, .. } if *name == c.name);
        self.connect_btn.activation.set_loading(connecting);
        let connect_label = if failed { "Reconnect" } else { "Connect" };
        let ay = inner.bottom().saturating_sub(1);
        let widths = [
            paint::button_width(connect_label),
            paint::button_width("Edit"),
            paint::button_width("Duplicate"),
            paint::button_width("Delete…"),
        ];
        let rects = layout::row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
        paint::button(
            connect_label,
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
                    cx.set_focus(FORM_NAME);
                    return (Route::Changed, None);
                }
                if f == DUP && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.duplicate(cx);
                    return (Route::Changed, None);
                }
                if f == DEL && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.delete_selected();
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
                    cx.set_focus(FORM_NAME);
                    return (Route::Changed, None);
                }
                if *id == DUP {
                    self.duplicate(cx);
                    return (Route::Changed, None);
                }
                if *id == DEL {
                    self.delete_selected();
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

    fn delete_selected(&mut self) {
        let Some(i) = self.selected else { return };
        self.pending_delete = Some(i);
    }

    pub fn take_delete_request(&mut self) -> Option<usize> {
        self.pending_delete.take()
    }

    pub fn delete_at(&mut self, i: usize) -> Option<String> {
        let name = self.connections.get(i)?.name.clone();
        self.connections.remove(i);
        self.selected = if self.connections.is_empty() {
            None
        } else {
            Some(i.min(self.connections.len() - 1))
        };
        self.tree = TreeState::new(
            self.selected_connection()
                .map(|connection| connection.name.clone()),
        );
        Some(name)
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
        if focus == Some(FILTER) {
            return vec![("Type", "Filter"), ("↓", "Into list"), ("Esc", "Clear")];
        }
        vec![
            ("↑ ↓", "Move"),
            ("Enter", "Connect"),
            ("/", "Filter"),
            ("Ctrl+N", "New"),
        ]
    }
}
