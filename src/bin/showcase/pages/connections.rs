use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent, Report};
use termrock::core::event::{Key, Outcome};
use termrock::core::focus::forward_blur;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::theme::{EnvKind, Glyphs, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layer::LayerTag;
use termrock::ui::layers::LayerResult;
use termrock::ui::layout::Split;
use termrock::ui::master_detail::MasterDetail;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::action_row::ActionRow;
use termrock::widgets::button::Button;
use termrock::widgets::detail_card::DetailCard;
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::empty::{self, EmptyAction, EmptyState};
use termrock::widgets::form_dialog::{FieldValue, FormAction, FormDialog, FormField};
use termrock::widgets::input::{InputAction, TextInput};
use termrock::widgets::notice::Notice;
use termrock::widgets::panel::Panel;
use termrock::widgets::progress::render_indeterminate;
use termrock::widgets::props::Prop;
use termrock::widgets::select::Select;
use termrock::widgets::tree::{TreeAction, TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("connections");
const FORM: WidgetId = ID.sub("form");
const DELETE_DLG: WidgetId = ID.sub("delete");
const NEW_EMPTY: WidgetId = ID.sub("new-empty");

const CONNECT_TICKS: u8 = 8;

const ENVS: &[&str] = &["production", "staging", "development", "local"];
const ENGINES: &[&str] = &["PostgreSQL", "MySQL", "SQLite"];

fn env_kind(i: usize) -> EnvKind {
    match i {
        0 => EnvKind::Production,
        1 => EnvKind::Staging,
        2 => EnvKind::Development,
        _ => EnvKind::Local,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Ok,
    AuthFailed,
    Unreachable,
}

#[derive(Debug, Clone)]
struct Conn {
    name: String,
    group: String,
    env: usize,
    engine: usize,
    host: String,
    database: String,
    user: String,
    ssl: bool,
    ssh: Option<String>,
    last_used: String,
    reach: Reach,
}

fn conn(
    name: &str,
    group: &str,
    env: usize,
    engine: usize,
    host: &str,
    database: &str,
    reach: Reach,
) -> Conn {
    Conn {
        name: name.into(),
        group: group.into(),
        env,
        engine,
        host: host.into(),
        database: database.into(),
        user: if env == 3 {
            "postgres".into()
        } else {
            "acme_ops".into()
        },
        ssl: env < 2,
        ssh: (env == 0).then(|| "bastion.acme.io".into()),
        last_used: ["1 hour ago", "yesterday", "3 days ago", "just now"][env].into(),
        reach,
    }
}

fn sample() -> Vec<Conn> {
    vec![
        conn(
            "Local PostgreSQL",
            "Personal",
            3,
            0,
            "localhost:5432",
            "dev",
            Reach::Ok,
        ),
        conn(
            "Scratch",
            "Personal",
            3,
            2,
            "~/scratch.db",
            "main",
            Reach::Ok,
        ),
        conn(
            "Development",
            "Acme",
            2,
            0,
            "dev-db.acme.io:5432",
            "acme_dev",
            Reach::Ok,
        ),
        conn(
            "Staging",
            "Acme",
            1,
            0,
            "staging-db.acme.io:5432",
            "acme_staging",
            Reach::AuthFailed,
        ),
        conn(
            "Analytics",
            "Acme",
            0,
            1,
            "analytics.acme.io:3306",
            "warehouse",
            Reach::Unreachable,
        ),
        conn(
            "Production",
            "Acme",
            0,
            0,
            "prod-db-1.acme.io:5432",
            "acme_prod",
            Reach::Ok,
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnState {
    Idle,
    Connecting {
        name: String,
        left: u8,
    },
    Connected {
        name: String,
    },
    Failed {
        name: String,
        message: String,
        detail: String,
    },
}

pub fn page() -> Box<dyn Page> {
    Box::new(ConnectionsPage::new())
}

pub struct ConnectionsPage {
    template: Template,
    opts: PageValues,
    conns: Vec<Conn>,
    pub filter: TextInput,
    pub tree: TreeView,

    selected: Option<usize>,
    state: ConnState,
    pub connect: Button,
    pub edit: Button,
    pub duplicate: Button,
    pub delete: Button,
    pub new: Button,

    editing: Option<Option<usize>>,

    pub md: MasterDetail,
}

impl ConnectionsPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Connections)),
            conns: sample(),
            filter: TextInput::new(ID.sub("filter"), "")
                .placeholder("Filter connections")
                .plain_label(),
            tree: TreeView::new(ID.sub("tree"), vec![]),
            selected: None,
            state: ConnState::Idle,
            connect: Button::primary(ID.sub("connect"), "Connect"),
            edit: Button::secondary(ID.sub("edit"), "Edit…"),
            duplicate: Button::subtle(ID.sub("duplicate"), "Duplicate"),
            delete: Button::danger(ID.sub("delete-btn"), "Delete…"),
            new: Button::secondary(ID.sub("new"), "New connection…"),
            editing: None,
            md: master_detail(40),
        };
        page.rebuild_tree();

        page.select_name("Production");
        page
    }

    fn list_share(&self) -> u16 {
        self.opts.number("connections.list_width").unwrap_or(40)
    }

    fn rebuild_tree(&mut self) {
        let by_env = self.opts.value("connections.group_by") == "environment";
        let mut groups: Vec<String> = Vec::new();
        for c in &self.conns {
            let g = if by_env {
                ENVS[c.env].to_owned()
            } else {
                c.group.clone()
            };
            if !groups.contains(&g) {
                groups.push(g);
            }
        }
        let glyphs = Glyphs::default();
        let nodes = groups
            .iter()
            .map(|g| {
                let kids = self
                    .conns
                    .iter()
                    .filter(|c| {
                        if by_env {
                            ENVS[c.env] == g
                        } else {
                            &c.group == g
                        }
                    })
                    .map(|c| {
                        let env = env_kind(c.env);
                        TreeNode::leaf(&c.name)
                            .glyph(glyphs.env(env))
                            .glyph_tone(env.tone())
                            .meta(["pg", "mysql", "sqlite"][c.engine])
                    })
                    .collect();
                TreeNode::dir(g, kids)
            })
            .collect();
        let filter = self.tree.filter.clone();
        self.tree =
            TreeView::new(ID.sub("tree"), nodes).show_meta(self.opts.on("connections.meta"));
        self.tree.expand_all();
        self.tree.filter = filter;
        self.tree.flatten();
        let name = self.selected.map(|i| self.conns[i].name.clone());
        self.selected = None;
        if let Some(n) = name {
            self.select_name(&n);
        }
    }

    fn select_name(&mut self, name: &str) {
        let row = self.tree.rows().iter().position(|r| {
            r.path.len() == 2 && self.tree.node(&r.path).is_some_and(|n| n.label == name)
        });
        if let Some(row) = row {
            self.tree.cursor = row;
        }
        self.sync_selected();
    }

    fn sync_selected(&mut self) {
        let Some(row) = self.tree.rows().get(self.tree.cursor) else {
            return;
        };
        if row.path.len() != 2 {
            return;
        }
        let label = self.tree.node(&row.path.clone()).map(|n| n.label.clone());
        if let Some(i) = label.and_then(|l| self.conns.iter().position(|c| c.name == l)) {
            self.selected = Some(i);
        }
    }

    fn start_connect(&mut self, cx: &mut Cx) {
        let Some(i) = self.selected else {
            return;
        };
        let name = self.conns[i].name.clone();
        cx.status(format!("Connecting to {name}…"));
        self.state = ConnState::Connecting {
            name,
            left: CONNECT_TICKS,
        };
    }

    fn finish_connect(&mut self, name: String, cx: &mut Cx) {
        let Some(c) = self.conns.iter_mut().find(|c| c.name == name) else {
            self.state = ConnState::Idle;
            return;
        };
        self.state = match c.reach {
            Reach::Ok => {
                c.last_used = "just now".into();
                cx.status(format!("✓ Connected to {name}"));
                ConnState::Connected { name }
            }
            Reach::AuthFailed => ConnState::Failed {
                message: "Authentication failed".into(),
                detail: format!(
                    "FATAL: password authentication failed for user \"{}\" (SQLSTATE 28P01)",
                    c.user
                ),
                name,
            },
            Reach::Unreachable => ConnState::Failed {
                message: "Could not reach the host".into(),
                detail: format!(
                    "Timed out after 10 s ({}). The host may be behind a VPN.",
                    c.host
                ),
                name,
            },
        };
        if matches!(self.state, ConnState::Failed { .. }) {
            cx.error("Connection failed");
        }
    }

    fn form(&self, index: Option<usize>) -> FormDialog {
        let c = index.map(|i| self.conns[i].clone());
        let text = |f: fn(&Conn) -> String, default: &str| c.as_ref().map_or(default.to_owned(), f);
        let title = if index.is_some() {
            "Edit connection"
        } else {
            "New connection"
        };
        FormDialog::new(
            FORM,
            title,
            vec![
                FormField::input(
                    "name",
                    TextInput::new(FORM.sub("name"), "Name")
                        .value(&text(|c| c.name.clone(), ""))
                        .required(true),
                ),
                FormField::select(
                    "engine",
                    Select::new(
                        FORM.sub("engine"),
                        "Engine",
                        ENGINES,
                        c.as_ref().map_or(0, |c| c.engine),
                    ),
                ),
                FormField::input(
                    "host",
                    TextInput::new(FORM.sub("host"), "Host")
                        .value(&text(|c| c.host.clone(), "localhost:5432"))
                        .required(true),
                ),
                FormField::input(
                    "database",
                    TextInput::new(FORM.sub("database"), "Database")
                        .value(&text(|c| c.database.clone(), "")),
                ),
                FormField::select(
                    "env",
                    Select::new(
                        FORM.sub("env"),
                        "Environment",
                        ENVS,
                        c.as_ref().map_or(2, |c| c.env),
                    ),
                ),
                FormField::note("result", vec![]),
            ],
        )
        .action(
            "test",
            Button::secondary(FORM.sub("test"), "Test connection"),
        )
        .width(self.opts.number("connections.form_width").unwrap_or(60))
        .meta(c.as_ref().map_or("draft", |c| ENVS[c.env]))
    }

    fn open_form(&mut self, index: Option<usize>, cx: &mut Cx) {
        self.editing = Some(index);
        cx.open(self.form(index), LayerTag::new("form"));
    }

    fn save_form(&mut self, r: &Report, cx: &mut Cx) {
        let Some(index) = self.editing.take() else {
            return;
        };
        let choice = |name: &str| {
            r.values.as_ref().and_then(|v| {
                v.iter().find_map(|(k, f)| match f {
                    FieldValue::Choice(i) if k == name => Some(*i),
                    _ => None,
                })
            })
        };
        let name = r.text("name");
        let mut c = match index {
            Some(i) => self.conns[i].clone(),
            None => conn(&name, "Acme", 2, 0, "", "", Reach::Ok),
        };
        c.name = name.clone();
        c.host = r.text("host");
        c.database = r.text("database");
        c.engine = choice("engine").unwrap_or(c.engine);
        c.env = choice("env").unwrap_or(c.env);
        match index {
            Some(i) => self.conns[i] = c,
            None => {
                c.last_used = "never".into();
                self.conns.push(c);
            }
        }
        self.rebuild_tree();
        self.select_name(&name);
        cx.status(format!("Saved {name}"));
    }

    fn duplicate_selected(&mut self, cx: &mut Cx) {
        let Some(i) = self.selected else {
            return;
        };
        let mut c = self.conns[i].clone();
        let mut name = format!("{} copy", c.name);
        let mut n = 2;
        while self.conns.iter().any(|x| x.name == name) {
            name = format!("{} copy {n}", c.name);
            n += 1;
        }
        c.name = name.clone();
        c.last_used = "never".into();
        self.conns.insert(i + 1, c);
        self.rebuild_tree();
        self.select_name(&name);
        cx.status(format!("Duplicated as {name}"));
    }

    fn ask_delete(&mut self, cx: &mut Cx) {
        let Some(i) = self.selected else {
            return;
        };
        let c = &self.conns[i];
        cx.open_dialog(Dialog::destructive(
            DELETE_DLG,
            &format!("Delete {}?", c.name),
            "The saved connection and its keychain password are removed. Databases are not touched.",
            "Delete",
        ));
    }

    fn activate(&mut self, id: WidgetId, cx: &mut Cx) -> Outcome {
        if id == self.connect.id {
            self.start_connect(cx);
        } else if id == self.edit.id {
            self.open_form(self.selected, cx);
        } else if id == self.duplicate.id {
            self.duplicate_selected(cx);
        } else if id == self.delete.id {
            self.ask_delete(cx);
        } else if id == self.new.id || id == NEW_EMPTY {
            self.open_form(None, cx);
        } else {
            return Outcome::Ignored;
        }
        Outcome::Changed
    }

    fn empty(&self) -> bool {
        self.opts.value("connections.list") == "empty" || self.conns.is_empty()
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let l = self.md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.render_list(l.master, buf, ctx);
        }
        if !l.detail.is_empty() {
            self.render_detail(l.detail, buf, ctx);
        }
        if !l.summary.is_empty()
            && let Some(i) = self.selected.filter(|_| !self.empty())
        {
            let c = &self.conns[i];
            let line = format!(
                "→ {} · {} · {} · Tab shows details",
                c.name, ENVS[c.env], c.host
            );
            buf.set_string(
                l.summary.x + 1,
                l.summary.y,
                termrock::ui::text::truncate(&line, l.summary.width.saturating_sub(1) as usize),
                t.muted().bg(t.canvas),
            );
        }
        self.md.finish(ctx);
    }

    fn render_list(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let count = if self.empty() {
            "0".to_owned()
        } else {
            self.conns.len().to_string()
        };
        let panel = Panel::card(Some("Connections"))
            .meta(&count)
            .focused(ctx.interaction.focused(self.tree.id));
        let bg = panel.bg(t);
        let Some(inner) = panel.try_render(r, buf, t) else {
            return;
        };
        if self.empty() {
            let e = EmptyState::new("No connections")
                .hint("Save a database to open it in one step")
                .actions(vec![EmptyAction::primary(NEW_EMPTY, "New connection…")]);
            empty::render_ctx(inner, buf, ctx, &e, bg);
            return;
        }
        self.filter.render(
            Rect::new(inner.x.saturating_sub(1), inner.y, inner.width + 1, 2),
            buf,
            ctx,
            bg,
        );
        let tree = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(4),
        );
        self.tree.render(tree, buf, ctx, bg);
        let w = self.new.width().min(inner.width);
        self.new.render(
            Rect::new(inner.x, inner.bottom().saturating_sub(1), w, 1),
            buf,
            ctx,
            bg,
        );
    }

    fn render_detail(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let selected = if self.empty() { None } else { self.selected };
        let focused = ctx.interaction.focused(self.md.detail);
        let Some(i) = selected else {
            let panel = Panel::card(Some("Details")).focused(focused);
            let bg = panel.bg(t);
            if let Some(inner) = panel.try_render(r, buf, t) {
                DetailCard::empty(
                    EmptyState::new("Select a connection")
                        .hint("Its host, environment and safety settings show here"),
                )
                .render(inner, buf, ctx, bg, &mut []);
            }
            ctx.focus_stop(self.connect.id);
            return;
        };
        let c = self.conns[i].clone();
        let env = env_kind(c.env);
        let mut facts = vec![
            Prop::new("Engine", ENGINES[c.engine]),
            Prop::new("Host", c.host.clone()).copyable(),
            Prop::new("Database", c.database.clone()),
            Prop::new("User", c.user.clone()),
            Prop::new("Environment", ENVS[c.env]).tone(env.tone()),
        ];
        if env == EnvKind::Production {
            facts.push(
                Prop::new(
                    "Safe Mode",
                    "Writes ask for confirmation and a typed acknowledgement",
                )
                .tone(Tone::Warning)
                .wrap(),
            );
        }
        facts.push(if c.ssl {
            Prop::new("SSL", "✓ on").tone(Tone::Success)
        } else {
            Prop::new("SSL", "off").tone(Tone::Secondary)
        });
        if let Some(ssh) = &c.ssh {
            facts.push(Prop::new("SSH", ssh.clone()).tone(Tone::Secondary));
        }
        facts.push(Prop::new("Last used", c.last_used.clone()).tone(Tone::Muted));
        if matches!(&self.state, ConnState::Connected { name } if *name == c.name) {
            facts.insert(
                0,
                Prop::new("Status", "✓ Connected · PostgreSQL 16.3 · 12 ms").tone(Tone::Success),
            );
        }
        let busy = matches!(self.state, ConnState::Connecting { .. });
        self.connect.disabled = busy;
        let failure = match &self.state {
            ConnState::Failed {
                name,
                message,
                detail,
            } if *name == c.name => Some(Notice::error(message.as_str()).detail(detail.as_str())),
            _ => None,
        };

        self.connect.label = if failure.is_some() {
            "Reconnect".into()
        } else {
            "Connect".into()
        };
        let connecting =
            matches!(&self.state, ConnState::Connecting { name, .. } if *name == c.name);

        let state_h = match &failure {
            Some(n) => n.height(r.width.saturating_sub(4)) + 1,
            None if connecting => 2,
            None => 0,
        };
        let card = DetailCard::new(&c.name)
            .meta(ENVS[c.env])
            .props(facts)
            .focused(focused)
            .fit(true)
            .reserve(state_h + 2)
            .actions(ActionRow::new());
        let l = card.render(
            r,
            buf,
            ctx,
            t.canvas,
            &mut [
                &mut self.connect,
                &mut self.edit,
                &mut self.duplicate,
                &mut self.delete,
            ],
        );
        if connecting {
            render_indeterminate(
                Rect::new(l.slot.x, l.slot.y, l.slot.width, 1),
                buf,
                ctx,
                "Connecting  ",
                l.bg,
            );
        } else if let Some(n) = &failure {
            n.render(l.slot, buf, ctx, l.bg);
        }
    }

    fn buttons(&mut self) -> [&mut Button; 5] {
        [
            &mut self.connect,
            &mut self.edit,
            &mut self.duplicate,
            &mut self.delete,
            &mut self.new,
        ]
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if let Some(r) = self.filter.route(ev) {
            if click {
                cx.focus.focus(self.filter.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.tree.route(ev) {
            if click && self.tree.row_at(ev).is_some() {
                cx.focus.focus(self.tree.id);
            }
            let (o, actions) = r.into_parts();
            self.sync_selected();
            if actions
                .into_iter()
                .any(|a| matches!(a, TreeAction::Activate(_)))
            {
                self.start_connect(cx);
                return Outcome::Changed;
            }
            return o;
        }
        let mut hit = None;
        for b in self.buttons() {
            if let Some(r) = b.route(ev) {
                hit = Some((b.id, r.outcome(), r.has_actions()));
                break;
            }
        }
        if let Some((id, o, act)) = hit {
            if click {
                cx.focus.focus(id);
            }
            if act {
                return self.activate(id, cx);
            }
            return o;
        }

        if click && ev.on(NEW_EMPTY) {
            return self.activate(ev.id, cx);
        }
        Outcome::Ignored
    }

    fn key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        let Some(f) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        if f == self.filter.id {
            let (o, a) = self.filter.on_key(key);
            match a {
                Some(InputAction::Changed) => {
                    let q = self.filter.text().to_owned();
                    self.tree.set_filter((!q.is_empty()).then_some(q.as_str()));
                    self.sync_selected();
                }
                Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                _ => {}
            }
            return o;
        }
        if f == self.tree.id {
            let (o, actions) = self.tree.on_key(key).into_parts();
            self.sync_selected();
            if actions
                .into_iter()
                .any(|a| matches!(a, TreeAction::Activate(_)))
            {
                self.start_connect(cx);
                return Outcome::Changed;
            }
            return o;
        }
        if f == NEW_EMPTY {
            let (o, act) = Button::secondary(f, "").on_key(key);
            if act {
                return self.activate(f, cx);
            }
            return o;
        }
        let mut hit = None;
        for b in self.buttons() {
            if b.id == f {
                hit = Some(b.on_key(key));
                break;
            }
        }
        match hit {
            Some((_, true)) => self.activate(f, cx),
            Some((o, false)) => o,
            None => Outcome::Ignored,
        }
    }

    fn tick(&mut self, cx: &mut Cx) -> Outcome {
        let ConnState::Connecting { name, left } = &mut self.state else {
            return Outcome::Ignored;
        };
        *left = left.saturating_sub(1);
        if *left == 0 {
            let name = name.clone();
            self.finish_connect(name, cx);
        }
        Outcome::Changed
    }

    fn apply_state_option(&mut self) {
        let Some(i) = self.selected else {
            return;
        };
        let name = self.conns[i].name.clone();
        self.state = match self.opts.value("connections.state") {
            "connecting" => ConnState::Connecting {
                name,
                left: CONNECT_TICKS,
            },
            "connected" => ConnState::Connected { name },
            "failed" => ConnState::Failed {
                name,
                message: "Could not reach the host".into(),
                detail: "Timed out after 10 s. The host may be behind a VPN.".into(),
            },
            _ => ConnState::Idle,
        };
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let glyphs = Glyphs::default();
    let leaf = |name: &str, env: EnvKind, meta: &str| {
        TreeNode::leaf(name)
            .glyph(glyphs.env(env))
            .glyph_tone(env.tone())
            .meta(meta)
    };
    let mut tree = TreeView::new(
        id.child(0),
        vec![TreeNode::dir(
            "Acme",
            vec![
                leaf("Staging", EnvKind::Staging, "pg"),
                leaf("Production", EnvKind::Production, "pg"),
            ],
        )],
    );
    tree.expand_all();
    StateGrid::new(id)
        .rows(&[
            Rest, Hover, Pressed, Focused, Selected, Disabled, Busy, Narrow,
        ])
        .column_width(24)
        .column("Connections", tree)
        .column("Connect", Button::primary(id.child(1), "Connect"))
        .column("Delete…", Button::danger(id.child(2), "Delete…"))
}

impl Page for ConnectionsPage {
    fn title(&self) -> &'static str {
        "Connections"
    }
    fn blurb(&self) -> &'static str {
        "Grouped connections with env glyphs, details, connect and the form"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            return self.tick(cx);
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev
            && *id == DELETE_DLG
        {
            if *result == DialogResult::Action(1)
                && let Some(i) = self.selected.take()
            {
                let name = self.conns.remove(i).name;
                self.state = ConnState::Idle;
                self.rebuild_tree();

                if let Some(next) = self.conns.get(i.min(self.conns.len().saturating_sub(1))) {
                    let next = next.name.clone();
                    self.select_name(&next);
                }
                cx.status(format!("Deleted {name}"));
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Blur(ch) => forward_blur(*ch, &mut [&mut self.filter]),
            PageEvent::Key(key) => {
                if let Some(o) = self.md.on_key(key, cx.focus) {
                    return o;
                }
                self.key(key, cx)
            }
            PageEvent::Paste(text) if cx.focus.is(self.filter.id) => self.filter.on_paste(text),
            PageEvent::Pointer(p) => {
                if let Some(o) = self.md.on_pointer(p) {
                    return o;
                }
                self.pointer(p, cx)
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_layer(&mut self, _tag: &LayerTag, report: Report, cx: &mut Cx) -> Outcome {
        match &report.result {
            Some(LayerResult::Form(FormAction::Save)) => self.save_form(&report, cx),
            Some(LayerResult::Form(FormAction::Cancel)) => self.editing = None,
            Some(LayerResult::Form(FormAction::Action(name))) if name == "test" => {
                cx.with_top::<FormDialog>(|f| {
                    let host = f.text("host");
                    let line = if host.trim().is_empty() {
                        ("✗ Host is required".to_owned(), Tone::Error)
                    } else if host.contains("analytics") {
                        ("✗ Timed out after 10 s".to_owned(), Tone::Error)
                    } else {
                        ("✓ Connected in 12 ms".to_owned(), Tone::Success)
                    };
                    f.set_note("result", vec![line]);
                });
            }
            _ => {}
        }
        if report.closed.is_some() {
            self.editing = None;
        }
        Outcome::Changed
    }

    fn editing(&self) -> bool {
        self.filter.editing
    }

    fn animating(&self) -> bool {
        matches!(self.state, ConnState::Connecting { .. })
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        let v = match focus {
            Some(f) if f == self.tree.id => {
                vec![("↑↓", "Move"), ("Enter", "Connect"), ("Tab", "Next")]
            }
            _ => vec![("Enter", "Activate"), ("Tab", "Next")],
        };
        let mut v = v;
        v.extend(
            self.md
                .hints(focus)
                .into_iter()
                .map(crate::pages::master_detail::page_hint),
        );
        v
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            if key == "connections.list_width" {
                self.md = master_detail(self.list_share());
            }
            self.rebuild_tree();
            if key == "connections.state" {
                self.apply_state_option();
            }
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

fn master_detail(share: u16) -> MasterDetail {
    MasterDetail::new(ID.sub("tree"), ID.sub("connect"))
        .detail_owns(&[ID.sub("edit"), ID.sub("duplicate"), ID.sub("delete-btn")])
        .resizable(Split::new(share, 24, 44))
        .collapse_below(80)
        .summary_rows(1)
}
