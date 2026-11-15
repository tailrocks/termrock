use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Stack, StackChild, StackScroll};
use junie_tui::ui::leave::{KeyContext, Leave, LeaveGuard, Resolved, Stakes, letters_allowed};
use junie_tui::ui::master_detail::{MasterDetail, Mode};
use junie_tui::widgets::action_row::ActionRow;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::choice::{Checkbox, RadioGroup, Toggle};
use junie_tui::widgets::detail_card::DetailCard;
use junie_tui::widgets::field_common::{PointerField, route_click, route_drag, route_press};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::tabs::Tabs;
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};

use crate::app::OpenDialog as _;
use crate::db::{ConnectOutcome, Connection, Engine, Environment, SafeMode};

const ID: WidgetId = WidgetId::of("connections");

pub const TREE: WidgetId = WidgetId::of("connections").sub("tree");

pub const NEW_ID: WidgetId = WidgetId::of("connections").sub("new");

/// Simulated connect attempts finish after this many ticks.
const CONNECT_TICKS: u8 = 12;

/// One configured connection with its stable identity. The key is allocated
/// once and never reused; the revision counts semantic settings edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEntry {
    key: ItemKey,
    value: Connection,
    revision: u64,
}

impl ConnectionEntry {
    pub fn key(&self) -> ItemKey {
        self.key
    }

    #[cfg(test)]
    pub fn value(&self) -> &Connection {
        &self.value
    }

    #[cfg(test)]
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// A mutation was refused before any state changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMutationError {
    /// A key, revision or attempt counter has no further value to hand out.
    IdentityExhausted,
}

impl ConnectionMutationError {
    pub fn message(self) -> &'static str {
        match self {
            ConnectionMutationError::IdentityExhausted => {
                "Connections cannot be changed right now; the request was not applied"
            }
        }
    }
}

/// A connect attempt, captured once when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectRequest {
    pub connection: ItemKey,
    pub revision: u64,
    pub generation: u64,
    pub snapshot: Connection,
    pub ticks_left: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnState {
    Idle,
    Connecting(ConnectRequest),
    Failed {
        key: ItemKey,
        revision: u64,
        message: String,
        detail: String,
    },
    Testing {
        ticks: u32,
    },
    Tested(Result<String, String>),
}

/// Why an admitted completion did not connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancelled {
    Deleted,
    Changed,
}

impl Cancelled {
    pub fn message(self) -> &'static str {
        match self {
            Cancelled::Deleted => "Connection was deleted; connect cancelled",
            Cancelled::Changed => "Connection was changed; connect cancelled",
        }
    }
}

pub enum ConnEvent {
    Connected {
        key: ItemKey,
        generation: u64,
        connection: Box<Connection>,
    },
    Cancelled(Cancelled),
}

/// How a `--connect` name failed to pick exactly one connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameLookup {
    NotFound,
    Ambiguous(usize),
}

pub struct ConnectionsScreen {
    entries: Vec<ConnectionEntry>,
    next_key: u64,
    last_generation: u64,
    /// Tree projection: `tree_keys[group][child]` is the entry behind that node.
    tree_keys: Vec<Vec<ItemKey>>,
    tree: TreeView,
    filter: TextInput,
    selected: Option<ItemKey>,
    delete_target: Option<ItemKey>,
    pub state: ConnState,
    connect_btn: Button,
    edit_btn: Button,
    dup_btn: Button,
    del_btn: Button,
    retry_btn: Button,

    md: MasterDetail,
    form: Option<ConnForm>,

    pub leave: LeaveGuard<FormLeave>,

    glyphs: junie_tui::theme::Glyphs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormLeave {
    Close,

    Edit(ItemKey),

    New,
}

struct ConnForm {
    target: Option<ItemKey>,
    expected_revision: u64,
    tabs: Tabs,
    name: TextInput,
    engine: Select,
    host: TextInput,
    port: TextInput,
    database: TextInput,
    user: TextInput,
    password: TextInput,
    prompt_pw: Checkbox,
    env: RadioGroup,
    group: Select,
    safe: RadioGroup,

    ssl: Toggle,
    ssh: Toggle,
    ssh_host: TextInput,
    ssh_user: TextInput,
    startup: junie_tui::widgets::textarea::TextArea,
    local_only: Toggle,
    test_btn: Button,
    cancel_btn: Button,
    save_btn: Button,
    save_connect_btn: Button,

    stacks: [[StackScroll; 2]; 2],

    pristine: Vec<String>,
}

fn port_validator(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    match s.parse::<u32>() {
        Ok(p) if p > 0 && p < 65536 => None,
        _ => Some("Port must be 1–65535".into()),
    }
}

fn name_validator(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        Some("Required".into())
    } else {
        None
    }
}

impl ConnForm {
    fn new(c: Option<&Connection>, target: Option<ItemKey>, expected_revision: u64) -> Self {
        let f = ID.sub("form");
        let engines = ["PostgreSQL", "MySQL", "SQLite"];
        let eng = c
            .map(|c| match c.engine {
                Engine::Postgres => 0,
                Engine::MySql => 1,
                Engine::Sqlite => 2,
            })
            .unwrap_or(0);
        let env_i = c
            .map(|c| match c.environment {
                Environment::Local => 0,
                Environment::Development => 1,
                Environment::Staging => 2,
                Environment::Production => 3,
            })
            .unwrap_or(0);
        let safe_i = c
            .map(|c| {
                SafeMode::ALL
                    .iter()
                    .position(|s| *s == c.safe_mode)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let mut tabs = Tabs::new(f.sub("tabs"), &["Basic", "Advanced"]);
        tabs.active = 0;
        Self {
            target,
            expected_revision,
            tabs,
            name: TextInput::new(f.sub("name"), "Name")
                .required(true)
                .validator(name_validator)
                .value(c.map(|c| c.name.as_str()).unwrap_or("")),
            engine: Select::new(f.sub("engine"), "Engine", &engines, eng),
            host: TextInput::new(f.sub("host"), "Host")
                .value(c.map(|c| c.host.as_str()).unwrap_or("localhost"))
                .help("Blank: driver default"),
            port: TextInput::new(f.sub("port"), "Port")
                .value(&c.map(|c| c.port.to_string()).unwrap_or("5432".into()))
                .validator(port_validator),
            database: TextInput::new(f.sub("db"), "Database")
                .value(c.map(|c| c.database.as_str()).unwrap_or(""))
                .help("Required for PostgreSQL"),
            user: TextInput::new(f.sub("user"), "Username")
                .value(c.map(|c| c.user.as_str()).unwrap_or("")),
            password: TextInput::new(f.sub("pw"), "Password")
                .placeholder("stored in the keychain")
                .masked()
                .help("Never written to connections.json"),
            prompt_pw: Checkbox::new(f.sub("promptpw"), "Prompt for password on connect", false),
            env: RadioGroup::new(
                f.sub("env"),
                "Environment",
                &["local", "development", "staging", "production"],
                env_i,
            ),
            group: Select::new(
                f.sub("group"),
                "Group",
                &["Personal", "Acme", "Clients"],
                if c.is_some_and(|c| c.group == "Acme") {
                    1
                } else {
                    0
                },
            ),
            safe: RadioGroup::new(
                f.sub("safe"),
                "Safe Mode",
                &[
                    "Silent",
                    "Alert",
                    "Alert (Full)",
                    "Safe Mode",
                    "Safe Mode (Full)",
                    "Read-Only",
                ],
                safe_i,
            ),
            ssl: Toggle::new(f.sub("ssl"), "Use SSL / TLS", c.is_some_and(|c| c.ssl)),
            ssh: Toggle::new(
                f.sub("ssh"),
                "SSH tunnel",
                c.is_some_and(|c| c.ssh.is_some()),
            ),
            ssh_host: TextInput::new(f.sub("sshhost"), "SSH host")
                .value(c.and_then(|c| c.ssh.as_deref()).unwrap_or(""))
                .placeholder("bastion.example.com"),
            ssh_user: TextInput::new(f.sub("sshuser"), "SSH user").value("deploy"),
            startup: junie_tui::widgets::textarea::TextArea::new(
                f.sub("startup"),
                "Startup commands",
                3,
            )
            .placeholder("SET statement_timeout = '60s';")
            .help("Run after every connect, one per line"),
            local_only: Toggle::new(f.sub("local"), "Local only (no iCloud sync)", false),
            test_btn: Button::secondary(f.sub("test"), "Test connection"),
            cancel_btn: Button::subtle(f.sub("cancel"), "Cancel"),
            save_btn: Button::secondary(f.sub("save"), "Save"),
            save_connect_btn: Button::primary(f.sub("saveconnect"), "Save & connect"),
            stacks: Default::default(),
            pristine: vec![],
        }
        .with_pristine()
    }

    fn with_pristine(mut self) -> Self {
        self.pristine = self.values();
        self
    }

    fn values(&self) -> Vec<String> {
        let c = self.to_connection(None);
        vec![
            format!("{c:?}"),
            self.password.text().to_owned(),
            self.ssh_user.text().to_owned(),
            self.startup.field.buffer.text().to_owned(),
            self.prompt_pw.checked.to_string(),
            self.local_only.on.to_string(),
        ]
    }

    fn dirty(&self) -> bool {
        self.values() != self.pristine
    }

    fn to_connection(&self, base: Option<&Connection>) -> Connection {
        let engine = match self.engine.selected {
            0 => Engine::Postgres,
            1 => Engine::MySql,
            _ => Engine::Sqlite,
        };
        let environment = match self.env.selected {
            0 => Environment::Local,
            1 => Environment::Development,
            2 => Environment::Staging,
            _ => Environment::Production,
        };
        Connection {
            name: self.name.text().trim().to_owned(),
            engine,
            host: self.host.text().to_owned(),
            port: self.port.text().parse().unwrap_or(5432),
            database: self.database.text().to_owned(),
            user: self.user.text().to_owned(),
            environment,
            safe_mode: SafeMode::ALL[self.safe.selected.min(5)],
            ssl: self.ssl.on,
            ssh: if self.ssh.on && !self.ssh_host.text().is_empty() {
                Some(self.ssh_host.text().to_owned())
            } else {
                None
            },
            group: self.group.value().to_owned(),
            last_used: base.map(|b| b.last_used.clone()).unwrap_or("never".into()),
            outcome: base.map(|b| b.outcome).unwrap_or(ConnectOutcome::Ok),
        }
    }

    fn validate(&mut self) -> bool {
        let a = self.name.validate();
        let b = self.port.validate();
        a && b
    }
}

pub fn env_tone(env: Environment) -> Tone {
    env.kind().tone()
}

impl ConnectionsScreen {
    pub fn new(connections: Vec<Connection>) -> Self {
        let entries: Vec<ConnectionEntry> = connections
            .into_iter()
            .enumerate()
            .map(|(i, value)| ConnectionEntry {
                key: ItemKey::num(i as u64),
                value,
                revision: 0,
            })
            .collect();
        let next_key = entries.len() as u64;
        let mut s = Self {
            selected: entries.first().map(ConnectionEntry::key),
            entries,
            next_key,
            last_generation: 0,
            tree_keys: vec![],
            delete_target: None,
            tree: TreeView::new(ID.sub("tree"), vec![]),
            filter: TextInput::new(ID.sub("filter"), "")
                .placeholder("Filter connections")
                .plain_label(),
            state: ConnState::Idle,
            connect_btn: Button::primary(ID.sub("connect"), "Connect"),
            edit_btn: Button::secondary(ID.sub("edit"), "Edit"),
            dup_btn: Button::subtle(ID.sub("dup"), "Duplicate"),
            del_btn: Button::danger(ID.sub("del"), "Delete…"),
            retry_btn: Button::secondary(ID.sub("retry"), "Retry"),
            md: MasterDetail::new(ID.sub("tree"), ID.sub("connect"))
                .master_share(33, 26, 40)
                .collapse_below(80)
                .detail_owns(&[ID.sub("edit"), ID.sub("dup"), ID.sub("del")]),
            form: None,
            leave: LeaveGuard::new(ID.sub("leave")),
            glyphs: junie_tui::theme::Glyphs::default(),
        };
        s.rebuild_tree();
        s
    }

    /// Fixture for the counter-exhaustion boundary: every counter starts at the
    /// given value instead of being iterated there.
    #[cfg(test)]
    pub fn with_counters(
        connections: Vec<Connection>,
        next_key: u64,
        last_generation: u64,
        revision: u64,
    ) -> Self {
        let mut s = Self::new(connections);
        s.next_key = next_key;
        s.last_generation = last_generation;
        for e in &mut s.entries {
            e.revision = revision;
        }
        s
    }

    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = &ConnectionEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: ItemKey) -> Option<&ConnectionEntry> {
        self.entries.iter().find(|e| e.key == key)
    }

    fn position(&self, key: ItemKey) -> Option<usize> {
        self.entries.iter().position(|e| e.key == key)
    }

    #[cfg(test)]
    pub fn selected_key(&self) -> Option<ItemKey> {
        self.selected
    }

    /// The one connection a case-insensitive CLI name designates.
    pub fn resolve_name(&self, name: &str) -> Result<ItemKey, NameLookup> {
        let mut hits = self
            .entries
            .iter()
            .filter(|e| e.value.name.eq_ignore_ascii_case(name));
        match (hits.next(), hits.count()) {
            (None, _) => Err(NameLookup::NotFound),
            (Some(e), 0) => Ok(e.key),
            (Some(_), more) => Err(NameLookup::Ambiguous(more + 1)),
        }
    }

    /// An owned copy of one connection's current settings.
    pub fn snapshot(&self, key: ItemKey) -> Option<Connection> {
        self.get(key).map(|e| e.value.clone())
    }

    fn reserve_key(&self) -> Result<(ItemKey, u64), ConnectionMutationError> {
        let next = self
            .next_key
            .checked_add(1)
            .ok_or(ConnectionMutationError::IdentityExhausted)?;
        Ok((ItemKey::num(self.next_key), next))
    }

    /// Settings equality: the `last_used` display stamp is bookkeeping.
    fn same_settings(a: &Connection, b: &Connection) -> bool {
        let mut a = a.clone();
        a.last_used.clone_from(&b.last_used);
        a == *b
    }

    /// Replace one connection's settings. Equal settings are a no-op and keep the
    /// revision; a refused edit changes nothing. Returns whether it changed.
    fn edit_settings(
        &mut self,
        key: ItemKey,
        value: Connection,
    ) -> Result<bool, ConnectionMutationError> {
        let Some(i) = self.position(key) else {
            return Ok(false);
        };
        if Self::same_settings(&self.entries[i].value, &value) {
            return Ok(false);
        }
        let revision = self.entries[i]
            .revision
            .checked_add(1)
            .ok_or(ConnectionMutationError::IdentityExhausted)?;
        self.entries[i].value = value;
        self.entries[i].revision = revision;
        if matches!(&self.state, ConnState::Failed { key: k, .. } if *k == key) {
            self.state = ConnState::Idle;
        }
        Ok(true)
    }

    /// Persist a Safe Mode change made from an open workbench.
    pub fn set_safe_mode(
        &mut self,
        key: ItemKey,
        level: SafeMode,
    ) -> Result<bool, ConnectionMutationError> {
        let Some(mut value) = self.snapshot(key) else {
            return Ok(false);
        };
        value.safe_mode = level;
        self.edit_settings(key, value)
    }

    fn set_last_used(&mut self, key: ItemKey, stamp: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.value.last_used = stamp.into();
        }
    }

    fn rebuild_tree(&mut self) {
        let mut groups: Vec<String> = Vec::new();
        for e in &self.entries {
            if !groups.contains(&e.value.group) {
                groups.push(e.value.group.clone());
            }
        }
        let mut tree_keys = Vec::with_capacity(groups.len());
        let nodes: Vec<TreeNode> = groups
            .iter()
            .map(|g| {
                let members: Vec<&ConnectionEntry> = self
                    .entries
                    .iter()
                    .filter(|e| &e.value.group == g)
                    .collect();
                tree_keys.push(members.iter().map(|e| e.key).collect());
                let children: Vec<TreeNode> = members
                    .iter()
                    .map(|e| {
                        let c = &e.value;
                        let env = c.environment.kind();
                        TreeNode::leaf(&c.name)
                            .glyph(self.glyphs.env(env))
                            .glyph_tone(env.tone())
                            .meta(c.engine.short())
                    })
                    .collect();
                TreeNode::dir(g, children)
            })
            .collect();
        self.tree_keys = tree_keys;
        let filter = self.tree.filter.clone();
        let cursor = self.tree.cursor;
        self.tree = TreeView::new(ID.sub("tree"), nodes);
        self.tree.expand_all();
        self.tree.filter = filter;
        self.tree.flatten();
        self.tree.cursor = cursor.min(self.tree.rows().len().saturating_sub(1));
        self.sync_selected_from_cursor();
    }

    fn connection_at_path(&self, path: &[usize]) -> Option<ItemKey> {
        let [group, child] = path else {
            return None;
        };
        self.tree_keys.get(*group)?.get(*child).copied()
    }

    fn sync_selected_from_cursor(&mut self) {
        let rows = self.tree.rows();
        if let Some(row) = rows.get(self.tree.cursor)
            && let Some(key) = self.connection_at_path(&row.path.clone())
        {
            self.selected = Some(key);
        }
    }

    pub fn tree(&self) -> &TreeView {
        &self.tree
    }

    pub fn tree_mut(&mut self) -> &mut TreeView {
        &mut self.tree
    }

    pub fn key_at_row(&self, row: usize) -> Option<ItemKey> {
        let path = self.tree.rows().get(row)?.path.clone();
        self.connection_at_path(&path)
    }

    pub fn select_row(&mut self, row: usize) {
        use junie_tui::ui::context::ContextSource;
        self.tree.context_select(row);
        self.sync_selected_from_cursor();
    }

    /// Select the row showing `key`; false when it is gone or filtered out.
    pub fn select_key(&mut self, key: ItemKey) -> bool {
        let row = (0..self.tree.rows().len()).find(|&r| self.key_at_row(r) == Some(key));
        match row {
            Some(row) => {
                self.select_row(row);
                true
            }
            None => false,
        }
    }

    pub fn selected_connection(&self) -> Option<&Connection> {
        self.selected.and_then(|k| self.get(k)).map(|e| &e.value)
    }

    /// Drop the form-scoped test result; connect attempts are not form state.
    fn clear_form_state(&mut self) {
        if matches!(self.state, ConnState::Testing { .. } | ConnState::Tested(_)) {
            self.state = ConnState::Idle;
        }
    }

    pub fn form_dirty(&self) -> bool {
        self.form.as_ref().is_some_and(ConnForm::dirty)
    }

    pub fn form_stakes(&self) -> Stakes {
        if self.form_dirty() {
            Stakes::Unsaved { changes: 0 }
        } else {
            Stakes::Clean
        }
    }

    pub fn leave_form(&mut self, to: FormLeave, cx: &mut crate::app::Cx) -> Outcome {
        let stakes = self.form_stakes();
        match self
            .leave
            .request(Leave::To(to), &stakes, &junie_tui::theme::Strings::DEFAULT)
        {
            Ok(_) => self.go_to(to, cx),
            Err(d) => cx.open_dialog(*d),
        }
        Outcome::Changed
    }

    fn go_to(&mut self, to: FormLeave, cx: &mut crate::app::Cx) {
        match to {
            FormLeave::Close => {
                self.form = None;
                self.clear_form_state();
                cx.focus.focus(self.tree.id);
            }
            FormLeave::Edit(key) => {
                self.open_form(Some(key));
                if let Some(form) = self.form.as_ref() {
                    cx.focus.focus(form.name.id);
                }
            }
            FormLeave::New => {
                self.open_new();
                if let Some(form) = self.form.as_ref() {
                    cx.focus.focus(form.name.id);
                }
            }
        }
    }

    pub fn resolve_leave(
        &mut self,
        result: junie_tui::widgets::dialog::DialogResult,
        cx: &mut crate::app::Cx,
    ) -> bool {
        let to = match self.leave.resolve(result) {
            Resolved::Stay => return false,
            Resolved::Discard(to) => to,
            Resolved::Save(to) => {
                self.submit(false, cx);
                if self.form.is_some() {
                    return false;
                }
                to
            }
        };
        match to {
            Leave::Quit => {
                self.form = None;
                self.clear_form_state();
                true
            }
            Leave::To(to) => {
                self.go_to(to, cx);
                false
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        if self.filter.editing {
            return true;
        }
        self.form.as_ref().is_some_and(|f| {
            f.name.editing
                || f.host.editing
                || f.port.editing
                || f.database.editing
                || f.user.editing
                || f.password.editing
                || f.ssh_host.editing
                || f.ssh_user.editing
                || f.startup.editing
        })
    }

    pub fn animating(&self) -> bool {
        matches!(
            self.state,
            ConnState::Connecting(_) | ConnState::Testing { .. }
        )
    }

    /// Begin a simulated attempt on `key`, capturing its settings now. A repeated
    /// start supersedes the earlier attempt. A refused start changes nothing.
    pub fn start_connect(&mut self, key: ItemKey) -> Result<(), ConnectionMutationError> {
        let Some(entry) = self.get(key) else {
            return Ok(());
        };
        let generation = self
            .last_generation
            .checked_add(1)
            .ok_or(ConnectionMutationError::IdentityExhausted)?;
        let request = ConnectRequest {
            connection: key,
            revision: entry.revision,
            generation,
            snapshot: entry.value.clone(),
            ticks_left: CONNECT_TICKS,
        };
        self.last_generation = generation;
        self.selected = Some(key);
        self.state = ConnState::Connecting(request);
        Ok(())
    }

    fn begin_connect(&mut self, key: ItemKey, cx: &mut crate::app::Cx) {
        if let Err(e) = self.start_connect(key) {
            cx.status_as(junie_tui::theme::StatusKind::Warn, e.message());
        }
    }

    pub fn open_form(&mut self, target: Option<ItemKey>) {
        let entry = match target {
            Some(key) => match self.get(key) {
                Some(e) => Some((key, e.revision, e.value.clone())),
                None => return,
            },
            None => None,
        };
        self.form = Some(match entry {
            Some((key, revision, value)) => ConnForm::new(Some(&value), Some(key), revision),
            None => ConnForm::new(None, None, 0),
        });
    }

    pub fn open_new(&mut self) {
        self.open_form(None);
    }

    pub fn tick(&mut self) -> Option<ConnEvent> {
        match &mut self.state {
            ConnState::Connecting(req) => {
                req.ticks_left = req.ticks_left.saturating_sub(1);
                if req.ticks_left == 0 {
                    let generation = req.generation;
                    return self.complete(generation);
                }
            }
            ConnState::Testing { ticks } => {
                *ticks += 1;
                if *ticks >= 10 {
                    let ok = self
                        .form
                        .as_ref()
                        .is_some_and(|f| !f.host.text().contains("analytics"));
                    self.state = ConnState::Tested(if ok {
                        Ok("Connected · PostgreSQL 16.3 · 12 ms".into())
                    } else {
                        Err("Connection timed out after 10 s".into())
                    });
                }
            }
            _ => {}
        }
        None
    }

    /// Admit the completion of attempt `generation` exactly once. A completion
    /// for any other generation, or after the attempt ended, emits nothing.
    pub fn complete(&mut self, generation: u64) -> Option<ConnEvent> {
        let req = match std::mem::replace(&mut self.state, ConnState::Idle) {
            ConnState::Connecting(req) if req.generation == generation => req,
            other => {
                self.state = other;
                return None;
            }
        };
        let Some(entry) = self.get(req.connection) else {
            return Some(ConnEvent::Cancelled(Cancelled::Deleted));
        };
        if entry.revision != req.revision {
            return Some(ConnEvent::Cancelled(Cancelled::Changed));
        }
        match req.snapshot.outcome {
            ConnectOutcome::Ok => {
                self.set_last_used(req.connection, "just now");
                let mut connection = req.snapshot;
                connection.last_used = "just now".into();
                Some(ConnEvent::Connected {
                    key: req.connection,
                    generation,
                    connection: Box::new(connection),
                })
            }
            ConnectOutcome::AuthFailed => {
                self.state = ConnState::Failed {
                    key: req.connection,
                    revision: req.revision,
                    message: "Authentication failed".into(),
                    detail: "FATAL: password authentication failed for user \"acme_app\" (SQLSTATE 28P01). Check the password in the keychain or use “Prompt for password”.".into(),
                };
                None
            }
            ConnectOutcome::Unreachable => {
                self.state = ConnState::Failed {
                    key: req.connection,
                    revision: req.revision,
                    message: "Could not reach the host".into(),
                    detail: "Connection timed out after 10 s (analytics.acme.io:3306). The host may be behind a VPN or the port may be blocked.".into(),
                };
                None
            }
        }
    }

    pub fn on_key(&mut self, key: &Key, cx: &mut crate::app::Cx) -> (Outcome, Option<ConnEvent>) {
        let text_focus = self.form_text_focus(cx.focus.current());
        if key.is_char('e')
            && !self.is_editing()
            && letters_allowed(KeyContext {
                text_focus,
                ..KeyContext::default()
            })
            && let Some(key) = self.selected
        {
            return (self.leave_form(FormLeave::Edit(key), cx), None);
        }
        let Some(f) = cx.focus.current() else {
            return (Outcome::Ignored, None);
        };
        if f == self.filter.id {
            let (o, ev) = self.filter.on_key(key);
            match ev {
                Some(InputAction::Changed)
                | Some(InputAction::Committed)
                | Some(InputAction::Cancelled) => {
                    let q = self.filter.text().to_owned();
                    self.tree
                        .set_filter(if q.is_empty() { None } else { Some(&q) });
                    self.sync_selected_from_cursor();
                }
                Some(InputAction::CommittedTab { backward }) => {
                    if backward {
                        cx.focus_prev()
                    } else {
                        cx.focus_next()
                    }
                }
                None => {}
            }
            if !o.consumed() && key.is(KeyCode::Down) {
                cx.focus.focus(self.tree.id);
                return (Outcome::Changed, None);
            }
            return (o, None);
        }
        if f == self.tree.id {
            if key.is_char('e') {
                let event = self.action(1, cx);
                return (Outcome::Changed, event);
            }
            if key.is_char('d') {
                let event = self.action(3, cx);
                return (Outcome::Changed, event);
            }
            if key.alt() && key.code == KeyCode::Char('d') {
                let event = self.action(2, cx);
                return (Outcome::Changed, event);
            }
            if key.is_char('/') {
                cx.focus.focus(self.filter.id);
                self.filter.begin_edit();
                return (Outcome::Changed, None);
            }
            let (o, ev) = self.tree.on_key(key);
            self.sync_selected_from_cursor();
            if let Some(TreeAction::Activate(path)) = ev
                && let Some(key) = self.connection_at_path(&path)
            {
                self.begin_connect(key, cx);
            }
            return (o, None);
        }
        if let Some(_form) = self.form.as_mut() {
            let o = self.on_form_key(key, f, cx);

            if o == Outcome::Ignored && key.is(KeyCode::Esc) {
                return (self.leave_form(FormLeave::Close, cx), None);
            }
            return (o, None);
        }

        if let Some(o) = self.md.on_key(key, cx.focus) {
            return (o, None);
        }
        let mut hit: Option<(Outcome, bool, usize)> = None;
        for (b, action) in [
            (&mut self.connect_btn, 0),
            (&mut self.edit_btn, 1),
            (&mut self.dup_btn, 2),
            (&mut self.del_btn, 3),
            (&mut self.retry_btn, 4),
        ] {
            if b.id == f {
                let (o, act) = b.on_key(key);
                hit = Some((o, act, action));
                break;
            }
        }
        match hit {
            Some((_, true, action)) => (Outcome::Changed, self.action(action, cx)),
            Some((o, false, _)) => (o, None),
            None => (Outcome::Ignored, None),
        }
    }

    fn action(&mut self, which: usize, cx: &mut crate::app::Cx) -> Option<ConnEvent> {
        match which {
            0 => {
                if let Some(key) = self.selected {
                    self.begin_connect(key, cx);
                }
            }
            4 => {
                let retry = match &self.state {
                    ConnState::Failed { key, .. } => Some(*key),
                    _ => self.selected,
                };
                if let Some(key) = retry {
                    self.begin_connect(key, cx);
                }
            }
            1 => {
                self.open_form(self.selected);
                if let Some(f) = &self.form {
                    cx.focus.focus(f.name.id);
                }
            }
            2 => {
                if let Some(key) = self.selected {
                    match self.duplicate(key) {
                        Ok(true) => cx.status("Duplicated"),
                        Ok(false) => {}
                        Err(e) => cx.status_as(junie_tui::theme::StatusKind::Warn, e.message()),
                    }
                }
            }
            3 => {
                if let Some(entry) = self.selected.and_then(|k| self.get(k)) {
                    let (key, c) = (entry.key, &entry.value);
                    let d = junie_tui::widgets::dialog::Dialog::destructive(
                        ID.sub("delete-dialog"),
                        "Delete connection?",
                        &format!(
                            "{} ({}@{}) will be removed from connections.json. Its password stays in the keychain until you remove it there.",
                            c.name, c.user, c.host
                        ),
                        "Delete",
                    );
                    self.delete_target = Some(key);
                    cx.open_dialog(d);
                }
            }
            _ => {}
        }
        None
    }

    /// Insert a copy after `key` under a fresh key. The copy keeps the original's
    /// label plus " (Copy)"; labels are not identity, so repeats may coincide.
    fn duplicate(&mut self, key: ItemKey) -> Result<bool, ConnectionMutationError> {
        let Some(i) = self.position(key) else {
            return Ok(false);
        };
        let (new_key, next_key) = self.reserve_key()?;
        let mut value = self.entries[i].value.clone();
        value.name = format!("{} (Copy)", value.name);
        value.last_used = "never".into();
        self.entries.insert(
            i + 1,
            ConnectionEntry {
                key: new_key,
                value,
                revision: 0,
            },
        );
        self.next_key = next_key;
        self.rebuild_tree();
        Ok(true)
    }

    fn delete(&mut self, key: ItemKey) {
        let Some(i) = self.position(key) else {
            return;
        };
        self.entries.remove(i);
        if self.selected == Some(key) {
            self.selected = self
                .entries
                .get(i.min(self.entries.len().saturating_sub(1)))
                .map(ConnectionEntry::key);
        }
        if matches!(&self.state, ConnState::Failed { key: k, .. } if *k == key) {
            self.state = ConnState::Idle;
        }
        self.rebuild_tree();
    }

    pub fn on_dialog_closed(
        &mut self,
        id: WidgetId,
        result: junie_tui::widgets::dialog::DialogResult,
    ) -> bool {
        if id == ID.sub("delete-dialog") {
            let target = self.delete_target.take();
            if result == junie_tui::widgets::dialog::DialogResult::Action(1)
                && let Some(key) = target
            {
                self.delete(key);
            }
            return true;
        }
        false
    }

    fn form_text_focus(&self, focus: Option<WidgetId>) -> bool {
        let (Some(form), Some(f)) = (self.form.as_ref(), focus) else {
            return false;
        };
        [
            form.name.id,
            form.host.id,
            form.port.id,
            form.database.id,
            form.user.id,
            form.password.id,
            form.ssh_host.id,
            form.ssh_user.id,
            form.startup.id,
        ]
        .contains(&f)
    }

    fn on_form_key(&mut self, key: &Key, f: WidgetId, cx: &mut crate::app::Cx) -> Outcome {
        let Some(form) = self.form.as_mut() else {
            return Outcome::Ignored;
        };
        if key.ctrl_char('s') {
            return self.submit(false, cx);
        }
        if f == form.tabs.id {
            return form.tabs.on_key(key).0;
        }
        macro_rules! input {
            ($w:expr) => {{
                let (o, ev) = $w.on_key(key);
                match ev {
                    Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                    Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                    _ => {}
                }
                return o;
            }};
        }
        if f == form.name.id {
            input!(form.name);
        }
        if f == form.host.id {
            input!(form.host);
        }
        if f == form.port.id {
            input!(form.port);
        }
        if f == form.database.id {
            input!(form.database);
        }
        if f == form.user.id {
            input!(form.user);
        }
        if f == form.password.id {
            input!(form.password);
        }
        if f == form.ssh_host.id {
            input!(form.ssh_host);
        }
        if f == form.ssh_user.id {
            input!(form.ssh_user);
        }
        if f == form.startup.id {
            input!(form.startup);
        }
        if f == form.engine.id {
            let (o, ev) = form.engine.on_key(key);
            if let Some(junie_tui::widgets::select::SelectAction::Changed(i)) = ev {
                let port = match i {
                    0 => "5432",
                    1 => "3306",
                    _ => "",
                };
                form.port = TextInput::new(form.port.id, "Port")
                    .value(port)
                    .validator(port_validator);
            }
            return o;
        }
        if f == form.group.id {
            return form.group.on_key(key).0;
        }
        if f == form.env.id {
            return form.env.on_key(key);
        }
        if f == form.safe.id {
            return form.safe.on_key(key);
        }
        if f == form.prompt_pw.id {
            return form.prompt_pw.on_key(key);
        }
        if f == form.ssl.id {
            return form.ssl.on_key(key);
        }
        if f == form.ssh.id {
            return form.ssh.on_key(key);
        }
        if f == form.local_only.id {
            return form.local_only.on_key(key);
        }
        if f == form.test_btn.id {
            let (o, act) = form.test_btn.on_key(key);
            if act {
                self.state = ConnState::Testing { ticks: 0 };
            }
            return o;
        }
        if f == form.cancel_btn.id {
            let (o, act) = form.cancel_btn.on_key(key);
            if act {
                self.form = None;
                self.clear_form_state();
                cx.focus.focus(self.tree.id);
            }
            return o;
        }
        if f == form.save_btn.id {
            let (o, act) = form.save_btn.on_key(key);
            if act {
                return self.submit(false, cx);
            }
            return o;
        }
        if f == form.save_connect_btn.id {
            let (o, act) = form.save_connect_btn.on_key(key);
            if act {
                return self.submit(true, cx);
            }
            return o;
        }
        Outcome::Ignored
    }

    fn submit(&mut self, connect: bool, cx: &mut crate::app::Cx) -> Outcome {
        let Some(form) = self.form.as_mut() else {
            return Outcome::Ignored;
        };
        for inp in [
            &mut form.name,
            &mut form.host,
            &mut form.port,
            &mut form.database,
            &mut form.user,
        ] {
            if inp.editing {
                inp.commit();
            }
        }
        if !form.validate() {
            cx.status_as(
                junie_tui::theme::StatusKind::Warn,
                "Fix the highlighted fields",
            );
            cx.focus.focus(if form.name.field.error.is_some() {
                form.name.id
            } else {
                form.port.id
            });
            return Outcome::Changed;
        }
        let (target, expected) = (form.target, form.expected_revision);
        let base = match target {
            Some(key) => match self.entries.iter().find(|e| e.key == key) {
                Some(e) if e.revision == expected => Some(e.value.clone()),
                Some(_) => {
                    cx.status_as(
                        junie_tui::theme::StatusKind::Warn,
                        "Connection changed since this form opened; reopen it to edit",
                    );
                    return Outcome::Changed;
                }
                None => {
                    cx.status_as(
                        junie_tui::theme::StatusKind::Warn,
                        "Connection no longer exists",
                    );
                    return Outcome::Changed;
                }
            },
            None => None,
        };
        let c = form.to_connection(base.as_ref());
        let saved = match target {
            Some(key) => self.edit_settings(key, c).map(|_| key),
            None => self.insert_new(c),
        };
        let key = match saved {
            Ok(key) => key,
            Err(e) => {
                cx.status_as(junie_tui::theme::StatusKind::Warn, e.message());
                return Outcome::Changed;
            }
        };
        self.form = None;
        self.clear_form_state();
        self.rebuild_tree();
        self.selected = Some(key);
        cx.status_as(junie_tui::theme::StatusKind::Ok, "Connection saved");
        if connect {
            self.begin_connect(key, cx);
        } else {
            cx.focus.focus(self.tree.id);
        }
        Outcome::Changed
    }

    fn insert_new(&mut self, value: Connection) -> Result<ItemKey, ConnectionMutationError> {
        let (key, next_key) = self.reserve_key()?;
        self.entries.push(ConnectionEntry {
            key,
            value,
            revision: 0,
        });
        self.next_key = next_key;
        Ok(key)
    }

    fn pointer_fields(&mut self) -> Vec<&mut dyn PointerField> {
        let mut v: Vec<&mut dyn PointerField> = vec![&mut self.filter];
        if let Some(f) = self.form.as_mut() {
            v.extend([
                &mut f.name as &mut dyn PointerField,
                &mut f.host,
                &mut f.port,
                &mut f.database,
                &mut f.user,
                &mut f.password,
                &mut f.ssh_host,
                &mut f.ssh_user,
                &mut f.startup,
            ]);
        }
        v
    }

    pub fn pointer(
        &mut self,
        ev: &PointerEvent,
        cx: &mut crate::app::Cx,
    ) -> (Outcome, Option<ConnEvent>) {
        let (id, pos) = (ev.id, ev.pos());
        if let Some(o) = self.form_stack_pointer(ev) {
            return (o, None);
        }
        match ev.phase() {
            Some(Phase::Click) => return self.click(ev, cx),
            Some(Phase::Press) => {
                if let Some(o) = route_press(&mut self.pointer_fields(), id, pos, ev.count()) {
                    return (o, None);
                }
            }
            Some(Phase::Drag) => {
                if let Some(o) = route_drag(&mut self.pointer_fields(), id, pos) {
                    return (o, None);
                }
            }
            _ => {}
        }

        if let Some(r) = self.tree.route(ev) {
            return (r.outcome(), None);
        }
        if let Some(form) = self.form.as_mut() {
            for sel in [&mut form.engine, &mut form.group] {
                if let Some(r) = sel.route(ev) {
                    return (r.outcome(), None);
                }
            }
        }
        (Outcome::Ignored, None)
    }

    fn click(
        &mut self,
        ev: &PointerEvent,
        cx: &mut crate::app::Cx,
    ) -> (Outcome, Option<ConnEvent>) {
        let (id, pos) = (ev.id, ev.pos());

        if id == NEW_ID {
            self.open_new();
            return (Outcome::Changed, None);
        }
        if let Some(o) = route_click(&mut self.pointer_fields(), id, pos, ev.count()) {
            cx.focus.focus(id);
            return (o, None);
        }
        if ev.on(self.tree.id) {
            if self.tree.row_at(ev).is_some() {
                cx.focus.focus(self.tree.id);
            }
            let (o, act) = self.tree.pointer(ev).into_parts();
            self.sync_selected_from_cursor();

            if let Some(TreeAction::Activate(path)) = act
                && let Some(key) = self.connection_at_path(&path)
            {
                self.begin_connect(key, cx);
            }
            return (o, None);
        }
        if self.form.is_some() {
            return (self.on_form_click(ev, cx), None);
        }
        let mut hit: Option<(bool, usize)> = None;
        for (b, action) in [
            (&mut self.connect_btn, 0),
            (&mut self.edit_btn, 1),
            (&mut self.dup_btn, 2),
            (&mut self.del_btn, 3),
            (&mut self.retry_btn, 4),
        ] {
            if b.id == id {
                hit = Some((b.on_click(), action));
                break;
            }
        }
        match hit {
            Some((true, action)) => (Outcome::Changed, self.action(action, cx)),
            Some((false, _)) => (Outcome::Changed, None),
            None => (Outcome::Ignored, None),
        }
    }

    fn form_stack_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        let form = self.form.as_mut()?;
        let tab = form.tabs.active.min(1);
        let f = ID.sub("form");
        [f.sub("left"), f.sub("right")]
            .into_iter()
            .zip(form.stacks[tab].iter_mut())
            .find_map(|(sid, st)| st.pointer(sid, ev))
    }

    fn on_form_click(&mut self, ev: &PointerEvent, cx: &mut crate::app::Cx) -> Outcome {
        let id = ev.id;
        let Some(form) = self.form.as_mut() else {
            return Outcome::Ignored;
        };
        if let Some(r) = form.tabs.route(ev) {
            cx.focus.focus(form.tabs.id);
            return r.outcome();
        }

        for sel in [&mut form.engine, &mut form.group] {
            if let Some(r) = sel.route(ev) {
                cx.focus.focus(sel.id);
                return r.outcome();
            }
        }
        for rg in [&mut form.env, &mut form.safe] {
            if let Some(r) = rg.route(ev) {
                cx.focus.focus(rg.id);
                return r.outcome();
            }
        }
        if form.prompt_pw.id == id {
            return form.prompt_pw.on_click();
        }
        for tg in [&mut form.ssl, &mut form.ssh, &mut form.local_only] {
            if tg.id == id {
                return tg.on_click();
            }
        }
        if form.test_btn.id == id && form.test_btn.on_click() {
            self.state = ConnState::Testing { ticks: 0 };
            return Outcome::Changed;
        }
        if form.cancel_btn.id == id && form.cancel_btn.on_click() {
            self.form = None;
            self.clear_form_state();
            cx.focus.focus(self.tree.id);
            return Outcome::Changed;
        }
        if form.save_btn.id == id && form.save_btn.on_click() {
            return self.submit(false, cx);
        }
        if form.save_connect_btn.id == id && form.save_connect_btn.on_click() {
            return self.submit(true, cx);
        }
        Outcome::Ignored
    }

    pub fn on_blur(&mut self, ch: junie_tui::core::focus::FocusChange) -> Outcome {
        use junie_tui::core::focus::forward_blur;
        let mut out = forward_blur(ch, &mut [&mut self.filter]);
        if let Some(f) = self.form.as_mut() {
            out = out.or(forward_blur(
                ch,
                &mut [
                    &mut f.name,
                    &mut f.engine,
                    &mut f.host,
                    &mut f.port,
                    &mut f.database,
                    &mut f.user,
                    &mut f.password,
                    &mut f.group,
                    &mut f.ssh_host,
                    &mut f.ssh_user,
                    &mut f.startup,
                ],
            ));
        }
        out
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if let Some(form) = self.form.as_mut() {
            for inp in [
                &mut form.name,
                &mut form.host,
                &mut form.port,
                &mut form.database,
                &mut form.user,
                &mut form.password,
                &mut form.ssh_host,
                &mut form.ssh_user,
            ] {
                if inp.editing {
                    return inp.on_paste(text);
                }
            }
            if form.startup.editing {
                return form.startup.on_paste(text);
            }
        }
        if self.filter.editing {
            return self.filter.on_paste(text);
        }
        Outcome::Ignored
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.form.is_some() {
            if self.is_editing() {
                return junie_tui::widgets::keyhint::EDIT_FIELD.to_vec();
            }
            return vec![
                junie_tui::widgets::widget_hints::EDIT,
                hint("←→", "Basic / Advanced"),
                hint("Ctrl+S", "Save"),
            ];
        }
        if focus == Some(self.filter.id) {
            return vec![
                hint("Type", "Filter"),
                hint("↓", "Into list"),
                hint("Esc", "Clear"),
            ];
        }
        vec![
            hint("↑↓", "Move"),
            hint("Enter", "Connect"),
            hint("e", "Edit…"),
            hint("d", "Delete…"),
            hint("Alt+D", "Duplicate"),
            hint("/", "Filter"),
            hint("Ctrl+N", "New"),
            junie_tui::widgets::keyhint::CONTEXT_MENU,
        ]
        .into_iter()
        .chain(self.md.hints(focus))
        .collect()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        if self.glyphs != t.glyphs {
            self.glyphs = t.glyphs;
            self.rebuild_tree();
        }
        let md = self.md.layout(area, buf, ctx, t.canvas);
        let (l, r) = match md.mode {
            Mode::Split => (md.master, md.detail),
            Mode::Master => (md.master, Rect::ZERO),
            Mode::Drawer => (Rect::ZERO, md.detail),
        };
        if md.mode != Mode::Drawer {
            self.render_list(l, buf, ctx);
        }
        if r.is_empty() {
            if self.form.is_some() {
                ctx.cover(buf);
                self.render_form(area, buf, ctx);
            } else if self.selected_connection().is_some() {
                self.md.finish(ctx);
            }
            return;
        }
        if self.form.is_some() {
            self.render_form(r, buf, ctx);
            return;
        }
        self.render_detail(r, buf, ctx);
    }

    fn render_list(&mut self, l: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let lf = ctx.interaction.focused(self.tree.id) || ctx.interaction.focused(self.filter.id);
        let count = format!("{}", self.entries.len());
        let panel = Panel::framed(Some("Connections")).focused(lf).meta(&count);
        let bg = panel.bg(t);
        let inner = panel.render(l, buf, t);
        self.filter.render(
            Rect::new(inner.x.saturating_sub(1), inner.y, inner.width + 1, 2),
            buf,
            ctx,
            bg,
        );
        let tree_area = Rect::new(
            inner.x.saturating_sub(1),
            inner.y + 2,
            inner.width + 1,
            inner.height.saturating_sub(2),
        );
        if self.entries.is_empty() {
            junie_tui::widgets::empty::render_ctx(
                tree_area,
                buf,
                ctx,
                &junie_tui::widgets::empty::EmptyState::new("No connections")
                    .hint("Ctrl+N creates one")
                    .actions(vec![junie_tui::widgets::empty::EmptyAction::primary(
                        NEW_ID,
                        "New connection",
                    )]),
                bg,
            );
        } else {
            self.tree.render(tree_area, buf, ctx, bg);
        }
    }

    fn render_detail(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let Some((key, revision, c)) = self
            .selected
            .and_then(|k| self.get(k))
            .map(|e| (e.key, e.revision, e.value.clone()))
        else {
            DetailCard::empty(
                junie_tui::widgets::empty::EmptyState::new("Select a connection")
                    .hint("↑↓ moves, Ctrl+N creates one"),
            )
            .render(r, buf, ctx, t.canvas, &mut []);
            return;
        };

        let ssl = if c.ssl {
            let ok = junie_tui::theme::StatusKind::Ok;
            Prop::new("SSL", format!("{} on", t.status(ok).0)).tone(ok.tone())
        } else {
            Prop::new("SSL", "off").tone(Tone::Secondary)
        };
        let ssh = c.ssh.clone().unwrap_or("off".into());
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
            Prop::new("Environment", c.environment.label()).tone(env_tone(c.environment)),
            Prop::new(
                "Safe Mode",
                format!("{} · {}", c.safe_mode.label(), c.safe_mode.description()),
            )
            .tone(if c.safe_mode >= SafeMode::Safe {
                Tone::Normal
            } else {
                Tone::Secondary
            })
            .wrap(),
            ssl,
            Prop::new("SSH", ssh).tone(Tone::Secondary),
            Prop::new("Last used", c.last_used.clone()).tone(Tone::Muted),
        ];
        if c.environment == Environment::Production && c.safe_mode == SafeMode::Silent {
            facts.insert(
                6,
                Prop::new(
                    "",
                    "Production with Silent safe mode: writes run without asking",
                )
                .tone(Tone::Warning)
                .wrap(),
            );
        }
        let connecting = matches!(&self.state, ConnState::Connecting(req) if req.connection == key);
        let failed = matches!(
            &self.state,
            ConnState::Failed { key: k, revision: r, .. } if *k == key && *r == revision
        );
        self.connect_btn.busy = connecting;
        self.connect_btn.label = if failed {
            "Reconnect".into()
        } else {
            "Connect".into()
        };

        let l = DetailCard::new(&c.name)
            .props(facts)
            .max_size(Some(70), Some(17))
            .render(r, buf, ctx, t.canvas, &mut []);
        let (inner, bg) = (l.slot, l.bg);
        let sy = l.slot.y;
        match &self.state {
            ConnState::Connecting(req) if req.connection == key => {
                let phase = match CONNECT_TICKS - req.ticks_left {
                    0..=3 => "Opening SSH tunnel…",
                    4..=7 => "Authenticating…",
                    _ => "Loading schema…",
                };
                junie_tui::widgets::progress::render_spinner(
                    Rect::new(inner.x, sy, inner.width, 1),
                    buf,
                    ctx,
                    phase,
                    bg,
                );
            }
            ConnState::Failed {
                key: k,
                revision: r,
                message,
                detail,
            } if *k == key && *r == revision => {
                Notice::error(message.as_str())
                    .detail(detail.as_str())
                    .message_lines(1)
                    .detail_tone(Tone::Muted)
                    .render(Rect::new(inner.x, sy, inner.width, 3), buf, ctx, bg);
            }
            _ => {}
        }
        ActionRow::new().render(
            Rect::new(l.slot.x, l.slot.bottom(), l.slot.width, 1),
            buf,
            ctx,
            bg,
            &mut [
                &mut self.connect_btn,
                &mut self.edit_btn,
                &mut self.dup_btn,
                &mut self.del_btn,
            ],
        );
    }

    fn render_form(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let title = if form.target.is_some() {
            "Edit connection"
        } else {
            "New connection"
        };
        let panel = Panel::card(Some(title)).meta("Ctrl+S Save");
        let bg = panel.bg(t);
        let inner = panel.render(Rect::new(r.x, r.y, r.width.min(84), r.height), buf, t);

        let error = form.name.field.error.is_some() || form.port.field.error.is_some();
        form.tabs.update_item(0, |it| it.error = error);
        let (strip, body) = Tabs::layout(inner);
        form.tabs.render(strip, buf, ctx, bg);

        let body = Rect {
            height: body.height.saturating_sub(2),
            ..body
        };
        let (lc, rc) = junie_tui::ui::layout::Split::new(58, 30, 24).horizontal(body, 4);
        let fh = TextInput::HEIGHT;

        let stack = Stack::default();
        let focus = ctx.interaction.focus;
        let tab = form.tabs.active.min(1);
        let lid = ID.sub("form").sub("left");
        let rid = ID.sub("form").sub("right");
        if tab == 0 {
            let lkids = [
                StackChild::fixed(fh),
                StackChild::fixed(Select::HEIGHT),
                StackChild::fixed(fh),
                StackChild::fixed(fh),
                StackChild::fixed(fh),
                StackChild::fixed(fh),
                StackChild::fixed(1),
            ];
            let lfocus = [
                &[form.name.id][..],
                &[form.engine.id],
                &[form.host.id, form.port.id],
                &[form.database.id],
                &[form.user.id],
                &[form.password.id],
                &[form.prompt_pw.id],
            ]
            .iter()
            .position(|ids| focus.is_some_and(|f| ids.contains(&f)));
            let l = stack.layout(lc, &lkids, &mut form.stacks[0][0], lfocus);
            Stack::chrome(&l, lc, buf, ctx, lid, &form.stacks[0][0]);
            match l.slot(0) {
                Some(r) => form.name.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.name.id),
            }
            match l.slot(1) {
                Some(r) => form.engine.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.engine.id),
            }
            match l.slot(2) {
                Some(r) => {
                    let (hl, hr) = junie_tui::ui::layout::Split::new(70, 12, 8).horizontal(r, 2);
                    form.host.render(hl, buf, ctx, bg);
                    form.port.render(hr, buf, ctx, bg);
                }
                None => {
                    ctx.focus_stop(form.host.id);
                    ctx.focus_stop(form.port.id);
                }
            }
            for (i, inp) in [&mut form.database, &mut form.user, &mut form.password]
                .into_iter()
                .enumerate()
            {
                match l.slot(3 + i) {
                    Some(r) => inp.render(r, buf, ctx, bg),
                    None => ctx.focus_stop(inp.id),
                }
            }
            match l.slot(6) {
                Some(r) => form.prompt_pw.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.prompt_pw.id),
            }

            let desc = SafeMode::ALL[form.safe.selected.min(5)].description();
            let desc_lines: Vec<String> =
                junie_tui::ui::text::wrap(desc, rc.width.saturating_sub(2) as usize)
                    .into_iter()
                    .take(2)
                    .collect();
            let rkids = [
                StackChild::fixed(form.env.height()),
                StackChild::fixed(1),
                StackChild::fixed(Select::HEIGHT),
                StackChild::fixed(form.safe.height()),
                StackChild::flex(0, desc_lines.len() as u16),
            ];
            let rfocus = [form.env.id, form.env.id, form.group.id, form.safe.id]
                .iter()
                .position(|&id| focus == Some(id));
            let rl = stack.layout(rc, &rkids, &mut form.stacks[0][1], rfocus);
            Stack::chrome(&rl, rc, buf, ctx, rid, &form.stacks[0][1]);
            match rl.slot(0) {
                Some(r) => form.env.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.env.id),
            }
            match rl.slot(2) {
                Some(r) => form.group.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.group.id),
            }
            match rl.slot(3) {
                Some(r) => form.safe.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.safe.id),
            }
            if let Some(r) = rl.slot(4) {
                let rows = r.height.min(desc_lines.len() as u16);
                let area = Rect::new(r.x + 2, r.y, r.width.saturating_sub(2), rows);
                let focused = focus == Some(form.safe.id);
                let lines = junie_tui::ui::reveal::clamp(ctx, area, desc, focused);
                for (i, line) in lines.iter().enumerate() {
                    buf.set_string(area.x, area.y + i as u16, line, t.muted().bg(bg));
                }
            }
        } else {
            form.ssh_host.disabled = !form.ssh.on;
            form.ssh_user.disabled = !form.ssh.on;
            let lkids = [
                StackChild::fixed(1),
                StackChild::fixed(1),
                StackChild::fixed(1),
                StackChild::fixed(fh),
                StackChild::fixed(fh),
                StackChild::fixed(1),
            ];
            let lfocus = [
                form.ssl.id,
                form.ssl.id,
                form.ssh.id,
                form.ssh_host.id,
                form.ssh_user.id,
                form.local_only.id,
            ]
            .iter()
            .position(|&id| focus == Some(id));
            let l = stack.layout(lc, &lkids, &mut form.stacks[1][0], lfocus);
            Stack::chrome(&l, lc, buf, ctx, lid, &form.stacks[1][0]);
            match l.slot(0) {
                Some(r) => form.ssl.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.ssl.id),
            }
            match l.slot(2) {
                Some(r) => form.ssh.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.ssh.id),
            }
            for (i, inp) in [&mut form.ssh_host, &mut form.ssh_user]
                .into_iter()
                .enumerate()
            {
                match l.slot(3 + i) {
                    Some(r) => inp.render(r, buf, ctx, bg),
                    None if !inp.disabled => ctx.focus_stop(inp.id),
                    None => {}
                }
            }
            match l.slot(5) {
                Some(r) => form.local_only.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.local_only.id),
            }

            let rkids = [
                StackChild::flex(3, form.startup.height()),
                StackChild::fixed(1),
                StackChild::fixed(1),
            ];
            let rfocus = (focus == Some(form.startup.id)).then_some(0);
            let rl = stack.layout(rc, &rkids, &mut form.stacks[1][1], rfocus);
            Stack::chrome(&rl, rc, buf, ctx, rid, &form.stacks[1][1]);
            match rl.slot(0) {
                Some(r) => form.startup.render(r, buf, ctx, bg),
                None => ctx.focus_stop(form.startup.id),
            }
            if let Some(r) = rl.slot(2) {
                buf.set_string(
                    r.x + 2,
                    r.y,
                    junie_tui::ui::reveal::cut(
                        Rect::new(r.x + 2, r.y, r.width.saturating_sub(2), 1),
                        "External clients: read only",
                    ),
                    t.muted().bg(bg),
                );
            }
        }

        let ay = inner.bottom().saturating_sub(1);
        form.test_btn.busy = matches!(self.state, ConnState::Testing { .. });
        let rects = ActionRow::new().render(
            Rect::new(inner.x, ay, inner.width, 1),
            buf,
            ctx,
            bg,
            &mut [
                &mut form.test_btn,
                &mut form.cancel_btn,
                &mut form.save_btn,
                &mut form.save_connect_btn,
            ],
        );
        if let ConnState::Tested(res) = &self.state {
            let (kind, m) = match res {
                Ok(m) => (junie_tui::theme::StatusKind::Ok, m),
                Err(e) => (junie_tui::theme::StatusKind::Fail, e),
            };
            let (glyph, gs) = t.status(kind);
            let x = rects[3].right() + 2;
            let room = inner.right().saturating_sub(x) as usize;
            let msg = junie_tui::ui::text::truncate(&format!("{glyph} {m}"), room);
            buf.set_string(x, ay, &msg, t.status_label(kind).bg(bg));
            if !msg.is_empty() {
                buf.set_string(x, ay, glyph, gs.bg(bg));
            }
        }
    }
}
